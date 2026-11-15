//! The items the player carries and the Basket page's cursor. The page itself is drawn by
//! `basketpage.rs` (the game's grid 568 and item card 752, as in its capture). What is left here
//! is the older grid (layout 456 on `a15_menu.g1t`: 25 cells, five rows of five, each a frame,
//! the item's picture, part 0xd7, and a number, part 0x54; the first column's cells are layouts
//! 452 to 455, the others 451) under its tab (layout 457) that the Drops screen still borrows;
//! it is not the game's (the game's drops screen is not drawn from it).
//!
//! A cell's picture is the item's `ImageNo` (`itemData.xml`) as the picture index in
//! `WinXls/a15_itempic.g1t` (the layouts' part 0xd7 and 0xd6 carry the same index as a cut).

use barrel_escha::data::layout::{Placed, Text};

use crate::play::hud::Element;
use crate::play::title::Button;

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
pub const COLUMNS: usize = 5;
/// The rows the Basket page shows (its grid's six; the older grid of the drops screen, five, is
/// not scrolled by [`Basket::press`]).
const ROWS: usize = 6;
const DIGIT_ADVANCE: i32 = 18;
/// The item picture pack.
pub const PICTURES: &str = "a15_itempic.g1t";
const PICTURE_SIZE: f32 = 96.0;
/// Where the chosen item's line goes (under the grid).
const LINE_AT: [i32; 2] = [170, 860];

/// One item in the basket: its name, picture and quality, and what the Basket page's card shows
/// of it (empty when it was made from its name and quality alone).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub name: String,
    pub picture: u32,
    pub quality: i32,
    /// `itemData`'s `level`: the card's Alchemy LV.
    pub level: i32,
    /// The element value (`value`) and which of fire, water, wind and earth the item has.
    pub value: i32,
    pub elements: [bool; 4],
    /// Property lines (`item_status.xml`'s ESP tags by name), effect lines and trait lines.
    pub properties: Vec<String>,
    pub effects: Vec<String>,
    pub traits: Vec<String>,
    /// How many of the item the basket and the container hold.
    pub in_basket: i32,
    pub in_container: i32,
}

/// The game's item tables the card reads (the rows of each file).
pub struct Catalog<'a> {
    pub items: &'a [barrel_formats::table::Element],
    pub status: &'a [barrel_formats::table::Element],
    pub potentials: &'a [barrel_formats::table::Element],
}

/// A property's name from its tag in `item_status.xml` (`ITEM_MIX_POT_ZONE_EARTH_2` is
/// `item_mat_pot.xml`'s "Earth Bonus x2"; the quality and level tags follow the same naming).
pub fn property_name(tag: &str) -> Option<String> {
    let tag = tag.strip_prefix("ITEM_MIX_POT_")?;
    if let Some(zone) = tag.strip_prefix("ZONE_") {
        let (element, n) = zone.rsplit_once('_')?;
        let n = n.trim_start_matches('P');
        return Some(match element {
            "ALL" => format!("All Elements x{n}"),
            "FIRE" => format!("Fire Bonus x{n}"),
            "WATER" => format!("Water Bonus x{n}"),
            "WIND" => format!("Wind Bonus x{n}"),
            "EARTH" => format!("Earth Bonus x{n}"),
            _ => return None,
        });
    }
    if let Some(n) = tag.strip_prefix("QUALITY_P") {
        return Some(format!("Effect Power+{}", 5 * n.parse::<i32>().ok()?));
    }
    tag.strip_prefix("LEVEL_").map(|n| format!("Appearance Lv+{n}"))
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
                ..Item::default()
            })
        })
        .collect()
}

/// What the player holds, for the Basket page.
pub struct Stock {
    pub items: Vec<barrel_escha::gather::Got>,
    /// The basket's size.
    pub limit: i32,
    /// The container's items (`itemData` rows).
    pub container: Vec<i32>,
}

/// The basket's items with what the card shows of each, from the game's tables.
pub fn held(stock: &Stock, cat: &Catalog) -> Vec<Item> {
    let attr = |row: &barrel_formats::table::Element, name: &str| row.attr(name).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    stock
        .items
        .iter()
        .filter_map(|g| {
            let row = cat.items.get(usize::try_from(g.item).ok()?)?;
            let status = cat.status.get(g.item as usize);
            let properties = (0..2).filter_map(|k| status?.attr(&format!("ESP_0{k}")).and_then(property_name)).collect();
            let traits = g.traits.iter().filter_map(|&t| Some(cat.potentials.get(t)?.attr("PotName")?.to_string())).collect();
            Some(Item {
                name: row.attr("ItemName").unwrap_or("?").to_string(),
                picture: row.attr("ImageNo").and_then(|v| v.parse().ok()).unwrap_or(0),
                quality: g.quality.round() as i32,
                level: attr(row, "level"),
                value: attr(row, "value"),
                elements: ["fire", "water", "wind", "soil"].map(|e| attr(row, e) > 0),
                properties,
                effects: Vec::new(),
                traits,
                in_basket: stock.items.iter().filter(|o| o.item == g.item).count() as i32,
                in_container: stock.container.iter().filter(|&&c| c == g.item).count() as i32,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_are_named_from_their_tags() {
        assert_eq!(property_name("ITEM_MIX_POT_ZONE_EARTH_2").as_deref(), Some("Earth Bonus x2"));
        assert_eq!(property_name("ITEM_MIX_POT_ZONE_FIRE_P4").as_deref(), Some("Fire Bonus x4"));
        assert_eq!(property_name("ITEM_MIX_POT_ZONE_ALL_3").as_deref(), Some("All Elements x3"));
        assert_eq!(property_name("ITEM_MIX_POT_QUALITY_P3").as_deref(), Some("Effect Power+15"));
        assert_eq!(property_name("ITEM_MIX_POT_LEVEL_2").as_deref(), Some("Appearance Lv+2"));
        assert_eq!(property_name("ITEM_POTENTIAL_RECYCLE_2"), None);
    }

    #[test]
    fn moves_in_rows_of_five_and_scrolls() {
        let mut b = Basket::default();
        assert!(!b.press(Button::Right, 30));
        assert!(!b.press(Button::Down, 30));
        assert_eq!(b.chosen, 6);
        for _ in 0..4 {
            b.press(Button::Down, 30);
        }
        assert_eq!((b.chosen, b.first), (26, 0));
        b.press(Button::Down, 30);
        assert_eq!(b.chosen, 26, "no row below");
        assert!(b.press(Button::Back, 30));
    }
}
