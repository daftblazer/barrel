//! The adventurer rank (English Escha executable): `Saves/rank/RankData.xml`, 21 rows of 0x28
//! bytes once loaded (`[0x140e00f00]` +0x18), the rank kept as a row index (global
//! `0x14109c1e4`) and the points as an integer (global `0x14109c1e0`).

use barrel_formats::table::Element;

/// One rank's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rank {
    /// The points it takes (`point`).
    pub points: i32,
    /// `coal` and `percent`: the month's base pay, and the share of the month's points paid on top
    /// (see [`pay`]).
    pub coal: i32,
    pub percent: i32,
    /// Per-activity rates (`c_btl`, `c_gat`, `c_bou`, `c_mix`, `c_tra`, `c_rel`, `c_que`):
    /// battle, gathering, bou, synthesis, tra, rel, requests (bou, tra and rel not named yet): pay per
    /// activity in the month (see [`pay`]).
    pub rates: [i32; 7],
    /// `rank`: the number shown.
    pub number: i32,
}

pub fn ranks(table: &Element) -> Vec<Rank> {
    let n = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    table
        .children
        .iter()
        .map(|e| Rank {
            points: n(e, "point"),
            coal: n(e, "coal"),
            percent: n(e, "percent"),
            rates: ["c_btl", "c_gat", "c_bou", "c_mix", "c_tra", "c_rel", "c_que"].map(|k| n(e, k)),
            number: n(e, "rank"),
        })
        .collect()
}

/// The top of the current tier (`0x14035fd90`): ranks go up in tiers of three, and this is
/// `(trunc((rank - 1) / 3) + 1) × 3`.
pub fn tier_top(rank: i32) -> i32 {
    ((rank - 1) / 3 + 1) * 3
}

/// The rank after the points change (`0x14035fdc0`): from the current rank, up through every
/// following row whose points are at most `points`, no further than the last row; never down.
/// The second value says whether the climb passed the tier's top (the game then tells the
/// manager at `0x140dfc220`, `0x1402681b0`, before storing the rank).
pub fn promote(ranks: &[Rank], rank: i32, points: i32) -> (i32, bool) {
    let last = ranks.len() as i32 - 1;
    let mut r = rank;
    while r < last && ranks[(r + 1) as usize].points <= points {
        r += 1;
    }
    if r <= rank {
        return (rank, false);
    }
    (r, r > tier_top(rank))
}

/// The points and rank as the game keeps them, with the points made in each assignment period.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RankState {
    pub points: i32,
    pub rank: i32,
    /// Points made in each of the nine assignment periods (`0x14109c1bc`).
    pub by_period: [i32; 9],
}

impl RankState {
    /// Adds `n` points (`0x14035fc90`): to the total, to the current period's count (`period`,
    /// global `0x141099e04`, when 0 to 8) and to the month's report (+0x138, not kept here),
    /// then promotes. Returns whether the climb passed a tier's top. The points come from
    /// requests done (`trunc(GetPoint)` when above 0, `0x14033b24d`), assignments cleared (their
    /// `Score`, `0x14035775c`), assignment card lines completed (`lineScore`, `0x1403605c6`, which
    /// adds the same way), event commands (`0x14002f410`), and are taken back as the negated sum
    /// of 25 assignments' scores at `0x1400cd9a2`.
    pub fn add(&mut self, ranks: &[Rank], n: i32, period: Option<usize>) -> bool {
        self.points = self.points.wrapping_add(n);
        if let Some(p) = period.filter(|&p| p < 9) {
            self.by_period[p] = self.by_period[p].wrapping_add(n);
        }
        let (rank, notice) = promote(ranks, self.rank, self.points);
        self.rank = rank;
        notice
    }
}

/// A month's pay (`0x140114530`, the month report's screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pay {
    /// The rank's `coal` (kept in the report at +0x128).
    pub base: i32,
    /// `trunc(the month's points × percent / 100)`, at least 0 (+0x12c).
    pub bonus: i32,
    /// Each rate times the month's count of that activity, summed.
    pub activity: i32,
    pub total: i32,
}

/// The pay for a month at `rank`, with `points` the points made that month (the report's
/// +0x138) and `counts` the month's activities in the rates' order (battles, gatherings, bou,
/// syntheses, tra, rel, requests: the month report's +0x10c to +0x124, each raised by one by
/// its own function, `0x1403639c0` and on, from battle, field, synthesis and request code). The report is then marked paid (+0x140 = 2).
pub fn pay(rank: &Rank, points: i32, counts: [i32; 7]) -> Pay {
    let base = rank.coal;
    let bonus = (points.wrapping_mul(rank.percent) / 100).max(0);
    let activity = rank.rates.iter().zip(counts).map(|(r, c)| r.wrapping_mul(c)).fold(0, i32::wrapping_add);
    Pay { base, bonus, activity, total: base.wrapping_add(bonus).wrapping_add(activity) }
}

