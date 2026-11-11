//! Encrypted tables (`*.xml.e`, Sophie 2 and the Ryza games): scrambled, checksummed and
//! LZ-compressed copies of XML tables. See `docs/survey.md`, "Encrypted tables", for the
//! stages; this follows them in order and checks the file's own checksums on the way.

use crate::Error;

const MULT: u32 = 0x3b9a73c9;
const MAX_SIZE: usize = 64 << 20;

fn be32(d: &[u8], at: usize) -> Result<u32, Error> {
    d.get(at..at + 4)
        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
        .ok_or(Error::Corrupt("xml.e read past the end"))
}

/// The generator every stage draws from.
struct Rng {
    state: u32,
    mult: u32,
}

impl Rng {
    fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(self.mult).wrapping_add(0x2f09);
        ((self.state as i32 >> 16) & 0x7fff) as u32
    }
}

/// Swap the bits of `buf` in pairs drawn at random, `chunk` bytes at a time. Applying it
/// twice with the same seed gives the bytes back.
fn bit_shuffle(buf: &mut [u8], seed: u32, chunk: usize, mult: u32) {
    let mut rng = Rng { state: seed, mult };
    for part in buf.chunks_mut(chunk) {
        let bits = part.len() * 8;
        let mut left: Vec<u16> = (0..bits as u16).collect();
        let mut order = Vec::with_capacity(bits);
        for _ in 0..bits {
            let i = rng.next() as usize % left.len();
            order.push(left.remove(i) as usize);
        }
        for pair in order.chunks_exact(2) {
            let (a, b) = (pair[0], pair[1]);
            let ba = (part[a >> 3] >> (a & 7)) & 1;
            let bb = (part[b >> 3] >> (b & 7)) & 1;
            part[a >> 3] = (part[a >> 3] & !(1 << (a & 7))) | (bb << (a & 7));
            part[b >> 3] = (part[b >> 3] & !(1 << (b & 7))) | (ba << (b & 7));
        }
    }
}

/// Undo the scrambling: returns the compressed data.
fn unscramble(file: &[u8]) -> Result<Vec<u8>, Error> {
    let version = be32(file, 0)?;
    if !(1..=2).contains(&version) {
        return Err(Error::NotThisFormat);
    }
    let mut d = file.get(16..).ok_or(Error::Corrupt("xml.e header"))?.to_vec();
    let n = d.len();
    if n < 16 {
        return Err(Error::Corrupt("xml.e too short"));
    }
    // 1. Bit shuffle of the last 0x800 bytes.
    let last = n.min(0x800);
    bit_shuffle(&mut d[n - last..], 0x6d3f, 0x100, MULT);
    // 2. Big-endian words.
    let mut rng = Rng { state: 0xcb53, mult: MULT };
    for w in d.chunks_exact_mut(2) {
        let r = rng.next();
        let mut v = u16::from_be_bytes([w[0], w[1]]) as u32;
        if r % 0x1462 >= 0xa31 {
            v ^= r;
        }
        w.copy_from_slice(&(v.wrapping_sub(r) as u16).to_be_bytes());
    }
    // 3. Bytes, up to the 0xff marker, with a multiplier from the stored key.
    let mut end = n - 12;
    while end > 0 && d[end - 1] == 0 {
        end -= 1;
    }
    if end == 0 || d[end - 1] != 0xff {
        return Err(Error::Corrupt("xml.e end marker"));
    }
    let marker = end - 1;
    let mult = be32(&d, n - 4)?.wrapping_add(MULT);
    let mut states = [0xa83bu32, 0xb11d, 0x88a5];
    let runs = [0x1dusize, 0x13, 0xb];
    let (mut pos, mut g, mut extra) = (0, 0, 0);
    while pos < marker {
        let mut r = Rng { state: states[g], mult };
        for _ in 0..runs[g] + extra {
            if pos >= marker {
                break;
            }
            d[pos] ^= (r.next() & 0xff) as u8;
            pos += 1;
        }
        states[g] = r.state;
        if g == 2 {
            extra += 1;
        }
        g = (g + 1) % 3;
    }
    d[marker] = 0;
    // 4. Checks.
    let (mut sum, mut xor) = (0u32, 0u32);
    for w in d[..marker / 4 * 4].chunks_exact(4) {
        let v = u32::from_be_bytes(w.try_into().unwrap());
        sum = sum.wrapping_sub(v);
        xor ^= !v;
    }
    if xor != be32(&d, n - 8)? || sum != be32(&d, n - 12)? {
        return Err(Error::Corrupt("xml.e checksum"));
    }
    d.truncate(marker);
    // 5. Bit shuffle of the first 0x800 bytes, still with stage 3's multiplier.
    let first = marker.min(0x800);
    bit_shuffle(&mut d[..first], 0x74b9, 0x80, mult);
    Ok(d)
}

