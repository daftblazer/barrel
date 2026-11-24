//! What the item card shows of an item (the Equipment, Container and Gathering Tools pages): the game's item table
//! (`saves/item/itemdata.xml`) and the save's unit of the item. Docs: docs/tables.md, "Ryza DX: the item card".

use barrel_formats::table;

use crate::save::Worn;

/// `STR_ITEM_NAME_n` is string `NAME_BASE + n`, the effects' names `EFFECT_BASE + number`, the potentials' `POTENTIAL_BASE + row` and
/// the categories' `CATEGORY_BASE + place in the category table` (`saves/text_en/strcombineall.xml`).
pub const NAME_BASE: u32 = 6_750_209;
pub const EFFECT_BASE: u32 = 6_881_281;
pub const POTENTIAL_BASE: u32 = 6_946_817;
pub const CATEGORY_BASE: u32 = 6_815_745;

/// A potential row of this number or more is no potential (the table has 290 rows; a unit's empty places hold 290).
const NO_POTENTIAL: u16 = 290;

/// An item's quality takes one rank letter off every `RANK_STEP` points: 30 is D, 50 C, 100 B (the capture's three cards).
/// STAND-IN: the steps above B and below D are not seen.
const RANK_STEP: u16 = 50;
/// The rank pictures run from S (0) to E (5); D, the lowest seen, is 4.
const RANK_D: usize = 4;

/// One row of `itemdata.xml`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub level: i32,
    pub elem_value: i32,
    /// Fire, ice, bolt and air (`elemFire`, `elemIce`, `elemThunder`, `elemAir`).
    pub elements: [bool; 4],
    /// HP, attack, defence and speed.
    pub status: [i32; 4],
    /// The attacker, defender and supporter levels (`r_atk`, `r_def`, `r_spt`).
    pub roles: [i32; 3],
    /// The characters who may wear it (`pc_0` to `pc_8`).
    pub wearers: [bool; 9],
    /// The first category's place in the category table.
    pub category: Option<usize>,
    pub weapon: bool,
}

/// The item table with the category order.
#[derive(Clone, Debug, Default)]
pub struct Items {
    rows: Vec<Row>,
}

/// What a card shows (strings by number, for the page to look up).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub name: u32,
    pub image: usize,
    pub level: i32,
    pub elem_value: i32,
    pub elements: [bool; 4],
    /// A weapon's free slots.
    pub slots: Option<u8>,
    pub quality: u16,
    /// The rank picture's number (0 is the best letter).
    pub rank: usize,
    pub effects: Vec<u32>,
    pub status: [i32; 4],
    /// The potentials: a name's string and the level.
    pub traits: Vec<(u32, u8)>,
    pub roles: [i32; 3],
    /// For each member of the party, whether the item suits them.
    pub equippable: Vec<bool>,
    pub category: Option<u32>,
}

impl Items {
    /// Reads `itemdata.xml` and `fieldmix/fieldmixcat.xml` (whose rows give the categories' order).
    pub fn read(item_data: &[u8], categories: &[u8]) -> Result<Items, String> {
        let root = table::parse(item_data).map_err(|e| format!("itemdata: {e}"))?;
        let order: Vec<String> = table::parse(categories).map_err(|e| format!("fieldmixcat: {e}"))?.children.iter().filter_map(|c| c.attr("category").map(str::to_string)).collect();
        let rows = root
            .children
            .iter()
            .map(|e| {
                let n = |k: &str| e.attr(k).and_then(|t| t.trim().parse::<i32>().ok()).unwrap_or(0);
                let has = |k: &str| e.attr(k).is_some_and(|v| v.trim() == "-1");
                let mut wearers = [false; 9];
                for (i, w) in wearers.iter_mut().enumerate() {
                    *w = has(&format!("pc_{i}"));
                }
                Row {
                    level: n("lv"),
                    elem_value: n("elemValue"),
                    elements: [has("elemFire"), has("elemIce"), has("elemThunder"), has("elemAir")],
                    status: [n("hp"), n("atk"), n("def"), n("spd")],
                    roles: [n("r_atk"), n("r_def"), n("r_spt")],
                    wearers,
                    category: e.attr("cat_0").and_then(|c| order.iter().position(|o| o == c)),
                    weapon: e.attr("kindTag") == Some("ITEM_KIND_WEAPON"),
                }
            })
            .collect();
        Ok(Items { rows })
    }

