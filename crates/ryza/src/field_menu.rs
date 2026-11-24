//! The field's menu (Y): the six buttons down the left, the party's photo with its three cards,
//! the money and level plate at the top right and the help bar, from the game's own layouts
//! (`uil_a21_main_menu.xml` for the buttons, the plates and the money, `uil_a21_main_menu_chara.xml`
//! for the photo and the cards, `uia_a21_main_menu.xml` for the glows' and slides' animations,
//! the styles of `uis_gen_a21_main_menu.xml`).
//!
//! Positions are in the 1280 x 720 layout and scale by `screen height / 720`, the layout centred
//! across the width (as the HUD's). Times are in the capture's game frames: [`Nav`] holds when
//! the menu opened, where the cursor is and when it came to or left each button, and
//! [`View::quads`] draws any frame from that (a window steps it with the keys, an offscreen run
//! replays the script's keys).

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;
use crate::load_view::{style, Style, PLATE_FRAMES, PLATE_LEAVE, PLATE_MIN, TEXT_TOP};
use crate::title::{curve, find, numbers, pair, Curve, LAYOUT_HEIGHT};

/// The textures of the menu's layer, in this order.
pub const FONT01: usize = 0;
pub const FONT00: usize = 1;
pub const BACKGROUND: usize = 2;
pub const PHOTO: usize = 3;
pub const MENU0: usize = 4;
pub const MENU1: usize = 5;
pub const MENU2: usize = 6;
pub const WHITE: usize = 7;
pub const MIX: usize = 8;
pub const ICON: usize = 9;
pub const GUIDE: usize = 10;
/// The cut-out figures of the characters (`a21v2_main_menu_image`) and the turning ring of a picked card (`a21_common` 5).
pub const FIGURES: usize = 11;
pub const RING: usize = 12;
pub const TEXTURES: usize = 13;

/// The styles' pixels count three to a layout pixel; the menu's textures hold half the styles'
/// pixels (`uvwh` runs to twice the texture's size).
const PER_LAYOUT: f32 = 3.0;
const STYLE_OVER_TEXTURE: f32 = 2.0;
/// The first of the string table's UI strings (`STR_UI_0000`).
const STR_UI_BASE: u32 = 6_619_137;
/// The first character name (Ryza); a character's id counts up from it.
const CHARACTER_NAMES: u32 = 262_148;
const CHARACTERS: u32 = 40;
const BROWN: [f32; 3] = [26.0 / 255.0, 0.0, 0.0];

/// The six buttons in cursor order, each with the picture of the name and icon lists
/// (`image_no`) the game shows on it: Container, Equip, Gathering Tools, Status, Guide, Settings.
// STAND-IN: The pictures' numbers are read off the capture's uv (the game sets each button's `image_no` in code); the order is the capture's cursor order.
pub const BUTTONS: [usize; 6] = [1, 3, 4, 6, 7, 8];
/// The Settings button's place in the cursor order.
pub const SETTINGS: usize = 5;

/// Frames from the menu's first frame to the cursor's first coming to its button (the cards'
/// fade has run out).
const CURSOR_AT: f32 = 30.0;
/// Frames the cursor's glow takes to come on (`anm_cursor_on`) and go (`anm_cursor_off`), and
/// the loop's length (`anm_cursor_on_loop`).
const ON_FRAMES: f32 = 18.0;
const OFF_FRAMES: f32 = 9.0;
/// The cursor's glow as the menu closes on B: it goes out linearly, starting `CLOSE_GLOW_AT` frames after the buttons' going and taking
/// `CLOSE_GLOW_FRAMES` (STAND-IN: fitted to the capture's two samples, 170/255 on 2555 and 20/255 on 2560; the slide of no one button fits both).
const CLOSE_GLOW_AT: f32 = 1.17;
const CLOSE_GLOW_FRAMES: f32 = 8.5;
/// The speed of the cursor's select pulse (`anim_ctrl select`).
const SELECT_SPEED: f32 = 1.5;
// STAND-IN: The loops run 121 frames (the animation's 120 and a frame at its end), as the Settings screen's cursor blink does (`options::BLINK_LOOP`, read off the same kind of capture); the real thing is the game's own animation clock.
const LOOP_FRAMES: f32 = 121.0;
/// The ring's own loop, 121 frames like the glow's (`field-menu`: the first loops; the pick's capture 2150 and 2560 after 4 and 5 loops). After the Settings screen the
/// ring is a frame ahead (`Nav::ring_skew`: `field-menu` 2440 and 2445).
const RING_LOOP_FRAMES: f32 = 121.0;
/// Frames the photo and the cards fade in over (`anm_common_juvenile_fade_in` at speeds 3 and 2).
const PHOTO_FADE: f32 = 20.0;
const CARD_FADE: f32 = 30.0;
/// Frames the background fades in over (`anm_common_fade_in`), the buttons slide in over
/// (`anm_menu_slide_in`) and the money plates (`anm_common_slide_in_right` at speed 1.5) and the
/// corner picture (speed 1).
const BG_FADE: f32 = 15.0;
const SLIDE: f32 = 12.0;
const SLIDE_MONEY: f32 = 20.0;
const SLIDE_PUT: f32 = 30.0;
/// The delay between one button's slide and the next's (`get_in`: 0.04 s).
const SLIDE_STEP: f32 = 2.4;
/// Closing: B at frame F takes `CLOSE_LAG` frames to start; the buttons go `SLIDE` frames each, one
/// `CLOSE_STEP` frames after the one before (`get_out`: 0.02 s), the light's `CLOSE_LIGHT_AT` frames
/// in; the money and the level plate go at 2.25 times the pace (`get_out_mix`: 1.5 over 1.5), the corner
/// picture at 3 (`get_out_others`); the menu is gone `BG_FADE` frames in, when the background has faded.
pub const CLOSE_LAG: f32 = 1.0;
const CLOSE_STEP: f32 = 1.2;
const CLOSE_LIGHT_AT: f32 = 8.4;
const CLOSE_MONEY_SPEED: f32 = 2.25;
const CLOSE_PUT_SPEED: f32 = 3.0;
/// The help bar's fade and its start after the menu's.
const HELP_FADE: f32 = 15.0;
const HELP_AT: f32 = 33.0;
/// The same after the Settings screen (`field-menu`: 1/3 of the fade on 2440, the menu's frames from 2402), and the frames its plate starts later.
const HELP_AT_BACK: f32 = 33.0;
const BACK_PLATE_LATE: f32 = 0.2;

// STAND-IN: The help bar's button glyphs (private-use characters of `mainfont01`) are the capture's; the game picks them by the pad's assignment. The words are the string table's.
const HELP_ENTRIES: [(char, u32); 5] = [('\u{4e1}', 4_194_366), ('\u{4ec}', 4_194_467), ('\u{4eb}', 4_194_451), ('\u{4d8}', 4_194_368), ('\u{4da}', 4_194_367)];
/// The help bar's plate and text come from the load screen's bar (`uil_a21_shade.xml`).
const MENU_WORDS: [u32; 5] = [HELP_ENTRIES[0].1, HELP_ENTRIES[1].1, HELP_ENTRIES[2].1, HELP_ENTRIES[3].1, HELP_ENTRIES[4].1];

/// The card's place in the photo's list (`list_0`: the clones 0, 2 and 3 for a party of three).
const CARD_SLOTS: [&str; 3] = ["clone_regular_status_0", "clone_regular_status_2", "clone_regular_status_3"];

/// What the menu reads of the game's files.
pub struct Sources<'a> {
    pub layout: &'a [u8],
    pub chara: &'a [u8],
    pub styles: &'a [u8],
    pub photo_styles: &'a [u8],
    pub mix_styles: &'a [u8],
    pub icon_styles: &'a [u8],
    pub anims: &'a [u8],
    pub common_anims: &'a [u8],
    pub help: &'a [u8],
    pub help_styles: &'a [u8],
    pub strings: &'a [u8],
    pub glyphs: [HashMap<char, Glyph>; 2],
    /// The styles of the characters' figures (`uis_gen_a21v2_main_menu_image.xml`).
    pub figure_styles: &'a [u8],
    /// The textures' sizes in pixels: photo, the three menu atlases, mix, icon, guide, figures, ring.
    pub sizes: [(u32, u32); 9],
}

/// A party member the cards show.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: u8,
    pub level: u8,
    /// The numbers the cards and pages show (`party_stats::Tables::stats`) and the worn items (the Equipment page's slots).
    pub stats: crate::party_stats::Stats,
    pub worn: [Option<crate::save::Worn>; 4],
}

/// What the menu shows of the play.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub party: Vec<Member>,
    pub money: u32,
}

/// A text node: where, how big, its font (0 is `MainFont01`), how aligned and spaced.
#[derive(Clone, Copy, Debug)]
struct Label {
    at: [f32; 2],
    size: f32,
    /// The node's own scale (it scales the line's top offset too).
    scale: f32,
    font: usize,
    center: bool,
    right: bool,
    spacing: f32,
    color: [f32; 3],
}

impl Label {
    fn of(e: &Element, parent: [f32; 2]) -> Label {
        let p = pair(e, "pos", [0.0; 2]);
        let c = numbers(e, "font_color");
        Label {
            at: [parent[0] + p[0], parent[1] + p[1]],
            size: e.attr("font_size").and_then(|v| v.trim().parse().ok()).unwrap_or(17.0) * pair(e, "scale", [1.0; 2])[0],
            scale: pair(e, "scale", [1.0; 2])[0],
            font: usize::from(e.attr("font") != Some("MainFont01")),
            center: e.attr("align") == Some("center"),
            right: e.attr("align") == Some("right"),
            spacing: e.attr("word_space").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0),
            color: if c.len() >= 3 { [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0] } else { BROWN },
        }
    }
}

/// A card's parts, within the card's node.
#[derive(Clone, Debug)]
struct Card {
    plate: ([f32; 2], Style),
    icon: ([f32; 2], Style),
    hp: Label,
    hp_value: Label,
    hp_slash: Label,
    hp_max: Label,
    name: Label,
    level: Label,
}

pub struct View {
    glyphs: [HashMap<char, Glyph>; 2],
    /// The atlases in style pixels (the textures' sizes twice over), indexed by texture.
    atlases: HashMap<usize, (f32, f32)>,
    strings: HashMap<u32, String>,
    buttons: Vec<[f32; 2]>,
    light: ([f32; 2], Style),
    base: Style,
    cursor: Style,
    cursor_on: Style,
    ring: Style,
    names: Style,
    icons: Style,
    ring_at: [f32; 2],
    ring_scale: f32,
    pivot: [f32; 2],
    base_size: f32,
    level_plate: ([f32; 2], Style),
    money_bg: ([f32; 2], [f32; 2], f32, Style),
    money_icon: ([f32; 2], f32, Style),
    money_text: Label,
    put: ([f32; 2], Style),
    shadow: ([f32; 2], [f32; 2], Style),
    photo: ([f32; 2], [f32; 2], f32, Style),
    mask: ([f32; 2], [f32; 2], f32),
    cards: Vec<([f32; 2], Card)>,
    help: Style,
    help_at: [f32; 2],
    help_text: Label,
    help_left: f32,
    slide: [Curve; 2],
    slide_right: Curve,
    cursor_loop: [Curve; 2],
    select: [Curve; 2],
    ring_loop: [Curve; 2],
    pick: PickLook,
}

