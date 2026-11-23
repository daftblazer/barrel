//! The field menu's Equipment page: the character's figure, the four slots with the worn items' pictures and
//! the stat panel with the real numbers, from the game's layouts (`uil_a21_menu_equip.xml`, `uil_a21_menu_chara.xml`,
//! `uil_a21_menu_chara_status.xml`) and their style files (`ui_cmn/gen_styles`, `ui_cmn/etc`).
//!
//! The layouts are read by a small generic walker: a node has a place, a turn and a scale about its top left,
//! an image takes its style's picture (a list's `image_no`) at the style's natural size or its `size_wh`, a text
//! its string, font and colour (with the nine-copy outline of `bold_size`). What the game's code decides (which
//! nodes show, the numbers, the item pictures) is given to the walker as a list of overrides by node path.
//!
//! Only the page's steady state is drawn; the item card, the title plate, the party icons, the help bar, the
//! element resists and every animation are not (docs/ryza/open.md, "Field menu, part 6").

use std::collections::{BTreeSet, HashMap};

use barrel_formats::table::{self, Element};
use barrel_render::text::{Glyph, Quad, Spin};

use crate::font::ATLAS_SIZE;
use crate::load_view::TEXT_TOP;
use crate::title::{numbers, pair, LAYOUT_HEIGHT};

/// The page layer's textures: the two fonts and the white, then the styles' (`Page::wanted`).
pub const FONT01: usize = 0;
pub const FONT00: usize = 1;
pub const WHITE: usize = 2;
const FIXED: usize = 3;

/// Style pixels per layout pixel (the styles are drawn for a 2160-line screen, the layouts for 720).
const PER_LAYOUT: f32 = 3.0;
/// A texture's size in style pixels over its own.
const STYLE_OVER_TEXTURE: f32 = 2.0;
/// `STR_UI_nnnn` is the string numbered `STR_UI_BASE + nnnn`.
const STR_UI_BASE: u32 = 6_619_137;
const OUTLINE_ALPHA: f32 = 0.5;
const BROWN: [f32; 4] = [26.0, 0.0, 0.0, 255.0];

/// What the page reads from the game's files, by install path (`\saves\ui_en\...`).
pub struct Files<'a> {
    pub read: &'a dyn Fn(&str) -> Result<Vec<u8>, String>,
    pub glyphs: [HashMap<char, Glyph>; 2],
}

/// A style's picture: its texture (a slot of the layer), its place in the atlas and the texture's size.
#[derive(Clone, Debug)]
struct Pic {
    path: String,
    index: usize,
    uvwh: [f32; 4],
}

/// What a page shows: the member whose page it is and the slot under the cursor.
pub struct Look<'a> {
    pub member: &'a crate::field_menu::Member,
    pub item_images: &'a dyn Fn(u16) -> usize,
    /// The slot (0 weapon, 1 armour, 2 and 3 accessories) the cursor is on.
    pub slot: usize,
    /// Frames since the page's first.
    pub age: f32,
}

/// A node the game's code changes: shown or hidden outright, another picture or another text.
#[derive(Clone, Debug, Default)]
struct Over {
    show: bool,
    hide: bool,
    image_no: Option<usize>,
    text: Option<String>,
}

pub struct Page {
    styles: HashMap<String, Vec<Pic>>,
    /// The layer's textures after the fixed three: (install path, texture index in its file) and its size.
    wanted: Vec<(String, usize)>,
    sizes: Vec<(u32, u32)>,
    strings: HashMap<u32, String>,
    equip: Element,
    chara: Element,
    status: Element,
    glyphs: [HashMap<char, Glyph>; 2],
}

/// The style files the layouts' styles are in.
const STYLE_FILES: [&str; 9] = [
    "ui_cmn\\gen_styles\\uis_gen_a21_btl_equip.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_common.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_icon.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_window.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21v2_common.xml",
    "ui_cmn\\gen_styles\\uis_gen_a21_cabinet.xml",
    "ui_cmn\\etc\\uis_a21_item_l.xml",
    "ui_cmn\\etc\\uis_a21_chara_stand_card.xml",
    "ui_cmn\\etc\\uis_a21_bg.xml",
];

fn texture_path(texture: &str) -> String {
    format!("\\data\\x64\\res_cmn\\{}", texture.replace('/', "\\"))
}

