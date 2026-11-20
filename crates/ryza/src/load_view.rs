//! What the load screen draws, from the game's own layout: `uil_a21_saveload.xml` (the list of
//! save rows at its left, the info panel at its right, the caption), the styles that place each
//! picture in its atlas (`uis_gen_a21_saveload.xml`, `uis_gen_a21_common.xml`,
//! `uis_gen_a21_caption.xml`), `uia_common.xml` (the fade and the cursor's blink), the help bar
//! (`uil_a21_shade.xml`) and the loading screen (`uil_a21_loading.xml`).
//!
//! Positions are in the 1280 x 720 layout and scale by `screen height / 720`, the layout centred
//! across the width, as the title's are. A style's pixels are the 2160-line atlas's (three to a
//! layout pixel, twice the pixels of the texture).

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;
use crate::saves::Slot;
use crate::title::{curve, find, numbers, pair, Curve, ATLAS_PER_LAYOUT, LAYOUT_HEIGHT, PATCH_VERSION};

/// The textures of the load screen's layer. The notice's two (`boot::SAVELOAD_LETTERS`,
/// `boot::SAVELOAD_PICTURES`) sit where the boot's layer has them.
pub const FONT01: usize = 0;
pub const FONT00: usize = 1;
pub const ROWS: usize = 4;
pub const COMMON: usize = 5;
pub const GUIDE: usize = 6;
pub const BACKGROUND: usize = 7;
pub const CAPTION: usize = 8;
pub const LOADING_FRAMES: usize = 9;
pub const LOADING_TEXT: usize = 10;
/// How many textures the layer has.
pub const TEXTURES: usize = 11;

// STAND-IN: The text line's offset (TEXT_TOP) is the title's, fitted to the capture; the real thing is the game's own text layout rule.
/// How far above a text node's `pos.y` its line top sits, in layout pixels (`title`'s fit; the
/// load screen's lines match it).
const TEXT_TOP: f32 = -4.0;
/// The text colour of the template `TEXTCOLOR_BROWN` (26, 0, 0).
const BROWN: [f32; 3] = [26.0 / 255.0, 0.0, 0.0];
/// Frames the common fade takes (`anm_common_fade_in`).
const FADE_FRAMES: f32 = 15.0;

// STAND-IN: The help bar's button glyphs are the capture's (the game picks the pictures for the pad's assignment); the words come from the string table.
/// The help bar's entries: the button glyph and the words' string (`str_menu_define`: Select,
/// Submenu, Back, Confirm).
const GUIDE_ENTRIES: [(char, u32); 4] = [('\u{4e2}', 4_194_366), ('\u{4de}', 4_194_317), ('\u{4d8}', 4_194_368), ('\u{4da}', 4_194_367)];
/// The caption's word: the save/load strings' "Load" (`str_saveload_define`; the layout's own
/// `STR_UI_0028` is empty in the tables, the game sets the caption's text in code).
const CAPTION_STRING: u32 = 4_456_470;
/// Every string number the screen reads besides the layout's own and the party's names.
const WORDS: [u32; 5] = [CAPTION_STRING, GUIDE_ENTRIES[0].1, GUIDE_ENTRIES[1].1, GUIDE_ENTRIES[2].1, GUIDE_ENTRIES[3].1];

/// A picture, or several, of a style: where each is in the atlas, and for a stretched ("wrap")
/// picture its left and right margins.
#[derive(Clone, Debug, PartialEq)]
struct Style {
    pictures: Vec<[f32; 4]>,
    margins: Option<[f32; 2]>,
}

fn style(styles: &Element, name: &str) -> Result<Style, String> {
    let image = |e: &Element| -> Option<[f32; 4]> { numbers(e, "uvwh").try_into().ok() };
    let found = styles.children.iter().find(|e| e.attr("name") == Some(name)).ok_or(format!("no style {name}"))?;
    let pictures = match found.name.as_str() {
        "image_list" => found.children.iter().filter_map(image).collect(),
        _ => image(found).into_iter().collect(),
    };
    let margins = (found.attr("prim_type") == Some("wrap")).then(|| {
        let m = numbers(found, "edge_margin");
        [m.first().copied().unwrap_or(0.0), m.get(2).copied().unwrap_or(0.0)]
    });
    Ok(Style { pictures, margins })
}