/// A request's points (`0x14033b24d`): its `GetPoint` cut to an integer, when above 0.
pub fn request_points(get_point: f32) -> i32 {
    if get_point > 0.0 { get_point as i32 } else { 0 }
}

/// The event that opens refining (`EVENT_MM04_SYS_040`, 66): row 9 of the comments (refining
/// done little) is only offered once it has been seen.
pub const REFINING_EVENT: usize = 66;

/// The month report's comment (`0x140114e20`, when the report's +0x130 is still -1): the rows
/// whose start to end takes in month `month + 1` (`month` the report's number, from 0), whose
/// counts each hold (a count above 0 asks for the month's count to be at least it with MORE,
/// at most it with LESS; 0 asks nothing, so "LESS 0" never bites), row 9 only once
/// `REFINING_EVENT` is seen; of those, the ones in the highest group, one picked with
/// `0x1404ca660 (0, n - 1)`. `counts` are the month's battles, gatherings, items, syntheses,
/// refinings, artifacts and requests (the report's +0x10c to +0x124). The pick is kept in the
/// report (+0x130). None when no row fits (no generator number is drawn then either).
pub fn comment(
    rows: &[crate::data::salary::Comment],
    month: i32,
    counts: [i32; 7],
    refining_seen: bool,
    rng: &mut barrel_engine::rng::Rng,
) -> Option<usize> {
    let mut found: Vec<usize> = Vec::new();
    let mut top = 0;
    for (k, r) in rows.iter().enumerate() {
        if r.start - 1 > month || r.end <= month || r.group < top {
            continue;
        }
        let holds = r.counts.iter().zip(counts).all(|(c, n)| match *c {
            Some((v, more)) if v > 0 => if more { n >= v } else { n <= v },
            _ => true,
        });
        if !holds || (k == 9 && !refining_seen) {
            continue;
        }
        found.push(k);
        top = top.max(r.group);
    }
    found.retain(|&k| rows[k].group >= top);
    if found.is_empty() {
        return None;
    }
    let n = rng.range(0, found.len() as i32 - 1);
    Some(found[n as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Rank> {
        [0, 750, 1500, 2500, 4000].iter().enumerate().map(|(i, &p)| Rank { points: p, coal: 300, percent: 10, rates: [0; 7], number: i as i32 + 1 }).collect()
    }

    #[test]
    fn points_add_up() {
        let t = table();
        let mut st = RankState::default();
        assert!(!st.add(&t, request_points(5.12) * 150, Some(0)));
        assert_eq!((st.points, st.rank, st.by_period[0]), (750, 1, 750));
        assert_eq!(request_points(-1.0), 0);
    }

    #[test]
    fn a_months_pay() {
        let r = Rank { points: 0, coal: 300, percent: 10, rates: [10, 5, 5, 15, 15, 10, 15], number: 1 };
        let p = pay(&r, 755, [2, 0, 0, 3, 0, 0, 1]);
        assert_eq!((p.base, p.bonus, p.activity, p.total), (300, 75, 20 + 45 + 15, 455));
        assert_eq!(pay(&r, -50, [0; 7]).bonus, 0);
    }

    #[test]
    fn promotion() {
        let t = table();
        assert_eq!(promote(&t, 0, 700), (0, false));
        assert_eq!(promote(&t, 0, 1600), (2, false));
        // From rank 1, the tier's top is 3: reaching 4 passes it.
        assert_eq!(tier_top(1), 3);
        assert_eq!(promote(&t, 1, 5000), (4, true));
        assert_eq!(promote(&t, 3, 0), (3, false));
    }

    #[test]
    fn month_comment() {
        use crate::data::salary::Comment;
        let row = |group, start, end, counts: [Option<(i32, bool)>; 7]| Comment { name: String::new(), group, start, end, counts, text: String::new() };
        let none = [None; 7];
        let mut many_battles = none;
        many_battles[0] = Some((5, true));
        let mut few_battles = none;
        few_battles[0] = Some((2, false));
        let rows = vec![row(1, 1, 48, none), row(3, 1, 48, many_battles), row(2, 1, 48, few_battles), row(5, 1, 1, none)];
        let mut rng = barrel_engine::rng::Rng::seeded(3);
        // The first month: the game-start row (group 5) beats everything.
        assert_eq!(comment(&rows, 0, [9, 0, 0, 0, 0, 0, 0], true, &mut rng), Some(3));
        // Later: 9 battles fit "many" (group 3) over the general row (group 1).
        assert_eq!(comment(&rows, 4, [9, 0, 0, 0, 0, 0, 0], true, &mut rng), Some(1));
        assert_eq!(comment(&rows, 4, [1, 0, 0, 0, 0, 0, 0], true, &mut rng), Some(2));
        assert_eq!(comment(&rows, 4, [3, 0, 0, 0, 0, 0, 0], true, &mut rng), Some(0));
        assert_eq!(comment(&rows, 60, [3, 0, 0, 0, 0, 0, 0], true, &mut rng), None);
    }
}
