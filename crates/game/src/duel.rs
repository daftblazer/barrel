//! A partial battle: two fighters (monsters, or a party member at a level) trading their
//! normal attacks, built from the pieces in
//! [`crate::battle`]. It is a check that the pieces fit, not the game's battle yet.
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
    StatBlock { level, hp: s.hp, attack: s.attack, defense: s.defense, speed: s.speed, resist: [0; 4], stat12: 0 }
}

/// Runs the duel until one side's HP is 0 or `max_hits` hits have been made; fighter i uses
/// `attacks[i]` (the party's normal attack is skill 0, the monsters' skill 1).
pub fn duel(a: &StatBlock, b: &StatBlock, attacks: [&Skill; 2], rng: &mut Rng, max_hits: usize) -> Vec<Hit> {
    let mut fighters = [stats(a), stats(b)];
    let mut timeline = Timeline { entries: vec![(0, 500), (1, 500)] };
    let mut hits = Vec::new();
    let effect_of = |s: &Skill| s.effects.iter().find(|e| e.act == 0).copied();
    let (Some(e0), Some(e1)) = (effect_of(attacks[0]), effect_of(attacks[1])) else { return hits };
    let effects = [e0, e1];
    let mut pending = -1;
    while hits.len() < max_hits {
        timeline.advance();
        let who = timeline.entries[0].0 as usize;
        let target = 1 - who;
        let (effect, attack) = (&effects[who], attacks[who]);
        // The roll (0x1401bd520): the amount, then the second value.
        let amount = rng.range(effect.min, effect.max);
        let _second = rng.range(effect.min2, effect.max2);
        let user = &fighters[who];
        let damage = battle::skill_damage(amount, user[stat::ATTACK], user[stat::LEVEL], fighters[target][stat::DEFENSE], true, 1.0);
        let damage = battle::reduce(damage, None, &Defender::default());
        let amount = battle::hit_amount(damage, 0);
        // The first hit of an action draws for its sound (0x1401af770).
        rng.next_31();
        let dealt = battle::take_hp_damage(&mut fighters[target], amount, None, None);
        hits.push(Hit { attacker: who, damage: dealt, hp_left: fighters[target][stat::HP] });
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
        let squirrel = StatBlock { level: 6, hp: 35, attack: 15, defense: 8, speed: 13, resist: [0; 4], stat12: 0 };
        let rat = StatBlock { level: 30, hp: 351, attack: 52, defense: 27, speed: 37, resist: [0; 4], stat12: 0 };
        let attack = Skill {
            name: "Attack".into(),
            category: String::new(),
            wait: 600,
            effects: vec![SkillEffect { act: 0, min: 48, max: 60, min2: 2, max2: 2 }],
            description: String::new(),
        };
        let mut rng = Rng::from_state([123456789, 362436069, 521288629, 88675123]);
        let hits = duel(&squirrel, &rat, [&attack, &attack], &mut rng, 100);
        let last = hits.last().unwrap();
        assert_eq!(last.hp_left, 0);
        // The faster, stronger rat wins.
        assert_eq!(last.attacker, 1);
        // Same seed, same battle.
        let mut rng = Rng::from_state([123456789, 362436069, 521288629, 88675123]);
        assert_eq!(duel(&squirrel, &rat, [&attack, &attack], &mut rng, 100), hits);
    }
}
