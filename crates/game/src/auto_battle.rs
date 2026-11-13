//! A field encounter fought out without the battle screen: the encounter group's monsters drawn
//! as the game draws them (`battle::encounter_monsters`), against party members at given levels,
//! by [`crate::skirmish`] (each member standing in for the player). The step between touching an
//! enemy in the field and the played battle screen.
//!
//! Not yet the game's: the party members' stats their base stats at their level (no equipment),
//! HP and MP full, no battle items; everything [`crate::skirmish`] leaves out.

use barrel_formats::battle_data::{self as bd, ActInfo, EncounterGroup, ItemEffect, LevelStats, Monster, PartySkill, Skill};
use barrel_formats::pe::Pe;

use crate::fight::Event;
pub use crate::skirmish::Command;
use crate::rng::Rng;

/// The party members' names by character number.
pub const MEMBER_NAMES: [&str; 11] = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
/// The events a battle may take before it is called off.
const MAX_EVENTS: usize = 1000;
/// A monster's size when `Chara.xml` gives none.
const MONSTER_SIZE: f32 = 100.0;

/// The tables a battle reads, loaded once.
pub struct Tables {
    monsters: Vec<Monster>,
    skills: Vec<Skill>,
    lists: Vec<Vec<PartySkill>>,
    growth: Vec<Vec<LevelStats>>,
    groups: Vec<EncounterGroup>,
    acts: Vec<ActInfo>,
    kinds: Vec<String>,
    effects: Vec<ItemEffect>,
    radii: Vec<f32>,
}