/// A text node: its place within its parent, size, font and alignment.
#[derive(Clone, Debug, PartialEq)]
struct Label {
    at: [f32; 2],
    size: f32,
    /// `MainFont01` unless the node names no font (the template's `MainFont00`).
    small: bool,
    align: Align,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Align {
    Left,
    Center,
    Right,
}

/// The first node called `name` below `e` (not `e` itself) with its place relative to `e`'s
/// origin, `base` added.
fn inside<'a>(e: &'a Element, name: &str, base: [f32; 2]) -> Option<(&'a Element, [f32; 2])> {
    e.children.iter().find_map(|c| find(c, name, base))
}

fn label(layout: &Element, name: &str, within: &str) -> Result<Label, String> {
    let (parent, _) = find(layout, within, [0.0; 2]).ok_or(format!("uil_a21_saveload: no {within}"))?;
    let (e, at) = inside(parent, name, [0.0; 2]).ok_or(format!("uil_a21_saveload: no {within}/{name}"))?;
    Ok(Label {
        at,
        size: e.attr("font_size").and_then(|v| v.trim().parse().ok()).unwrap_or(17.0),
        small: e.attr("font") == Some("MainFont01"),
        align: match e.attr("align") {
            Some("center") => Align::Center,
            Some("right") => Align::Right,
            _ => Align::Left,
        },
    })
}

/// One line of the info panel: its words' nodes and the rule under it.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    /// The node's own place in the panel.
    at: [f32; 2],
    title: Label,
    value: Label,
    /// The rule's place within the node and its width.
    rule: [f32; 2],
    rule_width: f32,
}

/// A text of the string table (`STR_UI_nnnn` is the string numbered `STR_UI_BASE + nnnn`).
const STR_UI_BASE: u32 = 6_619_137;
/// The first character name (Ryza); a character's id counts up from it.
const CHARACTER_NAMES: u32 = 262_148;
/// Names beyond this many are not looked up.
const CHARACTERS: u32 = 40;

/// What the screen reads of the game's files.
pub struct Sources<'a> {
    pub layout: &'a [u8],
    pub styles: &'a [u8],
    pub common_styles: &'a [u8],
    pub caption_styles: &'a [u8],
    pub help: &'a [u8],
    pub anims: &'a [u8],
    pub strings: &'a [u8],
    pub glyphs: [HashMap<char, Glyph>; 2],
    /// The atlases in style pixels: rows, common (the lines and the name plate), help bar, caption.
    pub atlases: [(u32, u32); 4],
    /// The loading screen: its layout, styles and animations, and its two atlases in style pixels (frames, text).
    pub loading_layout: &'a [u8],
    pub loading_styles: &'a [u8],
    pub loading_anims: &'a [u8],
    pub loading_atlases: [(u32, u32); 2],
}

/// The loading screen's turning picture and its two frames.
struct Loading {
    /// The node's place and, for each of the frame, the inside frame and the text: scale and the
    /// picture's corner within the node.
    at: [f32; 2],
    parts: [([f32; 4], f32, [f32; 2]); 3],
    roll: Curve,
}

