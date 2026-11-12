//! The battle data Escha & Logy DX compiles into its executable: the monsters with their stat
//! blocks, and the skills (`docs/tables.md`, "Monsters" and "Battle skills").
//!
//! The tables' addresses are the English executable's; [`monsters`] and [`skills`] check that
//! the records there look right and return `None` otherwise.

use crate::pe::Pe;

pub const MONSTERS_EN: u64 = 0x1_40d3_4810;
pub const MONSTER_COUNT: usize = 153;
const MONSTER_RECORD: usize = 48;
const BLOCK_WORDS: usize = 20;
const MAX_BLOCKS: usize = 16;

pub const SKILLS_EN: u64 = 0x1_40d5_1ed0;
/// The battle item table: 265 rows of 288 bytes from here to `0x140d2c690`, each starting with
/// its item id (the `itemData.xml` row; `0x1400e31a0` finds a row by it), then at +0xc, +0x10
/// and +0x14 its target word, area kind and action wait (`0x1402a25a0`, `0x1402a25e0`,
/// `0x1402a2620`).
pub const ITEM_BATTLE_EN: u64 = 0x1_40d1_9c70;
pub const ITEM_BATTLE_END_EN: u64 = 0x1_40d2_c690;
pub const ITEM_RECORD: usize = 288;
const SKILL_RECORD: usize = 96;
const MAX_SKILLS: usize = 1024;

pub const PARTY_GROWTH_EN: u64 = 0x1_40d8_f370;
/// The party members, in the executable's `PARTY_*` order (the getter `0x14032ce70` indexes
/// the blocks by party member number) (Escha, Logy, Awin, Threia, Reyfer,
/// Lucille, Linca, Wilbell, Micie, Katla, Nio).
pub const PARTY_COUNT: usize = 11;
pub const MAX_LEVEL: usize = 99;
const GROWTH_ROWS: usize = 100;

/// A party member's base stats at a level: HP, MP, attack, defence, speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelStats {
    pub hp: i32,
    pub mp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
}

/// Each party member's stats for levels 1 to 99 (`docs/tables.md`, "Party growth"): blocks of
/// 100 rows of five i32 (99 levels and a row of zeros), one per member.
pub fn party_growth(pe: &Pe, table: u64) -> Option<Vec<Vec<LevelStats>>> {
    let at = pe.offset(table)?;
    (0..PARTY_COUNT)
        .map(|c| {
            (0..MAX_LEVEL)
                .map(|l| {
                    let r = at + 20 * (GROWTH_ROWS * c + l);
                    let w = |i: usize| i32_at(pe.data, r + 4 * i);
                    let s = LevelStats { hp: w(0)?, mp: w(1)?, attack: w(2)?, defense: w(3)?, speed: w(4)? };
                    (s.hp > 0).then_some(s)
                })
                .collect()
        })
        .collect()
}

/// One of a monster's stat blocks (the same monster at another strength).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatBlock {
    pub level: i32,
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub resist: [i32; 4],
    /// Word 10, battle stat 12 (not named yet).
    pub stat12: i32,
    /// The skills it can use at this level, each with the event that plays it (from the
    /// second of the block's four pointers, words 14 and 15: `{i32 skill, i32 0, pointer to
    /// the event name}` records, ended by skill -1).
    pub skills: Vec<(u32, String)>,
    /// How it picks its action (`docs/tables.md`, "Monster actions"): the rules in order,
    /// and each of the 9 action slots' index into `skills`.
    pub rules: Vec<Rule>,
    pub slots: [i32; 9],
}

/// One of a monster's action rules (32 bytes: eight i32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Word 0 (2 or 3; not read by the chooser).
    pub kind: i32,
    /// Word 1: who the action is aimed at (handed on with the slot).
    pub target: i32,
    /// Words 2 and 3: the condition and its value.
    pub condition: i32,
    pub value: i32,
    /// Word 4: the chance, in percent, once the condition holds.
    pub chance: i32,
    /// Word 5: the action slot it picks.
    pub slot: i32,
    /// Word 6: how many times the slot may be used in a battle (-1: no limit).
    pub limit: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monster {
    pub name: String,
    pub family: u32,
    pub boss: bool,
    pub exp: i32,
    pub money: i32,
    pub blocks: Vec<StatBlock>,
}

