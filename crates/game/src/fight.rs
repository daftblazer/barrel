//! A battle in progress: the fighters with their stats and timed states, the turn timeline,
//! and the steps a turn is made of, built from the rules in [`crate::battle`]. Whoever drives
//! it (the duel, later the player's commands and the monsters' rules) asks whose turn is next,
//! starts that turn, and has the fighter act; each step reports what happened as [`Event`]s.
//!
//! Not in yet (each a way it can differ from the game): traits, passives and the field states;
//! items; the battle's start rules (everyone joins at wait 500, in order); the effect-roll
//! globals (neutral: no chain bonus, full MP power); the action's float (1.0); effects other
//! than HP damage (acts 0 and 8), poison (16) and the stat buffs and debuffs (40-42, 45-47);
//! and more than one target per action.

use crate::battle::{self, stat, Defender, Timeline, TimedState};
use crate::rng::Rng;
use barrel_formats::battle_data::{Skill, StatBlock};

pub type Stats = [i32; stat::COUNT];

/// One fighter.
#[derive(Clone, Debug)]
pub struct Fighter {
    /// 0 or 1 (the party's side is 0).
    pub side: u8,
    /// The stats with no states (+0x114) and as they stand (+0x88).
    pub base: Stats,
    pub stats: Stats,
    pub states: Vec<TimedState>,
    /// Actions taken so far (+0x2c8), which monster rules and `DAMAGE_TURN` read.
    pub actions: i32,
    /// Uses of each monster action slot.
    pub uses: [i32; 9],
    /// The maximum HP and MP cap: 999, or 999,999 for monsters.
    pub cap: i32,
}

impl Fighter {
    /// A fighter from a stat block (a monster's, or a party member's from its level).
    pub fn new(block: &StatBlock, side: u8, cap: i32) -> Fighter {
        let mut s = [0; stat::COUNT];
        s[stat::LEVEL] = block.level;
        s[stat::HP] = block.hp;
        s[stat::MAX_HP] = block.hp;
        s[stat::ATTACK] = block.attack;
        s[stat::DEFENSE] = block.defense;
        s[stat::SPEED] = block.speed;
        s[stat::RESIST..stat::RESIST + 4].copy_from_slice(&block.resist);
        Fighter { side, base: s, stats: s, states: Vec::new(), actions: 0, uses: [0; 9], cap }
    }

    pub fn hp(&self) -> i32 {
        self.stats[stat::HP]
    }

    pub fn down(&self) -> bool {
        self.hp() == 0
    }

    /// Puts a timed state on and recalculates the stats (`0x140191340`, `0x1401af440`).
    pub fn add_state(&mut self, state: u32, value: i32, second: i32) {
        battle::add_state(&mut self.states, state, value, second);
        self.restat();
    }

    fn restat(&mut self) {
        self.stats = battle::stats_with_states(&self.base, &self.stats, &self.states, self.cap);
    }
}

/// Something that happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A fighter's turn-start HP change took `damage` (poison).
    Poisoned { who: usize, damage: i32, hp_left: i32 },
    /// A fighter used a skill on another; `damage` is the HP it took (0 when the skill deals none).
    Acted { who: usize, skill: String, target: usize, damage: i32, hp_left: i32 },
}

pub struct Battle {
    pub fighters: Vec<Fighter>,
    pub timeline: Timeline,
    /// The pending wait change of `0x1401b9b70` (global 0x141077e54).
    pending: i32,
}

impl Battle {
    /// Everyone joins at wait 500, in the order given.
    pub fn new(fighters: Vec<Fighter>) -> Battle {
        let timeline = Timeline { entries: (0..fighters.len() as u32).map(|k| (k, 500)).collect() };
        Battle { fighters, timeline, pending: -1 }
    }

    /// The side that has won: the other side has nobody standing.
    pub fn winner(&self) -> Option<u8> {
        let standing = |side| self.fighters.iter().any(|f| f.side == side && !f.down());
        match (standing(0), standing(1)) {
            (true, false) => Some(0),
            (false, true) => Some(1),
            _ => None,
        }
    }

    /// Moves time on to the next turn and says whose it is.
    pub fn next(&mut self) -> usize {
        self.timeline.advance();
        self.timeline.entries[0].0 as usize
    }

