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

/// A save file for `state`, coded as the game codes one (the inverse of [`decode`]): the 0x100
/// plain bytes of `header` (a save's own, what the load screen shows), then the state with its
/// trailing zero bytes dropped, substituted, gamma-packed and sealed with the game's keys.
/// `seed` stands for the game's own (any value reads back). For answer keys only: Barrel never
/// writes the original's saves; this writes copies.
pub fn encode(header: &[u8], state: &[u8], seed: u32) -> Vec<u8> {
    use barrel_formats::save_code::{gamma, seal, substitute};
    let len = state.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let mut file = header[..HEADER].to_vec();
    file.extend(seal(&gamma(&substitute(&state[..len])), seed, &KEYS));
    file
}

/// Marks the story events numbered `done` (their places in the `EVENT_*` enum, as
/// [`events_done`] gives them) as played in a decoded state.
pub fn set_events_done(state: &mut [u8], done: &[usize]) -> Result<(), Error> {
    let d = find(state, &["event", "execFlag"]).ok_or(Error::Corrupt("save: no event flags"))?;
    let at = d.as_ptr() as usize - state.as_ptr() as usize;
    let count = u32::from_be_bytes(d.get(..4).ok_or(Error::Corrupt("save: no event count"))?.try_into().unwrap()) as usize;
    for &i in done {
        if i >= count {
            return Err(Error::Corrupt("save: an event past the flags"));
        }
        state[at + 4 + i] = 1;
    }
    Ok(())
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

/// Where a save puts Ryza: the field map's number (`FIELDMAP_*`, `maps::field_maps`), her
/// turn (degrees about y, 0 toward +z) and her feet in game units (1 cm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub map: usize,
    pub turn: f32,
    pub at: [f32; 3],
}

/// The save's place (`global`: a big-endian count of 76 bytes, then big-endian words: a 0, the
/// map, the turn in degrees, her feet in metres, then words not read). The game puts her there on
/// loading (ryza-recomp `load-path`: an autosave's 5, -1.0, 325.2, 61.80, 303.34 m arrive as
/// (32520, 6180, 30334) facing 1 degree off +z).
pub fn place(state: &[u8]) -> Result<Place, Error> {
    let d = find(state, &["global"]).ok_or(Error::Corrupt("save: no global record"))?;
    let word = |i: usize| d.get(4 + i * 4..8 + i * 4).map(|w| u32::from_be_bytes(w.try_into().unwrap())).ok_or(Error::Corrupt("save: global cut short"));
    let metres = |i: usize| word(i).map(|w| f32::from_bits(w) * 100.0);
    Ok(Place { map: word(1)? as usize, turn: f32::from_bits(word(2)?), at: [metres(3)?, metres(4)?, metres(5)?] })
}

/// The save's clock word (`time`: a big-endian count of 4 bytes, then one big-endian word).
pub fn clock(state: &[u8]) -> Option<u32> {
    let d = find(state, &["time"])?;
    d.get(4..8).map(|w| u32::from_be_bytes(w.try_into().unwrap()))
}

// STAND-IN: The hour the clock word shows is read off one capture (an autosave's 493 shows 10 AM on the HUD); a word of 50 to the hour fits that one point, the game's own rule is not read.
/// The hour (0 to 23) a clock word shows.
pub fn hour_of(word: u32) -> u32 {
    ((word as f32 / 50.0).round() as u32) % 24
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

/// A party member as the save keeps them (`party`: a version record, a big-endian count and that many
/// `Party` records of 942 bytes; the records after them are the squad lists).
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    /// The character's number (0 Ryza, 1 Klaudia, 2 Lent, 3 Tao ...), the party tables' column.
    pub id: u8,
    pub level: u8,
    /// The four worn items: weapon, armour, two accessories.
    pub worn: [Option<Worn>; 4],
}

/// An item unit of the save (`docs/formats.md`, "Ryza DX saves"): the worn items of a `Party` record, the container's and the tools'.
/// Little-endian: a serial (+0), the item's number in `itemData.xml` (+2; `0xffff` for none), its quality (+4), its level (+8),
/// four effects (+14: a number in the effects' strings, `0xffff` for none), three potentials (+22: a row of `item_potential.xml`,
/// 290 for none), their levels (+28) and the weapon's free slots (+45).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    pub item: u16,
    pub quality: u16,
    pub level: u8,
    pub effects: [u16; 4],
    pub potentials: [u16; 3],
    pub potential_levels: [u8; 3],
    pub slots: u8,
}

