//! Ryza DX saves (`<local app data>/KoeiTecmo/Atelier Ryza DX/<user>/GameData<NN>/data.dat`,
//! `AutoSaveData`, `SystemData`): 0x100 plain bytes the load screen shows, then the game's
//! state in the Gust DX saves' coding with this game's keys. The state is a 0x20-byte header
//! and named records, some holding records. Docs: docs/formats.md, "Ryza DX saves".

use barrel_formats::save_code::{self, ungamma, unsubstitute, Keys};
use barrel_formats::Error;

pub const HEADER: usize = 0x100;

/// Ryza DX's keys for the saves' coding (the game's `0x140477630`).
pub const KEYS: Keys = Keys { words: 0xd5c1, modulus: 0xdb2, threshold: 0x6d9, runs: [0x9f73, 0xaee3, 0x394d] };

/// The first four bytes of a decoded state.
const MAGIC: [u8; 4] = [0x01, 0x33, 0x79, 0xc9];

/// The state of a save file: its body opened with the game's keys, then unpacked.
pub fn decode(file: &[u8]) -> Result<Vec<u8>, Error> {
    let body = file.get(HEADER..).ok_or(Error::Corrupt("save: no body"))?;
    let state = unsubstitute(&ungamma(&save_code::open(body, &KEYS)?)?)?;
    if !state.starts_with(&MAGIC) {
        return Err(Error::Corrupt("save: not a Ryza DX state"));
    }
    Ok(state)
}

/// The record `name` among the records of `d` (each a little-endian u32 size counting itself,
/// a NUL-terminated name and its contents): its contents.
fn record<'a>(mut d: &'a [u8], name: &str) -> Option<&'a [u8]> {
    while d.len() >= 5 {
        let size = u32::from_le_bytes(d[..4].try_into().unwrap()) as usize;
        let whole = d.get(..size).filter(|_| size >= 5)?;
        let end = whole[4..].iter().position(|&b| b == 0)? + 4;
        if &whole[4..end] == name.as_bytes() {
            return Some(&whole[end + 1..]);
        }
        d = &d[size..];
    }
    None
}

/// The contents of the record at `path` in a decoded state (`["event", "execFlag"]`).
pub fn find<'a>(state: &'a [u8], path: &[&str]) -> Option<&'a [u8]> {
    path.iter().try_fold(state.get(0x20..)?, |d, name| record(d, name))
}

/// The numbers of the story events that have played (`event` / `execFlag`: a big-endian
/// count and a byte an event, in the order of the game's `EVENT_*` enum).
pub fn events_done(state: &[u8]) -> Result<Vec<usize>, Error> {
    let d = find(state, &["event", "execFlag"]).ok_or(Error::Corrupt("save: no event flags"))?;
    let count = u32::from_be_bytes(d.get(..4).ok_or(Error::Corrupt("save: no event count"))?.try_into().unwrap()) as usize;
    let flags = d.get(4..4 + count).ok_or(Error::Corrupt("save: event flags cut short"))?;
    Ok(flags.iter().enumerate().filter(|&(_, &f)| f != 0).map(|(i, _)| i).collect())
}

/// The story a save is at: its played events named by the executable's `EVENT_*` enum.
pub fn story(state: &[u8], pe: &barrel_formats::pe::Pe) -> Result<crate::story::Story, Error> {
    const FIRST: &str = "EVENT_AA01_010";
    let (names, first) = pe.enum_names(FIRST).ok_or(Error::Corrupt("the executable does not name the story's events"))?;
    let mut story = crate::story::Story::default();
    for i in events_done(state)? {
        story.finish(names.get(first + i).ok_or(Error::Corrupt("save: an event past the game's list"))?);
    }
    Ok(story)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, contents: &[u8]) -> Vec<u8> {
        let mut r = ((4 + name.len() + 1 + contents.len()) as u32).to_le_bytes().to_vec();
        r.extend_from_slice(name.as_bytes());
        r.push(0);
        r.extend_from_slice(contents);
        r
    }

    #[test]
    fn finds_the_event_flags() {
        let mut flags = 5u32.to_be_bytes().to_vec();
        flags.extend_from_slice(&[0, 1, 0, 0, 1]);
        let event = [rec("short", &[0; 6]), rec("execFlag", &flags)].concat();
        let mut state = MAGIC.to_vec();
        state.resize(0x20, 0);
        state.extend(rec("info", &[1, 2, 3]));
        state.extend(rec("event", &event));
        assert_eq!(events_done(&state).unwrap(), [1, 4]);
        assert!(find(&state, &["event", "date"]).is_none());
        assert!(events_done(&state[..0x20]).is_err());
    }

    /// A real save decoded (`BARREL_RYZA_SAVE` names a `data.dat`; not in the repository).
    #[test]
    fn decodes_a_real_save() {
        let Some(f) = std::env::var_os("BARREL_RYZA_SAVE") else { return };
        let state = decode(&std::fs::read(f).unwrap()).unwrap();
        // A game save has the story's events; the settings file has none.
        if find(&state, &["event"]).is_some() {
            assert!(!events_done(&state).unwrap().is_empty());
        }
    }
}
