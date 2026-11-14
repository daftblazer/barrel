//! Assignment bonuses (`Saves_EN/bonus/Bonus.xml`, applied by `0x14002f310`): each row is up to
//! five actions, `BonusEntryN` with `EntryNPrm1..3`. Most add to one of the 128 bonus values
//! the save keeps (`save::Summary::bonus_values`, `0x141079de0`); the others call into the
//! rest of the game (money, event flags, items, costumes). See docs/tables.md, "Bonuses".

use barrel_formats::table::Element;

/// The action kinds in the executable's order (its `BonusEntry_*` names; the switch's case
/// numbers).
pub const KINDS: [&str; 55] = [
    "GetMoney", "Event_Tag_On", "Event_Flag_On", "Event_Flag_Add", "Get_Trophy", "Add_Basket", "Add_Work_Point",
    "Get_Adv_Exp_Rate", "Add_Adv_Lv_Limit", "Get_Mix_Exp_Rate", "Get_Recipe", "Get_Mix_Skill", "Get_Mix_Bonus",
    "Add_Status_ALL", "Add_Status_HP", "Add_Status_MP", "Add_Status_ATK", "Add_Status_DEF", "Add_Status_SPD",
    "Reduce_FIRE", "Reduce_WATER", "Reduce_WIND", "Reduce_EIRTH", "Reduce_CONDITION", "Add_Power_Crct", "Add_Ap_Crct",
    "Reduce_Skill_Cost", "Add_Critical_Rate", "Add_Critical_Crct", "Add_Avoid_Crct", "Add_Act_Crct", "Add_Reduct_Crct",
    "Add_Fire_Dmg", "Add_Water_Dmg", "Add_Wind_Dmg", "Add_Earth_Dmg", "Add_Death_Avoid", "Add_Probability_Crct",
    "Add_Poison", "Add_Death", "Add_Darkness", "Add_Weak", "Add_Slow", "Add_Exp_Rate", "Add_Money_Rate", "Add_Drop_Rate",
    "Add_Item_Slot", "Reduce_Collect_Time", "Reduce_Move_Time", "Rec_Move_Fatigue", "Add_Sell_Cost", "Reduce_Buy_Cost",
    "GetGift", "Get_Explore_Equip", "Get_Costume",
];

/// The bonus value a kind adds its first parameter to, if it is one of those: the adventure
/// experience rate (1), the adventure level limit (2, a base of 50 capped at 99 where read),
/// the synthesis experience rate (3), HP, MP, attack, defense, speed (4 to 8), the
/// resistances, corrections and battle rates after them (9 to 35), the move fatigue (39) and
/// the sell and buy rates (40, 41).
pub fn value_slot(kind: usize) -> Option<usize> {
    match kind {
        7 => Some(1),
        8 => Some(2),
        9 => Some(3),
        14..=45 => Some(kind - 10),
        49..=51 => Some(kind - 10),
        _ => None,
    }
}

/// The synthesis bonus kinds (`MIX_SKILL_BASE_TYPE_*`, in the executable's order), the first
/// parameter of `Get_Mix_Bonus`.
pub const MIX_TYPES: [&str; 26] = [
    "FIELD_ADD", "EFFECT_ADD", "ATT_ADD", "ADD_CARD", "RESET", "EXP_ADD", "ATT_CHANGE", "INIT_COST_ADD", "USE_SKILL_DEC",
    "GET_PP_ADD", "INIT_PP_ADD", "MIX_DAY_DEC", "USE_ITEM_ADD", "PASSIVE_EFFECT_ADD", "INSERT_MAT_POSSIBLE",
    "FIELD_ELEM_POSSIBLE", "BATON_POSSIBLE", "POT_LEVEL_POSSIBLE", "CUT_ITEM_COST_DATE", "EFFECT_FORCE_ADD",
    "CUT_COST_POINT", "COUNT_USE_ADD", "ADD_COST_POINT", "ADD_ALL", "ADD_ITEM_ALL", "CONVERT_ATT_TO_EFF",
];

/// The slot of the 544 synthesis bonus values (`bonus/mixvalue`, `0x1410799a0`) a kind (and,
/// for the per-category kinds, an item category number) keeps its total in (`0x14036b870`).
pub fn mix_slot(kind: usize, category: i32) -> Option<usize> {
    let slot = match kind {
        0 => 0,
        1 => 1,
        2 => 3,
        5 => 4,
        7 => 5,
        8 => 6,
        9 => 7 + category,
        10 => 0x87,
        11 => 0x88,
        12 => 0x89 + category,
        13 => 0x189 + category,
        _ => return None,
    };
    usize::try_from(slot).ok().filter(|&s| s < 0x220)
}

/// Whether a kind is kept per item category (`GET_PP_ADD`, `USE_ITEM_ADD`,
/// `PASSIVE_EFFECT_ADD`): then `Get_Mix_Bonus`'s second parameter is the category and the
/// third the amount (`0x14036c400`), else the second is the amount.
pub fn mix_per_category(kind: usize) -> bool {
    matches!(kind, 9 | 12 | 13)
}

/// One action: its kind (an index into [`KINDS`]) and parameters as written (numbers, or
/// tags the game resolves through its enums).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub kind: usize,
    pub params: [Option<String>; 3],
}

