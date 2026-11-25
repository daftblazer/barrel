//! The field's steady HUD, from the game's own layouts: the minimap (`uil_a21_fm_minimap.xml`:
//! grid, terrain, her arrow, the glass layer, the frame and the north mark), the basket's count
//! (the same file's `cage`), the clock (`uil_a21_time_display.xml`) and the equipped tool's
//! disc (`uil_a21_collect_item.xml`), with the styles of `uis_gen_a21_field.xml`,
//! `uis_gen_a21_icon.xml`, `uis_gen_a21v2_common.xml` and `uis_gen_a21v2_various.xml`.
//!
//! Positions are in the 1280 x 720 layout and scale by `screen height / 720`, the layout
//! centred across the width, as the title's are. The two popups of the field are here too: the
//! area name's banner on arrival (`uil_a21_fm01.xml`, `current_place`) and the button help bar
//! along the bottom (`uil_a21_shade.xml`, as the load screen's). Left out (docs/ryza/open.md,
//! "Field HUD"): the messages' popups.

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Clip, Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;
use crate::load_view::{inside, style, Style, TEXT_TOP};
use crate::title::{curve, find, numbers, pair, Curve, LAYOUT_HEIGHT};

mod arrival;
mod gather;
use arrival::Arrival;
pub use arrival::SOUND_AT as MARQUEE_SOUND_AT;
pub use gather::{Found, Gathering, HAND_NOTICE_AT, STAFF_LEAVE_AT, STAFF_NOTICE_AT};
use gather::Popups;

/// The textures of the HUD's layer, in this order.
pub const FONT01: usize = 0;
pub const FONT00: usize = 1;
pub const FIELD: usize = 2;
pub const MINIMAP: usize = 3;
pub const COMMON: usize = 4;
pub const ICON: usize = 5;
pub const VARIOUS: usize = 6;
/// The glows of the minimap's markers (`a21_field.g1t#1`).
pub const GLOW: usize = 7;
/// The help bar's plate (`a21_common.g1t#3`).
pub const GUIDE: usize = 8;
/// The marquee's mask (`a21_battle.g1t#4`) and the autosave mark's orbs (`a21v2_saveload.g1t#0`).
pub const BATTLE: usize = 9;
pub const SAVELOAD: usize = 10;
/// The gimmicks' icon balloon (`a21_field.g1t#0`).
pub const BALLOON: usize = 11;
/// How many textures the layer has.
pub const TEXTURES: usize = 12;
/// The textures of the gathered items' pictures, when the layer has them (after the others), two for
/// each item in the order gathered: the small picture (`a21_item_s.g1t`, the item's own texture) and
/// the large one (`a21_item_l_<n>.g1t`).
pub const ITEM_PICTURES: usize = 12;

/// The layer's texture of the `k`th gathered item's small or large picture.
pub const fn item_texture(k: usize, large: bool) -> usize {
    ITEM_PICTURES + 2 * k + large as usize
}

/// The styles' pixels count three to a layout pixel (`load_view::ATLAS_PER_LAYOUT`).
const PER_LAYOUT: f32 = 3.0;
/// The first of the string table's UI strings (`STR_UI_0000`, as `load_view`).
const STR_UI_BASE: u32 = 6_619_137;
/// The string table's area names ("Kurken Island", ...) and map names ("Rasen District", ...),
/// each a list from its first number.
const AREA_NAMES: u32 = 2_228_225;
const MAP_NAMES: u32 = 917_505;
/// How many strings of each list are kept.
const NAMES: u32 = 64;
/// The menu words' strings (`str_menu_define`) the help bar reads, from the first to this many on.
const MENU_WORDS: u32 = 4_194_304;
const MENU_WORDS_SPAN: u32 = 512;

// STAND-IN: The help bar's button glyphs and the "Photo Mode" string are the capture's (the game picks the pictures for the pad's assignment; three strings of the table read "Photo Mode", which one the game takes is not checked); the other words are the table's own.
/// The help bar's entries in the order the game lists them: the button glyph (a private-use
/// character of `mainfont01`) and the words' string number.
pub(crate) const HELP_ENTRIES: [(char, u32); 7] = [
    ('\u{4f0}', 4_194_353),
    ('\u{4f1}', 4_194_351),
    ('\u{4e7}', 4_194_338),
    ('\u{4f4}', 720_900),
    ('\u{4eb}', 4_194_451),
    ('\u{4dc}', 4_194_341),
    ('\u{4de}', 4_194_559),
];

/// The frames the area name takes to fade in and out come from the layout's animation
/// (`anm_common_juvenile_fade_in`, 60 frames); it holds this many frames from the arrival before
/// it fades out (`hud-walk`: in from 1728, out from 1879, the map name's plate 30 frames
/// behind: the layout's `delay` of 0.5 s).
const AREA_OUT_AT: f32 = 151.0;

/// The frames the stick must have been at rest before the help bar comes up (`hud-walk`: the
/// fade-in's first step falls 121 frames after the stick's last frame, three times over).
const HELP_IDLE: f32 = 120.0;
/// Frames the help bar takes to fade in or out (`anm_common_fade_in`, as the load screen's).
const HELP_FADE: f32 = 15.0;
// STAND-IN: The idle count at the arrival is the capture's (the bar's first step falls 6 frames after the area name's); the real thing is the game's own count from the load.
/// Frames of rest the count has at the arrival.
const HELP_IDLE_AT_ARRIVAL: f32 = 114.0;
// STAND-IN: The first appearance's plate grow is fitted to the three frames the capture has of it (1740: the shortest plate, 1750: 0.375 of the way, 1760: full); the real thing is the layout's own size tween.
/// The frames from the bar's first showing the plate starts to grow after, and takes.
const HELP_GROW_FROM: f32 = 11.0;
const HELP_GROW_FRAMES: f32 = 16.0;
// STAND-IN: The short line's switch to the full one falls with the plate's grow start; the capture has it at 1740 (short, 7 frames in) and 1750 (full, 17 frames in).
/// The entries (of `HELP_ENTRIES`) the bar shows when it first comes up, before the full line.
const HELP_FIRST: [usize; 2] = [2, 5];
/// The entry the bar drops once she has swung the staff (`gather-rasen2` 3450 on: the line is without its sixth entry, the staff's hint
/// of the X button; the bar of 2591 to 2860, before the swing, has it).
// STAND-IN: that the hint goes with the first swing (rather than with the first gathering by it) is read off the one capture, whose
// first swing was a gathering; the game's own flag is not read.
const STAFF_HINT: usize = 5;
/// The returning bar's plate takes these frames to grow and is this far on at the fade's first frame (the menu's plate frames,
/// fitted on 2575 and 2580), and its fade starts this many frames after an arrival's would.
const BACK_GROW_FRAMES: f32 = 6.4;
const BACK_GROW_LEAD: f32 = 0.4;
const BACK_IDLE_LATER: f32 = 1.0;
/// The bar after the opening: its fade starts this many frames after the field's first (`opening-key4`: 14865 on 14862),
/// its plate has grown this many frames by then and takes this many frames in all.
const OPENING_BAR_LATE: f32 = 3.0;
const OPENING_GROW_LEAD: f32 = 5.03;
const OPENING_GROW_FRAMES: f32 = 12.03;
/// The plate's shortest width in layout pixels (376.5 px of the capture at 1.5 px a layout pixel).
const HELP_PLATE_MIN: f32 = 376.5 / 1.5;

/// The strings of the area name and the map name a field announces on arrival: their numbers in
/// the string table.
// STAND-IN: Only Rasenboden's is read (its area is the first of the list, its map the sixth, off `hud-walk`). The game takes them from a record per field (the info object at 0x141a940f0: records of 72 bytes, the area's index at +0 and the map name's string number at +0x38, the areas a list of 8-byte records with the name's number at +0), built at run time from a file not found in the archives' tables.
pub fn place_names(field: &str) -> Option<[u32; 2]> {
    (field == MAP_FIELD).then_some([AREA_NAMES, MAP_NAMES + 5])
}

/// How opaque a banner plate is `age` frames after the arrival: it fades in by the layout's
/// animation, holds, and fades out the same animation backwards, in whole 1/255 steps as the
/// game's colour bytes. `delay` frames late for the map's plate.
fn banner_alpha(fade: &Curve, age: f32, delay: f32) -> f32 {
    let t = age - delay;
    let along = if t < 0.0 {
        return 0.0;
    } else if t < AREA_OUT_AT {
        t / 60.0
    } else {
        fade.duration() - (t - AREA_OUT_AT) / 60.0
    };
    fade.at(along.max(0.0))[3].round() / 255.0
}

/// The help bar's state, stepped every frame: it comes up after the move stick has rested for
/// `HELP_IDLE` frames and goes at the stick's first frame, fading `HELP_FADE` frames each way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HelpBar {
    idle: f32,
    alpha: f32,
    /// Frames since the bar first showed (its plate grows on the first appearance only).
    shown: f32,
    /// A bar that shows its full line at once and grows its plate by its own rule, the frames
    /// before the fade the growth has run and the frames it takes (the menu's return, the
    /// arrival after the opening); `None` for the first showing's short line and slow grow.
    quick: Option<(f32, f32)>,
    /// Frames since the bar last began to show, and whether its line had the staff's hint when it was last shown and when this showing
    /// began (a changed line has the plate tween from the old line's width, `help_bar`).
    lit: f32,
    hint_last: bool,
    hint_from: bool,
}

impl HelpBar {
    /// The bar at a field's arrival.
    pub fn arriving() -> HelpBar {
        HelpBar { idle: HELP_IDLE_AT_ARRIVAL, alpha: 0.0, shown: 0.0, quick: None, lit: 0.0, hint_last: true, hint_from: true }
    }

