//! What the Settings screen draws, from the game's own layouts: the quick menu's pages
//! (`quick_menu/uil_quick_menu_settings.xml`: the list's window, the cursor, the rows and the
//! description panel; `uil_quick_menu_tab.xml`: the tabs), the styles that place each picture in
//! its atlas (`uis_gen_a21v2_common/various`, `uis_gen_a21_common/option/caption`), the help bar
//! (`uil_a21_shade.xml`) and the caption of the save screens' layout.
//!
//! The game builds the list twice, a graphics list and the plain one, and draws both, the
//! graphics one first and still with the rows' template words; the plain one hides it. Both are
//! drawn here, for the same trace.
//!
//! Positions are in the 1280 x 720 layout and scale by `screen height / 720`. A style's pixels
//! are the 2160-line atlas's (three to a layout pixel, twice the pixels of the texture).

use std::collections::HashMap;

use barrel_formats::table::{self, Element};
use barrel_render::text::{Glyph, Quad};

use crate::font::ATLAS_SIZE;
use crate::load_view::{Align, Label, Style, inside, label, style, TEXT_TOP};
use crate::title::{curve, find, numbers, pair, Curve, ATLAS_PER_LAYOUT, LAYOUT_HEIGHT};

/// The textures of the screen's layer.
pub const FONT01: usize = 0;
pub const FONT00: usize = 1;
pub const OPTION: usize = 2;
pub const V2_COMMON: usize = 3;
pub const V2_VARIOUS: usize = 4;
pub const COMMON_2: usize = 5;
pub const HELP: usize = 6;
pub const BG_08: usize = 7;
pub const BG_05: usize = 8;
pub const CAPTION: usize = 9;
pub const NO_IMAGE: usize = 10;
pub const TEXTURES: usize = 11;

/// Frames the common fade takes (`anm_common_fade_in`).
const FADE_FRAMES: f32 = 15.0;
/// The help bar fades in from this many frames after the screen starts (capture: 9/15 at age 11).
const HELP_AT: f32 = 2.0;
/// The help plate's shortest width, and the frames it takes to grow (as the load screen's).
const PLATE_MIN: f32 = 376.5 / 1.5;
const PLATE_FRAMES: f32 = 6.4;

// STAND-IN: the rows' pitch is fitted to the capture (the arrows' tops on rows 0 to 11: 51.409 pixels apart); the real thing is the menu component's own item height, not in the layout (its template rows sit at 130, 52, 98, ...).
/// How far apart the list's rows are, in layout pixels.
const ROW_PITCH: f32 = 377.0 / 11.0;
// STAND-IN: the plain list's rows sit this far from the graphics list's (capture: one layout pixel left, three lower); the real thing is the second menu's own item origin.
const PLAIN_SHIFT: [f32; 2] = [-1.0, 3.0];
// STAND-IN: text set centred on its node (`align_v=center`) has its line top this far above the node's `pos.y` less half its size, fitted to the capture (the description's heading, the rows' names, the shoulder keys); the real thing is the game's own line box.
const HEAD_FIT: f32 = -4.0;
const KEY_FIT: f32 = -7.8;
const NUM_FIT: f32 = -3.53;
const NAME_FIT: f32 = -6.0;
// STAND-IN: the description's lines are 34 pixels apart at 1080 lines (capture), the layout's size 16 and `line_space` 10 read as 16 + 10 / 1.5.
const LINE_SPACE: f32 = 10.0 / 1.5;
/// A description line holds this many half-width characters (`line_char_length` 20 full-width).
const LINE_CHARS: f32 = 40.0;

/// The text colour of the template `TEXTCOLOR_BROWN` (26, 0, 0), the tags `<CLGR>` (green) and `<CLRE>` (red).
const BROWN: [f32; 3] = [26.0 / 255.0, 0.0, 0.0];
const GREEN: [f32; 3] = [0.0, 140.0 / 255.0, 0.0];
const RED: [f32; 3] = [200.0 / 255.0, 0.0, 0.0];

/// What the screen reads of the game's files.
pub struct Sources<'a> {
    pub settings: &'a [u8],
    pub tab: &'a [u8],
    pub saveload: &'a [u8],
    pub help: &'a [u8],
    pub option_styles: &'a [u8],
    pub v2_common_styles: &'a [u8],
    pub v2_various_styles: &'a [u8],
    pub common_styles: &'a [u8],
    pub caption_styles: &'a [u8],
    pub anims: &'a [u8],
    pub glyphs: [HashMap<char, Glyph>; 2],
    /// The atlases in style pixels, by texture number (`OPTION` ... `CAPTION`; the others are 0).
    pub atlases: [(u32, u32); TEXTURES],
}

/// A row of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub value: String,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A choice among words (arrows on the cursor's row).
    Select,
    /// A number on a gauge: how far along it is (0 to 1) and the number.
    Gauge(f32, String),
}

