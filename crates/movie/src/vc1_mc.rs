//! VC-1 motion compensation (SMPTE ST 421 8.3.6.5): a block predicted from the reference
//! picture at a quarter-pel vector, bicubic for luma (bilinear in the half-pel bilinear mode),
//! bilinear for colour difference, the reference repeated past its edges. And the inter
//! blocks' inverse transforms (Annex A): 8 × 8, 8 × 4, 4 × 8, 4 × 4.

/// A reference plane: its pixels and size; reads past the edges repeat the edge pixels.
pub struct Plane<'a> {
    pub px: &'a [u8],
    pub stride: usize,
    pub width: i32,
    pub height: i32,
}

impl Plane<'_> {
    #[inline]
    fn at(&self, x: i32, y: i32) -> i32 {
        let x = x.clamp(0, self.width - 1) as usize;
        let y = y.clamp(0, self.height - 1) as usize;
        self.px[y * self.stride + x] as i32
    }
}

/// The bicubic taps by quarter shift (1 ¼, 2 ½, 3 ¾), P1 to P4.
const TAPS: [[i32; 4]; 4] = [[0, 16, 0, 0], [-4, 53, 18, -3], [-1, 9, 9, -1], [-3, 18, 53, -4]];
/// ffmpeg-style per-mode shift halves: the vertical pass's shift is (h + v) / 2 of these.
const SHIFT: [i32; 4] = [0, 5, 1, 5];

/// A `w` × `h` luma block at pixel (`x`, `y`) displaced by `mv` (quarter-pel), bicubic
/// (8.3.6.5.2), into `out` (row stride `w`).
pub fn bicubic(r: &Plane, x: i32, y: i32, mv: [i32; 2], w: usize, h: usize, rnd: i32, out: &mut [u8]) {
    let (ix, iy) = (x + (mv[0] >> 2), y + (mv[1] >> 2));
    let (fx, fy) = ((mv[0] & 3) as usize, (mv[1] & 3) as usize);
    for j in 0..h {
        for i in 0..w {
            let (px, py) = (ix + i as i32, iy + j as i32);
            let v = match (fx, fy) {
                (0, 0) => r.at(px, py),
                (_, 0) => {
                    // Horizontal only: r = RND.
                    let s: i32 = (0..4).map(|k| TAPS[fx][k] * r.at(px - 1 + k as i32, py)).sum();
                    if fx == 2 { (s + 8 - rnd) >> 4 } else { (s + 32 - rnd) >> 6 }
                }
                (0, _) => {
                    // Vertical only: r = 1 − RND.
                    let s: i32 = (0..4).map(|k| TAPS[fy][k] * r.at(px, py - 1 + k as i32)).sum();
                    if fy == 2 { (s + 8 - (1 - rnd)) >> 4 } else { (s + 32 - (1 - rnd)) >> 6 }
                }
                _ => {
                    // Vertical first, rounded by (S + 2^(shift − 1) − 1 + RND) >> shift, then
                    // horizontal, rounded by (S + 64 − RND) >> 7.
                    let shift = (SHIFT[fx] + SHIFT[fy]) >> 1;
                    let round = (1 << (shift - 1)) - 1 + rnd;
                    let mut t = [0i32; 4];
                    for (k, tv) in t.iter_mut().enumerate() {
                        let cx = px - 1 + k as i32;
                        let s: i32 = (0..4).map(|m| TAPS[fy][m] * r.at(cx, py - 1 + m as i32)).sum();
                        *tv = (s + round) >> shift;
                    }
                    let s: i32 = (0..4).map(|k| TAPS[fx][k] * t[k]).sum();
                    (s + 64 - rnd) >> 7
                }
            };
            out[j * w + i] = v.clamp(0, 255) as u8;
        }
    }
}

/// Bilinear (8.3.6.5.1): luma in the half-pel bilinear mode and every colour-difference block.
pub fn bilinear(r: &Plane, x: i32, y: i32, mv: [i32; 2], w: usize, h: usize, rnd: i32, out: &mut [u8]) {
    let (ix, iy) = (x + (mv[0] >> 2), y + (mv[1] >> 2));
    let (fx, fy) = (mv[0] & 3, mv[1] & 3);
    for j in 0..h {
        for i in 0..w {
            let (px, py) = (ix + i as i32, iy + j as i32);
            let v = if fx == 0 && fy == 0 {
                r.at(px, py)
            } else {
                let (a, b, c, d) = (r.at(px, py), r.at(px + 1, py), r.at(px, py + 1), r.at(px + 1, py + 1));
                ((4 - fx) * (4 - fy) * a + fx * (4 - fy) * b + (4 - fx) * fy * c + fx * fy * d + 8 - rnd) >> 4
            };
            out[j * w + i] = v as u8;
        }
    }
}

