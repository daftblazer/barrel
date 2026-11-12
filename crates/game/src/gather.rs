//! Drawing what a gathering list gives (`0x14023c210`, English Escha executable; the lists are
//! `barrel_formats::gather`, docs/tables.md "Gathering").

use barrel_formats::gather::Gather;

/// The weights the draw uses for a list's records: each record's own weight, plus a roll of
/// 30 to 100 when the player knows its item at level 2 or less (`knowledge(item)`, the save's
/// item knowledge list at `0x141079fe0`), so items not yet known turn up more.
pub fn weights(records: &[&Gather], knowledge: impl Fn(i32) -> i32, rng: &mut crate::rng::Rng) -> Vec<f32> {
    records
        .iter()
        .map(|g| {
            let bonus = if knowledge(g.item) <= 2 { rng.range(30, 100) } else { 0 };
            (g.weight + bonus) as f32
        })
        .collect()
}

/// Which of a point's lists it draws from (`0x1400d3160`): the level of a "table level-up"
/// effect on one of two items the game keeps (item units at `0x140dfc8d4` and `0x140dfc8b0`),
/// at most 4. A point whose byte +0x30 is set takes `ACT_ALC_TABLE_LEVELUP` (215) of the first;
/// the others `ACT_GAT_TABLE_LEVELUP` (216) of the second, plus `field_bonus` (`0x1410783b4`,
/// counted only while `0x1410783b0` is 4). The index picks the point's list word.
pub fn list_index(alchemy_point: bool, alc_level: i32, gat_level: i32, field_bonus: i32) -> usize {
    let level = if alchemy_point { alc_level } else { field_bonus + gat_level };
    level.clamp(0, 4) as usize
}

/// One pick from a list (the loop at `0x14023c80f`): a roll of 0 to 100, then the records in
/// order, those with a weight of their own above 0 each adding its share (weight × 100 /
/// total), the first to reach the roll; when none does, the first with a drawn weight above 0,
/// else the last. `None` for an empty list.
pub fn pick(records: &[&Gather], weights: &[f32], rng: &mut crate::rng::Rng) -> Option<usize> {
    if records.is_empty() {
        return None;
    }
    let total: f32 = weights.iter().sum();
    let total = if total <= 0.0 { 1.0 } else { total };
    let roll = rng.range(0, 100) as f32;
    let mut sum = 0.0f32;
    for (i, (g, w)) in records.iter().zip(weights).enumerate() {
        if g.weight > 0 {
            sum += w * 100.0 / total;
            if sum >= roll {
                return Some(i);
            }
        }
    }
    Some(weights[..records.len() - 1].iter().position(|&w| w > 0.0).unwrap_or(records.len() - 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::gather::Gather;

    fn g(item: i32, weight: i32) -> Gather {
        Gather { list: 71, item, quality: (0, 0), unread: [0; 3], weight, traits: Vec::new() }
    }

    #[test]
    fn known_items_draw_by_their_weights() {
        // List 71 with everything well known: the weights are the records' own.
        let list = [g(75, 50), g(12, 30), g(141, 15), g(142, 5)];
        let refs: Vec<&Gather> = list.iter().collect();
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        let w = weights(&refs, |_| 5, &mut rng);
        assert_eq!(w, vec![50.0, 30.0, 15.0, 5.0]);
        let mut counts = [0; 4];
        for _ in 0..10_000 {
            counts[pick(&refs, &w, &mut rng).unwrap()] += 1;
        }
        // About 50/30/15/5 percent (the roll runs 0 to 100, so the first share gets a little more).
        assert!((4700..5400).contains(&counts[0]), "{counts:?}");
        assert!((2700..3300).contains(&counts[1]), "{counts:?}");
        assert!(counts[3] > 300 && counts[3] < 700, "{counts:?}");
    }

    #[test]
    fn the_table_level_picks_the_list() {
        assert_eq!(list_index(false, 3, 0, 0), 0);
        assert_eq!(list_index(false, 0, 2, 1), 3);
        assert_eq!(list_index(false, 0, 9, 0), 4);
        assert_eq!(list_index(true, 2, 4, 0), 2);
    }

    #[test]
    fn unknown_items_get_a_boost() {
        let list = [g(75, 50), g(142, 5)];
        let refs: Vec<&Gather> = list.iter().collect();
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        let w = weights(&refs, |item| if item == 142 { 0 } else { 5 }, &mut rng);
        assert_eq!(w[0], 50.0);
        assert!((35.0..=105.0).contains(&w[1]));
    }
}
