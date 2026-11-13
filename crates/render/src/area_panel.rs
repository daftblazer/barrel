//! The area map's panels (the recording `ruins-entrance-lost-battle-60hz.mkv` at 0:45, on the
//! 1920 × 1080 screen): the region's name at the top right (layout 637, right-aligned, white on
//! a dark shadow), and the place's scroll under it: its name on the scroll's top, its
//! description (layout 641's first text), the "Gatherable Items" tab (layout 640, the game's
//! title for its "Items to gather") and ten ruled item lines (641's rules and texts). The game
//! places these layouts at run time: 637 at (1434, 61), 641 at (1450, 143), 640 at (1448, 305).
//! The description is drawn in the scroll's dark brown ink, not the layout's colour.
//!
//! Not yet the game's: the scroll itself (a flat parchment fill: 641's part 0x280 has no
//! sheet), the name's placing on it, the item lines (empty: the sector's gatherables are not
//! listed yet), the "X →" button at the scroll's foot.

use barrel_formats::layout::{Placed, Text};

use crate::hud::Element;

const REGION: u32 = 637;
const TAB: u32 = 640;
const PANEL: u32 = 641;
const REGION_AT: [i32; 2] = [1434, 61];
const PANEL_AT: [i32; 2] = [1450, 143];
const TAB_AT: [i32; 2] = [1448, 305];
/// The scroll's stand-in (left, top, right, bottom) and the name's centre on its top.
const SCROLL: [i32; 4] = [1464, 160, 1868, 872];
const NAME_AT: [i32; 2] = [1673, 158];
const INK: [u8; 4] = [0xe1, 0x3a, 0x2c, 0x1e];
/// The description's widest line (screen pixels), as the game breaks "A sector of the /
/// enormous ruins.".
const DESCRIPTION_WIDTH: f32 = 330.0;
const LINE_STEP: i32 = 38;

fn moved(p: &Placed, by: [i32; 2]) -> [i32; 2] {
    [p.at[0] + by[0], p.at[1] + by[1]]
}

/// The panels for place `name` (with `description`) in region `region`; `width_of` measures a
/// text at size `size` in screen pixels, for breaking the description into lines.
pub fn elements(
    pe: &barrel_formats::pe::Pe,
    region: &str,
    name: &str,
    description: &str,
    width_of: impl Fn(&str, f32) -> f32,
) -> Vec<Element> {
    let texts = |p: &Placed| barrel_formats::layout::text(pe, p);
    let mut out = Vec::new();
    for p in barrel_formats::layout::placed(pe, REGION) {
        if let Some(t) = texts(&p) {
            out.push(Element::Label(Text { at: moved(&p, REGION_AT), text: region.to_string(), ..t }));
        }
    }
    out.push(Element::Fill { rect: SCROLL, colour: [0.92, 0.87, 0.72, 0.96] });
    out.push(Element::Label(Text { at: NAME_AT, size: 36.0, colour: INK, text: name.to_string(), kind: 22 }));
    let panel = barrel_formats::layout::placed(pe, PANEL);
    let mut first_text = true;
    for p in &panel {
        let at = moved(p, PANEL_AT);
        match (p.node.part, texts(p)) {
            // The description in the first text's place, broken into lines.
            (0x164, Some(t)) if first_text && t.filled_in() => {
                first_text = false;
                for (k, line) in lines(description.trim(), |s| width_of(s, t.size)).into_iter().enumerate() {
                    out.push(Element::Label(Text { at: [at[0], at[1] + LINE_STEP * k as i32], text: line, colour: INK, ..t.clone() }));
                }
            }
            // Its own tab (layout 640 stands in its place), the item lines' texts (no list
            // yet) and the "...ETC" past the tenth.
            (0x164, _) | (0x92, _) | (0x280, _) => {}
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    for p in barrel_formats::layout::placed(pe, TAB) {
        let at = moved(&p, TAB_AT);
        match texts(&p) {
            Some(t) if p.node.part == 0x164 => out.push(Element::Label(Text { at, text: "Gatherable Items".into(), ..t })),
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    out
}

/// A neighbour's label (layout 638: its text centred), on a plate whose centre stands
/// [`LABEL_ABOVE`] over the label's place `at` (its route's dot).
/// Not yet the game's: the plate (a parchment fill; the game's has gilt ends) and the ink
/// (the layout's texts are white over a shadow; the game's label reads dark on the plate).
pub fn label(pe: &barrel_formats::pe::Pe, at: [i32; 2], name: &str, width: f32) -> Vec<Element> {
    let at = [at[0], at[1] - LABEL_ABOVE];
    let half = (width / 2.0) as i32 + 28;
    let mut out = vec![Element::Fill { rect: [at[0] - half, at[1] - 22, at[0] + half, at[1] + 24], colour: [0.93, 0.88, 0.74, 0.95] }];
    // The layout's white text, at its node less the node's own centre (320, -4).
    if let Some(t) = barrel_formats::layout::placed(pe, LABEL).iter().filter_map(|p| barrel_formats::layout::text(pe, p)).find(|t| t.colour[0] == 0) {
        out.push(Element::Label(Text { at: [at[0], at[1] - LABEL_RISE], text: name.to_string(), colour: INK, ..t }));
    }
    out
}

const LABEL: u32 = 638;
/// How far the label's centre stands over its dot (the recording: 48 pixels).
const LABEL_ABOVE: i32 = 48;
/// The label's text top above its centre.
const LABEL_RISE: i32 = 20;

/// `text` broken at spaces into lines no wider than [`DESCRIPTION_WIDTH`].
fn lines(text: &str, width: impl Fn(&str) -> f32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match out.last_mut() {
            Some(line) if width(&format!("{line} {word}")) <= DESCRIPTION_WIDTH => {
                line.push(' ');
                line.push_str(word);
            }
            _ => out.push(word.to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn descriptions_break_at_spaces() {
        // 15 pixels a letter: "A sector of the" is 225 wide, adding "enormous" makes 360.
        let w = |s: &str| s.len() as f32 * 15.0;
        assert_eq!(super::lines("A sector of the enormous ruins.", w), ["A sector of the", "enormous ruins."]);
    }
}
