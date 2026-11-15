//! The area map's panels (the recording `ruins-entrance-lost-battle-60hz.mkv` at 0:45, on the
//! 1920 × 1080 screen): the region's name at the top right (layout 637, right-aligned, white on
//! a dark shadow), and the place's scroll under it: its name on the scroll's top, its
//! description (layout 641's first text), the "Gatherable Items" tab (layout 640, the game's
//! title for its "Items to gather") and ten ruled item lines (641's rules and texts). The game
//! draws them as one layout, 645 (the scroll with 641, 640 and the button 646 nested), from the
//! scroll's top-left (1446, 150), and the region's name (637) at (1440, 59); this matches the
//! game's own draws of the area map (docs/escha/parity.md).
//! The description is drawn in the scroll's dark brown ink, not the layout's colour.
//!
//! The item lines are the sector's gatherables (`Gathering::field_items`), "????" for those
//! the player does not know yet (`Session::knows`).
//!
//! The scroll is part 0x92 cut 8 (462 × 750, `a15_common.g1t` picture 13, beside the tab's
//! and the rules' cuts 4 and 5), its top-left at (1446, 150) at its own size (found in the
//! recording by its roll; 641's part 0x280 is layout 640 nested, not a sheet).

use barrel_escha::data::layout::{Placed, Text};

use crate::play::hud::Element;

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
/// (Refitted 2026-11-14 by patch search on both maps' recordings: 6 right and 1 up of the first
/// measure.)
const REGION_AT: [i32; 2] = [1440, 60];
/// The area map's region name, as the game's capture of the area map has it (frame 3000 of
/// the load-field scenario; 1 higher than the world map's recordings).
const AREA_REGION_AT: [i32; 2] = [1440, 59];
/// The world map's item panel (fitted to its recordings), where the area map's own
/// scroll layout [`SCROLL`] stands at [`SCROLL_AT`].
const PANEL_AT: [i32; 2] = [1450, 143];
/// The scroll's sprite and its top-left, and the name's centre on its top.
const SCROLL_PART: i32 = 0x92;
const SCROLL_CUT: i32 = 8;
const SCROLL_AT: [i32; 2] = [1446, 150];
/// The area map's scroll layout: the scroll (part 0x92 cut 8), the item panel 641 with the tab
/// 640 nested in it, and the button 646, all placed from [`SCROLL_AT`] (the capture's draws sit
/// exactly where the layout puts them from there).
const SCROLL: u32 = 645;
/// The world map's scroll: the same picture upside down (its flat foot with the folded corners
/// on top, its roll at the foot), [`WORLD_SCROLL_RISE`] higher than the area map's (the
/// colseit-arrival recording at 1:12: the flat top and the roll by RMSE, 0.103 and 0.047 against
/// 0.138 and 0.107 for the upright picture cut short with its roll laid at the foot); no button.
/// The list's rules from [`WORLD_ROLL_TOP`] down, under the roll, are not drawn.
const WORLD_SCROLL_RISE: i32 = 9;
const WORLD_ROLL_TOP: i32 = 681;

/// The world map's scroll, upside down.
fn rolled_scroll(pe: &barrel_formats::pe::Pe) -> Vec<Element> {
    let Some(m) = barrel_escha::data::layout::sprite(pe, SCROLL_PART, SCROLL_CUT, [0, 0]) else { return Vec::new() };
    let [l, t, r, b] = m.src.map(f32::from);
    let [x, y] = [SCROLL_AT[0], SCROLL_AT[1] - WORLD_SCROLL_RISE];
    vec![Element::Slice { pack: m.texture.clone(), index: m.index, src: [l, b, r, t], rect: [x, y, x + (r - l) as i32, y + (b - t) as i32] }]
}
/// The scroll's own layout is 645: the scroll, 641 (the item panel) at its top-left, 646 (the
/// button) at (248, 678).
/// The pad buttons' sprites (cuts in pairs, as the font's `<IMnn>`: see `message::western`).
const BUTTON_ICONS: i32 = 0x41;
/// (The name's centre; its node, with [`TEXT_RISE`] taken off.)
const NAME_AT: [i32; 2] = [1674, 164];
/// The place's name on the scroll, the maps' dark red (79, 37, 37) as the capture draws it.
const NAME_INK: [u8; 4] = [0xff, 0x4f, 0x25, 0x25];
use crate::play::hud::TEXT_RISE;
/// The scroll's ink: the layouts' text colour (58, 26, 8), drawn opaque (the recording's
/// darkest text pixels are (57, 27, 6); the layouts' alpha byte 0xe1 would lighten it).
const INK: [u8; 4] = [0xff, 0x3a, 0x1a, 0x08];
/// The description's widest line (screen pixels), as the game breaks "A sector of the /
/// enormous ruins.".
const DESCRIPTION_WIDTH: f32 = 330.0;
const LINE_STEP: i32 = 38;
/// The area map's description lines, as the capture breaks them (37.1 apart).
const AREA_LINE_STEP: i32 = 37;
/// The item lines' rule (641's part 0xbe cut 5) and how far below its node the world map's
/// recordings draw it.
const RULE: i32 = 0xbe;
const RULE_DROP: i32 = 7;

