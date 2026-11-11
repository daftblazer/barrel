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
