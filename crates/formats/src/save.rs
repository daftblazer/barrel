//! Escha & Logy DX saves (`Documents/KoeiTecmo/Atelier Escha and Logy DX/SAVEDATA/
//! GAMEDATA<NN>.pcsave`, the game's `ROOT:savedata/GAMEDATA%02d`).
//!
//! ```text
//! 0x000  0x100 plain bytes, the summary the load screen shows:
//!        +0x28.. i32s (what each is is not read yet), +0x74.. -1 padding,
//!        +0xa4 the time it was saved, a Windows SYSTEMTIME (8 × u16: year, month,
//!        day of the week, day, hour, minute, second, milliseconds)
//! 0x100  the game state, coded ([`decode`])
//! ```
//!
//! `SYSDATA.pcsave` (the settings) is coded from its first byte. The coding
//! (`0x1400219a0`, decoding with its last argument 0) is undone by [`decode`]; see
//! `docs/formats.md`, "Saves".

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

/// The save stream (`state = state × key + 0x2f09`, its bits 16 and up used).
struct Stream {
    a: u32,
    b: u32,
}

impl Stream {
    fn next(&mut self) -> u32 {
        self.b = self.a.wrapping_mul(self.b).wrapping_add(0x2f09);
        self.b >> 16
    }
}

const SEED: u32 = 0x3b9a73c9;

/// Swaps bit pairs drawn from the stream over each block of up to 0x800 bytes
/// (`0x14001fe10`): the bit indices 0..8n are drawn out one at a time, entry
/// `(s & 0x7fff) % left` of those left, and each two drawn in a row trade places. Its own
/// inverse.
fn swap_bits(s: &mut Stream, data: &mut [u8], key: u32) {
    s.b = key;
    for block in data.chunks_mut(0x800) {
        let mut left: Vec<u16> = (0..(block.len() * 8) as u32).map(|i| i as u16).collect();
        let mut drawn = Vec::with_capacity(left.len());
        while !left.is_empty() {
            let j = (s.next() & 0x7fff) as usize % left.len();
            drawn.push(left.remove(j) as usize);
        }
        for pair in drawn.chunks_exact(2) {
            let (a, b) = (pair[0], pair[1]);
            let bit = |d: &[u8], i: usize| (d[i >> 3] >> (i & 7)) & 1;
            let (va, vb) = (bit(block, a), bit(block, b));
            block[a >> 3] = (block[a >> 3] & !(1 << (a & 7))) | (vb << (a & 7));
            block[b >> 3] = (block[b >> 3] & !(1 << (b & 7))) | (va << (b & 7));
        }
    }
}

/// Undoes the bytes' Exp-Golomb coding (`0x140020a80`): a big-endian u32 count, then for
/// each byte, read from the top bit down, `k` zeros and the `k + 1` bits from the next one
/// on give `v`, the byte `v - 1` (a lone one is 0; eight zeros are 0xff).
fn ungamma(d: &[u8]) -> Result<Vec<u8>, Error> {
    let size = u32::from_be_bytes(d.get(..4).ok_or(Error::Corrupt("save: no size"))?.try_into().unwrap()) as usize;
    if size > 1 << 24 {
        return Err(Error::TooLarge);
    }
    let (mut pos, mut bit) = (4usize, 7u32);
    let peek = |pos: usize, bit: u32| d.get(pos).map(|b| (b >> bit) & 1).ok_or(Error::Corrupt("save: bits run out"));
    let mut out = Vec::with_capacity(size);
    let advance = |pos: &mut usize, bit: &mut u32| {
        if *bit == 0 {
            *bit = 7;
            *pos += 1;
        } else {
            *bit -= 1;
        }
    };
    while out.len() < size {
        if peek(pos, bit)? == 1 {
            advance(&mut pos, &mut bit);
            out.push(0);
            continue;
        }
        let mut k = 0;
        loop {
            advance(&mut pos, &mut bit);
            k += 1;
            if k == 8 || peek(pos, bit)? == 1 {
                break;
            }
        }
        if k == 8 {
            out.push(0xff);
            continue;
        }
        let mut v = 0u32;
        for _ in 0..=k {
            v = (v << 1) | peek(pos, bit)? as u32;
            advance(&mut pos, &mut bit);
        }
        out.push((v - 1) as u8);
    }
    Ok(out)
}

/// Undoes the byte substitution (`0x140020740`): a count `n` (0 for 256), `n` table bytes, a
/// big-endian u32 length, then that many bytes, each an index into the table.
fn unsubstitute(d: &[u8]) -> Result<Vec<u8>, Error> {
    let n = match *d.first().ok_or(Error::Corrupt("save: no table"))? {
        0 => 256,
        n => n as usize,
    };
    let table = d.get(1..1 + n).ok_or(Error::Corrupt("save: table cut short"))?;
    let len = u32::from_be_bytes(d.get(1 + n..5 + n).ok_or(Error::Corrupt("save: no length"))?.try_into().unwrap()) as usize;
    let body = d.get(5 + n..5 + n + len).ok_or(Error::Corrupt("save: body cut short"))?;
    body.iter().map(|&b| table.get(b as usize).copied().ok_or(Error::Corrupt("save: index past the table"))).collect()
}