    /// The bar at the arrival after the opening: no rest to wait for, it fades in from the third frame (`opening-key4`: the
    /// first plate on 14865, the field's first frame 14862) with its full line, and its plate grows linearly over 12 frames
    /// from 5 frames before the fade's first (half grown on it, whole on 14871).
    // STAND-IN: that the bar needs no rest here, and the plate's grow, are read off the key's frames 14865 to 14871; the game's own count at a new game's arrival is not read.
    pub fn arriving_at_once() -> HelpBar {
        HelpBar { idle: HELP_IDLE - OPENING_BAR_LATE, quick: Some((OPENING_GROW_LEAD, OPENING_GROW_FRAMES)), ..HelpBar::arriving() }
    }

    /// The bar as the field comes back from the menu: it fades in a frame later than at an
    /// arrival, with its full line (it has shown before) and a plate that grows over `BACK_GROW_FRAMES`,
    /// from 0.4 frames before the fade (`field-menu` 2575 to 2585: opacity 2/15 and the plate 0.375 of
    /// the way on 2575, the whole plate on 2580).
    pub fn returning() -> HelpBar {
        HelpBar { idle: HELP_IDLE_AT_ARRIVAL - BACK_IDLE_LATER, quick: Some((BACK_GROW_LEAD, BACK_GROW_FRAMES)), ..HelpBar::arriving() }
    }

    /// Moves on `frames` frames (60 a second), the move stick out of its dead zone or not.
    pub fn step(&mut self, stick: bool, frames: f32) {
        self.step_swung(stick, frames, false);
    }

    /// As `step`, with whether she has swung the staff (the line then leaves out its hint: `STAFF_HINT`).
    pub fn step_swung(&mut self, stick: bool, frames: f32, swung: bool) {
        self.idle = if stick { 0.0 } else { self.idle + frames };
        let toward = if self.idle > HELP_IDLE { frames } else { -frames } / HELP_FADE;
        let was = self.alpha > 0.0;
        self.alpha = (self.alpha + toward).clamp(0.0, 1.0);
        if self.alpha > 0.0 {
            self.shown += frames;
            if !was {
                self.lit = 0.0;
                self.hint_from = self.hint_last;
            }
            self.lit += frames;
            self.hint_last = !swung;
        }
    }

    /// How opaque the bar is.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Whether the bar still shows its first, short line (the first frames of its first showing).
    fn first_line(&self) -> bool {
        self.quick.is_none() && self.shown < HELP_GROW_FROM
    }

    /// How far the plate has grown from its shortest to its full width, 0 to 1.
    fn grow(&self) -> f32 {
        if let Some((lead, frames)) = self.quick {
            return ((self.shown + lead) / frames).clamp(0.0, 1.0);
        }
        ((self.shown - HELP_GROW_FROM) / HELP_GROW_FRAMES).clamp(0.0, 1.0)
    }
}

/// A text node's layout settings.
#[derive(Clone, Copy, Debug)]
struct Label {
    at: [f32; 2],
    size: f32,
    small: bool,
    center: bool,
    right: bool,
    /// The node's colour, if it sets one.
    color: Option<[f32; 4]>,
    /// Layout pixels added to each glyph's advance (`word_space`).
    spacing: f32,
}

// STAND-IN: The basket's count (225 in the capture) is the capture's; where the game takes it from is not read.
/// The basket's count shown.
pub const BASKET: u32 = 225;
/// The one field whose minimap picture and scale are read (`a21_minimap_image_006`).
pub const MAP_FIELD: &str = "s06_t_rasen";

/// What the HUD shows of the play.
#[derive(Clone, Debug)]
pub struct State {
    /// Her feet in game units (x not mirrored).
    pub at: [f32; 3],
    /// Her turn in degrees about y, 0 toward +z.
    pub turn: f32,
    /// The hour on the clock, 0 to 23.
    pub hour: u32,
    /// The basket's count.
    pub basket: u32,
    /// The equipped tool (its place in the tool pictures).
    pub tool: usize,
    /// Whether the field has a minimap picture read (`Hud::map`).
    pub map: bool,
    /// The camera's yaw in degrees (the game's: 0 looking along +z) when the Map Rotation row is
    /// On, so the minimap turns with it; `None` draws it fixed-north.
    pub camera_yaw: Option<f32>,
    /// The frames the HUD has been up (60 a second), which the destination's glow pulses by.
    pub tick: f32,
    /// The clock the looping glows run by: `tick`, less the frames the game's animations lost (a capture's replay lists them).
    pub anim: f32,
    /// The frames since the field's arrival, which the area name's banner runs by.
    pub age: f32,
    /// The area name's and the map name's string numbers (`place_names`), if the field has them.
    pub place: Option<[u32; 2]>,
    /// The help bar.
    pub help: HelpBar,
    /// Whether this is the arrival after the opening, with its popups (the story marquee and the
    /// autosave mark; `age` runs their clocks).
    pub arrival: bool,
    /// The gathering's popups.
    pub gather: Gathering,
    /// The gimmicks' icons over the field (`world_mark`), in the camera's view.
    pub marks: Vec<WorldMark>,
    /// Whether she has swung the staff (the help bar then leaves out its hint, `STAFF_HINT`).
    pub swung: bool,
}

/// What the minimap marks, as places in the field (game units, x then z).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Markers {
    /// The ways out.
    pub exits: Vec<[f32; 2]>,
    /// The townsfolk.
    pub folk: Vec<[f32; 2]>,
    /// The fishing places and the water to gather from.
    pub fishing: Vec<[f32; 2]>,
    pub water: Vec<[f32; 2]>,
    /// Where the story goes next, if it is in this field: a place, or the way out toward it.
    pub destination: Option<[f32; 2]>,
    /// The doors into buildings (exits to an interior), which float an icon over them in the field (game units, x y z).
    pub doors: Vec<[f32; 3]>,
}

pub struct Sources<'a> {
    pub minimap: &'a [u8],
    pub time: &'a [u8],
    pub collect: &'a [u8],
    pub field_styles: &'a [u8],
    pub icon_styles: &'a [u8],
    pub common_styles: &'a [u8],
    pub various_styles: &'a [u8],
    pub strings: &'a [u8],
    /// `uil_a21_fm01.xml` (the area name's banner), `uia_common.xml`, the help bar's
    /// `uil_a21_shade.xml` and `uis_gen_a21_common.xml` (its plate).
    pub places: &'a [u8],
    pub anims: &'a [u8],
    pub help: &'a [u8],
    pub guide_styles: &'a [u8],
    /// `uis_gen_a21_battle.xml` (the marquee's mask), `uil_a21_autosave.xml` (the autosave mark),
    /// `uis_gen_a21v2_saveload.xml` (its orbs) and `uia_a21_saveload.xml` (their glows).
    pub battle_styles: &'a [u8],
    pub autosave: &'a [u8],
    pub spinner_styles: &'a [u8],
    pub saveload_anims: &'a [u8],
    pub glyphs: [HashMap<char, Glyph>; 2],
    /// Each texture's size in pixels, in the layer's order after the two fonts.
    pub sizes: [(u32, u32); TEXTURES - 2],
    /// `uia_a21_fm01.xml` (the item notices' fades).
    pub field_anims: &'a [u8],
    /// `uil_a21_fm_advertise01.xml` (the gimmicks' icon over the field).
    pub advertise: &'a [u8],
    /// The sizes of a gathered item's small and large pictures' textures (the same for every item), when the layer has them.
    pub item_sizes: Option<[(u32, u32); 2]>,
}

pub struct Hud {
    glyphs: [HashMap<char, Glyph>; 2],
    atlases: [(f32, f32); TEXTURES - 2],
    strings: HashMap<u32, String>,
    // The minimap.
    grid: ([f32; 4], [f32; 2]),
    mask: ([f32; 4], [f32; 2], f32),
    gmd: ([f32; 4], [f32; 2]),
    frame: ([f32; 4], [f32; 2]),
    north: ([f32; 4], [f32; 2]),
    arrow: [f32; 4],
    exit_icon: [f32; 4],
    folk_icon: [f32; 4],
    fishing_icon: [f32; 4],
    water_icon: [f32; 4],
    destination_icon: [f32; 4],
    /// The destination's two glows (`..._eff_01` and `_eff_02`).
    glows: [[f32; 4]; 2],
    // The basket.
    base: ([f32; 4], [f32; 2], f32),
    basket_icon: ([f32; 4], [f32; 2], f32),
    count_base: ([f32; 4], [f32; 2], f32, [f32; 2], [f32; 2]),
    count: Label,
    count_origin: [f32; 2],
    // The tool disc.
    disc: ([f32; 4], [f32; 2]),
    /// The highlight of a point the tool reaches (`ok_corsol`'s picture and its pulsing copy's).
    ring: [f32; 4],
    ring_glow: [f32; 4],
    disc_mask: ([f32; 4], f32),
    tools: Vec<[f32; 4]>,
    tool_at: [f32; 2],
    tool_size: f32,
    // The clock.
    clock_base: ([f32; 4], [f32; 2]),
    clock_ring: [f32; 4],
    clock_icons: Vec<[f32; 4]>,
    clock_icon_at: [f32; 2],
    date_base: ([f32; 4], [f32; 2]),
    am: Label,
    pm: Label,
    time: Label,
    time_origin: [f32; 2],
    // The area name's banner: the plates and their texts, the fade and the second plate's delay.
    area: ([f32; 4], [f32; 2], Label),
    map: ([f32; 4], [f32; 2], Label),
    fade: Curve,
    map_delay: f32,
    // The help bar.
    help: Style,
    help_at: [f32; 2],
    help_text: Label,
    help_left: f32,
    // The gimmicks' icon over the field.
    balloon: Balloon,
    // The arrival's popups.
    arrival: Option<Arrival>,
    // The gathering's popups, and the atlases of the item pictures.
    popups: Option<Popups>,
    item_atlases: Option<[(f32, f32); 2]>,
}