    /// The start of `who`'s turn (`0x1401ae4f0`): the heals and poisons netted into one HP
    /// change (`0x1401b0100`), then its timed states age (`0x140191120`).
    pub fn start_turn(&mut self, who: usize) -> Vec<Event> {
        let mut events = Vec::new();
        let f = &mut self.fighters[who];
        let poison: i32 = f.states.iter().filter(|s| s.state == 0x34).map(|s| s.value as i32).sum();
        let poison = if poison > 0 { battle::poison_damage(poison, f.stats[stat::DEFENSE]) } else { 0 };
        let change = battle::turn_hp_change(0, poison, 0);
        if change < 0 {
            let damage = battle::take_hp_damage(&mut f.stats, battle::hit_amount(-change, 0), None, None);
            events.push(Event::Poisoned { who, damage, hp_left: f.hp() });
        }
        battle::age_states(&mut f.states);
        f.restat();
        events
    }

    /// `who` uses `skill` on `target` (its foe; own-side skills put their states on `who`), then
    /// gets its next wait and counts the action. `slot` is the monster action slot it came from.
    pub fn act(&mut self, who: usize, skill: &Skill, slot: Option<usize>, target: usize, rng: &mut Rng) -> Vec<Event> {
        // Poison (`DAMAGE_HP_TURN`, act 16) on the target.
        if let Some(e) = skill.effects.iter().find(|e| e.act == 16) {
            let value = rng.range(e.min, e.max);
            let second = rng.range(e.min2, e.max2);
            battle::add_state(&mut self.fighters[target].states, 0x34, value, second);
        }
        // Stat buffs and debuffs on the skill's side: its own for target kinds 1 and 3.
        for e in &skill.effects {
            if let Some((state, minus)) = battle::stat_state(e.act) {
                let value = rng.range(e.min, e.max);
                let second = rng.range(e.min2, e.max2);
                let on = if matches!(skill.target, 1 | 3) { who } else { target };
                self.fighters[on].add_state(state, if minus { -value } else { value }, second);
            }
        }
        if let Some(k) = slot.filter(|&k| k < 9) {
            self.fighters[who].uses[k] += 1;
        }
        let damage = match skill.effects.iter().find(|e| e.act == 0 || e.act == 8) {
            None => 0,
            Some(effect) => {
                // The roll (`0x1401bd520`): the amount, then the second value.
                let amount = rng.range(effect.min, effect.max);
                let second = rng.range(effect.min2, effect.max2);
                let (user, foe) = (&self.fighters[who].stats, &self.fighters[target].stats);
                let amount = if effect.act == 8 {
                    battle::turn_damage(amount, second, self.fighters[who].actions, 1.0)
                } else {
                    // Elemental damage skips defence and meets the target's resistance.
                    let damage = battle::skill_damage(amount, user[stat::ATTACK], user[stat::LEVEL], foe[stat::DEFENSE], effect.element.is_none(), 1.0);
                    let mut resist = [0; 4];
                    resist.copy_from_slice(&foe[stat::RESIST..stat::RESIST + 4]);
                    battle::reduce(damage, effect.element, &Defender { resist, ..Defender::default() })
                };
                let amount = battle::hit_amount(amount, 0);
                // The first hit of an action draws for its sound (`0x1401af770`).
                rng.next_31();
                battle::take_hp_damage(&mut self.fighters[target].stats, amount, None, None)
            }
        };
        let events = vec![Event::Acted { who, skill: skill.name.clone(), target, damage, hp_left: self.fighters[target].hp() }];
        // Its next wait (`0x1401b9b70`, `0x14018c9d0`) and the action counted (`0x1401aecda`);
        // a finishing blow ends the battle before either.
        if !self.fighters[target].down() {
            let wait = battle::action_wait(skill.wait, self.fighters[who].stats[stat::SPEED], None, None, &mut self.pending);
            let wait = battle::wait_after_action(wait, None, None, true, rng);
            let i = self.timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
            self.timeline.entries[i].1 = wait;
        }
        self.fighters[who].actions += 1;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(hp: i32) -> StatBlock {
        StatBlock { level: 10, hp, attack: 30, defense: 0, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] }
    }

    #[test]
    fn poison_ticks_at_turn_start_and_can_end_the_battle() {
        let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(3), 1, 999_999)]);
        assert_eq!(b.winner(), None);
        b.fighters[1].add_state(0x34, 10, 2);
        let events = b.start_turn(1);
        // The damage is the amount dealt, not what was left to take.
        assert_eq!(events, [Event::Poisoned { who: 1, damage: 10, hp_left: 0 }]);
        assert_eq!(b.winner(), Some(0));
        // The count went down by the turn's ageing.
        assert_eq!(b.fighters[1].states[0].count, 1);
    }
}
