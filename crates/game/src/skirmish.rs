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
}

/// A carried item.
#[derive(Clone, Debug)]
pub struct Held {
    pub action: Skill,
    pub power: i32,
    pub count: u32,
}

impl Held {
    fn heals(&self, revive: bool) -> bool {
        let e = &self.action.effects;
        matches!(self.action.target, 1 | 3 | 4) && if revive { e.iter().any(|e| e.act == 81) } else { e.iter().any(|e| e.act == 19 || e.act == 21) }
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
        Some(Member { block, mp, attack, skills, items: Vec::new() })
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

/// Runs the battle until a side wins or `max_events` happened: the party are fighters 0..n,
/// the monster n. Returns the events and the winning side (0 the party).
pub fn skirmish(party: &[Member], foe: &Foe, rng: &mut Rng, max_events: usize) -> (Vec<Event>, Option<u8>) {
    let n = party.len();
    let mut fighters: Vec<Fighter> = party.iter().map(|m| Fighter::new(&m.block, 0, 999).with_mp(m.mp)).collect();
    fighters.push(Fighter::new(&foe.block, 1, 999_999));
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
                pick(true, fallen).or_else(|| pick(false, weakest.filter(|&k| share(k) <= 0.5)))
            };
            if let Some((i, target)) = item {
                left[who][i] -= 1;
                let held = &party[who].items[i];
                events.extend(b.use_item(who, &held.action, held.power, target, rng));
            } else {
                let ((skill, cost), target) = heal.unwrap_or_else(|| (party[who].choose(b.fighters[who].mp()), n));
                b.pay_mp(who, cost);
                events.extend(b.act(who, skill, None, target, rng));
            }
        } else {
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
        m.items.push(Held { action: salve, power: 100, count: 1 });
        // A monster that hits hard and never dies quickly.
        let foe_block = StatBlock { hp: 100_000, attack: 40, ..block };
        let claw = skill("Claw", 600, 30, 30);
        let foe = Foe { block: foe_block, normal: &claw, skills: None };
        let mut rng = Rng::from_state([5, 6, 7, 8]);
        let (events, _) = skirmish(&[m], &foe, &mut rng, 60);
        let used = events.iter().filter(|e| matches!(e, Event::Healed { skill, .. } if skill == "Salve")).count();
        assert_eq!(used, 1);
    }
}
