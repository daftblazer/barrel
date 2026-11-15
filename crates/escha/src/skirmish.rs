//! Party members against one monster on a [`crate::fight::Battle`]: the monster acts by its
//! rules and target picks, and each party member, standing in for the player, uses the
//! costliest skill it has learnt by its level and can pay for that is aimed at the foe and
//! deals damage, or else its normal attack; when a member is at half HP or less, the
//! costliest affordable heal on the weakest instead, and a reviving one on a fallen member
//! first, or a healing item it carries when no heal skill is affordable. Same gaps as
//! [`crate::fight`]; no attack items, support actions or finishing attacks.

use crate::battle::{self, stat};
use crate::duel::Fighter as Foe;
use crate::fight::{Battle, Event, Fighter};
use barrel_engine::rng::Rng;
use crate::data::battle_data::{PartySkill, Skill, StatBlock};

/// A party member: its stats at its level, its MP, its normal attack and the skills it may use.
pub struct Member<'a> {
    pub block: StatBlock,
    pub mp: i32,
    pub attack: &'a Skill,
    /// (skill, MP cost) for each learnt battle skill (commands 1 to 3).
    pub skills: Vec<(&'a Skill, i32)>,
    /// Items it carries: each as a battle action (`battle::item_action`), its power
    /// (`battle::item_power`) and how many.
    pub items: Vec<Held>,
    /// HP and MP to start with when below their maxima (a save's current values).
    pub now: Option<(i32, i32)>,
    /// Its size in battle (its `Chara.xml` radius: `battle_data::party_size`); 0 when unknown.
    pub size: f32,
}

/// A carried item.
#[derive(Clone, Debug)]
pub struct Held {
    pub action: Skill,
    pub power: i32,
    pub count: u32,
    /// Its traits (`item_potential.xml` rows), whose bonus each use rolls.
    pub traits: Vec<crate::data::battle_data::Trait>,
}

/// What item bonuses need: the act table and the `ITEM_SUB_*` names in enum order (empty: no
/// bonuses).
#[derive(Clone, Copy, Default)]
pub struct ItemRules<'a> {
    pub acts: &'a [crate::data::battle_data::ActInfo],
    pub kinds: &'a [&'a str],
    /// `item_effect.xml` rows, for the effects traits add.
    pub effects: &'a [crate::data::battle_data::ItemEffect],
}

impl Held {
    fn heals(&self, revive: bool) -> bool {
        let e = &self.action.effects;
        matches!(self.action.target, 1 | 3 | 4) && if revive { e.iter().any(|e| e.act == 81) } else { e.iter().any(|e| e.act == 19 || e.act == 21) }
    }

    /// Thrown at the other side: damage (acts 0, 8) or instant death (80, 176, 195).
    fn attacks(&self) -> bool {
        !matches!(self.action.target, 1 | 3 | 4) && self.action.effects.iter().any(|e| matches!(e.act, 0 | 8 | 80 | 176 | 195))
    }
}

