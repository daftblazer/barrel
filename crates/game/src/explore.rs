//! The battle item grid (`explore_equip`): each alchemist takes items into the field on a
//! row of 25 squares, as many open as the capacity allows. An item covers as many squares as
//! its `EquipCost` (`itemData.xml`), starting at its own square: the saved item list is
//! indexed by that first square. See docs/formats.md, "Battle items".

/// The grid's squares.
pub const SQUARES: usize = 25;
/// A square past the capacity.
pub const LOCKED: i32 = 25;
/// A free square.
pub const FREE: i32 = -1;

/// The grid as the game saves it, from the items (first square, equip cost) and the number
/// of open squares; `None` when two items overlap or one runs past the open squares.
pub fn grid(items: &[(usize, u32)], capacity: usize) -> Option<[i32; SQUARES]> {
    let open = capacity.min(SQUARES);
    let mut g = [FREE; SQUARES];
    g[open..].fill(LOCKED);
    for &(start, cost) in items {
        let end = start + cost.max(1) as usize;
        if end > open || g[start..end].iter().any(|&s| s != FREE) {
            return None;
        }
        g[start..end].fill(start as i32);
    }
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_cover_their_cost() {
        // One save's Escha: a Bomb (cost 3) at 0, another at 5, a Healing Salve (1) at 8.
        let g = grid(&[(0, 3), (5, 3), (8, 1)], 10).unwrap();
        assert_eq!(&g[..10], &[0, 0, 0, -1, -1, 5, 5, 5, 8, -1]);
        assert!(g[10..].iter().all(|&s| s == LOCKED));
        assert_eq!(grid(&[(0, 3), (2, 1)], 10), None);
        assert_eq!(grid(&[(9, 2)], 10), None);
    }
}
