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
use crate::rng::Rng;
use barrel_formats::battle_data::{PartySkill, Skill, StatBlock};

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
}

/// A carried item.
#[derive(Clone, Debug)]
pub struct Held {
    pub action: Skill,
    pub power: i32,
    pub count: u32,
    /// Its traits (`item_potential.xml` rows), whose bonus each use rolls.
    pub traits: Vec<barrel_formats::battle_data::Trait>,
}

/// What item bonuses need: the act table and the `ITEM_SUB_*` names in enum order (empty: no
/// bonuses).
#[derive(Clone, Copy, Default)]
pub struct ItemRules<'a> {
    pub acts: &'a [barrel_formats::battle_data::ActInfo],
    pub kinds: &'a [&'a str],
    /// `item_effect.xml` rows, for the effects traits add.
    pub effects: &'a [barrel_formats::battle_data::ItemEffect],
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
        Some(Member { block, mp, attack, skills, items: Vec::new(), now: None })
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
/// side (0 the party).
pub fn skirmish(party: &[Member], foes: &[Foe], foe_sizes: &[f32], rules: ItemRules, rng: &mut Rng, max_events: usize) -> (Vec<Event>, Option<u8>) {
    let n = party.len();
    // The formation (`battle::party_spot`, `battle::scatter`): each member at its sector's
    // place (its index mod 3), the monsters scattered round the centre, the biggest there.
    let mut fighters: Vec<Fighter> = party
        .iter()
        .enumerate()
        .map(|(k, m)| {
            let mut f = Fighter::new(&m.block, 0, 999).with_mp(m.mp);
            f.position = crate::battle::party_spot(k as i32, k as i32 % 3);
            if let Some((hp, mp)) = m.now {
                f.stats[stat::HP] = hp.clamp(0, f.stats[stat::MAX_HP]);
                f.stats[stat::MP] = mp.clamp(0, f.stats[stat::MAX_MP]);
            }
            f
        })
        .collect();
    let sizes: Vec<f32> = (0..foes.len()).map(|k| foe_sizes.get(k).copied().unwrap_or(MONSTER_SIZE)).collect();
    let spots = crate::battle::scatter(&sizes, rng);
    for (k, (foe, (spot, _))) in foes.iter().zip(spots).enumerate() {
        let mut f = Fighter::new(&foe.block, 1, 999_999);
        f.position = spot;
        f.size = sizes[k];
        fighters.push(f);
    }
    let mut b = Battle::new(fighters);
    let mut events = Vec::new();
    // What is left of each member's items.
    let mut left: Vec<Vec<u32>> = party.iter().map(|m| m.items.iter().map(|h| h.count).collect()).collect();
    while events.len() < max_events && b.winner().is_none() {
        let who = b.next();
        events.extend(b.start_turn(who));
        if b.winner().is_some() || b.fighters[who].down() {
            continue;
        }
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
                let held = &party[who].items[i];
                if held.traits.is_empty() || rules.kinds.is_empty() {
                    events.extend(b.use_item(who, &held.action, held.power, target, rng));
                } else {
                    // The bonus rolled for this use (a party target: no enemy-size terms; one target).
                    let traits: Vec<&barrel_formats::battle_data::Trait> = held.traits.iter().collect();
                    let mut action = held.action.clone();
                    action.effects.extend(battle::trait_effects(&traits, rules.effects, rng));
                    let boost = crate::fight::Boost { traits: &traits, kinds: rules.kinds, inputs: battle::ItemBonusInputs::default(), acts: rules.acts };
                    events.extend(b.use_item_boosted(who, &action, held.power, boost, target, rng));
                }
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
    }
    let winner = b.winner();
    (events, winner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::battle_data::SkillEffect;

    fn skill(name: &str, wait: i32, lo: i32, hi: i32) -> Skill {
        Skill { name: name.into(), category: String::new(), wait, target: 2, spread: 0, effects: vec![SkillEffect { act: 0, element: None, min: lo, max: hi, min2: 0, max2: 0 }], description: String::new() }
    }

    #[test]
    fn members_throw_attack_items() {
        let table = [skill("Attack", 600, 1, 1)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] };
        let mut m = Member::new(block.clone(), 0, &list, &table).unwrap();
        let mut doom = skill("Doom", 600, 0, 0);
        doom.effects[0].act = 195;
        m.items.push(Held { action: doom, power: 100, count: 1, traits: Vec::new() });
        let foe_block = StatBlock { hp: 5000, speed: 1, ..block };
        let claw = skill("Claw", 600, 1, 1);
        let foe = Foe { block: foe_block, normal: &claw, skills: None };
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
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] };
        let m = Member::new(block, 20, &list, &table).unwrap();
        assert_eq!(m.skills.len(), 1);
        assert_eq!(m.choose(20).0.name, "Shot");
        assert_eq!(m.choose(14).0.name, "Attack");
    }

    #[test]
    fn a_member_without_heal_skills_uses_a_healing_item_once() {
        let table = [skill("Attack", 600, 1, 1)];
        let list = [PartySkill { skill: 0, order: -1, command: 0, level: Some(0), mp: 0 }];
        let block = StatBlock { level: 10, hp: 100, attack: 40, defense: 10, speed: 20, resist: [0; 4], stat12: 0, skills: Vec::new(), rules: Vec::new(), slots: [-1; 9] };
        let mut m = Member::new(block.clone(), 0, &list, &table).unwrap();
        let salve = Skill { name: "Salve".into(), category: String::new(), wait: 600, target: 1, spread: 0, effects: vec![SkillEffect { act: 19, element: None, min: 40, max: 40, min2: 0, max2: 0 }], description: String::new() };
        m.items.push(Held { action: salve, power: 100, count: 1, traits: Vec::new() });
        // A monster that hits hard and never dies quickly.
        let foe_block = StatBlock { hp: 100_000, attack: 40, ..block };
        let claw = skill("Claw", 600, 30, 30);
        let foe = Foe { block: foe_block, normal: &claw, skills: None };
        let mut rng = Rng::from_state([5, 6, 7, 8]);
        let (events, _) = skirmish(&[m], std::slice::from_ref(&foe), &[], ItemRules::default(), &mut rng, 60);
        let used = events.iter().filter(|e| matches!(e, Event::Healed { skill, .. } if skill == "Salve")).count();
        assert_eq!(used, 1);
    }
}
