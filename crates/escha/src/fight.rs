//! A battle in progress: the fighters with their stats and timed states, the turn timeline,
//! and the steps a turn is made of, built from the rules in [`crate::battle`]. Whoever drives
//! it (the duel, later the player's commands and the monsters' rules) asks whose turn is next,
//! starts that turn, and has the fighter act; each step reports what happened as [`Event`]s.
//!
//! Not in yet (each a way it can differ from the game): traits, passives and the field states;
//! items; the battle's start rules (everyone joins at wait 500, in order); the effect-roll
//! globals (neutral: no chain bonus, full MP power); the action's float (1.0); effects other
//! than HP damage (acts 0 and 8), heals and revives (19, 21, 81), poison (16) and the stat buffs and debuffs
//! (40-42, 45-47);
//! and more than one target per action.

use crate::battle::{self, stat, Defender, Timeline, TimedState};
use barrel_engine::rng::Rng;
use crate::data::battle_data::{Skill, SkillEffect, StatBlock};

pub type Stats = [i32; stat::COUNT];

/// An item's trait bonus on its effects (`0x1401bd8ee` and on), rolled for each effect right
/// after its amounts: on a flag-3 act `battle::item_bonus` of its traits raises the amount and
/// second value (`battle::boost_amounts`, cut to the act's limit); on a flag-4 act its
/// `CHANGE_ACTUAL_POWER` sum (kind 39) is added as `battle::actual_power_extra`.
#[derive(Clone, Copy)]
pub struct Boost<'a> {
    pub traits: &'a [&'a crate::data::battle_data::Trait],
    /// The `ITEM_SUB_*` names in enum order.
    pub kinds: &'a [&'a str],
    pub inputs: battle::ItemBonusInputs,
    pub acts: &'a [crate::data::battle_data::ActInfo],
}

impl Boost<'_> {
    pub fn apply(&self, act: u32, amount: i32, second: i32, rng: &mut Rng) -> (i32, i32) {
        let Some(info) = self.acts.get(act as usize) else { return (amount, second) };
        let b = if info.flags[3] { battle::item_bonus(&self.inputs, |k| battle::kind_sum(self.traits, self.kinds, k, rng)) } else { 0 };
        // Flag 4: the CHANGE_ACTUAL_POWER sum, full or v² / amount (`battle::actual_power_extra`).
        let extra = if info.flags[4] { battle::actual_power_extra(amount, battle::kind_sum(self.traits, self.kinds, 39, rng)) } else { 0 };
        if b == 0 && extra == 0 {
            return (amount, second);
        }
        battle::boost_amounts(amount, second, b, info.limit, extra)
    }
}

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
    /// How big it stands (+0x358), counted in areas' reach and in the formation's spacing.
    pub size: f32,
    /// What it adds to its weight as a random target (virtual +0x128; not read yet, 0).
    pub draw: i32,
    /// The character a monster remembers (+0x314; -1 none), for target pick 4.
    pub remembered: i32,
    /// The index into its skill list of the skill its latest action uses (+0x2d0, set as each
    /// action starts; monster rule condition 3 reads it). 0 before its first action (the
    /// constructor's value is not read).
    pub last_skill: i32,
    /// Whether guts has saved it since its last heal (+0x28d).
    pub guts_used: bool,
    /// Instant death never lands on it (a monster's `death_immune`; party members never are).
    pub death_immune: bool,
    /// Its place around the battle (+0x394): a monster's sector (0 to 3, `battle::scatter`), a
    /// party member's sector plus 3; -1 when not placed (no flank attacks then).
    pub slot: i32,
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
        Fighter { side, base: s, stats: s, states: Vec::new(), actions: 0, uses: [0; 9], cap, position: [0.0; 3], size: 0.0, draw: 0, remembered: -1, last_skill: 0, guts_used: false, death_immune: false, slot: -1 }
    }

    /// With `power` as its skill power (stat 15, [`stat::POWER`]).
    pub fn with_power(mut self, power: i32) -> Fighter {
        self.base[stat::POWER] = power;
        self.stats[stat::POWER] = power;
        self
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
    /// An instant-death effect felled the target.
    Felled { who: usize, skill: String, target: usize },
    /// A fighter tried to run (the Run command); `escaped` ends the battle.
    Ran { who: usize, chance: i32, escaped: bool },
    /// A party member moved to another sector (the Move command).
    Moved { who: usize, sector: i32 },
    /// A party action's damage rate as it ends (global `0x141077e70`, the battle screen's
    /// "Damage Rate: (100 + rate) %"): its steps before the hits and each effect's after.
    Rate { who: usize, rate: i32, support: i32 },
}

