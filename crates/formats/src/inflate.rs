//! DEFLATE (RFC 1951) and the zlib wrapper (RFC 1950), decode only.
//!
//! The games compress with plain zlib, so this is all the decompression Barrel needs. Output
//! size is bounded by the caller so a corrupt stream cannot grow without limit.

use crate::Error;

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CODE_LENGTH_ORDER: [usize; 19] =
    [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit_buf: u64,
    bit_count: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0, bit_buf: 0, bit_count: 0 }
    }

    fn refill(&mut self) {
        while self.bit_count <= 56 {
            let byte = match self.data.get(self.pos) {
                Some(&b) => b,
                None => return,
            };
            self.pos += 1;
            self.bit_buf |= (byte as u64) << self.bit_count;
            self.bit_count += 8;
        }
    }

    fn bits(&mut self, n: u32) -> Result<u32, Error> {
        if n == 0 {
            return Ok(0);
        }
        if self.bit_count < n {
            self.refill();
            if self.bit_count < n {
                return Err(Error::Corrupt("deflate stream ends early"));
            }
        }
        let value = (self.bit_buf & ((1u64 << n) - 1)) as u32;
        self.bit_buf >>= n;
        self.bit_count -= n;
        Ok(value)
    }

    /// Drop the bits up to the next byte boundary and return the reader's byte position.
    fn align(&mut self) -> usize {
        let drop = self.bit_count % 8;
        self.bit_buf >>= drop;
        self.bit_count -= drop;
        self.pos - (self.bit_count / 8) as usize
    }

    fn skip_to(&mut self, pos: usize) {
        self.pos = pos;
        self.bit_buf = 0;
        self.bit_count = 0;
    }
}

/// A canonical Huffman decoder: codes looked up 9 bits at a time, longer codes walked.
struct Huffman {
    /// For each length 1..=15, how many codes have it.
    counts: [u16; 16],
    /// Symbols ordered by code.
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Self, Error> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut left: i32 = 1;
        for &c in &counts[1..] {
            left = (left << 1) - c as i32;
            if left < 0 {
                return Err(Error::Corrupt("over-subscribed huffman code"));
            }
        }
        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offsets[l as usize] as usize] = sym as u16;
                offsets[l as usize] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, Error> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= bits.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return Ok(self.symbols[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(Error::Corrupt("bad huffman code"))
    }
}

fn fixed_tables() -> (Huffman, Huffman) {
    let mut lit = [0u8; 288];
    lit[..144].fill(8);
    lit[144..256].fill(9);
    lit[256..280].fill(7);
    lit[280..].fill(8);
    let dist = [5u8; 30];
    (Huffman::new(&lit).unwrap(), Huffman::new(&dist).unwrap())
}

fn dynamic_tables(bits: &mut Bits) -> Result<(Huffman, Huffman), Error> {
    let hlit = bits.bits(5)? as usize + 257;
    let hdist = bits.bits(5)? as usize + 1;
    let hclen = bits.bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return Err(Error::Corrupt("too many deflate codes"));
    }
    let mut cl_lengths = [0u8; 19];
    for &i in &CODE_LENGTH_ORDER[..hclen] {
        cl_lengths[i] = bits.bits(3)? as u8;
    }
    let cl = Huffman::new(&cl_lengths)?;
    let mut lengths = [0u8; 286 + 30];
    let total = hlit + hdist;
    let mut i = 0;
    while i < total {
        let sym = cl.decode(bits)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                if i == 0 {
                    return Err(Error::Corrupt("repeat with no previous length"));
                }
                (lengths[i - 1], 3 + bits.bits(2)? as usize)
            }
            17 => (0, 3 + bits.bits(3)? as usize),
            _ => (0, 11 + bits.bits(7)? as usize),
        };
        if i + repeat > total {
            return Err(Error::Corrupt("code lengths overrun"));
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[256] == 0 {
        return Err(Error::Corrupt("no end-of-block code"));
    }
    Ok((Huffman::new(&lengths[..hlit])?, Huffman::new(&lengths[hlit..total])?))
}

