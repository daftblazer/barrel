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

use crate::battle::{self, stat, Defender, Timeline};
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

fn stats(b: &StatBlock) -> [i32; stat::COUNT] {
    let mut s = [0; stat::COUNT];
    s[stat::LEVEL] = b.level;
    s[stat::HP] = b.hp;
    s[stat::MAX_HP] = b.hp;
    s[stat::ATTACK] = b.attack;
    s[stat::DEFENSE] = b.defense;
    s[stat::SPEED] = b.speed;
    s[stat::RESIST..stat::RESIST + 4].copy_from_slice(&b.resist);
    s
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
    /// its skill list, then that skill's record), else the normal attack.
    fn pick(&self, situation: &crate::monster_ai::Situation, rng: &mut Rng) -> (Option<usize>, &Skill) {
        let Some(table) = self.skills.filter(|_| !self.block.rules.is_empty()) else { return (None, self.normal) };
        let (slot, _target) = crate::monster_ai::choose(&self.block.rules, situation, rng);
        let skill = usize::try_from(slot)
            .ok()
            .and_then(|k| self.block.slots.get(k))
            .and_then(|&i| usize::try_from(i).ok())
            .and_then(|i| self.block.skills.get(i))
            .and_then(|&(n, _)| table.get(n as usize));
        (usize::try_from(slot).ok(), skill.unwrap_or(self.normal))
    }
}

/// Runs the duel until one side's HP is 0 or `max_hits` actions have been taken. Each fighter
/// counts its own actions (the character's +0x2c8, raised once its action is done at
/// `0x1401aecda`), which its monster rules and its `DAMAGE_TURN` effects read. The phase is 0.
pub fn duel(a: &Fighter, b: &Fighter, rng: &mut Rng, max_hits: usize) -> Vec<Hit> {
    let sides = [a, b];
    let mut fighters = [stats(&a.block), stats(&b.block)];
    let bases = fighters;
    // Monsters' HP may pass 999 (party members' cap); both sides here are kept to 999,999.
    let restat = |f: &mut [i32; stat::COUNT], k: usize, states: &[battle::TimedState]| *f = battle::stats_with_states(&bases[k], f, states, 999_999);
    let mut timeline = Timeline { entries: vec![(0, 500), (1, 500)] };
    let mut hits = Vec::new();
    let mut uses = [[0i32; 9]; 2];
    let mut turns = [0i32; 2];
    let mut states: [Vec<battle::TimedState>; 2] = [Vec::new(), Vec::new()];
    let mut pending = -1;
    while hits.len() < max_hits {
        timeline.advance();
        let who = timeline.entries[0].0 as usize;
        let target = 1 - who;
        // Its turn starts (`0x1401ae4f0`): poison, then its states age.
        let poison: i32 = states[who].iter().filter(|s| s.state == 0x34).map(|s| s.value as i32).sum();
        let poison = if poison > 0 { battle::poison_damage(poison, fighters[who][stat::DEFENSE]) } else { 0 };
        let change = battle::turn_hp_change(0, poison, 0);
        if change < 0 {
            let damage = battle::hit_amount(-change, 0);
            let dealt = battle::take_hp_damage(&mut fighters[who], damage, None, None);
            // A poison tick: `attacker` is the poisoned fighter itself.
            hits.push(Hit { attacker: who, skill: "poison".into(), damage: dealt, hp_left: fighters[who][stat::HP] });
            if fighters[who][stat::HP] == 0 {
                break;
            }
        }
        battle::age_states(&mut states[who]);
        restat(&mut fighters[who], who, &states[who]);
        let situation = crate::monster_ai::Situation {
            hp: fighters[who][stat::HP],
            max_hp: fighters[who][stat::MAX_HP],
            turn: turns[who],
            phase: 0,
            uses: &uses[who],
        };
        let (slot, attack) = sides[who].pick(&situation, rng);
        // A poisoning effect (`DAMAGE_HP_TURN`, act 16) puts state 0x34 on the target.
        if let Some(e) = attack.effects.iter().find(|e| e.act == 16) {
            let value = rng.range(e.min, e.max);
            let second = rng.range(e.min2, e.max2);
            battle::add_state(&mut states[target], 0x34, value, second);
        }
        // Stat buffs go on the user, debuffs on the foe (taken; the skills' targets are not
        // read yet).
        for e in &attack.effects {
            if let Some((state, minus)) = battle::stat_state(e.act) {
                let value = rng.range(e.min, e.max);
                let second = rng.range(e.min2, e.max2);
                let on = if minus { target } else { who };
                battle::add_state(&mut states[on], state, if minus { -value } else { value }, second);
                restat(&mut fighters[on], on, &states[on]);
            }
        }
        if let Some(k) = slot.filter(|&k| k < 9) {
            uses[who][k] += 1;
        }
        // The skill's HP damage (act 0) or turn-grown damage (act 8, `DAMAGE_TURN`) effect is
        // worked; the others need parts not built yet.
        let Some(effect) = attack.effects.iter().find(|e| e.act == 0 || e.act == 8).copied() else {
            turns[who] += 1;
            hits.push(Hit { attacker: who, skill: attack.name.clone(), damage: 0, hp_left: fighters[target][stat::HP] });
            let wait = battle::action_wait(attack.wait, fighters[who][stat::SPEED], None, None, &mut pending);
            let i = timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
            timeline.entries[i].1 = battle::wait_after_action(wait, None, None, true, rng);
            continue;
        };
        let effect = &effect;
        // The roll (0x1401bd520): the amount, then the second value.
        let amount = rng.range(effect.min, effect.max);
        let _second = rng.range(effect.min2, effect.max2);
        let user = &fighters[who];
        if effect.act == 8 {
            rng.next_31();
            let amount = battle::hit_amount(battle::turn_damage(amount, _second, turns[who], 1.0), 0);
            let dealt = battle::take_hp_damage(&mut fighters[target], amount, None, None);
            hits.push(Hit { attacker: who, skill: attack.name.clone(), damage: dealt, hp_left: fighters[target][stat::HP] });
            turns[who] += 1;
            if fighters[target][stat::HP] == 0 {
                break;
            }
            let wait = battle::action_wait(attack.wait, fighters[who][stat::SPEED], None, None, &mut pending);
            let i = timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
            timeline.entries[i].1 = battle::wait_after_action(wait, None, None, true, rng);
            continue;
        }
        // Elemental damage skips defence and meets the target's resistance (stats 8 to 11).
        let damage = battle::skill_damage(amount, user[stat::ATTACK], user[stat::LEVEL], fighters[target][stat::DEFENSE], effect.element.is_none(), 1.0);
        let mut resist = [0; 4];
        resist.copy_from_slice(&fighters[target][stat::RESIST..stat::RESIST + 4]);
        let damage = battle::reduce(damage, effect.element, &Defender { resist, ..Defender::default() });
        let amount = battle::hit_amount(damage, 0);
        // The first hit of an action draws for its sound (0x1401af770).
        rng.next_31();
        let dealt = battle::take_hp_damage(&mut fighters[target], amount, None, None);
        hits.push(Hit { attacker: who, skill: attack.name.clone(), damage: dealt, hp_left: fighters[target][stat::HP] });
        turns[who] += 1;
        if fighters[target][stat::HP] == 0 {
            break;
        }
        let wait = battle::action_wait(attack.wait, fighters[who][stat::SPEED], None, None, &mut pending);
        let wait = battle::wait_after_action(wait, None, None, true, rng);
        let i = timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
        timeline.entries[i].1 = wait;
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
