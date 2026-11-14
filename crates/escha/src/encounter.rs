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
pub fn skips_battle(start: Start, party_level: i32, group_level: i32, kind: i32, strike_stage: usize, field_check: bool) -> bool {
    start == Start::Struck && party_level - group_level >= 10 && !(1..=5).contains(&kind) && strike_stage == 1 && !field_check
}

/// What a group struck down on the field gives (`0x1400847fe`, the battle start when the
/// encounter's +0x51 is set): its monsters drawn as for a battle ([`crate::battle::encounter_monsters`]),
/// then for each, in order, each of its drop slots in order taken with its chance
/// (`0x1404ca5f0`). Returns `(item, count)`; an item numbered 560 and up names a drop pool,
/// which [`pick_pool`] then draws from.
pub fn struck_down_drops(
    group: &crate::data::battle_data::EncounterGroup,
    monsters: &[crate::data::battle_data::Monster],
    rng: &mut barrel_engine::rng::Rng,
) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for m in crate::battle::encounter_monsters(group, rng) {
        for d in monsters.get(m as usize).map_or(&[][..], |m| &m.drops) {
            if rng.chance(d.chance) {
                out.push((d.item, d.count));
            }
        }
    }
    out
}

/// One draw from a drop pool (`0x140113810`): among the entries `eligible` allows (an item
/// whose `itemData` kind, the row's +0x28, is 2 must pass `0x140113bb0`, not read yet), a roll
/// of the low 31 bits modulo their total weight, then the first entry, in order, whose weight
/// the roll does not exceed, the roll going down by each weight passed.
pub fn pick_pool<'a>(
    pool: &'a [crate::data::battle_data::PoolEntry],
    eligible: impl Fn(i32) -> bool,
    rng: &mut barrel_engine::rng::Rng,
) -> Option<&'a crate::data::battle_data::PoolEntry> {
    let ok: Vec<&crate::data::battle_data::PoolEntry> = pool.iter().filter(|e| eligible(e.item)).collect();
    let total: i32 = ok.iter().map(|e| e.weight).sum();
    if total <= 0 {
        return None;
    }
    let mut roll = rng.next_31() % total;
    for e in &ok {
        if roll <= e.weight {
            return Some(e);
        }
        roll -= e.weight;
    }
    None
}

/// The traits a pool's item comes with (`0x140113e30`): [`crate::gather::random_traits`] over
/// the entry's level range and count (the item made fresh, so none it has to avoid; the game
/// leaves out traits it already has and fills its free slots of three).
pub fn pool_traits(
    entry: &crate::data::battle_data::PoolEntry,
    traits: &[crate::data::battle_data::Trait],
    fits: impl Fn(usize) -> bool,
    rng: &mut barrel_engine::rng::Rng,
) -> Vec<usize> {
    crate::gather::random_traits(traits, entry.trait_levels, entry.trait_count.min(3), fits, rng)
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

/// The field strike by its stage, the player's +0x1e0 (`0x14026e350`: 1 once event 733 has
/// been seen, `0x14035f270 (0x2dd)`, else 0): how far ahead of the player its centre sits
/// (`0x1408edd58`), its radius, the player's +0x1e4, for Escha (`0x1408edd60`) and for Logy
/// (`0x1408edd68`), and the part of the strike's motion, in seconds, when it can hit
/// (`0x14026e2c0`: from 0.35 to 0.6, `0x1408edd70`, `0x1408edd78`).
pub const STRIKE_REACH: [f64; 2] = [120.0, 160.0];
pub const STRIKE_RADIUS_ESCHA: [f64; 2] = [130.0, 190.0];
pub const STRIKE_RADIUS_LOGY: [f64; 2] = [150.0, 210.0];
pub const STRIKE_WINDOW: (f64, f64) = (0.35, 0.6);

/// The strike's stage and radius for the lead (`logy`) once event 733 is seen or not.
pub fn strike(logy: bool, upgraded: bool) -> (usize, f64) {
    let stage = upgraded as usize;
    (stage, if logy { STRIKE_RADIUS_LOGY[stage] } else { STRIKE_RADIUS_ESCHA[stage] })
}

/// Whether the strike can hit `t` seconds into its motion.
pub fn strike_out(t: f64) -> bool {
    (STRIKE_WINDOW.0..=STRIKE_WINDOW.1).contains(&t)
}

/// The strike check (`0x140270f50`): its centre is the player's point 2 (`0x1401f3b80`) moved
/// ahead along the player's facing (radians, 0 = +z) by [`STRIKE_REACH`]`[stage]`; the enemy
/// nearest it within its radius plus the strike's (the player's +0x1e4) is hit.
pub fn struck(point: [f64; 3], facing: f64, stage: usize, strike_radius: f64, enemies: &[([f64; 3], f64)]) -> Option<usize> {
    let reach = STRIKE_REACH.get(stage).copied().unwrap_or(0.0);
    let c = [point[0] + facing.sin() * reach, point[1], point[2] + facing.cos() * reach];
    let mut best: Option<(usize, f64)> = None;
    for (k, (e, r)) in enemies.iter().enumerate() {
        let d = ((c[0] - e[0]).powi(2) + (c[1] - e[1]).powi(2) + (c[2] - e[2]).powi(2)).sqrt();
        if r + strike_radius >= d && best.is_none_or(|(_, b)| d < b) {
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
    fn the_strike_lands_ahead() {
        let enemies = [([0.0, 0.0, 160.0], 60.0), ([0.0, 0.0, -160.0], 60.0)];
        assert_eq!(struck([0.0; 3], 0.0, 1, 30.0, &enemies), Some(0));
        assert_eq!(struck([0.0; 3], std::f64::consts::PI, 1, 30.0, &enemies), Some(1));
        assert_eq!(struck([0.0; 3], std::f64::consts::FRAC_PI_2, 1, 30.0, &enemies), None);
        assert_eq!((strike(false, false), strike(true, true)), ((0, 130.0), (1, 210.0)));
        assert!(strike_out(0.4) && !strike_out(0.2) && !strike_out(0.7));
    }

    #[test]
    fn pool_draws_by_weight() {
        use crate::data::battle_data::PoolEntry;
        let e = |item, weight| PoolEntry { item, weight, quality: (50, 50), trait_levels: (1, 5), trait_count: 1 };
        let pool = [e(364, 15), e(369, 15), e(374, 70)];
        let mut rng = barrel_engine::rng::Rng::from_state([1, 2, 3, 4]);
        let mut n = [0; 3];
        for _ in 0..10_000 {
            let got = pick_pool(&pool, |_| true, &mut rng).unwrap();
            n[pool.iter().position(|p| p == got).unwrap()] += 1;
        }
        assert!((1300..1700).contains(&n[0]) && (6700..7300).contains(&n[2]), "{n:?}");
        assert_eq!(pick_pool(&pool, |i| i == 369, &mut rng).unwrap().item, 369);
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