pub struct View {
    glyphs: [HashMap<char, Glyph>; 2],
    atlases: [(f32, f32); 4],
    /// The row frame, the cursor's picture and the icon (the layout's `data` row).
    frame: [f32; 4],
    cursor: [f32; 4],
    icon: [f32; 4],
    icon_at: [f32; 2],
    row_line: Style,
    list_at: [f32; 2],
    row_step: f32,
    row_line_at: [f32; 2],
    row_line_width: f32,
    row_line_gap: f32,
    name: Label,
    date: Label,
    line: Style,
    info_at: [f32; 2],
    plate: Style,
    plate_at: [f32; 2],
    heading: Label,
    play: Line,
    level: Line,
    money: Line,
    coins: Line,
    party_at: [f32; 2],
    party_step: f32,
    party_name: Label,
    party_lv_pre: Label,
    party_lv: Label,
    party_rule: [f32; 2],
    party_rule_width: f32,
    version: Line,
    caption: Style,
    caption_at: [f32; 2],
    caption_text: Label,
    help: Style,
    help_at: [f32; 2],
    help_text: Label,
    help_left: f32,
    blink: Curve,
    strings: HashMap<u32, String>,
    loading: Loading,
    loading_atlases: [(f32, f32); 2],
}

fn line(layout: &Element, node: &str) -> Result<Line, String> {
    let (n, at) = find(layout, node, [0.0; 2]).ok_or(format!("uil_a21_saveload: no {node}"))?;
    let rule = n.children.iter().find(|c| c.name == "image").ok_or(format!("uil_a21_saveload: {node} has no rule"))?;
    Ok(Line {
        at,
        title: label(n, "title", node)?,
        value: label(n, "text", node)?,
        rule: pair(rule, "pos", [0.0; 2]),
        rule_width: numbers(rule, "size_w").first().copied().unwrap_or(0.0),
    })
}

