//! The calendar (English Escha executable): the date is one integer, the global at
//! `0x1410a42cc` (read by `0x14035c050`), counting days from 0 in 30-day months and 360-day
//! years: `v = 360 × (calendar year - 1) + 30 × (month - 1) + (day - 1)`. Dates are shown in
//! school years that start in April (`0x1402f0300`: a month before April belongs to the
//! school year before), and built back by `0x1402f02d0 (school year, month, day)`. So April
//! 1st of school year 1 is 90, and March 30th of school year 1 is 449.
//!
//! The story's assignments run in four-month periods from April 1st of school year 1 for three
//! years (`subject/SubjectCardData.xml`: nine cards, "Year 1 April to July" ... "Year 3
//! December to March"). The requests of `quest/QuestAlc.xml` open and close on 91, 211, ...
//! 1171 and 1530: one more than each period's first day (what their comparison is, is not
//! read).
//!
//! Days pass through `0x14035bf10 (days, reason)`: for each day, the date goes up by one
//! (`0x1402f03a0`), the new day is worked (`0x14035bb50`, see [`daily_rolls`] and
//! [`daily_gain`]) and a message (kind 8, with the reason) is queued; `0x14035b930 (days,
//! reason)` follows.

pub const DAYS_PER_MONTH: u32 = 30;
pub const DAYS_PER_YEAR: u32 = 12 * DAYS_PER_MONTH;
/// April 1st of school year 1, where the first assignment period begins.
pub const FIRST_PERIOD_DAY: u32 = 90;
pub const PERIOD_DAYS: u32 = 4 * DAYS_PER_MONTH;
/// Nine assignment periods, three a school year.
pub const PERIODS: u32 = 9;

/// A date as the game shows it: a school year (from April), a month and a day (all from 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// The date of the game's day number `v` (`0x1402f0300` and the month and day it divides
    /// out).
    pub fn of(v: u32) -> Date {
        let month = v / DAYS_PER_MONTH % 12 + 1;
        let year = v / DAYS_PER_YEAR + u32::from(month >= 4);
        Date { year, month, day: v % DAYS_PER_MONTH + 1 }
    }

    /// Its day number (`0x1402f02d0`: `(year' × 12 + month) × 30 + day - 391`, `year'` being
    /// the school year, plus one for January to March).
    pub fn number(self) -> u32 {
        let year = self.year + u32::from(self.month < 4);
        (year * 12 + self.month) * DAYS_PER_MONTH + self.day - 391
    }
}

/// The assignment period (0 to 8, a `SubjectCardData` row) day `v` falls in; `None` before
/// the first one and after the last.
pub fn period(v: u32) -> Option<u32> {
    let k = v.checked_sub(FIRST_PERIOD_DAY)? / PERIOD_DAYS;
    (k < PERIODS).then_some(k)
}

/// The first and last day of period `k`.
pub fn period_days(k: u32) -> (u32, u32) {
    let first = FIRST_PERIOD_DAY + k * PERIOD_DAYS;
    (first, first + PERIOD_DAYS - 1)
}

/// How many records get a fresh roll from 0 to 100 at the start of each new day (`0x14035bb50`:
/// 0x24-byte records from `0x1410a42e0` to `0x1410ae6b4`, one per `EVENT_*`: the events'
/// records, saved whole, `barrel_formats::save::EventRecord`). Each roll
/// draws a number from the battle's generator, so a day costs this many draws first.
pub const DAILY_ROLLS: usize = 1165;

/// The day's rolls, in record order.
pub fn daily_rolls(rng: &mut crate::rng::Rng) -> Vec<i32> {
    (0..DAILY_ROLLS).map(|_| rng.range(0, 100)).collect()
}

/// A character's daily gain of its +0x2a4 (a 0 to 100 float, friendship presumably) by its
/// place in the 11-slot list at `0x141093530` (`0x1403303f0`): 0.11 in slots 0 to 2 or when not
/// in the list, 0.08 in slots 3 to 5, 0.03 further down. Every character but one (`0x140330120`
/// of `0x141098a48`, the leader presumably) gains each day.
pub fn daily_gain(slot: Option<usize>) -> f32 {
    match slot {
        Some(k) if k >= 6 => 0.03,
        Some(k) if k >= 3 => 0.08,
        _ => 0.11,
    }
}

/// The value after a gain (`0x14032db60`): added in floats, and kept at 100 when it would pass
/// it (only the top is clamped).
pub fn add_gain(value: f32, gain: f32) -> f32 {
    let v = value + gain;
    if v.max(0.0) > 100.0 { 100.0 } else { v }
}

/// Whether day `v` is a month's first (the new day's monthly work, `0x140363860` and the
/// manager at `0x140dfc220`'s `0x140267f50`, runs then).
pub fn month_start(v: u32) -> bool {
    v.is_multiple_of(DAYS_PER_MONTH)
}

/// Whether the request board is redrawn on day `v` (`0x14033aaa0`: three synthesis, four battle
/// and three gathering requests): the 1st, 11th and 21st of
/// each month (`(day of month) mod 10 == 1`).
pub fn board_day(v: u32) -> bool {
    (v % DAYS_PER_MONTH + 1) % 10 == 1
}