/// Everything the screen shows besides its layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub caption: String,
    pub tabs: Vec<String>,
    pub tab: usize,
    pub rows: Vec<Row>,
    pub cursor: usize,
    /// The description panel: the item's name and its text (`^00` breaks a line, `<CLGR>` ... `<CLNR>` colours).
    pub title: String,
    pub text: String,
    /// The help bar's entries: the button's glyph and its words.
    pub help: Vec<(char, String)>,
    /// The two shoulder keys' glyphs at the tabs' ends.
    pub shoulders: [char; 2],
}

/// Where a frame of the screen is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    /// Frames since the screen started to fade in.
    pub age: i32,
    /// The cursor's blink in frames (`anm_common_blink`'s 120).
    pub blink: f32,
}

struct Template {
    mask: [f32; 2],
    name: [f32; 2],
    value: [f32; 2],
    arrow_left: [f32; 2],
    arrow_right: [f32; 2],
    bar_base: [f32; 2],
    bar_gauge: [f32; 2],
    bar_size: [f32; 2],
    bar_num: [f32; 2],
    mask_size: [f32; 2],
}

/// One of the two lists: the layout's node, where its first row's node sits inside it and how
/// far the game's own placing of the rows differs from the layout's.
struct Menu {
    at: [f32; 2],
    clone: [f32; 2],
    shift: [f32; 2],
    /// Whether the list's scroll bar is drawn (the graphics list's is, the plain list's is not).
    scroll: bool,
}

pub struct View {
    glyphs: [HashMap<char, Glyph>; 2],
    atlases: [(f32, f32); TEXTURES],
    blink: Curve,
    tab_line: Style,
    tab_line_at: [f32; 2],
    tab_line_size: [f32; 2],
    tab_l: Style,
    tab_l_at: [f32; 2],
    tab_r: Style,
    tab_r_at: [f32; 2],
    tab_end_scale: [f32; 2],
    tab_pictures: Style,
    tab_at: Vec<[f32; 2]>,
    tab_picture_at: [f32; 2],
    tab_text: Label,
    tab_text_at: [f32; 2],
    key_base: Style,
    key_at: [[f32; 2]; 2],
    key_text: [Label; 2],
    key_text_at: [[f32; 2]; 2],
    caption: Style,
    caption_at: [f32; 2],
    caption_text: Label,
    panel: Style,
    panel_at: [f32; 2],
    panel_size: [f32; 2],
    head: Style,
    head_at: [f32; 2],
    head_size: [f32; 2],
    head_color: [f32; 3],
    head_text: Label,
    head_text_at: [f32; 2],
    body_text: Label,
    body_text_at: [f32; 2],
    window: Style,
    window_size: [f32; 2],
    window_at: [f32; 2],
    lines: Vec<[f32; 2]>,
    line: Style,
    line_size: [f32; 2],
    line_color: [f32; 3],
    line_alpha: f32,
    line_scale: [f32; 2],
    scroll: Style,
    scroll_at: [f32; 2],
    scroll_size: [f32; 2],
    cursor: Style,
    cursor_at: [f32; 2],
    cursor_size: [f32; 2],
    rows_at: [f32; 2],
    template: Template,
    name_text: Label,
    value_text: Label,
    num_text: Label,
    arrow: Style,
    gauge_base: Style,
    gauge: Style,
    menus: [Menu; 2],
    help: Style,
    help_at: [f32; 2],
    help_text: Label,
    help_left: f32,
}

fn first(s: &Style) -> Result<[f32; 4], String> {
    s.pictures.first().copied().ok_or_else(|| "a style without a picture".to_string())
}

fn node<'a>(layout: &'a Element, name: &str) -> Result<(&'a Element, [f32; 2]), String> {
    find(layout, name, [0.0; 2]).ok_or(format!("no node {name}"))
}

fn style_of(styles: &[&Element], e: &Element) -> Result<Style, String> {
    let name = e.attr("style").ok_or(format!("{}: no style", e.attr("name").unwrap_or("?")))?;
    styles.iter().find_map(|s| style(s, name).ok()).ok_or(format!("no style {name}"))
}