impl View {
    pub fn read(src: Sources) -> Result<View, String> {
        let layout = table::parse(src.layout).map_err(|e| format!("uil_a21_saveload: {e}"))?;
        let styles = table::parse(src.styles).map_err(|e| format!("uis_gen_a21_saveload: {e}"))?;
        let common = table::parse(src.common_styles).map_err(|e| format!("uis_gen_a21_common: {e}"))?;
        let caption = table::parse(src.caption_styles).map_err(|e| format!("uis_gen_a21_caption: {e}"))?;
        let help_layout = table::parse(src.help).map_err(|e| format!("uil_a21_shade: {e}"))?;
        let strings_root = table::parse(src.strings).map_err(|e| format!("strcombineall: {e}"))?;
        let strings = strings_root
            .children
            .iter()
            .filter_map(|s| Some((s.attr("String_No")?.parse::<u32>().ok()?, s.attr("Text")?.to_string())))
            .filter(|(n, _)| (STR_UI_BASE..STR_UI_BASE + 1000).contains(n) || WORDS.contains(&n.to_owned()) || (CHARACTER_NAMES..CHARACTER_NAMES + CHARACTERS).contains(n))
            .collect();
        let first = |s: &Style| s.pictures.first().copied().ok_or("a style without a picture".to_string());

        // The row template and the list.
        let (data, _) = find(&layout, "data", [0.0; 2]).ok_or("uil_a21_saveload: no data")?;
        let picture = |name: &str| -> Result<(Style, [f32; 2]), String> {
            let (e, at) = find(data, name, [0.0; 2]).ok_or(format!("uil_a21_saveload: no data/{name}"))?;
            Ok((style(&styles, e.attr("style").ok_or(format!("{name}: no style"))?)?, at))
        };
        let (frame, _) = picture("bg")?;
        let (cursor, _) = picture("__uimenu_cursor_active")?;
        let (icon, icon_at) = picture("icon_normal")?;
        let (_, list_at) = find(&layout, "list", [0.0; 2]).ok_or("uil_a21_saveload: no list")?;
        let (_, second) = find(&layout, "data_1", [0.0; 2]).ok_or("uil_a21_saveload: no data_1")?;
        let (two, _) = find(data, "2lines", [0.0; 2]).ok_or("uil_a21_saveload: no 2lines")?;
        let (rule, rule_at) = find(two, "line", [0.0; 2]).ok_or("uil_a21_saveload: no 2lines/line")?;
        let (_, rule_gap) = find(rule, "line_1", [0.0; 2]).ok_or("uil_a21_saveload: no line_1")?;
        let info_at = find(&layout, "info", [0.0; 2]).ok_or("uil_a21_saveload: no info")?.1;
        let (info, _) = find(&layout, "info", [0.0; 2]).ok_or("uil_a21_saveload: no info")?;
        let (plate_node, plate_at) = inside(info, "image", info_at).ok_or("uil_a21_saveload: no info/image")?;
        let (_, party_at) = find(&layout, "party", [0.0; 2]).ok_or("uil_a21_saveload: no party")?;
        let (_, second_chara) = find(&layout, "chara_1", [0.0; 2]).ok_or("uil_a21_saveload: no chara_1")?;
        let (_, first_chara) = find(&layout, "chara_0", [0.0; 2]).ok_or("uil_a21_saveload: no chara_0")?;
        let (chara, _) = find(&layout, "chara_0", [0.0; 2]).ok_or("uil_a21_saveload: no chara_0")?;
        let party_rule = chara.children.iter().find(|c| c.name == "image").ok_or("uil_a21_saveload: chara_0 has no rule")?;
        let (caption_node, caption_at) = find(&layout, "caption_main", [0.0; 2]).ok_or("uil_a21_saveload: no caption_main")?;
        let (help_bg, help_at) = find(&help_layout, "base", [0.0; 2]).ok_or("uil_a21_shade: no base")?;
        let (_, left_side) = inside(help_bg, "left_side", [0.0; 2]).ok_or("uil_a21_shade: no left_side")?;
        let help_label = label(&help_layout, "text", "bg")?;

        Ok(View {
            glyphs: src.glyphs,
            atlases: src.atlases.map(|(w, h)| (w as f32, h as f32)),
            frame: first(&frame)?,
            cursor: *style_picture(&cursor, 1)?,
            icon: first(&icon)?,
            icon_at,
            row_line: style(&common, rule.attr("style").ok_or("2lines/line: no style")?)?,
            list_at,
            row_step: second[1] - list_at[1],
            row_line_at: [rule_at[0], rule_at[1]],
            row_line_width: numbers(rule, "size_w").first().copied().unwrap_or(0.0),
            row_line_gap: rule_gap[1] - rule_at[1],
            name: label(data, "file", "data")?,
            date: label(data, "update_date", "data")?,
            line: style(&common, "gen_a21_common02_line")?,
            info_at,
            plate: style(&common, plate_node.attr("style").ok_or("info/image: no style")?)?,
            plate_at,
            heading: label(&layout, "text_title", "image")?,
            play: line(&layout, "play_time")?,
            level: line(&layout, "mix_lv")?,
            money: line(&layout, "money")?,
            coins: line(&layout, "goldcoin")?,
            party_at,
            party_step: second_chara[1] - first_chara[1],
            party_name: label(chara, "name", "chara_0")?,
            party_lv_pre: label(chara, "lv_pre", "chara_0")?,
            party_lv: label(chara, "lv", "chara_0")?,
            party_rule: pair(party_rule, "pos", [0.0; 2]),
            party_rule_width: numbers(party_rule, "size_w").first().copied().unwrap_or(0.0),
            version: line(&layout, "version")?,
            caption: style(&caption, caption_node.attr("style").ok_or("caption_main: no style")?)?,
            caption_at,
            caption_text: label(caption_node, "main", "caption_main")?,
            help: style(&common, help_bg.attr("style").ok_or("uil_a21_shade: base has no style")?)?,
            help_at,
            help_text: help_label,
            help_left: left_side[0],
            blink: curve(src.anims, "anm_common_blink", "color")?,
            strings,
            loading: read_loading(src.loading_layout, src.loading_styles, src.loading_anims)?,
            loading_atlases: src.loading_atlases.map(|(w, h)| (w as f32, h as f32)),
        })
    }
}

