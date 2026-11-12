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

/// The power an item's effect deals with, in place of the user's attack (the effect entry's
/// +0x18, filled by `0x14029d270` when it builds an item's effects): the value at +4 of the
/// held item (a float, presumably its quality) halved, plus 75, truncated: quality 50 gives
/// 100, quality 100 gives 125.
pub fn item_power(value: f32) -> i32 {
    // The game divides by 100 and multiplies back before truncating; the same number.
    (value * 0.5 + 75.0) as i32
}

/// A held item as a battle action (`0x14029d270`): its effects are the `item_effect` rows its
/// instance names (four 16-bit ids at +0x10, -1 for none; rows up to 0x2b9), each row's slots
/// in order, all at the item's power ([`item_power`] of its value). Not in yet: the override
/// record some items have (`0x1400e3480`: one id swapped for another) and the effects its kind-7
/// traits add (instance +0x8, `0x1402a1e20`).
pub fn item_action(name: &str, target: i32, wait: i32, effect_ids: &[i16], rows: &[barrel_formats::battle_data::ItemEffect]) -> barrel_formats::battle_data::Skill {
    let effects = effect_ids
        .iter()
        .filter(|&&id| (0..=0x2b9).contains(&id))
        .filter_map(|&id| rows.get(id as usize))
        .flat_map(|row| row.effects.iter().copied())
        .collect();
    barrel_formats::battle_data::Skill { name: name.to_string(), category: String::new(), wait, target, spread: 0, effects, description: String::new() }
}

/// The chance an instant-death act kills (the handlers at `0x1401bfd59`, `0x1401bfd9f` and
/// `0x1401bfdfe` in `0x1401bde10`'s switch), or `None` for any other act:
/// - `PERCENT_DEATH` (80): the amount + 5 × (user's level − target's level), percent;
/// - `DEATH_PERCENT` (176): certain when the target's HP is at most the amount percent of its
///   maximum (`HP × 100 / max(max HP, 1)`), else nothing;
/// - `ABSOLUTE_DEATH` (195): certain, and past the immunities ([`death_blocked`] does not apply).
pub fn death_chance(act: u32, amount: i32, user_level: i32, target: &crate::fight::Stats) -> Option<i32> {
    match act {
        80 => Some(amount + 5 * (user_level - target[stat::LEVEL])),
        176 => (amount >= target[stat::HP] * 100 / target[stat::MAX_HP].max(1)).then_some(100),
        195 => Some(100),
        _ => None,
    }
}

/// Whether a death event misses its target (`0x1401c1340`): under state 0x50, or when the
/// target's virtual +0x40 says so (not read yet; `immune`). Otherwise it rolls its chance
/// ([`crate::rng::Rng::chance`]).
/// The monsters an encounter group brings (`0x14019da40`): a count rolled between its least and
/// most, then its slots in order, each giving its monster up to its own most until the count is
/// reached (so later slots fill only what earlier ones leave). The game places them itself (each
/// comes with the position (0, -1, 0), "place it").
pub fn encounter_monsters(group: &barrel_formats::battle_data::EncounterGroup, rng: &mut crate::rng::Rng) -> Vec<i32> {
    let mut left = rng.range(group.count.0, group.count.1);
    let mut out = Vec::new();
    for &(monster, most) in &group.monsters {
        let mut n = most;
        while n > 0 && left > 0 {
            out.push(monster);
            n -= 1;
            left -= 1;
        }
    }
    out
}

/// Where the battle puts the monsters it places itself (`0x1401a3d00`; those whose setup
/// position is (0, -1, 0)). They go largest first (`+0x358`; ties keep their setup order here,
/// where the game's sort may not); the first stands at the centre, each other at a rolled angle
/// (0 to 360°) and distance (200 plus 0 to 100 × 2): (d sin θ, 0, d cos θ). Each also gets a
/// sector, its angle / 120 (the first's angle counts as 180). Returns, per monster in setup
/// order, its spot and sector. The game then nudges each spot (`0x1401b53b0`, not read yet).
pub fn scatter(sizes: &[f32], rng: &mut crate::rng::Rng) -> Vec<([f32; 3], i32)> {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| sizes[b].total_cmp(&sizes[a]));
    let mut out = vec![([0.0; 3], 0); sizes.len()];
    for (k, &i) in order.iter().enumerate() {
        out[i] = if k == 0 {
            ([0.0; 3], 180 / 120)
        } else {
            let angle = rng.range(0, 360);
            let distance = 200.0 + rng.range(0, 100) as f32 * 200.0 / 100.0;
            let t = (angle as f32).to_radians();
            ([distance * t.sin(), 0.0, distance * t.cos()], angle / 120)
        };
    }
    out
}

