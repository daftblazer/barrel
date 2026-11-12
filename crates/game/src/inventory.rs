//! The item lists (English Escha executable): one class (vtable `0x1408f7cb8`, set up by
//! `0x14029f310 (list, kind, size, slots)`) for the basket (kind 0, 120 slots, `0x1410bcab0`),
//! the container (1, 999), the two lists of important items (2 and 3, 100 each), the materials
//! (4, 200) and the battle's spoils (8, 40, `0x140d8de30`). A list keeps its kind (+8), its limit
//! (+0xc: how many slots count, `0x14029f3b0` sets it, never past the size) and its size (+0x10).

use barrel_formats::save::ItemUnit;

/// The next item serial (`0x14035cc30`, the counter at `0x14107c4c4`, which the save keeps as
/// `item/ID`): it gives out the counter's value and moves it on by one, or, once it has given
/// out 0x7fff, back to 0x3fff (setting the copy at `0x14109352c` too).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Serials(pub u16);

impl Serials {
    pub fn next(&mut self) -> u16 {
        let s = self.0;
        self.0 = if s < 0x7fff { s + 1 } else { 0x3fff };
        s
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemList {
    pub kind: i32,
    pub limit: usize,
    /// Its slots, `None` empty.
    pub slots: Vec<Option<ItemUnit>>,
}

impl ItemList {
    pub fn new(kind: i32, size: usize) -> ItemList {
        ItemList { kind, limit: size, slots: vec![None; size] }
    }

    /// How many slots are taken.
    pub fn count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// The room left (`0x14029f410`): the limit less the slots taken.
    pub fn room(&self) -> i64 {
        self.limit as i64 - self.count() as i64
    }

    /// Whether this kind of list numbers what it takes (`0x14029f440`: kinds 0 to 4, 8 and 10).
    fn numbers(&self) -> bool {
        (0..=4).contains(&self.kind) || self.kind == 8 || self.kind == 10
    }

    /// Adds `count` of `unit` (`0x14029f440`, the list's method 3). In the materials list (kind
    /// 4) an item that stacks (`itemData` `isStack`, the row's +0x2c, 1) joins the first slot,
    /// within the limit, holding the same item, its count (+0x22) going up by `count`; otherwise
    /// it takes the first empty slot within the limit with that count when it stacks, or with a
    /// count of 1 there and in each following empty slot for every further one when it does not.
    /// Units without a serial (negative) get the next one. Returns the first slot used, `None`
    /// when no slot within the limit was free (the further copies stop at the limit).
    pub fn add(&mut self, unit: ItemUnit, count: i16, stacks: bool, serials: &mut Serials) -> Option<usize> {
        let stacking = self.kind == 4 && stacks;
        let limit = self.limit.min(self.slots.len());
        let mut at = None;
        for k in 0..limit {
            match &mut self.slots[k] {
                Some(u) if stacking && u.item == unit.item => {
                    u.counts[3] += count;
                    return Some(k);
                }
                Some(_) => {}
                None => {
                    at = Some(k);
                    break;
                }
            }
        }
        let first = at?;
        let mut put = |slot: usize, n: i16, serials: &mut Serials| {
            let mut u = unit;
            if self.numbers() && (u.serial as i16) < 0 {
                u.serial = serials.next();
            }
            u.counts[3] = n;
            self.slots[slot] = Some(u);
        };
        if stacking {
            put(first, count, serials);
            return Some(first);
        }
        put(first, 1, serials);
        let mut slot = first + 1;
        for _ in 1..count.max(1) {
            if slot >= limit {
                break;
            }
            put(slot, 1, serials);
            slot += 1;
        }
        Some(first)
    }
}

impl ItemList {
    /// Takes out the item in slot `index` (`0x14029f6f0`): those after it move up one and the
    /// last taken slot is emptied, so a list stays packed from its start. `None` past the items.
    pub fn remove(&mut self, index: usize) -> Option<ItemUnit> {
        let count = self.count();
        if index >= count {
            return None;
        }
        let taken = self.slots[index];
        self.slots.copy_within(index + 1..count, index);
        self.slots[count - 1] = None;
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(item: i16) -> ItemUnit {
        ItemUnit { serial: 0xffff, item, quality: 50.0, traits: [-1; 3], word_e: 0, effects: [-1; 4], equip_cost: 0, word_1a: 0, counts: [0; 4] }
    }

    #[test]
    fn materials_stack_and_the_basket_does_not() {
        let mut serials = Serials(100);
        let mut materials = ItemList::new(4, 200);
        assert_eq!(materials.add(unit(70), 3, true, &mut serials), Some(0));
        assert_eq!(materials.add(unit(70), 2, true, &mut serials), Some(0));
        assert_eq!(materials.slots[0].unwrap().counts[3], 5);
        assert_eq!(materials.count(), 1);
        let mut basket = ItemList::new(0, 120);
        basket.limit = 60;
        assert_eq!(basket.add(unit(12), 3, false, &mut serials), Some(0));
        assert_eq!(basket.count(), 3);
        let serials_given: Vec<u16> = basket.slots[..3].iter().map(|s| s.unwrap().serial).collect();
        assert_eq!(serials_given, vec![101, 102, 103]);
        assert_eq!(basket.room(), 57);
    }

    #[test]
    fn taking_out_keeps_the_list_packed() {
        let mut serials = Serials(1);
        let mut basket = ItemList::new(0, 120);
        for item in [10, 11, 12] {
            basket.add(unit(item), 1, false, &mut serials);
        }
        assert_eq!(basket.remove(0).map(|u| u.item), Some(10));
        assert_eq!(basket.slots[..3].iter().map(|s| s.map(|u| u.item)).collect::<Vec<_>>(), vec![Some(11), Some(12), None]);
        assert_eq!(basket.remove(5), None);
    }

    #[test]
    fn serials_wrap() {
        let mut s = Serials(0x7ffe);
        assert_eq!((s.next(), s.next(), s.next()), (0x7ffe, 0x7fff, 0x3fff));
    }
}