pub struct Battle {
    pub fighters: Vec<Fighter>,
    pub timeline: Timeline,
    /// The pending wait change of `0x1401b9b70` (global 0x141077e54).
    pending: i32,
    /// The battle's tries to run so far (global `0x141077dcc`).
    tries: i32,
    /// A run succeeded (`0x141077e30`): the battle is over without a winner.
    pub escaped: bool,
    /// Each act's damage-rate step, added once an action's effects are applied (the act
    /// table's last word, `battle_data::ActInfo::last`: 5 for acts 0 and 2); none when empty.
    pub act_steps: Vec<i32>,
    /// The Support gauge (`battle::support_gain`): 0 to 900.
    pub support: i32,
    /// A Support Guard waiting for the next monster action on a party member: the guard and
    /// the share of the hit it takes off (`ASSIST_GUARD`'s amount, set on its +0x2b8).
    pub guard: Option<(usize, i32)>,
}

impl Battle {
    /// The timeline set up as the game's battle start (`Timeline::setup`) but without its
    /// random part: the fighters fastest first (by speed, the given order on ties), 100 apart
    /// with free entries between. [`Battle::rolled`] orders them as the game does.
    pub fn new(fighters: Vec<Fighter>) -> Battle {
        let mut order: Vec<u32> = (0..fighters.len() as u32).collect();
        order.sort_by_key(|&k| std::cmp::Reverse(fighters[k as usize].stats[stat::SPEED]));
        let timeline = Timeline::setup(&order);
        Battle { fighters, timeline, pending: -1, tries: 0, escaped: false, act_steps: Vec::new(), support: 0, guard: None }
    }

    /// A battle started as the game starts one (`0x14018c0c0`): every fighter rolls 0 to 50
    /// (`rng`, in `fighters`' order, the party first as the game's list has it) and adds its
    /// speed, and [`battle::opening_order`] makes the turn order of those.
    pub fn rolled(fighters: Vec<Fighter>, rng: &mut Rng) -> Battle {
        let rolled: Vec<(i32, u8)> = fighters.iter().map(|f| (rng.range(0, battle::INITIATIVE_ROLL) + f.stats[stat::SPEED], f.side)).collect();
        let timeline = Timeline::setup(&battle::opening_order(&rolled));
        Battle { fighters, timeline, pending: -1, tries: 0, escaped: false, act_steps: Vec::new(), support: 0, guard: None }
    }

    /// The act table's damage-rate steps ([`Battle::act_steps`]).
    pub fn with_act_steps(mut self, steps: Vec<i32>) -> Battle {
        self.act_steps = steps;
        // The gauge's start goes with the battle's own setup (`0x14019f813`).
        self.support = battle::SUPPORT_START;
        self
    }

    /// `who` waits (the Wait command, `BtlCharaActionWait`): nothing happens but its next wait,
    /// [`battle::WAIT_COMMAND`] taken as an action's (its execute only plays a pose).
    pub fn wait_turn(&mut self, who: usize, rng: &mut Rng) {
        self.end_action(who, battle::WAIT_COMMAND, rng);
    }

    /// `who` moves to `sector` (the Move command, `BtlCharaActionMove`, execute `0x1401bbeb0`):
    /// its place (+0x394) is the new sector's before the action runs, and the execute only
    /// shows the run there: `0x14019c310` hands the battle script's `setCommandMove` the place's
    /// spot and facing (the slot angles of `0x1401b0d40`), the action's +0x3c (which way round)
    /// and whether the member is the centre one (+0x74 = 0), and the script runs it there; the
    /// HUD's notice (`0x1401c9f60`) does nothing. Then the Move wait ([`battle::MOVE_COMMAND`]).
    /// Who stood on the sector before is not moved (`docs/escha/open.md`).
    pub fn move_turn(&mut self, who: usize, sector: i32, rng: &mut Rng) -> Event {
        let sector = sector.rem_euclid(3);
        let f = &mut self.fighters[who];
        f.slot = sector + 3;
        f.position = battle::party_spot(who as i32, sector);
        self.end_action(who, battle::MOVE_COMMAND, rng);
        Event::Moved { who, sector }
    }

