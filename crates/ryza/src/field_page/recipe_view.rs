//! The synthesis's Select Recipe screen: the recipe list in its frame, the tab row and the title plate, from
//! `uil_a21_mix_recipe.xml` (the game's code builds the list from `template_menu_node`, so the menu is made here, row by row).

use super::*;

/// One row of the recipe list.
pub struct RecipeRow {
    /// The recipe's item name string (`NAME_BASE + item`).
    pub name: u32,
    /// How many of it the container could make now (the list's count).
    pub count: u32,
    /// The row's text is the colour of a recipe that cannot be made now.
    pub short: bool,
    /// What the panel on the right says of the recipe when the cursor is on the row.
    pub info: RecipeInfo,
}

/// The panel on the right of Select Recipe (`product_info`): the item the recipe makes and what it needs.
pub struct RecipeInfo {
    /// The item's big picture (`etc_a21_item_l`).
    pub image: usize,
    pub level: i32,
    /// The alchemy level the panel's second circle shows.
    pub alchemy: u32,
    /// Units one synthesis makes (shown for a material; an attack item shows its CP and range instead).
    pub make: u32,
    pub attack: bool,
    pub elem_value: i32,
    /// Fire, ice, bolt and air.
    pub elements: [bool; 4],
    /// The categories the recipe's rows ask for, by place in the category table (the first loop's rows), and whether the container has one.
    pub materials: Vec<(usize, bool)>,
    /// The units the first row takes (the quantity box's circles) and the times the recipe can be made (the box's number).
    pub put: usize,
    pub times: u32,
}

/// What the Select Recipe screen shows.
pub struct RecipeLook {
    pub rows: Vec<RecipeRow>,
    /// The row under the cursor (and whose panel shows).
    pub cursor: usize,
    /// The cursor's bar is drawn (the tutorial pages hide it).
    pub cursor_shown: bool,
    /// Frames since the screen's first.
    pub age: f32,
    /// The ring graph behind the screen, from the cursor's move to the second recipe (its bars go over it).
    pub graph: Option<FieldLook>,
    /// The tutorial page up over the screen.
    pub tutorial: Option<Tutorial>,
    /// The frames since the screen began to come in (its parts slide in over 30: the list from the left, the panel from the right, the box from below, the title from above).
    pub opening: Option<f32>,
}

/// A tutorial page: its number (0 to 4), the frames since the box came up and the arrow's clock.
pub struct Tutorial {
    pub page: usize,
    pub age: f32,
    pub clock: f32,
}

/// The list's place: the layout's `recipe_menu` (2, 108) and its `menu_top` (0, 41).
const MENU_AT: [f32; 2] = [2.0, 149.0];
/// The distance from one row to the next (the game's code places them; the capture's rows are 25.13 layout pixels apart: STAND-IN).
const ROW_STEP: f32 = 25.13;
/// The item's picture in the panel is this colour when the recipe cannot be made now (the capture's, `synth-atelier` 2700).
const SHORT_ICON: [f32; 4] = [0.549, 0.443, 0.349, 1.0];
/// STAND-IN: the attack recipe's CP and its range ("All"), the capture's Craft at 2700; the table they come from is not read.
const CP_VALUE: u32 = 2;
/// The name and the count of a recipe that cannot be made now (the capture's, `synth-atelier` 2700).
const SHORT_TEXT: [f32; 4] = [1.0, 1.0 / 3.0, 1.0 / 3.0, 1.0];
/// The tab row's pictures in `gen_a21_mix01_choise` (All, Favourites and the three kinds).
const TAB_PICTURES: [usize; 5] = [0, 1, 6, 7, 8];
/// The title plate's string ("Select Recipe", `str_menu_define`).
const RECIPE_TITLE: u32 = 9_961_590;

