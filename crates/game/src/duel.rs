//! A partial battle: two fighters (monsters, or a party member at a level) acting in turn,
//! built from the pieces in [`crate::battle`]: a monster picks each action by its rules
//! ([`crate::monster_ai`]), a party member uses its normal attack. It is a check that the
//! pieces fit, not the game's battle yet.
//!
//! What it leaves out (each a reason it can differ from the game): the battle's start (both
//! join with wait 500, the first in front), every trait, state and passive (so every scale is
//! 1.0 and no quick-turn roll is made), the effect-roll globals (taken as neutral: no chain bonus, and full MP power, which the
//! monsters' normal attack always has since it costs no MP), the
//! action's float (taken as 1.0), item and party code, death checks beyond HP 0, and the
//! battle's other random draws.

use crate::battle::{self, stat};
use crate::rng::Rng;
use barrel_formats::battle_data::{Skill, StatBlock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// 0 or 1: which fighter acted.
    pub attacker: usize,
    /// The skill it used.
    pub skill: String,
    pub damage: i32,
    /// The target's HP after the hit.
    pub hp_left: i32,
}

/// A party member's base stats at a level as a stat block (no resistances, no equipment).
pub fn party_block(level: i32, s: &barrel_formats::battle_data::LevelStats) -> StatBlock {
    StatBlock { level, hp: s.hp, attack: s.attack, defense: s.defense, speed: s.speed, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] }
}

/// What an equipped item adds (`docs/tables.md`, "A party member's stats"): its HP, MP,
/// attack, defence and speed columns, at a quality. Which item column goes to which battle stat
/// is taken by name; the executable reads them as an array (item row +0x78) not mapped yet.
#[derive(Clone, Copy, Debug, Default)]
pub struct Equipment {
    pub hp: i32,
    pub mp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub quality: f32,
}

/// `block` with each item's share added (traits and potentials not counted).
pub fn equip(block: &StatBlock, items: &[Equipment]) -> StatBlock {
    let mut b = block.clone();
    for e in items {
        let add = |v: i32| battle::equipment_stat(v, 0, e.quality);
        b.hp += add(e.hp);
        b.attack += add(e.attack);
        b.defense += add(e.defense);
        b.speed += add(e.speed);
    }
    b
}

/// One side of a duel: its stat block, its normal attack, and (for a monster) the skill
/// table its rules pick from (`None`, or a block without rules: always the normal attack).
pub struct Fighter<'a> {
    pub block: StatBlock,
    pub normal: &'a Skill,
    pub skills: Option<&'a [Skill]>,
}

impl Fighter<'_> {
    /// The skill for this turn: by the block's rules when it has them (the slot's index into
    /// its skill list, then that skill's record, and the rule's target pick), else the normal
    /// attack (and no target pick: a party member's target is the player's choice).
    fn pick(&self, situation: &crate::monster_ai::Situation, rng: &mut Rng) -> (Option<usize>, &Skill, Option<i32>) {
        let Some(table) = self.skills.filter(|_| !self.block.rules.is_empty()) else { return (None, self.normal, None) };
        let (slot, target) = crate::monster_ai::choose(&self.block.rules, situation, rng);
        let skill = usize::try_from(slot)
            .ok()
            .and_then(|k| self.block.slots.get(k))
            .and_then(|&i| usize::try_from(i).ok())
            .and_then(|i| self.block.skills.get(i))
            .and_then(|&(n, _)| table.get(n as usize));
        (usize::try_from(slot).ok(), skill.unwrap_or(self.normal), Some(target))
    }
}

/// Runs the duel (a [`crate::fight::Battle`] of two) until one side's HP is 0 or `max_hits`
/// actions and poison ticks have happened. The phase is 0; both fighters are capped as
/// monsters (999,999 HP).
pub fn duel(a: &Fighter, b: &Fighter, rng: &mut Rng, max_hits: usize) -> Vec<Hit> {
    use crate::fight::{Battle, Event};
    let sides = [a, b];
    let mut battle = Battle::new(vec![crate::fight::Fighter::new(&a.block, 0, 999_999), crate::fight::Fighter::new(&b.block, 1, 999_999)]);
    let mut hits = Vec::new();
    let hit = |e: &Event| match e {
        // A poison tick: `attacker` is the poisoned fighter itself.
        Event::Poisoned { who, damage, hp_left } => Hit { attacker: *who, skill: "poison".into(), damage: *damage, hp_left: *hp_left },
        Event::Acted { who, skill, damage, hp_left, .. } => Hit { attacker: *who, skill: skill.clone(), damage: *damage, hp_left: *hp_left },
    };
    while hits.len() < max_hits {
        let who = battle.next();
        hits.extend(battle.start_turn(who).iter().map(hit));
        if battle.winner().is_some() {
            break;
        }
        let f = &battle.fighters[who];
        let situation = crate::monster_ai::Situation { hp: f.hp(), max_hp: f.stats[stat::MAX_HP], turn: f.actions, phase: 0, uses: &f.uses };
        let (slot, skill, mode) = sides[who].pick(&situation, rng);
        let candidates = battle.candidates(who, skill);
        let target = match mode {
            Some(mode) => battle.pick_target(mode, who, &candidates, rng),
            None => candidates.first().copied(),
        };
        // A skill on its own side (a heal, a self-buff) still meets the foe in this partial
        // battle's damage step, as before.
        let target = target.filter(|&t| t != who).unwrap_or(1 - who);
        hits.extend(battle.act(who, skill, slot, target, rng).iter().map(hit));
        if battle.winner().is_some() {
            break;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::battle_data::SkillEffect;

    #[test]
    fn a_duel_ends() {
        let squirrel = StatBlock { level: 6, hp: 35, attack: 15, defense: 8, speed: 13, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] };
        let rat = StatBlock { level: 30, hp: 351, attack: 52, defense: 27, speed: 37, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] };
        let attack = Skill {
            name: "Attack".into(),
            category: String::new(),
            wait: 600,
            target: 2,
            spread: 0,
            effects: vec![SkillEffect { act: 0, element: None, min: 48, max: 60, min2: 2, max2: 2 }],
            description: String::new(),
        };
        let mut rng = Rng::from_state([123456789, 362436069, 521288629, 88675123]);
        let (a, b) = (Fighter { block: squirrel.clone(), normal: &attack, skills: None }, Fighter { block: rat.clone(), normal: &attack, skills: None });
        let hits = duel(&a, &b, &mut rng, 100);
        let last = hits.last().unwrap();
        assert_eq!(last.hp_left, 0);
        // The faster, stronger rat wins.
        assert_eq!(last.attacker, 1);
        // Same seed, same battle.
        let mut rng = Rng::from_state([123456789, 362436069, 521288629, 88675123]);
        assert_eq!(duel(&a, &b, &mut rng, 100), hits);
    }
}
