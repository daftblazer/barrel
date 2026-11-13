//! Assignment cards' lines (English Escha executable; the assignments are
//! `barrel_formats::subject`, docs/tables.md "Requests" and the job points).

/// The card's inner 3 × 3 squares (of its 25, row by row), the only ones lines run through
/// (`0x140939600`).
pub const INNER: [usize; 9] = [6, 7, 8, 11, 12, 13, 16, 17, 18];

/// How many lines a card has (`0x140360340`): with every one of its 25 squares cleared, 9;
/// otherwise the rows, columns and two diagonals of its inner 3 × 3 whose three squares are all
/// cleared, up to 8.
pub fn lines(cleared: &[bool; 25]) -> u32 {
    if cleared.iter().all(|&c| c) {
        return 9;
    }
    let c = INNER.map(|s| cleared[s]);
    const LINES: [[usize; 3]; 8] = [[0, 1, 2], [3, 4, 5], [6, 7, 8], [0, 3, 6], [1, 4, 7], [2, 5, 8], [0, 4, 8], [2, 4, 6]];
    LINES.iter().filter(|l| l.iter().all(|&k| c[k])).count() as u32
}

/// What reaching `lines` pays on a card whose lines already paid number `paid` (`0x1403604d0`,
/// the save's `subject/bingo_num` byte for the card): each newly reached step k, from `paid` up
/// to `lines` (at most 9), its `(lineBonus, lineScore)` pair from the card's record (+0x20 + 8k):
/// the bonus row applied when not -1 and the score added to the job points when above 0
/// (`barrel_formats::subject::Card::steps`).
/// Returns the steps' pairs and the new count paid; nothing when `lines` is not above `paid`.
pub fn pay<T: Clone>(lines: u32, paid: u32, steps: &[T]) -> (Vec<T>, u32) {
    let lines = lines.min(9);
    if lines <= paid {
        return (Vec::new(), paid);
    }
    (steps.iter().skip(paid as usize).cloned().take((lines - paid) as usize).collect(), lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_on_a_card() {
        let mut c = [false; 25];
        for s in [6, 12, 18] {
            c[s] = true;
        }
        assert_eq!(lines(&c), 1);
        for s in [7, 8] {
            c[s] = true;
        }
        // The top row and the diagonal.
        assert_eq!(lines(&c), 2);
        assert_eq!(lines(&[true; 25]), 9);
        let steps = [(5, 10), (-1, 20), (7, 30)];
        assert_eq!(pay(2, 0, &steps), (vec![(5, 10), (-1, 20)], 2));
        assert_eq!(pay(2, 2, &steps), (vec![], 2));
    }
}