/// What a picked character's card and figure are made of (`uil_a21_main_menu_chara.xml`'s `regular_status` and `chara`).
#[derive(Clone, Debug)]
struct PickLook {
    /// The glow behind the card (`bg_hilight_on`), its ring (`bg_hilight_scale`: place, pivot, scale) and the turning ring (`bg_hilight_roll`: place, picture in `RING`, scale, pivot).
    glow: ([f32; 2], Style),
    ring: ([f32; 2], [f32; 2], f32),
    roll: ([f32; 2], [f32; 4], f32, [f32; 2]),
    /// The glow over the plate (`selected_bg`): place, pivot and scale.
    over: ([f32; 2], [f32; 2], f32),
    /// The blurred photo's quad (`blur_bg`): place, size, degrees.
    blur: ([f32; 2], [f32; 2], f32),
    /// The characters' figures by name: place, degrees and picture.
    figures: HashMap<String, ([f32; 2], f32, [f32; 4])>,
    blackout: Curve,
    blink: Curve,
    rolling: Curve,
}

fn first(s: &Style) -> Result<[f32; 4], String> {
    s.pictures.first().copied().ok_or_else(|| "a style without a picture".to_string())
}

impl View {
    pub fn read(src: Sources) -> Result<View, String> {
        let layout = table::parse(src.layout).map_err(|e| format!("uil_a21_main_menu: {e}"))?;
        let chara = table::parse(src.chara).map_err(|e| format!("uil_a21_main_menu_chara: {e}"))?;
        let styles = table::parse(src.styles).map_err(|e| format!("uis_gen_a21_main_menu: {e}"))?;
        let photo_styles = table::parse(src.photo_styles).map_err(|e| format!("uis_a21_main_menu_image: {e}"))?;
        let mix = table::parse(src.mix_styles).map_err(|e| format!("uis_gen_a21_mix: {e}"))?;
        let icons = table::parse(src.icon_styles).map_err(|e| format!("uis_gen_a21_icon: {e}"))?;
        let help_layout = table::parse(src.help).map_err(|e| format!("uil_a21_shade: {e}"))?;
        let help_styles = table::parse(src.help_styles).map_err(|e| format!("uis_gen_a21_common: {e}"))?;
        let strings_root = table::parse(src.strings).map_err(|e| format!("strcombineall: {e}"))?;
        let strings = strings_root
            .children
            .iter()
            .filter_map(|s| Some((s.attr("String_No")?.parse::<u32>().ok()?, s.attr("Text")?.to_string())))
            .filter(|(n, _)| (CHARACTER_NAMES..CHARACTER_NAMES + CHARACTERS).contains(n) || MENU_WORDS.contains(n) || PICK_ENTRIES.iter().any(|(_, w)| w == n) || crate::field_hud::HELP_ENTRIES.iter().any(|(_, w)| w == n))
            .collect();
        let missing = |what: &str| format!("uil_a21_main_menu: no {what}");
        fn named<'a>(e: &'a Element, name: &str) -> Result<(&'a Element, [f32; 2]), String> {
            find(e, name, [0.0; 2]).ok_or_else(|| format!("uil_a21_main_menu: no {name}"))
        }
        let at = |name: &str| -> Result<[f32; 2], String> { Ok(named(&layout, name)?.1) };
        let style_of = |e: &Element, name: &str| -> Result<Style, String> { style(&styles, e.attr("style").ok_or(format!("{name}: no style"))?) };

        // The buttons: the first one's parts and every button's place.
        let (first_button, _) = named(&layout, "menu_node")?;
        let in_button = |name: &str| -> Result<(&Element, [f32; 2]), String> { find(first_button, name, [0.0; 2]).ok_or_else(|| missing(name)) };
        let (bg, _) = in_button("bg")?;
        let (cursor, _) = in_button("bg_hilight")?;
        let (cursor_on, _) = in_button("bg_hilight_on")?;
        let (ring, ring_at) = in_button("bg_hilight_scale")?;
        let (name, name_at) = in_button("name")?;
        let (icon, icon_at) = in_button("icon")?;
        let buttons = (0..BUTTONS.len()).map(|i| at(&format!("select_{i}"))).collect::<Result<Vec<_>, String>>()?;
        let _ = (name_at, icon_at);
        let light_node = named(&layout, "menu_select_bg")?;
        let level_node = named(&layout, "status_bg_def")?;
        let money_node = named(&layout, "money_bg")?;
        // (`find` adds the starting node's own place: start from where its parent is.)
        let money_base = {
            let own = pair(money_node.0, "pos", [0.0; 2]);
            [money_node.1[0] - own[0], money_node.1[1] - own[1]]
        };
        let (money_bg, money_bg_at) = find(money_node.0, "bg", money_base).ok_or_else(|| missing("money bg"))?;
        let (money_icon, money_icon_at) = find(money_node.0, "icon", money_base).ok_or_else(|| missing("money icon"))?;
        let (money_param, money_param_at) = find(money_node.0, "param", money_base).ok_or_else(|| missing("money param"))?;
        let put_node = named(&layout, "mm_put")?;

        // The photo and its cards.
        let (photo_node, photo_at) = named(&chara, "photo")?;
        let (back_node, back_at) = named(&chara, "back")?;
        let (mask_node, mask_at) = named(&chara, "character_mask")?;
        let (regular, _) = named(&chara, "regular_status")?;
        let part = |name: &str| -> Result<(&Element, [f32; 2]), String> { find(regular, name, [0.0; 2]).ok_or_else(|| format!("uil_a21_main_menu_chara: no {name}")) };
        let (plate_node, plate_at) = part("status_bg")?;
        let (icon_node, icon_node_at) = part("battle_join")?;
        let (hp, hp_at) = part("hp")?;
        let label_in = |parent: &Element, parent_at: [f32; 2], name: &str| -> Result<Label, String> {
            let (e, _) = find(parent, name, [0.0; 2]).ok_or_else(|| format!("uil_a21_main_menu_chara: no {name}"))?;
            Ok(Label::of(e, parent_at))
        };
        let (name_node, _) = part("chara_name")?;
        let (level_text, _) = part("Lv_value")?;
        let card = Card {
            plate: (plate_at, style(&styles, plate_node.attr("style").ok_or("status_bg: no style")?)?),
            icon: (icon_node_at, style(&styles, icon_node.attr("style").ok_or("battle_join: no style")?)?),
            hp: Label::of(hp, [0.0; 2]),
            hp_value: label_in(hp, hp_at, "HP_value")?,
            hp_slash: label_in(hp, hp_at, "HP_slash")?,
            hp_max: label_in(hp, hp_at, "MHP_value")?,
            name: Label::of(name_node, [0.0; 2]),
            level: Label::of(level_text, [0.0; 2]),
        };
        let cards = CARD_SLOTS.iter().map(|s| Ok((named(&chara, s)?.1, card.clone()))).collect::<Result<Vec<_>, String>>()?;

        let (glow_node, glow_at) = part("bg_hilight_on")?;
        let (hl_ring, hl_ring_at) = part("bg_hilight_scale")?;
        let (hl_roll, hl_roll_at) = part("bg_hilight_roll")?;
        let (over_node, over_at) = part("selected_bg")?;
        let (blur_node, blur_at) = named(&chara, "blur_bg")?;
        let figure_styles = table::parse(src.figure_styles).map_err(|e| format!("uis_gen_a21v2_main_menu_image: {e}"))?;
        let mut figures = HashMap::new();
        for who in ["Tao", "Ryza", "Lent"] {
            let (e, at) = named(&chara, &format!("a21v2_main_menu_image_00_{who}"))?;
            let picture = first(&style(&figure_styles, e.attr("style").ok_or("a figure: no style")?)?)?;
            figures.insert(who.to_string(), (at, numbers(e, "rot").get(2).copied().unwrap_or(0.0), picture));
        }
        let roll_picture = first(&style(&help_styles, hl_roll.attr("style").ok_or("bg_hilight_roll: no style")?)?)?;
        let pick = PickLook {
            glow: (glow_at, style(&styles, glow_node.attr("style").ok_or("bg_hilight_on: no style")?)?),
            ring: (hl_ring_at, pair(hl_ring, "pivot_pos", [0.0; 2]), pair(hl_ring, "scale", [1.0; 2])[0]),
            roll: (hl_roll_at, roll_picture, pair(hl_roll, "scale", [1.0; 2])[0], pair(hl_roll, "pivot_pos", [0.0; 2])),
            over: (over_at, pair(over_node, "pivot_pos", [0.0; 2]), pair(over_node, "scale", [1.0; 2])[0]),
            blur: (blur_at, [numbers(blur_node, "size_w").first().copied().unwrap_or(0.0), numbers(blur_node, "size_h").first().copied().unwrap_or(0.0)], numbers(blur_node, "rot").get(2).copied().unwrap_or(0.0)),
            figures,
            blackout: curve(src.anims, "anm_select_blackout", "color")?,
            blink: curve(src.anims, "anm_status_blink_loop", "color")?,
            rolling: curve(src.anims, "anm_cursor_rolling", "rot")?,
        };

        let photo_style = style(&photo_styles, "etc_a21_main_menu_image")?;
        let size_of = |e: &Element| -> [f32; 2] { pair(e, "size_wh", [0.0; 2]) };
        let atlas = |texture: usize, (w, h): (u32, u32)| (texture, (w as f32 * STYLE_OVER_TEXTURE, h as f32 * STYLE_OVER_TEXTURE));
        let [photo_size, menu0, menu1, menu2, mix_size, icon_size, guide_size, figure_size, ring_size] = src.sizes;
        let atlases = HashMap::from([atlas(PHOTO, photo_size), atlas(MENU0, menu0), atlas(MENU1, menu1), atlas(MENU2, menu2), atlas(MIX, mix_size), atlas(ICON, icon_size), atlas(GUIDE, guide_size), atlas(FIGURES, figure_size), atlas(RING, ring_size)]);

        // The help bar's plate and text (as the load screen's).
        let (help_bg, help_at) = find(&help_layout, "base", [0.0; 2]).ok_or("uil_a21_shade: no base")?;
        let (_, left_side) = crate::load_view::inside(help_bg, "left_side", [0.0; 2]).ok_or("uil_a21_shade: no left_side")?;
        let (help_node, _) = find(&help_layout, "bg", [0.0; 2]).ok_or("uil_a21_shade: no bg")?;
        let (help_text_node, help_text_at) = crate::load_view::inside(help_node, "text", [0.0; 2]).ok_or("uil_a21_shade: no text")?;

