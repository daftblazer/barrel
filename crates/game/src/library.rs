//! The library's knowledge levels (the save's `knowledge` record, `save::Knowledge`): what
//! taking an item teaches (docs/formats.md, "Knowledge").

use barrel_formats::save::{ItemUnit, Knowledge};

/// Known in full: an item made or gathered.
pub const KNOWN: i32 = 10;
/// Seen: an item bought or given.
pub const SEEN: i32 = 2;

/// The level an item comes in at by how it was got (`0x14029f4f0`'s kind): kinds 0 to 4
/// teach it in full (and its recipe book), any other (bought, given) only show it.
pub fn level_for(kind: i32) -> i32 {
    if (0..=4).contains(&kind) { KNOWN } else { SEEN }
}

/// Raises `levels[i]` to `level` (levels never fall); false when `i` is out of the list.
fn raise(levels: &mut [i32], i: i32, level: i32) -> bool {
    match usize::try_from(i).ok().and_then(|i| levels.get_mut(i)) {
        Some(v) => {
            *v = (*v).max(level);
            true
        }
        None => false,
    }
}

/// What taking `unit` at `level` teaches (`0x140369840`): the item, its four effects, its
/// three traits (unless `traits_hidden`, the flag at `0x1410782e3`), and the item's two
/// synthesis skills (`mix_esp`, from the item's row). Returns whether the item rose to a
/// shown level, which posts the game's library notice.
pub fn learn_item(k: &mut Knowledge, unit: &ItemUnit, level: i32, traits_hidden: bool, mix_esp: [i32; 2]) -> bool {
    let item = usize::try_from(unit.item).ok().and_then(|i| k.item.get(i).copied());
    let noticed = item.is_some_and(|old| old < level) && level >= SEEN;
    if unit.item >= 0 {
        raise(&mut k.item, unit.item.into(), level);
        for e in unit.effects {
            raise(&mut k.eff, e.into(), level);
        }
        if !traits_hidden {
            for t in unit.traits {
                raise(&mut k.potential, t.into(), level);
            }
        }
        for m in mix_esp {
            raise(&mut k.mix_esp, m, level);
        }
    }
    noticed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taking_an_item_teaches_its_parts() {
        let mut k = Knowledge { item: vec![0; 4], eff: vec![0; 8], potential: vec![0; 8], mix_esp: vec![0; 4], ..Default::default() };
        let unit = ItemUnit { serial: 1, item: 2, quality: 30.0, traits: [3, -1, -1], word_e: 0, effects: [5, -1, -1, -1], word_18: 0, counts: [0; 4] };
        assert!(learn_item(&mut k, &unit, level_for(8), false, [1, -1]));
        assert_eq!((k.item[2], k.eff[5], k.potential[3], k.mix_esp[1]), (SEEN, SEEN, SEEN, SEEN));
        assert!(learn_item(&mut k, &unit, level_for(0), true, [-1, -1]));
        assert_eq!((k.item[2], k.eff[5], k.potential[3]), (KNOWN, KNOWN, SEEN));
        // Levels never fall, and a known item posts no second notice.
        assert!(!learn_item(&mut k, &unit, SEEN, false, [-1, -1]));
        assert_eq!(k.item[2], KNOWN);
    }
}