impl<'a> Member<'a> {
    /// A member from its skill list at `level`: command 0 is the attack, commands 1 to 3 the
    /// skills learnt at or below the level, at their listed MP cost (no equipment, so no cut).
    pub fn new(block: StatBlock, mp: i32, list: &[PartySkill], table: &'a [Skill]) -> Option<Member<'a>> {
        let level = block.level;
        let skill = |e: &PartySkill| table.get(e.skill as usize);
        let attack = list.iter().find(|e| e.command == 0).and_then(skill)?;
        let skills = list
            .iter()
            .filter(|e| (1..=3).contains(&e.command) && e.level.is_some_and(|l| i32::from(l) <= level))
            .filter_map(|e| Some((skill(e)?, battle::party_mp_cost(e.mp, 0, 0, 0))))
            .collect();
        Some(Member { block, mp, attack, skills, items: Vec::new(), now: None, size: 0.0 })
    }

    /// The costliest affordable heal aimed at its own side (one that revives, when `revive`).
    fn heal_with(&self, mp: i32, revive: bool) -> Option<(&'a Skill, i32)> {
        self.skills
            .iter()
            .filter(|(s, cost)| *cost <= mp && matches!(s.target, 1 | 3 | 4) && s.effects.iter().any(|e| e.act == 19 || e.act == 21))
            .filter(|(s, _)| !revive || s.effects.iter().any(|e| e.act == 81))
            .max_by_key(|(_, cost)| *cost)
            .copied()
    }

    fn choose(&self, mp: i32) -> (&'a Skill, i32) {
        self.skills
            .iter()
            .filter(|(s, cost)| *cost <= mp && s.target == 2 && s.effects.iter().any(|e| e.act == 0))
            .max_by_key(|(_, cost)| *cost)
            .map_or((self.attack, 0), |&(s, c)| (s, c))
    }
}

/// The size a monster gets when its own is not given (`foe_sizes` shorter than `foes`).
const MONSTER_SIZE: f32 = 100.0;

/// Runs the battle until a side wins or `max_events` happened: the party are fighters 0..n,
/// the monsters n and on, in `foes`' order, each monster as big as its entry in `foe_sizes`
/// (`battle_data::monster_size`, its `Chara.xml` radius). Returns the events and the winning
/// side (0 the party). Every member's turn is the stand-in's ([`Skirmish::auto`]).
pub fn skirmish(party: &[Member], foes: &[Foe], foe_sizes: &[f32], rules: ItemRules, rng: &mut Rng, max_events: usize) -> (Vec<Event>, Option<u8>) {
    let (events, winner, _) = fought(party, foes, foe_sizes, rules, rng, max_events);
    (events, winner)
}

/// [`skirmish`], with what is left of each member's items after it ([`Skirmish::items_left`]).
pub fn fought(party: &[Member], foes: &[Foe], foe_sizes: &[f32], rules: ItemRules, rng: &mut Rng, max_events: usize) -> (Vec<Event>, Option<u8>, Vec<Vec<u32>>) {
    let mut s = Skirmish::new(party, foes, foe_sizes, rules, rng);
    let mut events = Vec::new();
    while events.len() < max_events && s.winner().is_none() {
        let (who, start) = s.begin_turn();
        events.extend(start);
        if s.can_act(who) {
            events.extend(s.auto(who, rng));
        }
    }
    let winner = s.winner();
    (events, winner, s.items_left())
}

/// A member's command (a player's choice): the normal attack or one of the skills it may use
/// (`Member::skills`' index), on a fighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Attack { target: usize },
    Skill { skill: usize, target: usize },
    /// The Wait command: no action, its wait only.
    Wait,
    /// The Run command: a try to leave the battle.
    Run,
    /// The Move command: to another sector round the battle.
    Move { sector: i32 },
    /// The Item command: carried item `item` (its index in the member's items) on `target`.
    Item { item: usize, target: usize },
}

/// Member `who` uses `held` on `target` (`BtlCharaActionItem`): the item's action at its power,
/// with its traits' bonus rolled for this use when there are traits and the rules to read them
/// (a party target: no enemy-size terms; one target).
fn use_held(b: &mut Battle, rules: ItemRules, who: usize, held: &Held, target: usize, rng: &mut Rng) -> Vec<Event> {
    if held.traits.is_empty() || rules.kinds.is_empty() {
        return b.use_item(who, &held.action, held.power, target, rng);
    }
    let traits: Vec<&crate::data::battle_data::Trait> = held.traits.iter().collect();
    let mut action = held.action.clone();
    action.effects.extend(battle::trait_effects(&traits, rules.effects, rng));
    let boost = crate::fight::Boost { traits: &traits, kinds: rules.kinds, inputs: battle::ItemBonusInputs::default(), acts: rules.acts };
    b.use_item_boosted(who, &action, held.power, boost, target, rng)
}