    pub fn row(&self, item: u16) -> Option<&Row> {
        self.rows.get(usize::from(item))
    }

    /// The card of `unit` for a party of `party` (their character numbers), its picture number from `image`.
    pub fn card(&self, unit: &Worn, party: &[u8], image: usize) -> Card {
        let row = self.row(unit.item).cloned().unwrap_or_default();
        // STAND-IN: the element value shown is the table's plus one for each full hundred of quality (2 of the necklace, quality 100,
        // is 3 on its card; the staff's 1 and the shoes' 2 at quality 30 and 50 stay).
        let elem_value = row.elem_value + i32::from(unit.quality / 100);
        let level = if unit.level > 0 { i32::from(unit.level) } else { row.level };
        Card {
            name: NAME_BASE + u32::from(unit.item),
            image,
            level,
            elem_value,
            elements: row.elements,
            slots: row.weapon.then_some(unit.slots),
            quality: unit.quality,
            rank: RANK_D.saturating_sub(usize::from(unit.quality / RANK_STEP)),
            effects: unit.effects.iter().filter(|&&e| e != 0xffff).map(|&e| EFFECT_BASE + u32::from(e)).collect(),
            status: row.status,
            traits: unit.potentials.iter().zip(unit.potential_levels).filter(|(p, _)| **p < NO_POTENTIAL).map(|(&p, l)| (POTENTIAL_BASE + u32::from(p), l)).collect(),
            roles: row.roles,
            equippable: party.iter().map(|&c| row.wearers.get(usize::from(c)).copied().unwrap_or(false)).collect(),
            category: row.category.map(|c| CATEGORY_BASE + c as u32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = r#"<Root>
<itemData nameID="a" lv="4" elemValue="1" elemFire="-1" pc_0="-1" atk="6" spd="2" kindTag="ITEM_KIND_WEAPON" cat_0="ITEM_CATEGORY_WEAPON"/>
<itemData nameID="b" lv="26" elemValue="2" elemIce="-1" pc_0="-1" pc_1="-1" r_def="2" kindTag="ITEM_KIND_ACCESSORY" cat_0="ITEM_CATEGORY_ACCESSORY"/>
</Root>"#;
    const CATEGORIES: &str = r#"<Root><FieldMixCat No="0" category="ITEM_CATEGORY_WEAPON"/><FieldMixCat No="1" category="ITEM_CATEGORY_ACCESSORY"/></Root>"#;

    fn items() -> Items {
        Items::read(ITEMS.as_bytes(), CATEGORIES.as_bytes()).unwrap()
    }

    #[test]
    fn a_weapons_card_has_its_slots_and_status() {
        let unit = Worn { item: 0, quality: 30, level: 4, slots: 3, effects: [0xffff; 4], potentials: [NO_POTENTIAL; 3], ..Default::default() };
        let c = items().card(&unit, &[0, 2], 7);
        assert_eq!((c.level, c.elem_value, c.elements, c.slots, c.status, c.rank), (4, 1, [true, false, false, false], Some(3), [0, 6, 0, 2], 4));
        assert_eq!((c.name, c.image, c.equippable, c.category), (NAME_BASE, 7, vec![true, false], Some(CATEGORY_BASE)));
        assert!(c.effects.is_empty() && c.traits.is_empty());
    }

    #[test]
    fn an_accessorys_card_reads_its_effects_and_roles() {
        let unit = Worn { item: 1, quality: 100, level: 26, effects: [1175, 1276, 1279, 0xffff], potentials: [NO_POTENTIAL; 3], ..Default::default() };
        let c = items().card(&unit, &[0, 1], 0);
        assert_eq!((c.elem_value, c.slots, c.roles, c.rank, c.equippable), (3, None, [0, 2, 0], 2, vec![true, true]));
        assert_eq!(c.effects, vec![EFFECT_BASE + 1175, EFFECT_BASE + 1276, EFFECT_BASE + 1279]);
        assert_eq!(c.category, Some(CATEGORY_BASE + 1));
    }

    #[test]
    fn the_rank_follows_the_quality_in_steps_of_fifty() {
        let rank = |q| items().card(&Worn { item: 1, quality: q, ..Default::default() }, &[], 0).rank;
        assert_eq!((rank(30), rank(50), rank(100)), (4, 3, 2));
    }
}