fn read_loading(layout: &[u8], styles: &[u8], anims: &[u8]) -> Result<Loading, String> {
    let layout = table::parse(layout).map_err(|e| format!("uil_a21_loading: {e}"))?;
    let styles = table::parse(styles).map_err(|e| format!("uis_gen_a21_loading: {e}"))?;
    let (_, at) = find(&layout, "pos", [0.0; 2]).ok_or("uil_a21_loading: no pos")?;
    let part = |node: &str| -> Result<([f32; 4], f32, [f32; 2]), String> {
        let (n, _) = find(&layout, node, [0.0; 2]).ok_or(format!("uil_a21_loading: no {node}"))?;
        let (image, corner) = inside(n, if node == "text" { "rolling_text" } else { "image" }, [0.0; 2]).ok_or(format!("uil_a21_loading: {node} has no picture"))?;
        let picture = style(&styles, image.attr("style").ok_or("uil_a21_loading: picture without a style")?)?;
        Ok((*style_picture(&picture, 0)?, pair(n, "scale", [1.0; 2])[0], corner))
    };
    Ok(Loading {
        at,
        parts: [part("frame_outside")?, part("frame_inside")?, part("text")?],
        roll: curve(anims, "anim_rolling_text", "rot")?,
    })
}

fn style_picture(s: &Style, n: usize) -> Result<&[f32; 4], String> {
    s.pictures.get(n).ok_or_else(|| format!("a style has no picture {n}"))
}

/// Where a frame of the screen is: how far into its fade-in and fade-out, and the blink's phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    /// Frames since the screen started to fade in.
    pub age: i32,
    /// Frames since a save was chosen (the screen fades out), if one was.
    pub leaving: Option<i32>,
    /// The save the cursor is on.
    pub on: usize,
}

impl Moment {
    /// The screen's opacity (`anm_common_fade_in`, forward and then back).
    pub fn opacity(&self) -> f32 {
        let fade_in = (self.age as f32 / FADE_FRAMES).clamp(0.0, 1.0);
        let fade_out = self.leaving.map_or(1.0, |l| 1.0 - (l as f32 / FADE_FRAMES).clamp(0.0, 1.0));
        fade_in * fade_out
    }
}

struct Pen<'a> {
    view: &'a View,
    k: f32,
    left: f32,
    out: Vec<Quad>,
}

impl Pen<'_> {
    fn pic(&mut self, texture: usize, atlas: usize, uvwh: [f32; 4], at: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        let (aw, ah) = self.view.atlases[atlas];
        let [x, y, w, h] = uvwh;
        let (l, t) = (self.left + at[0] * self.k, at[1] * self.k);
        self.out.push(Quad { turn: false, spin: None, rect: [l, t, l + size[0] * self.k, t + size[1] * self.k], uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color, texture });
    }

    /// A picture of `s` at its natural size.
    fn natural(&mut self, texture: usize, atlas: usize, s: &Style, n: usize, at: [f32; 2], color: [f32; 4]) {
        let p = s.pictures[n];
        self.pic(texture, atlas, p, at, [p[2] / ATLAS_PER_LAYOUT, p[3] / ATLAS_PER_LAYOUT], color);
    }

    /// A stretched ("wrap") picture `width` layout pixels wide: the left margin, the middle's
    /// tiles (the last cut short) and the right margin.
    fn wrap(&mut self, texture: usize, atlas: usize, s: &Style, at: [f32; 2], width: f32, color: [f32; 4]) {
        let [x, y, w, h] = s.pictures[0];
        let [ml, mr] = s.margins.unwrap_or([0.0; 2]);
        let height = h / ATLAS_PER_LAYOUT;
        let (cap_l, cap_r) = (ml / ATLAS_PER_LAYOUT, mr / ATLAS_PER_LAYOUT);
        let (mid_src, tile) = (w - ml - mr, (w - ml - mr) / ATLAS_PER_LAYOUT);
        self.pic(texture, atlas, [x, y, ml, h], at, [cap_l, height], color);
        let mut at_x = at[0] + cap_l;
        let mut left = width - cap_l - cap_r;
        while left > 0.001 {
            let part = left.min(tile);
            self.pic(texture, atlas, [x + ml, y, mid_src * part / tile, h], [at_x, at[1]], [part, height], color);
            at_x += part;
            left -= part;
        }
        self.pic(texture, atlas, [x + w - mr, y, mr, h], [at_x, at[1]], [cap_r, height], color);
    }

    /// `text` set in a label's font and size with the label's node at `origin`.
    fn text(&mut self, l: &Label, origin: [f32; 2], text: &str, color: [f32; 4]) {
        let font = usize::from(!l.small);
        let glyphs = &self.view.glyphs[font];
        let s = l.size * self.k / ATLAS_SIZE;
        let width: f32 = text.chars().filter_map(|c| glyphs.get(&c)).map(|g| g.advance as f32 * s).sum();
        let x = self.left + (origin[0] + l.at[0]) * self.k;
        let mut pen = match l.align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let top = (origin[1] + l.at[1] + TEXT_TOP) * self.k;
        let (aw, ah) = (4096.0, 2048.0);
        for g in text.chars().filter_map(|c| glyphs.get(&c)) {
            let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
            let (l, t) = (pen + g.x_offset as f32 * s, top + g.y_offset as f32 * s);
            self.out.push(Quad { turn: false, spin: None, rect: [l, t, l + gw * s, t + gh * s], uv: [gx / aw, gy / ah, (gx + gw) / aw, (gy + gh) / ah], color, texture: if font == 0 { FONT01 } else { FONT00 } });
            pen += g.advance as f32 * s;
        }
    }
}