fn moved(p: &Placed, by: [i32; 2]) -> [i32; 2] {
    [p.at[0] + by[0], p.at[1] + by[1]]
}

/// The panels for place `name` (with `description`) in region `region`; `width_of` measures a
/// text at size `size` in screen pixels, for breaking the description into lines.
/// The maps' frame, under every panel: 650's diamond border, then layout 649's dark red
/// vignette (one corner sheet, a15_common #25, mirrored to the four corners; the capture draws
/// the border first).
pub const FRAME: [u32; 2] = [650, 649];

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
    tier: Option<usize>,
    width_of: impl Fn(&str, f32) -> f32,
) -> Vec<Element> {
    let texts = |p: &Placed| barrel_escha::data::layout::text(pe, p);
    // The game draws the layouts' texts opaque (their alpha byte 0xe1 is not applied) and
    // [`TEXT_RISE`] above where Barrel's glyph placement puts a node.
    let raised = |t: Text| Text { at: [t.at[0], t.at[1] - TEXT_RISE], colour: if t.colour[0] == 0 { t.colour } else { [0xff, t.colour[1], t.colour[2], t.colour[3]] }, ..t };
    let mut out = Vec::new();
    out.extend(header(pe));
    for p in barrel_escha::data::layout::placed(pe, REGION) {
        if let Some(t) = texts(&p) {
            let at = [p.at[0] + AREA_REGION_AT[0], p.at[1] + AREA_REGION_AT[1]];
            out.push(Element::Label(Text { at, colour: if t.colour[0] == 0 { t.colour } else { [0xff, t.colour[1], t.colour[2], t.colour[3]] }, text: region.to_string(), ..t }));
        }
    }
    let mut first_text = true;
    let mut row = 0;
    for p in &barrel_escha::data::layout::placed(pe, SCROLL) {
        let at = moved(p, SCROLL_AT);
        let sprite = |cut: i32| Element::Sprite { part: p.node.part, cut, at };
        match (p.node.part, texts(p)) {
            // The tab's title: the gatherables' (with their level), or "Monsters" (X on the map:
            // the escha-areamap-buttons recording, no level there).
            (0x164, Some(t)) if t.kind == 22 => out.push(Element::Label(raised(Text { at, text: if tier.is_some() { "Gatherable Items" } else { "Monsters" }.into(), ..t }))),
            // The description in the first text's place, broken into lines.
            (0x164, Some(t)) if first_text && t.filled_in() => {
                first_text = false;
                for (k, line) in lines(description, |s| width_of(s, t.size)).into_iter().enumerate() {
                    out.push(Element::Label(raised(Text { at: [at[0], at[1] + AREA_LINE_STEP * k as i32], text: line, colour: INK, ..t.clone() })));
                }
            }
            // The gatherable items, one a line, in the scroll's ink.
            (0x164, Some(t)) if t.filled_in() => {
                if let Some(item) = items.get(row) {
                    out.push(Element::Label(raised(Text { at, text: item.clone(), colour: INK, ..t })));
                }
                row += 1;
            }
            // "...ETC" past the tenth.
            (0x164, Some(t)) if items.len() > 10 => out.push(Element::Label(raised(Text { at, colour: INK, ..t }))),
            (0x164, _) => {}
            // The gathering level: cut 12 plus the points' tier (Lv1 to Lv4, MAX).
            (0x69, _) => {
                if let Some(tier) = tier {
                    out.push(Element::Sprite { part: 0x69, cut: 12 + tier.min(4) as i32, at });
                }
            }
            // The pad button's icon (the button's plate 646 is a plain sprite).
            (BUTTON_ICONS, _) => out.push(sprite(crate::play::message::western_sprite(p.node.cut.max(0) as u32) as i32)),
            _ => out.push(sprite(p.node.cut.max(0))),
        }
    }
    out.push(Element::Label(raised(Text { at: NAME_AT, size: 36.0, colour: NAME_INK, text: name.to_string(), kind: 22 })));
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
    out.extend(rolled_scroll(pe));
    if let Some((p, t)) = barrel_escha::data::layout::placed(pe, PANEL).iter().find_map(|p| texts(p).filter(|t| t.filled_in()).map(|t| (p.clone(), t))) {
        let at = moved(&p, PANEL_AT);
        for (k, line) in lines(description.trim(), |s| width_of(s, t.size)).into_iter().enumerate() {
            out.push(Element::Label(Text { at: [at[0] - WORLD_TEXT_LEFT, at[1] - WORLD_DESCRIPTION_RISE + LINE_STEP * k as i32], text: line, colour: INK, ..t.clone() }));
        }
    }
    for p in barrel_escha::data::layout::placed(pe, TAB) {
        let at = moved(&p, MAP_TAB_AT);
        match (p.node.part, texts(&p)) {
            (0x164, Some(t)) => out.push(Element::Label(Text { at: [at[0] - WORLD_TEXT_LEFT, at[1]], text: "Map".into(), ..t })),
            (0x69, _) => {}
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    // The sectors: the item panel's rows and rules (641, as on the area map), standing
    // [`WORLD_LIST_RISE`] higher with the tab.
    let mut row = 0;
    let mut first_text = true;
    for p in barrel_escha::data::layout::placed(pe, PANEL) {
        let [x, y] = moved(&p, PANEL_AT);
        let at = [x, y - WORLD_LIST_RISE];
        match (p.node.part, texts(&p)) {
            (0x164, Some(t)) if first_text && t.filled_in() => first_text = false,
            (0x164, Some(t)) if t.filled_in() => {
                if let Some(name) = sectors.get(row) {
                    out.push(Element::Label(Text { at: [at[0] - WORLD_TEXT_LEFT, at[1]], text: name.clone(), colour: INK, ..t }));
                }
                row += 1;
            }
            (RULE, _) if at[1] + RULE_DROP < SCROLL_AT[1] + WORLD_ROLL_TOP => out.push(Element::Sprite { part: RULE, cut: p.node.cut.max(0), at: [at[0], at[1] + RULE_DROP] }),
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
                // Each number's last digit from its node (measured on the recording at 1:25,
                // "4" and "8 /24", side by side at 300%: the days' 18 left, the month's 19
                // left, the day's 2 right; the "/" 2 left of its node).
                let (value, shift) = match numbers {
                    0 => (days.max(0) as u32, -18),
                    1 => (month, -19),
                    _ => (day, 2),
                };
                let digits: Vec<i32> = value.to_string().bytes().map(|b| i32::from(b - b'0')).collect();
                let start = at[0] + shift - TRAVEL_ADVANCE * (digits.len() as i32 - 1);
                for (k, d) in digits.into_iter().enumerate() {
                    out.push(Element::Sprite { part: TRAVEL_DIGITS, cut: d, at: [start + TRAVEL_ADVANCE * k as i32, at[1] + TRAVEL_DIGITS_DOWN] });
                }
                numbers += 1;
            }
            (TRAVEL_DIGITS, _) => out.push(Element::Sprite { part: TRAVEL_DIGITS, cut: p.node.cut.max(0), at: [at[0] - 2, at[1] + TRAVEL_DIGITS_DOWN] }),
            _ => out.push(Element::Sprite { part: p.node.part, cut: p.node.cut.max(0), at }),
        }
    }
    out
}

/// The travel panel's labels, as the recording has them.
const TRAVEL_LABEL: [u8; 4] = [0xff, 0xde, 0xba, 0x70];
const TRAVEL: u32 = 639;
/// (Refitted at 1:25 with the world camera of 3dab6a6: 16 higher than 1:18's measure.)
const TRAVEL_UNDER: [i32; 2] = [-191, 11];
const TRAVEL_DIGITS: i32 = 0xcf;
/// A digit's step (the recording's "24": 31 pixels).
const TRAVEL_ADVANCE: i32 = 31;
/// The digits stand this much lower than their nodes (the recording at 1:25, patch search).
const TRAVEL_DIGITS_DOWN: i32 = 5;
/// The world map's sector list: the item panel's rows (641) this much higher than on the area
/// map (the recording at 1:12: rows 45 apart as 641 has them, not 642's 50; the height by patch
/// search of the rows' names).
const WORLD_LIST_RISE: i32 = 42;
/// The world scroll's description stands this much higher than the area map's.
const WORLD_DESCRIPTION_RISE: i32 = 40;
/// The world scroll's texts (description, "Map", the sectors) stand this much further left
/// than the area map's (the recording at 1:12: 3 pixels at 720p, all alike).
const WORLD_TEXT_LEFT: i32 = 4;
const MAP_TAB_AT: [i32; 2] = [1448, 272];

/// A neighbour's label: layout 623, the plate part 0x27 (kind 19, three cuts: the left end
/// with the body 192 wide, a 48-wide middle repeated, the 48-wide right end, 72 high) at a
/// fixed 360 wide (its text's node at 176, centred; both labels of the area map's capture are
/// 360 whatever their name's length), the name centred on it in its dark ink. The plate's centre
/// stands [`LABEL_ABOVE`] over the label's place `at` (its route's dot), beside it as
/// [`LABEL_LEFT_OF`] / [`LABEL_RIGHT_OF`] say.
/// The gilt ornament on its outer end, as layouts 633 (right: part 0x90 through 629 and 627
/// at (306 + 88, 0)) and 634 (left: part 0x91 through 630 and 628 at (82, 0), the plate at
/// (36, 32)) place it, turned a quarter clockwise about its node (layout kind 7), the plate 32
/// below the layout's top. Not yet the game's: the ornament's bob (its four cuts).
pub fn label(pe: &barrel_formats::pe::Pe, at: [i32; 2], name: &str, side: Side) -> Vec<Element> {
    let (left, top, ornament) = match side {
        Side::Left | Side::Right => {
            let inward = side == Side::Left;
            let left = at[0] + if inward { LABEL_LEFT_OF } else { LABEL_RIGHT_OF } - PLATE_WIDTH / 2;
            let top = at[1] - LABEL_ABOVE - PLATE_HEIGHT / 2;
            // The layout's top-left, then the ornament's node in it.
            let (x0, y0) = if inward { (left - 36, top - 32) } else { (left, top - 32) };
            let ornament = if inward { Element::Turned { part: 0x91, cut: 0, at: [x0 + 82, y0] } } else { Element::Turned { part: 0x90, cut: 0, at: [x0 + 306 + 88, y0] } };
            (left, top, ornament)
        }
        // Layouts 631 (the ornament 0x90 upright over the plate, the plate 64 down) and 632 (the
        // plate first, 0x91 hanging 44 down), centred on the route.
        Side::Above => {
            let (left, top) = (at[0] + LABEL_UP_X - NAME_X, at[1] - LABEL_UP - PLATE_HEIGHT / 2);
            (left, top, Element::Sprite { part: 0x90, cut: 0, at: [left + 102, top - 64] })
        }
        Side::Below => {
            let (left, top) = (at[0] + LABEL_UP_X - NAME_X, at[1] + LABEL_DOWN - PLATE_HEIGHT / 2);
            (left, top, Element::Sprite { part: 0x91, cut: 0, at: [left + 102, top + 44] })
        }
    };
    let mut out = vec![Element::Sprite { part: PLATE, cut: 0, at: [left, top] }];
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
            // Opaque like the game's (the layout's alpha byte is not applied).
            let colour = [0xff, t.colour[1], t.colour[2], t.colour[3]];
            out.push(Element::Label(Text { at: [left + p.at[0], top + p.at[1] - TEXT_RISE], text: name.to_string(), colour, ..t }));
        }
    }
    out.push(ornament);
    out
}

