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

/// The random traits a gathered item gets (`0x14023d8c0`; only for an item whose row +0x30 is
/// set and a record without a fixed trait in word 13): `count` (the record's word 5, 0 to 3, plus
/// bonus kind 2) traits drawn without repeats, each uniformly from the traits whose `potLv` lies
/// within `low` (at most 98, at least 0) and `high` (at most 99, at least the low) and that `fit`
/// the item (`0x1402a26a0`, `0x1400e2f60`). Returns trait numbers in the order drawn.
pub fn random_traits(
    traits: &[barrel_formats::battle_data::Trait],
    (low, high): (i32, i32),
    count: i32,
    fits: impl Fn(usize) -> bool,
    rng: &mut crate::rng::Rng,
) -> Vec<usize> {
    let low = low.min(98).max(0);
    let high = high.min(99).max(low);
    let mut pool: Vec<usize> = (0..traits.len().min(400)).filter(|&t| (low..=high).contains(&traits[t].level) && fits(t)).collect();
    let mut out = Vec::new();
    for _ in 0..count.max(0) {
        if pool.is_empty() {
            break;
        }
        let k = rng.range(0, pool.len() as i32 - 1) as usize;
        out.push(pool.remove(k));
    }
    out
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

/// An item a gathering gave: its `itemData` row, quality and traits (`item_potential` rows).
#[derive(Clone, Debug, PartialEq)]
pub struct Got {
    pub item: i32,
    pub quality: f32,
    pub traits: Vec<usize>,
}

/// The kinds an item's `base` names, in `ITEM_KIND_*` order (a trait fits a kind).
const KINDS: [&str; 13] = [
    "ITEM_KIND_MATERIAL", "ITEM_KIND_MIX", "ITEM_KIND_ATTACK", "ITEM_KIND_HEAL", "ITEM_KIND_SUPPORT", "ITEM_KIND_FIELD", "ITEM_KIND_EQUIPMENT",
    "ITEM_KIND_IMPORTANT", "ITEM_KIND_BOOK", "ITEM_KIND_ITEM", "ITEM_KIND_WEAPON", "ITEM_KIND_ARMOR", "ITEM_KIND_ACCESSORY",
];

/// One gatherable item's row as the draw reads it: whether it stacks, its `effect`, its kind.
#[derive(Clone, Debug)]
struct ItemRow {
    stacks: bool,
    effect: f32,
    kind: i32,
}

/// What gathering needs, loaded from an install: the lists and points compiled into the
/// English executable, the items and the traits.
pub struct Gathering {
    pub rows: Vec<Gather>,
    pub points: Vec<barrel_formats::gather::Point>,
    items: Vec<ItemRow>,
    traits: Vec<barrel_formats::battle_data::Trait>,
}

impl Gathering {
    pub fn load(root: &std::path::Path) -> Result<Gathering, String> {
        let read = |rel: &str| std::fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"));
        let table = |rel: &str| -> Result<barrel_formats::table::Element, String> { barrel_formats::table::parse(&read(rel)?).map_err(|e| format!("{rel}: {e}")) };
        let exe = read("Atelier_Escha_and_Logy_EN.exe")?;
        let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| format!("executable: {e}"))?;
        let rows = barrel_formats::gather::gathering(&pe, barrel_formats::gather::GATHER_EN).ok_or("no gathering table")?;
        let points = barrel_formats::gather::points(&pe, barrel_formats::gather::POINTS_EN).ok_or("no gathering points")?;
        let items = table("Saves_EN/item/itemData.xml")?
            .children
            .iter()
            .map(|r| ItemRow {
                stacks: r.attr("isStack") == Some("ITEM_STACK_TYPE_STACK"),
                effect: r.attr("effect").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                kind: r.attr("base").and_then(|b| KINDS.iter().position(|k| *k == b)).unwrap_or(0) as i32,
            })
            .collect();
        let (acts, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
        let effects = barrel_formats::battle_data::item_effects(&table("Saves_EN/item/item_effect.xml")?, &acts);
        let tags: Vec<&str> = effects.iter().map(|r| r.tag.as_str()).collect();
        let traits = barrel_formats::battle_data::traits(&table("Saves_EN/item/item_potential.xml")?, &tags);
        Ok(Gathering { rows, points, items, traits })
    }

    /// What field map `map` can give, as the area map's "Gatherable Items" lists it: the items
    /// of its points' lists at table level `level` (their first at 0), each once, by item
    /// number (Sector One, map 26: list 71, Garden Stone 12, Moss Grass 75, Mountain Feather
    /// 141, Eagle Feather 142, the recording's order with the first unknown).
    pub fn field_items(&self, map: i32, level: i32) -> Vec<i32> {
        let mut items: Vec<i32> = self
            .points_in(map)
            .filter_map(|(_, p)| p.lists.get(list_index(p.alchemy, level, level, 0)).or(p.lists.last()).copied())
            .flat_map(|list| self.rows.iter().filter(move |g| g.list == list).map(|g| g.item))
            .collect();
        items.sort_unstable();
        items.dedup();
        items
    }

    /// The points standing in field map `map`.
    pub fn points_in(&self, map: i32) -> impl Iterator<Item = (usize, &barrel_formats::gather::Point)> {
        self.points.iter().enumerate().filter(move |(_, p)| p.map == map)
    }

    /// One gathering at point `point` with the table level `level` (both upgrades' levels; 0
    /// with none), the player's item knowledge, `picks` picks: what it gives, each as the draw
    /// makes it (`0x14023c210`): the point's tier list, weights with the unknown-item bonus,
    /// the picked record's quality, its fixed traits and, for a made item's record without a
    /// fixed trait word, random traits.
    pub fn gather(&self, point: usize, level: i32, knowledge: impl Fn(i32) -> i32, picks: u32, rng: &mut crate::rng::Rng) -> Vec<Got> {
        let Some(p) = self.points.get(point) else { return Vec::new() };
        let k = list_index(p.alchemy, level, level, 0);
        let Some(&list) = p.lists.get(k).or(p.lists.last()) else { return Vec::new() };
        let records: Vec<&Gather> = self.rows.iter().filter(|g| g.list == list).collect();
        let weights = weights(&records, knowledge, rng);
        let mut out = Vec::new();
        for _ in 0..picks {
            let Some(i) = pick(&records, &weights, rng) else { break };
            let g = records[i];
            let Some(row) = self.items.get(g.item.max(0) as usize) else { continue };
            let quality = quality(row.stacks, row.effect, g, 0, rng);
            let mut traits: Vec<usize> = g.traits.iter().map(|&t| t as usize).collect();
            if row.kind != 0 && g.word13 == -1 {
                traits.extend(random_traits(&self.traits, (g.unread[1], g.unread[2]), g.unread[0], |t| self.traits[t].fits(row.kind), rng));
            }
            out.push(Got { item: g.item, quality, traits });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs the game (BARREL_ROOT)"]
    fn sector_one_lists_the_recordings_items() {
        let Ok(root) = std::env::var("BARREL_ROOT") else { return };
        let g = super::Gathering::load(std::path::Path::new(&root)).unwrap();
        assert_eq!(g.field_items(26, 0), [12, 75, 141, 142]);
    }

    use super::*;
    use barrel_formats::gather::Gather;

    fn g(item: i32, weight: i32) -> Gather {
        Gather { list: 71, item, quality: (0, 0), unread: [0; 3], weight, traits: Vec::new(), word13: -1 }
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
    fn random_traits_come_from_the_level_range_without_repeats() {
        let t = |level| barrel_formats::battle_data::Trait { name: String::new(), group: String::new(), level, point: 1, kinds: [true; 9], slots: Vec::new() };
        let traits = vec![t(1), t(4), t(5), t(12), t(6)];
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        let got = random_traits(&traits, (4, 6), 5, |_| true, &mut rng);
        let mut sorted = got.clone();
        sorted.sort();
        assert_eq!(sorted, vec![1, 2, 4]);
        assert!(random_traits(&traits, (4, 6), 2, |t| t != 2, &mut rng).iter().all(|&x| x == 1 || x == 4));
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
