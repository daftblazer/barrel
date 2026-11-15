//! VC-1 bitplanes (SMPTE ST 421 7.2 and 8.7): one bit a macroblock, coded in the picture header
//! in one of seven modes, or left "raw" to the macroblock layer.

use crate::bits::{Bits, Vlc};

/// A decoded bitplane, or the note that each macroblock carries its own bit.
#[derive(Debug, Clone, PartialEq)]
pub enum Bitplane {
    Raw,
    /// One byte (0 or 1) a macroblock, row by row.
    Bits(Vec<u8>),
}

impl Bitplane {
    /// The bit of macroblock (x, y) for a decoded plane; `None` when the macroblock holds it.
    pub fn get(&self, x: usize, y: usize, cols: usize) -> Option<bool> {
        match self {
            Bitplane::Raw => None,
            Bitplane::Bits(b) => Some(b[y * cols + x] != 0),
        }
    }
}

/// Table 81: the Norm-6 tile code for each tile value k (0 to 63) as one (code, length), the
/// escapes and their following codes joined.
const NORM6: [(u32, u32, u32, u32); 64] = [
    (1, 1, 0, 0), (2, 4, 0, 0), (3, 4, 0, 0), (0, 8, 0, 0), (4, 4, 0, 0), (1, 8, 0, 0), (2, 8, 0, 0), (2, 5, 7, 5),
    (5, 4, 0, 0), (3, 8, 0, 0), (4, 8, 0, 0), (2, 5, 11, 5), (5, 8, 0, 0), (2, 5, 13, 5), (2, 5, 14, 5), (3, 5, 14, 8),
    (6, 4, 0, 0), (6, 8, 0, 0), (7, 8, 0, 0), (2, 5, 19, 5), (8, 8, 0, 0), (2, 5, 21, 5), (2, 5, 22, 5), (3, 5, 13, 8),
    (9, 8, 0, 0), (2, 5, 25, 5), (2, 5, 26, 5), (3, 5, 12, 8), (2, 5, 28, 5), (3, 5, 11, 8), (3, 5, 10, 8), (3, 5, 7, 4),
    (7, 4, 0, 0), (10, 8, 0, 0), (11, 8, 0, 0), (2, 5, 3, 5), (12, 8, 0, 0), (2, 5, 5, 5), (2, 5, 6, 5), (3, 5, 9, 8),
    (13, 8, 0, 0), (2, 5, 9, 5), (2, 5, 10, 5), (3, 5, 8, 8), (2, 5, 12, 5), (3, 5, 7, 8), (3, 5, 6, 8), (3, 5, 6, 4),
    (14, 8, 0, 0), (2, 5, 17, 5), (2, 5, 18, 5), (3, 5, 5, 8), (2, 5, 20, 5), (3, 5, 4, 8), (3, 5, 3, 8), (3, 5, 5, 4),
    (2, 5, 24, 5), (3, 5, 2, 8), (3, 5, 1, 8), (3, 5, 4, 4), (3, 5, 0, 8), (3, 5, 3, 4), (3, 5, 2, 4), (3, 5, 1, 1),
];

/// The joined Norm-6 codes.
pub fn norm6_codes() -> Vec<(u32, u8)> {
    NORM6.iter().map(|&(c, n, f, m)| ((c << m) | f, (n + m) as u8)).collect()
}

/// Table 69: IMODE, as (code, length) for Norm-2, Norm-6, Rowskip, Colskip, Diff-2, Diff-6, Raw.
const IMODE: [(u32, u8); 7] = [(0b10, 2), (0b11, 2), (0b010, 3), (0b011, 3), (0b001, 3), (0b0001, 4), (0b0000, 4)];

/// Table 80: Norm-2 pairs (0,0) 0b, (1,0) 100b, (0,1) 101b, (1,1) 11b, by `first | second << 1`.
const NORM2: [(u32, u8); 4] = [(0, 1), (0b100, 3), (0b101, 3), (0b11, 2)];

pub struct Tables {
    imode: Vlc,
    norm2: Vlc,
    norm6: Vlc,
}

impl Default for Tables {
    fn default() -> Self {
        Tables { imode: Vlc::new(&IMODE), norm2: Vlc::new(&NORM2), norm6: Vlc::new(&norm6_codes()) }
    }
}

fn row_skip(b: &mut Bits, plane: &mut [u8], stride: usize, x0: usize, y0: usize, width: usize, height: usize) {
    for y in y0..y0 + height {
        if b.bit() {
            for x in x0..x0 + width {
                plane[y * stride + x] = b.bit() as u8;
            }
        }
    }
}