impl View {
    pub fn read(src: Sources) -> Result<View, String> {
        let parse = |b: &[u8], what: &str| table::parse(b).map_err(|e| format!("{what}: {e}"));
        let settings = parse(src.settings, "uil_quick_menu_settings")?;
        let tab = parse(src.tab, "uil_quick_menu_tab")?;
        let saveload = parse(src.saveload, "uil_a21_saveload")?;
        let help_layout = parse(src.help, "uil_a21_shade")?;
        let styles = [
            parse(src.option_styles, "uis_gen_a21_option")?,
            parse(src.v2_common_styles, "uis_gen_a21v2_common")?,
            parse(src.v2_various_styles, "uis_gen_a21v2_various")?,
            parse(src.common_styles, "uis_gen_a21_common")?,
            parse(src.caption_styles, "uis_gen_a21_caption")?,
        ];
        let styles: Vec<&Element> = styles.iter().collect();
        let st = |e: &Element| style_of(&styles, e);
        let by_name = |n: &str| styles.iter().find_map(|s| style(s, n).ok()).ok_or(format!("no style {n}"));

        // The tabs (the Steam set: six).
        let (steam, steam_at) = node(&tab, "tabs_Steam")?;
        let (base, base_at) = inside(steam, "base", steam_at).ok_or("tabs_Steam: no base")?;
        let (line, line_at) = inside(base, "line", base_at).ok_or("base: no line")?;
        let (l_bg, l_at) = inside(base, "l_bg", base_at).ok_or("base: no l_bg")?;
        let (r_bg, r_at) = inside(base, "r_bg", base_at).ok_or("base: no r_bg")?;
        let tab_nodes: Vec<(&Element, [f32; 2])> = steam.children.iter().filter(|c| c.attr("name").is_some_and(|n| n.starts_with("option_tab"))).map(|c| (c, [steam_at[0] + pair(c, "pos", [0.0; 2])[0], steam_at[1] + pair(c, "pos", [0.0; 2])[1]])).collect();
        let (tab0, _) = tab_nodes[0];
        let (off, off_rel) = inside(tab0, "tab_off", [0.0; 2]).ok_or("option_tab1: no tab_off")?;
        let tab_label = label(tab0, "text", "option_tab1")?;
        let (_, tab_text_at) = inside(tab0, "text", [0.0; 2]).ok_or("option_tab1: no text")?;
        let (lr, lr_at) = inside(steam, "lr_key", steam_at).ok_or("tabs_Steam: no lr_key")?;
        let (l_base, l_base_at) = inside(lr, "l_base", lr_at).ok_or("lr_key: no l_base")?;
        let (r_base, r_base_at) = inside(lr, "r_base", lr_at).ok_or("lr_key: no r_base")?;
        let (l_sel, l_text_at) = inside(l_base, "select", [0.0; 2]).ok_or("l_base: no select")?;
        let (r_sel, r_text_at) = inside(r_base, "select", [0.0; 2]).ok_or("r_base: no select")?;

        // The caption of the save screens' layout.
        let (caption_node, caption_at) = node(&saveload, "caption_main")?;

        // The description panel.
        let (info, _) = node(&settings, "info_window")?;
        let info_at = node(&settings, "info_window")?.1;
        let (base_n, base_n_at) = inside(info, "base", info_at).ok_or("info_window: no base")?;
        let (panel_n, panel_at) = inside(base_n, "image", base_n_at).ok_or("info_window/base: no image")?;
        let (head_n, head_rel) = inside(panel_n, "header_base", [0.0; 2]).ok_or("info image: no header_base")?;
        let (name_n, name_at) = inside(info, "heade_rname", info_at).ok_or("info_window: no heade_rname")?;
        let (body_n, body_at) = inside(info, "option_explanation", info_at).ok_or("info_window: no option_explanation")?;
        let size_of = |e: &Element| pair(e, "size_wh", [0.0; 2]);
        let color_of = |e: &Element| -> [f32; 3] {
            let c = numbers(e, "color");
            [c.first().copied().unwrap_or(255.0) / 255.0, c.get(1).copied().unwrap_or(255.0) / 255.0, c.get(2).copied().unwrap_or(255.0) / 255.0]
        };
        // A text without a size is the template's 24; its node's scale multiplies it.
        let text_label = |e: &Element| Label {
            at: [0.0; 2],
            size: e.attr("font_size").and_then(|v| v.trim().parse().ok()).unwrap_or(24.0) * pair(e, "scale", [1.0; 2])[0],
            small: e.attr("font") == Some("MainFont01"),
            align: match e.attr("align") {
                Some("center") => Align::Center,
                Some("right") => Align::Right,
                _ => Align::Left,
            },
        };

        // The lists: the template's window, lines, scroll bar and cursor, and the rows' template.
        let (common, _) = node(&settings, "menu_template_common")?;
        let (win, win_at) = inside(common, "__uimenu_window", [0.0; 2]).ok_or("template: no window")?;
        let lines: Vec<[f32; 2]> = win.children.iter().filter(|c| c.name == "image").map(|c| pair(c, "pos", [0.0; 2])).collect();
        let first_line = win.children.iter().find(|c| c.name == "image").ok_or("window: no lines")?;
        let (scroll_n, scroll_at) = node(common, "scroll_base_1")?;
        let (cursor_n, cursor_at) = node(common, "__uimenu_cursor_active")?;
        let (_, list_at) = node(common, "__uimenu_item_list")?;
        let (_, first_clone) = node(common, "clone_template_select")?;
        let (select_t, _) = node(&settings, "template_select")?;
        let sub = |n: &str| inside(select_t, n, [0.0; 2]).ok_or(format!("template_select: no {n}"));
        let (mask_n, mask_at) = sub("image_2")?;
        let (name_t, name_t_at) = sub("name_text")?;
        let (value_t, value_t_at) = sub("select_text")?;
        let (left_n, left_at) = sub("left_arrow")?;
        let (_, right_at) = sub("right_arrow")?;
        let (base_g, base_g_at) = sub("base")?;
        let (gauge_n, gauge_at) = sub("gauge")?;
        let (num_n, num_at) = sub("num")?;
        let menu = |n: &str, shift: [f32; 2], scroll: bool| -> Result<Menu, String> {
            let (m, at) = node(&settings, n)?;
            let c = m.children.iter().find(|c| c.attr("name") == Some("clone_template_select")).ok_or("menu: no row")?;
            Ok(Menu { at, clone: pair(c, "pos", [0.0; 2]), shift, scroll })
        };
        let menus = [menu("menu_graphic", [0.0; 2], true)?, menu("menu_otption", PLAIN_SHIFT, false)?];

        // The help bar, as the load screen's.
        let (help_bg, help_at) = node(&help_layout, "base")?;
        let (_, left_side) = inside(help_bg, "left_side", [0.0; 2]).ok_or("uil_a21_shade: no left_side")?;
        let help_label = label(&help_layout, "text", "bg")?;

        let gauge_style = st(gauge_n)?;
        let blink = curve(src.anims, "anm_common_blink", "color")?;
        // Row 0's node inside the list: the template's item list and its first row's place.
        let rows_at = [list_at[0], first_clone[1]];
        let _ = (name_n, body_n, head_n);
        Ok(View {
            glyphs: src.glyphs,
            atlases: src.atlases.map(|(w, h)| (w as f32, h as f32)),
            blink,
            tab_line: st(line)?,
            tab_line_at: line_at,
            tab_line_size: size_of(line),
            tab_l: st(l_bg)?,
            tab_l_at: l_at,
            tab_r: st(r_bg)?,
            tab_r_at: r_at,
            tab_end_scale: pair(l_bg, "scale", [1.0; 2]),
            tab_pictures: st(off)?,
            tab_at: tab_nodes.iter().map(|t| t.1).collect(),
            tab_picture_at: off_rel,
            tab_text: tab_label,
            tab_text_at,
            key_base: st(l_base)?,
            key_at: [l_base_at, r_base_at],
            key_text: [text_label(l_sel), text_label(r_sel)],
            key_text_at: [l_text_at, r_text_at],
            caption: st(caption_node)?,
            caption_at,
            caption_text: label(caption_node, "main", "caption_main")?,
            panel: st(panel_n)?,
            panel_at,
            panel_size: size_of(panel_n),
            head: st(head_n)?,
            head_at: [panel_at[0] + head_rel[0], panel_at[1] + head_rel[1]],
            head_size: size_of(head_n),
            head_color: color_of(head_n),
            head_text: text_label(name_n),
            head_text_at: name_at,
            body_text: text_label(body_n),
            body_text_at: body_at,
            window: st(win)?,
            window_size: size_of(win),
            window_at: win_at,
            lines,
            line: st(first_line)?,
            line_size: size_of(first_line),
            line_color: color_of(first_line),
            line_alpha: numbers(first_line, "color").get(3).copied().unwrap_or(255.0) / 255.0,
            line_scale: pair(first_line, "scale", [1.0; 2]),
            scroll: st(scroll_n)?,
            scroll_at,
            scroll_size: size_of(scroll_n),
            cursor: st(cursor_n)?,
            cursor_at,
            cursor_size: size_of(cursor_n),
            rows_at,
            template: Template {
                mask: mask_at,
                name: name_t_at,
                value: value_t_at,
                arrow_left: left_at,
                arrow_right: right_at,
                bar_base: base_g_at,
                bar_gauge: gauge_at,
                bar_size: size_of(gauge_n),
                bar_num: num_at,
                mask_size: size_of(mask_n),
            },
            name_text: text_label(name_t),
            value_text: text_label(value_t),
            num_text: text_label(num_n),
            arrow: st(left_n)?,
            gauge_base: st(base_g)?,
            gauge: gauge_style,
            menus,
            help: by_name(help_bg.attr("style").ok_or("uil_a21_shade: base has no style")?)?,
            help_at,
            help_text: help_label,
            help_left: left_side[0],
        })
    }
}