        Ok(View {
            glyphs: src.glyphs,
            atlases,
            strings,
            buttons,
            light: (light_node.1, style_of(light_node.0, "menu_select_bg")?),
            base: style_of(bg, "bg")?,
            cursor: style_of(cursor, "bg_hilight")?,
            cursor_on: style_of(cursor_on, "bg_hilight_on")?,
            ring: style_of(ring, "bg_hilight_scale")?,
            names: style_of(name, "name")?,
            icons: style_of(icon, "icon")?,
            ring_at,
            ring_scale: pair(ring, "scale", [1.0; 2])[0],
            pivot: pair(cursor, "pivot_pos", [0.0; 2]),
            base_size: size_of(bg)[0],
            level_plate: (level_node.1, style_of(level_node.0, "status_bg_def")?),
            money_bg: (money_bg_at, size_of(money_bg), pair(money_bg, "scale", [1.0; 2])[0], style(&mix, money_bg.attr("style").ok_or("money bg: no style")?)?),
            money_icon: (money_icon_at, pair(money_icon, "scale", [1.0; 2])[0], style(&icons, money_icon.attr("style").ok_or("money icon: no style")?)?),
            money_text: Label::of(money_param, [money_param_at[0] - pair(money_param, "pos", [0.0; 2])[0], money_param_at[1] - pair(money_param, "pos", [0.0; 2])[1]]),
            put: (put_node.1, style_of(put_node.0, "mm_put")?),
            shadow: (back_at, size_of(back_node), style(&styles, back_node.attr("style").ok_or("back: no style")?)?),
            photo: (photo_at, size_of(photo_node), numbers(photo_node, "rot").get(2).copied().unwrap_or(0.0), photo_style),
            mask: (mask_at, [numbers(mask_node, "size_w").first().copied().unwrap_or(0.0), numbers(mask_node, "size_h").first().copied().unwrap_or(0.0)], numbers(mask_node, "rot").get(2).copied().unwrap_or(0.0)),
            cards,
            help: style(&help_styles, help_bg.attr("style").ok_or("uil_a21_shade: base has no style")?)?,
            help_at,
            help_text: Label { at: help_text_at, ..Label::of(help_text_node, [0.0; 2]) },
            help_left: left_side[0],
            slide: [curve(src.anims, "anm_menu_slide_in", "color")?, curve(src.anims, "anm_menu_slide_in", "pos")?],
            slide_right: curve(src.common_anims, "anm_common_slide_in_right", "pos")?,
            cursor_loop: [curve(src.anims, "anm_cursor_on_loop", "color")?, curve(src.anims, "anm_cursor_on_loop", "scale")?],
            select: [curve(src.anims, "anm_cursor_select", "color")?, curve(src.anims, "anm_cursor_select", "scale")?],
            ring_loop: [curve(src.anims, "anm_cursor_scale", "color")?, curve(src.anims, "anm_cursor_scale", "scale")?],
            pick,
        })
    }

    /// The first `n` cards' indexes from left to right (the pick moves along them).
    pub fn card_order(&self, n: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..n.min(self.cards.len())).collect();
        order.sort_by(|&a, &b| self.cards[a].0[0].total_cmp(&self.cards[b].0[0]));
        order
    }

    fn word(&self, n: u32) -> &str {
        self.strings.get(&n).map_or("", String::as_str)
    }

    fn name_of(&self, id: u8) -> &str {
        self.word(CHARACTER_NAMES + u32::from(id))
    }
}

/// Where the player is in the menu: when it opened and closed, the cursor and when it came to
/// and left each button.
#[derive(Clone, Debug, PartialEq)]
pub struct Nav {
    /// When the menu's nodes were made: the ring's loop runs from it.
    pub opened: f32,
    /// When the buttons, cards and plates last began to come in (the first frame; again after the Settings screen), and whether the background and the photo were there already.
    pub shown: f32,
    pub warm: bool,
    pub closed: Option<f32>,
    pub cursor: usize,
    on: [Option<f32>; BUTTONS.len()],
    off: [Option<f32>; BUTTONS.len()],
    /// When A was pressed on the cursor's button.
    pub chosen: Option<f32>,
    /// Whether A went on to the Settings screen (B comes back to the menu).
    pub in_settings: bool,
    /// The character pick A on Equip or Status leads to.
    pub pick: Option<Pick>,
    /// When the menu came back from a page (the blurred photo fades out from it).
    pub blur_out: Option<f32>,
    /// Frames the cursor's ring runs ahead of its loop (1 after the Settings screen).
    ring_skew: f32,
}

/// The character pick (`field-menu-equip`: A on Equip at 2060, the pick's first frame 2102).
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    /// The pick's first frame: the buttons begin to darken and the photo to blur.
    pub first: f32,
    /// The picked card (an index of the party) and the frame it was picked on (the first frame at the start).
    pub card: usize,
    pub since: f32,
    /// A on the card: the page (Equipment or Status; not drawn yet, the pick stands in for it) opens.
    pub confirmed: Option<f32>,
    /// B on the page.
    pub leaving: Option<f32>,
    /// The page's slot under the cursor (Equipment: weapon, armour, the two accessories), moved by up and down.
    pub slot: usize,
    /// The frame the cursor came to its slot (its blink restarts there); none: the page's first.
    pub slot_since: Option<f32>,
    /// The Status page (the button was Status) and not the Equipment page.
    pub status: bool,
    /// The Status page's skill under the cursor (a row of its list).
    pub row: usize,
}

impl Pick {
    /// Frames from A on the card to the page's first frame.
    pub fn page_at(&self) -> f32 {
        if self.status { STATUS_PAGE_AT } else { PAGE_AT }
    }
}

/// Frames from A on the character to the page's first frame (STAND-IN: the capture's page is whole by 2290, the first sample
/// after 2150; the opening is not measured).
pub const PAGE_AT: f32 = 10.0;

/// The Status page's first frame, after A on the card (`field-menu-status`: A on 2320, the page's parts at 0.704 of their
/// slides on 2330, so they began on 2320 and nothing shows before their first step).
pub const STATUS_PAGE_AT: f32 = 1.0;

/// STAND-IN: the tactics level the skills list is for (the save's own is not read; the capture's is the first).
pub const TACTICS_LEVEL: u8 = 1;

/// Frames from B on a page to the menu's coming back (`field-menu-equip`: B on 2500; the cards at 6/30 of their fade and the
/// photo's blur at 24/30 left on 2530, so both ran from 2524).
pub const PAGE_OUT: f32 = 24.0;

/// Frames from A on Equip or Status to the pick's first frame (`field-menu-equip`: A on 2060; the buttons' tint, the photo's
/// blur and the extra button pictures all run from 2102), and from the pick's first frame to the card glow's loops (2103).
pub const PICK_AT: f32 = 42.0;
const PICK_LOOPS_LATER: f32 = 1.0;
/// Frames from the pick's first frame to its help bar's first frame (the fade from 2106).
const PICK_BAR_LATER: f32 = 4.0;
/// The buttons darken over 20 frames (`anm_select_blackout` run at three times its speed), the photo blurs in over 30
/// (`blur_in`, speed 2) and the extra button pictures fade in over 20.
const BLACKOUT_FRAMES: f32 = 20.0;
const BLUR_FRAMES: f32 = 30.0;
/// The rolling ring turns a full circle in 480 frames (`anm_cursor_rolling`), the card's ring loops in 120 (`anm_cursor_scale`),
/// the glow over the plate in 120 (`anm_status_blink_loop`).
const ROLL_FRAMES: f32 = 480.0;
const LOOP_PICK_FRAMES: f32 = 120.0;
/// The Equip and Status buttons' places in the cursor order.
const EQUIP: usize = 1;
const STATUS: usize = 3;

impl Nav {
    /// The menu opening at frame `opened` (its first frame), the cursor on the first button
    /// once the cards have faded in.
    pub fn open(opened: f32) -> Nav {
        let mut on = [None; BUTTONS.len()];
        on[0] = Some(opened + CURSOR_AT);
        Nav { opened, shown: opened, warm: false, closed: None, cursor: 0, on, off: [None; BUTTONS.len()], chosen: None, in_settings: false, pick: None, blur_out: None, ring_skew: 0.0 }
    }

    /// The cursor moves `way` buttons (wrapping) at frame `at`.
    pub fn step(&mut self, way: i32, at: f32) {
        let next = (self.cursor as i32 + way).rem_euclid(BUTTONS.len() as i32) as usize;
        if next == self.cursor {
            return;
        }
        self.off[self.cursor] = Some(at);
        self.on[next] = Some(at);
        self.off[next] = None;
        self.cursor = next;
    }

    /// B on the Settings screen at frame `at`: the menu comes in again (the cards and the buttons, over the background and the photo, which stay) with the cursor where it was.
    pub fn back_from_settings(&mut self, at: f32) {
        self.in_settings = false;
        self.chosen = None;
        self.ring_skew = 1.0;
        self.shown = at + SETTINGS_BACK_LAG;
        self.warm = true;
        self.on = [None; BUTTONS.len()];
        self.off = [None; BUTTONS.len()];
        self.on[self.cursor] = Some(self.shown + CURSOR_AT);
    }

    /// A on the cursor's button at frame `at`.
    pub fn choose(&mut self, at: f32) {
        self.chosen = Some(at);
        self.in_settings = self.cursor == SETTINGS;
        if matches!(self.cursor, EQUIP | STATUS) {
            // (the leader's card is picked first)
            self.pick = Some(Pick { first: at + PICK_AT, card: 0, since: at + PICK_AT, confirmed: None, leaving: None, slot: 0, slot_since: None, status: self.cursor == STATUS, row: 0 });
        }
    }

    /// A key of the pick at frame `at`: left or right moves it along the cards (`order`: the party's cards from left to
    /// right) and stops at the ends (STAND-IN: not tried at an end); keys before its first frame do nothing.
    pub fn pick_step(&mut self, way: i32, at: f32, order: &[usize]) {
        let Some(p) = self.pick.as_mut().filter(|p| at >= p.first && p.confirmed.is_none()) else { return };
        let Some(rank) = order.iter().position(|&c| c == p.card) else { return };
        let next = (rank as i32 + way).clamp(0, order.len() as i32 - 1) as usize;
        if order[next] != p.card {
            p.card = order[next];
            p.since = at;
        }
    }

    /// A on the picked card.
    pub fn pick_confirm(&mut self, at: f32) {
        if let Some(p) = self.pick.as_mut().filter(|p| at >= p.first && p.confirmed.is_none()) {
            p.confirmed = Some(at);
        }
    }

