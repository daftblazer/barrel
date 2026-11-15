//! VC-1 motion vectors in progressive P pictures (SMPTE ST 421 8.3.5.2 to 8.3.5.4): the
//! differential in MVDATA / BLKMVDATA, the predictor from three neighbours (with pull-back and
//! hybrid prediction) and the reconstructed luma and colour-difference vectors. All in
//! quarter-pel units.

use crate::bits::{Bits, Vlc};

/// What an MVDATA / BLKMVDATA codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MvData {
    pub dx: i32,
    pub dy: i32,
    pub intra: bool,
    /// Coefficients follow (the "more_present flag").
    pub more: bool,
}

/// Table 75: (k_x, k_y, range_x, range_y) by MVRANGE.
pub const RANGES: [(u32, u32, i32, i32); 4] = [(9, 8, 256, 128), (10, 9, 512, 256), (12, 10, 2048, 512), (13, 11, 4096, 1024)];

const SIZE: [u32; 6] = [0, 2, 3, 4, 5, 8];
const OFFSET: [i32; 6] = [0, 1, 3, 7, 15, 31];

/// Figure 50. `half` is the half-pel modes' flag; the differential comes back in the picture's
/// own units (half-pel units in the half-pel modes).
pub fn read_mvdata(b: &mut Bits, vlc: &Vlc, range: u8, half: bool) -> Option<MvData> {
    let (kx, ky, _, _) = RANGES[range as usize];
    let mut index = vlc.read(b)? + 1;
    let more = index >= 37;
    if more {
        index -= 37;
    }
    let h = half as u32;
    let mut d = MvData { more, ..Default::default() };
    match index {
        0 => {}
        35 => {
            // Read as they stand: the modulo differential, wrapped later by `smod`.
            d.dx = b.read(kx - h) as i32;
            d.dy = b.read(ky - h) as i32;
        }
        36 => d.intra = true,
        _ => {
            let component = |b: &mut Bits, i: usize| -> i32 {
                let hpel = (half && i == 5) as u32;
                let val = b.read(SIZE[i] - hpel) as i32;
                let sign = -(val & 1);
                (sign ^ ((val >> 1) + OFFSET[i])) - sign
            };
            d.dx = component(b, index % 6);
            d.dy = component(b, index / 6);
        }
    }
    Some(d)
}

/// `a smod b` (4.11): `((a + b) & (2b − 1)) − b`, b a power of two.
pub fn smod(a: i32, b: i32) -> i32 {
    ((a + b) & (2 * b - 1)) - b
}

pub fn median3(a: i32, b: i32, c: i32) -> i32 {
    a.max(b).min(a.min(b).max(c))
}

/// `median4` (4.11): the sum less the largest and smallest, halved (C division).
pub fn median4(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let (mx, mn) = (a.max(b).max(c).max(d), a.min(b).min(c).min(d));
    (a + b + c + d - mx - mn) / 2
}

/// A predictor candidate: `None` out of bounds (outside the frame), else the block's vector
/// and whether it is intra (its vector then counts as zero).
pub type Candidate = Option<([i32; 2], bool)>;

/// The predictor before hybrid prediction (Figure 54), then pulled back (8.3.5.3.4) so the
/// region it points at keeps a pixel inside the frame: `at` is the block's or macroblock's
/// top-left in pixels, `size` 16 or 8, `frame` the macroblock-aligned size in pixels.
pub fn preliminary(a: Candidate, b: Candidate, c: Candidate, at: [i32; 2], size: i32, frame: [i32; 2]) -> [i32; 2] {
    let v = |c: Candidate| c.map(|(v, intra)| if intra { [0, 0] } else { v }).unwrap_or([0, 0]);
    let mut p = if a.is_some() {
        if c.is_none() && b.is_none() {
            v(a)
        } else {
            let (va, vb, vc) = (v(a), v(b), v(c));
            [median3(va[0], vb[0], vc[0]), median3(va[1], vb[1], vc[1])]
        }
    } else if c.is_some() {
        v(c)
    } else {
        [0, 0]
    };
    let low = -(size * 4 - 4);
    for k in 0..2 {
        let q = at[k] * 4;
        let high = frame[k] * 4 - 4;
        if q + p[k] < low {
            p[k] = low - q;
        }
        if q + p[k] > high {
            p[k] = high - q;
        }
    }
    p
}