struct Pen<'a> {
    view: &'a View,
    k: f32,
    left: f32,
    out: Vec<Quad>,
}

/// One text's look: size, font and alignment as the layout gives them.
struct Look {
    size: f32,
    small: bool,
    align: Align,
    /// Set centred on the node: how far above its place the line sits (`None`: the line's top at the node).
    centred: Option<f32>,
}

impl Pen<'_> {
    fn pic(&mut self, texture: usize, uvwh: [f32; 4], rect: [f32; 4], color: [f32; 4]) {
        let (aw, ah) = self.view.atlases[texture];
        let [x, y, w, h] = uvwh;
        self.out.push(Quad { turn: false, spin: None, rect, uv: [x / aw, y / ah, (x + w) / aw, (y + h) / ah], color, texture });
    }

    /// A picture at its natural size with its corner at `at` (layout pixels).
    fn natural(&mut self, texture: usize, s: &Style, n: usize, at: [f32; 2], color: [f32; 4]) {
        let p = s.pictures[n];
        let (l, t) = (self.left + at[0] * self.k, at[1] * self.k);
        let (w, h) = (p[2] / ATLAS_PER_LAYOUT * self.k, p[3] / ATLAS_PER_LAYOUT * self.k);
        self.pic(texture, p, [l, t, l + w, t + h], color);
    }

    /// A picture at its natural size times `scale`, about its corner `at`; a negative scale turns the picture over.
    fn scaled(&mut self, texture: usize, s: &Style, at: [f32; 2], scale: [f32; 2], color: [f32; 4]) {
        let p = s.pictures[0];
        let (x, y) = (self.left + at[0] * self.k, at[1] * self.k);
        let (w, h) = (p[2] / ATLAS_PER_LAYOUT * self.k * scale[0], p[3] / ATLAS_PER_LAYOUT * self.k * scale[1]);
        let (aw, ah) = self.view.atlases[texture];
        let (u0, u1) = (p[0] / aw, (p[0] + p[2]) / aw);
        let (v0, v1) = (p[1] / ah, (p[1] + p[3]) / ah);
        let (xs, us) = if w < 0.0 { ([x + w, x], [u1, u0]) } else { ([x, x + w], [u0, u1]) };
        let (ys, vs) = if h < 0.0 { ([y + h, y], [v1, v0]) } else { ([y, y + h], [v0, v1]) };
        self.out.push(Quad { turn: false, spin: None, rect: [xs[0], ys[0], xs[1], ys[1]], uv: [us[0], vs[0], us[1], vs[1]], color, texture });
    }

    /// A stretched ("wrap") picture `size` layout pixels at `at`: each margin at its natural size
    /// and the middle tiled at natural size along each axis, the last tile cut short; a middle of
    /// no size (the margins take the whole picture) is one strip at natural scale.
    fn wrap(&mut self, texture: usize, s: &Style, at: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        self.wrap_scaled(texture, s, at, size, [1.0; 2], color);
    }

    /// A stretched picture drawn, as it is, at `scale` about its corner.
    fn wrap_scaled(&mut self, texture: usize, s: &Style, at: [f32; 2], size: [f32; 2], scale: [f32; 2], color: [f32; 4]) {
        let p = s.pictures[0];
        let [l, t, r, b] = s.edges.unwrap_or([0.0; 4]);
        let u = self.k / ATLAS_PER_LAYOUT;
        let cols = segments(size[0] * self.k, l, p[2] - l - r, r, u);
        let rows = segments(size[1] * self.k, t, p[3] - t - b, b, u);
        let (ox, oy) = (self.left + at[0] * self.k, at[1] * self.k);
        for row in &rows {
            for col in &cols {
                let uvwh = [p[0] + col.2, p[1] + row.2, col.3, row.3];
                let (x0, y0) = (ox + col.0 * scale[0], oy + row.0 * scale[1]);
                self.pic(texture, uvwh, [x0, y0, x0 + col.1 * scale[0], y0 + row.1 * scale[1]], color);
            }
        }
    }

    /// Text at the node `at` (layout pixels), the colour's alpha as given; `bold` draws a ring of
    /// copies around it first, `reach` layout pixels out, in the ring's colour.
    fn text(&mut self, look: &Look, at: [f32; 2], text: &str, color: [f32; 4], bold: Option<(f32, [f32; 4])>) {
        let colored: Vec<(char, [f32; 4])> = text.chars().map(|c| (c, color)).collect();
        self.runs(look, at, &colored, bold);
    }

    fn runs(&mut self, look: &Look, at: [f32; 2], chars: &[(char, [f32; 4])], bold: Option<(f32, [f32; 4])>) {
        let font = usize::from(!look.small);
        let glyphs = &self.view.glyphs[font];
        let s = look.size * self.k / ATLAS_SIZE;
        let width: f32 = chars.iter().filter_map(|(c, _)| glyphs.get(c)).map(|g| g.advance as f32 * s).sum();
        let x = self.left + at[0] * self.k;
        let pen0 = match look.align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let top = match look.centred {
            Some(fit) => (at[1] + fit) * self.k - look.size * self.k / 2.0,
            None => (at[1] + TEXT_TOP) * self.k,
        };
        let tex = if font == 0 { FONT01 } else { FONT00 };
        let (aw, ah) = (4096.0, 2048.0);
        let mut run = |out: &mut Vec<Quad>, dx: f32, dy: f32, ring: Option<[f32; 4]>| {
            let mut pen = pen0 + dx;
            for (c, col) in chars {
                let Some(g) = glyphs.get(c) else { continue };
                let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
                let (l, t) = (pen + g.x_offset as f32 * s, top + dy + g.y_offset as f32 * s);
                out.push(Quad { turn: false, spin: None, rect: [l, t, l + gw * s, t + gh * s], uv: [gx / aw, gy / ah, (gx + gw) / aw, (gy + gh) / ah], color: ring.unwrap_or(*col), texture: tex });
                pen += g.advance as f32 * s;
            }
        };
        if let Some((reach, ring)) = bold {
            let r = reach * self.k;
            for dx in [-r, 0.0, r] {
                for dy in [-r, 0.0, r] {
                    run(&mut self.out, dx, dy, Some(ring));
                }
            }
        }
        run(&mut self.out, 0.0, 0.0, None);
    }

    fn width(&self, look: &Look, text: &str) -> f32 {
        let s = look.size * self.k / ATLAS_SIZE;
        text.chars().filter_map(|c| self.view.glyphs[usize::from(!look.small)].get(&c)).map(|g| g.advance as f32 * s).sum()
    }
}