impl Worn {
    /// A unit read from its 0x42 bytes; `None` for an empty place.
    pub fn read(u: &[u8]) -> Option<Worn> {
        let le = |at: usize| u.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let item = le(2)?;
        if item == 0xffff {
            return None;
        }
        Some(Worn {
            item,
            quality: le(4)?,
            level: *u.get(8)?,
            effects: [le(14)?, le(16)?, le(18)?, le(20)?],
            potentials: [le(22)?, le(24)?, le(26)?],
            potential_levels: [*u.get(28)?, *u.get(29)?, *u.get(30)?],
            slots: *u.get(45)?,
        })
    }
}

/// The first of a `Party` record's four item units, and a unit's size in bytes (`Worn`).
const WORN_AT: usize = 0x190;
pub const UNIT_SIZE: usize = 0x42;
const WORN_SIZE: usize = UNIT_SIZE;

/// The container's items (`item` / `m_unitContainer`: a big-endian byte count, then a unit each, an empty place's item `0xffff`):
/// the filled places in order, and the number of places.
pub fn container(state: &[u8]) -> Option<(Vec<Worn>, usize)> {
    units(state, &["item", "m_unitContainer"])
}

/// The gathering tools (`explore_equip` / `explore_equip_item`: the same list, 15 places).
pub fn tools(state: &[u8]) -> Option<(Vec<Worn>, usize)> {
    units(state, &["explore_equip", "explore_equip_item"])
}

/// The units of the list at `path`.
fn units(state: &[u8], path: &[&str]) -> Option<(Vec<Worn>, usize)> {
    let d = find(state, path)?;
    let bytes = u32::from_be_bytes(d.get(..4)?.try_into().ok()?) as usize;
    let list = d.get(4..4 + bytes)?;
    let places = list.chunks_exact(UNIT_SIZE).count();
    Some((list.chunks_exact(UNIT_SIZE).filter_map(Worn::read).collect(), places))
}

/// The library's levels of one list (`knowledge` / `item`, `eff`, ...: a big-endian byte count, then an i32 an entry; `docs/formats.md`, "Knowledge").
pub fn knowledge(state: &[u8], list: &str) -> Option<Vec<i32>> {
    let d = find(state, &["knowledge", list])?;
    let bytes = u32::from_be_bytes(d.get(..4)?.try_into().ok()?) as usize;
    Some(d.get(4..4 + bytes)?.chunks_exact(4).map(|b| i32::from_be_bytes(b.try_into().unwrap())).collect())
}