    /// `who` tries to run (the Run command, `BtlCharaActionEscape`, execute `0x1401ba560`): a
    /// chance roll on its speed less the action's value plus 60, and 30 more for each try after
    /// the third. The action's value, read live, is the fastest standing enemy's speed (19 with
    /// enemies of 19, 19, 15 and 15: the first enemy's would fit as well, `docs/escha/open.md`).
    /// The actor's trait 0x82 bonus is not added: Barrel's fighters do not carry traits yet.
    /// Escaped, the battle ends; else the try takes Run's wait ([`battle::RUN_COMMAND`]).
    pub fn run_turn(&mut self, who: usize, rng: &mut Rng) -> Event {
        let side = self.fighters[who].side;
        let top = self.fighters.iter().filter(|f| f.side != side && !f.down()).map(|f| f.stats[stat::SPEED]).max().unwrap_or(0);
        let mut chance = self.fighters[who].stats[stat::SPEED] - top + 60;
        if self.tries > 3 {
            chance += 30 * (self.tries - 3);
        }
        let escaped = rng.chance(chance);
        self.tries += 1;
        if escaped {
            self.escaped = true;
        } else {
            self.end_action(who, battle::RUN_COMMAND, rng);
        }
        Event::Ran { who, chance, escaped }
    }

    /// The pending wait change (`action_wait`'s `pending`), as the next action would find it.
    pub fn pending(&self) -> i32 {
        self.pending
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
            1..=3 => {
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
        self.timeline.entries.iter().map(|&(c, _)| c).find(|&c| c != crate::battle::FREE && !fallen[c as usize]).unwrap_or(0) as usize
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
        self.act_with(who, skill, slot, target, None, None, rng)
    }

    /// An item used by `who` on `target` (`BtlCharaActionItem`): its effects, in a skill's
    /// shape, act as a skill's do, except that its power (`battle::item_power` of the held
    /// item's value: the effect entry's +0x18) stands in for the user's stat 5 in the damage
    /// (`0x1401bdf23`: any action kind but 0). Not in yet: the flag 3 and 4 bonuses from the
    /// item's traits, and using the item up.
    pub fn use_item(&mut self, who: usize, item: &Skill, power: i32, target: usize, rng: &mut Rng) -> Vec<Event> {
        self.act_with(who, item, None, target, Some(power), None, rng)
    }

    /// [`Battle::use_item`] with the item's trait bonus ([`Boost`]).
    pub fn use_item_boosted(&mut self, who: usize, item: &Skill, power: i32, boost: Boost, target: usize, rng: &mut Rng) -> Vec<Event> {
        self.act_with(who, item, None, target, Some(power), Some(boost), rng)
    }

    fn act_with(&mut self, who: usize, skill: &Skill, slot: Option<usize>, target: usize, power: Option<i32>, boost: Option<Boost>, rng: &mut Rng) -> Vec<Event> {
        if let Some(k) = slot.filter(|&k| k < 9) {
            self.fighters[who].uses[k] += 1;
        }
        // As the action starts (`0x1401bac40`): a critical roll for each target (chance 5 plus
        // the actor's value, its virtual +0x100, and passive 0x1b's: not read, taken as 0).
        // A Support Guard steps in for a party member a monster's single-target action aims at
        // (`0x1401c0af0`: the guard's +0x2b8 takes its share off, `0x141077dfc` names it).
        let guarding = match self.guard {
            Some((g, share)) if self.fighters[who].side == 1 && skill.spread <= 0 && target != g && self.fighters[target].side == 0 && !self.fighters[g].down() => {
                self.guard = None;
                Some((g, share))
            }
            _ => None,
        };
        let target = guarding.map_or(target, |(g, _)| g);
        let targets = self.area(who, target, skill.spread);
        let mut critical = false;
        for _ in &targets {
            critical |= rng.chance(5);
        }
        // The damage rate (`docs/tables.md`, "HP damage"): cleared as each action starts, a party
        // action's steps before its hits scale them: 25 for a flank attack (a target on the
        // other side whose sector the actor's slot does not face), 10 for a critical.
        let rate = if self.fighters[who].side == 0 {
            let slot = self.fighters[who].slot;
            let flank = slot >= 0 && targets.iter().any(|&t| self.fighters[t].side == 1 && self.fighters[t].slot >= 0 && slot != battle::facing_slot(self.fighters[t].slot));
            25 * i32::from(flank) + 10 * i32::from(critical)
        } else {
            0
        };
        let scale = (100 + rate) as f32 / 100.0;
        // The rolls first (`0x1401bd520`): each effect's amount, then its second value, then an
        // item's bonus for it.
        let rolled: Vec<(SkillEffect, i32, i32)> = skill
            .effects
            .iter()
            .map(|e| {
                let amount = rng.range(e.min, e.max);
                let second = rng.range(e.min2, e.max2);
                let (amount, second) = boost.map_or((amount, second), |b| b.apply(e.act, amount, second, rng));
                (*e, amount, second)
            })
            .collect();
        // Then, on each fighter in the skill's area (`Battle::area`), each effect in its slot's
        // order (`0x1401bde10`), the same rolled amounts for all.
        // Fellings, reported after the action itself.
        let mut felled = Vec::new();
        let mut events = Vec::new();
        let mut hits = 0;
        let count = targets.len();
        for target in targets {
        let was_down = self.fighters[target].down();
        let (mut damage, mut healed, mut heals) = (0, 0, false);
        for &(e, amount, second) in &rolled {
            match e.act {
                // Poison (`DAMAGE_HP_TURN`) on the target.
                16 => battle::add_state(&mut self.fighters[target].states, 0x34, amount, second),
                // Instant death (`battle::death_chance`): the target's HP to 0 when it lands.
                80 | 176 | 195 => {
                    let t = &self.fighters[target];
                    let Some(chance) = battle::death_chance(e.act, amount, self.fighters[who].stats[stat::LEVEL], &t.stats) else { continue };
                    let lands = e.act == 195 || (!battle::death_blocked(&t.states, t.death_immune) && rng.chance(chance));
                    if lands && !t.down() {
                        self.fighters[target].stats[stat::HP] = 0;
                        felled.push(Event::Felled { who, skill: skill.name.clone(), target });
                    }
                }
                // Heals (`HEAL_HP`, 19; `HEAL_HP_BY_ATK`, 21) through the heal event
                // (`0x1401c1d00`) and revives (`CURE_REVIVE`, 81, the revive event `0x1401c1e90`).
                19 | 21 | 81 => {
                    heals = true;
                    let t = &self.fighters[target];
                    let has = |state| t.states.iter().any(|s| s.state == state);
                    let (amount, blocked) = match e.act {
                        81 => (battle::revive_amount(t.stats[stat::MAX_HP], amount, has(0x4d), has(0x4e), !t.down()), false),
                        21 => (battle::heal_by_attack(self.fighters[who].stats[stat::ATTACK], amount, 1.0), has(0x4e)),
                        _ => (amount, has(0x4e)),
                    };
                    let was_down = self.fighters[target].down();
                    healed += battle::heal(&mut self.fighters[target].stats, amount, blocked);
                    self.fighters[target].guts_used = false;
                    // Raised again (virtual +0xf0, `0x1401b7ed0`): its states cleared; it is
                    // back in the running on the timeline at the wait it has.
                    if was_down && !self.fighters[target].down() {
                        self.fighters[target].states.clear();
                        self.fighters[target].restat();
                    }
                }
                // Damage (`DAMAGE_HP`, and `ASSIST_ATTACK`, 6, a Support Attack's, through the
                // same handler `0x1401bdf6d`) and time-scaled damage (8).
                0 | 6 | 8 => {
                    let (user, foe) = (&self.fighters[who].stats, &self.fighters[target].stats);
                    let amount = if e.act == 8 {
                        battle::turn_damage(amount, second, self.fighters[who].actions, 1.0)
                    } else {
                        // Elemental damage skips defence and meets the target's resistance.
                        // Effect percent 2 (act flag 7): 100 plus the user's skill power.
                        let scale = scale * (100 + user[stat::POWER]) as f32 / 100.0;
                        let d = battle::skill_damage(amount, power.unwrap_or(user[stat::ATTACK]), user[stat::LEVEL], foe[stat::DEFENSE], e.element.is_none(), scale);
                        let mut resist = [0; 4];
                        resist.copy_from_slice(&foe[stat::RESIST..stat::RESIST + 4]);
                        battle::reduce(d, e.element, &Defender { resist, ..Defender::default() })
                    };
                    let amount = battle::hit_amount(amount, guarding.filter(|&(g, _)| g == target).map_or(0, |(_, share)| share));
                    // The first hit of an action draws for its sound (`0x1401af770`).
                    if hits == 0 {
                        rng.next_31();
                    }
                    hits += 1;
                    damage += battle::take_hp_damage(&mut self.fighters[target].stats, amount, None, None);
                }
                act => {
                    // Stat buffs and debuffs on the skill's side: its own for target kinds 1 and 3.
                    if let Some((state, minus)) = battle::stat_state(act) {
                        let on = if matches!(skill.target, 1 | 3) { who } else { target };
                        self.fighters[on].add_state(state, if minus { -amount } else { amount }, second);
                    }
                }
            }
        }
        // The Support gauge, per target of a party action (`0x1401c080f`): the actor's stat 16,
        // its critical, +50 for a target the action felled.
        if self.fighters[who].side == 0 && self.fighters[target].side == 1 {
            let felled_now = !was_down && self.fighters[target].down();
            self.support = battle::support_add(self.support, battle::support_gain(self.fighters[who].stats[stat::SUPPORT], count, critical, false, felled_now));
        }
        if heals {
            events.push(Event::Healed { who, skill: skill.name.clone(), target, amount: healed, hp_left: self.fighters[target].hp(), mp_left: self.fighters[who].mp() });
        } else {
            self.after_effect(target, rng);
            events.push(Event::Acted { who, skill: skill.name.clone(), target, damage, hp_left: self.fighters[target].hp(), mp_left: self.fighters[who].mp() });
        }
        }
        // The rate as the action ends (`0x14002aa70` after the effects): a step per effect.
        // Only in a battle that has the act table (the screen's figure: tests and duels do not).
        if self.fighters[who].side == 0 && !self.act_steps.is_empty() {
            let steps: i32 = rolled.iter().map(|(e, ..)| usize::try_from(e.act).ok().and_then(|a| self.act_steps.get(a)).copied().unwrap_or(0)).sum();
            events.push(Event::Rate { who, rate: rate + steps, support: self.support });
        }
        events.append(&mut felled);
        self.end_action(who, skill.wait, rng);
        events
    }

    /// Who an action on `centre` reaches (`0x1401a4430`): the fighters on the centre's side still
    /// standing (and the centre itself, standing or not) within the area `kind` gives. Kind 6 is a
    /// line from `who` towards the centre, 150 either side and 2,000 long (`0x1401bc5c0`,
    /// `0x1401bc700`); any other a circle about the centre on the ground of radius
    /// `RADIUS[kind]` (`0x1408041d8`), each fighter's size (+0x358) added to the reach.
    /// Kind 0 (radius 50) is taken as the centre alone: in the game no two fighters stand that
    /// close (the nudge parts monsters by their sizes; the party's places are hundreds apart),
    /// and fighters built without places all stand at the origin.
    pub fn area(&self, who: usize, centre: usize, kind: i32) -> Vec<usize> {
        if kind <= 0 {
            return vec![centre];
        }
        const RADIUS: [f32; 7] = [50.0, 350.0, 450.0, 650.0, 900.0, 1000.0, 150.0];
        let c = self.fighters[centre].position;
        let side = self.fighters[centre].side;
        let reaches = |k: usize| {
            let p = self.fighters[k].position;
            if kind == 6 {
                let u = self.fighters[who].position;
                let (dx, dz) = (c[0] - u[0], c[2] - u[2]);
                let len = (dx * dx + dz * dz).sqrt();
                if len == 0.0 {
                    return k == centre;
                }
                let (fx, fz) = (dx / len, dz / len);
                let (px, pz) = (p[0] - u[0], p[2] - u[2]);
                let along = px * fx + pz * fz;
                let across = (px * fz - pz * fx).abs();
                let size = self.fighters[k].size;
                (-size..=2000.0 + size).contains(&along) && across <= 150.0 + size
            } else {
                let r = RADIUS.get(kind.max(0) as usize).copied().unwrap_or(50.0);
                ((p[0] - c[0]).powi(2) + (p[2] - c[2]).powi(2)).sqrt() < r + self.fighters[k].size
            }
        };
        let mut out = vec![centre];
        out.extend((0..self.fighters.len()).filter(|&k| k != centre && self.fighters[k].side == side && !self.fighters[k].down() && reaches(k)));
        out
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
    /// Whether `who` may give a support costing `cost` now (`0x1401b7c00`: the gauge holds the
    /// cost; the member is standing: its +0x282 / +0x281 and check 9 are taken as that).
    pub fn may_support(&self, who: usize, cost: i32) -> bool {
        self.support >= cost && self.fighters[who].side == 0 && !self.fighters[who].down()
    }

    /// A Support Attack by `who` on `target` (`0x140192760`): the gauge pays `cost` as it is
    /// chosen, then `skill` acts as a party action (its hit fills the gauge as any does), out of
    /// turn: the supporter's place on the timeline stays as it was. Nothing when it may not.
    pub fn support_act(&mut self, who: usize, skill: &Skill, cost: i32, target: usize, rng: &mut Rng) -> Vec<Event> {
        if !self.may_support(who, cost) || self.fighters[target].down() {
            return Vec::new();
        }
        self.support = (self.support - cost).clamp(0, battle::SUPPORT_MAX);
        let place = self.timeline.entries.iter().position(|&(c, _)| c as usize == who).map(|i| (i, self.timeline.entries[i].1));
        let pending = self.pending;
        let events = self.act_with(who, skill, None, target, None, None, rng);
        if let Some((i, wait)) = place {
            self.timeline.entries[i].1 = wait;
        }
        self.pending = pending;
        events
    }

    /// `who` stands guard for the next monster action on another party member (Support Guard,
    /// `ASSIST_GUARD`): the gauge pays `cost` as it is chosen; false when it may not.
    pub fn support_guard(&mut self, who: usize, share: i32, cost: i32) -> bool {
        if !self.may_support(who, cost) {
            return false;
        }
        self.support = (self.support - cost).clamp(0, battle::SUPPORT_MAX);
        self.guard = Some((who, share));
        true
    }

    fn end_action(&mut self, who: usize, wait: i32, rng: &mut Rng) {
        if self.winner().is_none() {
            let wait = battle::action_wait(wait, self.fighters[who].stats[stat::SPEED], None, None, &mut self.pending);
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
        StatBlock { level: 10, hp, attack: 30, defense: 0, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() }
    }

    #[test]
    fn run_rolls_speed_less_the_fastest_enemy_plus_60() {
        let mut blocks = [block(100), block(100), block(100)];
        blocks[0].speed = 18;
        blocks[1].speed = 19;
        blocks[2].speed = 15;
        let mut b = Battle::new(vec![Fighter::new(&blocks[0], 0, 999), Fighter::new(&blocks[1], 1, 999), Fighter::new(&blocks[2], 1, 999)]);
        let mut rng = Rng::seeded(7);
        // Katla's live case: speed 18 against a top enemy speed of 19 → 59
        match b.run_turn(0, &mut rng) {
            Event::Ran { chance, .. } => assert_eq!(chance, 59),
            e => panic!("{e:?}"),
        }
        // after three tries each one adds 30
        b.escaped = false;
        b.tries = 5;
        match b.run_turn(0, &mut rng) {
            Event::Ran { chance, .. } => assert_eq!(chance, 59 + 60),
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn wait_takes_320_at_the_speed_formula() {
        let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(100), 1, 999)]);
        let who = b.next();
        let mut rng = Rng::seeded(1);
        b.wait_turn(who, &mut rng);
        let entry = b.timeline.entries.iter().find(|e| e.0 as usize == who).unwrap();
        assert_eq!(entry.1, battle::action_wait(battle::WAIT_COMMAND, 20, None, None, &mut -1));
        assert!(b.fighters.iter().all(|f| f.stats[stat::HP] == 100));
    }

    #[test]
    fn a_party_hit_reports_its_rate_with_the_act_steps() {
        // The recorded flank attack: 25 before the hit, the attack's own 5 after, 130 %.
        let mut fighters = vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(100), 1, 999)];
        fighters[0].slot = 3;
        fighters[1].slot = 1;
        let mut b = Battle::new(fighters).with_act_steps(vec![5; 8]);
        let mut rng = Rng::seeded(3);
        let attack = Skill { name: "Attack".into(), category: String::new(), wait: 600, target: 2, spread: 0, effects: vec![SkillEffect { act: 0, element: None, min: 10, max: 10, min2: 0, max2: 0 }], description: String::new() };
        let e = b.act(0, &attack, None, 1, &mut rng);
        let Some(Event::Rate { who: 0, rate, support }) = e.iter().find(|e| matches!(e, Event::Rate { .. })).cloned() else { panic!("{e:?}") };
        // Slot 3 does not face sector 1 (slot 4 does): a flank, 25; a critical adds 10.
        assert!(rate == 30 || rate == 40, "{rate}");
        // The gauge from its start: 25 for a single target, 37 on a critical.
        assert!(support == 125 || support == 137, "{support}");
    }