/// The icon the game floats over a gimmick (`fm_icon_gimmick` of `uil_a21_fm_advertise01.xml`): a balloon with the gimmick's
/// picture in it, laid out in world units around the node's origin.
struct Balloon {
    /// The balloon's picture (`gen_a21v2_field01_mini_balloon`) and its place from the origin.
    picture: [f32; 4],
    at: [f32; 2],
    /// The gimmick picture (`gen_a21_icon01_gimmick`'s `image_no`), its place in the balloon, its size and its tint.
    icon: [f32; 4],
    icon_at: [f32; 2],
    icon_size: f32,
    icon_color: [f32; 4],
    /// The node's scale (the layout keeps it at one half).
    scale: f32,
}

/// Where a gimmick's icon stands in the view: the node's origin on the screen and what a world unit measures there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldMark {
    /// The origin in screen pixels.
    pub at: [f32; 2],
    /// Pixels a world unit measures at the origin's depth.
    pub per_unit: f32,
    /// The icon's scale by its distance from the eye (`MARK_FAR`).
    pub near: f32,
}

/// How far above the gimmick the icon's origin floats (world units).
// STAND-IN: fitted to the capture (`gather-rasen2` 3130, the door at fixID 13: 49.75 to 50 units); the game takes it from its per-type
// table at run time (`0x141a94948` + 0x64), which is not read.
const MARK_LIFT: f32 = 50.0;
/// From this distance to the eye the icon is its full size; nearer it shrinks by `0.1 + 0.9 * d / 1000` (`0x14049d6ca` to `0x14049d734`:
/// the constants 1e6 for d squared, 1000, 0.9 and 0.1).
const MARK_FAR: f32 = 1000.0;
/// The UI's own camera looks with a vertical field of view of 45 degrees whatever the scene's: the balloon's width in the capture is
/// its 316 style pixels, three to a layout pixel, times the node's one half (52.67 units), over the focal length of 45 degrees.
const MARK_FOV: f32 = 45.0;
/// How far from the eye an icon is still there.
// STAND-IN: the capture's door shows from 1,244 units off (`gather-rasen2` 3130) and not from 2,710 (`hud-walk` 2750, in front of her); the
// game's own limit (its list of the gimmicks near her) is not read, and 2,000 is taken between them.
const MARK_RANGE: f32 = 2000.0;

/// Where the icon over the gimmick at `door` (game units) is in the view `vp` (render space, x mirrored) with the eye at `eye`,
/// or `None` where the origin is behind the eye.
pub fn world_mark(vp: &glam::Mat4, eye: glam::Vec3, door: [f32; 3], screen: (u32, u32)) -> Option<WorldMark> {
    let render = glam::Vec3::new(-door[0], door[1], door[2]);
    let c = *vp * glam::Vec4::new(render.x, render.y + MARK_LIFT, render.z, 1.0);
    if c.w <= 0.0 {
        return None;
    }
    let (w, h) = (screen.0 as f32, screen.1 as f32);
    let at = [(c.x / c.w * 0.5 + 0.5) * w, (c.y / c.w * 0.5 + 0.5) * h];
    let per_unit = h * 0.5 / (MARK_FOV.to_radians() * 0.5).tan() / c.w;
    let far = (eye - render).length();
    if far > MARK_RANGE {
        return None;
    }
    let near = if far >= MARK_FAR { 1.0 } else { 0.1 + 0.9 * far / MARK_FAR };
    Some(WorldMark { at, per_unit, near })
}

fn node<'a>(layout: &'a Element, name: &str) -> Result<(&'a Element, [f32; 2]), String> {
    find(layout, name, [0.0; 2]).ok_or_else(|| format!("layout: no node {name}"))
}

/// The pictures of a style: one, or each of a list's.
fn pictures(styles: &Element, name: &str) -> Result<Vec<[f32; 4]>, String> {
    let one = |e: &Element| -> Option<[f32; 4]> { numbers(e, "uvwh").try_into().ok() };
    let e = styles.children.iter().find(|e| e.attr("name") == Some(name)).ok_or(format!("no style {name}"))?;
    Ok(match e.name.as_str() {
        "image_list" => e.children.iter().filter_map(one).collect(),
        _ => one(e).into_iter().collect(),
    })
}

fn picture(styles: &Element, name: &str) -> Result<[f32; 4], String> {
    pictures(styles, name)?.first().copied().ok_or(format!("style {name} has no picture"))
}

fn label(e: &Element, at: [f32; 2]) -> Label {
    let rgba = numbers(e, "color");
    Label {
        at,
        size: e.attr("font_size").and_then(|v| v.trim().parse().ok()).unwrap_or(17.0),
        small: e.attr("font") == Some("MainFont01"),
        center: e.attr("align") == Some("center"),
        right: e.attr("align") == Some("right"),
        color: (rgba.len() == 4).then(|| [rgba[0] / 255.0, rgba[1] / 255.0, rgba[2] / 255.0, rgba[3] / 255.0]),
        spacing: 0.0,
    }
}

