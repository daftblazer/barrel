//! What an equipped item adds to each of a character's 32 stats (`0x14029dda0`), and the
//! stats in use (`0x14032de50`). See docs/tables.md, "Party growth".

use barrel_formats::battle_data::{ItemEffect, Trait};
use barrel_formats::save::ItemUnit;

/// The stats the in-use block holds (max HP first).
pub const STATS: usize = 32;

/// The `ACT_*` names each stat takes from an item's effects (`0x1402b4560`).
pub fn stat_acts(stat: usize) -> &'static [&'static str] {
    match stat {
        0 => &["ACT_EQUIP_ADD_PARAM_HP"],
        1 => &["ACT_EQUIP_ADD_PARAM_MP"],
        2 => &["ACT_EQUIP_ADD_PARAM_ATK"],
        3 => &["ACT_EQUIP_ADD_PARAM_DEF"],
        4 => &["ACT_EQUIP_ADD_PARAM_SPD"],
        5..=9 => &["ACT_EQUIP_ADD_RESIST"],
        10 => &["ACT_PASSIVE_CRITICAL", "ACT_EQUIP_ADD_CRITICAL"],
        11 => &["ACT_EQUIP_ADD_CRITBONUS"],
        12 => &["ACT_EQUIP_ADD_POWER"],
        13 => &["ACT_PASSIVE_CHANGE_SUPPORTGAUGE", "ACT_ADD_SUPGAUGE"],
        15 => &["ACT_AUTO_DODGE"],
        16 => &["ACT_PASSIVE_REDUCE_WT_SKILL"],
        17 => &["ACT_EQUIP_ADD_DMGCONVERT"],
        18 => &["ACT_EQUIP_ADD_FIRE_DMG"],
        19 => &["ACT_EQUIP_ADD_ICE_DMG"],
        20 => &["ACT_EQUIP_ADD_THUN_DMG"],
        22 => &["ACT_EQUIP_GUTS_POWER"],
        23 => &["ACT_EQUIP_ADD_PERCENT"],
        24 => &["ACT_BAD_POISON"],
        25 => &["ACT_BAD_SLEEP"],
        26 => &["ACT_BAD_BLIND"],
        27 => &["ACT_BAD_CURSE"],
        28 => &["ACT_BAD_SLOW"],
        29 => &["ACT_EQUIP_BONUS_EXP"],
        30 => &["ACT_EQUIP_BONUS_MONEY"],
        31 => &["ACT_PASSIVE_LUCKY_STAR", "ACT_EQUIP_ADD_DROPRATE"],
        _ => &[],
    }
}

/// The `ITEM_SUB_*` trait kinds each stat takes (`0x1402b4770`). `BTL_ADD_USEEFFECT` adds an
/// effect (its row in the slot's lower bound) through [`stat_acts`]; `EVOLVE_STATUS` is a
/// percentage of the stat's base.
pub fn stat_trait_kinds(stat: usize) -> &'static [&'static str] {
    const ADD: &str = "ITEM_SUB_BTL_ADD_USEEFFECT";
    match stat {
        0 => &["ITEM_SUB_EQI_ADD_MAXHP", "ITEM_SUB_EQI_ADD_ALLPARAM", ADD, "ITEM_SUB_EVOLVE_STATUS"],
        1 => &["ITEM_SUB_EQI_ADD_MAXMP", "ITEM_SUB_EQI_ADD_ALLPARAM", ADD, "ITEM_SUB_EVOLVE_STATUS"],
        2 => &["ITEM_SUB_EQI_ADD_ATTACK", "ITEM_SUB_EQI_ADD_ALLPARAM", ADD, "ITEM_SUB_EVOLVE_STATUS"],
        3 => &["ITEM_SUB_EQI_ADD_DEFENSE", "ITEM_SUB_EQI_ADD_ALLPARAM", ADD, "ITEM_SUB_EVOLVE_STATUS"],
        4 => &["ITEM_SUB_EQI_ADD_SPEED", "ITEM_SUB_EQI_ADD_ALLPARAM", ADD, "ITEM_SUB_EVOLVE_STATUS"],
        5 => &["ITEM_SUB_RESIST_FIRE", ADD],
        6 => &["ITEM_SUB_RESIST_WATER", ADD],
        7 => &["ITEM_SUB_RESIST_WIND", ADD],
        8 => &["ITEM_SUB_RESIST_EARTH", ADD],
        9 => &["ITEM_SUB_RESIST_BADSTATUS", ADD],
        12 => &["ITEM_SUB_EQI_ADD_SKILLPOWER", ADD],
        14 => &["ITEM_SUB_EQI_REDUCE_SKILLCOST", ADD],
        _ => &[ADD],
    }
}

/// An item row's equipment values (`itemData.xml`): whether it is equipment (`UseType`
/// `ITEM_USE_EQUIP`) and its `HP`, `MP`, `ATK`, `DEF`, `SPD`, `RES_FIRE` .. `RES_EARTH`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemRow {
    pub equip: bool,
    pub stats: [i32; 9],
}

impl ItemRow {
    pub fn from_row(row: &barrel_formats::table::Element) -> ItemRow {
        let n = |k: &str| row.attr(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        ItemRow {
            equip: row.attr("UseType") == Some("ITEM_USE_EQUIP"),
            stats: ["HP", "MP", "ATK", "DEF", "SPD", "RES_FIRE", "RES_WATER", "RES_WIND", "RES_EARTH"].map(n),
        }
    }
}

/// The tables the rule reads.
pub struct Tables<'a> {
    pub items: &'a [ItemRow],
    pub effects: &'a [ItemEffect],
    pub traits: &'a [Trait],
    /// The `ACT_*` names by number.
    pub acts: &'a [&'a str],
}