    /// Whether a page is up at frame `at`: A was pressed on a card `PAGE_AT` frames ago and B has not left it.
    pub fn page(&self, at: f32) -> Option<&Pick> {
        self.pick.as_ref().filter(|p| p.confirmed.is_some_and(|c| at >= c + p.page_at()) && p.leaving.is_none())
    }

    /// A direction on the Equipment page: the four slots are two rows of two (weapon and armour, the two accessories), so down
    /// goes from the weapon to the first accessory (`field-menu-equip`: DOWN on 2300, the cursor on the third slot) and left and
    /// right along a row (STAND-IN: the others stop at the edges, not captured).
    pub fn page_slot(&mut self, dx: i32, dy: i32, at: f32) {
        if self.page(at).is_none() {
            return;
        }
        if let Some(p) = self.pick.as_mut() {
            let (col, row) = ((p.slot % 2) as i32 + dx, (p.slot / 2) as i32 + dy);
            let slot = (row.clamp(0, 1) * 2 + col.clamp(0, 1)) as usize;
            if slot != p.slot {
                p.slot_since = Some(at);
            }
            p.slot = slot;
        }
    }

    /// LT or RT on a page: the next or the one before in the party's order (`party`: its size; `field-menu-equip`: RT on 2380 goes
    /// from Ryza to Lent), round the ends (STAND-IN: the wrap is not captured).
    pub fn page_card(&mut self, way: i32, at: f32, party: usize) {
        if self.page(at).is_none() || party == 0 {
            return;
        }
        if let Some(p) = self.pick.as_mut() {
            p.card = (p.card as i32 + way).rem_euclid(party as i32) as usize;
        }
    }

    /// B in the pick: back to the menu (STAND-IN: at once, the capture has no B before the page); B on the page: the page goes
    /// and the menu comes back `PAGE_OUT` frames later.
    pub fn pick_back(&mut self, at: f32) {
        match self.pick.as_mut() {
            Some(p) if at >= p.first && p.confirmed.is_none() => {
                self.pick = None;
                self.chosen = None;
            }
            Some(p) if p.confirmed.is_some_and(|c| at > c) && p.leaving.is_none() => p.leaving = Some(at),
            _ => {}
        }
    }

    /// The menu's coming back from a page, once its frame has come (`upto`: the frame the state is wanted at).
    fn settle(&mut self, upto: f32) {
        if let Some(back) = self.pick.as_ref().and_then(|p| p.leaving).map(|l| l + PAGE_OUT).filter(|&b| b <= upto) {
            self.back_from_page(back);
        }
    }

    /// The menu's coming back from a page at frame `at`: the cards and buttons come in again over the background and the photo,
    /// the cursor where it was, as after the Settings screen.
    fn back_from_page(&mut self, at: f32) {
        self.pick = None;
        self.chosen = None;
        self.shown = at;
        self.warm = true;
        self.on = [None; BUTTONS.len()];
        self.off = [None; BUTTONS.len()];
        self.on[self.cursor] = Some(self.shown + CURSOR_AT);
        self.blur_out = Some(at);
    }
}

/// A key of the menu's script.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    /// Y in the field: the menu opens.
    Open,
    Up,
    Down,
    /// A on the cursor's button.
    Confirm,
    /// B: the menu closes (on the Settings screen: the screen's own B).
    Back,
    /// A key of the Settings screen the menu opened (left, right, the tabs, a held direction's repeat).
    Screen(crate::options_run::Key),
    /// The Settings screen was left (its last frame): the menu comes in again.
    Return,
}

/// Frames from B on the Settings screen to the menu's coming in again (`field-menu`: B at 2400, the cards from 2402).
const SETTINGS_BACK_LAG: f32 = 2.0;

/// Frames from A on the Settings button to the Settings screen's first frame (`field-menu`: A at 2220; the screen's background
/// is at 3/15 of its fade on 2265, so its first frame is 2263).
pub const SETTINGS_AT: i32 = 43;

/// Frames from A on the Settings button to the first of the menu's going out (the buttons, the plates and the cards as on B,
/// the background and the photo stay): the Settings screen's first frame less 2 (`field-menu`: the cards at 0.65 of their
/// opacity on 2265, which is (1 - 4/30)^3, so the going out began on 2261).
const SETTINGS_OUT_AT: f32 = SETTINGS_AT as f32 - BACKDROP_BEFORE;

/// Frames the going out (and the blurred picture) begins before the Settings screen's first frame.
pub const BACKDROP_BEFORE: f32 = 2.0;

/// Frames from Y to the menu's first frame (`field-menu`: Y at 1900, the first frame 1906).
pub const OPEN_LAG: f32 = 6.0;

/// Frames after the menu's last one the field's own clock runs again (`field-menu`: the banner at 2570 stands where it was 4 frames after the field came back, which fits 2568).
pub const FIELD_RESUME_LAG: f32 = 2.0;

/// The frames the menu took over the field, one span (first frame, frame the field comes back
/// to, none while it is open) for each time it opened: the field stands still under it (the area
/// name's banner stops fading and carries on from where it was when the menu closes).
pub fn spans(keys: &[(f32, Key)]) -> Vec<(f32, Option<f32>)> {
    let mut spans: Vec<(f32, Option<f32>)> = Vec::new();
    let mut nav: Option<Nav> = None;
    for &(at, key) in keys {
        if let Some(n) = nav.as_mut() {
            n.settle(at);
        }
        match (key, nav.as_mut()) {
            (Key::Open, None) => {
                nav = Some(Nav::open(at + OPEN_LAG));
                spans.push((at + OPEN_LAG, None));
            }
            (Key::Down, Some(n)) if !n.in_settings && n.pick.is_none() => n.step(1, at),
            (Key::Up, Some(n)) if !n.in_settings && n.pick.is_none() => n.step(-1, at),
            (Key::Confirm, Some(n)) if !n.in_settings && n.pick.is_none() => n.choose(at),
            (Key::Confirm, Some(n)) => n.pick_confirm(at),
            (Key::Return, Some(n)) if n.in_settings => n.back_from_settings(at),
            (Key::Back, Some(n)) if n.in_settings => {}
            (Key::Back, Some(n)) if n.pick.is_some() => n.pick_back(at),
            (Key::Back, Some(_)) => {
                if let Some(last) = spans.last_mut() {
                    last.1 = Some(at + CLOSE_LAG + BG_FADE);
                }
                nav = None;
            }
            _ => {}
        }
    }
    spans
}

/// The menu at frame `tick` for the script's keys (each with the game frame its button went
/// down), or none while the field shows: the keys replayed in order, each at its frame.
pub fn replay(keys: &[(f32, Key)], tick: f32, order: &[usize]) -> Option<Nav> {
    let mut nav: Option<Nav> = None;
    for &(at, key) in keys {
        if nav.as_ref().and_then(|n| n.closed).is_some_and(|c| c + CLOSE_LAG + BG_FADE <= at.min(tick)) {
            nav = None;
        }
        if let Some(n) = nav.as_mut() {
            n.settle(at.min(tick));
        }
        match (key, nav.as_mut()) {
            (Key::Open, None) if at + OPEN_LAG <= tick => nav = Some(Nav::open(at + OPEN_LAG)),
            // On the Settings screen the keys are the screen's (`options_run`); it ends with `Return`.
            (_, Some(n)) if at <= tick && n.closed.is_none() => match key {
                Key::Down if !n.in_settings && n.pick.is_none() => n.step(1, at),
                Key::Up if !n.in_settings && n.pick.is_none() => n.step(-1, at),
                Key::Confirm if !n.in_settings && n.pick.is_none() => n.choose(at),
                Key::Confirm => n.pick_confirm(at),
                Key::Down if n.page(at).is_some() => n.page_slot(0, 1, at),
                Key::Up if n.page(at).is_some() => n.page_slot(0, -1, at),
                Key::Screen(crate::options_run::Key::Left) if n.page(at).is_some() => n.page_slot(-1, 0, at),
                Key::Screen(crate::options_run::Key::Right) if n.page(at).is_some() => n.page_slot(1, 0, at),
                Key::Screen(crate::options_run::Key::TabNext) if n.page(at).is_some() => n.page_card(1, at, order.len()),
                Key::Screen(crate::options_run::Key::TabPrev) if n.page(at).is_some() => n.page_card(-1, at, order.len()),
                Key::Screen(crate::options_run::Key::Left) => n.pick_step(-1, at, order),
                Key::Screen(crate::options_run::Key::Right) => n.pick_step(1, at, order),
                Key::Return if n.in_settings => n.back_from_settings(at),
                Key::Back if n.in_settings => {}
                Key::Back if n.pick.is_some() => n.pick_back(at),
                Key::Back => n.closed = Some(at),
                _ => {}
            },
            _ => {}
        }
    }
    // The page's B takes the menu back `PAGE_OUT` frames later.
    if let Some(n) = nav.as_mut() {
        n.settle(tick);
    }
    nav.filter(|n| n.closed.is_none_or(|c| tick < c + CLOSE_LAG + BG_FADE))
}

/// How the help bar looks: its opacity (outside 0 to 1 it is clamped), the share of its plate's width between the shortest
/// and the full and whether its words are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bar {
    alpha: f32,
    grow: f32,
    words: bool,
    /// Layout pixels added to the plate's width (the menu's bar comes in wider than it stays: `open_plate_extra`).
    extra: f32,
    /// Whose entries the line has.
    entries: Entries,
}

/// The lines the help bar shows: the field's own seven entries, the menu's five and the pick's three.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Entries {
    Field,
    Menu,
    Pick,
    /// The Equipment page's: the pad's cross and "Select", B and "Back", A and "Confirm".
    Page,
    /// The Status page's: the cross and "Select", B and "Back".
    StatusPage,
}

/// The pick's entries: the pad's cross and "Select Character", B and "Back", A and "Confirm".
const PICK_ENTRIES: [(char, u32); 3] = [('\u{4e0}', 4_194_439), ('\u{4d8}', 4_194_368), ('\u{4da}', 4_194_367)];

/// The Equipment page's entries: the cross and "Select" (the table's own words), B and "Back", A and "Confirm".
const PAGE_ENTRIES: [(char, u32); 3] = [('\u{4e2}', 4_194_366), ('\u{4d8}', 4_194_368), ('\u{4da}', 4_194_367)];

/// The Status page's plate is this much wider than its two entries need (layout pixels, the capture's 1544 against 1594) and its
/// width comes from the pick's bar's over this many frames (`field-menu-status` 2330 and 2340: 28 of 223 pixels left after 10 frames,
/// none after 20). STAND-IN: the cause of the extra width is not found (a third, empty entry's glyph and space are 50 pixels).
const STATUS_PLATE_EXTRA: f32 = 50.0 / 1.5;
const STATUS_BAR_FRAMES: f32 = 20.0;

