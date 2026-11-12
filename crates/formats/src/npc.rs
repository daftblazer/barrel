//! Who stands where in town (English Escha executable): `nspFM::clsFMDataNpc` (built by
//! `0x140236db0` into `[0x140dfc1f8]`, filled by `0x14023e440`) copies three compiled tables:
//! 190 conditions of 16 bytes from [`CONDITIONS_EN`] (to +0x20; condition `i` is read at
//! +0x10 + 16 × `i`, so it is compiled pair `i - 1`), 1,494 entries of 0x40 bytes from
//! `0x1408ab750` (to +0xbf0, not read) and [`PLACES`] 841 placements of 0x98 bytes from
//! [`PLACES_EN`] (to +0xc08). The field load (`0x140071d00 (manager, map)`) walks the placements
//! of the map: one passes when its first condition list is empty or has a condition that holds,
//! and none of the other four has one; a condition (`0x14035f320`) holds when every event of
//! its first list has been seen and none of its second. A passing placement adds its character
//! (`0x14026f700`, a `nspFM::clsFMNpc` whose number, +0x78, is the `Chara` row) unless it is
//! there already; numbers 318 to 322 instead pick a random companion from the party list.

use crate::pe::Pe;

pub const PLACES_EN: u64 = 0x1_408c_2cd0;
pub const PLACES: usize = 841;
pub const CONDITIONS_EN: u64 = 0x1_408a_ab70;
pub const CONDITIONS: usize = 190;
const PLACE: usize = 0x98;

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub map: i32,
    /// Condition numbers: the first list needs one that holds (or is empty), the other four
    /// none that does.
    pub conditions: [Vec<i32>; 5],
    /// The `Chara.xml` row (318 to 322: a random companion).
    pub chara: i32,
    /// +0x34, handed to the builder (not read yet).
    pub value: i32,
    /// +0x38 to the end, not read yet (+0x50 to +0x5c look like a position and an angle).
    pub rest: Vec<u8>,
}

/// A -1-ended list of up to 127 words at `va` (how `0x140071d00` walks them).
fn list(pe: &Pe, va: u64) -> Vec<i32> {
    let Some(mut o) = pe.offset(va) else { return Vec::new() };
    let mut out = Vec::new();
    while out.len() < 0x7f {
        let Some(b) = pe.data.get(o..o + 4) else { break };
        let v = i32::from_le_bytes(b.try_into().unwrap());
        if v == -1 {
            break;
        }
        out.push(v);
        o += 4;
    }
    out
}

pub fn places(pe: &Pe) -> Option<Vec<Place>> {
    let at = pe.offset(PLACES_EN)?;
    (0..PLACES)
        .map(|k| {
            let r = pe.data.get(at + PLACE * k..at + PLACE * (k + 1))?;
            let w = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            let q = |o: usize| u64::from_le_bytes(r[o..o + 8].try_into().unwrap());
            Some(Place {
                map: w(0),
                conditions: std::array::from_fn(|i| list(pe, q(8 + 8 * i))),
                chara: w(0x30),
                value: w(0x34),
                rest: r[0x38..].to_vec(),
            })
        })
        .collect()
}

/// Condition `i` (from 1): the events that must have been seen and the events that must not.
pub fn condition(pe: &Pe, i: i32) -> Option<(Vec<i32>, Vec<i32>)> {
    let k = usize::try_from(i).ok()?.checked_sub(1).filter(|&k| k < CONDITIONS)?;
    let at = pe.offset(CONDITIONS_EN)? + 16 * k;
    let q = |o: usize| pe.data.get(o..o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
    Some((list(pe, q(at)?), list(pe, q(at + 8)?)))
}