/// One axis of a stretched picture: `total` pixels split into the near margin (`a` style
/// pixels), the middle (`mid` style pixels of source, tiled) and the far margin (`b`); each
/// piece is (destination start, destination length, source start, source length), `u` pixels
/// to a style pixel.
fn segments(total: f32, a: f32, mid: f32, b: f32, u: f32) -> Vec<(f32, f32, f32, f32)> {
    let total = total.max((a + b) * u);
    let mut out = Vec::new();
    if a > 0.0 {
        out.push((0.0, a * u, 0.0, a));
    }
    let end = total - b * u;
    let mut at = a * u;
    if mid > 0.0 {
        let tile = mid * u;
        while end - at > 0.001 {
            let len = tile.min(end - at);
            out.push((at, len, a, len / u));
            at += len;
        }
    } else if end - at > 0.001 {
        out.push((at, end - at, a, (end - at) / u));
    }
    if b > 0.0 {
        out.push((end, b * u, a + mid, b));
    }
    out
}

/// Splits `text` into the description's lines: `^00` breaks a line, a line wraps at a space
/// past `LINE_CHARS`, `<CLGR>`, `<CLRE>` and `<CLNR>` set the colour from the next glyph (a tag
/// right after a break takes effect a glyph late, as the capture draws it).
fn description(text: &str) -> Vec<Vec<(char, [f32; 3])>> {
    let mut lines: Vec<Vec<(char, [f32; 3])>> = vec![Vec::new()];
    let mut color = BROWN;
    let mut pending: Option<[f32; 3]> = None;
    let mut words: Vec<Vec<(char, [f32; 3])>> = Vec::new();
    let mut word: Vec<(char, [f32; 3])> = Vec::new();
    let mut after_break = true;
    let mut chars = text.chars().peekable();
    let flush = |lines: &mut Vec<Vec<(char, [f32; 3])>>, word: &mut Vec<(char, [f32; 3])>| {
        let count = |l: &Vec<(char, [f32; 3])>| l.iter().map(|(c, _)| if c.is_ascii() { 1.0 } else { 2.0 }).sum::<f32>();
        let last = lines.last_mut().unwrap();
        if !last.is_empty() && count(last) + count(word) > LINE_CHARS {
            lines.push(std::mem::take(word));
        } else {
            last.append(word);
        }
    };
    while let Some(c) = chars.next() {
        if c == '<' {
            let tag: String = chars.by_ref().take_while(|&c| c != '>').collect();
            let to = match tag.as_str() {
                "CLGR" => Some(GREEN),
                "CLRE" => Some(RED),
                "CLNR" | "CLEG" => Some(BROWN),
                _ => None,
            };
            if let Some(to) = to {
                if after_break { pending = Some(to) } else { color = to }
            }
            continue;
        }
        if c == '^' {
            let _: String = chars.by_ref().take(2).collect();
            flush(&mut lines, &mut word);
            lines.push(Vec::new());
            after_break = true;
            continue;
        }
        word.push((c, color));
        if after_break {
            after_break = false;
            if let Some(p) = pending.take() {
                color = p;
            }
        }
        if c == ' ' {
            flush(&mut lines, &mut word);
        }
    }
    flush(&mut lines, &mut word);
    let _ = &mut words;
    lines
}