/// The Status page's entries: the cross and "Select", B and "Back" (`field-menu-status` 2400).
const STATUS_ENTRIES: [(char, u32); 2] = [('\u{4e1}', 4_194_366), ('\u{4d8}', 4_194_368)];

/// Frames the bar holds its opacity after the key that sends it out, and the frames its fade takes (`field-menu`: after A
/// on 2220 the opacity is 1 on 2225, 2/3 on 2230 and 1/3 on 2235; after B on 2550 the same on 2555, 2560 and 2565).
const BAR_HOLD: f32 = 5.0;

impl Bar {
    /// The bar `age` frames after the key that sends it out: the words go first, the plate shrinks to its shortest
    /// (`load_view`'s fitted rule, the same on this capture: 0.625 of the way at +5) and the plate fades after `BAR_HOLD`.
    fn out(age: f32) -> Bar {
        Bar { alpha: 1.0 - (age - BAR_HOLD) / HELP_FADE, grow: 1.0 - ((age - PLATE_LEAVE) / PLATE_FRAMES).clamp(0.0, 1.0), words: age < PLATE_LEAVE, extra: 0.0, entries: Entries::Menu }
    }
}

/// The field's bar under the opening menu: it holds until this many frames after the menu's first, then fades out over
/// `HELP_FADE` (`field-menu` 1910 to 1920: opacity 14/15 on 1910, 4/15 on 1920), its whole plate and line as it stood.
const FIELD_BAR_HOLD: f32 = 3.0;

/// The extra width the menu's own bar has as it comes in, in layout pixels, at frames after its first: read off the capture's
/// plate edges (`field-menu` 1940, 1945, 1950: 551.8, 717.1 and 750.1 layout pixels from the left, the last the full plate's).
/// STAND-IN: the game's own tween is not read; this joins the three samples by straight lines.
const OPEN_PLATE_EXTRA: [(f32, f32); 3] = [(1.0, 198.33), (6.0, 33.06), (11.0, 0.0)];

/// The opening plate's extra width `a` frames after the bar's first (`OPEN_PLATE_EXTRA`).
fn open_plate_extra(a: f32) -> f32 {
    let t = &OPEN_PLATE_EXTRA;
    if a <= t[0].0 {
        return t[0].1;
    }
    t.windows(2).find(|w| a < w[1].0).map_or(0.0, |w| w[0].1 + (w[1].1 - w[0].1) * (a - w[0].0) / (w[1].0 - w[0].0))
}

/// Opacity from an animation's alpha curve at `t` seconds, as the game's byte.
fn byte(a: f32) -> f32 {
    (a.round() / 255.0).clamp(0.0, 1.0)
}

struct Pen<'a> {
    v: &'a View,
    k: f32,
    left: f32,
    out: Vec<Quad>,
    /// The quads drawn added to what is under them.
    additive: Vec<usize>,
    /// The quads that are in the draw trace and not drawn (the game draws them through a blurring shader that Barrel does not have for a part of a layer).
    hidden: Vec<usize>,
}

impl Pen<'_> {
    /// The quads, which are added and which are in the trace only. A node at no opacity is not drawn (the additive list names
    /// quads by place).
    fn finish(self) -> (Vec<Quad>, Vec<usize>, Vec<usize>) {
        let Pen { out, additive, hidden, .. } = self;
        let mut places = Vec::with_capacity(out.len());
        let mut kept = 0;
        for q in &out {
            places.push(kept);
            kept += usize::from(q.color[3] > 0.0);
        }
        let additive = additive.into_iter().filter(|&i| out[i].color[3] > 0.0).map(|i| places[i]).collect();
        let hidden = hidden.into_iter().filter(|&i| out[i].color[3] > 0.0).map(|i| places[i]).collect();
        (out.into_iter().filter(|q| q.color[3] > 0.0).collect(), additive, hidden)
    }

    fn place(&self, at: [f32; 2]) -> [f32; 2] {
        [self.left + at[0] * self.k, at[1] * self.k]
    }

    fn uv(&self, texture: usize, p: [f32; 4]) -> [f32; 4] {
        let (aw, ah) = self.v.atlases[&texture];
        [p[0] / aw, p[1] / ah, (p[0] + p[2]) / aw, (p[1] + p[3]) / ah]
    }

    /// A picture `size` layout pixels at `at`.
    fn pic(&mut self, texture: usize, p: [f32; 4], at: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        let [l, t] = self.place(at);
        let uv = self.uv(texture, p);
        self.out.push(Quad { turn: false, spin: None, rect: [l, t, l + size[0] * self.k, t + size[1] * self.k], uv, color, texture });
    }

    /// A picture at its natural size.
    fn natural(&mut self, texture: usize, s: &Style, n: usize, at: [f32; 2], color: [f32; 4]) {
        let p = s.pictures[n];
        self.pic(texture, p, at, [p[2] / PER_LAYOUT, p[3] / PER_LAYOUT], color);
    }

    /// A picture scaled by `scale` about `pivot` (layout pixels within the picture).
    fn scaled(&mut self, texture: usize, p: [f32; 4], at: [f32; 2], size: [f32; 2], pivot: [f32; 2], scale: f32, color: [f32; 4]) {
        let at = [at[0] + pivot[0] * (1.0 - scale), at[1] + pivot[1] * (1.0 - scale)];
        self.pic(texture, p, at, [size[0] * scale, size[1] * scale], color);
    }

    /// A picture turned `degrees` clockwise about its top left.
    fn turned(&mut self, texture: usize, uv: [f32; 4], at: [f32; 2], size: [f32; 2], degrees: f32, color: [f32; 4]) {
        let [l, t] = self.place(at);
        let (w, h) = (size[0] * self.k, size[1] * self.k);
        let (s, c) = degrees.to_radians().sin_cos();
        let corner = |x: f32, y: f32| [l + x * c - y * s, t + x * s + y * c];
        let corners = [corner(0.0, 0.0), corner(w, 0.0), corner(w, h), corner(0.0, h)];
        let (min_x, max_x) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        let (min_y, max_y) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        self.out.push(Quad { turn: false, spin: Some(Spin::Corners(corners)), rect: [min_x, min_y, max_x, max_y], uv, color, texture });
    }

    fn text(&mut self, l: &Label, origin: [f32; 2], text: &str, alpha: f32) {
        let glyphs = &self.v.glyphs[l.font];
        let s = l.size * self.k / ATLAS_SIZE;
        // STAND-IN: The glyphs' spacing (`word_space`) comes out as 0.25 px of 1080 lines for each layout size above 12 times the node's value, fitted to the capture's HP (size 15: 0.75 px) and money (size 16: 1 px) figures; the real thing is the game's own text rule.
        let spacing = l.spacing * 0.25 * (l.size - 12.0).max(0.0) * self.k / 1.5;
        let advance = |g: &Glyph| g.advance as f32 * s + spacing;
        let width: f32 = text.chars().filter_map(|c| glyphs.get(&c)).map(advance).sum();
        let x = self.left + (origin[0] + l.at[0]) * self.k;
        let mut pen = if l.center {
            x - width / 2.0
        } else if l.right {
            x - width
        } else {
            x
        };
        let top = (origin[1] + l.at[1] + TEXT_TOP * l.scale) * self.k;
        let (aw, ah) = (4096.0, 2048.0);
        let color = [l.color[0], l.color[1], l.color[2], alpha];
        for g in text.chars().filter_map(|c| glyphs.get(&c)) {
            let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
            let (x0, y0) = (pen + g.x_offset as f32 * s, top + g.y_offset as f32 * s);
            self.out.push(Quad { turn: false, spin: None, rect: [x0, y0, x0 + gw * s, y0 + gh * s], uv: [gx / aw, gy / ah, (gx + gw) / aw, (gy + gh) / ah], color, texture: if l.font == 0 { FONT01 } else { FONT00 } });
            pen += advance(g);
        }
    }
}

/// How far an `ease`d pair of curves is at `age` frames over `frames`.
fn along(c: &Curve, age: f32, frames: f32) -> [f32; 4] {
    c.at((age / frames).clamp(0.0, 1.0) * c.duration())
}

impl View {
    /// The Equipment page's help bar, whole, on a `screen` of pixels; the plate's quads name this layer's `GUIDE` texture.
    ///
    /// On the Status page (`age` frames since A on the card) the plate comes from the pick's bar's width to its own over
    /// `STATUS_BAR_FRAMES`, out cubic, and is `STATUS_PLATE_EXTRA` wider than its words need.
    pub fn page_help(&self, screen: (u32, u32), status: bool, age: f32) -> Vec<Quad> {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let extra = if status {
            let (pick, own) = (self.plate_full(&self.help_line(&PICK_ENTRIES), k), self.plate_full(&self.help_line(&STATUS_ENTRIES), k) + STATUS_PLATE_EXTRA);
            STATUS_PLATE_EXTRA + (pick - own) * (1.0 - (age / STATUS_BAR_FRAMES).clamp(0.0, 1.0)).powi(3)
        } else {
            0.0
        };
        let mut pen = Pen { v: self, k, left: (screen.0 as f32 - 1280.0 * k) * 0.5, out: Vec::new(), additive: Vec::new(), hidden: Vec::new() };
        self.help_bar(&mut pen, Bar { alpha: 1.0, grow: 1.0, words: true, extra, entries: if status { Entries::StatusPage } else { Entries::Page } });
        pen.finish().0
    }