/// A skirmish fought a turn at a time: whoever's turn comes ([`Skirmish::begin_turn`]) acts
/// by a command ([`Skirmish::command`], a member's) or by the stand-in and the monsters' rules
/// ([`Skirmish::auto`]).
pub struct Skirmish<'p, 'a> {
    party: &'p [Member<'a>],
    foes: &'p [Foe<'a>],
    rules: ItemRules<'p>,
    pub battle: Battle,
    /// What is left of each member's items.
    left: Vec<Vec<u32>>,
    /// Each monster's reactions still to come (its block's list; `Reaction`).
    reactions: Vec<Vec<crate::data::battle_data::Reaction>>,
    /// The side last felled by the other side (`0x141077e34`, -1 none; set to -1 as the battle
    /// starts, `0x1401b4728`).
    felled: i32,
}

impl<'p, 'a> Skirmish<'p, 'a> {
    /// The battle set up: the formation (`battle::party_spot`, `battle::scatter`): each member at
    /// its sector's place (its index mod 3), the monsters scattered round the centre, the
    /// biggest there.
    pub fn new(party: &'p [Member<'a>], foes: &'p [Foe<'a>], foe_sizes: &[f32], rules: ItemRules<'p>, rng: &mut Rng) -> Self {
        let mut fighters: Vec<Fighter> = party
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let mut f = Fighter::new(&m.block, 0, 999).with_mp(m.mp);
                f.position = crate::battle::party_spot(k as i32, k as i32 % 3);
                f.slot = k as i32 % 3 + 3;
                f.size = m.size;
                if let Some((hp, mp)) = m.now {
                    f.stats[stat::HP] = hp.clamp(0, f.stats[stat::MAX_HP]);
                    f.stats[stat::MP] = mp.clamp(0, f.stats[stat::MAX_MP]);
                }
                f
            })
            .collect();
        let sizes: Vec<f32> = (0..foes.len()).map(|k| foe_sizes.get(k).copied().unwrap_or(MONSTER_SIZE)).collect();
        let spots = crate::battle::scatter(&sizes, rng);
        for (k, (foe, (spot, sector))) in foes.iter().zip(spots).enumerate() {
            let mut f = Fighter::new(&foe.block, 1, 999_999);
            f.slot = sector;
            f.death_immune = foe.death_immune;
            f.position = spot;
            f.size = sizes[k];
            fighters.push(f);
        }
        Skirmish {
            party,
            foes,
            rules,
            battle: Battle::new(fighters),
            left: party.iter().map(|m| m.items.iter().map(|h| h.count).collect()).collect(),
            reactions: foes.iter().map(|f| f.block.reactions.clone()).collect(),
            felled: -1,
        }
    }

    pub fn winner(&self) -> Option<u8> {
        self.battle.winner()
    }

    /// Whose turn comes next, and what its start did (poison).
    pub fn begin_turn(&mut self) -> (usize, Vec<Event>) {
        let who = self.battle.next();
        let events = self.battle.start_turn(who);
        (who, events)
    }

    /// Whether `who` acts this turn (the battle not won, `who` standing).
    pub fn can_act(&self, who: usize) -> bool {
        self.battle.winner().is_none() && !self.battle.fighters[who].down()
    }

    /// Whether `who` is a party member (whose turn waits for a command).
    pub fn is_member(&self, who: usize) -> bool {
        who < self.party.len()
    }

    /// The skills member `who` may use now: each with its MP cost and whether it can pay.
    pub fn skills(&self, who: usize) -> Vec<(&'a Skill, i32, bool)> {
        let mp = self.battle.fighters.get(who).map_or(0, |f| f.mp());
        self.party.get(who).map_or(Vec::new(), |m| m.skills.iter().map(|&(s, c)| (s, c, c <= mp)).collect())
    }

    /// The timeline's entries in turn order, each a fighter standing or `None` (a free entry or
    /// a fallen fighter's): the turn bar's slots.
    pub fn slots(&self) -> Vec<Option<usize>> {
        self.battle.timeline.slots().into_iter().map(|c| c.map(|c| c as usize).filter(|&k| self.battle.fighters.get(k).is_some_and(|f| !f.down()))).collect()
    }

    /// Where `who`'s next turn would land after each of its commands (the attack, then its
    /// skills, then Wait): how many entries wait less than the first entry's wait plus the command's
    /// wait (`battle::action_wait` as the action would take it, states left out), as the turn
    /// bar's next-turn mark counts them (`0x1401900c0`).
    pub fn next_places(&self, who: usize) -> Vec<usize> {
        let Some(member) = self.party.get(who) else { return Vec::new() };
        std::iter::once(member.attack.wait).chain(member.skills.iter().map(|&(s, _)| s.wait)).chain([battle::WAIT_COMMAND]).map(|w| self.next_place(who, w)).collect()
    }

    fn next_place(&self, who: usize, wait: i32) -> usize {
        let speed = self.battle.fighters.get(who).map_or(0, |f| f.stats[stat::SPEED]);
        let wait = battle::action_wait(wait, speed, None, None, &mut self.battle.pending());
        let entries = &self.battle.timeline.entries;
        let first = entries.first().map_or(0, |e| e.1);
        entries.iter().filter(|e| e.1 < first + wait).count()
    }

    /// The fighters standing, in turn order (the timeline's: the next to act first).
    pub fn order(&self) -> Vec<usize> {
        self.battle.timeline.entries.iter().map(|&(c, _)| c as usize).filter(|&k| self.battle.fighters.get(k).is_some_and(|f| !f.down())).collect()
    }

    /// The uses left of each member's items, in their carried order.
    pub fn items_left(&self) -> Vec<Vec<u32>> {
        self.left.clone()
    }

    /// Member `who`'s items: (name, how many are left, whether it is thrown at the other side),
    /// in its carried order.
    pub fn items(&self, who: usize) -> Vec<(String, u32, bool)> {
        let Some(m) = self.party.get(who) else { return Vec::new() };
        m.items.iter().zip(&self.left[who]).map(|(h, &n)| (h.action.name.clone(), n, h.action.target == 2)).collect()
    }

    /// The monsters still standing.
    pub fn foes_standing(&self) -> Vec<usize> {
        (self.party.len()..self.battle.fighters.len()).filter(|&k| !self.battle.fighters[k].down()).collect()
    }

    /// Member `who` acts by `command` (a skill it cannot pay for, or a target that is not a
    /// standing monster, falls back to the normal attack on the first one standing).
    pub fn command(&mut self, who: usize, command: Command, rng: &mut Rng) -> Vec<Event> {
        let before_target = self.foes_standing();
        let fallback = before_target.first().copied().unwrap_or(self.party.len());
        let valid = |t: usize| if before_target.contains(&t) { t } else { fallback };
        let member = &self.party[who];
        let mut events = Vec::new();
        match command {
            Command::Skill { skill, target } if self.skills(who).get(skill).is_some_and(|s| s.2) => {
                let (s, cost) = member.skills[skill];
                self.battle.pay_mp(who, cost);
                events.extend(self.battle.act(who, s, None, valid(target), rng));
            }
            Command::Skill { target, .. } | Command::Attack { target } => events.extend(self.battle.act(who, member.attack, None, valid(target), rng)),
            Command::Wait => self.battle.wait_turn(who, rng),
            Command::Run => events.push(self.battle.run_turn(who, rng)),
            Command::Move { sector } => events.push(self.battle.move_turn(who, sector, rng)),
            Command::Item { item, target } if self.left[who].get(item).is_some_and(|&n| n > 0) => {
                let held = &member.items[item];
                let candidates = self.battle.candidates(who, &held.action);
                let target = if candidates.contains(&target) { target } else { candidates.first().copied().unwrap_or(fallback) };
                self.left[who][item] -= 1;
                events.extend(use_held(&mut self.battle, self.rules, who, held, target, rng));
            }
            // none of it left: the normal attack
            Command::Item { target, .. } => events.extend(self.battle.act(who, member.attack, None, valid(target), rng)),
        }
        let mut all = events;
        let from = 0;
        react(&mut self.battle, self.foes, self.party.len(), &mut self.reactions, &mut self.felled, from, &mut all, rng);
        all
    }

    /// `who` acts by the stand-in (a member) or its rules (a monster).
    pub fn auto(&mut self, who: usize, rng: &mut Rng) -> Vec<Event> {
        let (party, foes, rules, n) = (self.party, self.foes, self.rules, self.party.len());
        let b = &mut self.battle;
        let left = &mut self.left;
        let mut events = Vec::new();
        // The party's target: the first monster still standing (a player's choice; this stand-in
        // works down the line).
        let foe_target = (n..b.fighters.len()).find(|&k| !b.fighters[k].down()).unwrap_or(n);
        if who < n {
            // The weakest standing member, healed when at half HP or less and a heal is affordable.
            let share = |k: usize| b.fighters[k].hp() as f32 / b.fighters[k].stats[stat::MAX_HP] as f32;
            let weakest = (0..n).filter(|&k| !b.fighters[k].down()).min_by(|&x, &y| share(x).total_cmp(&share(y)));
            let fallen = (0..n).find(|&k| b.fighters[k].down());
            let revive = fallen.and_then(|k| party[who].heal_with(b.fighters[who].mp(), true).map(|h| (h, k)));
            let heal = revive.or_else(|| weakest.filter(|&k| share(k) <= 0.5).and_then(|k| party[who].heal_with(b.fighters[who].mp(), false).map(|h| (h, k))));
            // No heal skill to pay for: a carried item that does it, the first one left.
            let item = if heal.is_some() {
                None
            } else {
                let pick = |revive: bool, on: Option<usize>| {
                    let k = on?;
                    let i = (0..party[who].items.len()).find(|&i| left[who][i] > 0 && party[who].items[i].heals(revive))?;
                    Some((i, k))
                };
                // Else an attack item on the monster, the first one left (a player's choice; this
                // stand-in throws them before skills).
                pick(true, fallen).or_else(|| pick(false, weakest.filter(|&k| share(k) <= 0.5))).or_else(|| {
                    let i = (0..party[who].items.len()).find(|&i| left[who][i] > 0 && party[who].items[i].attacks())?;
                    Some((i, foe_target))
                })
            };
            if let Some((i, target)) = item {
                left[who][i] -= 1;
                events.extend(use_held(b, rules, who, &party[who].items[i], target, rng));
            } else {
                let ((skill, cost), target) = heal.unwrap_or_else(|| (party[who].choose(b.fighters[who].mp()), foe_target));
                b.pay_mp(who, cost);
                events.extend(b.act(who, skill, None, target, rng));
            }
        } else {
            let foe = &foes[who - n];
            let f = &b.fighters[who];
            let situation = crate::monster_ai::Situation { hp: f.hp(), max_hp: f.stats[stat::MAX_HP], turn: f.actions, last_skill: f.last_skill, uses: &f.uses };
            let (slot, skill, mode) = foe.pick(&situation, rng);
            b.fighters[who].last_skill = foe.skill_index(slot);
            let candidates = b.candidates(who, skill);
            let target = b.pick_target(mode.unwrap_or(0), who, &candidates, rng);
            let target = target.unwrap_or(0);
            events.extend(b.act(who, skill, slot, target, rng));
        }
        react(&mut self.battle, foes, n, &mut self.reactions, &mut self.felled, 0, &mut events, rng);
        events
    }
}