/// Decodes a coded save body (a `GAMEDATA` file after its header, or all of `SYSDATA`):
/// - trailing zero bytes dropped, the rest padded to 16 (`P` bytes);
/// - the last `min(P, 0x800)` bytes' bit pairs swapped back (stream key `0x3745d`);
/// - each big-endian u16 word: `r` drawn (`& 0x7fff`); when `r % 0xec2 ≥ 0x761` the word is
///   xored with the next draw; then `r` taken off (stream key `0xd727`);
/// - the trailer: byte `P - 0x10` is 0xff, `P - 0xc` the xor and `P - 8` the sum of the data's
///   big-endian u32s, `P - 4` a seed (big-endian) added to the stream's multiplier;
/// - the data's bytes xored with the stream in runs, the three keys 0xa22f, 0xa129, 0x394d in
///   turn for 7, 13 and 23 bytes plus the round number, each key carried on to its next run;
/// - the sums checked, then [`ungamma`] and [`unsubstitute`].
pub fn decode(coded: &[u8]) -> Result<Vec<u8>, Error> {
    let len = coded.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let p = (len + 15) & !15;
    if p < 0x20 {
        return Err(Error::Corrupt("save: too short"));
    }
    let mut d = coded[..len].to_vec();
    d.resize(p, 0);
    let mut s = Stream { a: SEED, b: 0 };
    let m = p.min(0x800);
    swap_bits(&mut s, &mut d[p - m..], 0x3745d);
    s.b = 0xd727;
    for w in d.chunks_exact_mut(2) {
        let r = s.next() & 0x7fff;
        let mut v = u16::from_be_bytes([w[0], w[1]]) as u32;
        if r % 0xec2 >= 0x761 {
            v ^= s.next() & 0x7fff;
        }
        w.copy_from_slice(&(v.wrapping_sub(r) as u16).to_be_bytes());
    }
    if d[p - 0x10] != 0xff {
        return Err(Error::Corrupt("save: no end marker"));
    }
    let be = |o: usize| u32::from_be_bytes(d[o..o + 4].try_into().unwrap());
    let (xor_want, sum_want, seed) = (be(p - 0xc), be(p - 8), be(p - 4));
    s.a = seed.wrapping_add(SEED);
    let mut keys = [0xa22fu32, 0xa129, 0x394d];
    let data_len = p - 0x10;
    let (mut at, mut round, mut k) = (0usize, 0usize, 0usize);
    while at < data_len {
        s.b = keys[k];
        for _ in 0..round + [7, 13, 23][k] {
            if at == data_len {
                break;
            }
            d[at] ^= s.next() as u8;
            at += 1;
        }
        keys[k] = s.b;
        k += 1;
        if k == 3 {
            k = 0;
            round += 1;
        }
    }
    let (mut sum, mut xor) = (0u32, 0u32);
    for c in d[..data_len].chunks_exact(4) {
        let v = u32::from_be_bytes(c.try_into().unwrap());
        sum = sum.wrapping_add(v);
        xor ^= v;
    }
    if (sum, xor) != (sum_want, xor_want) {
        return Err(Error::Corrupt("save: checksums differ"));
    }
    unsubstitute(&ungamma(&d[..data_len])?)
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

    #[test]
    fn gamma_and_table() {
        // 3 bytes: 0 ("1"), 2 ("011" → 3 - 1), 0xff (eight zeros); then the table step.
        let bits = "1" .to_string() + "011" + "00000000";
        let mut packed = vec![0, 0, 0, 3];
        let mut byte = 0u8;
        for (i, c) in bits.chars().enumerate() {
            byte |= ((c == '1') as u8) << (7 - i % 8);
            if i % 8 == 7 {
                packed.push(byte);
                byte = 0;
            }
        }
        packed.push(byte);
        assert_eq!(ungamma(&packed).unwrap(), [0, 2, 0xff]);
        assert_eq!(unsubstitute(&[2, b'x', b'y', 0, 0, 0, 3, 1, 0, 1]).unwrap(), b"yxy");
    }

    #[test]
    fn bit_swaps_undo_themselves() {
        let mut s = Stream { a: SEED, b: 0 };
        let original: Vec<u8> = (0..40u8).map(|i| i.wrapping_mul(37)).collect();
        let mut d = original.clone();
        swap_bits(&mut s, &mut d, 0x3745d);
        assert_ne!(d, original);
        swap_bits(&mut s, &mut d, 0x3745d);
        assert_eq!(d, original);
    }

    /// The settings file of a test run against its state read from the game's memory
    /// (`BARREL_SYSDATA` = the file, `BARREL_SYSDATA_PLAIN` = the state; not in the repository).
    #[test]
    #[ignore]
    fn decodes_a_real_settings_file() {
        let (Ok(f), Ok(p)) = (std::env::var("BARREL_SYSDATA"), std::env::var("BARREL_SYSDATA_PLAIN")) else { return };
        let out = decode(&std::fs::read(f).unwrap()).unwrap();
        assert_eq!(out[..], std::fs::read(p).unwrap()[..out.len()]);
    }
}
