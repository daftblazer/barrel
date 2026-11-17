//! The coding of the Gust DX games' saves (Escha & Logy DX, Ryza DX): one scheme, each game
//! with its own keys ([`Keys`]). See `docs/formats.md`, "Saves".

use crate::Error;

/// A game's keys for the coding: the word pass's stream key, its modulus and threshold (a
/// word is xored when `r % modulus >= threshold`), and the three keys of the byte runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keys {
    pub words: u32,
    pub modulus: u32,
    pub threshold: u32,
    pub runs: [u32; 3],
}

/// The save stream (`state = state × key + 0x2f09`, its bits 16 and up used).
pub struct Stream {
    pub a: u32,
    pub b: u32,
}

impl Stream {
    pub fn next(&mut self) -> u32 {
        self.b = self.a.wrapping_mul(self.b).wrapping_add(0x2f09);
        self.b >> 16
    }
}

pub const SEED: u32 = 0x3b9a73c9;

/// Swaps bit pairs drawn from the stream over each block of up to 0x800 bytes
/// (`0x14001fe10`): the bit indices 0..8n are drawn out one at a time, entry
/// `(s & 0x7fff) % left` of those left, and each two drawn in a row trade places. Its own
/// inverse.
pub fn swap_bits(s: &mut Stream, data: &mut [u8], key: u32) {
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
pub fn ungamma(d: &[u8]) -> Result<Vec<u8>, Error> {
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
pub fn unsubstitute(d: &[u8]) -> Result<Vec<u8>, Error> {
    let n = match *d.first().ok_or(Error::Corrupt("save: no table"))? {
        0 => 256,
        n => n as usize,
    };
    let table = d.get(1..1 + n).ok_or(Error::Corrupt("save: table cut short"))?;
    let len = u32::from_be_bytes(d.get(1 + n..5 + n).ok_or(Error::Corrupt("save: no length"))?.try_into().unwrap()) as usize;
    let body = d.get(5 + n..5 + n + len).ok_or(Error::Corrupt("save: body cut short"))?;
    body.iter().map(|&b| table.get(b as usize).copied().ok_or(Error::Corrupt("save: index past the table"))).collect()
}

/// The data's bytes xored with the stream in runs: the three keys in turn for 7, 13 and 23
/// bytes plus the round number, each key carried on to its next run. Its own inverse.
fn runs(s: &mut Stream, d: &mut [u8], mut keys: [u32; 3]) {
    let (mut at, mut round, mut k) = (0usize, 0usize, 0usize);
    while at < d.len() {
        s.b = keys[k];
        for _ in 0..round + [7, 13, 23][k] {
            if at == d.len() {
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
}

fn sums(d: &[u8]) -> (u32, u32) {
    d.chunks_exact(4).fold((0u32, 0u32), |(sum, xor), c| {
        let v = u32::from_be_bytes(c.try_into().unwrap());
        (sum.wrapping_add(v), xor ^ v)
    })
}

/// Opens a coded body, giving the packed data inside:
/// - trailing zero bytes dropped, the rest padded to 16 (`P` bytes);
/// - the last `min(P, 0x800)` bytes' bit pairs swapped back (stream key `0x3745d`);
/// - each big-endian u16 word: `r` drawn (`& 0x7fff`); when `r % modulus ≥ threshold` the
///   word is xored with the next draw; then `r` taken off (stream key `words`);
/// - the trailer: byte `P - 0x10` is 0xff, `P - 0xc` the xor and `P - 8` the sum of the data's
///   big-endian u32s, `P - 4` a seed (big-endian) added to the stream's multiplier;
/// - the data's bytes xored with the stream in runs (`Keys::runs`), and the sums checked.
pub fn open(coded: &[u8], keys: &Keys) -> Result<Vec<u8>, Error> {
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
    s.b = keys.words;
    for w in d.chunks_exact_mut(2) {
        let r = s.next() & 0x7fff;
        let mut v = u16::from_be_bytes([w[0], w[1]]) as u32;
        if r % keys.modulus >= keys.threshold {
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
    d.truncate(p - 0x10);
    runs(&mut s, &mut d, keys.runs);
    if sums(&d) != (sum_want, xor_want) {
        return Err(Error::Corrupt("save: checksums differ"));
    }
    Ok(d)
}

/// Codes packed data as [`open`] reads it: padded to 16, xored in the seeded runs, the
/// 16-byte trailer added (0xff, then big-endian the xor and the sum of the data's big-endian
/// u32s and `seed`), the words' `r` added (then the xor where it applies) and the last
/// `min(P, 0x800)` bytes' bit pairs swapped. The data must not end in a zero byte.
pub fn seal(packed: &[u8], seed: u32, keys: &Keys) -> Vec<u8> {
    let mut d = packed.to_vec();
    d.resize((d.len() + 15) & !15, 0);
    let (sum, xor) = sums(&d);
    let mut s = Stream { a: seed.wrapping_add(SEED), b: 0 };
    runs(&mut s, &mut d, keys.runs);
    d.extend_from_slice(&[0xff, 0, 0, 0]);
    d.extend_from_slice(&xor.to_be_bytes());
    d.extend_from_slice(&sum.to_be_bytes());
    d.extend_from_slice(&seed.to_be_bytes());
    let p = d.len();
    let mut s = Stream { a: SEED, b: keys.words };
    for w in d.chunks_exact_mut(2) {
        let r = s.next() & 0x7fff;
        let mut v = (u16::from_be_bytes([w[0], w[1]]) as u32).wrapping_add(r) & 0xffff;
        if r % keys.modulus >= keys.threshold {
            v ^= s.next() & 0x7fff;
        }
        w.copy_from_slice(&(v as u16).to_be_bytes());
    }
    let m = p.min(0x800);
    swap_bits(&mut s, &mut d[p - m..], 0x3745d);
    d
}

/// Packs bytes as [`ungamma`] reads them: a big-endian u32 count, then each byte `b` as
/// `v = b + 1` in `floor(log2 v)` zeros and `v`'s bits from the top (0xff as eight zeros).
pub fn gamma(d: &[u8]) -> Vec<u8> {
    let mut out = (d.len() as u32).to_be_bytes().to_vec();
    let (mut byte, mut used) = (0u8, 0u32);
    let mut put = |bit: u32, out: &mut Vec<u8>| {
        byte |= (bit as u8) << (7 - used);
        used += 1;
        if used == 8 {
            out.push(byte);
            (byte, used) = (0, 0);
        }
    };
    for &b in d {
        if b == 0xff {
            for _ in 0..8 {
                put(0, &mut out);
            }
            continue;
        }
        let v = b as u32 + 1;
        let k = 31 - v.leading_zeros();
        for _ in 0..k {
            put(0, &mut out);
        }
        for i in (0..=k).rev() {
            put((v >> i) & 1, &mut out);
        }
    }
    if used > 0 {
        out.push(byte);
    }
    out
}

/// Writes bytes as [`unsubstitute`] reads them: a table of the distinct bytes, most frequent
/// first (so they get the shortest codes; the game's own order is not read), then each byte
/// as its place in the table.
pub fn substitute(d: &[u8]) -> Vec<u8> {
    let mut count = [0usize; 256];
    for &b in d {
        count[b as usize] += 1;
    }
    let mut table: Vec<u8> = (0..=255u8).filter(|&b| count[b as usize] > 0).collect();
    table.sort_by_key(|&b| std::cmp::Reverse(count[b as usize]));
    let mut place = [0u8; 256];
    for (i, &b) in table.iter().enumerate() {
        place[b as usize] = i as u8;
    }
    let mut out = vec![if table.len() == 256 { 0 } else { table.len() as u8 }];
    out.extend_from_slice(&table);
    out.extend_from_slice(&(d.len() as u32).to_be_bytes());
    out.extend(d.iter().map(|&b| place[b as usize]));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn sealed_data_opens() {
        let keys = Keys { words: 0xd5c1, modulus: 0xdb2, threshold: 0x6d9, runs: [0x9f73, 0xaee3, 0x394d] };
        let data: Vec<u8> = (0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8 | 1).collect();
        let opened = open(&seal(&data, 0x1234_5678, &keys), &keys).unwrap();
        assert_eq!(&opened[..data.len()], &data[..]);
        assert!(opened[data.len()..].iter().all(|&b| b == 0));
    }
}
