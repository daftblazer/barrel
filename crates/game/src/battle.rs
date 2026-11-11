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

#[cfg(test)]
mod tests {
    use super::*;

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
