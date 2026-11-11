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
            });
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
                effects.push(SkillEffect { act, min: v(slot + 2)?, max: v(slot + 3)?, min2: v(slot + 4)?, max2: v(slot + 5)? });
            }
        }
        out.push(Skill {
            name,
            category: u64_at(d, r + 8).and_then(|p| text(pe, p)).unwrap_or_default(),
            wait: v(2)?,
            effects,
            description: u64_at(d, r + 88).and_then(|p| text(pe, p)).unwrap_or_default(),
        });
    }
    (!out.is_empty()).then_some(out)
}