/// The reactions after an action (moment 0, `0x1401ac800`): the fighters the action struck (its
/// events from `from` on), and for each standing monster its reactions from the last back, the
/// first whose condition holds firing:
/// - type 9 (condition 8, `0x1401ab83f`): a struck, standing fighter of its character (`chara`,
///   the monster's own row) is at or below `percent` of its HP;
/// - type 14 (condition 10, `0x1401abb2a`): the side of the last fighter felled by the other side
///   (`0x1401c0707`, kept in `felled`, 0 the party, 1 the monsters) is `chara`; it is then
///   cleared, so one reaction takes it.
///
/// The monster then uses entry `skill` of its skill list, the reaction loses a use and goes at
/// exactly 0 (-1, the Ancient Beast King's, never does), and the monster's other reactions wait.
/// The game queues that skill on the monster's command list; here it is used at once, before the
/// next turn (when the game runs the queued command is not read).
fn react(b: &mut Battle, foes: &[Foe], n: usize, reactions: &mut [Vec<crate::data::battle_data::Reaction>], felled: &mut i32, from: usize, events: &mut Vec<Event>, rng: &mut Rng) {
    let mut struck = Vec::new();
    for e in &events[from..] {
        let (who, target, down) = match *e {
            Event::Acted { who, target, hp_left, .. } => (who, target, hp_left <= 0),
            Event::Healed { who, target, .. } => (who, target, false),
            Event::Felled { who, target, .. } => (who, target, true),
            _ => continue,
        };
        struck.push(target);
        let side = |k: usize| i32::from(k >= n);
        if down && side(who) != side(target) {
            *felled = side(target);
        }
    }
    if struck.is_empty() {
        return;
    }
    for (k, list) in reactions.iter_mut().enumerate() {
        let who = n + k;
        if b.winner().is_some() || b.fighters[who].down() {
            continue;
        }
        let holds = |r: &crate::data::battle_data::Reaction, felled: i32| match r.kind {
            9 => struck.iter().any(|&t| {
                let f = &b.fighters[t];
                t >= n && foes[t - n].chara == r.chara && !f.down() && f.hp() as i64 * 100 / f.stats[stat::MAX_HP].max(1) as i64 <= r.percent as i64
            }),
            14 => felled == r.chara,
            _ => false,
        };
        let Some(r) = (0..list.len()).rev().find(|&r| holds(&list[r], *felled)) else { continue };
        let reaction = list[r];
        if reaction.kind == 14 {
            *felled = -1;
        }
        list[r].uses -= 1;
        if list[r].uses == 0 {
            list.remove(r);
        }
        let Some(skill) = foes[k].listed_skill(reaction.skill) else { continue };
        let candidates = b.candidates(who, skill);
        let target = b.pick_target(0, who, &candidates, rng).unwrap_or(0);
        events.extend(b.act(who, skill, None, target, rng));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::battle_data::SkillEffect;

    fn skill(name: &str, wait: i32, lo: i32, hi: i32) -> Skill {
        Skill { name: name.into(), category: String::new(), wait, target: 2, spread: 0, effects: vec![SkillEffect { act: 0, element: None, min: lo, max: hi, min2: 0, max2: 0 }], description: String::new() }
    }

    #[test]
    fn a_member_acts_by_its_command() {
        let table = [skill("Attack", 600, 1, 1), skill("Shot", 550, 5, 5)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }, PartySkill { skill: 1, order: 1, command: 1, level: Some(1), mp: 10 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let m = Member::new(block.clone(), 10, &list, &table).unwrap();
        let claw = skill("Claw", 600, 1, 1);
        let foe = Foe { block: StatBlock { hp: 5000, speed: 1, ..block }, normal: &claw, skills: None, death_immune: false, chara: -1 };
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let party = [m];
        let foes = [foe];
        let mut s = Skirmish::new(&party, &foes, &[], ItemRules::default(), &mut rng);
        let who = loop {
            let (who, _) = s.begin_turn();
            if s.is_member(who) && s.can_act(who) {
                break who;
            }
            s.auto(who, &mut rng);
        };
        assert_eq!(s.skills(who).iter().map(|k| (k.0.name.as_str(), k.2)).collect::<Vec<_>>(), [("Shot", true)]);
        let e = s.command(who, Command::Skill { skill: 0, target: 1 }, &mut rng);
        assert!(matches!(&e[0], Event::Acted { who: 0, target: 1, skill, mp_left: 0, .. } if skill == "Shot"));
        // Out of MP: the normal attack instead.
        let e = s.command(who, Command::Skill { skill: 0, target: 9 }, &mut rng);
        assert!(matches!(&e[0], Event::Acted { target: 1, skill, .. } if skill == "Attack"));
    }

    #[test]
    fn the_item_command_uses_one_and_attacks_when_none_are_left() {
        let table = [skill("Attack", 600, 1, 1)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let mut m = Member::new(block.clone(), 0, &list, &table).unwrap();
        m.items.push(Held { action: skill("Bomb", 600, 2, 2), power: 100, count: 1, traits: Vec::new() });
        let foe_block = StatBlock { hp: 50_000, speed: 1, ..block };
        let claw = skill("Claw", 600, 1, 1);
        let foe = Foe { block: foe_block, normal: &claw, skills: None, death_immune: false, chara: -1 };
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let party = [m];
        let mut s = Skirmish::new(&party, std::slice::from_ref(&foe), &[], ItemRules::default(), &mut rng);
        assert_eq!(s.items(0), vec![("Bomb".to_string(), 1, true)]);
        let e = s.command(0, Command::Item { item: 0, target: 9 }, &mut rng);
        assert!(e.iter().any(|e| matches!(e, Event::Acted { skill, target: 1, .. } if skill == "Bomb")), "{e:?}");
        assert_eq!(s.items(0), vec![("Bomb".to_string(), 0, true)]);
        let e = s.command(0, Command::Item { item: 0, target: 1 }, &mut rng);
        assert!(e.iter().any(|e| matches!(e, Event::Acted { skill, .. } if skill == "Attack")), "{e:?}");
    }

    #[test]
    fn members_throw_attack_items() {
        let table = [skill("Attack", 600, 1, 1)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let mut m = Member::new(block.clone(), 0, &list, &table).unwrap();
        let mut doom = skill("Doom", 600, 0, 0);
        doom.effects[0].act = 195;
        m.items.push(Held { action: doom, power: 100, count: 1, traits: Vec::new() });
        let foe_block = StatBlock { hp: 5000, speed: 1, ..block };
        let claw = skill("Claw", 600, 1, 1);
        let foe = Foe { block: foe_block, normal: &claw, skills: None, death_immune: false, chara: -1 };
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let (events, winner) = skirmish(&[m], std::slice::from_ref(&foe), &[], ItemRules::default(), &mut rng, 60);
        assert!(events.iter().any(|e| matches!(e, Event::Felled { skill, .. } if skill == "Doom")));
        assert_eq!(winner, Some(0));
    }

    #[test]
    fn members_use_learnt_skills_they_can_pay_for() {
        let table = [skill("Attack", 600, 68, 75), skill("Shot", 550, 135, 150), skill("Spear", 650, 180, 200)];
        let list = [
            PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 },
            PartySkill { skill: 1, order: 1, command: 1, level: Some(7), mp: 15 },
            PartySkill { skill: 2, order: 3, command: 3, level: Some(25), mp: 28 },
        ];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let m = Member::new(block, 20, &list, &table).unwrap();
        assert_eq!(m.skills.len(), 1);
        assert_eq!(m.choose(20).0.name, "Shot");
        assert_eq!(m.choose(14).0.name, "Attack");
    }

    #[test]
    fn a_member_without_heal_skills_uses_a_healing_item_once() {
        let table = [skill("Attack", 600, 1, 1)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let mut m = Member::new(block.clone(), 0, &list, &table).unwrap();
        let salve = Skill { name: "Salve".into(), category: String::new(), wait: 600, target: 1, spread: 0, effects: vec![SkillEffect { act: 19, element: None, min: 40, max: 40, min2: 0, max2: 0 }], description: String::new() };
        m.items.push(Held { action: salve, power: 100, count: 1, traits: Vec::new() });
        // A monster that hits hard and never dies quickly.
        let foe_block = StatBlock { hp: 100_000, attack: 40, ..block };
        let claw = skill("Claw", 600, 30, 30);
        let foe = Foe { block: foe_block, normal: &claw, skills: None, death_immune: false, chara: -1 };
        let mut rng = Rng::from_state([5, 6, 7, 8]);
        let (events, _) = skirmish(&[m], std::slice::from_ref(&foe), &[], ItemRules::default(), &mut rng, 60);
        let used = events.iter().filter(|e| matches!(e, Event::Healed { skill, .. } if skill == "Salve")).count();
        assert_eq!(used, 1);
    }

    #[test]
    fn a_boss_answers_falling_to_half_once() {
        let table = [skill("Attack", 600, 10, 10), skill("Roar", 600, 1, 1)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 1000, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let m = Member::new(block.clone(), 0, &list, &table).unwrap();
        // A boss of 200 HP whose one reaction answers 50% or less with its second listed skill.
        let reaction = crate::data::battle_data::Reaction { skill: 1, kind: 9, chara: 48, percent: 50, uses: 1 };
        let foe_block = StatBlock { hp: 200, speed: 1, skills: vec![(0, String::new()), (1, String::new())], reactions: vec![reaction], ..block };
        let foe = Foe { block: foe_block, normal: &table[0], skills: Some(&table), death_immune: false, chara: 48 };
        let mut rng = Rng::from_state([5, 6, 7, 8]);
        let (events, _) = skirmish(&[m], std::slice::from_ref(&foe), &[], ItemRules::default(), &mut rng, 200);
        let roars: Vec<usize> = events.iter().enumerate().filter(|(_, e)| matches!(e, Event::Acted { skill, who: 1, .. } if skill == "Roar")).map(|(k, _)| k).collect();
        assert_eq!(roars.len(), 1, "{events:?}");
        // Right after the hit that took it to half or less.
        let Event::Acted { target: 1, hp_left, .. } = events[roars[0] - 1] else { panic!("{:?}", events[roars[0] - 1]) };
        assert!(hp_left * 2 <= 200 && hp_left > 0);
    }

    #[test]
    fn a_boss_answers_each_fallen_member() {
        let table = [skill("Attack", 600, 1, 1), skill("Crush", 600, 500, 500), skill("Gloat", 600, 0, 0), skill("Shout", 600, 0, 0)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9], reactions: Vec::new() };
        let party: Vec<Member> = (0..2).map(|_| Member::new(block.clone(), 0, &list, &table).unwrap()).collect();
        // The Ancient Beast King's kind: on any party member felled, entry 3 of its list, without end.
        let reaction = crate::data::battle_data::Reaction { skill: 3, kind: 14, chara: 0, percent: -1, uses: -1 };
        let foe_block = StatBlock { hp: 100_000, speed: 30, skills: (0..4).map(|k| (k, String::new())).collect(), reactions: vec![reaction], ..block };
        let foe = Foe { block: foe_block, normal: &table[1], skills: Some(&table), death_immune: false, chara: 148 };
        let mut rng = Rng::from_state([5, 6, 7, 8]);
        let (events, winner) = skirmish(&party, std::slice::from_ref(&foe), &[], ItemRules::default(), &mut rng, 200);
        let shouts = events.iter().filter(|e| matches!(e, Event::Acted { skill, .. } if skill == "Shout")).count();
        let fallen = events.iter().filter(|e| matches!(e, Event::Acted { target, hp_left, .. } if *target < 2 && *hp_left <= 0)).count();
        // Every fall but the one that ends the battle is answered.
        assert!(fallen >= 2);
        assert_eq!(shouts, fallen - usize::from(winner == Some(1)), "{events:?}");
    }
}
