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
}

/// What the Select Recipe screen shows.
pub struct RecipeLook {
    pub rows: Vec<RecipeRow>,
    /// The row under the cursor.
    pub cursor: usize,
    /// Frames since the screen's first.
    pub age: f32,
}

/// The list's place: the layout's `recipe_menu` (2, 108) and its `menu_top` (0, 41).
const MENU_AT: [f32; 2] = [2.0, 149.0];
/// The distance from one row to the next (the game's code places them; the capture's rows are 25.13 layout pixels apart: STAND-IN).
const ROW_STEP: f32 = 25.13;
/// The tab row's pictures in `gen_a21_mix01_choise` (All, Favourites and the three kinds).
const TAB_PICTURES: [usize; 5] = [0, 1, 6, 7, 8];
/// The title plate's string ("Select Recipe", `str_menu_define`).
const RECIPE_TITLE: u32 = 9_961_590;

impl Page {
    /// The Select Recipe screen's quads on a `screen` of pixels (the background and the help bar are other layers').
    pub fn recipe_quads(&self, look: &RecipeLook, screen: (u32, u32)) -> (Vec<Quad>, Vec<usize>, Vec<Clip>) {
        let k = screen.1 as f32 / LAYOUT_HEIGHT;
        let mut w = Walker { page: self, screen_k: k, out: Vec::new(), additive: Vec::new(), parent: [0.0; 2], card: true, ordinal: 0, mask: None, clips: Vec::new(), mix: true };
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
                        ["menu_item", "__uimenu_cursor_active"] if i == look.cursor => Over { show: true, paint: Some([base[0], base[1], base[2], 1.0]), ..Default::default() },
                        ["menu_item", "__uimenu_cursor_active", "add"] if i == look.cursor => Over { paint: Some([both[0], both[1], both[2], (191.0 * f).round() / 255.0]), ..Default::default() },
                        ["menu_item", "bookmark_bg"] => Over { show: true, ..Default::default() },
                        ["menu_item", "__uimenu_cursor_active", ..] | ["menu_item", "bookmark_bg", ..] | ["menu_item", "new_icon"] | ["menu_item", "r_icon"] => hide(),
                        ["menu_item", "name"] => Over { text: string(row.name), ..Default::default() },
                        ["menu_item", "count_bg", "count"] => Over { text: Some(row.count.to_string()), ..Default::default() },
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
        self.caption_plate(&mut w, &root, RECIPE_TITLE, Slide::NONE);
        (w.out, w.additive, w.clips)
    }
}