pub fn death_blocked(states: &[TimedState], immune: bool) -> bool {
    immune || states.iter().any(|s| s.state == 0x50)
}

/// The traits' sum of `ITEM_SUB_*` kind number `k` (`kinds` names them in enum order;
/// [`trait_value`], each slot rolled).
pub fn kind_sum(traits: &[&barrel_formats::battle_data::Trait], kinds: &[&str], k: usize, rng: &mut crate::rng::Rng) -> i32 {
    kinds.get(k).map_or(0, |name| trait_value(traits, name, false, rng))
}

/// The effects an item's traits add to its action (`0x14029d530`): each slot of kind
/// `ITEM_SUB_BTL_ADD_USEEFFECT` rolled from its minimum to maximum (`0x1402a1e20`) names an
/// `item_effect` row, whose used slots follow the item's own effects. The game does this only
/// while the instance's +0xe is 0 (not read yet; taken as always).
pub fn trait_effects(traits: &[&barrel_formats::battle_data::Trait], rows: &[barrel_formats::battle_data::ItemEffect], rng: &mut crate::rng::Rng) -> Vec<barrel_formats::battle_data::SkillEffect> {
    traits
        .iter()
        .flat_map(|t| t.slots.iter())
        .filter(|s| s.kind == "ITEM_SUB_BTL_ADD_USEEFFECT")
        .map(|s| rng.range(s.min, s.max))
        .filter(|&id| (0..=0x2b9).contains(&id))
        .filter_map(|id| rows.get(id as usize))
        .flat_map(|row| row.effects.iter().copied())
        .collect()
}

/// What an item's traits give of one kind (`0x14029db00 (item, kind, average)`): over its
/// traits (up to three, instance +0x8), each slot of that `ITEM_SUB_*` kind rolled from its
/// minimum to maximum (`Rng::range`), or its middle when `average`, summed.
pub fn trait_value(traits: &[&barrel_formats::battle_data::Trait], kind: &str, average: bool, rng: &mut crate::rng::Rng) -> i32 {
    traits
        .iter()
        .flat_map(|t| t.slots.iter())
        .filter(|s| s.kind == kind)
        .map(|s| if average { (s.min + s.max) / 2 } else { rng.range(s.min, s.max) })
        .sum()
}

/// A party member's chance to survive a fatal blow at 1 HP ("guts", the party character's
/// virtual +0x130, `0x1401b8850`, run on each target after an effect): only when its HP is 0
/// and guts has not already saved it since its last heal (+0x28d); stat 25 percent, at most
/// 75, or 100 under `ADD_GUTS` (state 4, which it then loses) or while its +0x2b8 or +0x2c0
/// is positive. The chance roll (`0x1404ca5f0`) is always drawn when it gets that far.
pub fn guts_chance(stat25: i32, add_guts: bool, forced: bool) -> i32 {
    if add_guts || forced { 100 } else { stat25.min(75) }
}

