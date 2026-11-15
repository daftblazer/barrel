//! Bit reading for the decoders: MSB-first over a byte slice, and VC-1's start-code escaping.

/// Reads bits most significant first. Past the end it reads zeros and sets [`Bits::over`].
pub struct Bits<'a> {
    bytes: &'a [u8],
    at: usize,
    over: bool,
}

impl<'a> Bits<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Bits { bytes, at: 0, over: false }
    }

    /// The bit position from the start.
    pub fn position(&self) -> usize {
        self.at
    }

    pub fn left(&self) -> usize {
        (self.bytes.len() * 8).saturating_sub(self.at)
    }

    /// Whether a read went past the end.
    pub fn over(&self) -> bool {
        self.over
    }

    pub fn bit(&mut self) -> bool {
        self.read(1) != 0
    }

    /// `n` bits (0 to 32) as an unsigned number.
    pub fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.skip(n);
        v
    }

    pub fn peek(&mut self, n: u32) -> u32 {
        debug_assert!(n <= 32);
        if n == 0 {
            return 0;
        }
        let (byte, shift) = (self.at / 8, self.at % 8);
        let mut word = 0u64;
        for k in 0..8 {
            word = (word << 8) | *self.bytes.get(byte + k).unwrap_or(&0) as u64;
        }
        ((word << shift) >> (64 - n)) as u32
    }

    pub fn skip(&mut self, n: u32) {
        self.at += n as usize;
        if self.at > self.bytes.len() * 8 {
            self.over = true;
        }
    }

    /// To the next byte boundary.
    pub fn align(&mut self) {
        self.at = self.at.div_ceil(8) * 8;
    }
}

/// A variable-length code: table lookups of up to [`Vlc::LEVEL`] bits a step, each symbol the
/// index of its (code, length) pair.
pub struct Vlc {
    /// (symbol or subtable start, length): a positive length is a symbol's, a negative one sends
    /// the lookup to the subtable at that start reading that many more bits; 0 is no code.
    table: Vec<(u32, i8)>,
    bits: u32,
}

impl Vlc {
    pub const LEVEL: u32 = 9;

    /// From (code, length) pairs; the codes must be prefix-free (lengths 1 to 32).
    pub fn new(codes: &[(u32, u8)]) -> Self {
        let list: Vec<(u32, u32, u32)> = codes.iter().enumerate().filter(|(_, c)| c.1 > 0).map(|(k, &(c, n))| (c, n as u32, k as u32)).collect();
        let bits = list.iter().map(|c| c.1).max().unwrap_or(1).min(Self::LEVEL);
        let mut table = Vec::new();
        Self::build(&mut table, &list, bits);
        Vlc { table, bits }
    }

    fn build(table: &mut Vec<(u32, i8)>, codes: &[(u32, u32, u32)], bits: u32) -> usize {
        let base = table.len();
        table.resize(base + (1 << bits), (0, 0));
        let mut longer: std::collections::BTreeMap<u32, Vec<(u32, u32, u32)>> = Default::default();
        for &(code, len, sym) in codes {
            if len <= bits {
                let first = (code << (bits - len)) as usize;
                for e in &mut table[base + first..base + first + (1 << (bits - len))] {
                    *e = (sym, len as i8);
                }
            } else {
                let rest = len - bits;
                longer.entry(code >> rest).or_default().push((code & ((1u64 << rest) - 1) as u32, rest, sym));
            }
        }
        for (prefix, sub) in longer {
            let sub_bits = sub.iter().map(|c| c.1).max().unwrap().min(Self::LEVEL);
            let at = Self::build(table, &sub, sub_bits);
            table[base + prefix as usize] = (at as u32, -(sub_bits as i8));
        }
        base
    }

    /// The next symbol, or `None` for bits no code starts with.
    pub fn read(&self, b: &mut Bits) -> Option<usize> {
        let (mut base, mut bits) = (0usize, self.bits);
        loop {
            let (v, len) = self.table[base + b.peek(bits) as usize];
            if len > 0 {
                b.skip(len as u32);
                return Some(v as usize);
            }
            if len == 0 {
                return None;
            }
            b.skip(bits);
            base = v as usize;
            bits = (-len) as u32;
        }
    }
}

/// Takes out the emulation-prevention bytes: a 3 after two zeros, when the byte after it is 0 to
/// 3 (SMPTE 421M annex E).
pub fn unescape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut zeros = 0;
    for (k, &b) in data.iter().enumerate() {
        if zeros >= 2 && b == 3 && data.get(k + 1).is_none_or(|&n| n <= 3) {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

/// Splits `data` at its start codes (`00 00 01 xx`): each unit's code and its escaped body.
/// Bytes before the first start code come back with code `None`.
pub fn units(data: &[u8]) -> Vec<(Option<u8>, &[u8])> {
    let mut starts = Vec::new();
    let mut k = 0;
    while k + 3 < data.len() {
        if data[k] == 0 && data[k + 1] == 0 && data[k + 2] == 1 {
            starts.push(k);
            k += 4;
        } else {
            k += 1;
        }
    }
    let mut out = Vec::new();
    if starts.first().is_none_or(|&s| s > 0) {
        out.push((None, &data[..*starts.first().unwrap_or(&data.len())]));
    }
    for (n, &s) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(data.len());
        out.push((Some(data[s + 3]), &data[s + 4..end]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_msb_first_across_bytes() {
        let mut b = Bits::new(&[0b1011_0011, 0b0101_0000]);
        assert_eq!(b.read(3), 0b101);
        assert_eq!(b.read(7), 0b10011_01);
        assert!(!b.over());
        b.read(8);
        assert!(b.over());
    }

    #[test]
    fn vlc_reads_short_and_long_codes() {
        // 1, 01, 001, then 13- and 14-bit codes past the first level.
        let v = Vlc::new(&[(1, 1), (1, 2), (1, 3), (1, 13), (1, 14), (0, 14)]);
        let data = [0b1010_0100, 0b0000_0000, 0b0010_0000, 0b0000_0000, 0b1000_0000, 0];
        let mut b = Bits::new(&data);
        let got: Vec<_> = (0..5).map(|_| v.read(&mut b).unwrap()).collect();
        assert_eq!(got, vec![0, 1, 2, 3, 4]);
        assert_eq!(b.position(), 1 + 2 + 3 + 13 + 14);
    }

    #[test]
    fn unescapes_and_splits() {
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3, 7]), vec![0, 0, 1, 0, 0, 3, 7]);
        let u = units(&[9, 0, 0, 1, 0x0f, 1, 2, 0, 0, 1, 0x0e, 3]);
        assert_eq!(u, vec![(None, &[9u8][..]), (Some(0x0f), &[1, 2][..]), (Some(0x0e), &[3][..])]);
    }
}
