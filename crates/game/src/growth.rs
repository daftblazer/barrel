//! Party levels from experience (English Escha executable): the experience table
//! (`barrel_formats::battle_data::exp_table`, `0x14005fb40 (0, level)`), the level limit and the
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

/// The level `exp` reaches: the highest whose table total it has, within the limit.
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
    }
}
