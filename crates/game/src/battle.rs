//! Battle arithmetic as Escha & Logy DX's executable does it (`docs/tables.md`, "Battle
//! skills" and the sections after it). The steps keep the original's mix of integer and
//! float arithmetic, since replays must match it exactly.

/// A battle character's stats by index (`docs/tables.md`, "Battle stats"): the executable keeps
/// 35 live values and 35 base values per character.
pub mod stat {
    pub const LEVEL: usize = 0;
    pub const HP: usize = 1;
    pub const MP: usize = 2;
    pub const MAX_HP: usize = 3;
    pub const MAX_MP: usize = 4;
    pub const ATTACK: usize = 5;
    pub const DEFENSE: usize = 6;
    pub const SPEED: usize = 7;
    /// Resistances to elements 0 to 3 are this and the next three.
    pub const RESIST: usize = 8;
    /// Drain: the percentage of a skill's damage that heals its user.
    pub const DRAIN: usize = 20;
    pub const COUNT: usize = 35;
}

/// The first part of what an equipped item adds to one of the first nine stats
/// (`0x14029dda0`): the item's own stat (the item table's equipment columns) plus its traits'
/// shares, scaled by its quality: quality 50 gives the stat as it is, 100 gives 125%.
pub fn equipment_stat(item_stat: i32, traits: i32, quality: f32) -> i32 {
    ((item_stat + traits) as f32 * ((quality * 0.5 + 75.0) / 100.0)) as i32
}

/// A skill's MP cost (`0x14015f6a0 (skill, CONSUME_MP, 0)`): the sum, over its effect slots
/// with `ACT_CONSUME_MP` (11), of a number rolled from the slot's minimum to its maximum.
pub fn mp_cost(slots: &[(u32, i32, i32)], rng: &mut crate::rng::Rng) -> i32 {
    const CONSUME_MP: u32 = 11;
    slots.iter().filter(|s| s.0 == CONSUME_MP).map(|&(_, lo, hi)| rng.range(lo, hi)).sum()
}

/// The power percentage an action gets from its user's MP (global `0x141077e50`, 100 unless
/// set, and set back to 100 after each action): a user short of the cost acts at
/// `MP × 100 / cost` percent.
pub fn mp_power(mp: i32, cost: i32) -> i32 {
    if cost > 0 { (mp.wrapping_mul(100) / cost).min(100) } else { 100 }
}

/// The base of an HP-damage effect (`0x1401c3290`): the rolled amount as a percentage of
/// `scale_stat`, raised by `attack` (`(attack^1.5)^1.1 / 50` percent, in floats).
pub fn base_damage(amount: i32, scale_stat: i32, attack: i32) -> i32 {
    let growth = (attack as f32).powf(1.5).powf(1.1) / 50.0 + 100.0;
    let part = amount.wrapping_mul(scale_stat) / 100;
    (growth * part as f32 / 100.0) as i32
}

/// An HP-damage effect of a skill (`DAMAGE_HP` and the kinds sharing its handler, at
/// `0x1401bdf6d`), before the target's reductions: the base from the user's stat 5 and
/// attack (the user's stat 0 plus its `EQUIP_PARAM_LEVEL` value), less a quarter of the
/// target's stat 6 for a physical effect, but never under a fifth of the base, times the
/// effect's scale. `physical` is the effect's +4 being -1 and the kind not `DAMAGE_4ELEMENT`.
pub fn skill_damage(amount: i32, user_stat5: i32, attack: i32, target_stat6: i32, physical: bool, scale: f32) -> i32 {
    let base = base_damage(amount, user_stat5, attack);
    let defense = if physical { target_stat6 / 4 } else { 0 };
    let damage = (base - defense).max(base / 5);
    (damage as f32 * scale) as i32
}

/// What the target brings against an HP-damage effect.
#[derive(Clone, Copy, Debug, Default)]
pub struct Defender {
    /// The target's stats 8 to 11: resistance in percent to elements 0 to 3.
    pub resist: [i32; 4],
    /// Passive `AUTO_DAMAGE_REDUCE`'s value, if the target has it.
    pub auto_reduce: Option<i32>,
    /// Passive `EQUIP_REDUCE_SUFFERDMG`'s value, if the target has it.
    pub equip_reduce: Option<i32>,
    /// Passive `PASSIVE_ABSORB_DAMAGE`'s value for the effect's element, if positive.
    pub absorb: Option<i32>,
}

