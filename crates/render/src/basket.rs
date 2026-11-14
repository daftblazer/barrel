//! The main menu's Basket page: the items the player carries, in the game's item grid (layout
//! 456 on `a15_menu.g1t`: 25 cells, five rows of five, each a frame, the item's picture, part
//! 0xd7, and a number, part 0x54; the first column's cells are layouts 452 to 455, the others
//! 451) under its tab (layout 457: a title between the LB / RB marks).
//!
//! A cell's picture is the item's `ImageNo` (`itemData.xml`) as the picture index in
//! `WinXls/a15_itempic.g1t` (528 pictures of 96 × 96; the layout's own placeholder is picture
//! 519). Read from the layouts: the pieces and their places.
//!
//! Not yet the game's (the game's Basket screen is not identified among its layouts; the grid is
//! the one its Container screen, layout 458, shows twice): the grid alone on the desk at the
//! layout's own place; the tab's title "Basket"; the cell's number taken as the item's quality;
//! the lit frame (part 0x17 cut 5) on the chosen cell only and the other items' pictures at half
//! strength; the chosen item's name and quality as a line under the grid; the first column's
//! cells of rows 2 to 5 (layouts 452 to 455: wider frames, 240 to 546 pixels, likely the
//! cursor over several cells) drawn as plain cells at their places; more than 25 items
//! scrolling by rows.

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::Element;
use crate::title::Button;

pub const GRID: u32 = 456;
const TAB: u32 = 457;
const CELLS: std::ops::RangeInclusive<u32> = 451..=455;
const NUMBER: i32 = 0x54;
/// A plain cell's pieces (layout 451), from the cell's place: the lit frame (part 0x17 cut 5),
/// the picture and the number.
const FRAME: i32 = 0x17;
const LIT_FRAME: i32 = 5;
const FRAME_AT: [i32; 2] = [-16, -14];
const PICTURE_AT: [i32; 2] = [18, 16];
const NUMBER_AT: [i32; 2] = [84, 52];
const COLUMNS: usize = 5;
const ROWS: usize = 5;
const DIGIT_ADVANCE: i32 = 18;
/// The item picture pack.
pub const PICTURES: &str = "a15_itempic.g1t";
const PICTURE_SIZE: f32 = 96.0;
/// Where the chosen item's line goes (under the grid).
const LINE_AT: [i32; 2] = [170, 860];

/// One item in the basket: its name, picture and quality.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub picture: u32,
    pub quality: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Basket {
    pub chosen: usize,
    /// The first row shown.
    pub first: usize,
}

impl Basket {
    /// Moves over `count` items; Back returns true (the page closes).
    pub fn press(&mut self, b: Button, count: usize) -> bool {
        let n = count.max(1);
        match b {
            Button::Left => self.chosen = self.chosen.saturating_sub(1),
            Button::Right => self.chosen = (self.chosen + 1).min(n - 1),
            Button::Up => self.chosen = self.chosen.saturating_sub(COLUMNS),
            Button::Down => self.chosen = if self.chosen + COLUMNS < n { self.chosen + COLUMNS } else { self.chosen },
            Button::Back => return true,
            Button::Confirm => {}
        }
        let row = self.chosen / COLUMNS;
        if row < self.first {
            self.first = row;
        } else if row >= self.first + ROWS {
            self.first = row + 1 - ROWS;
        }
        false
    }

    /// The elements to draw from layout 456's placed nodes (`texts` gives a node's text).
    pub fn elements(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, items: &[Item]) -> Vec<Element> {
        let mut out = Vec::new();
        // The 25 cells' places (where each cell layout sits), row by row.
        let mut cells: Vec<[i32; 2]> = Vec::new();
        for p in nodes.iter().filter(|p| p.path.get(1).is_some_and(|c| CELLS.contains(c))) {
            let origin = [p.at[0] - p.node.at[0], p.at[1] - p.node.at[1]];
            if !cells.contains(&origin) {
                cells.push(origin);
            }
        }
        cells.sort_by_key(|o| (o[1], o[0]));
        let mut line = None;
        for p in nodes {
            let n = p.node;
            match p.path.get(1) {
                Some(c) if CELLS.contains(c) => {} // drawn from the cell places below
                Some(&TAB) if n.part == 0x164 => {
                    if let Some(t) = texts(p) {
                        line = Some(t.clone());
                        out.push(Element::Label(Text { text: "Basket".into(), ..t }));
                    }
                }
                _ if n.part == 0x164 => {}
                _ => out.push(Element::stored(p)),
            }
        }
        for (slot, o) in cells.iter().enumerate() {
            let k = self.first * COLUMNS + slot;
            let Some(it) = items.get(k) else { continue };
            let lit = k == self.chosen;
            if lit {
                out.push(Element::Sprite { part: FRAME, cut: LIT_FRAME, at: [o[0] + FRAME_AT[0], o[1] + FRAME_AT[1]] });
            }
            out.push(Element::Picture { pack: PICTURES.into(), index: it.picture, size: [PICTURE_SIZE; 2], at: [o[0] + PICTURE_AT[0], o[1] + PICTURE_AT[1]], dim: !lit });
            // Right-aligned on the number's node (a kind 28 number, as the Status page's).
            let digits = it.quality.max(0).to_string();
            let last = digits.len() as i32 - 1;
            for (i, d) in digits.bytes().enumerate() {
                out.push(Element::Sprite { part: NUMBER, cut: i32::from(d - b'0'), at: [o[0] + NUMBER_AT[0] - DIGIT_ADVANCE * (last - i as i32), o[1] + NUMBER_AT[1]] });
            }
        }
        // The chosen item's name and quality, in the tab's text style.
        if let Some(t) = line {
            let text = match items.get(self.chosen) {
                Some(it) => format!("{}   Quality {}", it.name, it.quality),
                None => "The basket is empty.".into(),
            };
            out.push(Element::Label(Text { text, at: LINE_AT, kind: 21, ..t }));
        }
        out
    }
}

/// The basket's items from the session's (item row, quality) pairs and `itemData.xml`'s rows.
pub fn items(basket: &[(i32, f32)], rows: &[barrel_formats::table::Element]) -> Vec<Item> {
    basket
        .iter()
        .filter_map(|&(item, quality)| {
            let row = rows.get(usize::try_from(item).ok()?)?;
            Some(Item {
                name: row.attr("ItemName").unwrap_or("?").to_string(),
                picture: row.attr("ImageNo").and_then(|v| v.parse().ok()).unwrap_or(0),
                quality: quality.round() as i32,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_in_rows_of_five_and_scrolls() {
        let mut b = Basket::default();
        assert!(!b.press(Button::Right, 30));
        assert!(!b.press(Button::Down, 30));
        assert_eq!(b.chosen, 6);
        for _ in 0..4 {
            b.press(Button::Down, 30);
        }
        assert_eq!((b.chosen, b.first), (26, 1));
        b.press(Button::Down, 30);
        assert_eq!(b.chosen, 26, "no row below");
        assert!(b.press(Button::Back, 30));
    }
}