/// What an item's trait bonus to its effects is worked from (`0x1401bd8ee` and on) besides its
/// traits' sums by `ITEM_SUB_*` kind ([`item_bonus`] asks for those): the item's and battle's
/// numbers the terms multiply.
#[derive(Clone, Copy, Debug)]
pub struct ItemBonusInputs {
    /// `0x14029dd10 (item)`, taken from 5 for the rank bonus (the item's rank, presumably).
    pub rank: i32,
    /// `0x14029dc50 (item)` (its cost, presumably).
    pub cost: i32,
    /// The item's i16 +0x1c (uses left, presumably: 1 gives the last-use bonus), +0x1e and
    /// +0x20 (summed for `COUNT_POWER`), +0x18 (for `COST_PROPORTION_BONUS`).
    pub uses_left: i32,
    pub counts: i32,
    pub x18: i32,
    /// Under `FIELD_ITEM_SINGLE_USE` (state 0x4f): the last-use bonus always counts.
    pub single_use: bool,
    /// For a monster target, `0x140111a80 (its +0x78)` + 1 (its size class, presumably);
    /// `None` for a party target, whose big and small enemy terms are skipped.
    pub monster_size: Option<i32>,
    /// How many targets the action has.
    pub targets: i32,
    /// `0x140190b90 (timeline, user)`, for `WT_PROPORTION_BONUS`.
    pub wait_term: i32,
}

impl Default for ItemBonusInputs {
    fn default() -> Self {
        ItemBonusInputs { rank: 0, cost: 0, uses_left: 0, counts: 0, x18: 0, single_use: false, monster_size: None, targets: 1, wait_term: 0 }
    }
}

/// An item action's bonus percentage (the sum `0x1401bd8ee` builds for each flag-3 effect; cut
/// to the act's bonus limit by [`boost_amounts`]). `sum(k)` is the traits' sum of kind `k`
/// (`0x14029db00 (item, k, 0)`, [`kind_sum`]), asked for in the game's order (0, 25, 31, 28
/// at the last use, 35, 34, 29 and 30 on a monster, 26, 27, 41, 42): each ask rolls.
pub fn item_bonus(i: &ItemBonusInputs, mut s: impl FnMut(usize) -> i32) -> i32 {
    let mut b = s(0);
    b += s(25).wrapping_mul(5 - i.rank);
    b += s(31).wrapping_mul(i.cost);
    if i.uses_left == 1 || i.single_use {
        b += s(28);
    }
    b += s(35).wrapping_mul(i.counts);
    b += s(34).wrapping_mul(i.uses_left);
    if let Some(size) = i.monster_size {
        b += s(29).wrapping_mul(size);
        b += s(30) / size.max(1);
    }
    let targets = i.targets.max(1);
    b += s(26).wrapping_mul(targets);
    b += ((s(27) as i64 as u64) / targets as u64) as i32;
    b += s(41).wrapping_mul(i.x18);
    (s(42).wrapping_mul(i.wait_term) as f32 / 100.0 + b as f32) as i32
}

/// The rolled amount and second value raised by a bonus (`0x1401bdbcc`): the bonus is cut to
/// `limit` (the act's bonus limit, `0x14002aa40`), both values are multiplied by
/// `(100 + bonus) / 100` (integer), and `extra` is added to the amount.
pub fn boost_amounts(amount: i32, second: i32, bonus: i32, limit: i32, extra: i32) -> (i32, i32) {
    let b = bonus.min(limit) + 100;
    (amount.wrapping_mul(b) / 100 + extra, second.wrapping_mul(b) / 100)
}

/// The flat addition an item's `CHANGE_ACTUAL_POWER` traits (kind 39) give an act with flag 4
/// (`0x1401bdad5`): their sum `v` when the amount is at most `v`, else `v² / amount`.
pub fn actual_power_extra(amount: i32, v: i32) -> i32 {
    if v <= 0 {
        0
    } else if amount <= v {
        v
    } else {
        v.wrapping_mul(v) / amount
    }
}

/// `HEAL_HP_BY_ATK`'s amount (act 21, `0x1401bedf3`): the user's attack times the rolled amount
/// in percent, cut to an integer, then times the scale. (`HEAL_HP`, act 19, is the rolled
/// amount times the scale.)
pub fn heal_by_attack(attack: i32, amount: i32, scale: f32) -> i32 {
    (((attack.wrapping_mul(amount)) / 100) as f32 * scale) as i32
}