/// The help plate's shortest width (376.5 px of the capture at 1.5 px a layout pixel), the frames
/// its grow and shrink take, and the frames after the choice the shrink starts at (fitted).
const PLATE_MIN: f32 = 376.5 / 1.5;
const PLATE_FRAMES: f32 = 6.4;
const PLATE_LEAVE: f32 = 2.6;

impl View {
    /// The help bar's line: each entry's button glyph, then its words.
    fn guide_text(&self) -> String {
        let word = |n: u32| self.strings.get(&n).map_or("", String::as_str);
        GUIDE_ENTRIES.iter().map(|&(glyph, n)| format!("{glyph}{}", word(n))).collect::<Vec<_>>().join(" ")
    }

    fn string(&self, n: u32) -> &str {
        self.strings.get(&(STR_UI_BASE + n)).map_or("", String::as_str)
    }

    /// The load screen's quads for `slots` at `moment`, in draw order, on a `screen` of pixels;
    /// the `notice`'s quads (the game draws them over the rows and the caption, under the help
    /// bar) are placed where the game draws them.
    pub fn quads(&self, slots: &[Slot], moment: Moment, notice: Vec<Quad>, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        let mut pen = Pen { view: self, k, left, out: Vec::new() };
        let top = moment.opacity();
        if top <= 0.0 {
            // Only the help bar is left of a screen that has faded out.
            pen.out.extend(notice);
            self.help(&mut pen, &moment);
            return pen.out;
        }
        let white = |a: f32| [1.0, 1.0, 1.0, a];
        let brown = |a: f32| [BROWN[0], BROWN[1], BROWN[2], a];

        // The background: the screen's picture tiled twice over (the node is 3840 x 2160 style pixels drawn at twice their layout size).
        pen.out.push(Quad { turn: false, spin: None, rect: [0.0, 0.0, screen.0 as f32 * 2.0, screen.1 as f32 * 2.0], uv: [0.0, 0.0, 2.0, 2.0], color: white(top), texture: BACKGROUND });

        // The info panel (it fades in on its own as well, so its opacity is the product).
        // (its own fade runs in with the screen's and is done by the time the screen goes out)
        let info = top * (moment.age as f32 / FADE_FRAMES).clamp(0.0, 1.0);
        pen.natural(COMMON, 1, &self.plate, 0, self.plate_at, white(info));
        pen.text(&self.heading, self.plate_at, self.string(375), brown(info));
        let slot = slots.get(moment.on);
        let h = slot.map(|s| &s.header);
        let rows: [(&Line, String); 4] = [
            (&self.play, h.map_or(String::new(), |h| h.play_time())),
            (&self.level, h.map_or(String::new(), |h| h.alchemy_level.to_string())),
            (&self.money, h.map_or(String::new(), |h| h.money.to_string())),
            (&self.coins, h.map_or(String::new(), |h| h.gold_coins.to_string())),
        ];
        let titles = [831, 377, 378, 364];
        for ((l, value), t) in rows.iter().zip(titles) {
            self.info_line(&mut pen, l, l.at, self.string(t), value, info);
        }
        if let Some(h) = h {
            for (i, &(id, level)) in h.party.iter().enumerate().take(9) {
                let origin = [self.party_at[0], self.party_at[1] + self.party_step * i as f32];
                let name = self.strings.get(&(CHARACTER_NAMES + u32::from(id))).map_or("", String::as_str);
                pen.text(&self.party_name, origin, name, brown(info));
                pen.text(&self.party_lv_pre, origin, self.string(91), brown(info));
                pen.text(&self.party_lv, origin, &level.to_string(), brown(info));
                pen.wrap(COMMON, 1, &self.line, [origin[0] + self.party_rule[0], origin[1] + self.party_rule[1]], self.party_rule_width, white(info));
            }
        }
        self.info_line(&mut pen, &self.version, self.version.at, self.string(832), PATCH_VERSION, info);

        // The rows.
        for (i, slot) in slots.iter().enumerate().take(5) {
            let origin = [self.list_at[0], self.list_at[1] + self.row_step * i as f32];
            pen.pic(ROWS, 0, self.frame, origin, [self.frame[2] / ATLAS_PER_LAYOUT, self.frame[3] / ATLAS_PER_LAYOUT], white(top));
            for n in 0..2 {
                let at = [origin[0] + self.row_line_at[0], origin[1] + self.row_line_at[1] + self.row_line_gap * n as f32];
                pen.wrap(COMMON, 1, &self.row_line, at, self.row_line_width, white(top));
            }
            if i == moment.on {
                let blink = self.blink.at(moment.age as f32 / 60.0)[3] / 255.0;
                pen.pic(ROWS, 0, self.cursor, origin, [self.cursor[2] / ATLAS_PER_LAYOUT, self.cursor[3] / ATLAS_PER_LAYOUT], white(top * 120.0 / 255.0 * blink));
            }
            pen.pic(ROWS, 0, self.icon, [origin[0] + self.icon_at[0], origin[1] + self.icon_at[1]], [self.icon[2] / ATLAS_PER_LAYOUT, self.icon[3] / ATLAS_PER_LAYOUT], white(top));
            pen.text(&self.name, origin, &slot.name, brown(top));
            pen.text(&self.date, origin, &slot.header.saved_at(), brown(top));
        }

        // The caption and the help bar.
        pen.natural(CAPTION, 3, &self.caption, 0, self.caption_at, white(top));
        pen.text(&self.caption_text, self.caption_at, self.strings.get(&CAPTION_STRING).map_or("", String::as_str), [BROWN[0], BROWN[1], BROWN[2], top]);
        pen.out.extend(notice);
        self.help(&mut pen, &moment);
        pen.out
    }

