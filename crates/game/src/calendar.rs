//! The calendar: the game counts days from 1, in 30-day months and 360-day years, day 1 being
//! the first of January of year 1. The story's assignments run in four-month periods from the
//! first of April of year 1 (day 91) for three years (`subject/SubjectCardData.xml`: nine
//! cards, "Year 1 April to July" ... "Year 3 December to March"). This is read from the data:
//! every request's `StartDate` and `EndDate` in `quest/QuestAlc.xml` falls on a period's first
//! day (91, 211, 331, ... 1171) or on 1530; the game's own date code is not read yet.

pub const DAYS_PER_MONTH: u32 = 30;
pub const DAYS_PER_YEAR: u32 = 12 * DAYS_PER_MONTH;
/// The first day of the first assignment period (April 1st, year 1).
pub const FIRST_PERIOD_DAY: u32 = 91;
pub const PERIOD_DAYS: u32 = 4 * DAYS_PER_MONTH;
/// Nine assignment periods, three a year.
pub const PERIODS: u32 = 9;

/// A date on the game's calendar (all from 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// The date of day number `n` (from 1).
    pub fn of(n: u32) -> Date {
        let d = n.max(1) - 1;
        Date { year: d / DAYS_PER_YEAR + 1, month: d % DAYS_PER_YEAR / DAYS_PER_MONTH + 1, day: d % DAYS_PER_MONTH + 1 }
    }

    /// Its day number.
    pub fn number(self) -> u32 {
        (self.year - 1) * DAYS_PER_YEAR + (self.month - 1) * DAYS_PER_MONTH + self.day
    }
}

/// The assignment period (0 to 8, a `SubjectCardData` row) day `n` falls in; `None` before
/// the first one and after the last.
pub fn period(n: u32) -> Option<u32> {
    let k = n.checked_sub(FIRST_PERIOD_DAY)? / PERIOD_DAYS;
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
        assert_eq!(Date::of(1), Date { year: 1, month: 1, day: 1 });
        assert_eq!(Date::of(91), Date { year: 1, month: 4, day: 1 });
        assert_eq!(Date::of(360), Date { year: 1, month: 12, day: 30 });
        assert_eq!(Date::of(451), Date { year: 2, month: 4, day: 1 });
        for n in [1, 91, 330, 1171, 1530] {
            assert_eq!(Date::of(n).number(), n);
        }
    }

    #[test]
    fn periods() {
        assert_eq!(period(90), None);
        assert_eq!(period(91), Some(0));
        assert_eq!(period(210), Some(0));
        assert_eq!(period(211), Some(1));
        assert_eq!(period(1170), Some(8));
        assert_eq!(period(1171), None);
        assert_eq!(period_days(2), (331, 450));
        // The third period runs December to March.
        assert_eq!((Date::of(331).month, Date::of(450).month), (12, 3));
    }
}