impl Hud {
    pub fn read(src: Sources) -> Result<Hud, String> {
        let minimap = table::parse(src.minimap).map_err(|e| format!("uil_a21_fm_minimap: {e}"))?;
        let time = table::parse(src.time).map_err(|e| format!("uil_a21_time_display: {e}"))?;
        let collect = table::parse(src.collect).map_err(|e| format!("uil_a21_collect_item: {e}"))?;
        let field = table::parse(src.field_styles).map_err(|e| format!("uis_gen_a21_field: {e}"))?;
        let icons = table::parse(src.icon_styles).map_err(|e| format!("uis_gen_a21_icon: {e}"))?;
        let common = table::parse(src.common_styles).map_err(|e| format!("uis_gen_a21v2_common: {e}"))?;
        let various = table::parse(src.various_styles).map_err(|e| format!("uis_gen_a21v2_various: {e}"))?;
        let strings_root = table::parse(src.strings).map_err(|e| format!("strcombineall: {e}"))?;
        let strings = strings_root
            .children
            .iter()
            .filter_map(|s| Some((s.attr("String_No")?.parse::<u32>().ok()?, s.attr("Text")?.to_string())))
            .filter(|(n, _)| {
                (STR_UI_BASE..STR_UI_BASE + 1000).contains(n)
                    || (AREA_NAMES..AREA_NAMES + NAMES).contains(n)
                    || (MAP_NAMES..MAP_NAMES + NAMES).contains(n)
                    || (MENU_WORDS..MENU_WORDS + MENU_WORDS_SPAN).contains(n)
                    || HELP_ENTRIES.iter().any(|(_, w)| w == n)
                    || [arrival::ADDED_LINE, arrival::STORY_TITLE, gather::PROMPT_WORD, gather::SWING_WORD].contains(n)
                    || (crate::item_info::NAME_BASE..crate::item_info::NAME_BASE + 1000).contains(n)
            })
            .collect();
        let places = table::parse(src.places).map_err(|e| format!("uil_a21_fm01: {e}"))?;
        let help_layout = table::parse(src.help).map_err(|e| format!("uil_a21_shade: {e}"))?;
        let guide = table::parse(src.guide_styles).map_err(|e| format!("uis_gen_a21_common: {e}"))?;
        let scale_of = |e: &Element| pair(e, "scale", [1.0; 2])[0];
        let style_of = |e: &Element| e.attr("style").map(str::to_string).ok_or_else(|| "a picture without a style".to_string());

        // The minimap: the pictures hang on the `minimap` node (its own place added up by `find`).
        let (grid_e, grid_at) = node(&minimap, "map_bg")?;
        let (mask_e, mask_at) = node(&minimap, "alpha_mask")?;
        let (gmd_e, gmd_at) = node(&minimap, "map_layer")?;
        let (frame_e, frame_at) = node(&minimap, "map_frame")?;
        let (north_e, north_at) = node(&minimap, "map_n")?;
        let (player_e, _) = node(&minimap, "icon_player_img")?;
        let arrow_no = player_e.attr("image_no").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
        let arrow = *pictures(&field, &style_of(player_e)?)?.get(arrow_no).ok_or("no player arrow picture")?;
        // The markers' pictures: a node's style and its `image_no` in the style's list.
        let icon = |name: &str| -> Result<[f32; 4], String> {
            let (e, _) = node(&minimap, name)?;
            let no = e.attr("image_no").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
            pictures(&field, &style_of(e)?)?.get(no).copied().ok_or(format!("{name}: no picture {no}"))
        };
        // The basket (the layout's `cage`; `find` adds its place to its children's).
        let (cbase_e, cbase_at) = node(&minimap, "base")?;
        let (cage, _) = node(&minimap, "cage")?;
        let (icon_e, icon_at) = node(cage, "icon")?;
        let (num_base, num_at) = node(cage, "num_base")?;
        let (num_bg, _) = node(num_base, "base")?;
        let (num_e, _) = node(num_base, "num")?;
        // The tool disc: the select copy of `equip_icon` sits under the `selector` at the source's own place, so the source's `offset` (its place in the clone tree) added to the selector's.
        let (_, selector_at) = node(&collect, "selector")?;
        let (_, offset_at) = node(&collect, "offset")?;
        let (bg_e, _) = node(&collect, "bg")?;
        let (item_e, item_at) = node(&collect, "item")?;
        let (mask_c, _) = node(&collect, "item_mask")?;
        let tool_at = [item_at[0] - offset_at[0], item_at[1] - offset_at[1]];
        // The clock.
        let (icon_base_e, icon_base_at) = node(&time, "icon_base")?;
        let (ring_e, _) = node(&time, "icon_base_1")?;
        let (clock_e, clock_at) = node(&time, "time_zone")?;
        let (date_e, date_at) = node(&time, "bg")?;
        let (am_e, am_at) = node(&time, "am")?;
        let (pm_e, pm_at) = node(&time, "pm")?;
        let (time_e, time_at) = node(&time, "time")?;
        let disc_at = [selector_at[0] + offset_at[0], selector_at[1] + offset_at[1]];
        // The area name's banner: a plate and a centred text for the area, the same for the map
        // behind them; the map's fade starts after the layout's `delay`.
        let banner = |plate: &str, text: &str| -> Result<([f32; 4], [f32; 2], Label), String> {
            let (p, at) = node(&places, plate)?;
            let (t, text_at) = node(&places, text)?;
            let mut l = label(t, text_at);
            let rgba = numbers(t, "font_color");
            l.color = (rgba.len() == 4).then(|| [rgba[0] / 255.0, rgba[1] / 255.0, rgba[2] / 255.0, rgba[3] / 255.0]);
            Ok((picture(&field, &style_of(p)?)?, at, l))
        };
        let (fade_in, _) = node(&places, "fade_in_with_area")?;
        let map_delay = fade_in
            .children
            .iter()
            .find(|b| b.attr("node") == Some("map_name"))
            .and_then(|b| b.attr("delay")?.trim().parse::<f32>().ok())
            .ok_or("uil_a21_fm01: fade_in_with_area has no map_name delay")?;
        // The help bar: the load screen's own layout (`uil_a21_shade.xml`).
        let (help_bg, help_at) = find(&help_layout, "base", [0.0; 2]).ok_or("uil_a21_shade: no base")?;
        let (_, left_side) = inside(help_bg, "left_side", [0.0; 2]).ok_or("uil_a21_shade: no left_side")?;
        let (help_bg_node, _) = node(&help_layout, "bg")?;
        let (help_text, help_text_at) = inside(help_bg_node, "text", [0.0; 2]).ok_or("uil_a21_shade: no bg/text")?;
        // The gimmicks' icon: the balloon and the picture inside it, as the layout places them under the node's own scale.
        let advertise = table::parse(src.advertise).map_err(|e| format!("uil_a21_fm_advertise01: {e}"))?;
        let (gimmick_e, _) = find(&advertise, "fm_icon_gimmick", [0.0; 2]).ok_or("uil_a21_fm_advertise01: no fm_icon_gimmick")?;
        let balloon_e = gimmick_e.children.iter().find(|e| e.attr("name") == Some("iconbg")).ok_or("fm_icon_gimmick: no iconbg")?;
        let glyph_e = balloon_e.children.iter().find(|e| e.attr("name") == Some("icon_gimmick")).ok_or("fm_icon_gimmick: no icon_gimmick")?;
        let glyph_no = glyph_e.attr("image_no").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
        let tint = numbers(glyph_e, "color");
        let balloon = Balloon {
            picture: picture(&field, &style_of(balloon_e)?)?,
            at: pair(balloon_e, "pos", [0.0; 2]),
            icon: pictures(&icons, &style_of(glyph_e)?)?.get(glyph_no).copied().ok_or("fm_icon_gimmick: no such picture")?,
            icon_at: pair(glyph_e, "pos", [0.0; 2]),
            icon_size: pair(glyph_e, "size_wh", [50.0; 2])[0],
            icon_color: if tint.len() == 4 { [tint[0] / 255.0, tint[1] / 255.0, tint[2] / 255.0, tint[3] / 255.0] } else { [1.0; 4] },
            scale: scale_of(gimmick_e),
        };
        let atlases = src.sizes.map(|(w, h)| (w as f32 * 2.0, h as f32 * 2.0));
        let arrival = Arrival::read(&src, &places, &field, &atlases)?;
        let popups = Popups::read(&src, &places, &field)?;
        let item_atlases = src.item_sizes.map(|s| s.map(|(w, h)| (w as f32 * 2.0, h as f32 * 2.0)));
        Ok(Hud {
            glyphs: src.glyphs,
            atlases,
            strings,
            grid: (picture(&field, &style_of(grid_e)?)?, grid_at),
            mask: (picture(&field, &style_of(mask_e)?)?, mask_at, scale_of(mask_e)),
            gmd: (picture(&field, &style_of(gmd_e)?)?, gmd_at),
            frame: (picture(&field, &style_of(frame_e)?)?, frame_at),
            north: (picture(&field, &style_of(north_e)?)?, north_at),
            arrow,
            exit_icon: icon("icon_fm_change_img")?,
            folk_icon: icon("icon_npc")?,
            fishing_icon: icon("icon_fishing_img")?,
            water_icon: icon("icon_water_img")?,
            destination_icon: icon("icon_map_change_destination_img")?,
            glows: [icon("icon_map_change_destination_img_eff_01")?, icon("icon_map_change_destination_img_eff_02")?],
            base: (picture(&common, &style_of(cbase_e)?)?, cbase_at, pair(cbase_e, "size_wh", [0.0; 2])[0]),
            basket_icon: (picture(&icons, &style_of(icon_e)?)?, icon_at, pair(icon_e, "size_wh", [0.0; 2])[0]),
            count_base: (picture(&various, &style_of(num_bg)?)?, num_at, scale_of(num_bg), pair(num_bg, "size_wh", [0.0; 2]), pair(num_bg, "pos", [0.0; 2])),
            count: label(num_e, pair(num_e, "pos", [0.0; 2])),
            count_origin: num_at,
            disc: (picture(&field, &style_of(bg_e)?)?, disc_at),
            ring: picture(&field, "gen_a21_field01_equip_corsol03")?,
            ring_glow: picture(&field, "gen_a21_field01_equip_corsol02")?,
            disc_mask: (picture(&field, &style_of(mask_c)?)?, scale_of(mask_c)),
            tools: pictures(&icons, &style_of(item_e)?)?,
            tool_at,
            tool_size: pair(item_e, "size_wh", [50.0; 2])[0],
            clock_base: (picture(&field, &style_of(icon_base_e)?)?, icon_base_at),
            clock_ring: picture(&field, &style_of(ring_e)?)?,
            clock_icons: pictures(&field, &style_of(clock_e)?)?,
            clock_icon_at: clock_at,
            date_base: (picture(&field, &style_of(date_e)?)?, date_at),
            am: label(am_e, am_at),
            pm: label(pm_e, pm_at),
            time: label(time_e, time_at),
            time_origin: [0.0; 2],
            area: banner("area_neme_bg", "area_name_text")?,
            map: banner("map_name_bg", "map_name_text")?,
            fade: curve(src.anims, "anm_common_juvenile_fade_in", "color")?,
            map_delay: map_delay * 60.0,
            help: style(&guide, help_bg.attr("style").ok_or("uil_a21_shade: base has no style")?)?,
            help_at,
            help_text: label(help_text, help_text_at),
            help_left: left_side[0],
            balloon,
            arrival: Some(arrival),
            popups: Some(popups),
            item_atlases,
        })
    }

    /// The texts of `STR_UI_nnnn`.
    fn string(&self, n: u32) -> &str {
        self.strings.get(&(STR_UI_BASE + n)).map_or("", String::as_str)
    }

    /// A string of the table by its own number.
    fn word(&self, n: u32) -> &str {
        self.strings.get(&n).map_or("", String::as_str)
    }

    fn banner_alpha(&self, age: f32, delay: f32) -> f32 {
        banner_alpha(&self.fade, age, delay)
    }