/// The two words the request board's generator starts from (`0x14033ab20`): `seed` is the date
/// plus the global at `0x1410989f0`, each word the Mersenne Twister seeding step of the one
/// before (`(x ^ (x >> 30)) × 0x6c078965 + i`, the shift arithmetic, from i = 1).
pub fn board_seed(date: u32, global: i32) -> [i32; 2] {
    let step = |x: i32, i: i32| ((x ^ (x >> 30)).wrapping_mul(0x6c07_8965)).wrapping_add(i);
    let s0 = step((date as i32).wrapping_add(global), 1);
    [s0, step(s0, 2)]
}

/// The game's first day, April 1st of school year 1.
pub fn first_day() -> u32 {
    Date { year: 1, month: 4, day: 1 }.number()
}

/// The month report a day falls in (`0x141093570` + 0x144 × it): the months since the game's
/// first day (0 for the first April, 12 for the next; 60 reports, five years, are kept).
pub fn month_index(v: u32) -> u32 {
    v.saturating_sub(first_day()) / DAYS_PER_MONTH
}

/// The month that has just ended when day `v` begins a month (the monthly step
/// `0x140363860`): the months from the game's first day (`0x1402f02d0 (year 1, April, 1)`) to
/// the first of the new month, less one, so counted from the game's start, not the school
/// year's.
pub fn ended_month(v: u32) -> u32 {
    month_index(v).saturating_sub(1)
}

/// A month's total in its report (`0x1403637e0`, records of 0x144 bytes at `0x141093570`, one
/// per month): the count of syntheses of each of the 265 recipes that month (a byte each, +0 to
/// +0x108) times the price (`cost`, `itemData` +0x24) of the item the recipe makes, summed into
/// +0x134. The month end also stores the rank then (global `0x14109c1e4`, +0x13c) and marks the
/// report as new (+0x140).
pub fn month_total(counts: &[u8], cost_of_recipe: impl Fn(usize) -> i32) -> i32 {
    counts.iter().enumerate().map(|(i, &c)| i32::from(c).wrapping_mul(cost_of_recipe(i))).fold(0, i32::wrapping_add)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_dates() {
        assert_eq!(Date::of(90), Date { year: 1, month: 4, day: 1 });
        assert_eq!(Date::of(360), Date { year: 1, month: 1, day: 1 });
        assert_eq!(Date::of(449), Date { year: 1, month: 3, day: 30 });
        assert_eq!(Date::of(450), Date { year: 2, month: 4, day: 1 });
        assert_eq!(Date { year: 1, month: 4, day: 1 }.number(), 90);
        for v in [90, 91, 330, 359, 360, 1170, 1529] {
            assert_eq!(Date::of(v).number(), v);
        }
    }

    #[test]
    fn a_day_for_the_characters() {
        assert_eq!(daily_gain(Some(1)), 0.11);
        assert_eq!(daily_gain(None), 0.11);
        assert_eq!(daily_gain(Some(4)), 0.08);
        assert_eq!(daily_gain(Some(9)), 0.03);
        assert_eq!(add_gain(99.95, 0.11), 100.0);
        assert_eq!(add_gain(10.0, 0.08), 10.08);
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        let rolls = daily_rolls(&mut rng);
        assert_eq!(rolls.len(), DAILY_ROLLS);
        assert!(rolls.iter().all(|r| (0..=100).contains(r)));
    }

    #[test]
    fn the_days_things_happen() {
        assert!(month_start(90) && !month_start(91));
        let board: Vec<u32> = (90..120).filter(|&v| board_day(v)).map(|v| Date::of(v).day).collect();
        assert_eq!(board, [1, 11, 21]);
        let [a, b] = board_seed(90, 0);
        assert_eq!(a, 90i32.wrapping_mul(0x6c07_8965) + 1);
        assert_eq!(b, ((a ^ (a >> 30)).wrapping_mul(0x6c07_8965)).wrapping_add(2));
    }

    #[test]
    fn month_reports() {
        // May 1st ends April; April 1st (a new school year) ends March.
        assert_eq!(ended_month(Date { year: 1, month: 5, day: 1 }.number()), 0);
        assert_eq!(ended_month(Date { year: 2, month: 4, day: 1 }.number()), 11);
        // Counted from the game's start: the second year's April is report 12, not 0.
        assert_eq!(ended_month(Date { year: 2, month: 5, day: 1 }.number()), 12);
        assert_eq!(month_index(Date { year: 1, month: 8, day: 20 }.number()), 4);
        assert_eq!(month_total(&[2, 0, 1], |i| [500, 550, 300][i]), 1300);
    }

    #[test]
    fn periods() {
        assert_eq!(period(89), None);
        assert_eq!(period(90), Some(0));
        assert_eq!(period(209), Some(0));
        assert_eq!(period(210), Some(1));
        assert_eq!(period(1169), Some(8));
        assert_eq!(period(1170), None);
        // The third period runs December to March of school year 1.
        let (a, b) = period_days(2);
        assert_eq!((Date::of(a), Date::of(b)), (Date { year: 1, month: 12, day: 1 }, Date { year: 1, month: 3, day: 30 }));
    }
}
