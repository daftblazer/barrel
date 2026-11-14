//! Gestures in events: what a message line's gesture number (its third field) makes the
//! speaker do, as Escha & Logy DX's executable does it (`docs/formats.md`, "Event messages").
//!
//! A gesture is a chain of body motions. The executable keeps them as a table of 438
//! entries, each two forms of 17 i32: a first motion, up to 15 steps (under 10,000: switch to
//! that motion; 10,000 + n: play division n of the current one, `SubCharaDiv`/`CharaDiv`;
//! -1: unused) and an end type. Logy's line gestures 196 (`EV_ACT_6` division 0, raising his
//! hand to his chin, then 1, holding it: a looping division the chain stays in) and 198
//! (division 5, lowering it, then `F_WAIT`) match the oracle frames of his opening.
//! Motion numbers are the `MOTION_*` enum; the tag without `MOTION_` is a `CharaBase` column.

use barrel_formats::pe::Pe;

pub const COUNT: usize = 438;
const ENTRY: usize = 0x88;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Switch to this motion.
    Motion(u32),
    /// Play this division of the current motion (`Repeat` for its number in the table: the
    /// steps were first read as repeat counts).
    Repeat(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub first: u32,
    pub steps: Vec<Step>,
    pub end: i32,
}

/// Entry 0's first form, as the table holds it: motion 31 (`F_WAIT`), played once more,
/// end type 1; then an empty second form. The table is found by this.
fn first_entry() -> Vec<u8> {
    let mut words = vec![31i32, 10000];
    words.extend([-1; 14]);
    words.push(1);
    words.extend([-1; 17]);
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn form(words: &[i32]) -> Option<Form> {
    let first = u32::try_from(words[0]).ok()?;
    let steps = words[1..16]
        .iter()
        .filter(|&&w| w >= 0)
        .map(|&w| if w >= 10000 { Step::Repeat((w - 10000) as u32) } else { Step::Motion(w as u32) })
        .collect();
    Some(Form { first, steps, end: words[16] })
}

/// Each gesture's forms (the second is empty in most); `None` where a gesture has none.
pub fn read(exe: &[u8]) -> Option<Vec<[Option<Form>; 2]>> {
    let needle = first_entry();
    let at = exe.windows(needle.len()).position(|w| w == needle.as_slice())?;
    let body = exe.get(at..at + COUNT * ENTRY)?;
    Some(
        body.chunks_exact(ENTRY)
            .map(|e| {
                let w: Vec<i32> = e.chunks_exact(4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).collect();
                [form(&w[..17]), form(&w[17..34])]
            })
            .collect(),
    )
}

/// The motion names (`CharaBase` columns) by number, from the executable's `MOTION_*` enum.
pub fn motions(pe: &Pe) -> Vec<String> {
    pe.enum_names("MOTION_WAIT")
        .map(|(names, first)| {
            names[first..].iter().take_while(|n| n.starts_with("MOTION_")).map(|n| n["MOTION_".len()..].to_string()).collect()
        })
        .unwrap_or_default()
}

impl Form {
    /// The motion the gesture holds while it plays: the current motion at its first repeat
    /// (the first motion when it repeats at once).
    pub fn main_motion(&self) -> u32 {
        let mut current = self.first;
        for s in &self.steps {
            match *s {
                Step::Motion(m) => current = m,
                Step::Repeat(_) => return current,
            }
        }
        current
    }
}

impl Form {
    /// The chain's segments in order: (motion, division), a motion with no division step
    /// playing whole.
    pub fn segments(&self) -> Vec<(u32, Option<u32>)> {
        let mut out = Vec::new();
        let (mut current, mut played) = (self.first, false);
        for s in &self.steps {
            match *s {
                Step::Motion(m) => {
                    if !played {
                        out.push((current, None));
                    }
                    (current, played) = (m, false);
                }
                Step::Repeat(n) => {
                    out.push((current, Some(n)));
                    played = true;
                }
            }
        }
        if !played {
            out.push((current, None));
        }
        out
    }

    /// Where the chain is `elapsed` seconds in: its segment and the seconds that segment began
    /// after the gesture did. `length` gives a segment's seconds and whether it loops (None:
    /// unknown, taken as the last). A looping segment holds the chain, as does the last.
    pub fn at(&self, elapsed: f32, mut length: impl FnMut(u32, Option<u32>) -> Option<(f32, bool)>) -> ((u32, Option<u32>), f32) {
        let segments = self.segments();
        let mut began = 0.0;
        for (k, &seg) in segments.iter().enumerate() {
            if k + 1 == segments.len() {
                return (seg, began);
            }
            let Some((seconds, loops)) = length(seg.0, seg.1) else { return (seg, began) };
            if loops || elapsed < began + seconds {
                return (seg, began);
            }
            began += seconds;
        }
        ((self.first, None), 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_play_divisions_in_turn() {
        // 198: division 5 of motion 61 (0.83 s), then 31 whole.
        let lower = Form { first: 61, steps: vec![Step::Repeat(5), Step::Motion(31), Step::Repeat(0)], end: 1 };
        assert_eq!(lower.segments(), [(61, Some(5)), (31, Some(0))]);
        let len = |_m: u32, d: Option<u32>| Some(if d == Some(5) { (0.83, false) } else { (4.0, false) });
        assert_eq!(lower.at(0.5, len), ((61, Some(5)), 0.0));
        assert_eq!(lower.at(4.0, len), ((31, Some(0)), 0.83));
        // 196: division 0 (0.97 s), then 1, which loops and so holds.
        let raise = Form { first: 61, steps: vec![Step::Repeat(0), Step::Repeat(1)], end: 1 };
        let len = |_m: u32, d: Option<u32>| Some(if d == Some(1) { (4.0, true) } else { (0.97, false) });
        assert_eq!(raise.at(30.0, len), ((61, Some(1)), 0.97));
        assert_eq!(Form { first: 31, steps: vec![], end: 1 }.segments(), [(31, None)]);
    }

    #[test]
    fn finds_and_reads_the_table() {
        let mut exe = vec![0u8; 8];
        exe.extend(first_entry());
        // Entry 1: motion 61 for five more loops, then 31.
        let mut e: Vec<i32> = vec![61, 10005, 31, 10000];
        e.extend([-1; 12]);
        e.push(1);
        e.extend([-1; 17]);
        exe.extend(e.iter().flat_map(|w| w.to_le_bytes()));
        exe.resize(8 + COUNT * ENTRY, 0xff);
        let g = read(&exe).unwrap();
        assert_eq!(g.len(), COUNT);
        let f = g[1][0].as_ref().unwrap();
        assert_eq!(f.steps, [Step::Repeat(5), Step::Motion(31), Step::Repeat(0)]);
        assert_eq!((f.main_motion(), f.end), (61, 1));
        assert!(g[1][1].is_none());
    }
}