/// `CURE_REVIVE`'s amount (act 81, the revive event `0x1401c1e90`): `value` percent of the
/// target's maximum HP, cut to an integer; nothing under `FIELD_INVALID_REVIE` (state 0x4d),
/// nor under `FIELD_INVALID_HEAL` (0x4e) for a target still standing. It is given through
/// [`heal`], so it raises a fallen character from 0 HP.
pub fn revive_amount(max_hp: i32, value: i32, invalid_revive: bool, invalid_heal: bool, standing: bool) -> i32 {
    if invalid_revive || (invalid_heal && standing) {
        return 0;
    }
    max_hp.wrapping_mul(value) / 100
}

/// Heals a character (`0x1401afa20`, the heal event's apply): HP goes up by `amount`, no higher
/// than the maximum; under `FIELD_INVALID_HEAL` (state 0x4e) a character still standing is not
/// healed. Returns the HP gained. (The game also clears the character's +0x28d.)
pub fn heal(stats: &mut [i32; stat::COUNT], amount: i32, blocked: bool) -> i32 {
    if blocked && stats[stat::HP] > 0 {
        return 0;
    }
    let before = stats[stat::HP];
    stats[stat::HP] = stats[stat::HP].wrapping_add(amount).min(stats[stat::MAX_HP]);
    stats[stat::HP] - before
}

/// A party member's MP cost for a skill (virtual +0xb8 of the party character, `0x1401b82e0`,
/// through `0x14032f200`): the list's cost cut by `equipment_cut` percent (the equipment's
/// effects of kind 0x16, summed by `0x14032f7c0`), less `leader_cut` when the game's
/// `0x14035ebb0` names this member (a value from `0x14002f2f0 (0x10)`, not read further), not
/// under 0; then raised by the `CHANGE_SKILLCOST` (state 0x2e) values in percent.
pub fn party_mp_cost(listed: i32, equipment_cut: i32, leader_cut: i32, skillcost: i32) -> i32 {
    let cost = ((100 - equipment_cut).wrapping_mul(listed) / 100 - leader_cut).max(0);
    (100 + skillcost).wrapping_mul(cost) / 100
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

/// A state's exclusion group (`0x140191db0`): states of one group replace each other. Each
/// `PLUS_*` stat state shares one with its `MINUS_*` (0x12 and 0x26 group 1, ... 0x19 and 0x2d
/// group 9, skipping 4); 0 and 1 are group 10, 0x1d and 0x1e 11, 0x1a 12, 0x11 and 0x2f 13;
/// the rest have none.
pub fn state_group(state: u32) -> Option<u32> {
    Some(match state {
        0 | 1 => 10,
        0x1d | 0x1e => 11,
        0x1a => 12,
        0x11 | 0x2f => 13,
        0x12..=0x14 => state - 0x11,
        0x15..=0x19 => state - 0x10,
        0x26..=0x28 => state - 0x25,
        0x29..=0x2d => state - 0x24,
        _ => return None,
    })
}

/// Adds a timed state (`0x140191340`). The state it meets is the first of its group, or, for
/// a state with no group, the first of the same number. A meeting state of the same number
/// with no count stays if the new value is not stronger (compared as floats); in every other
/// case the meeting state goes and the new one is added at the end, so a buff wipes the
/// matching debuff and a poison put on again starts its count afresh.
pub fn add_state(states: &mut Vec<TimedState>, state: u32, value: i32, second: i32) {
    let new = TimedState { state, count: state_count(state, second), value: value as f32 };
    let group = state_group(state);
    let meets = |s: &TimedState| match group {
        Some(g) => state_group(s.state) == Some(g),
        None => s.state == state,
    };
    if let Some(i) = states.iter().position(meets) {
        if states[i].state == state && states[i].count == -1 && new.value <= states[i].value {
            return;
        }
        states.remove(i);
    }
    states.push(new);
}

/// How much defence softens damage over time (`0x1401b9050`, `0x1401b90c0`): trunc(d / 10) +
/// trunc(d / 3), kept to 0..=999.
fn over_time_cut(defense: i32) -> i32 {
    (defense / 10 + defense / 3).clamp(0, 999)
}

/// A turn's poison (state 0x34, `DAMAGE_HP_TURN`; `0x1401b9050`): `value` (the states' values
/// added up) cut by the character's defence, at least 1.
pub fn poison_damage(value: i32, defense: i32) -> i32 {
    let v = (100 - over_time_cut(defense)).wrapping_mul(value);
    if v < 200 { 1 } else { v / 100 }
}

/// A turn's poison by missing HP (state 0x35, `DAMAGE_HP_TURN_DIFFERENTIAL`; `0x1401b90c0`):
/// `value` percent of the HP missing, cut by the defence, at least 1.
pub fn poison_by_missing_hp(value: i32, hp: i32, max_hp: i32, defense: i32) -> i32 {
    let v = (100 - over_time_cut(defense)).wrapping_mul(max_hp - hp).wrapping_mul(value);
    if v < 20000 { 1 } else { v / 10000 }
}

/// The HP change at a character's turn start (`0x1401b0100`): the `HEAL_HP_TURN` amounts (and
/// the two HP traits) minus the poisons, netted into one number. Positive is healed (a heal
/// event, `0x1401c1d00`), negative is dealt as ordinary damage of the opposite amount through
/// `0x1401c1970` (so [`hit_amount`] applies), 0 is nothing.
pub fn turn_hp_change(heal: i32, poison: i32, poison_by_missing: i32) -> i32 {
    heal.wrapping_sub(poison).wrapping_sub(poison_by_missing)
}

/// Ages a character's timed states by one of its turns (`0x140191120`, called at its turn's
/// start after the HP change): counted states go down by one and go when they reach 0; states
/// without a count stay. (The game passes the character's +0x2d8 as a float; that it is one a
/// turn is taken, not read.)
pub fn age_states(states: &mut Vec<TimedState>) {
    for s in states.iter_mut() {
        if s.count > 0 {
            s.count -= 1;
        }
    }
    states.retain(|s| s.count != 0);
}

/// The battle stats a timed state changes by its value (`0x140191e90`): attack (5), defence
/// (6), speed (7) for the `PLUS_*` (0x12-0x14), `MINUS_*` (0x26-0x28) and two further families
/// (0x36-0x38, 0x3e-0x40); stats 8 to 12 for 0x15-0x19 and their kin; 13 (critical) for 0x1b,
/// 14 for 0x24, 18 (evade) for 0x1c; all three of attack, defence and speed for 0x49
/// (`FIELD_ACUMULATE_WEAK`). State 0x11 (`PLUS_LEVEL_STATUS`) works from the attack and is not
/// worked here; the rest change no stat.
pub fn state_stats(state: u32) -> &'static [usize] {
    use stat::*;
    const S: [usize; 8] = [ATTACK, DEFENSE, SPEED, 8, 9, 10, 11, 12];
    match state {
        0x12..=0x19 => &S[(state - 0x12) as usize..][..1],
        0x26..=0x2d => &S[(state - 0x26) as usize..][..1],
        0x36..=0x3d => &S[(state - 0x36) as usize..][..1],
        0x3e..=0x45 => &S[(state - 0x3e) as usize..][..1],
        0x1b => &[13],
        0x24 => &[14],
        0x1c => &[18],
        0x49 => &S[..3],
        _ => &[],
    }
}