    /// The HUD's quads for `state` on a `screen` of pixels, in the order the game draws them, and
    /// its clips: the minimap's mask is a stencil the terrain and her arrow draw inside (ryza-recomp
    /// `hud-walk`: the mask quad writes stencil 1 with no colour, the terrain, markers and arrow
    /// test equal to 1); the tool disc's mask goes to a target of its own the tool's picture is
    /// multiplied by, drawn here as a mask that clips nothing (the tool sits inside the disc).
    pub fn quads(&self, state: &State, markers: &Markers, screen: (u32, u32)) -> (Vec<Quad>, Vec<Clip>, Vec<std::ops::Range<usize>>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        let mut pen = Pen { hud: self, k, left, out: Vec::new() };
        let white = [1.0; 4];
        let mut clips = Vec::new();
        let mut additive = Vec::new();

        // The gimmicks' icons over the field, under the HUD.
        for m in &state.marks {
            self.world_icon(&mut pen, m);
        }

        // The story marquee of the arrival after the opening, under everything else.
        if state.arrival {
            self.marquee(&mut pen, state.age, &mut clips, self.word(arrival::STORY_TITLE));
        }

        // The area name's banner on arrival: the area's plate and name, then the map's.
        if let Some(names) = state.place {
            for ((plate, at, text), (name, delay)) in [(&self.area, (names[0], 0.0)), (&self.map, (names[1], self.map_delay))] {
                let alpha = self.banner_alpha(state.age, delay);
                if alpha > 0.0 {
                    pen.picture(FIELD, *plate, *at, 1.0, [1.0, 1.0, 1.0, alpha]);
                    let ink = text.color.unwrap_or([0.0, 0.0, 0.0, 1.0]);
                    pen.shadowed(text, self.word(name), [ink[0], ink[1], ink[2], alpha]);
                }
            }
        }

        // The gathering's prompt and notices, under the minimap.
        self.gathering(&mut pen, &state.gather);

        // The minimap.
        pen.picture(FIELD, self.grid.0, self.grid.1, 1.0, white);
        let map_mask = pen.out.len();
        pen.picture(FIELD, self.mask.0, self.mask.1, self.mask.2, white);
        let center = self.arrow_center();
        let spin = state.camera_yaw.map(minimap_turn);
        if state.map {
            pen.terrain(state.at, center, spin);
        }
        // The markers, clipped with the terrain: the ways out, then the townsfolk.
        let beside = |at: &[f32; 2]| marker_at(center, [state.at[0], state.at[2]], *at, spin.unwrap_or(0.0));
        for at in &markers.exits {
            pen.centered(FIELD, self.exit_icon, beside(at), 1.0, white);
        }
        for at in &markers.water {
            pen.centered(FIELD, self.water_icon, beside(at), 1.0, white);
        }
        for at in &markers.fishing {
            pen.centered(FIELD, self.fishing_icon, beside(at), 1.0, white);
        }
        for at in &markers.folk {
            pen.centered(FIELD, self.folk_icon, beside(at), 1.0, white);
        }
        pen.arrow(self.arrow, center, state.turn, spin.unwrap_or(0.0));
        clips.push(Clip { mask: map_mask, quads: map_mask + 1..pen.out.len() });
        pen.picture(FIELD, self.gmd.0, self.gmd.1, 1.0, white);
        // The frame turns about its own middle, the north mark about the frame's.
        pen.picture(FIELD, self.frame.0, self.frame.1, 1.0, white);
        let middle = [self.frame.1[0] + self.frame.0[2] / PER_LAYOUT / 2.0, self.frame.1[1] + self.frame.0[3] / PER_LAYOUT / 2.0];
        if let Some(angle) = spin {
            pen.turn_last(middle, angle);
        }
        pen.picture(FIELD, self.north.0, self.north.1, 1.0, white);
        if let Some(angle) = spin {
            pen.turn_last(middle, angle);
        }
        // The destination and its glows, over the frame: on the map where it is in view, else on
        // the ring at its edge toward it.
        let glow = pen.out.len() + 1;
        if let Some(at) = &markers.destination {
            let mut at = beside(at);
            let off = [at[0] - center[0], at[1] - center[1]];
            let far = off[0].hypot(off[1]);
            if far > DESTINATION_RING {
                at = [center[0] + off[0] * DESTINATION_RING / far, center[1] + off[1] * DESTINATION_RING / far];
            }
            // With an arrival the loop starts 2 frames before it (hud-walk's 1728 and the opening's 14862 both fit it); a still without one keeps the fitted phase.
            let arrival = state.tick - state.age;
            let (alpha, scale) = glow_pulse(state.anim, if arrival > 0.0 { arrival - 2.0 } else { GLOW_PHASE });
            let tint = |a: f32| [GLOW_TINT[0], GLOW_TINT[1], GLOW_TINT[2], a];
            pen.centered(FIELD, self.destination_icon, at, 1.0, white);
            // The game draws no glow while the pulse is at 0 (`hud-walk` 1800: loop frame 3).
            if alpha > 0.0 {
                pen.centered(GLOW, self.glows[0], at, scale, tint(alpha));
                pen.centered(GLOW, self.glows[1], at, 1.0, tint(alpha));
            }
        }
        additive.push(glow..pen.out.len());

        // The basket: its disc, its picture and the count on its plate.
        let dim = [0.0, 0.0, 0.0, 100.0 / 255.0];
        pen.sized(COMMON, self.base.0, self.base.1, self.base.2, dim);
        pen.sized(ICON, self.basket_icon.0, self.basket_icon.1, self.basket_icon.2, white);
        let (plate, at, scale, size, own) = self.count_base;
        pen.wrap(VARIOUS, plate, [at[0] + own[0], at[1] + own[1]], size[0], scale, dim);
        let count = state.basket.to_string();
        pen.bold_text(&self.count, self.count_origin, &count, [1.0; 4]);

        // The equipped tool.
        pen.picture(FIELD, self.disc.0, self.disc.1, 1.0, white);
        clips.push(Clip { mask: pen.out.len(), quads: 0..0 });
        pen.picture(FIELD, self.disc_mask.0, self.disc.1, self.disc_mask.1, white);
        // The highlight of a staff point in reach: the ring and its copy over it, and between them the pulsing glow (added).
        let gathering = &state.gather;
        if gathering.staff && gathering.near.is_some() && gathering.pressed.is_none_or(|p| p < gathering.notice_at) {
            pen.picture(FIELD, self.ring, self.disc.1, 1.0, white);
            let (alpha, scale) = ring_pulse(state.anim, if state.tick - state.age > 0.0 { state.tick - state.age - 2.0 } else { GLOW_PHASE });
            if alpha > 0.0 {
                let half = self.ring_glow[2] / PER_LAYOUT / 2.0;
                let middle = [self.disc.1[0] + half, self.disc.1[1] + half];
                let from = pen.out.len();
                pen.centered(FIELD, self.ring_glow, middle, scale, [1.0, 1.0, 1.0, alpha]);
                additive.push(from..pen.out.len());
            }
            pen.picture(FIELD, self.ring, self.disc.1, 1.0, white);
        }
        if let Some(tool) = self.tools.get(state.tool) {
            pen.sized(ICON, *tool, [self.disc.1[0] + self.tool_at[0], self.disc.1[1] + self.tool_at[1]], self.tool_size, [26.0 / 255.0, 0.0, 0.0, 1.0]);
        }

        // The clock: its disc and ring, the period's picture, the plate and the hour.
        pen.picture(FIELD, self.clock_base.0, self.clock_base.1, 1.0, white);
        pen.picture(FIELD, self.clock_ring, self.clock_base.1, 1.0, white);
        let period = match state.hour {
            5..=10 => 1,
            11..=16 => 2,
            17..=19 => 3,
            _ => 0,
        };
        if let Some(icon) = self.clock_icons.get(period) {
            pen.picture(FIELD, *icon, self.clock_icon_at, 1.0, white);
        }
        pen.picture(FIELD, self.date_base.0, self.date_base.1, 1.0, white);
        let (mark, word) = if state.hour < 12 { (&self.am, 284) } else { (&self.pm, 285) };
        let color = mark.color.unwrap_or(white);
        pen.text(mark, self.time_origin, self.string(word), color);
        let shown = match state.hour % 12 {
            0 => 12,
            h => h,
        };
        pen.text(&self.time, self.time_origin, &shown.to_string(), white);
        if state.help.alpha() > 0.0 {
            self.help_bar(&mut pen, &state.help, state.swung);
        }
        if state.arrival {
            self.autosave_mark(&mut pen, state.age);
        }
        (pen.out, clips, additive)
    }

    /// A gimmick's icon: the balloon about its middle (it shrinks about it by the distance's scale) and the gimmick's picture in it.
    fn world_icon(&self, pen: &mut Pen, m: &WorldMark) {
        let b = &self.balloon;
        // The layout's pixels (the style's three to one) are world units times the node's scale.
        let unit = b.scale * m.per_unit;
        let (w, h) = (b.picture[2] / PER_LAYOUT, b.picture[3] / PER_LAYOUT);
        let middle = [m.at[0] + (b.at[0] + w / 2.0) * unit, m.at[1] + (b.at[1] + h / 2.0) * unit];
        let size = [w * unit * m.near, h * unit * m.near];
        let rect = [middle[0] - size[0] / 2.0, middle[1] - size[1] / 2.0, middle[0] + size[0] / 2.0, middle[1] + size[1] / 2.0];
        let screen = [2.0 * pen.left + 1280.0 * pen.k, 720.0 * pen.k];
        if rect[2] < 0.0 || rect[3] < 0.0 || rect[0] > screen[0] || rect[1] > screen[1] {
            return;
        }
        pen.quad(BALLOON, pen.uv(BALLOON, b.picture), rect, [1.0; 4]);
        // The picture sits at its place from the balloon's corner, scaled with it.
        let ratio = unit * m.near;
        let at = [rect[0] + b.icon_at[0] * ratio, rect[1] + b.icon_at[1] * ratio];
        let side = b.icon_size * ratio;
        pen.quad(ICON, pen.uv(ICON, b.icon), [at[0], at[1], at[0] + side, at[1] + side], b.icon_color);
    }

    /// The button help bar at the screen's lower right (the load screen's look): a plate as wide
    /// as its words, tiled, and the line of each key's glyph and words.
    fn help_bar(&self, pen: &mut Pen, bar: &HelpBar, swung: bool) {
        let entry = |&(glyph, n): &(char, u32)| format!("{glyph}{}", self.word(n));
        let full_line = |hint: bool| HELP_ENTRIES.iter().enumerate().filter(|&(i, _)| hint || i != STAFF_HINT).map(|(_, e)| entry(e)).collect::<Vec<_>>().join(" ");
        let line = if bar.first_line() { HELP_FIRST.iter().map(|&i| entry(&HELP_ENTRIES[i])).collect::<Vec<_>>().join(" ") } else { full_line(!swung) };
        let s = self.help_text.size * pen.k / ATLAS_SIZE;
        // STAND-IN: The plate is one atlas pixel of the text (size / ATLAS_SIZE layout pixels) wider than the advances' sum, fitted to the capture as the load screen's is (`load_view::help_bar`); the real thing is the game's own text-width measure.
        let plate = |line: &str| line.chars().filter_map(|c| self.glyphs[0].get(&c)).map(|g| g.advance as f32 * s).sum::<f32>() / pen.k + self.help_text.size / ATLAS_SIZE - self.help_text.at[0] + self.help_left;
        let full = plate(&line);
        let mut width = (HELP_PLATE_MIN + (full - HELP_PLATE_MIN) * bar.grow()).max(HELP_PLATE_MIN);
        // A line that changed since the bar last showed (the hint gone) has the plate tween from the old line's width, as the returning bar's grows
        // (`gather-rasen2` 3450: 4 frames into the fade, 0.625 of the way from the 7 entries' plate to the 6 entries').
        if bar.hint_from == swung && !bar.first_line() && bar.quick.is_none() {
            let old = plate(&full_line(bar.hint_from));
            width = old + (full - old) * (bar.lit / BACK_GROW_FRAMES).clamp(0.0, 1.0);
        }
        let alpha = [1.0, 1.0, 1.0, bar.alpha()];
        pen.tiled(GUIDE, &self.help, [1280.0 - width, self.help_at[1]], width, alpha);
        pen.text(&self.help_text, [1280.0, 720.0], &line, alpha);
    }

