//! The field's steady HUD, from the game's own layouts: the minimap (`uil_a21_fm_minimap.xml`:
//! grid, terrain, her arrow, the glass layer, the frame and the north mark), the basket's count
//! (the same file's `cage`), the clock (`uil_a21_time_display.xml`) and the equipped tool's
//! disc (`uil_a21_collect_item.xml`), with the styles of `uis_gen_a21_field.xml`,
//! `uis_gen_a21_icon.xml`, `uis_gen_a21v2_common.xml` and `uis_gen_a21v2_various.xml`.
//!
//! Positions are in the 1280 x 720 layout and scale by `screen height / 720`, the layout
//! centred across the width, as the title's are. Left out (docs/ryza/open.md, "Field HUD"):
//! the minimap's markers (townsfolk, gimmicks, the destination), the area name's and the
//! messages' popups.

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Clip, Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;
use crate::load_view::TEXT_TOP;
use crate::title::{find, numbers, pair, LAYOUT_HEIGHT};

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
/// How many textures the layer has.
pub const TEXTURES: usize = 8;

/// The styles' pixels count three to a layout pixel (`load_view::ATLAS_PER_LAYOUT`).
const PER_LAYOUT: f32 = 3.0;
/// The first of the string table's UI strings (`STR_UI_0000`, as `load_view`).
const STR_UI_BASE: u32 = 6_619_137;

/// A text node's layout settings.
#[derive(Clone, Copy, Debug)]
struct Label {
    at: [f32; 2],
    size: f32,
    small: bool,
    center: bool,
    /// The node's colour, if it sets one.
    color: Option<[f32; 4]>,
}

// STAND-IN: The basket's count (225 in the capture) is the capture's; where the game takes it from is not read.
/// The basket's count shown.
pub const BASKET: u32 = 225;
/// The one field whose minimap picture and scale are read (`a21_minimap_image_006`).
pub const MAP_FIELD: &str = "s06_t_rasen";

/// What the HUD shows of the play.
#[derive(Clone, Copy, Debug)]
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
    /// The frames the HUD has been up (60 a second), which the destination's glow pulses by.
    pub tick: f32,
}

/// What the minimap marks, as places in the field (game units, x then z).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Markers {
    /// The ways out.
    pub exits: Vec<[f32; 2]>,
    /// The townsfolk.
    pub folk: Vec<[f32; 2]>,
    /// Where the story goes next, if it is in this field: a place, or the way out toward it.
    pub destination: Option<[f32; 2]>,
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
    pub glyphs: [HashMap<char, Glyph>; 2],
    /// Each texture's size in pixels, in the layer's order after the two fonts.
    pub sizes: [(u32, u32); TEXTURES - 2],
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
        color: (rgba.len() == 4).then(|| [rgba[0] / 255.0, rgba[1] / 255.0, rgba[2] / 255.0, rgba[3] / 255.0]),
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
            .filter(|(n, _)| (STR_UI_BASE..STR_UI_BASE + 1000).contains(n))
            .collect();
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
        Ok(Hud {
            glyphs: src.glyphs,
            atlases: src.sizes.map(|(w, h)| (w as f32 * 2.0, h as f32 * 2.0)),
            strings,
            grid: (picture(&field, &style_of(grid_e)?)?, grid_at),
            mask: (picture(&field, &style_of(mask_e)?)?, mask_at, scale_of(mask_e)),
            gmd: (picture(&field, &style_of(gmd_e)?)?, gmd_at),
            frame: (picture(&field, &style_of(frame_e)?)?, frame_at),
            north: (picture(&field, &style_of(north_e)?)?, north_at),
            arrow,
            exit_icon: icon("icon_fm_change_img")?,
            folk_icon: icon("icon_npc")?,
            destination_icon: icon("icon_map_change_destination_img")?,
            glows: [icon("icon_map_change_destination_img_eff_01")?, icon("icon_map_change_destination_img_eff_02")?],
            base: (picture(&common, &style_of(cbase_e)?)?, cbase_at, pair(cbase_e, "size_wh", [0.0; 2])[0]),
            basket_icon: (picture(&icons, &style_of(icon_e)?)?, icon_at, pair(icon_e, "size_wh", [0.0; 2])[0]),
            count_base: (picture(&various, &style_of(num_bg)?)?, num_at, scale_of(num_bg), pair(num_bg, "size_wh", [0.0; 2]), pair(num_bg, "pos", [0.0; 2])),
            count: label(num_e, pair(num_e, "pos", [0.0; 2])),
            count_origin: num_at,
            disc: (picture(&field, &style_of(bg_e)?)?, disc_at),
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
        })
    }

    /// The texts of `STR_UI_nnnn`.
    fn string(&self, n: u32) -> &str {
        self.strings.get(&(STR_UI_BASE + n)).map_or("", String::as_str)
    }

    /// The HUD's quads for `state` on a `screen` of pixels, in the order the game draws them, and
    /// its clips: the minimap's mask is a stencil the terrain and her arrow draw inside (ryza-recomp
    /// `hud-walk`: the mask quad writes stencil 1 with no colour, the terrain, markers and arrow
    /// test equal to 1); the tool disc's mask goes to a target of its own the tool's picture is
    /// multiplied by, drawn here as a mask that clips nothing (the tool sits inside the disc).
    pub fn quads(&self, state: &State, markers: &Markers, screen: (u32, u32)) -> (Vec<Quad>, Vec<Clip>, std::ops::Range<usize>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        let mut pen = Pen { hud: self, k, left, out: Vec::new() };
        let white = [1.0; 4];

        // The minimap.
        pen.picture(FIELD, self.grid.0, self.grid.1, 1.0, white);
        let map_mask = pen.out.len();
        pen.picture(FIELD, self.mask.0, self.mask.1, self.mask.2, white);
        let center = self.arrow_center();
        if state.map {
            pen.terrain(state.at, center);
        }
        // The markers, clipped with the terrain: the ways out, then the townsfolk.
        let beside = |at: &[f32; 2]| [center[0] + (at[0] - state.at[0]) / MARKER_UNITS_PER_PIXEL, center[1] + (at[1] - state.at[2]) / MARKER_UNITS_PER_PIXEL];
        for at in &markers.exits {
            pen.centered(FIELD, self.exit_icon, beside(at), 1.0, white);
        }
        for at in &markers.folk {
            pen.centered(FIELD, self.folk_icon, beside(at), 1.0, white);
        }
        pen.arrow(self.arrow, center, state.turn);
        let mut clips = vec![Clip { mask: map_mask, quads: map_mask + 1..pen.out.len() }];
        pen.picture(FIELD, self.gmd.0, self.gmd.1, 1.0, white);
        pen.picture(FIELD, self.frame.0, self.frame.1, 1.0, white);
        pen.picture(FIELD, self.north.0, self.north.1, 1.0, white);
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
            let (alpha, scale) = glow_pulse(state.tick);
            let tint = |a: f32| [GLOW_TINT[0], GLOW_TINT[1], GLOW_TINT[2], a];
            pen.centered(FIELD, self.destination_icon, at, 1.0, white);
            pen.centered(GLOW, self.glows[0], at, scale, tint(alpha));
            pen.centered(GLOW, self.glows[1], at, 1.0, tint(alpha));
        }
        let glow = glow..pen.out.len();

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
        (pen.out, clips, glow)
    }

    /// Where her arrow is: the middle of the minimap's window, which the terrain is moved under.
    // STAND-IN: The arrow's middle (1729, 886 of the capture's 1080 lines) is read off the capture; the real thing is the player marker's place in the minimap's own node tree, which the code moves the terrain under.
    fn arrow_center(&self) -> [f32; 2] {
        [1729.0 / 1.5, 886.0 / 1.5]
    }
}