/// One effect slot of a skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillEffect {
    /// The `ACT_*` number.
    pub act: u32,
    /// The element, 0 to 3 (Fire, Water, Wind, Earth: Bomb 0, Ice Bomb 1, Lightning Bomb
    /// 2); `None` for physical (-1). The effect's second word, the battle's effect +4.
    pub element: Option<u8>,
    pub min: i32,
    pub max: i32,
    /// The second pair (rolled into the effect's second value).
    pub min2: i32,
    pub max2: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub category: String,
    /// The action's wait before speed (600 for the normal attack).
    pub wait: i32,
    /// Whom it is aimed at (record +0x10, `0x14015f900`; the target picker `0x1401abf70`):
    /// 1 and 3 a character on the user's side (3: Howling's self-buff, heals are 1), 2 one on
    /// the other side (attacks, debuffs), 0 (passives) and 4 no side.
    pub target: i32,
    /// Record +0x14: 1 only on the Floating Alchemy bombs (an area, presumably; not read).
    pub spread: i32,
    pub effects: Vec<SkillEffect>,
    pub description: String,
}

fn i32_at(d: &[u8], at: usize) -> Option<i32> {
    d.get(at..at + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()))
}

fn u64_at(d: &[u8], at: usize) -> Option<u64> {
    d.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
}

/// A display string (names and descriptions have spaces, unlike the enum tags `Pe::name_at`
/// reads): up to 512 bytes of UTF-8 before a 0.
fn text(pe: &Pe, va: u64) -> Option<String> {
    let rest = pe.data.get(pe.offset(va)?..)?;
    let end = rest.iter().take(512).position(|&b| b == 0)?;
    let s = std::str::from_utf8(&rest[..end]).ok()?;
    (!s.is_empty() && !s.chars().any(char::is_control)).then(|| s.to_string())
}