    /// The help bar for `moment`: it outlasts the screen's own fade-out by 5 frames.
    fn help(&self, pen: &mut Pen, moment: &Moment) {
        // The help bar fades in 20 frames after the screen's start and out from 5 frames after the choice (read off the capture's 1580-1615).
        let guide = ((moment.age - 20) as f32 / FADE_FRAMES).clamp(0.0, 1.0) * moment.leaving.map_or(1.0, |l| 1.0 - ((l - 5) as f32 / FADE_FRAMES).clamp(0.0, 1.0));
        // STAND-IN: The plate's width steps (the capture: 533 px at 1580, full 627 from 1585, 533 at 1605, 376 at 1610-1615) are fitted as a linear grow over PLATE_FRAMES from the bar's start and, after the choice, a shrink from PLATE_LEAVE that also takes the words away; the real thing is the layout's own tween (not found in uia_*.xml).
        let grow = ((moment.age - 20) as f32 / PLATE_FRAMES).clamp(0.0, 1.0) * moment.leaving.map_or(1.0, |l| 1.0 - ((l as f32 - PLATE_LEAVE) / PLATE_FRAMES).clamp(0.0, 1.0));
        let words = moment.leaving.is_none_or(|l| (l as f32) < PLATE_LEAVE);
        if guide > 0.0 {
            self.help_bar(pen, guide, grow, words);
        }
    }