/// A character's battle stats with its timed states (`0x1401af440`, run whenever a state is
/// added or goes): the base stats, keeping the current HP and MP; each state's value added to
/// the stats it changes ([`state_stats`]; `MINUS_*` amounts were negated when the effect was
/// applied); the maximum HP raised by the `ADD_MAXHP` (0x1a) percentages; then the maximum HP
/// and MP kept to 1..=`cap` (999, or 999,999 for monsters), HP and MP to 0..=the maximum, and
/// attack, defence and speed to 1..=999.
pub fn stats_with_states(base: &[i32; stat::COUNT], current: &[i32; stat::COUNT], states: &[TimedState], cap: i32) -> [i32; stat::COUNT] {
    use stat::*;
    let mut out = *base;
    out[HP] = current[HP];
    out[MP] = current[MP];
    for s in states {
        for &k in state_stats(s.state) {
            out[k] = out[k].wrapping_add(s.value as i32);
        }
    }
    let more: i32 = states.iter().filter(|s| s.state == 0x1a).map(|s| s.value as i32).sum();
    out[MAX_HP] = out[MAX_HP].wrapping_add(out[MAX_HP].wrapping_mul(more) / 100);
    for k in [MAX_HP, MAX_MP] {
        out[k] = out[k].min(cap).max(1);
    }
    out[HP] = out[HP].min(out[MAX_HP]).max(0);
    out[MP] = out[MP].min(out[MAX_MP]).max(0);
    for k in [ATTACK, DEFENSE, SPEED] {
        out[k] = out[k].clamp(1, 999);
    }
    out
}

