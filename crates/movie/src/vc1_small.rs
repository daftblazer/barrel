//! VC-1's small code tables of sections 7.1, entered by hand from SMPTE ST 421:2013 (as bit
//! strings, the way the standard prints them) and checked prefix-free by the tests.

use crate::bits::Vlc;

/// A transform type: 8 × 8, 8 × 4 (two 8-wide halves, top and bottom), 4 × 8 (left and right
/// halves) or 4 × 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    T8x8,
    T8x4,
    T4x8,
    T4x4,
}

/// Which halves of an 8 × 4 / 4 × 8 block are coded: bit 1 the top / left, bit 0 the bottom /
/// right (3 both); 0 when the pattern comes from `SUBBLKPAT` (4 × 4) or does not apply (8 × 8).
pub type Halves = u8;

const TOP: Halves = 2;
const BOTTOM: Halves = 1;
const BOTH: Halves = 3;
const NA: Halves = 0;

use Transform::*;

/// Tables 54 to 56: TTMB, by rate (PQUANT < 5, 5 to 12, ≥ 13): (code, transform, halves,
/// signalled for the whole macroblock).
pub const TTMB: [&[(&str, Transform, Halves, bool)]; 3] = [
    &[
        ("11", T8x8, NA, false), ("101110", T8x4, BOTTOM, false), ("1011111", T8x4, TOP, false), ("00", T8x4, BOTH, false),
        ("10110", T4x8, BOTTOM, false), ("10101", T4x8, TOP, false), ("01", T4x8, BOTH, false), ("100", T4x4, NA, false),
        ("10100", T8x8, NA, true), ("1011110001", T8x4, BOTTOM, true), ("101111001", T8x4, TOP, true), ("101111011", T8x4, BOTH, true),
        ("101111000000", T4x8, BOTTOM, true), ("101111000001", T4x8, TOP, true), ("10111100001", T4x8, BOTH, true), ("101111010", T4x4, NA, true),
    ],
    &[
        ("110", T8x8, NA, false), ("0110", T8x4, BOTTOM, false), ("0011", T8x4, TOP, false), ("0111", T8x4, BOTH, false),
        ("1111", T4x8, BOTTOM, false), ("1110", T4x8, TOP, false), ("000", T4x8, BOTH, false), ("010", T4x4, NA, false),
        ("10", T8x8, NA, true), ("0010100", T8x4, BOTTOM, true), ("0010001", T8x4, TOP, true), ("001011", T8x4, BOTH, true),
        ("001001", T4x8, BOTTOM, true), ("00100001", T4x8, TOP, true), ("0010101", T4x8, BOTH, true), ("00100000", T4x4, NA, true),
    ],
    &[
        ("110", T8x8, NA, false), ("000", T8x4, BOTTOM, false), ("1110", T8x4, TOP, false), ("00101", T8x4, BOTH, false),
        ("010", T4x8, BOTTOM, false), ("011", T4x8, TOP, false), ("0011", T4x8, BOTH, false), ("1111", T4x4, NA, false),
        ("10", T8x8, NA, true), ("0010000001", T8x4, BOTTOM, true), ("00100001", T8x4, TOP, true), ("001001", T8x4, BOTH, true),
        ("00100000001", T4x8, BOTTOM, true), ("001000001", T4x8, TOP, true), ("0010001", T4x8, BOTH, true), ("00100000000", T4x4, NA, true),
    ],
];

/// Tables 62 to 64: TTBLK, by rate: (code, transform, halves).
pub const TTBLK: [&[(&str, Transform, Halves)]; 3] = [
    &[("00", T8x4, BOTH), ("01", T4x8, BOTH), ("11", T8x8, NA), ("101", T4x4, NA), ("10000", T8x4, TOP), ("10001", T8x4, BOTTOM), ("10010", T4x8, BOTTOM), ("10011", T4x8, TOP)],
    &[("11", T8x8, NA), ("000", T4x8, BOTTOM), ("001", T4x8, TOP), ("010", T4x4, NA), ("011", T8x4, BOTH), ("101", T4x8, BOTH), ("1000", T8x4, BOTTOM), ("1001", T8x4, TOP)],
    &[("01", T8x8, NA), ("000", T4x8, BOTH), ("001", T4x4, NA), ("100", T8x4, BOTTOM), ("110", T4x8, BOTTOM), ("111", T4x8, TOP), ("1010", T8x4, BOTH), ("1011", T8x4, TOP)],
];

/// Tables 65 to 67: SUBBLKPAT for 4 × 4 blocks, by rate: (code, pattern; bit 3 the top-left
/// sub-block, bit 2 top-right, bit 1 bottom-left, bit 0 bottom-right).
pub const SUBBLKPAT4: [&[(&str, u8)]; 3] = [
    &[("1", 15), ("0000", 11), ("0001", 13), ("0010", 7), ("00110", 12), ("00111", 3), ("01000", 10), ("01001", 5), ("01010", 8), ("01011", 4), ("01100", 2), ("01110", 1), ("01111", 14), ("011010", 6), ("011011", 9)],
    &[("01", 15), ("000", 2), ("0011", 12), ("1000", 3), ("1001", 10), ("1010", 5), ("1101", 8), ("1110", 1), ("1111", 4), ("00100", 6), ("00101", 9), ("10110", 14), ("10111", 7), ("11000", 13), ("11001", 11)],
    &[("010", 4), ("011", 8), ("101", 1), ("110", 2), ("0001", 12), ("0010", 3), ("0011", 10), ("1000", 5), ("1111", 15), ("00000", 6), ("00001", 9), ("10010", 14), ("10011", 13), ("11100", 7), ("11101", 11)],
];