    /// The menu's quads at game frame `frame` (in draw order) on a `screen` of pixels, which of them are drawn additively and
    /// which are only in the trace.
    pub fn quads(&self, nav: &Nav, shown: &Shown, frame: f32, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<usize>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut pen = Pen { v: self, k, left: (screen.0 as f32 - 1280.0 * k) * 0.5, out: Vec::new(), additive: Vec::new(), hidden: Vec::new() };
        let age = frame - nav.shown;
        let white = |a: f32| [1.0, 1.0, 1.0, a];
        let incubic = |a: f32, frames: f32| (a / frames).clamp(0.0, 1.0).powi(3);
        let outcubic = |a: f32, frames: f32| 1.0 - (1.0 - (a / frames).clamp(0.0, 1.0)).powi(3);
        // Closing: `out` frames since the buttons began to go (the layout's animations run backwards), on B and on A on
        // Settings (which leaves the background and the photo).
        let going = nav.closed.map(|c| c + CLOSE_LAG).or(nav.chosen.filter(|_| nav.in_settings).map(|c| c + SETTINGS_OUT_AT));
        let out = going.map_or(-1.0, |g| frame - g);
        let closing = out >= 0.0;
        let closed = closing && nav.closed.is_some();
        // Frames into the character pick (from its first frame).
        let picked = nav.pick.as_ref().filter(|p| frame >= p.first).map(|p| (p, frame - p.first));
        let bg = if closed { 1.0 - out / BG_FADE } else if nav.warm { 1.0 } else { age / BG_FADE }.clamp(0.0, 1.0);
        pen.out.push(Quad { turn: false, spin: None, rect: [0.0, 0.0, screen.0 as f32 * 2.0, screen.1 as f32 * 2.0], uv: [0.0, 0.0, 2.0, 2.0], color: white(byte(bg * 255.0)), texture: BACKGROUND });

        // The photo, turned, and the mask drawn for its cards' fade.
        let (photo_at, photo_size, photo_turn, photo_style) = &self.photo;
        let photo_at = *photo_at;
        let uv = pen.uv(PHOTO, photo_style.pictures[0]);
        let photo = if closed { incubic(CARD_FADE - out, CARD_FADE) } else if nav.warm { 1.0 } else { incubic(age, PHOTO_FADE) };
        pen.turned(PHOTO, uv, photo_at, *photo_size, *photo_turn, white(byte(photo * 255.0)));
        let (shadow_at, shadow_size, shadow_style) = &self.shadow;
        let shadow = shadow_style.pictures[0];
        pen.pic(MENU2, shadow, *shadow_at, *shadow_size, white(180.0 / 255.0));
        let cards = byte(if closing { incubic(CARD_FADE - out, CARD_FADE) } else { incubic(age, CARD_FADE) } * 255.0);
        // The photo blurs in over the pick (the game's blurring quad over the photo's place; it draws as `sys_white` here).
        let blur = picked.map(|(_, a)| incubic(a, BLUR_FRAMES)).or(nav.blur_out.map(|r| incubic(BLUR_FRAMES - (frame - r), BLUR_FRAMES)));
        if let Some(b) = blur {
            let (at, size, turn) = self.pick.blur;
            // (STAND-IN: not drawn, only in the trace; the photo stays sharp until a layer can blur a part of itself)
            pen.hidden.push(pen.out.len());
            pen.turned(WHITE, [0.0, 0.0, 0.125, 0.125], at, size, turn, white(byte(b * 255.0)));
        }
        let (mask_at, mask_size, mask_turn) = &self.mask;
        // The game draws the mask into a target of its own (the trace's `rt ...#7`), never onto the screen: in the trace only.
        pen.hidden.push(pen.out.len());
        pen.turned(WHITE, [0.0, 0.0, 0.125, 0.125], *mask_at, *mask_size, *mask_turn, white(cards));
        // A page (Equipment, Status) over the menu: only the background, the photo and the corner picture stay under it.
        if nav.page(frame).is_some() {
            let (put_at, put) = &self.put;
            pen.natural(MENU1, put, 0, *put_at, white(1.0));
            return pen.finish();
        }
        // The picked character's figure, sharp over the blurred photo.
        if let Some((p, _)) = picked {
            if let Some(&(at, turn, picture)) = shown.party.get(p.card).and_then(|m| self.pick.figures.get(self.name_of(m.id))) {
                let uv = pen.uv(FIGURES, picture);
                pen.turned(FIGURES, uv, at, [picture[2] / PER_LAYOUT, picture[3] / PER_LAYOUT], turn, white(1.0));
            }
        }

        for (n, ((at, card), member)) in self.cards.iter().zip(&shown.party).enumerate() {
            let origin = *at;
            let c = white(cards);
            let picked_here = picked.filter(|(p, _)| p.card == n);
            // The picked card's loops run from the pick's second frame, or from the frame before the key that moved the pick here
            // (`field-menu-status`: RIGHT on 2200, the ring's opacity 4/255 and the glow's 3/255 on 2200).
            let loops = picked_here.map(|(p, _)| (frame - if p.since == p.first { p.first + PICK_LOOPS_LATER } else { p.since - 1.0 }).max(0.0));
            if let Some(l) = loops {
                self.pick_glow(&mut pen, origin, l);
            }
            pen.natural(MENU0, &card.plate.1, 0, [origin[0] + card.plate.0[0], origin[1] + card.plate.0[1]], c);
            pen.natural(MENU1, &card.icon.1, 0, [origin[0] + card.icon.0[0], origin[1] + card.icon.0[1]], c);
            if let Some(l) = loops {
                // The glow over the plate blinks (`anm_status_blink_loop`).
                let (over_at, pivot, scale) = self.pick.over;
                let l = (l % LOOP_PICK_FRAMES) / 60.0;
                let v = self.pick.blink.at(l);
                pen.additive.push(pen.out.len());
                let p = card.plate.1.pictures[0];
                pen.scaled(MENU0, p, [origin[0] + over_at[0], origin[1] + over_at[1]], [p[2] / PER_LAYOUT, p[3] / PER_LAYOUT], pivot, scale, [byte(v[0]), byte(v[1]), byte(v[2]), byte(v[3])]);
            }
            let hp = member.stats.hp;
            let number = |n: i32| n.to_string();
            pen.text(&card.hp_value, origin, &number(hp), cards);
            pen.text(&card.hp_slash, origin, "/", cards);
            pen.text(&card.hp_max, origin, &number(hp), cards);
            pen.text(&card.name, origin, self.name_of(member.id), cards);
            pen.text(&card.level, origin, &member.level.to_string(), cards);
        }

        // The buttons.
        let (light_at, light) = &self.light;
        // A button's slide at its place `n` in the order (the light is the first, with no delay; going out each waits less).
        let slide_state = |n: f32| -> (f32, f32) {
            let u = if closing { 1.0 - (out - n * CLOSE_STEP) / SLIDE } else { (age - n * SLIDE_STEP) / SLIDE };
            let t = u.clamp(0.0, 1.0) * self.slide[0].duration();
            (self.slide[0].at(t)[3] / 255.0, self.slide[1].at(t)[0])
        };
        let (a, dx) = if closing { slide_state(CLOSE_LIGHT_AT / CLOSE_STEP) } else { slide_state(0.0) };
        pen.natural(MENU1, light, 0, [light_at[0] + dx, light_at[1]], white(byte(a * 255.0)));
        for (i, at) in self.buttons.iter().enumerate() {
            let (a, dx) = slide_state(i as f32);
            let at = [at[0] + dx, at[1]];
            let alpha = byte(a * 255.0);
            // The pick darkens the buttons (`anm_select_blackout`) and draws each over again through the blur.
            let (tint, blur) = match picked {
                Some((_, p)) => {
                    let v = self.pick.blackout.at((p / BLACKOUT_FRAMES).min(1.0) * self.pick.blackout.duration());
                    ([byte(v[0]), byte(v[1]), byte(v[2]), alpha], Some(incubic(p, BLACKOUT_FRAMES)))
                }
                None => (white(alpha), None),
            };
            pen.pic(MENU1, self.base.pictures[0], at, [self.base_size, self.base_size], tint);
            // (the glow goes with the first button's slide; on the way to Settings it goes out by itself over `OFF_FRAMES`:
            // the capture's 142/255 on 2265 is 1 - 4/9)
            let glow_slide = if closed { byte((1.0 - (out - CLOSE_GLOW_AT) / CLOSE_GLOW_FRAMES).clamp(0.0, 1.0) * 255.0) } else if closing { byte((1.0 - out / OFF_FRAMES).clamp(0.0, 1.0) * 255.0) } else { alpha };
            if picked.is_none() {
                self.glow(&mut pen, nav, i, at, glow_slide, frame);
            }
            let image = BUTTONS[i];
            pen.natural(MENU0, &self.names, image, [at[0] + 2.0, at[1] + 103.0], tint);
            pen.natural(MENU1, &self.icons, image, [at[0] + 13.0, at[1] + 14.0], tint);
            if let Some(b) = blur {
                pen.hidden.push(pen.out.len());
                pen.natural(MENU0, &self.names, 0, [at[0] + 2.0, at[1] + 103.0], white(byte(b * 255.0) * alpha));
                pen.hidden.push(pen.out.len());
                pen.pic(MENU1, self.base.pictures[0], at, [self.base_size, self.base_size], white(byte(b * 255.0) * alpha));
            }
        }

        // The level plate and the money, sliding in from the right, and the corner picture.
        // (in: over `frames`; out: at `speed` times the 30 frames' pace)
        // (the pick sends the level plate and the money out the same way, from its first frame)
        let slid = |frames: f32, speed: f32| match (closing, picked) {
            (true, _) => (1.0 - out * speed / SLIDE_PUT).clamp(0.0, 1.0) * frames,
            (_, Some((_, a))) => (1.0 - a * speed / SLIDE_PUT).clamp(0.0, 1.0) * frames,
            _ => age,
        };
        let right = |frames: f32, speed: f32| along(&self.slide_right, slid(frames, speed), frames)[0];
        let (lv_at, lv) = &self.level_plate;
        let a = byte(outcubic(slid(SLIDE_MONEY, CLOSE_MONEY_SPEED), SLIDE_MONEY) * 255.0);
        pen.natural(MENU1, lv, 0, [lv_at[0] + right(SLIDE_MONEY, CLOSE_MONEY_SPEED), lv_at[1]], white(a));
        let (bg_at, bg_size, bg_scale, bg_style) = &self.money_bg;
        pen.pic(MIX, bg_style.pictures[0], [bg_at[0] + right(SLIDE_MONEY, CLOSE_MONEY_SPEED), bg_at[1]], [bg_size[0] * bg_scale, bg_size[1] * bg_scale], white(byte(a * 120.0)));
        let (icon_at, icon_scale, icon_style) = &self.money_icon;
        let p = icon_style.pictures[0];
        pen.pic(ICON, p, [icon_at[0] + right(SLIDE_MONEY, CLOSE_MONEY_SPEED), icon_at[1]], [p[2] / PER_LAYOUT * icon_scale, p[3] / PER_LAYOUT * icon_scale], white(a));
        pen.text(&self.money_text, [right(SLIDE_MONEY, CLOSE_MONEY_SPEED), 0.0], &shown.money.to_string(), a);
        let (put_at, put) = &self.put;
        // (it stays where it is on the way to Settings: the capture draws it whole under the blurred picture)
        if closed || !(closing || nav.warm || picked.is_some()) {
            pen.natural(MENU1, put, 0, [put_at[0] + right(SLIDE_PUT, CLOSE_PUT_SPEED), put_at[1]], white(byte(outcubic(slid(SLIDE_PUT, CLOSE_PUT_SPEED), SLIDE_PUT) * 255.0)));
        } else {
            pen.natural(MENU1, put, 0, *put_at, white(1.0));
        }
        // The bar goes out from the key that closes the menu or opens Settings, not with the buttons.
        let bar = match nav.closed.or(nav.chosen.filter(|_| nav.in_settings || nav.pick.is_some())) {
            // (the pick's own bar comes in 4 frames after its first frame, its plate growing as it fades in)
            Some(_) if picked.is_some_and(|(_, a)| a >= PICK_BAR_LATER) => {
                let a = picked.map_or(0.0, |(_, a)| a) - PICK_BAR_LATER;
                Bar { alpha: a / HELP_FADE, grow: (a / PLATE_FRAMES).clamp(0.0, 1.0), words: true, extra: 0.0, entries: Entries::Pick }
            }
            Some(key) => Bar::out(frame - key),
            None => {
                // (back from Settings it comes up one frame earlier, with its plate a fifth of a frame late, as the screen's own)
                // (and from a page with its plate 0.2 frames early: `field-menu-equip` 2560, opacity 3/15 and the plate 0.50 of the way; STAND-IN, one sample)
                let (at, late) = if nav.warm { (HELP_AT_BACK, if nav.blur_out.is_some() { -BACK_PLATE_LATE } else { BACK_PLATE_LATE }) } else { (HELP_AT, 0.0) };
                if !nav.warm && age < FIELD_BAR_HOLD + HELP_FADE {
                    // The field's bar fading out as the menu comes up.
                    self.help_bar(&mut pen, Bar { alpha: 1.0 - (age - FIELD_BAR_HOLD) / HELP_FADE, grow: 1.0, words: true, extra: 0.0, entries: Entries::Field });
                }
                Bar { alpha: (age - at) / HELP_FADE, grow: if nav.warm { ((age - at - late) / PLATE_FRAMES).clamp(0.0, 1.0) } else { 1.0 }, words: true, extra: if nav.warm { 0.0 } else { open_plate_extra(age - at) }, entries: Entries::Menu }
            }
        };
        self.help_bar(&mut pen, bar);
        pen.finish()
    }