/// What effect row `effect` adds to `stat` (`0x1402a20f0`). `ACT_EQUIP_ADD_RESIST` slots
/// name their element in the first range (0 to 4 for stats 5 to 9) and the amount in the
/// second.
fn effect_amount(t: &Tables, effect: i32, stat: usize, roll: &mut dyn FnMut(i32, i32) -> i32) -> i32 {
    let Some(row) = usize::try_from(effect).ok().and_then(|e| t.effects.get(e)) else { return 0 };
    let takes = stat_acts(stat);
    let mut sum = 0;
    for slot in &row.effects {
        let Some(act) = t.acts.get(slot.act as usize) else { continue };
        if !takes.contains(act) {
            continue;
        }
        if *act == "ACT_EQUIP_ADD_RESIST" {
            if roll(slot.min, slot.max) == stat as i32 - 5 {
                sum += roll(slot.min2, slot.max2);
            }
        } else {
            sum += roll(slot.min, slot.max);
        }
    }
    sum
}

/// What `unit` adds to `stat` (`0x14029dda0`), `base` being the character's base value of
/// it. `roll(lo, hi)` draws from a range (the game's random draw, which is `lo` when the
/// bounds are equal).
pub fn item_stat(t: &Tables, unit: &ItemUnit, stat: usize, base: i32, roll: &mut dyn FnMut(i32, i32) -> i32) -> i32 {
    if stat >= STATS {
        return 0;
    }
    let mut sum = 0;
    if stat < 9 {
        if let Some(row) = usize::try_from(unit.item).ok().and_then(|i| t.items.get(i)).filter(|r| r.equip) {
            sum = row.stats[stat];
        }
    }
    for e in unit.effects {
        sum += effect_amount(t, e.into(), stat, roll);
    }
    let mut total = (sum as f32 * ((unit.quality * 0.5 + 75.0) / 100.0)) as i32;
    if unit.word_e == 0 {
        let kinds = stat_trait_kinds(stat);
        for &tr in &unit.traits {
            let Some(tr) = usize::try_from(tr).ok().filter(|&i| i <= 0x18f).and_then(|i| t.traits.get(i)) else { continue };
            for slot in &tr.slots {
                if !kinds.contains(&slot.kind.as_str()) {
                    continue;
                }
                total += match slot.kind.as_str() {
                    "ITEM_SUB_BTL_ADD_USEEFFECT" => effect_amount(t, slot.min, stat, roll),
                    "ITEM_SUB_EVOLVE_STATUS" => roll(slot.min, slot.max) * base / 100,
                    _ => roll(slot.min, slot.max),
                };
            }
        }
    }
    total
}

/// The stats in use (`0x14032de50`): base + second block + what the equipped items add +
/// (for the protagonist) the bonus values from 4 on.
pub fn in_use(t: &Tables, base: &[i32; STATS], extra: &[i32; STATS], equipped: &[ItemUnit], bonus: Option<&[i32]>, roll: &mut dyn FnMut(i32, i32) -> i32) -> [i32; STATS] {
    std::array::from_fn(|k| {
        let mut v = base[k] + extra[k];
        for u in equipped {
            v += item_stat(t, u, k, base[k], roll);
        }
        if let Some(b) = bonus {
            v += b.get(4 + k).copied().unwrap_or(0).max(0);
        }
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::battle_data::SkillEffect;

    #[test]
    fn an_item_adds_its_row_effects_and_traits() {
        let acts = ["ACT_NONE", "ACT_EQUIP_ADD_PARAM_ATK", "ACT_EQUIP_ADD_RESIST"];
        let effects = vec![ItemEffect {
            name: "Atk+".into(),
            tag: String::new(),
            group: String::new(),
            effects: vec![SkillEffect { act: 1, element: None, min: 10, max: 10, min2: 0, max2: 0 }, SkillEffect { act: 2, element: None, min: 1, max: 1, min2: 7, max2: 7 }],
        }];
        let traits = vec![Trait { name: "Power".into(), group: String::new(), slots: vec![barrel_formats::battle_data::TraitSlot { kind: "ITEM_SUB_EVOLVE_STATUS".into(), min: 10, max: 10 }] }];
        let items = vec![ItemRow { equip: true, stats: [0, 0, 20, 0, 0, 0, 0, 0, 0] }];
        let t = Tables { items: &items, effects: &effects, traits: &traits, acts: &acts };
        let unit = ItemUnit { serial: 1, item: 0, quality: 50.0, traits: [0, -1, -1], word_e: 0, effects: [0, -1, -1, -1], word_18: 0, counts: [0; 4] };
        let mut roll = |lo: i32, _hi: i32| lo;
        // Attack: (20 + 10) × (50 × 0.5 + 75) / 100 = 30, plus 10% of a base of 40.
        assert_eq!(item_stat(&t, &unit, 2, 40, &mut roll), 34);
        // Water resistance (stat 6, element 1): 7 × 1.0.
        assert_eq!(item_stat(&t, &unit, 6, 0, &mut roll), 7);
        assert_eq!(item_stat(&t, &unit, 5, 0, &mut roll), 0);
    }
}