impl Action {
    fn number(&self, k: usize) -> Option<i32> {
        self.params[k].as_deref()?.trim().parse().ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bonus {
    pub name: String,
    pub text: String,
    pub actions: Vec<Action>,
}

/// The table's rows, in order (an assignment's `Bonus` names one by its `Name`). Actions
/// whose kind is not one of [`KINDS`] are dropped.
pub fn bonuses(table: &Element) -> Vec<Bonus> {
    table
        .children
        .iter()
        .map(|e| Bonus {
            name: e.attr("Name").unwrap_or_default().to_string(),
            text: e.attr("Text").unwrap_or_default().to_string(),
            actions: (1..=5)
                .filter_map(|n| {
                    let kind = e.attr(&format!("BonusEntry{n}"))?.strip_prefix("BonusEntry_")?;
                    let kind = KINDS.iter().position(|k| *k == kind)?;
                    let params = std::array::from_fn(|p| e.attr(&format!("Entry{n}Prm{}", p + 1)).map(str::to_string));
                    Some(Action { kind, params })
                })
                .collect(),
        })
        .collect()
}

/// The lasting state the value-type actions change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The 128 bonus values.
    pub values: Vec<i32>,
    /// The 17 gathering-time reductions (`bonus/collectTimeBonus`).
    pub collect_time: Vec<u8>,
    /// The 544 synthesis bonus values (`bonus/mixvalue`), read through `0x14036c3d0`.
    pub mix: Vec<i32>,
}

/// Applies `bonus`'s value-type actions to `state` and returns the others for the caller
/// (money, flags, items: the systems that own them). `Reduce_Collect_Time` adds its second
/// parameter to one gathering slot (its first), or to all 17 when that is -1.
/// `Reduce_Move_Time` does nothing in the game (its case falls through).
///
/// `Get_Mix_Bonus` adds to a synthesis bonus value; `categories` names the item category
/// numbers (the `ITEM_CATEGORY_*` enum from `ITEM_CATEGORY_PLANT` on).
pub fn apply<'b>(state: &mut State, bonus: &'b Bonus, categories: &[&str]) -> Vec<&'b Action> {
    state.values.resize(state.values.len().max(128), 0);
    state.mix.resize(state.mix.len().max(0x220), 0);
    state.collect_time.resize(state.collect_time.len().max(17), 0);
    let mut rest = Vec::new();
    for a in &bonus.actions {
        if let Some(slot) = value_slot(a.kind) {
            state.values[slot] += a.number(0).unwrap_or(0);
        } else if KINDS[a.kind] == "Reduce_Collect_Time" {
            let add = a.number(1).unwrap_or(0) as u8;
            match a.number(0) {
                Some(-1) | None => state.collect_time.iter_mut().for_each(|t| *t = t.wrapping_add(add)),
                Some(i) => {
                    if let Some(t) = usize::try_from(i).ok().and_then(|i| state.collect_time.get_mut(i)) {
                        *t = t.wrapping_add(add);
                    }
                }
            }
        } else if KINDS[a.kind] == "Get_Mix_Bonus" {
            let Some(kind) = a.params[0].as_deref().and_then(|t| t.strip_prefix("MIX_SKILL_BASE_TYPE_")).and_then(|t| MIX_TYPES.iter().position(|m| *m == t)) else {
                // The six "Synth Skill Frame" rows name no kind (what the game reads then is not
                // checked; one save's FIELD_ADD total of 2 suggests 1 each).
                rest.push(a);
                continue;
            };
            let (category, amount) = if mix_per_category(kind) {
                let c = a.params[1].as_deref().and_then(|t| categories.iter().position(|c| *c == t)).map_or(-1, |c| c as i32);
                (c, a.number(2).unwrap_or(0))
            } else {
                (0, a.number(1).unwrap_or(0))
            };
            if let Some(slot) = mix_slot(kind, category).filter(|_| category >= 0) {
                state.mix[slot] += amount;
            }
        } else if KINDS[a.kind] != "Reduce_Move_Time" {
            rest.push(a);
        }
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_add_to_their_values() {
        let xml = br#"<Root><Bonus Name="Body Study" BonusEntry1="BonusEntry_Add_Status_HP" Entry1Prm1="5" BonusEntry2="BonusEntry_Reduce_Collect_Time" Entry2Prm1="-1" Entry2Prm2="2" BonusEntry3="BonusEntry_GetMoney" Entry3Prm1="500"/></Root>"#;
        let table = barrel_formats::table::parse(xml).unwrap();
        let list = bonuses(&table);
        assert_eq!(list[0].actions.len(), 3);
        let mut s = State::default();
        let rest = apply(&mut s, &list[0], &[]);
        assert_eq!(s.values[4], 5);
        assert!(s.collect_time.iter().all(|&t| t == 2));
        assert_eq!(rest.len(), 1);
        assert_eq!(KINDS[rest[0].kind], "GetMoney");
        assert_eq!(value_slot(51), Some(41));
    }

    #[test]
    fn synthesis_bonuses_fill_their_slots() {
        let xml = br#"<Root><Bonus Name="Bomb Study" BonusEntry1="BonusEntry_Get_Mix_Bonus" Entry1Prm1="MIX_SKILL_BASE_TYPE_USE_ITEM_ADD" Entry1Prm2="ITEM_CATEGORY_BOMB" Entry1Prm3="1" BonusEntry2="BonusEntry_Get_Mix_Bonus" Entry2Prm1="MIX_SKILL_BASE_TYPE_INIT_COST_ADD" Entry2Prm2="10"/></Root>"#;
        let table = barrel_formats::table::parse(xml).unwrap();
        let mut s = State::default();
        apply(&mut s, &bonuses(&table)[0], &["ITEM_CATEGORY_PLANT", "ITEM_CATEGORY_BOMB"]);
        assert_eq!(s.mix[0x89 + 1], 1);
        assert_eq!(s.mix[5], 10);
    }
}