    /// The glow behind a picked card (the layout's `selected_bg_back`): the glow itself, its ring growing and fading in a loop
    /// (`anm_cursor_scale`) and the ring that turns (`anm_cursor_rolling`), `loops` frames into their loops; all drawn added.
    fn pick_glow(&self, pen: &mut Pen, origin: [f32; 2], loops: f32) {
        let [x, y] = origin;
        let (glow_at, glow) = &self.pick.glow;
        pen.additive.push(pen.out.len());
        pen.natural(MENU1, glow, 0, [x + glow_at[0], y + glow_at[1]], [1.0; 4]);
        let (ring_at, pivot, scale) = self.pick.ring;
        let t = (loops % LOOP_PICK_FRAMES) / 60.0;
        let p = self.ring.pictures[0];
        pen.additive.push(pen.out.len());
        pen.scaled(MENU1, p, [x + ring_at[0], y + ring_at[1]], [p[2] / PER_LAYOUT, p[3] / PER_LAYOUT], pivot, self.ring_loop[1].at(t)[0] * scale, [1.0, 1.0, 1.0, byte(self.ring_loop[0].at(t)[3])]);
        // The turning ring: a picture of 160 layout pixels scaled about its middle and turned about it.
        let (roll_at, picture, scale, pivot) = self.pick.roll;
        let degrees = self.pick.rolling.at((loops % ROLL_FRAMES) / 60.0)[2];
        let (w, h) = (picture[2] / PER_LAYOUT * scale * pen.k, picture[3] / PER_LAYOUT * scale * pen.k);
        let middle = pen.place([x + roll_at[0] + pivot[0], y + roll_at[1] + pivot[1]]);
        let (s, c) = degrees.to_radians().sin_cos();
        let corner = |dx: f32, dy: f32| [middle[0] + dx * c - dy * s, middle[1] + dx * s + dy * c];
        let corners = [corner(-w / 2.0, -h / 2.0), corner(w / 2.0, -h / 2.0), corner(w / 2.0, h / 2.0), corner(-w / 2.0, h / 2.0)];
        let (left, right) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q[0]), b.max(q[0])));
        let (top, bottom) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q[1]), b.max(q[1])));
        pen.additive.push(pen.out.len());
        let uv = pen.uv(RING, picture);
        pen.out.push(Quad { turn: false, spin: Some(Spin::Corners(corners)), rect: [left, top, right, bottom], uv, color: [0.0, byte(200.0), 1.0, byte(120.0)], texture: RING });
    }

    /// A button's glow: the cursor's glow and ring while the cursor is on it and as they fade
    /// after it left.
    fn glow(&self, pen: &mut Pen, nav: &Nav, i: usize, at: [f32; 2], slide: f32, frame: f32) {
        let (alpha, loop_age) = match (nav.on[i], nav.off[i]) {
            (Some(on), None) => {
                let age = frame - on;
                if age < 0.0 {
                    return;
                }
                if age < ON_FRAMES {
                    (age / ON_FRAMES, None)
                } else {
                    let l = (age - ON_FRAMES) % LOOP_FRAMES;
                    (self.cursor_loop[0].at(l / 60.0)[3] / 255.0, Some(l))
                }
            }
            (Some(on), Some(off)) => {
                let age = frame - off;
                if age >= OFF_FRAMES || frame < on {
                    return;
                }
                let held = ((off - on) - ON_FRAMES).max(0.0) % LOOP_FRAMES;
                (1.0 - age / OFF_FRAMES, (off - on > ON_FRAMES).then_some(held))
            }
            _ => return,
        };
        let on_alpha = match nav.off[i] {
            Some(off) => 1.0 - (frame - off) / OFF_FRAMES,
            None => ((frame - nav.on[i].unwrap_or(frame)) / ON_FRAMES).min(1.0),
        };
        let mut scale = loop_age.map_or(1.0, |l| self.cursor_loop[1].at(l / 60.0)[0]);
        let mut alpha = alpha;
        if let (Some(chosen), true) = (nav.chosen.filter(|&c| frame >= c), i == nav.cursor) {
            // A pressed on it: the cursor's own pulse (`anm_cursor_select`, speed 1.5) replaces the loop.
            let t = (frame - chosen) * SELECT_SPEED / 60.0;
            alpha = self.select[0].at(t)[3] / 255.0;
            scale = if t < 0.5 { self.select[1].at(t)[0] } else { 1.0 };
        }
        // A part at no opacity is not drawn (the game hides a node a fade has not begun on or has finished).
        let mut part = |pen: &mut Pen, p: [f32; 4], at: [f32; 2], pivot: [f32; 2], scale: f32, color: [f32; 4]| {
            if color[3] > 0.0 {
                pen.additive.push(pen.out.len());
                pen.scaled(MENU1, p, at, [p[2] / PER_LAYOUT, p[3] / PER_LAYOUT], pivot, scale, color);
            }
        };
        part(pen, self.cursor.pictures[0], at, self.pivot, scale, [0.0, 1.0, 1.0, byte(alpha * slide * 255.0)]);
        part(pen, self.cursor_on.pictures[0], at, [0.0; 2], 1.0, [1.0, 1.0, 1.0, byte(on_alpha * slide * 255.0)]);
        // The ring runs from the menu's first frame, whichever button has the glow.
        let t = ((frame - nav.opened + nav.ring_skew) % RING_LOOP_FRAMES) / 60.0;
        let ring_alpha = self.ring_loop[0].at(t)[3] / 255.0 * on_alpha;
        let ring_scale = self.ring_loop[1].at(t)[0] * self.ring_scale;
        let p = self.ring.pictures[0];
        let middle = [p[2] / PER_LAYOUT / 2.0, p[3] / PER_LAYOUT / 2.0];
        part(pen, p, [at[0] + self.ring_at[0], at[1] + self.ring_at[1]], middle, ring_scale, [1.0, 1.0, 1.0, byte(ring_alpha * slide * 255.0)]);
    }

    /// The bar's line: each entry's button glyph and word, one space between.
    fn help_line(&self, entries: &[(char, u32)]) -> String {
        entries.iter().map(|&(glyph, n)| format!("{glyph}{}", self.word(n))).collect::<Vec<_>>().join(" ")
    }

    /// The plate's full width in layout pixels for `line` on a screen `k` times the layout.
    fn plate_full(&self, line: &str, k: f32) -> f32 {
        let s = self.help_text.size * k / ATLAS_SIZE;
        let words: f32 = line.chars().filter_map(|c| self.glyphs[0].get(&c)).map(|g| g.advance as f32 * s).sum::<f32>() / k;
        // STAND-IN: The plate is one atlas pixel of the text wider than the advances' sum, fitted to the capture as the load screen's is (`load_view::help_bar`).
        words + self.help_text.size / ATLAS_SIZE - self.help_text.at[0] + self.help_left
    }

    /// The help bar: the plate and the entries' words as `bar` says.
    fn help_bar(&self, pen: &mut Pen, bar: Bar) {
        let alpha = bar.alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        let entries: &[(char, u32)] = match bar.entries {
            Entries::Field => &crate::field_hud::HELP_ENTRIES,
            Entries::Menu => &HELP_ENTRIES,
            Entries::Pick => &PICK_ENTRIES,
            Entries::Page => &PAGE_ENTRIES,
            Entries::StatusPage => &STATUS_ENTRIES,
        };
        let line = self.help_line(entries);
        let full = self.plate_full(&line, pen.k);
        let width = PLATE_MIN + (full - PLATE_MIN) * bar.grow + bar.extra;
        let [x, y, w, h] = self.help.pictures[0];
        let [ml, mr] = self.help.margins.unwrap_or([0.0; 2]);
        let (cap_l, cap_r, height) = (ml / PER_LAYOUT, mr / PER_LAYOUT, h / PER_LAYOUT);
        let at = [1280.0 - width, self.help_at[1]];
        let color = [1.0, 1.0, 1.0, byte(alpha * 255.0)];
        pen.pic(GUIDE, [x, y, ml, h], at, [cap_l, height], color);
        let tile = (w - ml - mr) / PER_LAYOUT;
        let (mut at_x, mut left) = (at[0] + cap_l, width - cap_l - cap_r);
        while left > 0.001 {
            let part = left.min(tile);
            pen.pic(GUIDE, [x + ml, y, (w - ml - mr) * part / tile, h], [at_x, at[1]], [part, height], color);
            at_x += part;
            left -= part;
        }
        pen.pic(GUIDE, [x + w - mr, y, mr, h], [at_x, at[1]], [cap_r, height], color);
        if bar.words {
            let label = Label { color: [1.0, 1.0, 1.0], ..self.help_text };
            pen.text(&label, [1280.0, 720.0], &line, byte(alpha * 255.0));
        }
    }
}

/// The backdrop layer's textures: `sys_white` (the blurred menu is drawn through its quad) and `a21_common` picture 1 (the two
/// vignettes).
pub const BACKDROP_WHITE: usize = 0;
pub const BACKDROP_VIGNETTE: usize = 1;
pub const BACKDROP_TEXTURES: usize = 2;

/// Frames the blurred picture and its vignettes take to come in, counted from the menu's first going out (`field-menu`:
/// 8/15 of their opacity on 2265, four frames on, and whole on 2270). STAND-IN: one sample on the way, a line is fitted.
const BACKDROP_FRAMES: f32 = 7.5;

