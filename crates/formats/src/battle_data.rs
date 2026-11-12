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
    /// The area kind (record +0x20, read by `0x14015f840` for the gatherer `0x1401a4430`):
    /// 0 the target alone, 1 to 5 circles of 350 to 1,000 about it, 6 a line from the user
    /// (`fight::Battle::area`). Over the 577 records: 0 361, 2 117, 5 50, 1 9, 3 4, 6 4.
    /// (Word +0x14, read here before by mistake, is something else: 1 on the Floating
    /// Alchemy bombs.)
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
            // The area kind (`0x14015f840` reads +0x20; word 1, +0x14, is something else).
            spread: v(4)?,
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
    /// `potLv` (the record's +0x20): the level the Cost Bonus effect sums over an item's traits.
    pub level: i32,
    /// `point` (+0x24): the cost the trait screens show for it.
    pub point: i32,
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

/// The rows of `item_potential.xml`, in order (the trait numbers items hold). A slot's bounds
/// are numbers, or for `ITEM_SUB_BTL_ADD_USEEFFECT` `item_effect` tags (`ITEM_EFF_YOBI_072`),
/// read as their row numbers in `effect_tags` (-1 when not there).
pub fn traits(root: &crate::table::Element, effect_tags: &[&str]) -> Vec<Trait> {
    let int = |e: &crate::table::Element, k: &str| {
        let v = e.attr(k).unwrap_or("").trim();
        v.parse::<i32>().ok().or_else(|| v.starts_with("ITEM_EFF_").then(|| effect_tags.iter().position(|t| *t == v).map_or(-1, |i| i as i32))).unwrap_or(0)
    };
    root.children
        .iter()
        .map(|row| Trait {
            name: row.attr("PotName").unwrap_or("").to_string(),
            group: row.attr("PotGrpTag").unwrap_or("").to_string(),
            level: int(row, "potLv"),
            point: int(row, "point"),
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

/// The encounter groups' table in Escha's English executable (`0x140754bc0`, 226 × 0x38
/// bytes; `0x140050340` returns entry k, `0x1400501b0` lists the entries of a group).
pub const ENCOUNTER_GROUPS_EN: u64 = 0x1407_54bc0;
const ENCOUNTER_GROUP_COUNT: usize = 226;

/// One encounter group: who a field enemy's battle brings (`docs/tables.md`, "Encounter
/// groups").
#[derive(Clone, Debug, PartialEq)]
pub struct EncounterGroup {
    pub id: i32,
    /// Word 2: 0, 3 or 4, which the battle setup keeps at +0x3c (meaning not read yet).
    pub kind: i32,
    /// How many monsters, at least and at most (the battle rolls between them).
    pub count: (i32, i32),
    /// Up to three (monster, most of it) slots, filled in order until the count is reached.
    pub monsters: Vec<(i32, i32)>,
    /// Words 11 to 13: other monsters, which the battle setup lists apart at +0x48
    /// (reinforcements beside some bosses, by the look of it; how they join is not read yet).
    pub extra: Vec<i32>,
}

/// The encounter groups (none if the table is not where `at` says).
pub fn encounter_groups(pe: &Pe, at: u64) -> Vec<EncounterGroup> {
    let Some(start) = pe.offset(at) else { return vec![] };
    let data = pe.data;
    (0..ENCOUNTER_GROUP_COUNT)
        .map_while(|k| {
            let w = |i: usize| data.get(start + 0x38 * k + 4 * i..start + 0x38 * k + 4 * i + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
            Some(EncounterGroup {
                id: w(0)?,
                kind: w(2)?,
                count: (w(3)?, w(4)?),
                monsters: (0..3).filter_map(|s| Some((w(5 + 2 * s)?, w(6 + 2 * s)?))).filter(|&(m, _)| m >= 0).collect(),
                extra: (11..14).filter_map(w).filter(|&m| m >= 0).collect(),
            })
        })
        .collect()
}

/// The field enemies' builder in Escha's English executable (`clsFMDataEnemy`'s second
/// virtual, `0x14024a500`): it fills a table of 0x114 × 0x68 bytes on its stack field by
/// field, with immediates, then copies it into the enemy data (`0x140dfc1f0` + 0x10).
pub const FIELD_ENEMIES_BUILDER_EN: u64 = 0x1_4024_a500;
const FIELD_ENEMY_COUNT: usize = 0x114;
const FIELD_ENEMY_SIZE: usize = 0x68;

/// One enemy on the field maps (`docs/tables.md`, "Field enemies").
#[derive(Clone, Debug, PartialEq)]
pub struct FieldEnemy {
    /// The `Chara.xml` row it occupies on the map: a generic slot, "Enemy 1" (418) on, the
    /// bosses' "Enemy 30" (448); its look comes from its group's monsters.
    pub chara: i32,
    /// Word +4 (meaning not read yet).
    pub word4: i32,
    /// +8: a scale, 1 for most.
    pub scale: f32,
    /// +0xc and +0x10: conditions (-1 none), checked with `0x14035f2d0` before it appears.
    pub conditions: (i32, i32),
    /// +0x14: the area it appears in.
    pub area: i32,
    /// +0x1c: its kind (0 for most; 3 and 5 on some).
    pub kind: i32,
    /// +0x38 and +0x40: its field AI scripts (`Script/<name>.nut`), the second presumably for
    /// when it has noticed the player.
    pub ai: (String, String),
    /// +0x4c: the encounter group its battle draws from (`EncounterGroup::id`).
    pub group: i32,
    /// +0x50: where it stands.
    pub position: [f32; 3],
    /// The whole record's words, for the fields not named yet.
    pub words: [u32; FIELD_ENEMY_SIZE / 4],
}

/// The field enemies, rebuilt by replaying the builder's stores (it uses six instruction
/// forms: `mov dword [rsp+d], imm32`; `movss xmm0, [rip+r]`; `xorps xmm0, xmm0`;
/// `movss [rsp+d], xmm0`; `lea rax, [rip+r]`; `mov [rsp+d], rax`). None if the code is not
/// there.
pub fn field_enemies(pe: &Pe, builder: u64) -> Option<Vec<FieldEnemy>> {
    let start = pe.offset(builder)?;
    let code = pe.data.get(start..)?;
    let mut table = vec![0u32; FIELD_ENEMY_COUNT * FIELD_ENEMY_SIZE / 4];
    let mut pointers: std::collections::HashMap<usize, u64> = Default::default();
    let (mut xmm0, mut rax) = (0u32, 0u64);
    let u32_at = |i: usize| code.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let rip = |i: usize, len: usize, rel: u32| (builder as i64 + i as i64 + len as i64 + rel as i32 as i64) as u64;
    let store = |table: &mut Vec<u32>, d: u32, v: u32| {
        if let Some(o) = (d as usize).checked_sub(0x30) {
            if let Some(w) = table.get_mut(o / 4) {
                *w = v;
            }
        }
    };
    let mut i = 0;
    let mut started = false;
    while i < code.len() {
        let b = &code[i..];
        match b {
            [0xc7, 0x44, 0x24, d, ..] => {
                store(&mut table, *d as u32, u32_at(i + 4)?);
                i += 8;
            }
            [0xc7, 0x84, 0x24, ..] => {
                store(&mut table, u32_at(i + 3)?, u32_at(i + 7)?);
                i += 11;
            }
            [0xf3, 0x0f, 0x10, 0x05, ..] => {
                let at = rip(i, 8, u32_at(i + 4)?);
                xmm0 = u32_at(pe.offset(at)?.wrapping_sub(start))?;
                i += 8;
            }
            [0x0f, 0x57, 0xc0, ..] => {
                xmm0 = 0;
                i += 3;
            }
            [0xf3, 0x0f, 0x11, 0x44, 0x24, d, ..] => {
                store(&mut table, *d as u32, xmm0);
                i += 6;
            }
            [0xf3, 0x0f, 0x11, 0x84, 0x24, ..] => {
                store(&mut table, u32_at(i + 5)?, xmm0);
                i += 9;
            }
            [0x48, 0x8d, 0x05, ..] => {
                rax = rip(i, 7, u32_at(i + 3)?);
                i += 7;
            }
            [0x48, 0x89, 0x44, 0x24, d, ..] => {
                pointers.insert(*d as usize, rax);
                i += 5;
            }
            [0x48, 0x89, 0x84, 0x24, ..] => {
                pointers.insert(u32_at(i + 4)? as usize, rax);
                i += 8;
            }
            _ if started => break,
            _ => {
                // The prologue (push, the stack probe, sub rsp): skip a byte at a time.
                i += 1;
                continue;
            }
        }
        started = true;
    }
    let name = |d: usize| pointers.get(&d).and_then(|&va| pe.name_at(va)).unwrap_or("").to_string();
    let f = |w: u32| f32::from_bits(w);
    Some(
        (0..FIELD_ENEMY_COUNT)
            .map(|k| {
                let w: [u32; FIELD_ENEMY_SIZE / 4] = table[k * FIELD_ENEMY_SIZE / 4..(k + 1) * FIELD_ENEMY_SIZE / 4].try_into().unwrap();
                let base = 0x30 + k * FIELD_ENEMY_SIZE;
                FieldEnemy {
                    chara: w[0] as i32,
                    word4: w[1] as i32,
                    scale: f(w[2]),
                    conditions: (w[3] as i32, w[4] as i32),
                    area: w[5] as i32,
                    kind: w[7] as i32,
                    ai: (name(base + 0x38), name(base + 0x40)),
                    group: w[0x13] as i32,
                    position: [f(w[0x14]), f(w[0x15]), f(w[0x16])],
                    words: w,
                }
            })
            .filter(|e| e.chara != 0 || e.group != 0)
            .collect(),
    )
}

/// Escha's and Logy's synthesis skills: 68 records of 56 bytes (English executable; getter
/// `0x1400fd480`), each skill twice, for user 0 (Escha) and 1 (Logy). docs/tables.md,
/// "Synthesis skills".
pub const MIX_SKILLS_EN: u64 = 0x1_407b_5f40;
pub const MIX_SKILL_COUNT: usize = 68;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixSkill {
    pub name: String,
    /// The skill switch's case (`synthesis::Skill`); 7 to 17 are passives.
    pub kind: i32,
    pub params: [i32; 2],
    /// 0 Escha, 1 Logy.
    pub user: i32,
    /// The alchemy level it is learnt at; -1 for the two learnt otherwise.
    pub level: i32,
    /// Its cost in Fire, Water, Wind and Earth points.
    pub cost: [i32; 4],
    pub description: String,
}

pub fn mix_skills(pe: &Pe, table: u64) -> Option<Vec<MixSkill>> {
    (0..MIX_SKILL_COUNT)
        .map(|k| {
            let va = table + 56 * k as u64;
            let at = pe.offset(va)?;
            let d = pe.data;
            let w = |o: usize| i32_at(d, at + o);
            Some(MixSkill {
                name: text(pe, u64_at(d, at)?).unwrap_or_default(),
                kind: w(8)?,
                params: [w(12)?, w(16)?],
                user: w(24)?,
                level: w(28)?,
                cost: [w(32)?, w(36)?, w(40)?, w(44)?],
                description: text(pe, u64_at(d, at + 48)?).unwrap_or_default(),
            })
        })
        .collect()
}

/// The first `Chara.xml` row of the monsters: monster m (the monster table's order) is row
/// `MONSTER_CHARA_FIRST + m` (row 18 is the Poison Squirrel, monster 0).
pub const MONSTER_CHARA_FIRST: usize = 18;

/// Every `Chara.xml` row's `Radius`, the size a fighter of that row has in battle: the battle
/// data's 128-byte records (`0x141077ed0`, one per row) hold it at +0x64, where a fighter's
/// size (+0x358) is copied from (read from a running game: all 192 records read matched).
pub fn chara_radii(chara: &crate::table::Element) -> Vec<f32> {
    chara.children.iter().map(|r| r.attr("Radius").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)).collect()
}

/// A monster's size: its `Chara.xml` row's radius.
pub fn monster_size(radii: &[f32], monster: usize) -> Option<f32> {
    radii.get(MONSTER_CHARA_FIRST + monster).copied()
}

/// A party member's size: its `Chara.xml` row's radius. The party tables' order (Escha, Logy,
/// Awin, Threia, Reyfer, Lucille, Linca, Wilbell, Micie, Katla, Nio) follows the rows, except
/// that Nio (member 10) is row 11, after Marion.
pub fn party_size(radii: &[f32], member: usize) -> Option<f32> {
    radii.get(if member >= 10 { member + 1 } else { member }).copied()
}