    #[test]
    fn a_support_attack_pays_its_cost_and_keeps_the_supporters_turn() {
        let fighters = vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(100), 0, 999), Fighter::new(&block(500), 1, 999_999)];
        let mut b = Battle::new(fighters).with_act_steps(vec![5; 8]);
        let mut rng = Rng::seeded(5);
        let attack = Skill { name: "Support Attack".into(), category: String::new(), wait: 600, target: 2, spread: 0, effects: vec![SkillEffect { act: 6, element: None, min: 45, max: 50, min2: 0, max2: 0 }], description: String::new() };
        let wait_before = b.timeline.entries.iter().find(|e| e.0 == 1).unwrap().1;
        assert!(!b.may_support(1, 150), "100 in the gauge");
        assert!(b.support_act(1, &attack, 150, 2, &mut rng).is_empty());
        let e = b.support_act(1, &attack, 100, 2, &mut rng);
        assert!(e.iter().any(|e| matches!(e, Event::Acted { who: 1, skill, .. } if skill == "Support Attack")), "{e:?}");
        // 100 paid, then its own hit's 25 (37 on a critical).
        assert!(b.support == 25 || b.support == 37, "{}", b.support);
        assert_eq!(b.timeline.entries.iter().find(|e| e.0 == 1).unwrap().1, wait_before);
    }

    #[test]
    fn a_support_guard_takes_the_hit_for_a_third_less() {
        let fighters = vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(100), 0, 999), Fighter::new(&block(500), 1, 999_999)];
        let claw = Skill { name: "Claw".into(), category: String::new(), wait: 600, target: 2, spread: 0, effects: vec![SkillEffect { act: 0, element: None, min: 30, max: 30, min2: 0, max2: 0 }], description: String::new() };
        let hit = |guard: bool| {
            let mut b = Battle::new(fighters.clone()).with_act_steps(vec![5; 8]);
            if guard {
                assert!(b.support_guard(1, 33, 50));
                assert_eq!(b.support, 50);
            }
            let e = b.act(2, &claw, None, 0, &mut Rng::seeded(9));
            e.iter().find_map(|e| match *e { Event::Acted { target, damage, .. } => Some((target, damage)), _ => None }).unwrap()
        };
        let (t, plain) = hit(false);
        let (g, guarded) = hit(true);
        assert_eq!((t, g), (0, 1));
        assert_eq!(guarded, (100 - 33) * plain / 100);
    }

    #[test]
    fn move_takes_300_and_stands_at_the_new_sector() {
        let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(100), 1, 999)]);
        let who = b.next();
        let mut rng = Rng::seeded(1);
        assert_eq!(b.move_turn(who, 4, &mut rng), Event::Moved { who, sector: 1 });
        assert_eq!(b.fighters[who].slot, 4);
        assert_eq!(b.fighters[who].position, battle::party_spot(who as i32, 1));
        let entry = b.timeline.entries.iter().find(|e| e.0 as usize == who).unwrap();
        assert_eq!(entry.1, battle::action_wait(battle::MOVE_COMMAND, 20, None, None, &mut -1));
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
    fn a_revived_fighter_gets_turns_again() {
        use crate::data::battle_data::SkillEffect;
        let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(100), 0, 999), Fighter::new(&block(500), 1, 999_999)]);
        b.fighters[1].stats[stat::HP] = 0;
        b.fighters[1].add_state(0x12, 10, 0);
        // While down, 1 never comes up.
        for _ in 0..6 {
            let who = b.next();
            assert_ne!(who, 1);
            let i = b.timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
            b.timeline.entries[i].1 = 600;
        }
        let revive = Skill { name: "Revive".into(), category: String::new(), wait: 600, target: 4, spread: 0, effects: vec![SkillEffect { act: 81, element: None, min: 15, max: 15, min2: 0, max2: 0 }], description: String::new() };
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let events = b.act(0, &revive, None, 1, &mut rng);
        assert_eq!(events, [Event::Healed { who: 0, skill: "Revive".into(), target: 1, amount: 15, hp_left: 15, mp_left: 0 }]);
        assert!(b.fighters[1].states.is_empty());
        let came_up = (0..6).any(|_| {
            let who = b.next();
            let i = b.timeline.entries.iter().position(|&(c, _)| c as usize == who).unwrap();
            b.timeline.entries[i].1 = 600;
            who == 1
        });
        assert!(came_up);
    }

    #[test]
    fn an_item_hits_with_its_power_in_place_of_stat_5() {
        use crate::data::battle_data::SkillEffect;
        let bomb = Skill { name: "Bomb".into(), category: String::new(), wait: 600, target: 2, spread: 0, effects: vec![SkillEffect { act: 0, element: Some(0), min: 100, max: 100, min2: 0, max2: 0 }], description: String::new() };
        let hit = |power: Option<i32>| {
            let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(5000), 1, 999_999)]);
            let mut rng = Rng::from_state([1, 2, 3, 4]);
            match power {
                Some(p) => b.use_item(0, &bomb, p, 1, &mut rng),
                None => b.act(0, &bomb, None, 1, &mut rng),
            };
            5000 - b.fighters[1].hp()
        };
        // Stat 5 is 30 for the user; an item of quality 50 hits with 100 instead, the
        // same formula otherwise (elemental: no defence).
        let level = block(100).level;
        assert_eq!(hit(None), battle::skill_damage(100, 30, level, 0, false, 1.0));
        assert_eq!(hit(Some(battle::item_power(50.0))), battle::skill_damage(100, 100, level, 0, false, 1.0));
        assert!(hit(Some(100)) > hit(None));
    }

    #[test]
    fn an_absolute_death_fells_its_target() {
        use crate::data::battle_data::SkillEffect;
        let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), Fighter::new(&block(500), 1, 999_999)]);
        let doom = Skill { name: "Doom".into(), category: String::new(), wait: 600, target: 2, spread: 0, effects: vec![SkillEffect { act: 195, element: None, min: 0, max: 0, min2: 0, max2: 0 }], description: String::new() };
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let events = b.use_item(0, &doom, 100, 1, &mut rng);
        assert_eq!(events[1], Event::Felled { who: 0, skill: "Doom".into(), target: 1 });
        assert!(b.fighters[1].down());
        assert_eq!(b.winner(), Some(0));
    }

    #[test]
    fn instant_death_misses_an_immune_monster() {
        use crate::data::battle_data::SkillEffect;
        let skill = |act| Skill { name: "Death".into(), category: String::new(), wait: 600, target: 2, spread: 0, effects: vec![SkillEffect { act, element: None, min: 100, max: 100, min2: 0, max2: 0 }], description: String::new() };
        let fight = |act, immune| {
            let mut foe = Fighter::new(&block(500), 1, 999_999);
            foe.death_immune = immune;
            let mut b = Battle::new(vec![Fighter::new(&block(100), 0, 999), foe]);
            b.use_item(0, &skill(act), 100, 1, &mut Rng::from_state([1, 2, 3, 4]));
            b.fighters[1].down()
        };
        // DEATH_PERCENT at 100: certain, unless the target is immune; ABSOLUTE_DEATH lands anyway.
        assert!(fight(176, false) && !fight(176, true) && fight(195, true));
    }

    #[test]
    fn death_chances() {
        let mut t: Stats = [0; stat::COUNT];
        t[stat::LEVEL] = 10;
        t[stat::HP] = 30;
        t[stat::MAX_HP] = 100;
        // 20 + 5 × (14 - 10).
        assert_eq!(battle::death_chance(80, 20, 14, &t), Some(40));
        // HP at 30%: 30 lands, 29 does not.
        assert_eq!((battle::death_chance(176, 30, 1, &t), battle::death_chance(176, 29, 1, &t)), (Some(100), None));
        assert_eq!((battle::death_chance(195, 0, 1, &t), battle::death_chance(0, 100, 1, &t)), (Some(100), None));
        let shield = [battle::TimedState { state: 0x50, count: -1, value: 0.0 }];
        assert!(battle::death_blocked(&shield, false) && !battle::death_blocked(&[], false));
    }

    #[test]
    fn a_boost_raises_flag_3_acts_up_to_their_limit() {
        use crate::data::battle_data::ActInfo;
        let mut flags = [false; 12];
        flags[3] = true;
        let acts = [ActInfo { flags, limit: 50, last: 0 }, ActInfo { flags: [false; 12], limit: 9999, last: 0 }];
        use crate::data::battle_data::{Trait, TraitSlot};
        let slot = |kind: &str, v| TraitSlot { kind: kind.into(), min: v, max: v };
        let t = Trait { name: String::new(), group: String::new(), level: 0, point: 0, kinds: [false; 9], slots: vec![slot("ITEM_SUB_ITM_CHANGE_POWER", 80), slot("ITEM_SUB_CHANGE_ACTUAL_POWER", 7)] };
        let mut kinds = vec!["ITEM_SUB_ITM_CHANGE_POWER"];
        kinds.resize(39, "");
        kinds.push("ITEM_SUB_CHANGE_ACTUAL_POWER");
        let traits = [&t];
        let boost = Boost { traits: &traits, kinds: &kinds, inputs: battle::ItemBonusInputs::default(), acts: &acts };
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        // Act 0: the bonus cut to 50, no flag 4 so no extra.
        assert_eq!(boost.apply(0, 100, 10, &mut rng), (150, 15));
        // Act 1: no flags, untouched.
        assert_eq!(boost.apply(1, 100, 10, &mut rng), (100, 10));
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

    #[test]
    fn areas_reach_by_distance() {
        let block = StatBlock { level: 10, hp: 100, attack: 10, defense: 10, speed: 10, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let mut fighters: Vec<Fighter> = (0..4).map(|k| Fighter::new(&block, (k > 0) as u8, 999)).collect();
        // The user at (0, 0, -800); monsters at the centre, 300 and 500 to the side.
        fighters[0].position = [0.0, 0.0, -800.0];
        fighters[2].position = [300.0, 0.0, 0.0];
        fighters[3].position = [500.0, 0.0, 0.0];
        let b = Battle::new(fighters);
        assert_eq!(b.area(0, 1, 0), [1]);
        assert_eq!(b.area(0, 1, 1), [1, 2]);
        assert_eq!(b.area(0, 1, 5), [1, 2, 3]);
        // A line from the user through the centre catches neither of those beside it.
        assert_eq!(b.area(0, 1, 6), [1]);
    }
}
