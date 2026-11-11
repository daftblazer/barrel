//! Escha & Logy DX saves (`Documents/KoeiTecmo/Atelier Escha and Logy DX/SAVEDATA/
//! GAMEDATA<NN>.pcsave`, the game's `ROOT:savedata/GAMEDATA%02d`).
//!
//! ```text
//! 0x000  0x100 plain bytes, the summary the load screen shows:
//!        +0x28.. i32s (what each is is not read yet), +0x74.. -1 padding,
//!        +0xa4 the time it was saved, a Windows SYSTEMTIME (8 × u16: year, month,
//!        day of the week, day, hour, minute, second, milliseconds)
//! 0x100  the game state, encrypted (not read yet)
//! ```
//!
//! `SYSDATA.pcsave` (the settings) is encrypted from its first byte.

use crate::Error;

pub const HEADER: usize = 0x100;

/// When a save was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedAt {
    pub year: u16,
    pub month: u16,
    pub weekday: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
}

/// A save's plain header and its encrypted body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save<'a> {
    /// The header's i32 words from +0x28 (summary values).
    pub summary: Vec<i32>,
    pub saved_at: SavedAt,
    pub body: &'a [u8],
}

pub fn parse(d: &[u8]) -> Result<Save<'_>, Error> {
    if d.len() < HEADER {
        return Err(Error::Corrupt("save shorter than its header"));
    }
    let u16_at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let w: Vec<u16> = (0..8).map(|k| u16_at(0xa4 + 2 * k)).collect();
    let saved_at = SavedAt { year: w[0], month: w[1], weekday: w[2], day: w[3], hour: w[4], minute: w[5], second: w[6], millisecond: w[7] };
    if !(1..=12).contains(&saved_at.month) || !(1..=31).contains(&saved_at.day) {
        return Err(Error::NotThisFormat);
    }
    let summary = (0x28..0x74).step_by(4).map(|o| i32::from_le_bytes(d[o..o + 4].try_into().unwrap())).collect();
    Ok(Save { summary, saved_at, body: &d[HEADER..] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header() {
        let mut d = vec![0u8; HEADER + 4];
        d[0x28] = 5;
        for (k, v) in [2026u16, 8, 6, 1, 18, 8, 31, 579].iter().enumerate() {
            d[0xa4 + 2 * k..0xa6 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
        let s = parse(&d).unwrap();
        assert_eq!(s.summary[0], 5);
        assert_eq!((s.saved_at.year, s.saved_at.month, s.saved_at.day, s.saved_at.hour), (2026, 8, 1, 18));
        assert_eq!(s.body.len(), 4);
    }
}