/// The damage after the target's reductions (`0x1401be495` on): an elemental effect
/// (`element` 0 to 3) by the resistance (at most 100%), a physical one by
/// `EQUIP_REDUCE_SUFFERDMG` (at most 50%) when the target has it, else `AUTO_DAMAGE_REDUCE`
/// (at most 80%). The executable takes the second in place of the first, not both. Then an
/// absorbing target turns the damage into healing: `-(damage x absorb / 100)`.
pub fn reduce(damage: i32, element: Option<u8>, d: &Defender) -> i32 {
    let cut = match element {
        Some(e) => d.resist.get(e as usize).map_or(0, |&r| r.min(100)),
        None => d.equip_reduce.map(|v| v.min(50)).or(d.auto_reduce.map(|v| v.min(80))).unwrap_or(0),
    };
    let damage = (100 - cut).wrapping_mul(damage) / 100;
    match d.absorb {
        Some(a) if a > 0 => -(a.wrapping_mul(damage) / 100),
        _ => damage,
    }
}

/// How many times (turns) a timed state lasts when added (`0x140191d00`): the effect's
/// second value for the counted states (2 `HEAL_HP_TURN`, 3, 5 `ADD_ACTION`, 6 `REDUCE_WT`,
/// 0x34 `DAMAGE_HP_TURN`, 0x35 `DAMAGE_HP_TURN_DIFFERENTIAL`), 5 for the buffs and debuffs
/// (7 and 0x12 to 0x2d: `ADD_PERCENT_QUICK`, `PLUS_*`, `MINUS_*`, critical, evade, ...), and
/// -1 (no count) for the rest.
pub fn state_count(state: u32, second: i32) -> i32 {
    match state {
        2 | 3 | 5 | 6 | 0x34 | 0x35 => second,
        7 | 0x12..=0x19 | 0x1b | 0x1c | 0x1f..=0x2d => 5,
        _ => -1,
    }
}

/// A character's timed state (the battle keeps them as 0xa4-byte records in the lists at
/// character +0x1a0; here only what the rules use).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedState {
    pub state: u32,
    /// Turns left ([`state_count`]); -1 for none.
    pub count: i32,
    /// The effect's rolled amount (kept as a float, as the game does).
    pub value: f32,
}

/// Adds a timed state (`0x140191340`): a state already there with no count is replaced only by
/// a stronger value (compared as floats); otherwise the new one goes in (how a counted state
/// already there is merged, and the exclusion groups of `0x140191db0`, are not read yet).
pub fn add_state(states: &mut Vec<TimedState>, state: u32, value: i32, second: i32) {
    let new = TimedState { state, count: state_count(state, second), value: value as f32 };
    if let Some(i) = states.iter().position(|s| s.state == state) {
        if states[i].count == -1 && new.value <= states[i].value {
            return;
        }
        states.remove(i);
    }
    states.push(new);
}

/// `ACT_DAMAGE_TURN`'s damage (`0x1401bebdc`): the rolled amount grown by `per_turn` percent
/// (the effect's second value) for each action the user has taken so far (its +0x2c8), cut
/// to an integer, then times the action's scale. No attack, defence or resistance: it goes
/// straight to [`hit_amount`]. Flame Gale (36 to 45, 5 a turn) hits 5% harder each turn.
pub fn turn_damage(amount: i32, per_turn: i32, turn: i32, scale: f32) -> i32 {
    let grown = (per_turn.wrapping_mul(turn) + 100).wrapping_mul(amount) / 100;
    (grown as f32 * scale) as i32
}

/// The damage event's amount (`0x1401c1970`): the target's +0x2b8 percent taken off, and never
/// under 1. (The executable also adds it to the target's +0x2c4 while +0x2c0 is positive.)
pub fn hit_amount(damage: i32, target_2b8: i32) -> i32 {
    let d = if target_2b8 > 0 { (100 - target_2b8).wrapping_mul(damage) / 100 } else { damage };
    d.max(1)
}