/// Inflate a raw DEFLATE stream. Returns the output and the number of input bytes used.
pub fn inflate(input: &[u8], max_out: usize) -> Result<(Vec<u8>, usize), Error> {
    let mut out: Vec<u8> = Vec::new();
    let mut bits = Bits::new(input);
    loop {
        let last = bits.bits(1)? == 1;
        match bits.bits(2)? {
            0 => {
                let pos = bits.align();
                let header = input.get(pos..pos + 4).ok_or(Error::Corrupt("stored block header"))?;
                let len = u16::from_le_bytes([header[0], header[1]]) as usize;
                let nlen = u16::from_le_bytes([header[2], header[3]]) as usize;
                if len != !nlen & 0xffff {
                    return Err(Error::Corrupt("stored block length check"));
                }
                let body = input.get(pos + 4..pos + 4 + len).ok_or(Error::Corrupt("stored block"))?;
                if out.len() + len > max_out {
                    return Err(Error::TooLarge);
                }
                out.extend_from_slice(body);
                bits.skip_to(pos + 4 + len);
            }
            kind @ (1 | 2) => {
                let (lit, dist) = if kind == 1 { fixed_tables() } else { dynamic_tables(&mut bits)? };
                loop {
                    let sym = lit.decode(&mut bits)?;
                    if sym < 256 {
                        if out.len() >= max_out {
                            return Err(Error::TooLarge);
                        }
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let i = (sym - 257) as usize;
                        if i >= 29 {
                            return Err(Error::Corrupt("bad length symbol"));
                        }
                        let len = LENGTH_BASE[i] as usize + bits.bits(LENGTH_EXTRA[i] as u32)? as usize;
                        let d = dist.decode(&mut bits)? as usize;
                        if d >= 30 {
                            return Err(Error::Corrupt("bad distance symbol"));
                        }
                        let distance = DIST_BASE[d] as usize + bits.bits(DIST_EXTRA[d] as u32)? as usize;
                        if distance > out.len() {
                            return Err(Error::Corrupt("distance before start"));
                        }
                        if out.len() + len > max_out {
                            return Err(Error::TooLarge);
                        }
                        let start = out.len() - distance;
                        if distance >= len {
                            out.extend_from_within(start..start + len);
                        } else {
                            for k in 0..len {
                                let b = out[start + k];
                                out.push(b);
                            }
                        }
                    }
                }
            }
            _ => return Err(Error::Corrupt("reserved deflate block type")),
        }
        if last {
            break;
        }
    }
    let used = bits.align();
    Ok((out, used))
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}
/// A gzip member (RFC 1952: its header, optional fields included, then the deflate stream),
/// its size taken from the trailer and held to `max_out`. Ayesha DX's gzipped `*.PSSG.gz`.
pub fn gunzip(input: &[u8], max_out: usize) -> Result<Vec<u8>, Error> {
    if input.len() < 18 || input[0] != 0x1f || input[1] != 0x8b || input[2] != 8 {
        return Err(Error::NotThisFormat);
    }
    let flags = input[3];
    let mut at = 10;
    if flags & 4 != 0 {
        let len = u16::from_le_bytes([*input.get(at).ok_or(Error::Corrupt("gzip: truncated"))?, *input.get(at + 1).ok_or(Error::Corrupt("gzip: truncated"))?]) as usize;
        at += 2 + len;
    }
    for flag in [8u8, 16] {
        if flags & flag != 0 {
            at += input.get(at..).ok_or(Error::Corrupt("gzip: truncated"))?.iter().position(|&b| b == 0).ok_or(Error::Corrupt("gzip: name not ended"))? + 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    let size = u32::from_le_bytes(input[input.len() - 4..].try_into().unwrap()) as usize;
    if size > max_out {
        return Err(Error::TooLarge);
    }
    let (out, _) = inflate(input.get(at..).ok_or(Error::Corrupt("gzip: truncated"))?, size)?;
    if out.len() != size {
        return Err(Error::Corrupt("gzip: size does not match the trailer"));
    }
    Ok(out)
}


/// Decompress one zlib stream, checking its header and Adler-32.
pub fn zlib_decompress(input: &[u8], max_out: usize) -> Result<Vec<u8>, Error> {
    let [cmf, flg, ..] = *input else {
        return Err(Error::Corrupt("zlib header"));
    };
    if cmf & 0x0f != 8 || ((cmf as u16) << 8 | flg as u16) % 31 != 0 || flg & 0x20 != 0 {
        return Err(Error::Corrupt("zlib header"));
    }
    let (out, used) = inflate(&input[2..], max_out)?;
    let tail = input.get(2 + used..2 + used + 4).ok_or(Error::Corrupt("zlib checksum missing"))?;
    if u32::from_be_bytes([tail[0], tail[1], tail[2], tail[3]]) != adler32(&out) {
        return Err(Error::Corrupt("zlib checksum"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn gunzip_reads_a_member() {
        // gzip.compress(b"PSSG barrel", mtime=0)
        let gz = [0x1f, 0x8b, 0x8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x2, 0xff, 0xb, 0x8, 0xe, 0x76, 0x57, 0x48, 0x4a, 0x2c, 0x2a, 0x4a, 0xcd, 0x1, 0x0, 0x4c, 0x48, 0x83, 0xcd, 0xb, 0x0, 0x0, 0x0];
        assert_eq!(gunzip(&gz, 64).unwrap(), b"PSSG barrel");
        assert!(gunzip(&gz, 4).is_err(), "held to the size given");
    }

    use super::*;

    #[test]
    fn stored_and_fixed() {
        // zlib.compress(b"hello hello hello", 9) and a stored block of "abc".
        let fixed = [
            0x78, 0xda, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00, 0x3a, 0x2e,
            0x06, 0x7d,
        ];
        assert_eq!(zlib_decompress(&fixed, 64).unwrap(), b"hello hello hello");
        let stored = [0x78, 0x01, 0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c', 0x02, 0x4d, 0x01, 0x27];
        assert_eq!(zlib_decompress(&stored, 64).unwrap(), b"abc");
        assert!(matches!(zlib_decompress(&fixed, 4), Err(Error::TooLarge)));
    }
}