impl Page {
    /// The Select Recipe screen's quads on a `screen` of pixels (the background and the help bar are other layers').
    pub fn recipe_quads(&self, look: &RecipeLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: true, bold_scale: 1.0, run_base: 0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        if let Some(graph) = &look.graph {
            self.ring_graph(&mut w, &root, graph);
            w.cull_outside(screen);
            self.bars(&mut w, &root);
        }
        let slide = |from: [f32; 2]| look.opening.map_or(Slide::NONE, |age| Slide::in_from(age, from));
        w.slid(slide([-40.0, 0.0]), |w| self.recipe_list(w, &root, look));
        // The panel on the right.
        if let Some(row) = look.rows.get(look.cursor) {
            w.slid(slide([40.0, 0.0]), |w| self.product_panel(w, &root, row, None));
        }
        // The quantity box at the bottom (`remain_put_num` of the caption layout): a circle for each unit the first row takes, on the right.
        if let Some(row) = look.rows.get(look.cursor) {
            w.slid(slide([0.0, 40.0]), |w| self.quantity_box(w, &root, &row.info, 0, 0.0));
        }
        self.caption_plate(&mut w, &root, RECIPE_TITLE, slide([0.0, -30.0]));
        if let Some(t) = &look.tutorial {
            self.tutorial_box(&mut w, &root, t);
        }
        (w.out, w.additive, w.clips)
    }
}

/// What the Add Materials screen's panel adds to Select Recipe's (`create_info`: the quality gauge and the effects it gives).
pub struct AddPanel {
    /// The quality the placed materials give.
    pub quality: u32,
    /// The quality the gauge fills to: the placed materials' (the number also previews the material under the list's cursor).
    pub gauge: u32,
    /// The page of the effects list and the number of pages.
    pub page: (u32, u32),
    /// The name each of the three effect rows shows (green: a ring's tier reached), none for an empty row.
    pub effects: [Option<String>; 3],
}

/// The colour of a reached effect's name (the capture's bytes: 0, 135, 10).
const REACHED: [f32; 4] = [0.0, 135.0 / 255.0, 10.0 / 255.0, 1.0];