/// The timed state an `ACT_*` buff or debuff puts on (part of `0x1401c0e80`): `PLUS_ATK`,
/// `PLUS_DEF`, `PLUS_SPD` (acts 40-42) give 0x12-0x14, `MINUS_ATK`, `MINUS_DEF`, `MINUS_SPD`
/// (45-47) 0x26-0x28, the minuses with their amount negated (`0x1401bf1da`).
pub fn stat_state(act: u32) -> Option<(u32, bool)> {
    match act {
        40..=42 => Some((0x12 + act - 40, false)),
        45..=47 => Some((0x26 + act - 45, true)),
        _ => None,
    }
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
    /// Everyone takes part; see [`Timeline::advance_with`] for fallen characters.
    pub fn advance(&mut self) -> i32 {
        self.advance_with(|_| false)
    }

    /// [`Timeline::advance`] where `out(character)` says whose entry is out of the running (a
    /// fallen character's: the down handler sets the entry's +0x10 to -1, `0x14018ccf0`):
    /// such entries at the front are pushed back by 1000 (no further than 1000) and skipped,
    /// and the time taken off is the first entry still in the running's wait, off it and every
    /// entry after it.
    pub fn advance_with(&mut self, out: impl Fn(u32) -> bool) -> i32 {
        self.entries.sort_by_key(|&(_, w)| w);
        let mut first = 0;
        while first < self.entries.len() && out(self.entries[first].0) {
            self.entries[first].1 = (self.entries[first].1 + MAX_WAIT).min(MAX_WAIT);
            first += 1;
        }
        let Some(&(_, passed)) = self.entries.get(first) else { return 0 };
        for e in &mut self.entries[first..] {
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
    fn an_item_takes_its_rows_effects_in_order() {
        use barrel_formats::battle_data::{ItemEffect, SkillEffect};
        let fx = |act, min| SkillEffect { act, element: None, min, max: min, min2: 0, max2: 0 };
        let rows = vec![
            ItemEffect { name: "Heal S".into(), tag: String::new(), group: String::new(), effects: vec![fx(19, 30)] },
            ItemEffect { name: "Revive".into(), tag: String::new(), group: String::new(), effects: vec![fx(81, 20), fx(19, 5)] },
        ];
        let item = item_action("Salve", 1, 600, &[1, -1, 0, 0x300], &rows);
        assert_eq!(item.effects.iter().map(|e| (e.act, e.min)).collect::<Vec<_>>(), [(81, 20), (19, 5), (19, 30)]);
        assert_eq!((item.target, item.wait), (1, 600));
    }

    #[test]
    fn trait_values_sum_the_kind_asked_for() {
        use barrel_formats::battle_data::{Trait, TraitSlot};
        let slot = |kind: &str, min, max| TraitSlot { kind: kind.into(), min, max };
        let power = Trait { name: "Power+".into(), group: String::new(), slots: vec![slot("ITEM_SUB_ITM_CHANGE_POWER", 10, 10)] };
        let mixed = Trait { name: "Mixed".into(), group: String::new(), slots: vec![slot("ITEM_SUB_ITM_CHANGE_POWER", 4, 8), slot("ITEM_SUB_ITM_ADD_PRICE", 50, 50)] };
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        assert_eq!(trait_value(&[&power, &mixed], "ITEM_SUB_ITM_CHANGE_POWER", true, &mut rng), 16);
        assert_eq!(trait_value(&[&mixed], "ITEM_SUB_ITM_ADD_PRICE", false, &mut rng), 50);
        let rolled = trait_value(&[&mixed], "ITEM_SUB_ITM_CHANGE_POWER", false, &mut rng);
        assert!((4..=8).contains(&rolled));
    }

    #[test]
    fn traits_add_their_effect_rows() {
        use barrel_formats::battle_data::{ItemEffect, SkillEffect, Trait, TraitSlot};
        let fx = |act| SkillEffect { act, element: None, min: 5, max: 5, min2: 0, max2: 0 };
        let row = |act| ItemEffect { name: String::new(), tag: String::new(), group: String::new(), effects: vec![fx(act)] };
        let rows = [row(0), row(9), row(21)];
        let slot = |kind: &str, v| TraitSlot { kind: kind.into(), min: v, max: v };
        let t = Trait { name: "Fatal Blow".into(), group: String::new(), slots: vec![slot("ITEM_SUB_ITM_ADD_EFFCRIT", 100), slot("ITEM_SUB_BTL_ADD_USEEFFECT", 2)] };
        let none = Trait { name: String::new(), group: String::new(), slots: vec![slot("ITEM_SUB_BTL_ADD_USEEFFECT", -1)] };
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        assert_eq!(trait_effects(&[&t, &none], &rows, &mut rng), [fx(21)]);
    }

    #[test]
    fn item_power_from_quality() {
        assert_eq!((item_power(0.0), item_power(50.0), item_power(100.0), item_power(999.0)), (75, 100, 125, 574));
    }

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
    fn turn_start_nets_heal_and_poison() {
        assert_eq!(turn_hp_change(20, 8, 0), 12);
        assert_eq!(turn_hp_change(5, 8, 3), -6);
        assert_eq!(turn_hp_change(0, 0, 0), 0);
    }

    #[test]
    fn buffs_and_debuffs_change_stats() {
        let mut base = [0; stat::COUNT];
        base[stat::MAX_HP] = 200;
        base[stat::MAX_MP] = 50;
        base[stat::ATTACK] = 40;
        base[stat::DEFENSE] = 5;
        base[stat::SPEED] = 30;
        let mut current = base;
        current[stat::HP] = 150;
        let mut states = Vec::new();
        add_state(&mut states, 0x12, 15, 0);
        add_state(&mut states, 0x27, -10, 0);
        add_state(&mut states, 0x1a, 50, 0);
        let s = stats_with_states(&base, &current, &states, 999);
        assert_eq!((s[stat::ATTACK], s[stat::DEFENSE], s[stat::SPEED]), (55, 1, 30));
        assert_eq!((s[stat::MAX_HP], s[stat::HP]), (300, 150));
        assert_eq!(stat_state(46), Some((0x27, true)));
    }

    #[test]
    fn groups_replace() {
        let mut states = Vec::new();
        add_state(&mut states, 0x26, -10, 0);
        add_state(&mut states, 0x12, 15, 0);
        assert_eq!(states.iter().map(|s| (s.state, s.value)).collect::<Vec<_>>(), [(0x12, 15.0)]);
        add_state(&mut states, 0x34, 10, 3);
        age_states(&mut states);
        add_state(&mut states, 0x34, 4, 3);
        assert_eq!(states.iter().find(|s| s.state == 0x34).map(|s| (s.count, s.value)), Some((3, 4.0)));
        assert_eq!(state_group(0x19), Some(9));
        assert_eq!(state_group(0x2d), Some(9));
        assert_eq!(state_group(0x34), None);
    }

    #[test]
    fn fallen_entries_wait_out_the_turn() {
        let mut t = Timeline { entries: vec![(0, 100), (1, 200), (2, 300)] };
        // 0 has fallen: pushed to 1000, and 1's 200 passes for the others.
        assert_eq!(t.advance_with(|c| c == 0), 200);
        assert_eq!(t.entries, [(1, 0), (2, 100), (0, 1000)]);
        assert_eq!(guts_chance(90, false, false), 75);
        assert_eq!(guts_chance(10, true, false), 100);
    }

    #[test]
    fn revives() {
        assert_eq!(revive_amount(200, 15, false, false, false), 30);
        assert_eq!(revive_amount(200, 15, true, false, false), 0);
        assert_eq!(revive_amount(200, 15, false, true, true), 0);
        assert_eq!(revive_amount(200, 15, false, true, false), 30);
    }

    #[test]
    fn item_bonuses() {
        let i = ItemBonusInputs { rank: 3, cost: 10, uses_left: 1, targets: 2, ..Default::default() };
        let mut sums = [0; 43];
        sums[0] = 20;
        sums[25] = 5;
        sums[28] = 30;
        sums[26] = 4;
        // 20 + 5 × 2 + 30 (last use) + 4 × 2 targets.
        let mut asked = Vec::new();
        assert_eq!(item_bonus(&i, |k| { asked.push(k); sums[k] }), 68);
        // Not on a monster: 29 and 30 are never asked for.
        assert_eq!(asked, [0, 25, 31, 28, 35, 34, 26, 27, 41, 42]);
        assert_eq!(boost_amounts(100, 10, 68, 9999, 5), (173, 16));
        assert_eq!(boost_amounts(100, 10, 68, 50, 0), (150, 15));
        assert_eq!(actual_power_extra(20, 30), 30);
        assert_eq!(actual_power_extra(60, 30), 15);
    }

    #[test]
    fn heals() {
        let mut s = [0; stat::COUNT];
        s[stat::HP] = 50;
        s[stat::MAX_HP] = 100;
        assert_eq!(heal_by_attack(80, 50, 1.0), 40);
        assert_eq!(heal(&mut s, 40, false), 40);
        assert_eq!(heal(&mut s, 40, false), 10);
        assert_eq!(s[stat::HP], 100);
        s[stat::HP] = 10;
        assert_eq!(heal(&mut s, 40, true), 0);
    }

    #[test]
    fn party_mp_costs() {
        assert_eq!(party_mp_cost(15, 0, 0, 0), 15);
        assert_eq!(party_mp_cost(15, 20, 0, 0), 12);
        assert_eq!(party_mp_cost(15, 20, 20, 0), 0);
        assert_eq!(party_mp_cost(28, 0, 0, 50), 42);
    }

    #[test]
    fn poison() {
        // Defence 30: cut 3 + 10 = 13, so 10 poison does 8.
        assert_eq!(poison_damage(10, 30), 8);
        assert_eq!(poison_damage(1, 0), 1);
        assert_eq!(poison_by_missing_hp(10, 50, 100, 0), 5);
        let mut states = Vec::new();
        add_state(&mut states, 0x34, 10, 2);
        age_states(&mut states);
        assert_eq!(states[0].count, 1);
        age_states(&mut states);
        assert!(states.is_empty());
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

#[cfg(test)]
mod encounter_tests {
    use super::*;
    use barrel_formats::battle_data::EncounterGroup;

    #[test]
    fn slots_fill_in_order() {
        // Group 12: 3 or 4, Ore Rats (36) up to 2 then Stray Dogs (4) up to 2.
        let g = EncounterGroup { id: 12, kind: 0, count: (3, 4), monsters: vec![(36, 2), (4, 2)], extra: vec![] };
        let mut rng = crate::rng::Rng::seeded(1);
        for _ in 0..50 {
            let m = encounter_monsters(&g, &mut rng);
            assert!(m == [36, 36, 4] || m == [36, 36, 4, 4], "{m:?}");
        }
        // A count above the slots' sum stops at the sum.
        let g = EncounterGroup { count: (5, 5), monsters: vec![(1, 1), (2, 2)], ..g };
        assert_eq!(encounter_monsters(&g, &mut rng), [1, 2, 2]);
    }

    #[test]
    fn monsters_scatter_round_the_largest() {
        let mut rng = crate::rng::Rng::seeded(7);
        let spots = scatter(&[1.0, 3.0, 2.0], &mut rng);
        // The largest (second) at the centre, sector 1.
        assert_eq!(spots[1], ([0.0; 3], 1));
        for (k, (p, sector)) in spots.iter().enumerate().filter(|&(k, _)| k != 1) {
            let d = (p[0] * p[0] + p[2] * p[2]).sqrt();
            assert!((200.0..=400.01).contains(&d), "{k}: {d}");
            assert!((0..=3).contains(sector) && p[1] == 0.0);
        }
    }
}