/// The command stream's packing: a count, then an Elias-gamma code, most significant bit
/// first (k zero bits, then the value in k + 1 bits; eight zeros stand for 0).
fn gamma(buf: &[u8]) -> Result<Vec<u8>, Error> {
    let n = be32(buf, 0)? as usize;
    if n > MAX_SIZE {
        return Err(Error::TooLarge);
    }
    let mut out = Vec::with_capacity(n);
    let (mut pos, mut bit) = (4usize, 7u32);
    let peek = |pos: usize, bit: u32| -> Result<u8, Error> {
        buf.get(pos).map(|b| (b >> bit) & 1).ok_or(Error::Corrupt("xml.e command stream past the end"))
    };
    let advance = |pos: &mut usize, bit: &mut u32| {
        if *bit == 0 {
            *bit = 7;
            *pos += 1;
        } else {
            *bit -= 1;
        }
    };
    while out.len() < n {
        let mut k = 0;
        while k < 8 && peek(pos, bit)? == 0 {
            advance(&mut pos, &mut bit);
            k += 1;
        }
        if k == 8 {
            out.push(0);
            continue;
        }
        let mut v = 0u32;
        for _ in 0..=k {
            v = (v << 1) | peek(pos, bit)? as u32;
            advance(&mut pos, &mut bit);
        }
        out.push(v as u8);
    }
    Ok(out)
}

fn take<'a>(d: &'a [u8], at: &mut usize) -> Result<&'a [u8], Error> {
    let len = be32(d, *at)? as usize;
    let s = d.get(*at + 4..*at + 4 + len).ok_or(Error::Corrupt("xml.e stream past the end"))?;
    *at += 4 + len;
    Ok(s)
}

/// The LZ decompression: a command stream, a literal stream and an extra stream.
fn decompress(c: &[u8]) -> Result<Vec<u8>, Error> {
    let total = be32(c, 0)? as usize;
    if total > MAX_SIZE {
        return Err(Error::TooLarge);
    }
    let mut at = 4;
    let cmd = gamma(take(c, &mut at)?)?;
    let lit = take(c, &mut at)?;
    let ext = take(c, &mut at)?;
    let bad = || Error::Corrupt("xml.e commands run past their streams");
    let mut out: Vec<u8> = Vec::with_capacity(total);
    let (mut i, mut li, mut ei) = (0, 0, 0);
    let next = |s: &[u8], k: &mut usize| -> Result<usize, Error> {
        let v = *s.get(*k).ok_or_else(bad)? as usize;
        *k += 1;
        Ok(v)
    };
    while i < cmd.len() {
        let op = next(&cmd, &mut i)?;
        match op {
            1 => out.push(next(lit, &mut li)? as u8),
            2 => {
                let d = next(&cmd, &mut i)?;
                let b = *out.len().checked_sub(d).and_then(|p| out.get(p)).ok_or_else(bad)?;
                out.push(b);
            }
            3..=5 => {
                let (len, dist) = match op {
                    3 => {
                        let a = next(&cmd, &mut i)?;
                        let len = next(&cmd, &mut i)? + 1;
                        (len, a + len)
                    }
                    4 => {
                        let len = next(&cmd, &mut i)? + 1;
                        (len, next(lit, &mut li)? + len)
                    }
                    _ => {
                        let h = next(&cmd, &mut i)?;
                        let len = next(&cmd, &mut i)? + 1;
                        (len, (h << 8) + next(lit, &mut li)? + len)
                    }
                };
                let start = (out.len() + 1).checked_sub(dist).ok_or_else(bad)?;
                for k in 0..len {
                    let b = *out.get(start + k).ok_or_else(bad)?;
                    out.push(b);
                }
            }
            6 | 7 => {
                let n = if op == 6 { next(&cmd, &mut i)? + 8 } else { next(ext, &mut ei)? + 14 };
                out.extend_from_slice(lit.get(li..li + n).ok_or_else(bad)?);
                li += n;
            }
            _ => return Err(Error::Corrupt("xml.e unknown command")),
        }
        if out.len() > total {
            return Err(Error::Corrupt("xml.e output longer than declared"));
        }
    }
    if out.len() != total {
        return Err(Error::Corrupt("xml.e output shorter than declared"));
    }
    Ok(out)
}