impl Tables {
    /// From the English executable, `item_effect.xml`'s table (`effects_xml`) and `Chara.xml`'s
    /// (`chara`, for the fighters' sizes).
    pub fn load(pe: &Pe, effects_xml: &barrel_formats::table::Element, chara: Option<&barrel_formats::table::Element>) -> Result<Tables, String> {
        let (act_names, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
        Ok(Tables {
            monsters: bd::monsters(pe, bd::MONSTERS_EN).ok_or("no monster table")?,
            skills: bd::skills(pe, bd::SKILLS_EN).ok_or("no skill table")?,
            lists: bd::party_skills(pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists")?,
            growth: bd::party_growth(pe, bd::PARTY_GROWTH_EN).ok_or("no party growth table")?,
            groups: bd::encounter_groups(pe, bd::ENCOUNTER_GROUPS_EN),
            acts: bd::acts(pe, bd::ACTS_EN),
            kinds: bd::trait_kinds(pe).into_iter().map(str::to_string).collect(),
            effects: bd::item_effects(effects_xml, &act_names),
            radii: chara.map(bd::chara_radii).unwrap_or_default(),
        })
    }
}

/// How a battle went: the fighters' names (the party first), what happened and the winning
/// side (0 the party; none when it was called off).
#[derive(Clone, Debug)]
pub struct Outcome {
    pub names: Vec<String>,
    /// Each fighter's HP and MP as the battle starts (all full): HP, max HP, MP, max MP.
    pub start: Vec<[i32; 4]>,
    pub events: Vec<Event>,
    pub winner: Option<u8>,
}

impl Outcome {
    /// What happened, a line per event and one for the end.
    pub fn lines(&self) -> Vec<String> {
        let name = |k: &usize| self.names.get(*k).map_or("?", String::as_str);
        let mut out: Vec<String> = self
            .events
            .iter()
            .map(|e| match e {
                Event::Felled { who, skill, target } => format!("{} uses {skill} on {}: felled", name(who), name(target)),
                Event::Poisoned { who, damage, hp_left } => format!("{} takes {damage} poison damage ({hp_left} HP left)", name(who)),
                Event::Acted { who, skill, target, damage, hp_left, .. } => format!("{} uses {skill} on {}: {damage} damage ({hp_left} HP left)", name(who), name(target)),
                Event::Healed { who, skill, target, amount, hp_left, .. } => format!("{} uses {skill} on {}: heals {amount} ({hp_left} HP)", name(who), name(target)),
            })
            .collect();
        out.push(match self.winner {
            Some(0) => "the party wins".into(),
            Some(_) => "the monsters win".into(),
            None => format!("no winner after {} events", self.events.len()),
        });
        out
    }
}

/// Encounter group `group`'s monsters (by its id) drawn as the game draws them, each with its
/// size (`Chara.xml` radius).
pub fn draw(t: &Tables, group: i32, rng: &mut Rng) -> Result<Vec<(usize, f32)>, String> {
    let g = t.groups.iter().find(|g| g.id == group).ok_or_else(|| format!("no encounter group {group}"))?;
    Ok(crate::battle::encounter_monsters(g, rng).into_iter().map(|m| (m as usize, bd::monster_size(&t.radii, m as usize).unwrap_or(MONSTER_SIZE))).collect())
}

/// Encounter group `group` (its id, as a field enemy names it) against `party`, each a
/// (character number, level).
pub fn fight(t: &Tables, party: &[(usize, i32)], group: i32, rng: &mut Rng) -> Result<Outcome, String> {
    let drawn: Vec<usize> = draw(t, group, rng)?.into_iter().map(|(m, _)| m).collect();
    fight_drawn(t, party, &drawn, rng)
}

/// Monsters already drawn (`draw`) against `party`: the battle scatters them with `rng` first
/// (`battle::scatter`), so a copy of `rng` taken now places them where the battle does.
pub fn fight_drawn(t: &Tables, party: &[(usize, i32)], drawn: &[usize], rng: &mut Rng) -> Result<Outcome, String> {
    fight_with(t, party, drawn, rng, None).map(|(o, _)| o)
}

fn fight_with(t: &Tables, party: &[(usize, i32)], drawn: &[usize], rng: &mut Rng, commands: Option<&[Command]>) -> Result<(Outcome, Option<Turn>), String> {
    let mut names = Vec::new();
    let mut start = Vec::new();
    let mut members = Vec::new();
    for &(c, level) in party {
        let stats = t.growth.get(c).and_then(|l| l.get((level as usize).wrapping_sub(1))).ok_or_else(|| format!("character {c}: no growth row for level {level}"))?;
        let block = crate::duel::party_block(level, stats);
        let list = t.lists.get(c).ok_or_else(|| format!("character {c}: no skill list"))?;
        let mut member = crate::skirmish::Member::new(block, stats.mp, list, &t.skills).ok_or("no party attack")?;
        member.size = bd::party_size(&t.radii, c).unwrap_or(0.0);
        start.push([stats.hp, stats.hp, stats.mp, stats.mp]);
        members.push(member);
        names.push(MEMBER_NAMES.get(c).map_or_else(|| format!("member {c}"), |n| n.to_string()));
    }
    let normal = t.skills.get(1).ok_or("no skill 1")?;
    let mut foes = Vec::new();
    for &k in drawn {
        let m = t.monsters.get(k).filter(|m| !m.blocks.is_empty()).ok_or_else(|| format!("monster {k}: none"))?;
        foes.push(crate::duel::Fighter { block: m.blocks[0].clone(), normal, skills: Some(&t.skills[..]), death_immune: m.death_immune, chara: bd::MONSTER_CHARA_FIRST as i32 + k as i32 });
        names.push(m.name.clone());
        start.push([m.blocks[0].hp, m.blocks[0].hp, 0, 0]);
    }
    let sizes: Vec<f32> = drawn.iter().map(|&k| bd::monster_size(&t.radii, k).unwrap_or(MONSTER_SIZE)).collect();
    let kinds: Vec<&str> = t.kinds.iter().map(String::as_str).collect();
    let rules = crate::skirmish::ItemRules { acts: &t.acts, kinds: &kinds, effects: &t.effects };
    let (events, winner, turn) = match commands {
        None => {
            let (events, winner) = crate::skirmish::skirmish(&members, &foes, &sizes, rules, rng, MAX_EVENTS);
            (events, winner, None)
        }
        Some(commands) => run_commanded(&members, &foes, &sizes, rules, rng, commands),
    };
    Ok((Outcome { names, start, events, winner }, turn))
}

/// A member's turn waiting for a command: who, the skills it may use (name, MP cost, whether it
/// can pay) and the monsters standing (fighter numbers).
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub who: usize,
    pub skills: Vec<(String, i32, bool)>,
    pub foes: Vec<usize>,
}

/// Monsters already drawn against `party` with the player's `commands`, one per member turn in
/// order, from the battle's generator `rng` (taken by value: the same generator and commands
/// always fight the same battle, so a battle grows a command at a time). Returns the battle so
/// far and the member turn waiting for the next command (none once a side has won).
pub fn fight_commanded(t: &Tables, party: &[(usize, i32)], drawn: &[usize], mut rng: Rng, commands: &[Command]) -> Result<(Outcome, Option<Turn>), String> {
    fight_with(t, party, drawn, &mut rng, Some(commands))
}

fn run_commanded(members: &[crate::skirmish::Member], foes: &[crate::duel::Fighter], sizes: &[f32], rules: crate::skirmish::ItemRules, rng: &mut Rng, commands: &[Command]) -> (Vec<Event>, Option<u8>, Option<Turn>) {
    let mut s = crate::skirmish::Skirmish::new(members, foes, sizes, rules, rng);
    let mut events = Vec::new();
    let mut given = commands.iter();
    while events.len() < MAX_EVENTS && s.winner().is_none() {
        let (who, start) = s.begin_turn();
        events.extend(start);
        if !s.can_act(who) {
            continue;
        }
        if !s.is_member(who) {
            events.extend(s.auto(who, rng));
            continue;
        }
        match given.next() {
            Some(&c) => events.extend(s.command(who, c, rng)),
            None => {
                let skills = s.skills(who).into_iter().map(|(k, cost, can)| (k.name.clone(), cost, can)).collect();
                return (events, None, Some(Turn { who, skills, foes: s.foes_standing() }));
            }
        }
    }
    let winner = s.winner();
    (events, winner, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_name_the_fighters() {
        let o = Outcome {
            names: vec!["Escha".into(), "Puni".into()],
            start: vec![[10; 4], [10; 4]],
            events: vec![Event::Acted { who: 0, skill: "Attack".into(), target: 1, damage: 12, hp_left: 0, mp_left: 0 }],
            winner: Some(0),
        };
        assert_eq!(o.lines(), vec!["Escha uses Attack on Puni: 12 damage (0 HP left)".to_string(), "the party wins".into()]);
    }
}
