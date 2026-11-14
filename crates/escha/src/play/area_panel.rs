//! The area map's panels (the recording `ruins-entrance-lost-battle-60hz.mkv` at 0:45, on the
//! 1920 × 1080 screen): the region's name at the top right (layout 637, right-aligned, white on
//! a dark shadow), and the place's scroll under it: its name on the scroll's top, its
//! description (layout 641's first text), the "Gatherable Items" tab (layout 640, the game's
//! title for its "Items to gather") and ten ruled item lines (641's rules and texts). The game
//! places these layouts at run time: 637 at (1434, 61), 641 at (1450, 143), 640 at (1448, 305).
//! The description is drawn in the scroll's dark brown ink, not the layout's colour.
//!
//! The item lines are the sector's gatherables (`Gathering::field_items`), "????" for those
//! the player does not know yet (`Session::knows`).
//!
//! The scroll is part 0x92 cut 8 (462 × 750, `a15_common.g1t` picture 13, beside the tab's
//! and the rules' cuts 4 and 5), its top-left at (1446, 150) at its own size (found in the
//! recording by its roll; 641's part 0x280 is layout 640 nested, not a sheet).
//!
//! Not yet the game's: the name's placing on it, the "X →" button at the scroll's foot.

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::Element;

const REGION: u32 = 637;
/// The maps' header (layout 652, at the screen's top-left): the feather (part 0x92 cut 2),
/// layout 651 (the dark band, cut 11, and the gold line, cut 3) and the region's name (637,
/// drawn by its own code at [`REGION_AT`]).
const HEADER: u32 = 652;

/// The header's sprites, in the layout's order (the name over them).
fn header(pe: &barrel_formats::pe::Pe) -> Vec<Element> {
    barrel_escha::data::layout::placed(pe, HEADER).iter().filter(|p| p.node.part < 0x164).map(Element::stored).collect()
}
const TAB: u32 = 640;
const PANEL: u32 = 641;
const REGION_AT: [i32; 2] = [1434, 61];
const PANEL_AT: [i32; 2] = [1450, 143];
const TAB_AT: [i32; 2] = [1448, 305];
/// The scroll's sprite and its top-left, and the name's centre on its top.
const SCROLL_PART: i32 = 0x92;
const SCROLL_CUT: i32 = 8;
const SCROLL_AT: [i32; 2] = [1446, 150];
/// The scroll's own layout is 645: the scroll, 641 (the item panel) at its top-left, 646 (the
/// button) at (248, 678).
const NEXT_BUTTON: u32 = 646;
const NEXT_BUTTON_AT: [i32; 2] = [248, 678];
/// The pad buttons' sprites (cuts in pairs, as the font's `<IMnn>`: see `message::western`).
const BUTTON_ICONS: i32 = 0x41;
const NAME_AT: [i32; 2] = [1673, 158];
/// The scroll's ink: the layouts' text colour (58, 26, 8), drawn opaque (the recording's
/// darkest text pixels are (57, 27, 6); the layouts' alpha byte 0xe1 would lighten it).
const INK: [u8; 4] = [0xff, 0x3a, 0x1a, 0x08];
/// The description's widest line (screen pixels), as the game breaks "A sector of the /
/// enormous ruins.".
const DESCRIPTION_WIDTH: f32 = 330.0;
const LINE_STEP: i32 = 38;
/// The item lines' rule (641's part 0xbe cut 5) and how far below its node the game draws it.
const RULE: i32 = 0xbe;
const RULE_DROP: i32 = 7;

fn moved(p: &Placed, by: [i32; 2]) -> [i32; 2] {
    [p.at[0] + by[0], p.at[1] + by[1]]
}

/// The panels for place `name` (with `description`) in region `region`; `width_of` measures a
/// text at size `size` in screen pixels, for breaking the description into lines.
/// The maps' frame, under every panel: layout 649's dark red vignette (one corner sheet,
/// a15_common #25, mirrored to the four corners) and 650's diamond border.
pub const FRAME: [u32; 2] = [649, 650];

/// The world and area maps' frame (`FRAME`), as the layouts store it.
pub fn frame(pe: &barrel_formats::pe::Pe) -> Vec<Element> {
    FRAME.iter().flat_map(|&id| barrel_escha::data::layout::placed(pe, id)).map(|p| Element::stored(&p)).collect()
}