impl Page {
    /// Reads the layouts and styles for `party` (the pictures and textures its members need).
    pub fn read(files: Files, party: &[crate::field_menu::Member], item_image: &dyn Fn(u16) -> usize) -> Result<Page, String> {
        let read = |p: &str| (files.read)(&format!("\\saves\\{p}"));
        let layout = |name: &str| -> Result<Element, String> { table::parse(&read(&format!("ui_en\\a21_main_menu\\{name}"))?).map_err(|e| format!("{name}: {e}")) };
        let (equip, chara, status) = (layout("uil_a21_menu_equip.xml")?, layout("uil_a21_menu_chara.xml")?, layout("uil_a21_menu_chara_status.xml")?);
        let mut styles: HashMap<String, Vec<Pic>> = HashMap::new();
        for f in STYLE_FILES {
            let root = table::parse(&read(f)?).map_err(|e| format!("{f}: {e}"))?;
            let pic = |e: &Element| -> Option<Pic> {
                let uvwh = numbers(e, "uvwh").try_into().ok()?;
                Some(Pic { path: texture_path(e.attr("texture")?), index: e.attr("texture_index")?.trim().parse().ok()?, uvwh })
            };
            for e in &root.children {
                let Some(name) = e.attr("name") else { continue };
                let list: Vec<Pic> = if e.name == "image_list" { e.children.iter().filter_map(pic).collect() } else { pic(e).into_iter().collect() };
                styles.insert(name.to_string(), list);
            }
        }
        // The textures: every style a layout names (a list's every picture, except the long lists of items and figures:
        // only the party's).
        let mut used = BTreeSet::new();
        for doc in [&equip, &chara, &status] {
            collect_styles(doc, &mut used);
        }
        let mut wanted: BTreeSet<(String, usize)> = BTreeSet::new();
        for name in &used {
            if name == "etc_a21_item_l" || name == "etc_a21_chara_stand_card" {
                continue;
            }
            for p in styles.get(name).into_iter().flatten() {
                wanted.insert((p.path.clone(), p.index));
            }
        }
        for m in party {
            if let Some(p) = styles.get("etc_a21_chara_stand_card").and_then(|l| l.get(usize::from(m.id))) {
                wanted.insert((p.path.clone(), p.index));
            }
            for w in m.worn.iter().flatten() {
                if let Some(p) = styles.get("etc_a21_item_l").and_then(|l| l.get(item_image(w.item))) {
                    wanted.insert((p.path.clone(), p.index));
                }
            }
        }
        let wanted: Vec<(String, usize)> = wanted.into_iter().collect();
        let sizes = wanted
            .iter()
            .map(|(p, i)| {
                let bytes = (files.read)(p)?;
                let g1t = barrel_formats::g1t::G1t::parse(&bytes).map_err(|e| format!("{p}: {e}"))?;
                let t = g1t.textures.get(*i).ok_or(format!("{p}: no texture {i}"))?;
                Ok((t.width, t.height))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let strings_root = table::parse(&read("text_en\\strcombineall.xml")?).map_err(|e| format!("strcombineall: {e}"))?;
        let strings = strings_root
            .children
            .iter()
            .filter_map(|s| Some((s.attr("String_No")?.parse::<u32>().ok()?, s.attr("Text")?.to_string())))
            .filter(|(n, _)| (STR_UI_BASE..STR_UI_BASE + 1000).contains(n) || (262_148..262_188).contains(n))
            .collect();
        Ok(Page { styles, wanted, sizes, strings, equip, chara, status, glyphs: files.glyphs })
    }

    /// The layer's textures after the fonts and the white: (install path, texture index).
    pub fn wanted(&self) -> &[(String, usize)] {
        &self.wanted
    }

    /// The page's quads in draw order on a `screen` of pixels.
    pub fn quads(&self, look: &Look, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, out: Vec::new() };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };

        // The character's figure (`uil_a21_menu_chara.xml`'s `chara_image_1`, whose place the equip layout sets: `chara_image_pos`).
        let at = find_pos(&self.equip, "chara_image_pos").unwrap_or([0.0; 2]);
        let figure = Over { show: true, image_no: Some(usize::from(look.member.id)), ..Default::default() };
        if let Some(e) = find(&self.chara, "chara_image_1") {
            let xf = root.node(at, 0.0, [1.0; 2]);
            w.image(e, xf, &figure);
        }

        // The four slots (`equip/battle`, shown by the game's code), the cursor's on `look.slot`.
        let names = ["weapon", "armor", "accessory1", "accessory2"];
        if let Some(top) = find(&self.equip, "equip") {
            let slots = |path: &[&str]| -> Over {
                let (slot, part) = match path {
                    [.., s, p] => (names.iter().position(|n| n == s), *p),
                    _ => (None, ""),
                };
                let Some(slot) = slot else { return Over { show: path.last() == Some(&"battle"), ..Default::default() } };
                match part {
                    "hilight" => Over { show: slot == look.slot, ..Default::default() },
                    "item" => match look.member.worn[slot] {
                        Some(worn) => Over { image_no: Some((look.item_images)(worn.item)), ..Default::default() },
                        None => Over { hide: true, ..Default::default() },
                    },
                    _ => Over::default(),
                }
            };
            w.walk(top, root.node(pair(top, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &slots, &HashMap::new());
        }

        // The stat panel (`uil_a21_menu_chara_status.xml`'s `top`, shown by the game's code).
        if let Some(top) = find(&self.status, "top") {
            let s = look.member.stats;
            let name = self.strings.get(&(262_148 + u32::from(look.member.id))).cloned().unwrap_or_default();
            let values = [("lv", look.member.level.to_string()), ("hp", s.hp.to_string()), ("atk", s.attack.to_string()), ("def", s.defense.to_string()), ("spd", s.speed.to_string()), ("all", s.equip_quality.to_string()), ("dex", s.dexterity.to_string())];
            let panel = |path: &[&str]| -> Over {
                if path.len() == 1 {
                    return Over { show: true, ..Default::default() };
                }
                // Only the numbers and the labels, the name, the lines and the window.
                match path {
                    [.., "param", "lv", "value"] => Over { text: Some(look.member.level.to_string()), ..Default::default() },
                    [.., "hp", "parent", "current", "value"] => Over { text: Some(s.hp.to_string()), ..Default::default() },
                    [.., "param", "hp", "parent", "max", "now"] => Over { show: true, text: Some(s.hp.to_string()), ..Default::default() },
                    [.., "param", stat, "now"] => Over { text: values.iter().find(|(n, _)| n == stat).map(|(_, v)| v.clone()), ..Default::default() },
                    [.., "change"] | [.., "role"] | [.., "att_d"] | [.., "role_info"] | [.., "clone_targets"] | [.., "change_template"] | [.., "updown"] => Over { hide: true, ..Default::default() },
                    ["top", "status", "text"] => Over { text: Some(name.clone()), ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.walk(top, root.node(pair(top, "pos", [0.0; 2]), 0.0, [1.0; 2]), &mut Vec::new(), &panel, &HashMap::new());
        }
        w.out
    }

    /// A style's picture `n` as its slot in the layer and its place in the atlas as uv.
    fn picture(&self, style: &str, n: usize) -> Option<(usize, [f32; 4], [f32; 2])> {
        let list = self.styles.get(style)?;
        let p = list.get(n).or_else(|| list.first())?;
        let slot = self.wanted.iter().position(|(path, i)| *path == p.path && *i == p.index)?;
        let (tw, th) = self.sizes[slot];
        let (aw, ah) = (tw as f32 * STYLE_OVER_TEXTURE, th as f32 * STYLE_OVER_TEXTURE);
        let uv = [p.uvwh[0] / aw, p.uvwh[1] / ah, (p.uvwh[0] + p.uvwh[2]) / aw, (p.uvwh[1] + p.uvwh[3]) / ah];
        Some((FIXED + slot, uv, [p.uvwh[2] / PER_LAYOUT, p.uvwh[3] / PER_LAYOUT]))
    }

    /// The node a `copy` names: its first definition with children, in the equip or status layout.
    fn template(&self, name: &str) -> Option<&Element> {
        [&self.equip, &self.status].into_iter().find_map(|doc| find_with_children(doc, name))
    }

    fn string(&self, id: &str) -> Option<&str> {
        let n: u32 = id.strip_prefix("STR_UI_")?.parse().ok()?;
        self.strings.get(&(STR_UI_BASE + n)).map(String::as_str)
    }
}

fn collect_styles(e: &Element, out: &mut BTreeSet<String>) {
    if let Some(s) = e.attr("style") {
        out.insert(s.to_string());
    }
    e.children.iter().for_each(|c| collect_styles(c, out));
}

fn find<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    if e.attr("name") == Some(name) {
        return Some(e);
    }
    e.children.iter().find_map(|c| find(c, name))
}

fn find_pos(e: &Element, name: &str) -> Option<[f32; 2]> {
    find(e, name).map(|n| pair(n, "pos", [0.0; 2]))
}

/// A node's place on the screen: a 2 x 2 matrix (columns `a, b` and `c, d`) and a translation.
#[derive(Clone, Copy, Debug)]
struct Xf {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    x: f32,
    y: f32,
}

impl Xf {
    fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        [self.a * p[0] + self.c * p[1] + self.x, self.b * p[0] + self.d * p[1] + self.y]
    }

    /// A child at `pos` turned `degrees` clockwise and scaled, about its top left.
    fn node(&self, pos: [f32; 2], degrees: f32, scale: [f32; 2]) -> Xf {
        let (s, c) = degrees.to_radians().sin_cos();
        let (la, lb, lc, ld) = (c * scale[0], s * scale[0], -s * scale[1], c * scale[1]);
        let [x, y] = self.apply(pos);
        Xf { a: self.a * la + self.c * lb, b: self.b * la + self.d * lb, c: self.a * lc + self.c * ld, d: self.b * lc + self.d * ld, x, y }
    }
}

struct Walker<'a> {
    page: &'a Page,
    out: Vec<Quad>,
}

impl Walker<'_> {
    /// Draws `e` and what is under it. `path` is the names from the walk's start; `over` the game's changes by path;
    /// `commands` the `command` changes a clone's parent gave by node name.
    fn walk(&mut self, e: &Element, xf: Xf, path: &mut Vec<String>, over: &dyn Fn(&[&str]) -> Over, commands: &HashMap<String, Vec<Element>>) {
        let name = e.attr("name").unwrap_or("").to_string();
        path.push(name.clone());
        let names: Vec<&str> = path.iter().map(String::as_str).collect();
        let mut o = over(&names);
        if let Some(cs) = commands.get(&name) {
            for c in cs {
                if let Some(n) = c.attr("set_image_no").and_then(|v| v.trim().parse().ok()) {
                    o.image_no = Some(n);
                }
                if let Some(s) = c.attr("set_string_id") {
                    o.text = o.text.take().or_else(|| self.page.string(s).map(str::to_string));
                }
            }
        }
        let hidden = (e.attr("visible") == Some("false") && !o.show) || o.hide;
        if !hidden && e.name != "usuals" && e.name != "clone_targets" && name != "clone_targets" && name != "usuals" {
            // This node itself: its place from the node's own attributes (the start node's place is the caller's).
            let me = if path.len() == 1 { xf } else { xf.node(pair(e, "pos", [0.0; 2]), numbers(e, "rot").get(2).copied().unwrap_or(0.0), pair(e, "scale", [1.0; 2])) };
            match e.name.as_str() {
                "image" => self.image(e, me, &o),
                "text" => self.text(e, me, &o),
                _ => {}
            }
            // A copied node's children: the template's (found by name) and its own; the clone's `command`s change them.
            let mut children: Vec<&Element> = Vec::new();
            let mut cmds: HashMap<String, Vec<Element>> = commands.clone();
            let page = self.page;
            if let Some(template) = e.attr("clone_child_tree").and_then(|t| page.template(t)) {
                children.extend(template.children.iter());
            }
            for c in &e.children {
                if c.name == "command" {
                    if let Some(t) = c.attr("target_node") {
                        cmds.entry(t.to_string()).or_default().push(c.clone());
                    }
                } else {
                    children.push(c);
                }
            }
            for c in children {
                self.walk(c, me, path, over, &cmds);
            }
        }
        path.pop();
    }

    fn image(&mut self, e: &Element, xf: Xf, o: &Over) {
        let Some(style) = e.attr("style") else { return };
        let n = o.image_no.or_else(|| e.attr("image_no").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
        let Some((texture, uv, natural)) = self.page.picture(style, n) else { return };
        let size = {
            let wh = numbers(e, "size_wh");
            let (w, h) = (numbers(e, "size_w").first().copied(), numbers(e, "size_h").first().copied());
            [w.or(wh.first().copied()).unwrap_or(natural[0]), h.or(wh.get(1).copied()).unwrap_or(natural[1])]
        };
        let c = numbers(e, "color");
        let color = if c.len() >= 4 { [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, c[3] / 255.0] } else { [1.0; 4] };
        let corners = [xf.apply([0.0, 0.0]), xf.apply([size[0], 0.0]), xf.apply(size), xf.apply([0.0, size[1]])];
        let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        let upright = xf.b.abs() < 1e-6 && xf.c.abs() < 1e-6;
        self.out.push(Quad { turn: false, spin: (!upright).then_some(Spin::Corners(corners)), rect: [x0, y0, x1, y1], uv, color, texture });
    }

    fn text(&mut self, e: &Element, xf: Xf, o: &Over) {
        let string = o.text.clone().or_else(|| e.attr("string_id").and_then(|s| self.page.string(s)).map(str::to_string));
        let Some(string) = string else { return };
        let font = usize::from(e.attr("font") != Some("MainFont01"));
        let glyphs = &self.page.glyphs[font];
        let k = xf.a.hypot(xf.b);
        let size = e.attr("font_size").and_then(|v| v.trim().parse().ok()).unwrap_or(17.0);
        let s = size * k / ATLAS_SIZE;
        // STAND-IN: the glyphs' spacing, as `field_menu`'s text: 0.25 px of 1080 lines for each layout size above 12 times `word_space`.
        let spacing = e.attr("word_space").and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0) * 0.25 * (size - 12.0).max(0.0) * k / 1.5;
        let advance = |g: &Glyph| g.advance as f32 * s + spacing;
        let width: f32 = string.chars().filter_map(|c| glyphs.get(&c)).map(advance).sum();
        let [x, y] = xf.apply([0.0, 0.0]);
        let left = match e.attr("align") {
            Some("center") => x - width / 2.0,
            Some("right") => x - width,
            _ => x,
        };
        let top = y + TEXT_TOP * k;
        let color = {
            let c = numbers(e, "font_color");
            let c: [f32; 4] = if c.len() >= 4 {
                [c[0], c[1], c[2], c[3]]
            } else if e.attr("copy") == Some("TEXTCOLOR_BEIGE") {
                [255.0, 255.0, 228.0, 255.0]
            } else {
                BROWN
            };
            c.map(|v| v / 255.0)
        };
        let mut run = |dx: f32, dy: f32, color: [f32; 4]| {
            let mut pen = left + dx;
            for g in string.chars().filter_map(|c| glyphs.get(&c)) {
                let (gx, gy, gw, gh) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
                let (x0, y0) = (pen + g.x_offset as f32 * s, top + dy + g.y_offset as f32 * s);
                self.out.push(Quad { turn: false, spin: None, rect: [x0, y0, x0 + gw * s, y0 + gh * s], uv: [gx / 4096.0, gy / 2048.0, (gx + gw) / 4096.0, (gy + gh) / 2048.0], color, texture: if font == 0 { FONT01 } else { FONT00 } });
                pen += advance(g);
            }
        };
        // The outline: the text in `bold_color` at half its alpha at each of 3 x 3 offsets of `bold_size`, columns first.
        if let Some(bold) = e.attr("bold_size").and_then(|v| v.trim().parse::<f32>().ok()) {
            let b = numbers(e, "bold_color");
            let outline = if b.len() >= 4 { [b[0] / 255.0, b[1] / 255.0, b[2] / 255.0, b[3] / 255.0 * OUTLINE_ALPHA] } else { [0.0, 0.0, 0.0, OUTLINE_ALPHA] };
            for dx in [-1.0, 0.0, 1.0] {
                for dy in [-1.0, 0.0, 1.0] {
                    run(dx * bold * k, dy * bold * k, outline);
                }
            }
        }
        run(0.0, 0.0, color);
    }
}

fn find_with_children<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    if e.attr("name") == Some(name) && !e.children.is_empty() {
        return Some(e);
    }
    e.children.iter().find_map(|c| find_with_children(c, name))
}