/// The monsters, by monster number.
pub fn monsters(pe: &Pe, table: u64) -> Option<Vec<Monster>> {
    let at = pe.offset(table)?;
    let d = pe.data;
    let mut out = Vec::with_capacity(MONSTER_COUNT);
    for k in 0..MONSTER_COUNT {
        let r = at + MONSTER_RECORD * k;
        let name = text(pe, u64_at(d, r)?)?;
        let stats = pe.offset(u64_at(d, r + 40)?)?;
        let mut blocks = Vec::new();
        for b in 0..MAX_BLOCKS {
            let w = |i: usize| i32_at(d, stats + 4 * (BLOCK_WORDS * b + i));
            let level = w(0)?;
            if level <= 0 {
                break;
            }
            blocks.push(StatBlock {
                level,
                hp: w(1)?,
                attack: w(2)?,
                defense: w(3)?,
                speed: w(4)?,
                resist: [w(6)?, w(7)?, w(8)?, w(9)?],
                stat12: w(10)?,
                skills: u64_at(d, stats + 4 * (BLOCK_WORDS * b + 14))
                    .and_then(|list| pe.offset(list))
                    .map(|list| {
                        (0..16)
                            .map_while(|i| {
                                let skill = i32_at(d, list + 16 * i)?;
                                let event = u64_at(d, list + 16 * i + 8).and_then(|v| text(pe, v)).unwrap_or_default();
                                (skill >= 0).then_some((skill as u32, event))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                rules: Vec::new(),
                slots: [-1; 9],
            });
            // The first pointer (words 12, 13): {pointer to the rules, i32 slot[9]}; the rules
            // are 32-byte records ended by one whose first word is -1.
            if let Some(header) = u64_at(d, stats + 4 * (BLOCK_WORDS * b + 12)).and_then(|h| pe.offset(h)) {
                let block = blocks.last_mut().expect("just pushed");
                for (k, slot) in block.slots.iter_mut().enumerate() {
                    *slot = i32_at(d, header + 8 + 4 * k).unwrap_or(-1);
                }
                if let Some(list) = u64_at(d, header).and_then(|l| pe.offset(l)) {
                    block.rules = (0..32)
                        .map_while(|i| {
                            let w = |k: usize| i32_at(d, list + 32 * i + 4 * k);
                            let kind = w(0)?;
                            (kind != -1).then_some(())?;
                            Some(Rule { kind, target: w(1)?, condition: w(2)?, value: w(3)?, chance: w(4)?, slot: w(5)?, limit: w(6)? })
                        })
                        .collect();
                }
            }
        }
        out.push(Monster {
            name,
            family: i32_at(d, r + 8)? as u32,
            boss: i32_at(d, r + 12)? == 1,
            exp: i32_at(d, r + 16)?,
            money: i32_at(d, r + 20)?,
            blocks,
        });
    }
    Some(out)
}

/// The skills, by `SKILL_*` number, up to the record without a name.
pub fn skills(pe: &Pe, table: u64) -> Option<Vec<Skill>> {
    let at = pe.offset(table)?;
    let d = pe.data;
    let mut out = Vec::new();
    for k in 0..MAX_SKILLS {
        let r = at + SKILL_RECORD * k;
        let Some(name) = u64_at(d, r).and_then(|p| text(pe, p)) else { break };
        let v = |i: usize| i32_at(d, r + 16 + 4 * i);
        let mut effects = Vec::new();
        for slot in [6usize, 12] {
            if let Ok(act) = u32::try_from(v(slot)?) {
                let element = u8::try_from(v(slot + 1)?).ok().filter(|&e| e < 4);
                effects.push(SkillEffect { act, element, min: v(slot + 2)?, max: v(slot + 3)?, min2: v(slot + 4)?, max2: v(slot + 5)? });
            }
        }
        out.push(Skill {
            name,
            category: u64_at(d, r + 8).and_then(|p| text(pe, p)).unwrap_or_default(),
            wait: v(2)?,
            target: v(0)?,
            spread: v(1)?,
            effects,
            description: u64_at(d, r + 88).and_then(|p| text(pe, p)).unwrap_or_default(),
        });
    }
    (!out.is_empty()).then_some(out)
}

/// Where each party member's skill list is (English Escha executable): 11 pointers, one per
/// member in the party tables' order, read by `0x14032f200`.
pub const PARTY_SKILLS_EN: u64 = 0x1_40d9_6f90;
pub const PARTY_MEMBERS: usize = 11;

/// One entry of a party member's skill list (12 bytes, ended by skill -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartySkill {
    /// The skill (an index into the skill table).
    pub skill: u32,
    /// +4: its place among the member's numbered skills (1 to 10), -1 for the normal attack,
    /// the support actions (AC Attack, Guard, Move, Swap) and Double Draw.
    pub order: i32,
    /// +8: the battle command it sits on (0 the attack, 1 to 3 the skills, 4 the AC special,
    /// 5 and 6 the passives, 7 the finishing attack, 8 and on the support actions).
    pub command: u8,
    /// +9: the level it is learnt at; `None` (0xff) when not learnt by level.
    pub level: Option<u8>,
    /// +0xa: its MP cost before reductions (`0x14032f2f9`, a signed byte).
    pub mp: i32,
}

/// Each member's skill list.
pub fn party_skills(pe: &Pe, table: u64) -> Option<Vec<Vec<PartySkill>>> {
    let d = pe.data;
    (0..PARTY_MEMBERS)
        .map(|m| {
            let mut at = pe.offset(u64_at(d, pe.offset(table)? + 8 * m)?)?;
            let mut out = Vec::new();
            loop {
                let skill = i32_at(d, at)?;
                if skill == -1 || out.len() > 64 {
                    break;
                }
                let b = d.get(at + 8..at + 12)?;
                out.push(PartySkill { skill: skill as u32, order: i32_at(d, at + 4)?, command: b[0], level: (b[1] != 0xff).then_some(b[1]), mp: b[2] as i8 as i32 });
                at += 12;
            }
            Some(out)
        })
        .collect()
}

/// One row of `Saves/item/item_effect.xml`: an item effect's battle effects, as the loader
/// (`0x1400e8b00` and on, English Escha executable) reads it: `actTag` with `attributeTag`,
/// `min_1`..`max_1` (the amount) and `min_2`..`max_2` (the second value), then `actTag2` with
/// `attributeTag2`, `min_3`..`max_4`. The rows' unsuffixed `min` and `max` and the Japanese
/// columns are not read by the game.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemEffect {
    pub name: String,
    pub tag: String,
    /// `g_tag`, the effect's group (`ITEM_EFF_GROUP_DAMAGE`, ...).
    pub group: String,
    /// The two effect slots that are not `ACT_NONE`, in the shape of a skill's.
    pub effects: Vec<SkillEffect>,
}

/// The rows of `item_effect.xml`, in order (they are the `ITEM_EFF_*` enum); `acts` names the
/// `ACT_*` numbers (`Pe::enum_names("ACT_DAMAGE_HP")`). An element tag outside Fire, Water,
/// Wind and Earth (`ELEMENT_PHYSICS`) is physical.
pub fn item_effects(root: &crate::table::Element, acts: &[&str]) -> Vec<ItemEffect> {
    let element = |t: Option<&str>| match t? {
        "ELEMENT_FIRE" => Some(0),
        "ELEMENT_WATER" => Some(1),
        "ELEMENT_WIND" => Some(2),
        "ELEMENT_EARTH" => Some(3),
        _ => None,
    };
    root.children
        .iter()
        .map(|row| {
            let n = |k: &str| row.attr(k).and_then(|v| v.parse().ok()).unwrap_or(0);
            let mut effects = Vec::new();
            for (act, attr, lo, hi, lo2, hi2) in [("actTag", "attributeTag", "min_1", "max_1", "min_2", "max_2"), ("actTag2", "attributeTag2", "min_3", "max_3", "min_4", "max_4")] {
                let Some(act) = row.attr(act).and_then(|t| acts.iter().position(|a| *a == t)) else { continue };
                effects.push(SkillEffect { act: act as u32, element: element(row.attr(attr)), min: n(lo), max: n(hi), min2: n(lo2), max2: n(hi2) });
            }
            ItemEffect {
                name: row.attr("ItemEffectName").unwrap_or_default().to_string(),
                tag: row.attr("ItemEffTag").unwrap_or_default().to_string(),
                group: row.attr("g_tag").unwrap_or_default().to_string(),
                effects,
            }
        })
        .collect()
}

#[cfg(test)]
mod item_effect_tests {
    use super::*;

    #[test]
    fn an_item_effect_row() {
        let xml = r#"<Root><item_effect ItemEffectName="Fire Damage L" ItemEffTag="ITEM_EFF_DAMAGE_FIRE_1" g_tag="ITEM_EFF_GROUP_DAMAGE" attributeTag="ELEMENT_FIRE" actTag="ACT_DAMAGE_HP" min_1="23" max_1="31" min_2="0" max_2="0" attributeTag2="ELEMENT_PHYSICS" actTag2="ACT_NONE" min="21" max="35"/></Root>"#;
        let root = crate::table::parse_str(xml).unwrap();
        let rows = item_effects(&root, &["ACT_DAMAGE_HP", "ACT_X"]);
        assert_eq!(rows[0].effects, [SkillEffect { act: 0, element: Some(0), min: 23, max: 31, min2: 0, max2: 0 }]);
        assert_eq!(rows[0].group, "ITEM_EFF_GROUP_DAMAGE");
    }
}

/// An item row's battle words: whom it is aimed at (as a skill's target word), its area kind
/// (as a skill's +0x20) and its action's wait before speed (300 to 900 on battle items, 0 on
/// the rest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemBattle {
    /// The item (its `itemData.xml` row).
    pub item: i32,
    pub target: i32,
    pub area: i32,
    pub wait: i32,
}

/// Every row of the battle item table, from `table` to `end`.
pub fn item_battles(pe: &Pe, table: u64, end: u64) -> Vec<ItemBattle> {
    let mut out = Vec::new();
    let mut va = table;
    while va + ITEM_RECORD as u64 <= end {
        let Some(at) = pe.offset(va) else { break };
        let w = |k: usize| pe.data.get(at + 4 * k..at + 4 * k + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let (Some(item), Some(target), Some(area), Some(wait)) = (w(0), w(3), w(4), w(5)) else { break };
        out.push(ItemBattle { item, target, area, wait });
        va += ITEM_RECORD as u64;
    }
    out
}

/// One trait (`item_potential.xml`, the 0x58-byte records `0x14029db00` reads): its name,
/// group and two slots, each an `ITEM_SUB_*` kind (`PotBase`, `PotBase2`) with the range its
/// value is rolled from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trait {
    pub name: String,
    pub group: String,
    pub slots: Vec<TraitSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitSlot {
    /// The `ITEM_SUB_*` tag (`ITEM_SUB_ITM_CHANGE_POWER` ...); `ITEM_SUB_BASE_NONE` slots are
    /// left out.
    pub kind: String,
    pub min: i32,
    pub max: i32,
}

/// The rows of `item_potential.xml`, in order (the trait numbers items hold).
pub fn traits(root: &crate::table::Element) -> Vec<Trait> {
    let int = |e: &crate::table::Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
    root.children
        .iter()
        .map(|row| Trait {
            name: row.attr("PotName").unwrap_or("").to_string(),
            group: row.attr("PotGrpTag").unwrap_or("").to_string(),
            slots: [("PotBase", "min", "max"), ("PotBase2", "min2", "max2")]
                .iter()
                .filter_map(|&(k, lo, hi)| {
                    let kind = row.attr(k).filter(|t| !t.is_empty() && *t != "ITEM_SUB_BASE_NONE")?;
                    Some(TraitSlot { kind: kind.to_string(), min: int(row, lo), max: int(row, hi) })
                })
                .collect(),
        })
        .collect()
}

/// The per-act table (`0x14002aa10` asks its flags, `0x14002aa40` its limit): 20-byte records,
/// one per `ACT_*` number from 0 to 0xe9 (record -1 sits before it), each 12 flag bytes, the
/// limit an item's bonus is cut to (9,999 for most) and a last word.
pub const ACTS_EN: u64 = 0x1_40ce_9590;
pub const ACT_COUNT: usize = 0xea;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActInfo {
    /// Flag 3: an item's trait bonus applies (`0x1401bd8ee`); flag 4: its `extra`.
    pub flags: [bool; 12],
    pub limit: i32,
    pub last: i32,
}

/// Every act's flags and limit.
pub fn acts(pe: &Pe, table: u64) -> Vec<ActInfo> {
    (0..ACT_COUNT)
        .map_while(|k| {
            let at = pe.offset(table + 20 * k as u64)?;
            let b = pe.data.get(at..at + 20)?;
            let mut flags = [false; 12];
            for (f, &v) in flags.iter_mut().zip(&b[..12]) {
                *f = v != 0;
            }
            let word = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            Some(ActInfo { flags, limit: word(12), last: word(16) })
        })
        .collect()
}

/// The `ITEM_SUB_*` names in enum order (trait kinds: 0 `ITEM_SUB_ITM_CHANGE_POWER`, 25
/// `ITEM_SUB_BTL_RANK_BONUS`, 39 `ITEM_SUB_CHANGE_ACTUAL_POWER` ...), from the executable.
pub fn trait_kinds<'a>(pe: &Pe<'a>) -> Vec<&'a str> {
    pe.enum_names("ITEM_SUB_ITM_CHANGE_POWER")
        .map(|(names, first)| names[first..].iter().copied().take_while(|n| n.starts_with("ITEM_SUB_")).collect())
        .unwrap_or_default()
}