    /// The loading screen `age` frames after it came up: the two frames and, turning
    /// about its middle, the text, at the lower right.
    // STAND-IN: The loading screen's glows and light ring (drawn additively in the game, which the layer cannot) and the inside frame's swing about the vertical axis are left out; the capture's one frame (1725) shows neither.
    pub fn loading(&self, age: i32, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
        let l = &self.loading;
        let mut out = Vec::new();
        for (n, (uvwh, scale, corner)) in l.parts.iter().enumerate() {
            let (texture, (aw, ah)) = if n == 2 { (LOADING_TEXT, self.loading_atlases[1]) } else { (LOADING_FRAMES, self.loading_atlases[0]) };
            let size = [uvwh[2] / ATLAS_PER_LAYOUT * scale, uvwh[3] / ATLAS_PER_LAYOUT * scale];
            let (x, y) = (left + (l.at[0] + corner[0] * scale) * k, (l.at[1] + corner[1] * scale) * k);
            let [ux, uy, uw, uh] = *uvwh;
            let rect = [x, y, x + size[0] * k, y + size[1] * k];
            let spin = (n == 2).then(|| Spin::About { pivot: [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0], angle: l.roll.at(age as f32 / 60.0)[2].to_radians() });
            out.push(Quad { turn: false, spin, rect, uv: [ux / aw, uy / ah, (ux + uw) / aw, (uy + uh) / ah], color: [1.0; 4], texture });
        }
        out
    }

    fn info_line(&self, pen: &mut Pen, l: &Line, node: [f32; 2], title: &str, value: &str, alpha: f32) {
        let brown = [BROWN[0], BROWN[1], BROWN[2], alpha];
        pen.text(&l.title, node, title, brown);
        pen.text(&l.value, node, value, brown);
        pen.wrap(COMMON, 1, &self.line, [node[0] + l.rule[0], node[1] + l.rule[1]], l.rule_width, [1.0, 1.0, 1.0, alpha]);
    }

    /// The help bar at the screen's lower right: a stretched plate as wide as its words.
    fn help_bar(&self, pen: &mut Pen, alpha: f32, grow: f32, words_shown: bool) {
        let s = self.help_text.size * pen.k / ATLAS_SIZE;
        let guide = self.guide_text();
        let words: f32 = guide.chars().filter_map(|c| self.glyphs[0].get(&c)).map(|g| g.advance as f32 * s).sum::<f32>() / pen.k;
        let right = 1280.0;
        // STAND-IN: The plate is one atlas pixel of the text (size / ATLAS_SIZE layout pixels) wider than the advances' sum, fitted to the capture (0.444 layout pixels at size 16); the real thing is the game's own text-width measure.
        let full = words + self.help_text.size / ATLAS_SIZE - self.help_text.at[0] + self.help_left;
        let width = PLATE_MIN + (full - PLATE_MIN) * grow;
        let at = [right - width, self.help_at[1]];
        pen.wrap(GUIDE, 2, &self.help, at, width, [1.0, 1.0, 1.0, alpha]);
        if words_shown {
            pen.text(&self.help_text, [1280.0, 720.0], &guide, [1.0, 1.0, 1.0, alpha]);
        }
    }
}