impl Page {
    /// The panel on the right (`product_info`) of Select Recipe (`add`: None) or of Add Materials, whose `create_info` takes the place of the list of materials.
    pub(super) fn product_panel(&self, w: &mut Walker, root: &Xf, row: &RecipeRow, add: Option<&AddPanel>) {
        let string = |n: u32| self.strings.get(&n).cloned();
        let hide = || Over { hide: true, ..Default::default() };
        if let Some(panel) = find(&self.mix_docs.recipe, "product_info") {
            let info = &row.info;
            let at = pair(panel, "pos", [0.0; 2]);
            let n = |s: &str, p: &str| s.strip_prefix(p).and_then(|k| k.parse::<usize>().ok()).unwrap_or(0);
            let over = |p: &[&str]| -> Over {
                match p {
                    ["product_info"] => Over { show: true, ..Default::default() },
                    ["product_info", "mix_mat_info"] if add.is_none() => Over { show: true, ..Default::default() },
                    ["product_info", "create_info"] | ["product_info", "create_info", "frame"] | ["product_info", "create_info", "quality"] if add.is_some() => Over { show: true, ..Default::default() },
                    ["product_info", "create_info", "mix_eff"] if add.is_some() => Over { show: true, ..Default::default() },
                    ["product_info", "create_info", "mix_eff", "list", node, rest @ ..] => {
                        let k = n(node, "node_");
                        match (k < 3, rest) {
                            (false, _) => hide(),
                            (true, ["cap_text"]) => Over { text: string(EFFECT_CAPS[k]), ..Default::default() },
                            (true, ["name"]) => Over { text: Some(add.and_then(|a| a.effects[k].clone()).unwrap_or_default()), tint: Some(REACHED), ..Default::default() },
                            _ => Over::default(),
                        }
                    }
                    ["product_info", "create_info", "quality", "gauge", gauge] => {
                        // (each gauge holds a hundred points; the first and the last are drawn even when empty)
                        let q = add.map_or(0.0, |a| a.gauge as f32);
                        let share = |from: f32| ((q - from) / 100.0).clamp(0.0, 1.0);
                        match *gauge {
                            "bar_body" => Over { crop: Some(share(0.0)), ..Default::default() },
                            "bar_body_1" if q > 100.0 => Over { crop: Some(share(100.0)), ..Default::default() },
                            "bar_body_2" if q > 200.0 => Over { crop: Some(share(200.0)), ..Default::default() },
                            "bar_body_3" => Over { crop: Some(share(300.0)), ..Default::default() },
                            "bar_body_1" | "bar_body_2" => hide(),
                            _ => Over::default(),
                        }
                    }
                    ["product_info", "create_info", "quality", "value"] => Over { text: add.map(|a| a.quality.to_string()), ..Default::default() },
                    ["product_info", "create_info", "page_num", "now_page"] => Over { text: add.map(|a| a.page.0.to_string()), ..Default::default() },
                    ["product_info", "create_info", "page_num", "max_page"] => Over { text: add.map(|a| a.page.1.to_string()), ..Default::default() },
                    ["product_info", "create_info", ..] if add.is_none() => hide(),
                    ["product_info", "common_info", "cp_and_range"] if info.attack && add.is_none() => Over { show: true, ..Default::default() },
                    ["product_info", "common_info", "mix_make_num"] if !info.attack || add.is_some() => Over { show: true, ..Default::default() },
                    ["product_info", "mix_mat_info", "list", node, rest @ ..] => {
                        let k = n(node, "node_");
                        match (info.materials.get(k), rest) {
                            (None, _) => hide(),
                            (Some(_), ["item_icon"]) => hide(),
                            (Some(&(c, _)), ["cat_icon"]) => Over { image_no: Some(c), ..Default::default() },
                            (Some(&(c, have)), ["name"]) => Over { text: string(CATEGORY_BASE + c as u32), tint: (!have).then_some(SHORT_TEXT), ..Default::default() },
                            _ => Over::default(),
                        }
                    }
                    ["product_info", "common_info", "alc_lv", "dummy", "value"] => Over { text: Some(info.alchemy.to_string()), ..Default::default() },
                    ["product_info", "common_info", "lv", "dummy", "value"] => Over { text: Some(info.level.to_string()), tint: Some([1.0; 4]), ..Default::default() },
                    ["product_info", "common_info", "cp_and_range", ":1", ":1"] => Over { tint: Some([1.0; 4]), ..Default::default() },
                    ["product_info", "common_info", "mix_make_num", "dummy", "value"] => Over { text: Some(info.make.to_string()), ..Default::default() },
                    ["product_info", "common_info", "cp_and_range", "range_type"] => Over { text: string(ALL_WORD), ..Default::default() },
                    ["product_info", "common_info", "cp_and_range", _, "dummy", "cp_value"] => Over { text: Some(CP_VALUE.to_string()), ..Default::default() },
                    ["product_info", "common_info", "element", "dummy", "value"] => Over { text: Some(info.elem_value.to_string()), ..Default::default() },
                    ["product_info", "common_info", "element", "list", node, part, ..] => {
                        let lit = info.elements.get(n(node, "node_")).copied().unwrap_or(false);
                        match (*part, lit) {
                            ("off", true) | ("dummy", false) => hide(),
                            _ => Over::default(),
                        }
                    }
                    ["product_info", "common_info", "item", "icon"] => Over { image_no: Some(info.image), paint: row.short.then_some(SHORT_ICON), ..Default::default() },
                    ["product_info", "warning"] | ["product_info", "warning", "lack_mix_lv"] if row.short && add.is_none() => Over { show: true, ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.walk(panel, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
        }
    }
}

/// The effect rows' captions ("Effect 1" to "Effect 3", `str_ui`: STR_UI_0351 and the three after the second).
const EFFECT_CAPS: [u32; 3] = [6_619_488, 6_619_491, 6_619_492];

impl Page {
    /// The box at the bottom of the synthesis screens (`remain_put_num` of the caption layout): a circle for each unit the first row takes, on the right, and the number of times.
    pub(super) fn quantity_box(&self, w: &mut Walker, root: &Xf, info: &RecipeInfo, lit: usize, shift: f32) {
        let hide = || Over { hide: true, ..Default::default() };
        if let Some(box_) = find(&self.mix_docs.caption, "remain_put_num") {
            let at = pair(box_, "pos", [0.0; 2]);
            let at = [at[0] + shift, at[1]];
            let put = info.put.min(4);
            let over = |p: &[&str]| -> Over {
                match p {
                    ["remain_put_num"] => Over { show: true, ..Default::default() },
                    ["remain_put_num", "list", node, ..] if p.len() == 3 => {
                        let k = node.strip_prefix("node:").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
                        // (the circles stand on the right of the row; the game moves each: the first stands at 112 / (the number shown), then 28 across and a quarter down for each)
                        if k + put >= 4 && k <= 3 { Over { at: Some([112.0 / put as f32 + 28.0 * (k + put - 4) as f32, 0.25 * (k + put - 4) as f32]), ..Default::default() } } else { hide() }
                    }
                    ["remain_put_num", "list", node, "light"] => {
                        let k = node.strip_prefix("node:").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
                        if k + put >= 4 && k + put - 4 < lit { Over { show: true, ..Default::default() } } else { hide() }
                    }
                    ["remain_put_num", "num"] => Over { text: Some(info.times.to_string()), ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.walk(box_, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
        }
    }
}

impl Page {
    /// The recipe list: its frame and rows with the cursor, the tab row and the page's name.
    pub(super) fn recipe_list(&self, w: &mut Walker, root: &Xf, look: &RecipeLook) {
        let docs = &self.mix_docs;
        let string = |n: u32| self.strings.get(&n).cloned();
        let hide = || Over { hide: true, ..Default::default() };
        let (base, both, f) = cursor_colors(look.age);
        // The list's frame and rows.
        let Some(menu) = find(&docs.recipe, "template_menu_node") else { return };
        if let Some(frame) = find(menu, "frame") {
            let at = pair(frame, "pos", [0.0; 2]);
            w.walk(frame, root.node([MENU_AT[0] + at[0], MENU_AT[1] + at[1]], 0.0, [1.0; 2]), &mut Vec::new(), &|_: &[&str]| Over::default(), &HashMap::new());
        }
        if let (Some(contents), Some(item)) = (find(menu, "contents"), find(menu, "menu_item")) {
            let at = pair(contents, "pos", [0.0; 2]);
            for (i, row) in look.rows.iter().enumerate() {
                let over = |p: &[&str]| -> Over {
                    match p {
                        ["menu_item"] => Over { show: true, ..Default::default() },
                        ["menu_item", "__uimenu_cursor_active"] if i == look.cursor && look.cursor_shown => Over { show: true, paint: Some([base[0], base[1], base[2], 1.0]), ..Default::default() },
                        ["menu_item", "__uimenu_cursor_active", "add"] if i == look.cursor && look.cursor_shown => Over { paint: Some([both[0], both[1], both[2], (191.0 * f).round() / 255.0]), ..Default::default() },
                        ["menu_item", "bookmark_bg"] => Over { show: true, ..Default::default() },
                        ["menu_item", "__uimenu_cursor_active", ..] | ["menu_item", "bookmark_bg", ..] | ["menu_item", "new_icon"] | ["menu_item", "r_icon"] => hide(),
                        ["menu_item", "name"] => Over { text: string(row.name), tint: row.short.then_some(SHORT_TEXT), ..Default::default() },
                        ["menu_item", "count_bg", "count"] => Over { text: Some(row.count.to_string()), tint: Some([1.0; 4]), ..Default::default() },
                        _ => Over::default(),
                    }
                };
                w.walk(item, root.node([MENU_AT[0] + at[0], MENU_AT[1] + at[1] + ROW_STEP * i as f32], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
            }
        }
        // The tab row and the page's name.
        if let Some(menu) = find(&docs.recipe, "recipe_menu") {
            let at = pair(menu, "pos", [0.0; 2]);
            if let Some(tab) = find(menu, "tab") {
                let t = pair(tab, "pos", [0.0; 2]);
                let over = |p: &[&str]| -> Over {
                    match p {
                        ["tab"] => Over { show: true, ..Default::default() },
                        ["tab", "tab_frame", "l1", ..] | ["tab", "tab_frame", "r1", ..] | ["tab", "tabon", ..] => hide(),
                        ["tab", "tab_frame", "select_L"] => Over { text: Some(TAB_LEFT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                        ["tab", "tab_frame", "select_R"] => Over { text: Some(TAB_RIGHT_BUTTON.to_string()), size: Some(BUTTON_SIZE), color: Some([1.0; 4]), ..Default::default() },
                        ["tab", "taboff", "tab0", "hilight"] => Over { show: true, ..Default::default() },
                        ["tab", "taboff", t, "text"] => Over { image_no: Some(TAB_PICTURES[t.strip_prefix("tab").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0).min(4)]), natural: true, ..Default::default() },
                        _ => Over::default(),
                    }
                };
                w.walk(tab, root.node([at[0] + t[0], at[1] + t[1]], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
            }
            if let Some(name) = find(menu, "page_name") {
                let t = pair(name, "pos", [0.0; 2]);
                w.walk(name, root.node([at[0] + t[0], at[1] + t[1]], 0.0, [1.0; 2]), &mut Vec::new(), &|_: &[&str]| Over { text: string(ALL_WORD), ..Default::default() }, &HashMap::new());
            }
        }
    }
}

/// The tutorial pages' strings (`str_tutorial`: "Helpful Synthesis Features", then the four pages).
const TUTORIAL_STRINGS: [u32; 5] = [10_027_054, 10_027_055, 10_027_056, 10_027_057, 10_027_058];
/// The box's place (layout pixels): the layout's root is at (325, 401); the game stands it 31 higher (`synth-atelier` 2500: the panel's top at 761 of 1080 lines).
const BALLOON_AT: [f32; 2] = [325.0, 370.0];
/// The width the box's text wraps at (layout pixels). STAND-IN: fitted to the pages' breaks.
const BALLOON_WRAP: f32 = 520.0;
/// The words stand this much higher than the layout's `msg_text` (layout pixels; STAND-IN: the capture's first line is 19 pixels of 1080 lines above where the text routine puts it).
const TEXT_RAISE: f32 = 12.7;
/// The distance between the box's lines (layout pixels: 36 of 1080 lines).
const TEXT_STEP: f32 = 24.0;
/// The pad's buttons the tutorial texts name (`<KEY_ASSIGN_...>`), as the glyphs the font has for them (STAND-IN: the pad's assignment is not read; the capture's pages show these).
const BUTTONS: [(&str, char); 3] = [("<KEY_ASSIGN_MIX_AUTO>", '\u{4e7}'), ("<KEY_ASSIGN_MIX_MATERIAL_MULTI_SELECT>", '\u{4e7}'), ("<KEY_ASSIGN_BASIC_OK>", '\u{4da}')];
/// The arrow turns once in 240 frames (`anm_rotate`).
const ARROW_FRAMES: f32 = 240.0;

/// A tutorial text without its markup, and where each colour starts (`<#RRGGBB>`, `<CLNR>` back to the plain colour), by character.
fn markup(text: &str) -> (String, Vec<(usize, [f32; 4])>) {
    let (mut plain, mut runs) = (String::new(), Vec::new());
    let mut rest = text;
    'text: while !rest.is_empty() {
        if let Some(tag) = rest.strip_prefix("<#").and_then(|r| r.get(..7)).filter(|t| t.ends_with('>')) {
            let v = u32::from_str_radix(&tag[..6], 16).unwrap_or(0);
            runs.push((plain.chars().count(), [(v >> 16) as f32 / 255.0, (v >> 8 & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0, 1.0]));
            rest = &rest[9..];
            continue;
        }
        if let Some(r) = rest.strip_prefix("<CLNR>") {
            runs.push((plain.chars().count(), [26.0 / 255.0, 0.0, 0.0, 1.0]));
            rest = r;
            continue;
        }
        for (tag, glyph) in BUTTONS {
            if let Some(r) = rest.strip_prefix(tag) {
                plain.push(glyph);
                rest = r;
                continue 'text;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    (plain, runs)
}

/// A copy of `e` with the attribute `name` of every node called `node` set to `value`.
fn with_attr(e: &Element, node: &str, name: &str, value: &str) -> Element {
    let mut copy = e.clone();
    if copy.attr("name") == Some(node) {
        copy.attributes.retain(|(k, _)| k != name);
        copy.attributes.push((name.to_string(), value.to_string()));
    }
    copy.children = e.children.iter().map(|c| with_attr(c, node, name, value)).collect();
    copy
}

impl Page {
    /// The tutorial box (`uil_a21_balloon_frame_20x03.xml`'s `size_s`): the panel, the page's words and the turning arrow, rising and fading in over 30 frames.
    pub(super) fn tutorial_box(&self, w: &mut Walker, root: &Xf, t: &Tutorial) {
        let Some(size_s) = find(&self.mix_docs.balloon, "size_s") else { return };
        let Some(text) = TUTORIAL_STRINGS.get(t.page).and_then(|n| self.strings.get(n)) else { return };
        let (plain, runs) = markup(text);
        let size_s = with_attr(size_s, "msg_text", "limit_width", &BALLOON_WRAP.to_string());
        let origin = root.node(BALLOON_AT, 0.0, [1.0; 2]);
        // The lines stand 24 layout pixels apart, the block centred on a line of the box (a single line 12 pixels below the first of two).
        let lines = find(&size_s, "msg_text").map_or(1, |m| w.wrap(m, &origin, &Over::default(), &plain).len()).max(1);
        let first = 90.0 - TEXT_RAISE + TEXT_STEP / 2.0 * (2.0 - lines as f32);
        let over = |p: &[&str]| -> Over {
            match p {
                ["size_s"] => Over { show: true, ..Default::default() },
                ["size_s", "panel", "msg_text"] => Over { text: Some(plain.clone()), runs: runs.clone(), at: Some([86.0, first]), step: Some(TEXT_STEP), ..Default::default() },
                ["size_s", "panel", "name_base", ..] => Over { hide: true, ..Default::default() },
                ["size_s", "panel", "prompt", ..] => Over { hide: true, ..Default::default() },
                _ => Over::default(),
            }
        };
        w.slid(Slide::in_from(t.age, [0.0, 30.0]), |w| {
            w.walk(&size_s, origin, &mut Vec::new(), &over, &HashMap::new());
            // The arrow: its shadow, the two turning halves (turned the one way and the other) and the core, at the layout's `prompt`.
            let prompt = origin.node([-12.0 + 600.0, 137.0 + 100.0], 0.0, [1.0; 2]);
            let angle = (360.0 * t.clock / ARROW_FRAMES) % 360.0;
            let half = 100.0 / 3.0 / 2.0;
            let shadow = Element { name: "image".into(), attributes: vec![("style".into(), "alpha_circle".into()), ("size_wh".into(), "25.6, 25.6, 0".into())], children: Vec::new() };
            w.image(&shadow, prompt.node([16.4, 17.2], 49.4, [1.0; 2]).node([-12.8, -12.8], 0.0, [1.0; 2]), &Over { paint: Some([26.0 / 255.0, 0.0, 0.0, 60.0 / 255.0]), ..Default::default() });
            for (style, turn) in [("gen_a21_baloon_frame01_ic_prop02", angle), ("gen_a21_baloon_frame01_ic_prop", -angle), ("gen_a21_baloon_frame01_ic_core", 0.0)] {
                let part = Element { name: "image".into(), attributes: vec![("style".into(), style.into())], children: Vec::new() };
                w.image(&part, prompt.node([half, half], turn, [1.0; 2]).node([-half, -half], 0.0, [1.0; 2]), &Over::default());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorials_markup_gives_the_plain_text_and_where_each_colour_starts() {
        let (plain, runs) = markup("Use <#006e5a>Auto-Add<CLNR> or press <KEY_ASSIGN_BASIC_OK>.");
        assert_eq!(plain, "Use Auto-Add or press \u{4da}.");
        assert_eq!(runs.iter().map(|r| r.0).collect::<Vec<_>>(), [4, 12]);
        assert_eq!(runs[0].1, [0.0, 110.0 / 255.0, 90.0 / 255.0, 1.0]);
    }

    #[test]
    fn a_nodes_attribute_is_set_by_its_name_anywhere_below() {
        let child = Element { name: "text".into(), attributes: vec![("name".into(), "msg_text".into())], children: Vec::new() };
        let root = Element { name: "node".into(), attributes: vec![("name".into(), "size_s".into())], children: vec![child] };
        let set = with_attr(&root, "msg_text", "limit_width", "520");
        assert_eq!(set.children[0].attr("limit_width"), Some("520"));
        assert_eq!(set.attr("limit_width"), None);
    }
}