// STAND-IN: The minimap's scale and place for Rasenboden are fitted to six captured frames (hud-walk 2030 to 3330, her place from the walk's state): the terrain's left edge falls 1 layout pixel per 120 game units, its top edge 1 per 125, and x = 0, z = 0 put them at (1163.40, 590.66); the game's own per-map bounds are not found in its tables.
/// Layout pixels the terrain moves per game unit of her walk (x, z), and its corner at the origin.
const TERRAIN_PER_UNIT: [f32; 2] = [1.0 / 120.0, 1.0 / 125.0];
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
fn glow_pulse(tick: f32) -> (f32, f32) {
    let t = (tick - GLOW_PHASE).rem_euclid(GLOW_PERIOD);
    let alpha = match t {
        t if t < 5.0 => 0.0,
        t if t < 10.0 => (t - 5.0) / 5.0,
        t => ((70.0 - t) / 60.0).max(0.0),
    };
    let u = ((t - 10.0) / 60.0).clamp(0.0, 1.0);
    // Quantised to bytes, as the game's colour.
    ((alpha * 255.0).round() / 255.0, 0.8 + 0.2 * (1.0 - (1.0 - u).powi(3)))
}

const TERRAIN_ORIGIN: [f32; 2] = [1163.40, 590.66];
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
        // The atlases follow the two fonts.
        let (aw, ah) = self.hud.atlases[texture - FIELD];
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
    fn terrain(&mut self, at: [f32; 3], _arrow: [f32; 2]) {
        let (x, y) = (TERRAIN_ORIGIN[0] - at[0] * TERRAIN_PER_UNIT[0], TERRAIN_ORIGIN[1] - at[2] * TERRAIN_PER_UNIT[1]);
        let [l, t] = self.at([x, y]);
        let s = TERRAIN_SIZE * self.k;
        self.quad(MINIMAP, [0.0, 0.0, 2.0, 2.0], [l, t, l + s, t + s], [1.0; 4]);
    }

    /// Her arrow turned about its middle to face the way she does on the fixed-north map. The
    /// game keeps the turn within a quarter either side of the picture's own upright and flips
    /// the picture's texture coordinates for the other half (the capture's six frames).
    fn arrow(&mut self, p: [f32; 4], center: [f32; 2], turn: f32) {
        let [cx, cy] = self.at(center);
        let (w, h) = (p[2] / PER_LAYOUT * self.k, p[3] / PER_LAYOUT * self.k);
        let mut r = (180.0 - turn).rem_euclid(360.0);
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

    fn glyphs(&mut self, l: &Label, origin: [f32; 2], shift: [f32; 2], text: &str, color: [f32; 4]) {
        let font = usize::from(!l.small);
        let glyphs = &self.hud.glyphs[font];
        let s = l.size * self.k / ATLAS_SIZE;
        let width: f32 = text.chars().filter_map(|c| glyphs.get(&c)).map(|g| g.advance as f32 * s).sum();
        let x = self.left + (origin[0] + l.at[0] + shift[0]) * self.k;
        let mut pen = if l.center { x - width / 2.0 } else { x };
        let top = (origin[1] + l.at[1] + shift[1] + TEXT_TOP) * self.k;
        let (aw, ah) = (4096.0, 2048.0);
        for g in text.chars().filter_map(|c| glyphs.get(&c)) {
            let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
            let (x0, y0) = (pen + g.x_offset as f32 * s, top + g.y_offset as f32 * s);
            self.quad(if font == 0 { FONT01 } else { FONT00 }, [gx / aw, gy / ah, (gx + gw) / aw, (gy + gh) / ah], [x0, y0, x0 + gw * s, y0 + gh * s], color);
            pen += g.advance as f32 * s;
        }
    }
}
