//! What events change: group 0 when they start (`0x1402178d0`), group 1 at their script's
//! `ECT_END` (`0x140052c0b`) (`Event/eventprocess/EventProcessDataAll.epd`, read by
//! `0x140059c60` into a map at `0x140df9eb0`): an i32 count (1,165, one per `EVENT_*`), then per
//! event a group count and, per group, a word count and that many words. The words are event
//! commands (`0x7710xxxx`-`0x7716xxxx`, run by the event command handler `0x140052391`), each
//! group ending with `ECT_END` (`0x7f000000`).

use crate::Error;

/// The commands found in the file: word, operand count, what the handler does.
pub static CODES: &[(u32, usize, &str)] = &[
    (0x77100002, 1, "money down by the operand (0x14036a8e0 of its negative)"),
    (0x77100004, 2, "item set to the basket (where 0) or the container (1) (0x1400d2ab0)"),
    (0x77110001, 1, "character joins the companion list (0x140330220)"),
    (0x7711000a, 2, "character's level raised to the hero's level + operand 2 (0x14032daa0)"),
    (0x77120001, 2, "event marked seen (operand 2 non-zero) or not (0x14035f480)"),
    (0x77130002, 2, "event flag change (0x14035f6b0)"),
    (0x77130004, 2, "event flag set to the value (0x14035f660)"),
    (0x77140002, 2, "book knowledge level (0x140369560)"),
    (0x77140005, 2, "a knowledge level, table 0x14107a8a0 (0x140369460)"),
    (0x77160002, 1, "the command passed to the event listeners (0x140296a60), numbers 101 to 210"),
];

pub fn parse(d: &[u8]) -> Result<Vec<Vec<Vec<u32>>>, Error> {
    let mut p = 0usize;
    let mut next = || -> Result<u32, Error> {
        let v = d.get(p..p + 4).ok_or(Error::Corrupt("event process data ends early"))?;
        p += 4;
        Ok(u32::from_le_bytes(v.try_into().unwrap()))
    };
    let n = next()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let g = next()?;
        let mut groups = Vec::new();
        for _ in 0..g {
            let m = next()?;
            groups.push((0..m).map(|_| next()).collect::<Result<Vec<_>, _>>()?);
        }
        out.push(groups);
    }
    Ok(out)
}

/// Splits a group into (word, operands), stopping at `ECT_END`.
pub fn commands(group: &[u32]) -> Result<Vec<(u32, &[u32])>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < group.len() && group[i] != 0x7f00_0000 {
        let n = CODES.iter().find(|c| c.0 == group[i]).ok_or(Error::Corrupt("unknown event process code"))?.1;
        let ops = group.get(i + 1..i + 1 + n).ok_or(Error::Corrupt("event process command cut short"))?;
        out.push((group[i], ops));
        i += 1 + n;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits() {
        let g = [0x77110001, 1, 0x77130004, 0x18, 2, 0x77120001, 0x1fd, 1, 0x7f000000];
        let c = super::commands(&g).unwrap();
        assert_eq!(c.iter().map(|c| c.0).collect::<Vec<_>>(), [0x77110001, 0x77130004, 0x77120001]);
        assert_eq!(c[1].1, [0x18, 2]);
        let mut d = Vec::new();
        for v in [1u32, 1, 1, 0x7f000000] {
            d.extend(v.to_le_bytes());
        }
        assert_eq!(super::parse(&d).unwrap(), vec![vec![vec![0x7f000000]]]);
    }

    /// The real file reads to its end, every group splits by `CODES` and ends with `ECT_END`
    /// (`BARREL_ROOT` = the game's folder; not in the repository).
    #[test]
    #[ignore]
    fn the_real_file_splits() {
        let Ok(root) = std::env::var("BARREL_ROOT") else { return };
        let d = std::fs::read(std::path::Path::new(&root).join("Event/eventprocess/EventProcessDataAll.epd")).unwrap();
        let events = super::parse(&d).unwrap();
        assert_eq!(events.len(), 1165);
        let mut n = 4;
        for e in &events {
            n += 4;
            for g in e {
                n += 4 * (1 + g.len());
                let words: usize = super::commands(g).unwrap().iter().map(|c| 1 + c.1.len()).sum();
                assert_eq!((words + 1, g.last()), (g.len(), Some(&0x7f00_0000)));
            }
        }
        assert_eq!(n, d.len());
    }
}
