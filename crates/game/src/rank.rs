//! The adventurer rank (English Escha executable): `Saves/rank/RankData.xml`, 21 rows of 0x28
//! bytes once loaded (`[0x140e00f00]` +0x18), the rank kept as a row index (global
//! `0x14109c1e4`) and the points as an integer (global `0x14109c1e0`).

use barrel_formats::table::Element;

/// One rank's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rank {
    /// The points it takes (`point`).
    pub points: i32,
    /// `coal` and `percent`: a sum and a percentage that come with the rank (pay, presumably).
    pub coal: i32,
    pub percent: i32,
    /// Per-activity rates (`c_btl`, `c_gat`, `c_bou`, `c_mix`, `c_tra`, `c_rel`, `c_que`):
    /// battle, gathering, bou (adventure?), synthesis, travel, rel (?), requests; how the
    /// points are earned from them is not read yet.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Rank> {
        [0, 750, 1500, 2500, 4000].iter().enumerate().map(|(i, &p)| Rank { points: p, coal: 300, percent: 10, rates: [0; 7], number: i as i32 + 1 }).collect()
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
}
