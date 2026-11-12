//! When events play (`Event/eventCheck/EventCheckDataAll.ecd`, read by `0x1400548f0` into a map
//! at `0x140df9a10` keyed by event number): an i32 count (1,165, one per `EVENT_*`), then per
//! event 14 i32 (kept at the record's +0x28 to +0x5c), a group count and, per group, a word count
//! and that many words (kept per group at the record's +0x60). The groups hold condition code:
//! words `0x700000NN` followed by their operands (`0x7000001f 15 0x70000020 31` ...).

use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventCheck {
    /// The 14 words (word 1: 10, 100, 500 ... by the events, a priority presumably).
    pub words: [i32; 14],
    pub groups: Vec<Vec<i32>>,
}

pub fn parse(d: &[u8]) -> Result<Vec<EventCheck>, Error> {
    let mut p = 0usize;
    let mut next = || -> Result<i32, Error> {
        let v = d.get(p..p + 4).ok_or(Error::Corrupt("event check data ends early"))?;
        p += 4;
        Ok(i32::from_le_bytes(v.try_into().unwrap()))
    };
    let n = next()?;
    let mut out = Vec::new();
    for _ in 0..n.max(0) {
        let mut words = [0i32; 14];
        for w in &mut words {
            *w = next()?;
        }
        let g = next()?;
        let mut groups = Vec::new();
        for _ in 0..g.max(0) {
            let m = next()?;
            groups.push((0..m.max(0)).map(|_| next()).collect::<Result<Vec<_>, _>>()?);
        }
        out.push(EventCheck { words, groups });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_groups() {
        let mut d = Vec::new();
        let mut put = |v: i32| d.extend(v.to_le_bytes());
        put(1);
        for v in [0, 500, 0, 0, 0, 0, 0, 0, -1, -1, -1, -1, -1, -1] {
            put(v);
        }
        put(2);
        put(2);
        put(0x7000001f);
        put(15);
        put(0);
        let e = super::parse(&d).unwrap();
        assert_eq!((e[0].words[1], e[0].groups.clone()), (500, vec![vec![0x7000001f, 15], vec![]]));
    }
}