/// Hybrid prediction (Figure 55): whether a HYBRIDPRED bit follows. When it does, the bit
/// picks A (1) or C (0); else the preliminary predictor stands.
pub fn hybrid_needed(pre: [i32; 2], a: Candidate, c: Candidate) -> bool {
    let (Some((va, ia)), Some((vc, ic))) = (a, c) else { return false };
    let dist = |v: [i32; 2], intra: bool| if intra { pre[0].abs() + pre[1].abs() } else { (pre[0] - v[0]).abs() + (pre[1] - v[1]).abs() };
    dist(va, ia) > 32 || dist(vc, ic) > 32
}

/// The vector a HYBRIDPRED bit picks (an intra candidate's is zero).
pub fn hybrid_pick(top: bool, a: Candidate, c: Candidate) -> [i32; 2] {
    let pick = if top { a } else { c };
    pick.map(|(v, intra)| if intra { [0, 0] } else { v }).unwrap_or([0, 0])
}

/// 8.3.5.4.3 / 8.3.5.4.5: a colour-difference vector from a luma one, and FASTUVMC's rounding.
pub fn chroma(l: [i32; 2], fast_uvmc: bool) -> [i32; 2] {
    const RND: [i32; 4] = [0, 0, 0, 1];
    l.map(|v| {
        let c = (v + RND[(v & 3) as usize]) >> 1;
        if fast_uvmc {
            // Quarter-pel positions go to the half / whole pel toward zero.
            match c % 2 {
                1 => c - 1,
                -1 => c + 1,
                _ => c,
            }
        } else {
            c
        }
    })
}

/// Figure 57: the luma vector a 4-MV macroblock's colour-difference vector comes from, or
/// `None` when 3 or 4 of its luma blocks are intra (the colour-difference blocks then are too).
pub fn four_mv_luma_for_chroma(blocks: &[([i32; 2], bool); 4]) -> Option<[i32; 2]> {
    let inter: Vec<[i32; 2]> = blocks.iter().filter(|b| !b.1).map(|b| b.0).collect();
    Some(match inter.len() {
        4 => [0, 1].map(|k| median4(inter[0][k], inter[1][k], inter[2][k], inter[3][k])),
        3 => [0, 1].map(|k| median3(inter[0][k], inter[1][k], inter[2][k])),
        2 => [0, 1].map(|k| (inter[0][k] + inter[1][k]) / 2),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_follow_the_standard() {
        assert_eq!(smod(255, 256), 255);
        assert_eq!(smod(256, 256), -256);
        assert_eq!(smod(-257, 256), 255);
        assert_eq!(median3(5, -2, 9), 5);
        assert_eq!(median4(1, 8, 3, 4), 3);
        assert_eq!(median4(-3, -4, 1, 0), -1);
        assert_eq!(chroma([3, -3], false), [2, -2]);
        assert_eq!(chroma([1, -5], true), [0, -2]);
        assert_eq!(chroma([3, -3], true), [2, -2]);
    }

    #[test]
    fn candidates_and_pullback() {
        // A alone (B and C out): A itself.
        assert_eq!(preliminary(Some(([8, 4], false)), None, None, [16, 16], 16, [64, 64]), [8, 4]);
        // Median of three, an intra one counting as zero.
        assert_eq!(preliminary(Some(([8, 4], false)), Some(([100, 100], true)), Some(([2, 9], false)), [16, 16], 16, [64, 64]), [2, 4]);
        // Pulled back: a macroblock at x 0 pointing 30 pixels left keeps a pixel inside.
        assert_eq!(preliminary(None, None, Some(([-120, 0], false)), [0, 16], 16, [64, 64]), [-60, 0]);
        // Hybrid: needed when the median strays more than 32 quarter-pels from A or C.
        assert!(hybrid_needed([40, 0], Some(([0, 0], false)), Some(([40, 0], false))));
        assert!(!hybrid_needed([4, 0], Some(([0, 0], false)), Some(([8, 0], false))));
    }
}