const T8: [[i32; 8]; 8] = [
    [12, 12, 12, 12, 12, 12, 12, 12],
    [16, 15, 9, 4, -4, -9, -15, -16],
    [16, 6, -6, -16, -16, -6, 6, 16],
    [15, -4, -16, -9, 9, 16, 4, -15],
    [12, -12, -12, 12, 12, -12, -12, 12],
    [9, -16, 4, 15, -15, -4, 16, -9],
    [6, -16, 16, -6, -6, 16, -16, 6],
    [4, -9, 15, -16, 16, -15, 9, -4],
];
const T4: [[i32; 4]; 4] = [[17, 17, 17, 17], [22, 10, -10, -22], [17, -17, -17, 17], [10, -22, 22, -10]];

fn basis(n: usize, k: usize, x: usize) -> i32 {
    if n == 8 { T8[k][x] } else { T4[k][x] }
}

/// The N × M inverse transform (Figure 159): `d` holds M rows of N coefficients; rows
/// `E = (D·T_N + 4) >> 3`, columns `R = (T_Mᵀ·E + C_M·1 + 64) >> 7`, C_8 adding 1 to the
/// lower four rows, C_4 nothing.
pub fn inverse(d: &[i32], n: usize, m: usize) -> Vec<i32> {
    let mut e = vec![0i32; n * m];
    for r in 0..m {
        for x in 0..n {
            let s: i32 = (0..n).map(|k| d[r * n + k] * basis(n, k, x)).sum();
            e[r * n + x] = (s + 4) >> 3;
        }
    }
    let mut out = vec![0i32; n * m];
    for y in 0..m {
        let c = (m == 8 && y >= 4) as i32;
        for x in 0..n {
            let s: i32 = (0..m).map(|k| basis(m, k, y) * e[k * n + x]).sum();
            out[y * n + x] = (s + c + 64) >> 7;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Vec<u8> {
        (0..16 * 16).map(|i| ((i % 16) * 10 + (i / 16) * 3) as u8).collect()
    }

    #[test]
    fn whole_pel_vectors_copy_and_edges_repeat() {
        let px = ramp();
        let r = Plane { px: &px, stride: 16, width: 16, height: 16 };
        let mut out = [0u8; 4];
        bicubic(&r, 2, 2, [4, 8], 2, 2, 0, &mut out);
        assert_eq!(out, [px[4 * 16 + 3], px[4 * 16 + 4], px[5 * 16 + 3], px[5 * 16 + 4]]);
        bilinear(&r, 0, 0, [-40, -40], 2, 2, 0, &mut out);
        assert_eq!(out, [px[0]; 4]);
    }

    #[test]
    fn filters_keep_flat_areas_flat() {
        let px = vec![77u8; 16 * 16];
        let r = Plane { px: &px, stride: 16, width: 16, height: 16 };
        for mv in [[1, 0], [2, 0], [3, 3], [1, 2], [2, 1], [0, 3]] {
            for rnd in [0, 1] {
                let mut out = [0u8; 16];
                bicubic(&r, 4, 4, mv, 4, 4, rnd, &mut out);
                assert_eq!(out, [77; 16], "bicubic {mv:?} rnd {rnd}");
                bilinear(&r, 4, 4, mv, 4, 4, rnd, &mut out);
                assert_eq!(out, [77; 16], "bilinear {mv:?} rnd {rnd}");
            }
        }
    }

    #[test]
    fn small_transforms_spread_a_dc_flat() {
        for (n, m) in [(8, 4), (4, 8), (4, 4)] {
            let mut d = vec![0i32; n * m];
            d[0] = 64;
            let r = inverse(&d, n, m);
            assert!(r.iter().all(|&v| v == r[0]), "{n}x{m}: {r:?}");
        }
    }
}