    /// Where her arrow is: the middle of the minimap's window, which the terrain is moved under.
    // STAND-IN: The arrow's middle (1729, 886 of the capture's 1080 lines) is read off the capture; the real thing is the player marker's place in the minimap's own node tree, which the code moves the terrain under.
    fn arrow_center(&self) -> [f32; 2] {
        [1729.0 / 1.5, 886.0 / 1.5]
    }
}

/// How far the minimap is turned for a camera yaw (game degrees), in radians clockwise on the
/// screen: the camera's yaw plus a half turn (ryza-recomp `map-rotation`, six frames: the frame's
/// turn is 180 degrees plus the camera's yaw, to 0.05 degrees; yaw -1.00 gives 179.0).
pub fn minimap_turn(camera_yaw: f32) -> f32 {
    (180.0 + camera_yaw).to_radians()
}

/// Where a thing at `thing` (game units, x then z) is on the minimap when she stands at `her`:
/// the arrow's middle `center`, plus the way from her to it at `MARKER_UNITS_PER_PIXEL`, turned
/// by `angle` (radians clockwise on the screen) about `center`.
pub fn marker_at(center: [f32; 2], her: [f32; 2], thing: [f32; 2], angle: f32) -> [f32; 2] {
    let (dx, dy) = ((thing[0] - her[0]) / MARKER_UNITS_PER_PIXEL, (thing[1] - her[1]) / MARKER_UNITS_PER_PIXEL);
    let (s, c) = angle.sin_cos();
    [center[0] + dx * c - dy * s, center[1] + dx * s + dy * c]
}

// STAND-IN: The minimap's scale and place for Rasenboden are fitted to six captured frames (hud-walk 2030 to 3330, her place from the walk's state): the terrain's corner falls 1 layout pixel per 125 game units in x and in z (the markers' scale; x fitted as 120 before the markers showed 125 fits all six frames to under a pixel), and x = 0, z = 0 put it at (1152.67, 590.66); the game's own per-map bounds are not found in its tables.
/// Layout pixels the terrain moves per game unit of her walk (x, z), and its corner at the origin.
const TERRAIN_PER_UNIT: [f32; 2] = [1.0 / 125.0, 1.0 / 125.0];
/// Game units to a layout pixel on the minimap, for the markers (all 18 exit markers of the six
/// captured frames of `hud-walk` fit to under half a pixel; the terrain's x was fitted as 120).
const MARKER_UNITS_PER_PIXEL: f32 = 125.0;
/// How far from the arrow's middle a destination outside the map sits, in layout pixels (the
/// capture's 120 pixels on 1080 lines).
const DESTINATION_RING: f32 = 80.0;
/// The glows' colour (`color="155, 255, 255"` on the layout's `_eff` nodes).
const GLOW_TINT: [f32; 3] = [155.0 / 255.0, 1.0, 1.0];
/// The glows' blink loops every 71 frames of the capture (`anm_icon_eff_blink_01` and `_02` are
/// 70 frames long; the loop's 71 and its phase, `GLOW_PHASE`, are fitted to the 75 glow quads of
/// `hud-walk` 1740 to 2560: alpha off to 5, up to 10, down to 70, within 0.001).
const GLOW_PERIOD: f32 = 71.0;
/// The capture frame the loop is at its start in (`hud-walk` frame + 49 is a multiple of 71).
const GLOW_PHASE: f32 = -49.0;

/// The glows' alpha and the first glow's scale at `tick` frames (the layout's animations: alpha
/// 0 until frame 5, 1 at 10, 0 at 70; the first glow's scale 0.8 until 10, easing out to 1 at 70).
fn glow_pulse(tick: f32, start: f32) -> (f32, f32) {
    let t = (tick - start).rem_euclid(GLOW_PERIOD);
    let alpha = match t {
        t if t < 5.0 => 0.0,
        t if t < 10.0 => (t - 5.0) / 5.0,
        t => ((70.0 - t) / 60.0).max(0.0),
    };
    let u = ((t - 10.0) / 60.0).clamp(0.0, 1.0);
    // Quantised to bytes, as the game's colour.
    ((alpha * 255.0).round() / 255.0, 0.8 + 0.2 * (1.0 - (1.0 - u).powi(3)))
}

/// The tool ring's pulse (`anm_a21_fm_minimap_equip_icon`: alpha 0 until frame 5, 128 at 10, 0 at 70; scale 1 until 10, easing out to 1.1
/// at 70) on the glows' loop (`glow_pulse`), as the colour byte it is drawn with.
fn ring_pulse(anim: f32, start: f32) -> (f32, f32) {
    let t = (anim - start).rem_euclid(GLOW_PERIOD);
    let alpha = match t {
        t if t < 5.0 => 0.0,
        t if t < 10.0 => (t - 5.0) / 5.0,
        t => ((70.0 - t) / 60.0).max(0.0),
    };
    let u = ((t - 10.0) / 60.0).clamp(0.0, 1.0);
    ((alpha * 128.0).round() / 255.0, 1.0 + 0.1 * (1.0 - (1.0 - u).powi(3)))
}

const TERRAIN_ORIGIN: [f32; 2] = [1152.67, 590.66];
/// The terrain picture's size in layout pixels (it is drawn twice over, its style being twice
/// the texture's size).
const TERRAIN_SIZE: f32 = 1024.0;

struct Pen<'a> {
    hud: &'a Hud,
    k: f32,
    left: f32,
    out: Vec<Quad>,
}