const LABEL: u32 = 623;
const PLATE: i32 = 0x27;
/// (The capture's plates are 360 wide: 192, 48, 48, 24 and 48 pieces.)
const PLATE_WIDTH: i32 = 360;
/// Where 623's text node centres the name on its plate (the centre the recordings' 352-wide
/// plates had, for the labels above and below a route).
const NAME_X: i32 = 176;
const PLATE_HEIGHT: i32 = 72;
const PLATE_LEFT: i32 = 192;
const PLATE_END: i32 = 48;
/// How far the plate's centre stands over its dot, and where it stands beside it: the centre
/// of a label left of the screen's middle (the ornament on its left) is [`LABEL_LEFT_OF`] right
/// of its dot, that of one right of it [`LABEL_RIGHT_OF`] (the area map's capture: "Ruins
/// Entrance" and "Sector Two").
const LABEL_ABOVE: i32 = 47;
const LABEL_LEFT_OF: i32 = 27;
const LABEL_RIGHT_OF: i32 = -12;
/// A label on a route going up or down the screen: its plate's centre that far over (under)
/// the route's third dot, and right of it. Above: the colseit-arrival recording at 1:12
/// (Colseit's label north of the Southern Ruins, fitted at 1080p: 11 under the dot, 6 right).
/// Below: no recording yet, taken as the mirror.
const LABEL_UP: i32 = -11;
const LABEL_DOWN: i32 = -11;
const LABEL_UP_X: i32 = 6;

/// Where a neighbour's label stands by its route: beside it on a route going left or right
/// (layouts 634 / 633, the ornament on the outer end), above or below on one going up or down
/// (631 / 632, the ornament over or under the plate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

/// `text` broken at spaces into lines no wider than [`DESCRIPTION_WIDTH`]; a space ending the
/// text stays on its last line (the game draws it).
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
    if text.ends_with(' ')
        && let Some(last) = out.last_mut()
    {
        last.push(' ');
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