impl View {
    fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
        [c[0], c[1], c[2], a]
    }

    /// The screen's quads for `screen` at `moment`, in draw order, on a window of `window` pixels.
    pub fn quads(&self, s: &Screen, moment: Moment, window: (u32, u32)) -> Vec<Quad> {
        let k = window.1 as f32 / LAYOUT_HEIGHT;
        let left = (window.0 as f32 - 1280.0 * k) * 0.5;
        let mut pen = Pen { view: self, k, left, out: Vec::new() };
        let fade = (moment.age as f32 / FADE_FRAMES).clamp(0.0, 1.0);
        let white = [1.0; 4];
        let brown = |a: f32| Self::rgba(BROWN, a);
        if moment.age <= 0 {
            return pen.out;
        }

        // The background (the screen's picture tiled twice over: 3840 x 2160 style pixels drawn at twice their layout size) and the caption.
        let big = [0.0, 0.0, window.0 as f32 * 2.0, window.1 as f32 * 2.0];
        for texture in [BG_08, BG_05] {
            pen.out.push(Quad { turn: false, spin: None, rect: big, uv: [0.0, 0.0, 2.0, 2.0], color: [1.0, 1.0, 1.0, fade], texture });
        }
        pen.natural(CAPTION, &self.caption, 0, self.caption_at, [1.0, 1.0, 1.0, fade]);
        let look = |l: &Label, centred: Option<f32>| Look { size: l.size, small: l.small, align: l.align, centred };
        pen.text(&look(&self.caption_text, None), [self.caption_at[0] + self.caption_text.at[0], self.caption_at[1] + self.caption_text.at[1]], &s.caption, brown(fade), None);

        // The tabs.
        pen.wrap(V2_COMMON, &self.tab_line, self.tab_line_at, self.tab_line_size, white);
        pen.scaled(V2_COMMON, &self.tab_l, self.tab_l_at, self.tab_end_scale, white);
        pen.scaled(V2_COMMON, &self.tab_r, self.tab_r_at, self.tab_end_scale, white);
        let tab_color = [63.0 / 255.0, 45.0 / 255.0, 16.0 / 255.0, 1.0];
        for (i, name) in s.tabs.iter().enumerate().take(self.tab_at.len()) {
            let at = self.tab_at[i];
            let picture = [at[0] + self.tab_picture_at[0], at[1] + self.tab_picture_at[1]];
            pen.natural(OPTION, &self.tab_pictures, 1, picture, white);
            if i == s.tab {
                pen.natural(OPTION, &self.tab_pictures, 0, picture, white);
            }
            pen.text(&look(&self.tab_text, None), [at[0] + self.tab_text_at[0], at[1] + self.tab_text_at[1]], name, tab_color, None);
        }
        for n in 0..2 {
            pen.natural(V2_COMMON, &self.key_base, 0, self.key_at[n], white);
            let at = [self.key_at[n][0] + self.key_text_at[n][0], self.key_at[n][1] + self.key_text_at[n][1]];
            pen.text(&look(&self.key_text[n], Some(KEY_FIT)), at, &s.shoulders[n].to_string(), white, None);
        }

        // The description panel.
        pen.wrap(V2_COMMON, &self.panel, self.panel_at, self.panel_size, white);
        pen.wrap(V2_COMMON, &self.head, self.head_at, self.head_size, Self::rgba(self.head_color, 1.0));
        pen.text(&look(&self.head_text, Some(HEAD_FIT)), self.head_text_at, &s.title, white, None);
        let body = look(&self.body_text, None);
        for (n, line) in description(&s.text).iter().enumerate() {
            let at = [self.body_text_at[0], self.body_text_at[1] + n as f32 * (self.body_text.size + LINE_SPACE)];
            let chars: Vec<(char, [f32; 4])> = line.iter().map(|&(c, col)| (c, Self::rgba(col, 1.0))).collect();
            pen.runs(&body, at, &chars, None);
        }

        // The graphics list, with the rows' template words (the game draws it under the plain list), then the plain list.
        let placeholder: Vec<Row> = (0..12).map(|_| Row { name: PLACEHOLDER.0.to_string(), value: PLACEHOLDER.1.to_string(), kind: Kind::Gauge(1.0, "0".into()) }).collect();
        self.list(&mut pen, &self.menus[0], &placeholder, 0, moment, true);
        self.list(&mut pen, &self.menus[1], &s.rows, s.cursor, moment, false);

        // The help bar.
        self.help_bar(&mut pen, &s.help, moment);
        pen.out
    }

    fn list(&self, pen: &mut Pen, menu: &Menu, rows: &[Row], cursor: usize, moment: Moment, template: bool) {
        let white = [1.0; 4];
        let at = menu.at;
        pen.wrap(V2_VARIOUS, &self.window, [at[0] + self.window_at[0], at[1] + self.window_at[1]], self.window_size, white);
        for l in &self.lines {
            let p = [at[0] + self.window_at[0] + l[0], at[1] + self.window_at[1] + l[1]];
            pen.wrap_scaled(V2_COMMON, &self.line, p, self.line_size, self.line_scale, Self::rgba(self.line_color, self.line_alpha));
        }
        if menu.scroll {
            pen.wrap(V2_VARIOUS, &self.scroll, [at[0] + self.scroll_at[0], at[1] + self.scroll_at[1]], self.scroll_size, [1.0, 1.0, 1.0, 127.0 / 255.0]);
        }
        let blink = self.blink.at(moment.blink.rem_euclid(120.0) / 60.0)[3] / 255.0;
        let cursor_at = [at[0] + self.cursor_at[0], at[1] + self.cursor_at[1] + cursor as f32 * ROW_PITCH];
        pen.wrap(COMMON_2, &self.cursor, cursor_at, self.cursor_size, [1.0, 1.0, 1.0, blink]);
        for (n, row) in rows.iter().enumerate().take(12) {
            let o = [menu.at[0] + menu.clone[0] + menu.shift[0] + self.rows_at[0], menu.at[1] + menu.clone[1] + menu.shift[1] + self.rows_at[1] + n as f32 * ROW_PITCH];
            self.row(pen, row, o, template || n == cursor);
        }
    }

    fn row(&self, pen: &mut Pen, row: &Row, o: [f32; 2], arrows: bool) {
        let t = &self.template;
        let look = |l: &Label, centred: Option<f32>| Look { size: l.size, small: l.small, align: l.align, centred };
        let brown = Self::rgba(BROWN, 1.0);
        let abs = |p: [f32; 2]| [o[0] + p[0], o[1] + p[1]];
        let mask = abs(t.mask);
        let k = pen.k;
        let rect = [pen.left + mask[0] * k, mask[1] * k, pen.left + (mask[0] + t.mask_size[0]) * k, (mask[1] + t.mask_size[1]) * k];
        pen.out.push(Quad { turn: false, spin: None, rect, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], texture: NO_IMAGE });
        pen.text(&look(&self.name_text, Some(NAME_FIT)), abs(t.name), &row.name, brown, None);
        // The buttons' pictures in a value are drawn white, the words brown.
        let value: Vec<(char, [f32; 4])> = row.value.chars().map(|c| (c, if ('\u{4d0}'..'\u{500}').contains(&c) { [1.0; 4] } else { brown })).collect();
        pen.runs(&look(&self.value_text, None), abs(t.value), &value, None);
        if arrows {
            let p = self.arrow.pictures[0];
            let (w, h) = (p[2] / ATLAS_PER_LAYOUT, p[3] / ATLAS_PER_LAYOUT);
            let (lx, rx) = (o[0] + t.arrow_left[0], o[0] + t.arrow_right[0]);
            let y = o[1] + t.arrow_left[1];
            let (aw, ah) = pen.view.atlases[OPTION];
            let uv = [p[0] / aw, p[1] / ah, (p[0] + p[2]) / aw, (p[1] + p[3]) / ah];
            // The left arrow is the same picture turned about its left edge.
            pen.out.push(Quad { turn: false, spin: None, rect: [pen.left + (lx - w) * k, y * k, pen.left + lx * k, (y + h) * k], uv: [uv[2], uv[1], uv[0], uv[3]], color: [1.0; 4], texture: OPTION });
            pen.out.push(Quad { turn: false, spin: None, rect: [pen.left + rx * k, y * k, pen.left + (rx + w) * k, (y + h) * k], uv, color: [1.0; 4], texture: OPTION });
        }
        if let Kind::Gauge(ratio, number) = &row.kind {
            let bar = abs(t.bar_base);
            pen.natural(V2_VARIOUS, &self.gauge_base, 0, bar, [1.0; 4]);
            let g = first(&self.gauge).unwrap_or_default();
            let gp = abs(t.bar_gauge);
            let full = t.bar_size[0];
            let (aw, ah) = pen.view.atlases[V2_VARIOUS];
            let part = ratio.clamp(0.0, 1.0);
            let rect = [pen.left + gp[0] * k, gp[1] * k, pen.left + (gp[0] + full * part) * k, (gp[1] + t.bar_size[1]) * k];
            pen.out.push(Quad { turn: false, spin: None, rect, uv: [g[0] / aw, g[1] / ah, (g[0] + g[2] * part) / aw, (g[1] + g[3]) / ah], color: [1.0; 4], texture: V2_VARIOUS });
            let num = abs(t.bar_num);
            pen.text(&look(&self.num_text, Some(NUM_FIT)), num, number, [1.0; 4], Some((0.7, [BROWN[0], 0.0, 0.0, 0.5])));
        }
    }

    fn help_bar(&self, pen: &mut Pen, entries: &[(char, String)], moment: Moment) {
        let alpha = ((moment.age as f32 - HELP_AT) / FADE_FRAMES).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        let grow = ((moment.age as f32 - HELP_AT) / PLATE_FRAMES).clamp(0.0, 1.0);
        let guide = entries.iter().map(|(g, w)| format!("{g}{w}")).collect::<Vec<_>>().join(" ");
        let look = Look { size: self.help_text.size, small: self.help_text.small, align: self.help_text.align, centred: None };
        let s = self.help_text.size * pen.k / ATLAS_SIZE;
        let words = pen.width(&look, &guide) / pen.k;
        let right = 1280.0;
        // STAND-IN: The plate is one atlas pixel of the text wider than the advances' sum (as the load screen's).
        let full = words + 2.0 * self.help_text.size / ATLAS_SIZE - self.help_text.at[0] + self.help_left;
        let _ = s;
        let width = PLATE_MIN + (full - PLATE_MIN) * grow;
        let at = [right - width, self.help_at[1]];
        pen.wrap(HELP, &self.help, at, [width, first(&self.help).map_or(0.0, |p| p[3] / ATLAS_PER_LAYOUT)], [1.0, 1.0, 1.0, alpha]);
        pen.text(&look, [1280.0 + self.help_text.at[0], 720.0 + self.help_text.at[1]], &guide, [1.0, 1.0, 1.0, alpha], None);
    }
}

/// The graphics list's rows' template words, the ones the game leaves in it.
const PLACEHOLDER: (&str, &str) = ("Ambient Occlusion", "1920x1080");
