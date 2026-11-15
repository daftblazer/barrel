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
    fn unescapes_and_splits() {
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3, 7]), vec![0, 0, 1, 0, 0, 3, 7]);
        let u = units(&[9, 0, 0, 1, 0x0f, 1, 2, 0, 0, 1, 0x0e, 3]);
        assert_eq!(u, vec![(None, &[9u8][..]), (Some(0x0f), &[1, 2][..]), (Some(0x0e), &[3][..])]);
    }
}