fn column_skip(b: &mut Bits, plane: &mut [u8], stride: usize, x0: usize, y0: usize, width: usize, height: usize) {
    for x in x0..x0 + width {
        if b.bit() {
            for y in y0..y0 + height {
                plane[y * stride + x] = b.bit() as u8;
            }
        }
    }
}

impl Tables {
    /// Reads a bitplane of `cols` × `rows` macroblocks.
    pub fn read(&self, b: &mut Bits, cols: usize, rows: usize) -> Option<Bitplane> {
        let invert = b.bit();
        let mode = self.imode.read(b)?;
        if mode == 6 {
            return Some(Bitplane::Raw);
        }
        let n = cols * rows;
        let mut plane = vec![0u8; n];
        match mode {
            0 | 4 => {
                let mut k = 0;
                if n % 2 == 1 {
                    plane[0] = b.bit() as u8;
                    k = 1;
                }
                while k < n {
                    let pair = self.norm2.read(b)?;
                    plane[k] = (pair & 1) as u8;
                    plane[k + 1] = (pair >> 1) as u8;
                    k += 2;
                }
            }
            1 | 5 => {
                if rows % 3 == 0 && cols % 3 != 0 {
                    // 2 × 3 vertical tiles, after `cols % 2` columns.
                    for y in (0..rows).step_by(3) {
                        for x in (cols % 2..cols).step_by(2) {
                            let k = self.norm6.read(b)?;
                            for i in 0..6 {
                                plane[(y + i / 2) * cols + x + i % 2] = ((k >> i) & 1) as u8;
                            }
                        }
                    }
                    column_skip(b, &mut plane, cols, 0, 0, cols % 2, rows);
                } else {
                    // 3 × 2 horizontal tiles, after `rows % 2` rows and `cols % 3` columns.
                    for y in (rows % 2..rows).step_by(2) {
                        for x in (cols % 3..cols).step_by(3) {
                            let k = self.norm6.read(b)?;
                            for i in 0..6 {
                                plane[(y + i / 3) * cols + x + i % 3] = ((k >> i) & 1) as u8;
                            }
                        }
                    }
                    column_skip(b, &mut plane, cols, 0, 0, cols % 3, rows);
                    row_skip(b, &mut plane, cols, cols % 3, 0, cols - cols % 3, rows % 2);
                }
            }
            2 => row_skip(b, &mut plane, cols, 0, 0, cols, rows),
            3 => column_skip(b, &mut plane, cols, 0, 0, cols, rows),
            _ => unreachable!(),
        }
        if mode == 4 || mode == 5 {
            // Diff-1 (8.7.3.8): each bit against its predictor, INVERT standing for A.
            let a = invert as u8;
            for y in 0..rows {
                for x in 0..cols {
                    let i = y * cols + x;
                    let p = if x == 0 && y == 0 {
                        a
                    } else if y == 0 {
                        plane[i - 1]
                    } else if x == 0 {
                        plane[i - cols]
                    } else if plane[i - 1] != plane[i - cols] {
                        a
                    } else {
                        plane[i - 1]
                    };
                    plane[i] ^= p;
                }
            }
        } else if invert {
            for v in &mut plane {
                *v ^= 1;
            }
        }
        (!b.over()).then_some(Bitplane::Bits(plane))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm6_codes_are_prefix_free_and_complete() {
        let codes = norm6_codes();
        let kraft: f64 = codes.iter().map(|&(_, n)| 0.5f64.powi(n as i32)).sum();
        assert!(kraft <= 1.0 + 1e-12, "Kraft sum {kraft}");
        let strs: Vec<String> = codes.iter().map(|&(c, n)| format!("{c:0w$b}", w = n as usize)).collect();
        for (i, a) in strs.iter().enumerate() {
            for (j, b) in strs.iter().enumerate() {
                assert!(i == j || !b.starts_with(a.as_str()), "{a} ({i}) is a prefix of {b} ({j})");
            }
        }
    }

    #[test]
    fn rowskip_and_invert() {
        // INVERT 1, IMODE 010 (rowskip), 2 × 2: row 0 skipped, row 1 "1 0" → inverted 1 1 / 0 1.
        let data = [0b1010_0110, 0];
        let mut b = Bits::new(&data);
        let p = Tables::default().read(&mut b, 2, 2).unwrap();
        assert_eq!(p, Bitplane::Bits(vec![1, 1, 0, 1]));
        assert_eq!(b.position(), 1 + 3 + 1 + 3);
    }
}