impl Pen<'_> {
    fn quad(&mut self, texture: usize, uv: [f32; 4], rect: [f32; 4], color: [f32; 4]) {
        self.out.push(Quad { turn: false, spin: None, rect, uv, color, texture });
    }

    fn uv(&self, texture: usize, p: [f32; 4]) -> [f32; 4] {
        // The atlases follow the two fonts; the item pictures' after the others.
        let (aw, ah) = match self.hud.item_atlases {
            Some(a) if texture >= ITEM_PICTURES => a[(texture - ITEM_PICTURES) % 2],
            _ => self.hud.atlases[texture - FIELD],
        };
        [p[0] / aw, p[1] / ah, (p[0] + p[2]) / aw, (p[1] + p[3]) / ah]
    }

    fn at(&self, at: [f32; 2]) -> [f32; 2] {
        [self.left + at[0] * self.k, at[1] * self.k]
    }

    /// A picture at its natural size times `scale`.
    fn picture(&mut self, texture: usize, p: [f32; 4], at: [f32; 2], scale: f32, color: [f32; 4]) {
        let [l, t] = self.at(at);
        let (w, h) = (p[2] / PER_LAYOUT * scale * self.k, p[3] / PER_LAYOUT * scale * self.k);
        self.quad(texture, self.uv(texture, p), [l, t, l + w, t + h], color);
    }

    /// A picture at its natural size times `scale`, its middle at `at`.
    fn centered(&mut self, texture: usize, p: [f32; 4], at: [f32; 2], scale: f32, color: [f32; 4]) {
        let (w, h) = (p[2] / PER_LAYOUT * scale, p[3] / PER_LAYOUT * scale);
        self.picture(texture, p, [at[0] - w / 2.0, at[1] - h / 2.0], scale, color);
    }

    /// A square picture `size` layout pixels wide (the node's `size_wh`).
    fn sized(&mut self, texture: usize, p: [f32; 4], at: [f32; 2], size: f32, color: [f32; 4]) {
        let [l, t] = self.at(at);
        let s = size * self.k;
        self.quad(texture, self.uv(texture, p), [l, t, l + s, t + s], color);
    }

    /// A stretched plate: its left and right margins (the style's `edge_margin`) and the middle
    /// between, `width` layout pixels across before `scale`; a plate narrower than its margins
    /// is two caps.
    fn wrap(&mut self, texture: usize, p: [f32; 4], at: [f32; 2], width: f32, scale: f32, color: [f32; 4]) {
        let [x, y, w, h] = p;
        // The margins of `gen_a21v2_option01_auto_bg` (99 and 106 style pixels).
        let (ml, mr) = (99.0, 106.0);
        let [l, t] = self.at(at);
        let unit = scale * self.k / PER_LAYOUT;
        let (cap_l, cap_r) = (ml * unit, mr * unit);
        let bottom = t + h * unit;
        let middle = (width * scale * self.k - cap_l - cap_r).max(0.0);
        self.quad(texture, self.uv(texture, [x, y, ml, h]), [l, t, l + cap_l, bottom], color);
        let mut right = l + cap_l;
        if middle > 0.0 {
            let src = w - ml - mr;
            self.quad(texture, self.uv(texture, [x + ml, y, src, h]), [right, t, right + middle, bottom], color);
            right += middle;
        }
        self.quad(texture, self.uv(texture, [x + w - mr, y, mr, h]), [right, t, right + cap_r, bottom], color);
    }

    /// The terrain picture under her arrow: it slides with her walk, twice over.
    fn terrain(&mut self, at: [f32; 3], arrow: [f32; 2], spin: Option<f32>) {
        let (x, y) = (TERRAIN_ORIGIN[0] - at[0] * TERRAIN_PER_UNIT[0], TERRAIN_ORIGIN[1] - at[2] * TERRAIN_PER_UNIT[1]);
        let [l, t] = self.at([x, y]);
        let s = TERRAIN_SIZE * self.k;
        self.quad(MINIMAP, [0.0, 0.0, 2.0, 2.0], [l, t, l + s, t + s], [1.0; 4]);
        if let Some(angle) = spin {
            self.turn_last(arrow, angle);
        }
    }

    /// Turns the quad just drawn by `angle` (radians clockwise on the screen) about `pivot`
    /// (layout pixels).
    fn turn_last(&mut self, pivot: [f32; 2], angle: f32) {
        let pivot = self.at(pivot);
        if let Some(q) = self.out.last_mut() {
            q.spin = Some(Spin::About { pivot, angle });
        }
    }

    /// Her arrow turned about its middle to face the way she does on the map, itself turned by
    /// `map_turn` radians (0 fixed-north). The game keeps the turn within a quarter either side
    /// of the picture's own upright and flips the picture's texture coordinates for the other
    /// half (the capture's six frames, and six more with the map turned).
    fn arrow(&mut self, p: [f32; 4], center: [f32; 2], turn: f32, map_turn: f32) {
        let [cx, cy] = self.at(center);
        let (w, h) = (p[2] / PER_LAYOUT * self.k, p[3] / PER_LAYOUT * self.k);
        let mut r = (180.0 - turn + map_turn.to_degrees()).rem_euclid(360.0);
        if r > 180.0 {
            r -= 360.0;
        }
        let flipped = r.abs() > 90.0;
        let angle = if flipped { r - 180.0 * r.signum() } else { r };
        let uv = self.uv(FIELD, p);
        let uv = if flipped { [uv[2], uv[3], uv[0], uv[1]] } else { uv };
        self.out.push(Quad { turn: false, spin: Some(Spin::About { pivot: [cx, cy], angle: angle.to_radians() }), rect: [cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0], uv, color: [1.0; 4], texture: FIELD });
    }

    /// `text` in a label's font and size, its node at `origin`.
    fn text(&mut self, l: &Label, origin: [f32; 2], text: &str, color: [f32; 4]) {
        self.glyphs(l, origin, [0.0, 0.0], text, color);
    }

    /// Text with the shadow the game draws round it (`bold_size` 0.5 layout pixels, in nine
    /// copies at the black `bold_color`'s alpha 127, half of it again as the game's 2D shader has it).
    fn bold_text(&mut self, l: &Label, origin: [f32; 2], text: &str, color: [f32; 4]) {
        let step = 0.5;
        for dx in [-step, 0.0, step] {
            for dy in [-step, 0.0, step] {
                self.glyphs(l, origin, [dx, dy], text, [0.0, 0.0, 0.0, 127.0 / 255.0 * 0.5]);
            }
        }
        self.glyphs(l, origin, [0.0, 0.0], text, color);
    }

    /// Text with an outline and a drop shadow (`bold_size` and `drop_shadow` of a text node): the
    /// shadow, black at half alpha, 3 layout pixels right and down; the outline, `bold`'s colour
    /// (taken by the text's own, at a quarter of the alpha) in a grid of copies `bold`'s size
    /// round the text, in `ceil(size)` steps either side; then the text. `gather-rasen` 2205: the
    /// 27 copies of every glyph of "New Discovery".
    fn dropped(&mut self, l: &Label, origin: [f32; 2], text: &str, color: [f32; 4], bold: ([f32; 4], f32), tint: [f32; 4]) {
        self.glyphs(l, origin, [3.0, 3.0], text, [0.0, 0.0, 0.0, 0.5 * tint[3]]);
        let (ink, size) = bold;
        let steps = size.ceil().max(1.0) as i32;
        let each = size / steps as f32;
        let shade = [ink[0] * color[0], ink[1] * color[1], ink[2] * color[2], ink[3] * 0.25 * tint[3]];
        for ix in -steps..=steps {
            for iy in -steps..=steps {
                self.glyphs(l, origin, [ix as f32 * each, iy as f32 * each], text, shade);
            }
        }
        self.glyphs(l, origin, [0.0; 2], text, color);
    }

    /// A banner's text: nine copies of its colour at half the alpha one layout pixel round it, the
    /// text over them (`hud-walk` 1830: the game's `bold_size` 1 with the font's own colour).
    fn shadowed(&mut self, l: &Label, text: &str, color: [f32; 4]) {
        for dx in [-1.0, 0.0, 1.0] {
            for dy in [-1.0, 0.0, 1.0] {
                self.glyphs(l, [0.0; 2], [dx, dy], text, [color[0], color[1], color[2], color[3] / 2.0]);
            }
        }
        self.glyphs(l, [0.0; 2], [0.0; 2], text, color);
    }

    /// A tiled ("wrap") plate `width` layout pixels wide at `at`: the left margin, the middle's
    /// tiles (the last cut short) and the right margin (as `load_view`'s plates).
    fn tiled(&mut self, texture: usize, s: &Style, at: [f32; 2], width: f32, color: [f32; 4]) {
        let [x, y, w, h] = s.pictures[0];
        let [ml, mr] = s.margins.unwrap_or([0.0; 2]);
        let (cap_l, cap_r) = (ml / PER_LAYOUT, mr / PER_LAYOUT);
        let tile = (w - ml - mr) / PER_LAYOUT;
        let height = h / PER_LAYOUT;
        let put = |pen: &mut Pen, src: [f32; 4], x: f32, wide: f32| {
            let [l, t] = pen.at([x, at[1]]);
            let rect = [l, t, l + wide * pen.k, t + height * pen.k];
            pen.quad(texture, pen.uv(texture, src), rect, color);
        };
        put(self, [x, y, ml, h], at[0], cap_l);
        let mut at_x = at[0] + cap_l;
        let mut left = width - cap_l - cap_r;
        while left > 0.001 {
            let part = left.min(tile);
            put(self, [x + ml, y, (w - ml - mr) * part / tile, h], at_x, part);
            at_x += part;
            left -= part;
        }
        put(self, [x + w - mr, y, mr, h], at_x, cap_r);
    }

    fn glyphs(&mut self, l: &Label, origin: [f32; 2], shift: [f32; 2], text: &str, color: [f32; 4]) {
        let font = usize::from(!l.small);
        let glyphs = &self.hud.glyphs[font];
        let s = l.size * self.k / ATLAS_SIZE;
        let gap = l.spacing * self.k;
        let width: f32 = text.chars().filter_map(|c| glyphs.get(&c)).map(|g| g.advance as f32 * s + gap).sum();
        let x = self.left + (origin[0] + l.at[0] + shift[0]) * self.k;
        let mut pen = if l.center {
            x - width / 2.0
        } else if l.right {
            x - width
        } else {
            x
        };
        let top = (origin[1] + l.at[1] + shift[1] + TEXT_TOP) * self.k;
        let (aw, ah) = (4096.0, 2048.0);
        for g in text.chars().filter_map(|c| glyphs.get(&c)) {
            let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
            let (x0, y0) = (pen + g.x_offset as f32 * s, top + g.y_offset as f32 * s);
            self.quad(if font == 0 { FONT01 } else { FONT00 }, [gx / aw, gy / ah, (gx + gw) / aw, (gy + gh) / ah], [x0, y0, x0 + gw * s, y0 + gh * s], color);
            pen += g.advance as f32 * s + gap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 2], b: [f32; 2]) {
        assert!((a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3, "{a:?} vs {b:?}");
    }

    /// `anm_common_juvenile_fade_in`: the colour's alpha from 0 to 255 over 60 frames, in cubic.
    fn fade() -> Curve {
        let xml = br#"<root><anim name="anm_common_juvenile_fade_in"><curve attr="color" interpolate="easing_incubic"><key value="255, 255, 255, 0"/><key frame60="60" value="255, 255, 255, 255"/></curve></anim></root>"#;
        curve(xml, "anm_common_juvenile_fade_in", "color").unwrap()
    }

    #[test]
    fn the_banner_fades_in_holds_and_fades_out_as_the_capture_has_it() {
        let f = fade();
        // hud-walk, the area plate (arrival at 1728): the capture's alphas in 1/255.
        for (frame, byte) in [(1740.0, 2.0), (1750.0, 13.0), (1760.0, 39.0), (1770.0, 87.0), (1780.0, 166.0), (1790.0, 255.0), (1870.0, 255.0)] {
            assert_eq!(banner_alpha(&f, frame - 1728.0, 0.0) * 255.0, byte, "frame {frame}");
        }
        // The fade out is the same curve run backwards from 151 frames in.
        for (frame, byte) in [(1880.0, 242.0), (1890.0, 139.0), (1900.0, 70.0), (1910.0, 29.0), (1920.0, 8.0), (1930.0, 1.0)] {
            assert_eq!(banner_alpha(&f, frame - 1728.0, 0.0) * 255.0, byte, "frame {frame}");
        }
        assert_eq!(banner_alpha(&f, 1940.0 - 1728.0, 0.0), 0.0);
        // The map's plate follows 30 frames behind, both ways.
        assert_eq!(banner_alpha(&f, 1770.0 - 1728.0, 30.0) * 255.0, 2.0);
        assert_eq!(banner_alpha(&f, 1910.0 - 1728.0, 30.0) * 255.0, 242.0);
        assert_eq!(banner_alpha(&f, 5.0, 30.0), 0.0);
    }

    /// The help bar at each frame from the arrival at 1728, the move stick out of its dead zone
    /// at the `hud-walk` pad script's spans (the pad's frames reach the game 2 later).
    fn help_at(frame: f32) -> HelpBar {
        let spans = [(1900.0, 2020.0), (2040.0, 2130.0), (2150.0, 2210.0), (2230.0, 2320.0), (2420.0, 2540.0), (2560.0, 2620.0), (2780.0, 3080.0), (3100.0, 3300.0)];
        let mut bar = HelpBar::arriving();
        let mut f = 1728.0;
        while f <= frame {
            bar.step(spans.iter().any(|&(a, b)| f >= a + 2.0 && f < b + 2.0), 1.0);
            f += 1.0;
        }
        bar
    }

    #[test]
    fn the_help_bar_comes_up_after_121_frames_of_rest_and_goes_with_the_stick() {
        // The capture's alphas (the plate quads' colour, 1/15 a frame).
        for (frame, steps) in [(1740.0, 7.0), (1750.0, 15.0), (1910.0, 6.0), (2750.0, 9.0), (2790.0, 6.0), (3430.0, 9.0), (3440.0, 15.0)] {
            assert!((help_at(frame).alpha() * 15.0 - steps).abs() < 1e-3, "frame {frame}");
        }
        // Down while the stick is out and at the shorter rests between the walks.
        for frame in [1930.0, 2330.0, 2400.0, 2700.0, 2740.0, 3000.0, 3400.0, 3421.0] {
            assert_eq!(help_at(frame).alpha(), 0.0, "frame {frame}");
        }
    }

    #[test]
    fn the_bar_after_the_opening_fades_in_from_the_fourth_frame_with_its_plate_half_grown() {
        // `opening-key4`: the field's frames 14862 to 14871; the first plate on 14865 at 1/15, 785 pixels of 1191.5 (the
        // shortest 376.5): half of the way, whole on 14871.
        let mut bar = HelpBar::arriving_at_once();
        let mut seen = Vec::new();
        for _ in 0..10 {
            bar.step(false, 1.0);
            seen.push((bar.alpha(), bar.grow()));
        }
        assert_eq!(seen[..3].iter().map(|s| s.0).sum::<f32>(), 0.0);
        assert!((seen[3].0 - 1.0 / 15.0).abs() < 1e-6);
        assert!((seen[3].1 - 0.5012).abs() < 0.001, "{}", seen[3].1);
        assert_eq!(seen[9].1, 1.0);
        assert!(!bar.first_line());
    }

    #[test]
    fn the_help_bar_shows_a_short_line_and_a_short_plate_on_its_first_showing_only() {
        let (first, later) = (help_at(1740.0), help_at(3430.0));
        assert!(first.first_line() && first.grow() == 0.0);
        let (mid, full) = (help_at(1750.0), help_at(1760.0));
        assert!(!mid.first_line() && (mid.grow() - 0.375).abs() < 1e-3 && full.grow() == 1.0);
        assert!(!later.first_line() && later.grow() == 1.0);
    }

    #[test]
    fn only_rasenboden_has_its_names_read() {
        assert_eq!(place_names(MAP_FIELD), Some([2_228_225, 917_510]));
        assert_eq!(place_names("s01_i_attic"), None);
    }

    /// A view as the renderer builds it: right-handed, looking along +z in render space, y down on the screen.
    fn looking_along_z(eye: glam::Vec3) -> glam::Mat4 {
        let flip = glam::Mat4::from_scale(glam::Vec3::new(1.0, -1.0, 1.0));
        flip * glam::Mat4::perspective_rh(45f32.to_radians(), 16.0 / 9.0, 10.0, 100_000.0) * glam::Mat4::look_at_rh(eye, eye + glam::Vec3::Z, glam::Vec3::Y)
    }

    #[test]
    fn an_icon_floats_fifty_units_over_its_gimmick_and_shrinks_within_a_thousand() {
        let eye = glam::Vec3::ZERO;
        let vp = looking_along_z(eye);
        // A door straight ahead, 50 under the eye's height: its lifted origin is the screen's middle; the scene's x is mirrored.
        let far = world_mark(&vp, eye, [0.0, -50.0, 1500.0], (1920, 1080)).unwrap();
        assert!((far.at[0] - 960.0).abs() < 0.01 && (far.at[1] - 540.0).abs() < 0.01, "{far:?}");
        assert_eq!(far.near, 1.0);
        // A world unit measures the 45 degree focal length (540 / tan 22.5 degrees) over the depth.
        assert!((far.per_unit - 540.0 / (22.5f32.to_radians().tan() * 1500.0)).abs() < 1e-4);
        let near = world_mark(&vp, eye, [0.0, 0.0, 500.0], (1920, 1080)).unwrap();
        assert!((near.near - (0.1 + 0.9 * 0.5)).abs() < 1e-5);
        // Behind the eye, and past the range: none.
        assert_eq!(world_mark(&vp, eye, [0.0, 0.0, -500.0], (1920, 1080)), None);
        assert_eq!(world_mark(&vp, eye, [0.0, 0.0, 2500.0], (1920, 1080)), None);
    }

    #[test]
    fn the_tool_rings_glow_pulses_with_the_glows_loop_in_bytes() {
        // Off to frame 5, half up at 7.5, 128 of 255 at 10 with its scale 1, then down to nothing at 70 with its scale 1.1.
        assert_eq!(ring_pulse(5.0, 0.0), (0.0, 1.0));
        assert!((ring_pulse(10.0, 0.0).0 - 128.0 / 255.0).abs() < 1e-6 && ring_pulse(10.0, 0.0).1 == 1.0);
        let (alpha, scale) = ring_pulse(40.0, 0.0);
        assert!((alpha - 64.0 / 255.0).abs() < 1e-6 && (scale - (1.0 + 0.1 * 0.875)).abs() < 1e-5, "{alpha} {scale}");
        // The loop is 71 frames: 71 is its start again.
        assert_eq!(ring_pulse(71.0 + 40.0, 0.0), ring_pulse(40.0, 0.0));
    }

    #[test]
    fn a_line_that_lost_its_hint_has_the_plate_tween_from_the_old_width_on_the_next_showing() {
        // Shown with the hint, faded out, shown again after a swing: the showing begins from the line it last had.
        let mut bar = HelpBar::arriving();
        for _ in 0..200 {
            bar.step_swung(false, 1.0, false);
        }
        assert!(bar.alpha() > 0.0 && bar.hint_last);
        // The stick fades it out with its hint; she swings while it is gone; it fades in again.
        for _ in 0..20 {
            bar.step_swung(true, 1.0, false);
        }
        assert_eq!(bar.alpha(), 0.0);
        for _ in 0..5 {
            bar.step_swung(true, 1.0, true);
        }
        for _ in 0..124 {
            bar.step_swung(false, 1.0, true);
        }
        assert!(bar.alpha() > 0.0 && bar.hint_from && !bar.hint_last, "the old line had the hint, the new has not");
        assert!(bar.lit >= 1.0 && bar.lit < BACK_GROW_FRAMES);
    }

    #[test]
    fn a_marker_stays_where_the_fixed_map_has_it_at_no_turn() {
        // 1250 units east and 625 south of her: 10 and 5 layout pixels from the arrow.
        close(marker_at([100.0, 200.0], [0.0, 0.0], [1250.0, 625.0], 0.0), [110.0, 205.0]);
    }

    #[test]
    fn the_map_turns_clockwise_about_the_arrow() {
        let quarter = std::f32::consts::FRAC_PI_2;
        // East of her swings to south on the screen (y down), south to west.
        close(marker_at([100.0, 200.0], [0.0, 0.0], [1250.0, 0.0], quarter), [100.0, 210.0]);
        close(marker_at([100.0, 200.0], [0.0, 0.0], [0.0, 1250.0], quarter), [90.0, 200.0]);
    }

    #[test]
    fn the_camera_behind_her_at_yaw_zero_turns_the_map_a_half_turn() {
        assert!((minimap_turn(0.0) - std::f32::consts::PI).abs() < 1e-6);
        // The capture's second frame: camera yaw -61.02 degrees, the frame turned 118.98.
        assert!((minimap_turn(-61.02).to_degrees() - 118.98).abs() < 1e-3);
        close(marker_at([0.0, 0.0], [0.0, 0.0], [1250.0, 0.0], minimap_turn(0.0)), [-10.0, 0.0]);
    }

    #[test]
    fn the_returning_bar_fades_a_frame_later_and_its_plate_is_on_its_way_at_the_second_frame() {
        let (mut back, mut first) = (HelpBar::returning(), HelpBar::arriving());
        let (mut back_at, mut first_at) = (0, 0);
        for n in 1..=10 {
            back.step(false, 1.0);
            first.step(false, 1.0);
            if back.alpha() > 0.0 && back_at == 0 {
                back_at = n;
            }
            if first.alpha() > 0.0 && first_at == 0 {
                first_at = n;
            }
        }
        assert_eq!(first_at + 1, back_at);
        assert!(!back.first_line());
        // 2575: opacity 2/15, the plate 0.375 of the way.
        let mut b = HelpBar::returning();
        (0..back_at + 1).for_each(|_| b.step(false, 1.0));
        assert!((b.alpha() - 2.0 / 15.0).abs() < 1e-6);
        assert!((b.grow() - 0.375).abs() < 1e-6);
    }
}