/// Takes `amount` HP damage off a character (`0x1401af770`), returning what reached HP.
/// `reverse` is passive `PASSIVE_REVERSE_DAMAGE` (that percentage goes to MP instead, MP not
/// under 0); `hp_to_mp` is passive `EQUIP_HPTOMP_MPTOHP` (a surviving character gains that
/// percentage of the HP damage as MP, up to its maximum). HP does not go under 0.
///
/// Not done here: MP damage turning back into HP under `EQUIP_HPTOMP_MPTOHP` (a heal, which
/// `FIELD_INVALID_HEAL` can stop), and the random voice the first hit of an action draws.
pub fn take_hp_damage(stats: &mut [i32; stat::COUNT], amount: i32, reverse: Option<i32>, hp_to_mp: Option<i32>) -> i32 {
    let mut amount = amount;
    if let Some(v) = reverse {
        let part = v.wrapping_mul(amount) / 100;
        stats[stat::MP] = (stats[stat::MP] - part).max(0);
        amount = (amount - part).max(0);
    }
    stats[stat::HP] = (stats[stat::HP] - amount).max(0);
    if amount != 0 && stats[stat::HP] > 0 {
        if let Some(v) = hp_to_mp {
            let gain = amount.wrapping_mul(v) / 100;
            stats[stat::MP] = (stats[stat::MP] + gain).min(stats[stat::MAX_MP]);
        }
    }
    amount
}

/// The turn order (the battle's timeline at +0xa28): characters by wait, the first acts next.
/// Waits run from 0 to `MAX_WAIT`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    /// `(character, wait)`, in turn order.
    pub entries: Vec<(u32, i32)>,
}

pub const MAX_WAIT: i32 = 1000;

impl Timeline {
    fn index(&self, who: u32) -> Option<usize> {
        self.entries.iter().position(|&(c, _)| c == who)
    }

    /// Moves time on to the next turn (`0x14018c7e0`), returning how much passed: the
    /// entries are sorted by wait (stably: the executable's sort is an insertion sort for
    /// lists this short), the next entry's wait is taken off it and every entry after it (not
    /// under 0), and the list is sorted again. The next entry then has wait 0 and acts.
    ///
    /// Not done here: entries at the front whose +0x10 is negative are pushed back by 1000
    /// instead (what +0x10 is, is still open).
    pub fn advance(&mut self) -> i32 {
        self.entries.sort_by_key(|&(_, w)| w);
        let Some(&(_, passed)) = self.entries.first() else { return 0 };
        for e in &mut self.entries {
            e.1 = (e.1 - passed).clamp(0, MAX_WAIT);
        }
        self.entries.sort_by_key(|&(_, w)| w);
        passed
    }

    /// Pushes a character back by `amount` (`0x14018d490`): it goes behind every character
    /// whose wait is no more than its new one. Returns its new place.
    pub fn delay(&mut self, who: u32, amount: i32) -> Option<usize> {
        let i = self.index(who)?;
        let wait = (self.entries[i].1 + amount).clamp(0, MAX_WAIT);
        let at = (i..self.entries.len()).find(|&j| j != i && wait < self.entries[j].1).map_or(self.entries.len() - 1, |j| j - 1);
        self.entries.remove(i);
        self.entries.insert(at, (who, wait));
        Some(at)
    }

    /// Brings a character forward by `amount` (`0x14018d5e0`): it goes ahead of every
    /// character whose wait is no less than its new one. Returns its new place.
    pub fn hasten(&mut self, who: u32, amount: i32) -> Option<usize> {
        let i = self.index(who)?;
        let wait = (self.entries[i].1 - amount).clamp(0, MAX_WAIT);
        let at = (0..=i).rev().find(|&j| j != i && wait > self.entries[j].1).map_or(0, |j| j + 1);
        self.entries.remove(i);
        self.entries.insert(at, (who, wait));
        Some(at)
    }
}

/// An action's wait for a character (`0x1401b9b70 (character, wait)`), from the skill's wait
/// (+0x18 in its record; 600 for the normal attack): speed cuts it by speed / 5 percent, at
/// most 25%; timed state 0xb adds half its value plus 50; timed state `REDUCE_WT` cuts it by its
/// value in percent, at most 50%; then a pending extra wait (global `0x141077e54`, when not
/// negative) is added and used up. Never under 0.
pub fn action_wait(wait: i32, speed: i32, state_b: Option<i32>, reduce_wt: Option<i32>, pending: &mut i32) -> i32 {
    let mut w = ((100.0 - (speed as f32 / 5.0).min(25.0)) * wait as f32 / 100.0) as i32;
    if let Some(v) = state_b {
        w += v / 2 + 50;
    }
    if let Some(v) = reduce_wt {
        w = (100 - v.min(50)).wrapping_mul(w) / 100;
    }
    if *pending >= 0 {
        w += *pending;
        *pending = 0;
    }
    w.max(0)
}

