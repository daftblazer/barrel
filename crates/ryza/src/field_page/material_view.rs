//! The synthesis's Add Materials material list: the Container-style grid of the ring's category (`uil_a21_cabinet_mix.xml` and its cell) with the
//! item panel under it (`uil_a21_mix_simple_item_info.xml`), which slide in from the left when a ring is chosen and out again when a material is.

use super::*;

/// The grid's columns (`uil_a21_cabinet_mix.xml`: seven cells in a row).
const COLUMNS: usize = 7;
/// The slide of the list and the panel: 40 layout pixels from the left, over `SLIDE_FRAMES` (`anm_common_slide_in_left`).
const SLIDE_FROM: [f32; 2] = [-40.0, 0.0];
/// The grid's slider (top and height in the bar's layout pixels).
// STAND-IN: the capture's slider (a thumb 358.5 pixels of 1080 lines high from the top of the track, 239 layout pixels: the cell rows' share is not read).
/// Where a cell's one element icon stands in the list of four (layout pixels: the middle of the row of four, 1.5 steps of 15.67).
// STAND-IN: fitted to the capture's icons (35.3 pixels of 1080 lines right of the first place's).
const ICON_AT: [f32; 2] = [23.5, 0.0];
const THUMB: [f32; 2] = [18.0, 239.0];

/// One unit in the list.
pub struct MaterialItem {
    /// The item's small picture (`etc_a21_item_s`).
    pub image: usize,
    /// The element whose icon the cell shows (the item's strongest: fire, ice, bolt, air).
    pub element: Option<usize>,
    /// The item's name string.
    pub name: u32,
}

/// What the panel under the list says of the item under the cursor.
pub struct MaterialInfo {
    /// The item's big picture (`etc_a21_item_l`).
    pub image: usize,
    pub quality: u16,
    pub elements: [bool; 4],
    pub elem_value: i32,
}

/// The material list as it is now.
pub struct MaterialsLook {
    pub items: Vec<MaterialItem>,
    pub cursor: usize,
    pub info: MaterialInfo,
    /// The frames the slide-in of the list has run (30 and up: in place); a list going out runs it backwards.
    pub slide: f32,
    /// The frames since the cursor's colours began.
    pub age: f32,
    /// The list is going out (A on a material): its cursor is gone.
    pub leaving: bool,
}

impl Page {
    /// The list and the panel under it.
    pub(super) fn material_list(&self, w: &mut Walker, root: &Xf, look: &MaterialsLook) {
        let slide = Slide::in_from(look.slide, SLIDE_FROM);
        let string = |n: u32| self.strings.get(&n).cloned();
        let row = |s: &str, name: &str| s.strip_prefix(name).and_then(|n| n.strip_prefix(':')).and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
        let (base, both, f) = cursor_colors(look.age + CURSOR_PHASE_LAG);
        let name = look.items.get(look.cursor).and_then(|i| string(i.name));
        let over = |path: &[&str]| -> Over {
            match path {
                ["default"] => Over { show: true, ..Default::default() },
                ["default", "tab_top", ..] => Over { hide: true, ..Default::default() },
                ["default", "scroll_bar"] => Over { thumb: Some(THUMB), ..Default::default() },
                ["default", "item_name"] => Over { text: name.clone(), ..Default::default() },
                ["default", "sheet", r, c, part, rest @ ..] => {
                    let idx = row(r, "row") * COLUMNS + row(c, "cell");
                    let item = look.items.get(idx);
                    match (*part, rest) {
                        ("item_image", []) if item.is_some() => Over { image_no: Some(item.map_or(0, |i| i.image)), show: true, ..Default::default() },
                        ("highlight", []) if idx == look.cursor && item.is_some() && !look.leaving => Over { show: true, paint: Some([base[0], base[1], base[2], 1.0]), ..Default::default() },
                        ("highlight", _) if idx == look.cursor && item.is_some() && !look.leaving => Over { paint: Some([both[0], both[1], both[2], (191.0 * f).round() / 255.0]), ..Default::default() },
                        ("cc_value", []) if item.is_some() => Over { show: true, text: Some("1".into()), ..Default::default() },
                        ("elem", rest) if item.is_some() => {
                            let own = item.and_then(|i| i.element);
                            match rest {
                                [] | ["list"] => Over::default(),
                                // (the one icon stands at the middle of the four places)
                                ["list", node] if own == Some(row(node, "elem_node")) => Over { at: Some(ICON_AT), ..Default::default() },
                                ["list", node, "on"] if own == Some(row(node, "elem_node")) => Over { show: true, ..Default::default() },
                                ["list", _] => Over::default(),
                                _ => Over { hide: true, ..Default::default() },
                            }
                        }
                        _ => Over { hide: true, ..Default::default() },
                    }
                }
                _ => Over::default(),
            }
        };
        if let Some(info) = find(&self.mix_docs.simple, "simple_info") {
            let at = pair(info, "pos", [0.0; 2]);
            let top = pair(find(&self.mix_docs.simple, "top").unwrap_or(info), "pos", [0.0; 2]);
            let item = &look.info;
            let over = |path: &[&str]| -> Over {
                match path {
                    ["simple_info"] => Over { show: true, ..Default::default() },
                    ["simple_info", "dummy", "item", "icon"] => Over { image_no: Some(item.image), ..Default::default() },
                    ["simple_info", "dummy", "quality", "value"] => Over { text: Some(item.quality.to_string()), ..Default::default() },
                    ["simple_info", "dummy", "element", "value"] => Over { text: Some(item.elem_value.to_string()), ..Default::default() },
                    ["simple_info", "dummy", "element", "list", node, part] => {
                        let k = node.strip_prefix("node_").and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
                        let lit = item.elements.get(k).copied().unwrap_or(false);
                        match (*part, lit) {
                            ("on", false) => Over { hide: true, ..Default::default() },
                            ("off", true) => Over { hide: true, ..Default::default() },
                            _ => Over::default(),
                        }
                    }
                    _ => Over::default(),
                }
            };
            w.slid(slide, |w| w.walk(info, root.node([top[0] + at[0], top[1] + at[1]], 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
        if let Some(top) = find(&self.mix_docs.cabinet, "default") {
            let at = pair(find(&self.mix_docs.cabinet, "top").unwrap_or(top), "pos", [0.0; 2]);
            w.slid(slide, |w| w.walk(top, root.node(at, 0.0, [1.0; 2]), &mut Vec::new(), &over, &HashMap::new()));
        }
    }
}