/// Table 68: SUBBLKPAT for 8 × 4 / 4 × 8 blocks (signalled per block): the halves coded.
pub const SUBBLKPAT2: &[(&str, Halves)] = &[("10", BOTTOM), ("0", BOTH), ("11", TOP)];

/// Tables 46 / 47: MVMODE, low rate (PQUANT > 12) and high rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MvMode {
    /// One vector a macroblock, quarter-pel bicubic.
    One,
    /// One or four vectors a macroblock (`MVTYPEMB`), quarter-pel bicubic.
    Mixed,
    /// One vector, half-pel bicubic.
    OneHalf,
    /// One vector, half-pel bilinear.
    OneHalfBilinear,
    IntensityCompensation,
}

pub const MVMODE_LOW: &[(&str, MvMode)] = &[("1", MvMode::OneHalfBilinear), ("01", MvMode::One), ("001", MvMode::OneHalf), ("0000", MvMode::Mixed), ("0001", MvMode::IntensityCompensation)];
pub const MVMODE_HIGH: &[(&str, MvMode)] = &[("1", MvMode::One), ("01", MvMode::Mixed), ("001", MvMode::OneHalf), ("0000", MvMode::OneHalfBilinear), ("0001", MvMode::IntensityCompensation)];
/// Tables 49 / 50: MVMODE2 after intensity compensation.
pub const MVMODE2_LOW: &[(&str, MvMode)] = &[("1", MvMode::OneHalfBilinear), ("01", MvMode::One), ("001", MvMode::OneHalf), ("000", MvMode::Mixed)];
pub const MVMODE2_HIGH: &[(&str, MvMode)] = &[("1", MvMode::One), ("01", MvMode::Mixed), ("001", MvMode::OneHalf), ("000", MvMode::OneHalfBilinear)];
/// Table 37: MVRANGE 0 to 3.
pub const MVRANGE: &[(&str, u8)] = &[("0", 0), ("10", 1), ("110", 2), ("111", 3)];

/// Table 40: BFRACTION, as (numerator, denominator); 1111110b and 1111111b are reserved in
/// the advanced profile.
pub const BFRACTION: &[(&str, (u8, u8))] = &[
    ("000", (1, 2)), ("001", (1, 3)), ("010", (2, 3)), ("011", (1, 4)), ("100", (3, 4)), ("101", (1, 5)), ("110", (2, 5)),
    ("1110000", (3, 5)), ("1110001", (4, 5)), ("1110010", (1, 6)), ("1110011", (5, 6)), ("1110100", (1, 7)), ("1110101", (2, 7)),
    ("1110110", (3, 7)), ("1110111", (4, 7)), ("1111000", (5, 7)), ("1111001", (6, 7)), ("1111010", (1, 8)), ("1111011", (3, 8)),
    ("1111100", (5, 8)), ("1111101", (7, 8)),
];

/// A table of bit-string codes as a [`Vlc`] and its values by symbol.
pub struct Coded<T: Copy> {
    pub vlc: Vlc,
    pub values: Vec<T>,
}

impl<T: Copy> Coded<T> {
    pub fn new<R>(rows: &[R], code: impl Fn(&R) -> (&str, T)) -> Self {
        let mut codes = Vec::new();
        let mut values = Vec::new();
        for r in rows {
            let (s, v) = code(r);
            codes.push((u32::from_str_radix(s, 2).unwrap(), s.len() as u8));
            values.push(v);
        }
        Coded { vlc: Vlc::new(&codes), values }
    }

    pub fn read(&self, b: &mut crate::bits::Bits) -> Option<T> {
        self.vlc.read(b).map(|k| self.values[k])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, codes: &[&str]) {
        for (i, a) in codes.iter().enumerate() {
            for (j, b) in codes.iter().enumerate() {
                assert!(i == j || !b.starts_with(a), "{name}: {a} is a prefix of {b}");
            }
        }
        let kraft: f64 = codes.iter().map(|c| 0.5f64.powi(c.len() as i32)).sum();
        assert!(kraft <= 1.0 + 1e-12, "{name}: Kraft sum {kraft}");
    }

    #[test]
    fn small_tables_are_prefix_free() {
        for (k, t) in TTMB.iter().enumerate() {
            check(&format!("TTMB {k}"), &t.iter().map(|r| r.0).collect::<Vec<_>>());
            assert_eq!(t.len(), 16);
        }
        for (k, t) in TTBLK.iter().enumerate() {
            check(&format!("TTBLK {k}"), &t.iter().map(|r| r.0).collect::<Vec<_>>());
        }
        for (k, t) in SUBBLKPAT4.iter().enumerate() {
            check(&format!("SUBBLKPAT {k}"), &t.iter().map(|r| r.0).collect::<Vec<_>>());
            let mut seen: Vec<u8> = t.iter().map(|r| r.1).collect();
            seen.sort();
            assert_eq!(seen, (1..=15).collect::<Vec<_>>(), "SUBBLKPAT {k} patterns");
        }
        for (name, t) in [("MVMODE low", MVMODE_LOW), ("MVMODE high", MVMODE_HIGH), ("MVMODE2 low", MVMODE2_LOW), ("MVMODE2 high", MVMODE2_HIGH)] {
            check(name, &t.iter().map(|r| r.0).collect::<Vec<_>>());
        }
        check("BFRACTION", &BFRACTION.iter().map(|r| r.0).collect::<Vec<_>>());
        check("SUBBLKPAT2", &SUBBLKPAT2.iter().map(|r| r.0).collect::<Vec<_>>());
        check("MVRANGE", &MVRANGE.iter().map(|r| r.0).collect::<Vec<_>>());
    }
}