/// The save's members in its own order (all of its `party` records, in the party or not).
pub fn members(state: &[u8]) -> Option<Vec<Member>> {
    let d = find(state, &["party"])?;
    let version = u32::from_le_bytes(d.get(..4)?.try_into().ok()?) as usize;
    let count = u32::from_be_bytes(d.get(version..version + 4)?.try_into().ok()?) as usize;
    let mut rest = d.get(version + 4..)?;
    let mut out = Vec::new();
    for _ in 0..count {
        let size = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
        let whole = rest.get(..size)?;
        let end = whole[4..].iter().position(|&b| b == 0)? + 4;
        let c = &whole[end + 1..];
        let word = |i: usize| c.get(i * 4..i * 4 + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()));
        let mut worn = [None; 4];
        for (k, w) in worn.iter_mut().enumerate() {
            let at = WORN_AT + k * WORN_SIZE;
            *w = Worn::read(c.get(at..at + UNIT_SIZE)?);
        }
        out.push(Member { id: word(0)? as u8, level: word(1)? as u8, worn });
        rest = &rest[size..];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `global` as the autosave of ryza-recomp's `load-path` has it.
    #[test]
    fn the_clock_word_gives_the_hud_hour() {
        assert_eq!(hour_of(493), 10);
        assert_eq!(hour_of(1250), 1);
    }

    #[test]
    fn place_reads_global() {
        let words = [0, 5, (-1.0f32).to_bits(), 325.2f32.to_bits(), 61.8001f32.to_bits(), 303.3401f32.to_bits()];
        let mut body = 76u32.to_be_bytes().to_vec();
        body.extend(words.iter().flat_map(|w| w.to_be_bytes()));
        body.resize(4 + 76, 0);
        let mut state = vec![0u8; 0x20];
        state.extend(rec("global", &body));
        let p = place(&state).unwrap();
        assert_eq!((p.map, p.turn), (5, -1.0));
        assert!((p.at[0] - 32520.0).abs() < 0.01 && (p.at[1] - 6180.01).abs() < 0.01 && (p.at[2] - 30334.01).abs() < 0.01);
    }

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
            // ... and the party: Ryza (0) at level 3 wears the first staff (row 406) and the first armour (456).
            let party = members(&state).unwrap();
            assert_eq!((party[0].id, party[0].level), (0, 3));
            assert_eq!(party[0].worn[0].map(|w| (w.item, w.quality)), Some((406, 30)));
            assert_eq!(party[0].worn[1].map(|w| w.item), Some(456));
            assert_eq!(party[0].worn[2], None);
        }
    }

    /// A party record's layout built by hand: the version record, the count and a record with one worn item.
    #[test]
    fn reads_a_party_record() {
        let mut c = vec![0u8; 942];
        c[4..8].copy_from_slice(&5u32.to_be_bytes());
        c[WORN_AT..WORN_AT + 6].copy_from_slice(&[0, 0, 0x96, 1, 30, 0]);
        for k in 1..4 {
            c[WORN_AT + k * WORN_SIZE + 2..WORN_AT + k * WORN_SIZE + 4].copy_from_slice(&[0xff, 0xff]);
        }
        let mut party = rec("version", &[0, 1]);
        party.extend_from_slice(&1u32.to_be_bytes());
        party.extend(rec("Party", &c));
        let state = [vec![0; 0x20], rec("party", &party)].concat();
        let m = members(&state).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].id, m[0].level), (0, 5));
        assert_eq!(m[0].worn.map(|w| w.map(|w| (w.item, w.quality))), [Some((406, 30)), None, None, None]);
    }

    /// An item unit of 0x42 bytes as the container's first (the capture's Quartz Necklace: level 26, quality 100, three effects).
    fn necklace() -> Vec<u8> {
        let mut u = vec![0u8; UNIT_SIZE];
        u[0..2].copy_from_slice(&74u16.to_le_bytes());
        u[2..4].copy_from_slice(&486u16.to_le_bytes());
        u[4] = 100;
        u[8] = 26;
        for (k, e) in [1175u16, 1276, 1279, 0xffff].into_iter().enumerate() {
            u[14 + 2 * k..16 + 2 * k].copy_from_slice(&e.to_le_bytes());
        }
        for k in 0..3 {
            u[22 + 2 * k..24 + 2 * k].copy_from_slice(&290u16.to_le_bytes());
        }
        u
    }

    #[test]
    fn a_unit_reads_its_item_quality_level_effects_and_potentials() {
        let w = Worn::read(&necklace()).unwrap();
        assert_eq!((w.item, w.quality, w.level, w.effects, w.potentials), (486, 100, 26, [1175, 1276, 1279, 0xffff], [290; 3]));
        let mut empty = necklace();
        empty[2..4].copy_from_slice(&[0xff, 0xff]);
        assert!(Worn::read(&empty).is_none());
    }

    #[test]
    fn the_container_lists_its_filled_places_and_counts_all() {
        // Three places, the middle one empty.
        let mut empty = necklace();
        empty[2..4].copy_from_slice(&[0xff, 0xff]);
        let list = [necklace(), empty, necklace()].concat();
        let mut c = (list.len() as u32).to_be_bytes().to_vec();
        c.extend(list);
        let item = rec("m_unitContainer", &c);
        let state = [vec![0; 0x20], rec("item", &item)].concat();
        let (units, places) = container(&state).unwrap();
        assert_eq!((units.len(), places), (2, 3));
        assert!(tools(&state).is_none());
    }
}