/// The vignettes' places on the 1920 x 1080 screen, their pictures' uv and opacity at full (read off the capture's quads; the
/// layout that holds them is not found, the game draws them over the blurred picture of any menu behind its sub-screens).
const VIGNETTES: [([f32; 4], [f32; 4], f32); 2] = [
    ([-9.0, -7.5, 1941.0, 1132.5], [0.3671875, 0.0107421875, 0.6015625, 0.06933594], 0.5882353),
    ([-9.0, 525.0, 1941.0, 1087.5], [0.0625, 0.0107421875, 0.328125, 0.9951172], 0.34509805),
];

/// The blurred picture's opacity `age` frames after the menu began to go out for the Settings screen, or, once the screen
/// was left, `out` frames after its last frame (`field-menu`: whole on 2400, 0.467 on 2405: the same 7.5 frames going out).
pub fn backdrop_fade(age: f32, out: Option<f32>) -> f32 {
    match out {
        Some(out) => 1.0 - out / BACKDROP_FRAMES,
        None => age / BACKDROP_FRAMES,
    }
    .clamp(0.0, 1.0)
}

/// The blurred picture of the menu and the vignettes over it at `fade` opacity, on a `screen` of pixels. The first quad is the picture's (the renderer draws it from the blur of what the layers
/// before it drew); its colour is white with the fade as opacity.
pub fn backdrop(fade: f32, screen: (u32, u32)) -> Vec<Quad> {
    if fade <= 0.0 {
        return Vec::new();
    }
    let (w, h) = (screen.0 as f32, screen.1 as f32);
    let (sx, sy) = (w / 1920.0, h / 1080.0);
    let mut quads = vec![Quad { turn: false, spin: None, rect: [0.0, 0.0, w, h], uv: [0.0, 0.0, 0.125, 0.125], color: [1.0, 1.0, 1.0, byte(fade * 255.0)], texture: BACKDROP_WHITE }];
    for (r, uv, alpha) in VIGNETTES {
        quads.push(Quad { turn: false, spin: None, rect: [r[0] * sx, r[1] * sy, r[2] * sx, r[3] * sy], uv, color: [1.0, 1.0, 1.0, byte(fade * alpha * 255.0)], texture: BACKDROP_VIGNETTE });
    }
    quads
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opening_plate_is_wide_and_settles_by_the_eleventh_frame() {
        assert!((open_plate_extra(1.0) - 198.33).abs() < 1e-3);
        assert!((open_plate_extra(6.0) - 33.06).abs() < 1e-3);
        assert_eq!(open_plate_extra(11.0), 0.0);
        assert_eq!(open_plate_extra(40.0), 0.0);
        assert!(open_plate_extra(3.5) > open_plate_extra(6.0));
    }

    #[test]
    fn the_blurred_picture_is_whole_by_its_eighth_frame_and_the_vignettes_follow_it() {
        assert!(backdrop(0.0, (1920, 1080)).is_empty());
        assert_eq!(backdrop_fade(4.0, None), 4.0 / 7.5);
        assert!((backdrop_fade(0.0, Some(4.0)) - 0.4667).abs() < 1e-3);
        let q = backdrop(backdrop_fade(4.0, None), (1920, 1080));
        assert_eq!(q.len(), 3);
        assert_eq!(q[0].rect, [0.0, 0.0, 1920.0, 1080.0]);
        assert!((q[0].color[3] * 255.0 - 136.0).abs() < 0.01);
        assert!((q[1].color[3] * 255.0 - 80.0).abs() < 0.01);
        assert_eq!(backdrop(backdrop_fade(9.0, None), (1920, 1080))[0].color[3], 1.0);
    }

    #[test]
    fn the_bar_goes_out_as_the_capture_shows_it() {
        // A on 2220, frames 2225, 2230 and 2235
        let b = Bar::out(5.0);
        assert_eq!((b.alpha, b.words), (1.0, false));
        assert!((b.grow - 0.625).abs() < 1e-6);
        let b = Bar::out(10.0);
        assert!((b.alpha - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(b.grow, 0.0);
        assert!((Bar::out(15.0).alpha - 1.0 / 3.0).abs() < 1e-6);
        assert!(Bar::out(1.0).words);
    }

    #[test]
    fn the_cursor_walks_the_buttons_in_order_and_wraps() {
        let mut nav = Nav::open(1906.0);
        assert_eq!(nav.cursor, 0);
        for (i, at) in [2000.0, 2040.0, 2080.0, 2120.0, 2160.0].into_iter().enumerate() {
            nav.step(1, at);
            assert_eq!(nav.cursor, i + 1);
        }
        assert_eq!(nav.cursor, SETTINGS);
        nav.step(1, 2200.0);
        assert_eq!(nav.cursor, 0);
        nav.step(-1, 2210.0);
        assert_eq!(nav.cursor, SETTINGS);
    }

    #[test]
    fn a_move_starts_the_new_glow_and_the_old_ones_fade_at_the_same_frame() {
        let mut nav = Nav::open(1906.0);
        nav.step(1, 2000.0);
        assert_eq!(nav.off[0], Some(2000.0));
        assert_eq!(nav.on[1], Some(2000.0));
        assert_eq!(nav.on[0], Some(1936.0));
    }

    #[test]
    fn the_script_opens_the_menu_six_frames_after_y_and_walks_the_cursor() {
        let keys = [(1900.0, Key::Open), (2000.0, Key::Down), (2040.0, Key::Down)];
        assert!(replay(&keys, 1905.0, &[]).is_none());
        assert_eq!(replay(&keys, 1906.0, &[]).unwrap().opened, 1906.0);
        assert_eq!(replay(&keys, 2010.0, &[]).unwrap().cursor, 1);
        assert_eq!(replay(&keys, 2100.0, &[]).unwrap().cursor, 2);
    }

    #[test]
    fn the_cursor_comes_to_its_first_button_once_the_cards_have_faded_in() {
        assert_eq!(Nav::open(1906.0).on[0], Some(1906.0 + CURSOR_AT));
    }

    const ORDER: [usize; 3] = [1, 0, 2];

    fn pick_keys() -> Vec<(f32, Key)> {
        use crate::options_run::Key as S;
        vec![(1900.0, Key::Open), (2000.0, Key::Down), (2060.0, Key::Confirm), (2200.0, Key::Screen(S::Right)), (2260.0, Key::Screen(S::Left)), (2300.0, Key::Screen(S::Left))]
    }

    #[test]
    fn a_on_equip_starts_a_pick_of_the_leader_42_frames_later() {
        let nav = replay(&pick_keys(), 2100.0, &ORDER).unwrap();
        let pick = nav.pick.unwrap();
        assert_eq!((pick.first, pick.card), (2102.0, 0));
    }

    #[test]
    fn a_on_the_container_button_starts_no_pick() {
        let keys = [(1900.0, Key::Open), (2060.0, Key::Confirm)]; // (the cursor is on the Container)
        assert!(replay(&keys, 2200.0, &ORDER).unwrap().pick.is_none());
    }

    #[test]
    fn left_and_right_move_the_pick_along_the_cards_and_stop_at_the_ends() {
        let keys = pick_keys();
        assert_eq!(replay(&keys, 2210.0, &ORDER).unwrap().pick.unwrap().card, 2);
        assert_eq!(replay(&keys, 2270.0, &ORDER).unwrap().pick.unwrap().card, 0);
        // (the second left: from the leader to the card on its left, the third stays)
        assert_eq!(replay(&keys, 2310.0, &ORDER).unwrap().pick.unwrap().card, 1);
    }

    #[test]
    fn keys_before_the_picks_first_frame_do_nothing() {
        use crate::options_run::Key as S;
        let keys = [(1900.0, Key::Open), (2000.0, Key::Down), (2060.0, Key::Confirm), (2070.0, Key::Screen(S::Right)), (2080.0, Key::Back)];
        let pick = replay(&keys, 2090.0, &ORDER).unwrap().pick.unwrap();
        assert_eq!(pick.card, 0);
    }

    #[test]
    fn b_on_a_page_brings_the_menu_back_24_frames_later() {
        let keys = [(1900.0, Key::Open), (2000.0, Key::Down), (2060.0, Key::Confirm), (2140.0, Key::Confirm), (2500.0, Key::Back)];
        assert!(replay(&keys, 2523.0, &ORDER).unwrap().pick.is_some());
        let nav = replay(&keys, 2524.0, &ORDER).unwrap();
        assert!(nav.pick.is_none() && nav.warm);
        assert_eq!((nav.shown, nav.blur_out), (2524.0, Some(2524.0)));
        // (B on the menu after that closes it)
        let keys = [keys.as_slice(), &[(2580.0, Key::Back)]].concat();
        assert!(replay(&keys, 2590.0, &ORDER).unwrap().closed.is_some());
    }

    #[test]
    fn the_pick_does_not_close_the_menu_on_b() {
        let keys = [(1900.0, Key::Open), (2000.0, Key::Down), (2060.0, Key::Confirm), (2150.0, Key::Back)];
        let nav = replay(&keys, 2160.0, &ORDER).unwrap();
        assert!(nav.closed.is_none() && nav.pick.is_none());
    }

    fn page_keys() -> Vec<(f32, Key)> {
        use crate::options_run::Key as S;
        vec![(1900.0, Key::Open), (2000.0, Key::Down), (2060.0, Key::Confirm), (2140.0, Key::Confirm), (2300.0, Key::Down), (2380.0, Key::Screen(S::TabNext)), (2500.0, Key::Back)]
    }

    #[test]
    fn the_page_is_up_from_ten_frames_after_a_on_the_card_until_b() {
        let keys = page_keys();
        assert!(replay(&keys, 2149.0, &ORDER).unwrap().page(2149.0).is_none());
        assert!(replay(&keys, 2150.0, &ORDER).unwrap().page(2150.0).is_some());
        assert!(replay(&keys, 2501.0, &ORDER).unwrap().page(2501.0).is_none());
    }

    #[test]
    fn down_moves_to_the_third_slot_and_rt_to_the_next_character() {
        let keys = page_keys();
        let nav = replay(&keys, 2290.0, &ORDER).unwrap();
        assert_eq!((nav.pick.as_ref().unwrap().slot, nav.pick.as_ref().unwrap().card), (0, 0));
        let nav = replay(&keys, 2400.0, &ORDER).unwrap();
        assert_eq!((nav.pick.as_ref().unwrap().slot, nav.pick.as_ref().unwrap().card), (2, 1));
    }

    #[test]
    fn rt_goes_round_the_party() {
        use crate::options_run::Key as S;
        let mut keys = page_keys()[..4].to_vec();
        keys.extend([(2200.0, Key::Screen(S::TabNext)), (2210.0, Key::Screen(S::TabPrev)), (2220.0, Key::Screen(S::TabPrev))]);
        let nav = replay(&keys, 2230.0, &ORDER).unwrap();
        assert_eq!(nav.pick.unwrap().card, 2);
    }
}
