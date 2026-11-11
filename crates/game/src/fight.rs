//! A battle in progress: the fighters with their stats and timed states, the turn timeline,
//! and the steps a turn is made of, built from the rules in [`crate::battle`]. Whoever drives
//! it (the duel, later the player's commands and the monsters' rules) asks whose turn is next,
//! starts that turn, and has the fighter act; each step reports what happened as [`Event`]s.
//!
//! Not in yet (each a way it can differ from the game): traits, passives and the field states;
//! items; the battle's start rules (everyone joins at wait 500, in order); the effect-roll
//! globals (neutral: no chain bonus, full MP power); the action's float (1.0); effects other
//! than HP damage (acts 0 and 8), heals (19, 21), poison (16) and the stat buffs and debuffs
//! (40-42, 45-47);
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
    /// Where it stands (the model's +0x230; not read yet, all 0), for the nearest and
    /// farthest target picks.
    pub position: [f32; 3],
    /// What it adds to its weight as a random target (virtual +0x128; not read yet, 0).
    pub draw: i32,
    /// The character a monster remembers (+0x314; -1 none), for target pick 4.
    pub remembered: i32,
    /// Whether guts has saved it since its last heal (+0x28d).
    pub guts_used: bool,
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
        Fighter { side, base: s, stats: s, states: Vec::new(), actions: 0, uses: [0; 9], cap, position: [0.0; 3], draw: 0, remembered: -1, guts_used: false }
    }

    /// With `mp` as its MP and maximum MP (the monsters' blocks have none).
    pub fn with_mp(mut self, mp: i32) -> Fighter {
        for s in [&mut self.base, &mut self.stats] {
            s[stat::MP] = mp;
            s[stat::MAX_MP] = mp;
        }
        self
    }

    pub fn mp(&self) -> i32 {
        self.stats[stat::MP]
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
    Acted { who: usize, skill: String, target: usize, damage: i32, hp_left: i32, mp_left: i32 },
    /// A fighter's skill healed another (or itself) by `amount`.
    Healed { who: usize, skill: String, target: usize, amount: i32, hp_left: i32, mp_left: i32 },
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

    /// The fighters a skill of `who`'s may be aimed at (`0x140189b00` fills the battle's list,
    /// `battle+0xab8`): those standing on its own side for target kinds 1 and 3, on the other
    /// side for 2, on either for the rest (the picker's side -1; Vital Check, a heal that also
    /// revives, is kind 4).
    pub fn candidates(&self, who: usize, skill: &Skill) -> Vec<usize> {
        let own = self.fighters[who].side;
        let side = match skill.target {
            1 | 3 => Some(own),
            2 => Some(1 - own),
            _ => None,
        };
        (0..self.fighters.len()).filter(|&k| side.is_none_or(|s| self.fighters[k].side == s) && !self.fighters[k].down()).collect()
    }

    /// Picks a target among `candidates` by a monster rule's target `mode` (`0x14018a2f0`):
    /// 1 the nearest to `who`, 2 the farthest (the first on ties), 3 the lowest share of its
    /// maximum HP (first on ties), 4 the one `who` remembers (+0x314) and 5 the one the battle
    /// marks (global 0x141077e3c, not kept here), each when it is a candidate; otherwise, and
    /// for 0, a weighted draw (`0x14018a090`): each weighs `max(draw + 40, 1)`, and one
    /// number below the total (`0x1404ca7a0` modulo it, always drawn) falls on the first
    /// whose running total passes it.
    pub fn pick_target(&self, mode: i32, who: usize, candidates: &[usize], rng: &mut Rng) -> Option<usize> {
        let first = *candidates.first()?;
        let distance = |k: usize| {
            let (a, b) = (self.fighters[who].position, self.fighters[k].position);
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };
        let share = |k: usize| self.fighters[k].stats[stat::HP] as f32 / self.fighters[k].stats[stat::MAX_HP] as f32;
        let mut best = first;
        match mode {
            1 | 2 | 3 => {
                for &k in &candidates[1..] {
                    let better = match mode {
                        1 => distance(best) > distance(k),
                        2 => distance(k) > distance(best),
                        _ => share(best) > share(k),
                    };
                    if better {
                        best = k;
                    }
                }
                return Some(best);
            }
            4 => {
                let r = self.fighters[who].remembered;
                if let Some(&k) = candidates.iter().find(|&&k| k as i32 == r) {
                    return Some(k);
                }
            }
            _ => {}
        }
        let weights: Vec<i32> = candidates.iter().map(|&k| (self.fighters[k].draw + 40).max(1)).collect();
        let total: i32 = weights.iter().sum();
        let r = rng.next_31() % total;
        let mut run = 0;
        for (&k, w) in candidates.iter().zip(&weights) {
            run += w;
            if run > r {
                return Some(k);
            }
        }
        Some(first)
    }

    /// Moves time on to the next turn and says whose it is; fallen fighters' entries are out
    /// of the running.
    pub fn next(&mut self) -> usize {
        let fallen: Vec<bool> = self.fighters.iter().map(Fighter::down).collect();
        self.timeline.advance_with(|c| fallen[c as usize]);
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

    /// `who` pays a skill's MP before acting (`0x1401af970` at `0x1401b8440`; MP not under 0).
    pub fn pay_mp(&mut self, who: usize, cost: i32) {
        let f = &mut self.fighters[who];
        f.stats[stat::MP] = (f.stats[stat::MP] - cost).max(0);
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
        // A heal (`HEAL_HP`, act 19; `HEAL_HP_BY_ATK`, 21) through the heal event (`0x1401c1d00`).
        if let Some(effect) = skill.effects.iter().find(|e| e.act == 19 || e.act == 21) {
            let amount = rng.range(effect.min, effect.max);
            rng.range(effect.min2, effect.max2);
            let amount = if effect.act == 21 { battle::heal_by_attack(self.fighters[who].stats[stat::ATTACK], amount, 1.0) } else { amount };
            let blocked = self.fighters[target].states.iter().any(|s| s.state == 0x4e);
            let amount = battle::heal(&mut self.fighters[target].stats, amount, blocked);
            self.fighters[target].guts_used = false;
            let event = Event::Healed { who, skill: skill.name.clone(), target, amount, hp_left: self.fighters[target].hp(), mp_left: self.fighters[who].mp() };
            self.end_action(who, skill, rng);
            return vec![event];
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
        self.after_effect(target, rng);
        let events = vec![Event::Acted { who, skill: skill.name.clone(), target, damage, hp_left: self.fighters[target].hp(), mp_left: self.fighters[who].mp() }];
        self.end_action(who, skill, rng);
        events
    }

    /// What happens to a target after an effect (`0x1401c0676` and on): a party member's guts
    /// (stat 25) may keep it at 1 HP; one that has gone down loses its timed states (`0x1401b0580`
    /// through the down handler, virtual +0xf0) and drops out of the timeline.
    fn after_effect(&mut self, target: usize, rng: &mut Rng) {
        let f = &mut self.fighters[target];
        if f.side == 0 && f.hp() <= 0 && !f.guts_used {
            // `ADD_GUTS` is spent before the roll, whatever it gives.
            let add_guts = f.states.iter().any(|s| s.state == 4);
            f.states.retain(|s| s.state != 4);
            if rng.chance(battle::guts_chance(f.stats[25], add_guts, false)) {
                f.guts_used = true;
                f.stats[stat::HP] = 1;
            }
        }
        if f.down() {
            f.states.clear();
            f.restat();
        }
    }

    /// Its next wait (`0x1401b9b70`, `0x14018c9d0`) and the action counted (`0x1401aecda`); an
    /// action that wins the battle ends it before the wait.
    fn end_action(&mut self, who: usize, skill: &Skill, rng: &mut Rng) {
        if self.winner().is_none() {
            let wait = battle::action_wait(skill.wait, self.fighters[who].stats[stat::SPEED], None, None, &mut self.pending);
            let wait = battle::wait_after_action(wait, None, None, true, rng);
            let i = self.timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
            self.timeline.entries[i].1 = wait;
        }
        self.fighters[who].actions += 1;
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

    #[test]
    fn target_picks() {
        let monster = Fighter::new(&block(500), 1, 999_999);
        let mut party: Vec<Fighter> = [100, 80, 120].iter().map(|&hp| Fighter::new(&block(hp), 0, 999)).collect();
        party[1].stats[stat::HP] = 20;
        party[2].stats[stat::HP] = 30;
        party.push(monster);
        let b = Battle::new(party);
        let skill = Skill { name: String::new(), category: String::new(), wait: 600, target: 2, spread: 0, effects: Vec::new(), description: String::new() };
        let c = b.candidates(3, &skill);
        assert_eq!(c, [0, 1, 2]);
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        // Lowest share of maximum HP: 20/80 = 0.25 against 30/120 = 0.25, the first wins.
        assert_eq!(b.pick_target(3, 3, &c, &mut rng), Some(1));
        // The weighted draw: equal weights of 40, one number below 120.
        let mut check = Rng::from_state([1, 2, 3, 4]);
        let r = check.next_31() % 120;
        assert_eq!(b.pick_target(0, 3, &c, &mut rng), Some(c[(r / 40) as usize]));
    }
}
