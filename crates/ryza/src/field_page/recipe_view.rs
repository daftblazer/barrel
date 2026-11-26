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
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: true, bold_scale: 1.0 };
        let root = Xf { a: k, b: 0.0, c: 0.0, d: k, x: (screen.0 as f32 - 1280.0 * k) / 2.0, y: 0.0 };
        let docs = &self.mix_docs;
        let string = |n: u32| self.strings.get(&n).cloned();
        let hide = || Over { hide: true, ..Default::default() };
        let (base, both, f) = cursor_colors(look.age);
        // The list's frame and rows.
        let Some(menu) = find(&docs.recipe, "template_menu_node") else { return (w.out, w.additive, w.clips) };
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
        // The panel on the right.
        if let Some(row) = look.rows.get(look.cursor) {
            self.product_panel(&mut w, &root, row, None);
        }
        // The quantity box at the bottom (`remain_put_num` of the caption layout): a circle for each unit the first row takes, on the right.
        if let Some(row) = look.rows.get(look.cursor) {
            self.quantity_box(&mut w, &root, &row.info);
        }
        self.caption_plate(&mut w, &root, RECIPE_TITLE, Slide::NONE);
        (w.out, w.additive, w.clips)
    }
}

/// What the Add Materials screen's panel adds to Select Recipe's (`create_info`: the quality gauge and the effects it gives).
pub struct AddPanel {
    /// The quality the placed materials give.
    pub quality: u32,
    /// The page of the effects list and the number of pages.
    pub page: (u32, u32),
}

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
                            (true, ["name"]) => Over { text: Some(String::new()), ..Default::default() },
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
    pub(super) fn quantity_box(&self, w: &mut Walker, root: &Xf, info: &RecipeInfo) {
        let hide = || Over { hide: true, ..Default::default() };
        if let Some(box_) = find(&self.mix_docs.caption, "remain_put_num") {
            let at = pair(box_, "pos", [0.0; 2]);
            let put = info.put.min(4);
            let over = |p: &[&str]| -> Over {
                match p {
                    ["remain_put_num"] => Over { show: true, ..Default::default() },
                    ["remain_put_num", "list", node, ..] if p.len() == 3 => {
                        let k = node.strip_prefix("node:").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
                        // (the circles stand on the right of the row; the game moves each: the first stands at 112 / (the number shown), then 28 across and a quarter down for each)
                        if k + put >= 4 && k <= 3 { Over { at: Some([112.0 / put as f32 + 28.0 * (k + put - 4) as f32, 0.25 * (k + put - 4) as f32]), ..Default::default() } } else { hide() }
                    }
                    ["remain_put_num", "list", _, "light"] => hide(),
                    ["remain_put_num", "num"] => Over { text: Some(info.times.to_string()), ..Default::default() },
                    _ => Over::default(),
                }
            };
            w.walk(box_, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new());
        }
    }
}