/// A character's wait after it acts (`0x14018d180`), from the action's wait: with timed state
/// `ADD_PERCENT_QUICK` (`quick_state`, its value a chance in percent) a successful roll cuts it
/// by 25%; with passive `PASSIVE_PERCENT_QUICK` (`quick_passive`: chance, cut) a successful
/// roll cuts it by the passive's amount instead. Each roll draws only when the character has
/// the state or passive, the state's first. `counts` is `0x1401b07c0`'s answer for the
/// character (false gives wait 0; what it tests is still open). At most `MAX_WAIT`.
pub fn wait_after_action(
    wait: i32,
    quick_state: Option<i32>,
    quick_passive: Option<(i32, i32)>,
    counts: bool,
    rng: &mut crate::rng::Rng,
) -> i32 {
    let mut cut = None;
    if let Some(p) = quick_state {
        if rng.chance(p) {
            cut = Some(25);
        }
    }
    if let Some((p, amount)) = quick_passive {
        if rng.chance(p) {
            cut = Some(amount);
        }
    }
    let wait = match cut {
        Some(c) => (wait as f32 * (100 - c) as f32 / 100.0) as i32,
        None => wait,
    };
    if counts { wait.min(MAX_WAIT) } else { 0 }
}

/// How far `RETURN_WT` pushes a target back (`0x1401c0313`): its amount divided by how many
/// times the target has been pushed back (its +0x30c counter, this one included; when that
/// resets is not yet found), less the
/// target's `RESIST_KNOCKBACK` percent.
pub fn knockback(amount: i32, times: i32, resist: i32) -> i32 {
    (1.0 / times as f32 * amount as f32 * (100 - resist) as f32 / 100.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_count_and_replace() {
        let mut states = Vec::new();
        add_state(&mut states, 0x34, 10, 2);
        add_state(&mut states, 0x12, 30, 0);
        assert_eq!(states.iter().map(|s| (s.state, s.count)).collect::<Vec<_>>(), [(0x34, 2), (0x12, 5)]);
        // An uncounted state keeps the stronger value.
        add_state(&mut states, 0x1a, 50, 0);
        add_state(&mut states, 0x1a, 20, 0);
        assert_eq!(states.last().unwrap().value, 50.0);
        add_state(&mut states, 0x1a, 80, 0);
        assert_eq!(states.last().unwrap().value, 80.0);
    }

    #[test]
    fn turn_damage_grows_with_the_turns() {
        assert_eq!(turn_damage(40, 5, 0, 1.0), 40);
        assert_eq!(turn_damage(40, 5, 10, 1.0), 60);
        assert_eq!(turn_damage(45, 5, 3, 1.0), 51);
    }

    #[test]
    fn damage_by_hand() {
        // 64 * 100 / 100 = 64; (10^1.5)^1.1 = 44.67, so x 100.893 / 100 -> 64.57 -> 64.
        assert_eq!(base_damage(64, 100, 10), 64);
        assert_eq!(skill_damage(64, 100, 10, 40, true, 1.0), 54);
        assert_eq!(skill_damage(64, 100, 10, 40, false, 1.0), 64);
        // Defense cannot take it under a fifth of the base.
        assert_eq!(skill_damage(64, 100, 10, 1000, true, 1.0), 12);
        assert_eq!(skill_damage(64, 100, 10, 40, true, 1.5), 81);
        // At attack 100: (100^1.5)^1.1 = 1000^1.1 = 1995, / 50 = 39.9 percent more.
        assert_eq!(base_damage(100, 100, 100), 139);
    }

    #[test]
    fn taking_damage() {
        assert_eq!(hit_amount(0, 0), 1);
        assert_eq!(hit_amount(200, 25), 150);
        let mut s = [0; stat::COUNT];
        s[stat::HP] = 100;
        s[stat::MP] = 10;
        s[stat::MAX_MP] = 30;
        assert_eq!(take_hp_damage(&mut s, 40, Some(50), Some(100)), 20);
        // 20 went to MP (10 -> 0), 20 to HP, and the 20 HP damage came back as MP.
        assert_eq!((s[stat::HP], s[stat::MP]), (80, 20));
        assert_eq!(take_hp_damage(&mut s, 500, None, Some(100)), 500);
        assert_eq!((s[stat::HP], s[stat::MP]), (0, 20));
    }

    #[test]
    fn turn_order() {
        let mut t = Timeline { entries: vec![(1, 100), (2, 200), (3, 300), (4, 300)] };
        // Pushed to 300: behind both at 300.
        assert_eq!(t.delay(1, 200), Some(3));
        assert_eq!(t.entries, [(2, 200), (3, 300), (4, 300), (1, 300)]);
        // Brought forward to 200: ahead of the one at 200.
        assert_eq!(t.hasten(4, 100), Some(0));
        assert_eq!(t.entries, [(4, 200), (2, 200), (3, 300), (1, 300)]);
        assert_eq!(t.delay(2, 5000), Some(3));
        assert_eq!(t.entries[3], (2, MAX_WAIT));
        assert_eq!(t.hasten(9, 1), None);
        let mut t = Timeline { entries: vec![(1, 300), (2, 120), (3, 120), (4, 500)] };
        assert_eq!(t.advance(), 120);
        assert_eq!(t.entries, [(2, 0), (3, 0), (1, 180), (4, 380)]);
        assert_eq!(knockback(50, 1, 0), 50);
        assert_eq!(knockback(50, 2, 20), 20);
    }

    #[test]
    fn equipment() {
        assert_eq!(equipment_stat(20, 0, 50.0), 20);
        assert_eq!(equipment_stat(20, 4, 100.0), 30);
        assert_eq!(equipment_stat(7, 0, 0.0), 5);
    }

    #[test]
    fn mp() {
        use crate::rng::Rng;
        let mut r = Rng::from_state([1, 2, 3, 4]);
        let before = r.state();
        assert_eq!(mp_cost(&[(0, 64, 80), (11, 12, 12)], &mut r), 12);
        assert_eq!(r.state(), before);
        assert_eq!(mp_power(6, 12), 50);
        assert_eq!(mp_power(30, 12), 100);
        assert_eq!(mp_power(0, 0), 100);
    }

    #[test]
    fn action_waits() {
        let mut none = -1;
        // Speed 50: 10% off 600.
        assert_eq!(action_wait(600, 50, None, None, &mut none), 540);
        // Speed 300 caps at 25%.
        assert_eq!(action_wait(600, 300, None, None, &mut none), 450);
        assert_eq!(action_wait(600, 0, Some(31), Some(80), &mut none), 332);
        let mut pending = 40;
        assert_eq!(action_wait(600, 0, None, None, &mut pending), 640);
        assert_eq!(pending, 0);
    }

    #[test]
    fn wait_after_acting() {
        use crate::rng::Rng;
        let seed = [123456789, 362436069, 521288629, 88675123];
        // The first draw is 3701687786: & 0x7fffffff = 1554204138, % 100 = 38.
        let mut r = Rng::from_state(seed);
        assert_eq!(wait_after_action(600, Some(39), None, true, &mut r), 450);
        let mut r = Rng::from_state(seed);
        assert_eq!(wait_after_action(600, Some(38), None, true, &mut r), 600);
        let mut r = Rng::from_state(seed);
        assert_eq!(wait_after_action(600, None, None, true, &mut r), 600);
        assert_eq!(r.state(), seed);
        assert_eq!(wait_after_action(2000, None, None, true, &mut r), MAX_WAIT);
        assert_eq!(wait_after_action(600, None, None, false, &mut r), 0);
    }

    #[test]
    fn reductions() {
        let none = Defender::default();
        assert_eq!(reduce(200, None, &none), 200);
        let d = Defender { resist: [50, 0, 150, 0], auto_reduce: Some(90), ..none };
        assert_eq!(reduce(200, Some(0), &d), 100);
        assert_eq!(reduce(200, Some(2), &d), 0);
        assert_eq!(reduce(200, None, &d), 40);
        // The equipment reduction replaces the automatic one.
        let d = Defender { equip_reduce: Some(10), ..d };
        assert_eq!(reduce(200, None, &d), 180);
        let d = Defender { absorb: Some(50), ..none };
        assert_eq!(reduce(201, Some(1), &d), -100);
    }
}
