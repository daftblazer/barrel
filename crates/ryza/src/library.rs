//! What the Guide's Item and Effect/Trait pages list: the entries of the game's library the save knows (`knowledge/item`, `knowledge/eff`) and
//! the library tables (`saves/library/libraryitem.xml`, `libraryeffdetail.xml`). Docs: docs/formats.md "Knowledge", docs/ryza/open.md "Field menu, part 11".

use barrel_formats::table;

use crate::item_info::{Items, EFFECT_BASE, NAME_BASE};

/// The string of an effect's description (`str_library_eff_detail.xml`): `DETAIL_BASE + the effect's number`.
const DETAIL_BASE: u32 = 3_538_945;
/// "???" (`str_item_define.xml`), the name of an item the library shows only as a silhouette.
pub const UNKNOWN_NAME: u32 = 6_684_707;

/// The Item page's four tabs: items to use (and the gathering tools), equipment, materials and the rest.
/// STAND-IN: only the first tab is captured; the others take the item kinds that fit their pictures (potion, sword and shield, fruit, key).
pub const TABS: usize = 4;

/// One card of the Item page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// The picture's number (`imgNo`) and the item's level (`lv`), and its name's string.
    pub image: usize,
    pub level: i32,
    pub name: u32,
    /// Whether the card shows the item (else a silhouette under "???").
    pub revealed: bool,
}

/// One entry of the Effects tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    /// The effect's name's string and its description's.
    pub name: u32,
    pub text: u32,
    /// Its power in stars (0 to 5).
    pub stars: u8,
}

/// The library's tables.
#[derive(Clone, Debug, Default)]
pub struct Library {
    /// Whether each library item may be shown (`permit`), and each effect's power and permit.
    item_permit: Vec<bool>,
    effects: Vec<(u8, bool)>,
}

/// The knowledge level a library entry has to be at before its picture and name show. STAND-IN: the capture's items are at 10 (made or
/// taken) and show as "???", so what reveals one is not in the capture; 11 never comes, so every item shows its silhouette.
const REVEALED_AT: i32 = 11;

impl Library {
    pub fn read(items: &[u8], effects: &[u8]) -> Result<Library, String> {
        let permit = |e: &table::Element| e.attr("permit").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
        let items = table::parse(items).map_err(|e| format!("libraryitem: {e}"))?;
        let effects = table::parse(effects).map_err(|e| format!("libraryeffdetail: {e}"))?;
        Ok(Library {
            item_permit: items.children.iter().map(permit).collect(),
            effects: effects.children.iter().map(|e| (e.attr("power").and_then(|p| p.trim().parse().ok()).unwrap_or(0), permit(e))).collect(),
        })
    }

    /// The cards of tab `tab`: the items the save knows (a level above 0) in the table's order, as many as `limit`.
    pub fn cards(&self, items: &Items, know: &[i32], tab: usize, image: &dyn Fn(u16) -> usize) -> Vec<Card> {
        (0..self.item_permit.len().min(know.len()))
            .filter(|&i| self.item_permit[i] && know[i] > 0 && items.row(i as u16).is_some_and(|r| tab_of(&r.kind) == tab))
            .map(|i| Card { image: image(i as u16), level: items.row(i as u16).map_or(0, |r| r.level), name: NAME_BASE + i as u32, revealed: know[i] >= REVEALED_AT })
            .collect()
    }

    /// The share of the library's items the save knows, in per cent (the page's "Gather Rate").
    pub fn item_rate(&self, know: &[i32]) -> u32 {
        rate(self.item_permit.iter().enumerate().filter(|&(_, &p)| p).count(), self.item_permit.iter().enumerate().filter(|&(i, &p)| p && know.get(i).is_some_and(|&k| k > 0)).count())
    }

    /// The effects the save knows, in the table's order.
    pub fn known_effects(&self, know: &[i32]) -> Vec<Effect> {
        self.effects.iter().enumerate().filter(|&(i, &(_, permit))| permit && know.get(i).is_some_and(|&k| k > 0)).map(|(i, &(power, _))| Effect { name: EFFECT_BASE + i as u32, text: DETAIL_BASE + i as u32, stars: power }).collect()
    }

    /// The share of the library's effects the save knows, in per cent.
    pub fn effect_rate(&self, know: &[i32]) -> u32 {
        rate(self.effects.iter().filter(|e| e.1).count(), self.known_effects(know).len())
    }
}

/// The tab an item kind (`kindTag`) is listed under.
fn tab_of(kind: &str) -> usize {
    match kind {
        "ITEM_KIND_HEAL" | "ITEM_KIND_ATTACK" | "ITEM_KIND_BUFFER" | "ITEM_KIND_DEBUFFER" | "ITEM_KIND_FIELD" => 0,
        "ITEM_KIND_WEAPON" | "ITEM_KIND_ARMOR" | "ITEM_KIND_ACCESSORY" => 1,
        "ITEM_KIND_MATERIAL" | "ITEM_KIND_MIX" => 2,
        _ => 3,
    }
}

fn rate(total: usize, known: usize) -> u32 {
    if total == 0 { 0 } else { (known as f32 * 100.0 / total as f32).round() as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_is_the_known_share_in_per_cent_rounded() {
        assert_eq!(rate(462, 8), 2);
        assert_eq!(rate(698, 8), 1);
        assert_eq!(rate(0, 0), 0);
    }

    #[test]
    fn kinds_fall_under_their_tabs() {
        assert_eq!(tab_of("ITEM_KIND_FIELD"), 0);
        assert_eq!(tab_of("ITEM_KIND_WEAPON"), 1);
        assert_eq!(tab_of("ITEM_KIND_MATERIAL"), 2);
        assert_eq!(tab_of("ITEM_KIND_IMPORTANT"), 3);
    }

    #[test]
    fn the_effects_known_are_listed_in_the_tables_order_with_their_stars() {
        let lib = Library::read(b"<Root/>", br#"<Root><E power="1" permit="true"/><E power="3" permit="true"/><E permit="false"/><E power="2" permit="true"/></Root>"#).unwrap();
        let list = lib.known_effects(&[0, 10, 10, 10]);
        assert_eq!(list.iter().map(|e| (e.name - EFFECT_BASE, e.stars)).collect::<Vec<_>>(), vec![(1, 3), (3, 2)]);
        assert_eq!(list[0].text, DETAIL_BASE + 1);
    }
}
