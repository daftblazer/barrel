//! The alchemy level: the experience a synthesis gives and the levels it reaches (docs/engine.md "Ryza DX: the alchemy level").
//!
//! The experience each level needs is a table compiled into the executable (99 rows, the alchemy's column): it is read from the
//! player's own executable, found by the code that looks it up, never copied.

/// The highest alchemy level.
pub const MAX_LEVEL: u32 = 99;

/// How many times a recipe has to have been made before its experience starts to shrink with the alchemy level (the game counts up to it).
pub const MADE_FOR_PENALTY: u32 = 10;

/// The experience lost for each alchemy level once a recipe has been made `MADE_FOR_PENALTY` times (the game's f32 -0.2, truncated).
const PENALTY_PER_LEVEL: f32 = -0.2;

/// The experience each alchemy level needs, from level 1 (0) up.
#[derive(Clone, Debug, PartialEq)]
pub struct Levels {
    need: Vec<u32>,
}

/// The lookup's code: `cmp eax, 0x62; ja ..; lea rcx, [rdi + rdi*2]; lea rax, [rip + TABLE]`, then the alchemy column's
/// `mov eax, [rax + rcx*4 - 8]` (the other column, `- 0xc`, is read just before it). `None` stands for the jump's offset and the table's.
const LOOKUP: [Option<u8>; 24] = {
    const fn b(v: u8) -> Option<u8> {
        Some(v)
    }
    [
        b(0x83), b(0xf8), b(0x62), b(0x77), None, b(0x48), b(0x8d), b(0x0c), b(0x7f), b(0x48), b(0x8d), b(0x05),
        None, None, None, None, b(0x8b), b(0x44), b(0x88), b(0xf4), b(0x48), b(0x8b), b(0x5c), b(0x24),
    ]
};

/// Each table row: three i32, the alchemy's experience the second.
const ROW: usize = 12;
const ALCHEMY_COLUMN: usize = 4;

impl Levels {
    /// The table from the game's executable, or `None` if its lookup is not found (another build).
    pub fn from_exe(pe: &barrel_formats::pe::Pe) -> Option<Levels> {
        let data = pe.data;
        let at = data.windows(LOOKUP.len()).position(|w| w.iter().zip(&LOOKUP).all(|(b, p)| p.is_none_or(|p| *b == p)))?;
        let lea = at + 12;
        let rel = i32::from_le_bytes(data[lea..lea + 4].try_into().ok()?);
        let next = pe.va(lea + 4)?;
        let table = pe.offset(next.checked_add_signed(rel as i64)?)?;
        let need = (0..MAX_LEVEL as usize)
            .map(|row| data.get(table + row * ROW + ALCHEMY_COLUMN..).and_then(|b| b.get(..4)).map(|b| u32::from_le_bytes(b.try_into().unwrap())))
            .collect::<Option<Vec<u32>>>()?;
        Levels::new(need)
    }

    /// A table of the experience each level from 1 needs (rising, level 1 at 0).
    pub fn new(need: Vec<u32>) -> Option<Levels> {
        (need.first() == Some(&0) && need.windows(2).all(|w| w[0] <= w[1])).then_some(Levels { need })
    }

    /// The experience `level` needs, if the table has it.
    pub fn need(&self, level: u32) -> Option<u32> {
        self.need.get(level.checked_sub(1)? as usize).copied()
    }

    /// The level `exp` reaches.
    pub fn level_for(&self, exp: u32) -> u32 {
        self.need.iter().take_while(|&&n| n <= exp).count().max(1) as u32
    }

    /// The Level UP screen's "next": the experience still needed for the level after `level`, `None` at the top ("--").
    pub fn next(&self, level: u32, exp: u32) -> Option<u32> {
        Some(self.need(level + 1)?.saturating_sub(exp))
    }
}

/// The experience a synthesis gives: `batches` times the made item's level; once the recipe has been made `MADE_FOR_PENALTY` times,
/// less a fifth of the alchemy level; at least 1. At the top level it gives none.
pub fn gain(level: u32, batches: u32, item_level: u32, made_before: u32) -> u32 {
    if level >= MAX_LEVEL {
        return 0;
    }
    let made = (batches * item_level) as i32;
    let exp = if made_before < MADE_FOR_PENALTY { made } else { made + (level as f32 * PENALTY_PER_LEVEL) as i32 };
    exp.max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> Levels {
        Levels::new(vec![0, 1, 4, 8, 14, 21]).unwrap()
    }

    #[test]
    fn levels_and_next() {
        let l = levels();
        assert_eq!(l.level_for(0), 1);
        assert_eq!(l.level_for(1), 2);
        assert_eq!(l.level_for(3), 2);
        assert_eq!(l.level_for(4), 3);
        // The capture's Level UP: one experience from 0 reaches level 2, the next needs 3 more.
        assert_eq!(l.next(2, 1), Some(3));
        assert_eq!(l.next(6, 21), None);
    }

    #[test]
    fn gains() {
        assert_eq!(gain(1, 1, 1, 0), 1);
        assert_eq!(gain(1, 1, 0, 0), 1);
        assert_eq!(gain(20, 2, 15, 3), 30);
        assert_eq!(gain(20, 2, 15, 10), 26);
        assert_eq!(gain(30, 1, 3, 10), 1);
        assert_eq!(gain(99, 1, 50, 0), 0);
    }

    #[test]
    fn rejects_a_table_that_is_not_one() {
        assert!(Levels::new(vec![3, 1]).is_none());
        assert!(Levels::new(vec![]).is_none());
    }
}