/// Decrypt an `.xml.e` file into the table's XML text.
pub fn decrypt(file: &[u8]) -> Result<Vec<u8>, Error> {
    let size = be32(file, 4)? as usize;
    let out = decompress(&unscramble(file)?)?;
    if out.len() != size {
        return Err(Error::Corrupt("xml.e size differs from its header"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_undoes_itself() {
        let orig: Vec<u8> = (0..300u32).map(|i| (i * 37 + 11) as u8).collect();
        let mut d = orig.clone();
        bit_shuffle(&mut d, 0x6d3f, 0x100, MULT);
        assert_ne!(d, orig);
        bit_shuffle(&mut d, 0x6d3f, 0x100, MULT);
        assert_eq!(d, orig);
    }

    #[test]
    fn gamma_code() {
        // 1 → "1"; 2 → "010"; 5 → "00101"; 0 → "00000000". Together: 1010 0010 1000 0000 00…
        let buf = [0, 0, 0, 4, 0b1010_0010, 0b1000_0000, 0b0000_0000];
        assert_eq!(gamma(&buf).unwrap(), [1, 2, 5, 0]);
    }

    fn streams(total: u32, cmd: &[u8], lit: &[u8], ext: &[u8]) -> Vec<u8> {
        // Pack cmd with the gamma code (values 1..=255 only).
        let mut bits = Vec::new();
        for &v in cmd {
            if v == 0 {
                bits.extend([0u8; 8]);
                continue;
            }
            let k = 8 - v.leading_zeros() as usize; // bit length
            bits.extend(std::iter::repeat_n(0u8, k - 1));
            for b in (0..k).rev() {
                bits.push((v >> b) & 1);
            }
        }
        let mut packed = (cmd.len() as u32).to_be_bytes().to_vec();
        for byte in bits.chunks(8) {
            packed.push(byte.iter().enumerate().fold(0, |acc, (j, &b)| acc | (b << (7 - j))));
        }
        let mut c = total.to_be_bytes().to_vec();
        for s in [&packed[..], lit, ext] {
            c.extend((s.len() as u32).to_be_bytes());
            c.extend_from_slice(s);
        }
        c
    }

    #[test]
    fn lz_commands() {
        // "ab" as literals; command 3 copies len bytes from out.len() + 1 - (a + len), so
        // a = 0, len = 3 copies "aba" from the start; then command 2 copies 1 byte back.
        let c = streams(6, &[1, 1, 3, 0, 2, 2, 1], b"ab", &[]);
        assert_eq!(decompress(&c).unwrap(), b"ababaa");
        // 9 literals with command 6 (n + 8), 15 with command 7 (n + 14 from the extra stream).
        let lit: Vec<u8> = (b'a'..=b'x').collect();
        let c = streams(24, &[6, 1, 7], &lit, &[1]);
        assert_eq!(decompress(&c).unwrap(), lit);
    }
}
