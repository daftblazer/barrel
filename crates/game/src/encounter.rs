//! Meeting a field enemy (English Escha executable). The field's contact check (`0x140270260`)
//! makes an encounter (`0x140081260`, object built by `0x140083e60`) when the player's field
//! strike hits an enemy (`0x140270f50`, only while the strike is out, `0x14026e2c0`) or, failing
//! that, when they touch (`0x140270cc0`). See docs/tables.md, "The field's battle start".

/// How an encounter began (the encounter's +0x20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// The player's strike hit first (1; not when the enemy's brain refuses it, its +0x130
    /// method +0x68).
    Struck,
    /// They touched, or a script forced it (`forceEncount`, `0x14028c650`) (0).
    Contact,
}

/// The average of the levels given (`0x140330a10` over the party list at `0x141093530`,
/// `0x140084410` over the group's three monster slots), whole-number division; 0 for none.
pub fn average_level(levels: &[i32]) -> i32 {
    if levels.is_empty() { 0 } else { levels.iter().sum::<i32>() / levels.len() as i32 }
}

/// Whether the encounter ends without a battle (`0x140084410` sets the encounter's +0x51 and the
/// battle start, `0x140084710`, then makes no battle request): the player struck first, the
/// party's average level is at least 10 above the group's, the encounter's +0x24 (the contact
/// check's third output; a forced encounter's is 1) is not 1 to 5, the player's +0x1e0 is 1, and
/// a field check (`0x1402177c0` on the field object's +0x58, not read yet) says no.
pub fn skips_battle(start: Start, party_level: i32, group_level: i32, kind: i32, player_state: i32, field_check: bool) -> bool {
    start == Start::Struck && party_level - group_level >= 10 && !(1..=5).contains(&kind) && player_state == 1 && !field_check
}

/// The touch check (`0x140270cc0`): the enemy nearest the player whose distance (3D, from the
/// player's place raised by 100 to the enemy's) is at most the enemy's radius × 1.1 plus the
/// player's × 1.5 (both the field character's +0x90). Enemies are `(place, radius)`; the
/// enemy's height is first moved by its monster row's +0x5c/+0x60/+0x74 (×1.3 for some kinds),
/// left out here. Returns the index of the one touched.
pub fn touched(player: [f64; 3], player_radius: f64, enemies: &[([f64; 3], f64)]) -> Option<usize> {
    let p = [player[0], player[1] + 100.0, player[2]];
    let mut best: Option<(usize, f64)> = None;
    for (k, (e, r)) in enemies.iter().enumerate() {
        let d = ((p[0] - e[0]).powi(2) + (p[1] - e[1]).powi(2) + (p[2] - e[2]).powi(2)).sqrt();
        if r * 1.1 + player_radius * 1.5 >= d && best.is_none_or(|(_, b)| d < b) {
            best = Some((k, d));
        }
    }
    best.map(|(k, _)| k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_touching_enemy() {
        let enemies = [([0.0, 100.0, 120.0], 50.0), ([0.0, 100.0, 90.0], 50.0), ([0.0, 100.0, 400.0], 50.0)];
        assert_eq!(touched([0.0; 3], 40.0, &enemies), Some(1));
        assert_eq!(touched([0.0, 0.0, 500.0], 40.0, &enemies[..2]), None);
    }

    #[test]
    fn strong_parties_strike_weak_enemies_down() {
        let (party, group) = (average_level(&[15, 14, 16]), average_level(&[3, 4]));
        assert_eq!((party, group), (15, 3));
        assert!(skips_battle(Start::Struck, party, group, -1, 1, false));
        assert!(!skips_battle(Start::Contact, party, group, -1, 1, false));
        assert!(!skips_battle(Start::Struck, 12, 3, -1, 1, false));
        assert!(!skips_battle(Start::Struck, party, group, 1, 1, false));
        assert!(!skips_battle(Start::Struck, party, group, -1, 0, false));
    }
}
