//! How a monster picks its action (`0x1401a5290` in the English executable; the rules are
//! its stat block's, `docs/tables.md`, "Monster actions").
//!
//! The rules are tried in order; the first whose condition holds, whose slot has not reached
//! its use limit, and whose chance roll succeeds gives the action: its slot (then a skill
//! through the block's slot list) and its target. If none does, the game logs an error and
//! takes slot 0 at target 0.

use barrel_formats::battle_data::Rule;

use crate::rng::Rng;

/// What the rules can ask about the monster and the battle.
pub struct Situation<'a> {
    /// Its HP and maximum HP (battle stats 1 and 3).
    pub hp: i32,
    pub max_hp: i32,
    /// The battle's turn count (the owner's +0x2c8) and phase (+0x2d0).
    pub turn: i32,
    pub phase: i32,
    /// How many times each slot has been used this battle.
    pub uses: &'a [i32],
}

/// Whether a rule's condition holds (`0x1401a5360`).
pub fn holds(rule: &Rule, s: &Situation) -> bool {
    match rule.condition {
        0 => true,
        // Every `value`-th turn: the turn count's remainder is value - 1.
        1 => rule.value > 0 && s.turn % rule.value == rule.value - 1,
        // HP at or below `value` percent of the maximum (the percentage cut to an integer).
        2 => s.max_hp * rule.value / 100 >= s.hp,
        // The battle's phase is `value`.
        3 => s.phase == rule.value,
        _ => false,
    }
}

/// The action: (slot, target). Each rule that passes its condition and limit draws one
/// `chance` roll, in order, from `rng`.
pub fn choose(rules: &[Rule], s: &Situation, rng: &mut Rng) -> (i32, i32) {
    for rule in rules {
        if !holds(rule, s) {
            continue;
        }
        if rule.limit != -1 {
            let used = usize::try_from(rule.slot).ok().and_then(|k| s.uses.get(k)).copied().unwrap_or(0);
            if used >= rule.limit {
                continue;
            }
        }
        if rng.chance(rule.chance) {
            return (rule.slot, rule.target);
        }
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(condition: i32, value: i32, chance: i32, slot: i32, limit: i32) -> Rule {
        Rule { kind: 3, target: 0, condition, value, chance, slot, limit }
    }

    #[test]
    fn conditions() {
        let uses = [0; 9];
        let s = |hp, turn| Situation { hp, max_hp: 200, turn, phase: 2, uses: &uses };
        assert!(holds(&rule(1, 6, 100, 1, -1), &s(200, 5)));
        assert!(!holds(&rule(1, 6, 100, 1, -1), &s(200, 6)));
        assert!(holds(&rule(2, 75, 100, 6, 1), &s(150, 0)));
        assert!(!holds(&rule(2, 75, 100, 6, 1), &s(151, 0)));
        assert!(holds(&rule(3, 2, 100, 0, -1), &s(1, 0)));
    }

    #[test]
    fn the_grand_dragon_opens_with_its_once_only_slot() {
        // The Grand Dragon's rules: slot 5 once (every turn), slot 1 every 6th turn, slot 6
        // once at 75% HP.
        let rules = [rule(1, 1, 100, 5, 1), rule(1, 6, 100, 1, -1), rule(2, 75, 100, 6, 1)];
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let mut uses = [0; 9];
        let first = choose(&rules, &Situation { hp: 35000, max_hp: 35000, turn: 0, phase: 0, uses: &uses }, &mut rng);
        assert_eq!(first, (5, 0));
        uses[5] = 1;
        // Slot 5 is spent; turn 5 is a sixth turn, so slot 1; at turn 1 and full HP nothing
        // fits and the fallback is slot 0.
        assert_eq!(choose(&rules, &Situation { hp: 35000, max_hp: 35000, turn: 5, phase: 0, uses: &uses }, &mut rng), (1, 0));
        assert_eq!(choose(&rules, &Situation { hp: 35000, max_hp: 35000, turn: 1, phase: 0, uses: &uses }, &mut rng), (0, 0));
        assert_eq!(choose(&rules, &Situation { hp: 20000, max_hp: 35000, turn: 1, phase: 0, uses: &uses }, &mut rng), (6, 0));
    }
}
