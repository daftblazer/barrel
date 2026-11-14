//! When events play (`Event/eventCheck/EventCheckDataAll.ecd`, read by `0x1400548f0` into a map
//! at `0x140df9a10` keyed by event number): an i32 count (1,165, one per `EVENT_*`), then per
//! event 14 i32 (kept at the record's +0x28 to +0x5c), a group count and, per group, a word count
//! and that many words (kept per group at the record's +0x60). The groups hold condition code:
//! codes `0x70000000 + n` (`EVENT_CONDITION_*` up to 0x7d, `EVENT_TRIGGER_*` from 0x7e, see
//! `event_check_codes`) each followed by its operands. Group 0 holds conditions only.

use barrel_formats::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventCheck {
    /// The 14 words (word 1: 10, 100, 500 ... by the events, a priority presumably; words 8
    /// to 10 and 11 to 13 the first and last date it may start on, year, month and day, -1 for
    /// any: event 62 from year 1, month 7, day 15 to year 1, month 8, day 30; see
    /// `game::event_start`). Word 13 lands at the record's +0x5c, which is not the event
    /// runner's +0x5c that `u7` jumps from (the runner is another object).
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

/// One condition or trigger of a group: the code's index past `0x70000000` (a name in
/// `event_check_codes::CODES`) and its operands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command<'a> {
    pub code: usize,
    pub operands: &'a [i32],
}

impl Command<'_> {
    pub fn name(&self) -> &'static str {
        crate::data::event_check_codes::CODES[self.code].2
    }
    /// The trigger group (`EVENT_CHECK_TRIGGER_GROUP_*`) of a trigger; None for a condition.
    pub fn trigger(&self) -> Option<u8> {
        u8::try_from(crate::data::event_check_codes::CODES[self.code].1).ok()
    }
}

/// Splits a group into its commands by the codes' word counts.
pub fn commands(group: &[i32]) -> Result<Vec<Command<'_>>, Error> {
    let codes = crate::data::event_check_codes::CODES;
    let mut out = Vec::new();
    let mut i = 0;
    while i < group.len() {
        let code = (group[i] as u32).wrapping_sub(0x7000_0000) as usize;
        let n = codes.get(code).ok_or(Error::Corrupt("unknown event check code"))?.0 as usize;
        let operands = group.get(i + 1..i + n).ok_or(Error::Corrupt("event check command cut short"))?;
        out.push(Command { code, operands });
        i += n;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_commands() {
        let g = [0x7000001f, 15, 0x70000020, 31, 0x70000027, 4, 1];
        let c = super::commands(&g).unwrap();
        let names: Vec<_> = c.iter().map(|c| c.name()).collect();
        assert_eq!(names, ["CONDITION_EVENT_OCCURRENCE", "CONDITION_EVENT_NOT_OCCURRENCE", "CONDITION_EVENTFLAG_MT"]);
        assert_eq!((c[2].operands, c[2].trigger()), (&[4, 1][..], None));
        let t = super::commands(&[0x7000007f, 0, 4, 0, 5]).unwrap();
        assert_eq!((t[0].name(), t[0].trigger()), ("TRIGGER_MAP_FROM_TO_AFTER", Some(1)));
        assert!(super::commands(&[0x7000001f]).is_err());
    }

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

    /// The real file reads to its end and every group splits by the code table (`BARREL_ROOT` =
    /// the game's folder; not in the repository).
    #[test]
    #[ignore]
    fn the_real_file_splits() {
        let Ok(root) = std::env::var("BARREL_ROOT") else { return };
        let d = std::fs::read(std::path::Path::new(&root).join("Event/eventCheck/EventCheckDataAll.ecd")).unwrap();
        let events = super::parse(&d).unwrap();
        assert_eq!(events.len(), 1165);
        let mut n = 4;
        for e in &events {
            n += 4 * 15;
            for (k, g) in e.groups.iter().enumerate() {
                n += 4 * (1 + g.len());
                let cmds = super::commands(g).unwrap();
                assert!(k > 0 || cmds.iter().all(|c| c.trigger().is_none()));
            }
        }
        assert_eq!(n, d.len());
    }
}
