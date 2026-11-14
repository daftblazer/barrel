//! Party levels from experience (English Escha executable): the experience table
//! (`crate::data::battle_data::exp_table`, `0x14005fb40 (0, level)`), the level limit and the
//! battle's experience as the result screen counts it.

/// The level limit (`0x14032dc50`): 50 plus bonus value 2 (`0x14002f2f0 (2)`), and never past 99
/// (the table's end).
pub fn level_limit(limit_bonus: i32) -> i32 {
    (50 + limit_bonus).min(99)
}

/// The experience a battle gives (`0x1401d85b0`): its total (`battle::rewards`) times 100 plus
/// bonus value 1 (the experience rate, `0x14002f2f0 (1)`) over 100, cut toward zero.
pub fn battle_exp(total: i32, rate_bonus: i32) -> i32 {
    total * (100 + rate_bonus) / 100
}

/// The experience still needed for the next level (`0x14032dc50`): the table's total for one
/// level past `level`, held at the limit, less `exp`, at least 0; `None` past the table.
pub fn to_next(table: &[(i32, i32)], level: i32, exp: i32, limit_bonus: i32) -> Option<i32> {
    let next = (level + 1).min(50 + limit_bonus);
    let need = table.get(usize::try_from(next - 1).ok()?)?.0;
    Some((need - exp).max(0))
}

/// The level `exp` reaches (`0x14005fc60`): the count of table totals it has, within the limit.
pub fn level_for(table: &[(i32, i32)], exp: i32, limit_bonus: i32) -> i32 {
    let limit = level_limit(limit_bonus);
    let mut level = 1;
    for (k, &(need, _)) in table.iter().enumerate() {
        if (k as i32 + 1) <= limit && exp >= need {
            level = k as i32 + 1;
        }
    }
    level
}

/// The most experience a member keeps (`0x14032dc20` and `0x14032dbf0` hold it there, and at 0).
pub const EXP_MAX: i32 = 999_999;

/// A party member's growth: level (record +4), experience (+8) and the levels gained in all
/// (+0x2a0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Growth {
    pub level: i32,
    pub exp: i32,
    pub gained: i32,
}

impl Growth {
    /// A battle's experience reaching this member (`0x1401d86f0` on): it is added whole (each
    /// fighter and the party's seventh, the list `0x141093530` slot 6, get all of it), held
    /// within 0 and [`EXP_MAX`]; then, unless `held` (the fighter's flag +0x286), the member
    /// goes up to the level its experience reaches (`0x14032d8c0`, `0x14032d790`) and the
    /// levels gained are counted.
    pub fn gain(&mut self, table: &[(i32, i32)], amount: i32, limit_bonus: i32, held: bool) {
        self.exp = (self.exp + amount).clamp(0, EXP_MAX);
        let level = level_for(table, self.exp, limit_bonus);
        if !held && level > self.level {
            self.gained += level - self.level;
            self.level = level;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_from_experience() {
        let table: Vec<(i32, i32)> = [0, 10, 20, 40, 70, 110].iter().map(|&n| (n, 0)).chain((6..99).map(|k| (110 + 100 * (k - 5), 0))).collect();
        assert_eq!(level_for(&table, 0, 0), 1);
        assert_eq!(level_for(&table, 45, 0), 4);
        assert_eq!(to_next(&table, 4, 45, 0), Some(25));
        assert_eq!(battle_exp(19, 10), 20);
        // At the limit the next level is the limit itself: nothing more to gain.
        assert_eq!(to_next(&table, 50, 1_000_000, 0), Some(0));
        assert_eq!(level_for(&table, 1_000_000, 0), 50);
        assert_eq!(level_limit(60), 99);
        let mut g = Growth { level: 3, exp: 25, gained: 0 };
        g.gain(&table, 50, 0, false);
        assert_eq!(g, Growth { level: 5, exp: 75, gained: 2 });
        g.gain(&table, 2_000_000, 0, true);
        assert_eq!((g.level, g.exp), (5, EXP_MAX));
    }
}

/// The alchemy level an alchemy experience reaches (`0x14005fd20`): how many of the experience
/// table's second-column totals, in its first 50 rows, it has; as rows 1 to 3 need 0, never
/// below 3, and 50 at most. A new game starts at level 3 with that row's 4 (`0x14036a660`; the
/// level is kept at `0x14107c4c8`, the experience at `0x14109355c`).
pub fn alchemy_level(table: &[(i32, i32)], exp: i32) -> i32 {
    table.iter().take(50).take_while(|&&(_, need)| exp >= need).count() as i32
}

/// The alchemy experience still needed for the next level (`0x14036a700`): the second column at
/// one level past `level`, less `exp`; `None` from level 50 (the column ends there).
pub fn alchemy_to_next(table: &[(i32, i32)], level: i32, exp: i32) -> Option<i32> {
    if level + 1 > 50 {
        return None;
    }
    table.get(usize::try_from(level).ok()?).map(|&(_, need)| need - exp)
}

/// Which of a party member's three texts its friendship value (record +0x2a4, 0 to 100, raised
/// each day by its place in the party list) shows (`0x14032cf00`): below 30 the first, below 60
/// the second, else the third (from a table of 11 records of 0x30 bytes at `0x140d94a20`,
/// filled at run time: the member, then the three texts).
pub fn friendship_tier(friendship: f32) -> usize {
    if friendship < 30.0 {
        0
    } else if friendship < 60.0 {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod alchemy_tests {
    #[test]
    fn alchemy_levels() {
        let table: Vec<(i32, i32)> = [0, 0, 0, 4, 9, 15].iter().map(|&n| (0, n)).chain((6..99).map(|k| (0, if k < 50 { 15 + 10 * (k - 5) } else { 0 }))).collect();
        assert_eq!(super::alchemy_level(&table, 0), 3);
        assert_eq!(super::alchemy_level(&table, 4), 4);
        assert_eq!(super::alchemy_level(&table, 14), 5);
        assert_eq!(super::alchemy_to_next(&table, 4, 4), Some(5));
        assert_eq!(super::alchemy_to_next(&table, 50, 0), None);
    }
}

#[cfg(test)]
mod friendship_tests {
    #[test]
    fn tiers() {
        assert_eq!([0.0, 29.9, 30.0, 59.9, 60.0, 100.0].map(super::friendship_tier), [0, 0, 1, 1, 2, 2]);
    }
}