pub fn elements(
    pe: &barrel_formats::pe::Pe,
    region: &str,
    name: &str,
    description: &str,
    items: &[String],
    tier: usize,
    width_of: impl Fn(&str, f32) -> f32,
) -> Vec<Element> {
    let texts = |p: &Placed| barrel_escha::data::layout::text(pe, p);
    let mut out = Vec::new();
    out.extend(header(pe));
    for p in barrel_escha::data::layout::placed(pe, REGION) {
        if let Some(t) = texts(&p) {
            out.push(Element::Label(Text { at: moved(&p, REGION_AT), text: region.to_string(), ..t }));
        }
    }
    out.push(Element::Sprite { part: SCROLL_PART, cut: SCROLL_CUT, at: SCROLL_AT });
    // The button at the scroll's foot (layout 646 at (248, 678) in the scroll's layout 645:
    // the arrow plate and the button's icon).
    for p in barrel_escha::data::layout::placed(pe, NEXT_BUTTON) {
        let cut = if p.node.part == BUTTON_ICONS { crate::message::western(p.node.cut.max(0) as u32) as i32 } else { p.node.cut.max(0) };
        out.push(Element::Sprite { part: p.node.part, cut, at: [SCROLL_AT[0] + NEXT_BUTTON_AT[0] + p.at[0], SCROLL_AT[1] + NEXT_BUTTON_AT[1] + p.at[1]] });
    }
    out.push(Element::Label(Text { at: NAME_AT, size: 36.0, colour: INK, text: name.to_string(), kind: 22 }));
    let panel = barrel_escha::data::layout::placed(pe, PANEL);
    let mut first_text = true;
    let mut row = 0;
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
            // The gatherable items, one a line, in the scroll's ink.
            (0x164, Some(t)) if t.filled_in() => {
                if let Some(item) = items.get(row) {
                    out.push(Element::Label(Text { at, text: item.clone(), colour: INK, ..t }));
                }
                row += 1;
            }
            // "...ETC" past the tenth.
            (0x164, Some(t)) if items.len() > 10 => out.push(Element::Label(Text { at, colour: INK, ..t })),
            // The rules, 7 lower than their nodes (the recording: each rule's line 4.5 to 5
            // pixels lower at 720p, the texts over them where their nodes put them).
            (RULE, _) => out.push(Element::Sprite { part: RULE, cut: p.node.cut.max(0), at: [at[0], at[1] + RULE_DROP] }),
            // Its own tab (layout 640 stands in its place).
            (0x164, _) | (0x92, _) | (0x280, _) => {}
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    for p in barrel_escha::data::layout::placed(pe, TAB) {
        let at = moved(&p, TAB_AT);
        match texts(&p) {
            Some(t) if p.node.part == 0x164 => out.push(Element::Label(Text { at, text: "Gatherable Items".into(), ..t })),
            // The gathering level: cut 12 plus the points' tier (Lv1 to Lv4, MAX).
            _ if p.node.part == 0x69 => out.push(Element::Sprite { part: 0x69, cut: 12 + tier.min(4) as i32, at }),
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    out
}

/// The world map's panels (the recording `colseit-arrival-60hz.mkv`, 1:15 and 1:25): the
/// place's name at the top right (637); for a region, the scroll with its description (641's
/// first text) and the "Map" tab (640's bar, retitled, without the level icon) over its
/// sectors' list (layout 642's rows and rules at (1442, 228); the description 40 pixels higher
/// than on the area map's scroll); for the town, no scroll. Not yet
/// the game's: the list shows every sector, where the game leaves out those not open yet
/// (Sector Three at the user's 8/20).
pub fn world_elements(pe: &barrel_formats::pe::Pe, name: &str, description: &str, sectors: &[String], width_of: impl Fn(&str, f32) -> f32) -> Vec<Element> {
    let texts = |p: &Placed| barrel_escha::data::layout::text(pe, p);
    let mut out = Vec::new();
    out.extend(header(pe));
    for p in barrel_escha::data::layout::placed(pe, REGION) {
        if let Some(t) = texts(&p) {
            out.push(Element::Label(Text { at: moved(&p, REGION_AT), text: name.to_string(), ..t }));
        }
    }
    if description.is_empty() {
        return out;
    }
    out.push(Element::Sprite { part: SCROLL_PART, cut: SCROLL_CUT, at: SCROLL_AT });
    if let Some((p, t)) = barrel_escha::data::layout::placed(pe, PANEL).iter().find_map(|p| texts(p).filter(|t| t.filled_in()).map(|t| (p.clone(), t))) {
        let at = moved(&p, PANEL_AT);
        for (k, line) in lines(description.trim(), |s| width_of(s, t.size)).into_iter().enumerate() {
            out.push(Element::Label(Text { at: [at[0], at[1] - WORLD_DESCRIPTION_RISE + LINE_STEP * k as i32], text: line, colour: INK, ..t.clone() }));
        }
    }
    for p in barrel_escha::data::layout::placed(pe, TAB) {
        let at = moved(&p, MAP_TAB_AT);
        match (p.node.part, texts(&p)) {
            (0x164, Some(t)) => out.push(Element::Label(Text { at, text: "Map".into(), ..t })),
            (0x69, _) => {}
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    let mut row = 0;
    for p in barrel_escha::data::layout::placed(pe, LIST) {
        let at = moved(&p, LIST_AT);
        match (p.node.part, texts(&p)) {
            (0x164, Some(t)) if t.filled_in() && t.kind == 21 => {
                if let Some(name) = sectors.get(row) {
                    out.push(Element::Label(Text { at, text: name.clone(), colour: INK, ..t }));
                }
                row += 1;
            }
            (0xbe, _) => out.push(Element::Sprite { part: 0xbe, cut: p.node.cut.max(0), at }),
            _ => {}
        }
    }
    out
}

/// The world map's travel panel (layout 639: "Time", the trip's days, "Date", the arrival's
/// month / day), while the cursor stands on another place than the start, hung under the
/// cursor's mark: its origin is the mark's screen place plus [`TRAVEL_UNDER`] (the recording at
/// 1:18: Colseit's mark at (960, 615), the panel at (769, 642)). Its numbers are 639's digit nodes (kind 29: part 0xcf,
/// `a15_mix.g1t` texture 3, cut = digit, cut 12 the "/"), each number ending near its node:
/// measured on the recording at 1:18 / 1:25 ("4", "8 /24"), the days' and the month's last
/// digit stand 20 pixels left of their nodes, the day's 11 right of its node and the "/" 6
/// right of its. The labels are the recording's tan, not 639's blue. Not yet the game's: those
/// offsets are measured, not read from the panel's code.
pub fn travel(pe: &barrel_formats::pe::Pe, mark: [i32; 2], days: i32, month: u32, day: u32) -> Vec<Element> {
    let origin = [mark[0] + TRAVEL_UNDER[0], mark[1] + TRAVEL_UNDER[1]];
    let mut out = Vec::new();
    let mut numbers = 0;
    for p in barrel_escha::data::layout::placed(pe, TRAVEL) {
        let at = moved(&p, origin);
        match (p.node.part, barrel_escha::data::layout::text(pe, &p)) {
            (0x164, Some(t)) if !t.text.is_empty() => out.push(Element::Label(Text { at, colour: TRAVEL_LABEL, ..t })),
            (0x164, _) => {}
            (TRAVEL_DIGITS, _) if p.node.kind == 29 => {
                let (value, shift) = match numbers {
                    0 => (days.max(0) as u32, -20),
                    1 => (month, -20),
                    _ => (day, 11),
                };
                let digits: Vec<i32> = value.to_string().bytes().map(|b| i32::from(b - b'0')).collect();
                let start = at[0] + shift - TRAVEL_ADVANCE * (digits.len() as i32 - 1);
                for (k, d) in digits.into_iter().enumerate() {
                    out.push(Element::Sprite { part: TRAVEL_DIGITS, cut: d, at: [start + TRAVEL_ADVANCE * k as i32, at[1]] });
                }
                numbers += 1;
            }
            (TRAVEL_DIGITS, _) => out.push(Element::Sprite { part: TRAVEL_DIGITS, cut: p.node.cut.max(0), at: [at[0] + 6, at[1]] }),
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    out
}

/// The travel panel's labels, as the recording has them.
const TRAVEL_LABEL: [u8; 4] = [0xff, 0xde, 0xba, 0x70];
const TRAVEL: u32 = 639;
const TRAVEL_UNDER: [i32; 2] = [-191, 27];
const TRAVEL_DIGITS: i32 = 0xcf;
/// A digit's step (the recording's "24": 31 pixels).
const TRAVEL_ADVANCE: i32 = 31;
const LIST: u32 = 642;
const LIST_AT: [i32; 2] = [1442, 228];
/// The world scroll's description stands this much higher than the area map's.
const WORLD_DESCRIPTION_RISE: i32 = 40;
const MAP_TAB_AT: [i32; 2] = [1448, 276];

/// A neighbour's label: layout 623, the plate part 0x27 (kind 19, three cuts: the left end
/// with the body 192 wide, a 48-wide middle repeated, the 48-wide right end, 72 high) at a
/// fixed 352 wide (its text's node at 176, centred; both labels of the recording are 352
/// whatever their name's length), the name centred on it in its dark ink. The plate's centre
/// stands [`LABEL_ABOVE`] over the label's place `at` (its route's dot), [`LABEL_IN`] nearer
/// the screen's middle (where the player stands): the recording's two labels, -22 and +28.
/// The gilt ornament on its outer end, as layouts 633 (right: part 0x90 through 629 and 627
/// at (306 + 88, 0)) and 634 (left: part 0x91 through 630 and 628 at (82, 0), the plate at
/// (36, 32)) place it, turned a quarter clockwise about its node (layout kind 7), the plate 32
/// below the layout's top. Not yet the game's: the ornament's bob (its four cuts).
pub fn label(pe: &barrel_formats::pe::Pe, at: [i32; 2], name: &str, _width: f32) -> Vec<Element> {
    let inward = if at[0] < 960 { LABEL_IN } else { -LABEL_IN };
    let left = at[0] + inward - PLATE_WIDTH / 2;
    let top = at[1] - LABEL_ABOVE - PLATE_HEIGHT / 2;
    let mut out = vec![Element::Sprite { part: PLATE, cut: 0, at: [left, top] }];
    // The layout's top-left, then the ornament's node in it.
    let (x0, y0) = if inward > 0 { (left - 36, top - 32) } else { (left, top - 32) };
    let ornament = if inward > 0 { Element::Turned { part: 0x91, cut: 0, at: [x0 + 82, y0] } } else { Element::Turned { part: 0x90, cut: 0, at: [x0 + 306 + 88, y0] } };
    if let Some(m) = barrel_escha::data::layout::sprite(pe, PLATE, 1, [0, 0]) {
        let [x0, y0, x1, y1] = m.src.map(f32::from);
        let (mut x, end) = (left + PLATE_LEFT, left + PLATE_WIDTH - PLATE_END);
        while x < end {
            let w = (end - x).min(PLATE_END);
            out.push(Element::Slice { pack: m.texture.clone(), index: m.index, src: [x0, y0, x0 + (x1 - x0) * w as f32 / PLATE_END as f32, y1], rect: [x, top, x + w, top + PLATE_HEIGHT] });
            x += w;
        }
    }
    out.push(Element::Sprite { part: PLATE, cut: 2, at: [left + PLATE_WIDTH - PLATE_END, top] });
    for p in barrel_escha::data::layout::placed(pe, LABEL) {
        if let Some(t) = barrel_escha::data::layout::text(pe, &p) {
            out.push(Element::Label(Text { at: [left + p.at[0], top + p.at[1]], text: name.to_string(), ..t }));
        }
    }
    out.push(ornament);
    out
}

const LABEL: u32 = 623;
const PLATE: i32 = 0x27;
const PLATE_WIDTH: i32 = 352;
const PLATE_HEIGHT: i32 = 72;
const PLATE_LEFT: i32 = 192;
const PLATE_END: i32 = 48;
/// How far the plate's centre stands over its dot, and nearer the screen's middle.
const LABEL_ABOVE: i32 = 52;
const LABEL_IN: i32 = 25;

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
