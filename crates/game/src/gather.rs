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

/// The field's gathering state (`0x1410783b0`: a mode, then three values), which some field
/// effects set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldState {
    pub mode: i32,
    pub values: [i32; 3],
}

/// One of the gathering bonuses (`0x1400d3030 (kind, gather_type, cleared)`): kind 0, the extra
/// picks, is the field's first value for a type-0 gather while its mode is 3 (nothing when
/// `cleared`); kinds 1, 2 and 3 (the random traits' levels, a third not followed yet, the
/// quality percentage) are its second, third and first values for a type-1 gather while its mode
/// is 5; kinds 4 and 5 are the gathering and alchemy table levels. Anything else is 0.
pub fn bonus(kind: u32, gather_type: i32, cleared: bool, field: FieldState, gat_level: i32, alc_level: i32) -> i32 {
    match kind {
        0 if gather_type == 0 && !cleared && field.mode == 3 => field.values[0],
        1 if gather_type == 1 && field.mode == 5 => field.values[1],
        2 if gather_type == 1 && field.mode == 5 => field.values[2],
        3 if gather_type == 1 && field.mode == 5 => field.values[0],
        4 => gat_level,
        5 => alc_level,
        _ => 0,
    }
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

/// A gathered item's quality (`0x14023c8b0`): a stacking item (`itemData` `isStack`
/// `ITEM_STACK_TYPE_STACK`, the loaded row's +0x2c) comes at its row's `effect` (+0x40), at
/// most 120; any other at a roll between the record's quality bounds, raised by
/// `bonus_percent` percent of itself (`0x1400d3030 (3, ...)`, 0 when the gather's flag says so),
/// within 0 and 120.
pub fn quality(stacks: bool, effect: f32, record: &Gather, bonus_percent: i32, rng: &mut crate::rng::Rng) -> f32 {
    if stacks {
        return effect.min(120.0).max(0.0);
    }
    let q = rng.range(record.quality.0, record.quality.1) as f32;
    let q = if bonus_percent > 0 { q + bonus_percent as f32 * 0.01 * q } else { q };
    q.min(120.0).max(0.0)
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
    fn bonuses_follow_the_field_mode() {
        let field = FieldState { mode: 5, values: [20, 3, 7] };
        assert_eq!(bonus(3, 1, false, field, 0, 0), 20);
        assert_eq!(bonus(1, 1, false, field, 0, 0), 3);
        assert_eq!(bonus(0, 0, false, field, 0, 0), 0);
        assert_eq!(bonus(0, 0, false, FieldState { mode: 3, ..field }, 0, 0), 20);
        assert_eq!(bonus(0, 0, true, FieldState { mode: 3, ..field }, 0, 0), 0);
        assert_eq!((bonus(4, 0, false, field, 2, 1), bonus(5, 0, false, field, 2, 1)), (2, 1));
    }

    #[test]
    fn gathered_quality() {
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        // A Spiky Bud (stacks, effect 60) comes at 60, as the running game's did.
        assert_eq!(quality(true, 60.0, &g(70, 50), 0, &mut rng), 60.0);
        let chest = Gather { quality: (60, 70), ..g(195, 40) };
        let q = quality(false, 50.0, &chest, 0, &mut rng);
        assert!((60.0..=70.0).contains(&q));
        let q = quality(false, 50.0, &Gather { quality: (100, 100), ..g(195, 40) }, 50, &mut rng);
        assert_eq!(q, 120.0);
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
