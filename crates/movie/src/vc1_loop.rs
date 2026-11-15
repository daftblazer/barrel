//! VC-1's in-loop deblocking filter (SMPTE ST 421 8.6): segments of four pixel pairs across a
//! block edge, the third pair deciding whether the other three are filtered.

/// Filters one pixel pair: `p` holds P1 to P8 across the edge (P4 | P5), read and written
/// through `at`. Returns whether the segment's other pairs are to be filtered (Figure 82 when
/// `third`, else Figure 83, which filters the same way and has no say).
#[inline]
fn pair(px: &mut [u8], base: usize, step: usize, pquant: i32) -> bool {
    let p = |k: usize| px[base + k * step] as i32;
    let (p1, p2, p3, p4, p5, p6, p7, p8) = (p(0), p(1), p(2), p(3), p(4), p(5), p(6), p(7));
    let a0 = (2 * (p3 - p6) - 5 * (p4 - p5) + 4) >> 3;
    if a0.abs() >= pquant {
        return false;
    }
    let a1 = (2 * (p1 - p4) - 5 * (p2 - p3) + 4) >> 3;
    let a2 = (2 * (p5 - p8) - 5 * (p6 - p7) + 4) >> 3;
    let a3 = a1.abs().min(a2.abs());
    if a3 >= a0.abs() {
        return false;
    }
    // C division: truncation toward zero.
    let mut d = 5 * (a0.signum() * a3 - a0) / 8;
    let clip = (p4 - p5) / 2;
    if clip == 0 {
        return false;
    }
    if clip > 0 {
        d = d.clamp(0, clip);
    } else {
        d = d.clamp(clip, 0);
    }
    px[base + 3 * step] = (p4 - d) as u8;
    px[base + 4 * step] = (p5 + d) as u8;
    true
}

/// Filters a 4-pixel segment across an edge: `first` is the index of its first pair's P1,
/// `along` the step from one pair to the next, `across` the step from P1 to P2.
#[inline]
fn segment(px: &mut [u8], first: usize, along: usize, across: usize, pquant: i32) {
    if pair(px, first + 2 * along, across, pquant) {
        for k in [0, 1, 3] {
            pair(px, first + k * along, across, pquant);
        }
    }
}

/// An I, BI or B picture's filter on one plane (8.6.1): every 8 × 8 block edge inside it, the
/// horizontal edges first.
pub fn intra_plane(px: &mut [u8], width: usize, height: usize, pquant: i32) {
    for y in (8..height).step_by(8) {
        for x in (0..width).step_by(4) {
            segment(px, (y - 4) * width + x, 1, width, pquant);
        }
    }
    for x in (8..width).step_by(8) {
        for y in (0..height).step_by(4) {
            segment(px, y * width + x - 4, width, 1, pquant);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small step across an edge is smoothed; a large one (an edge in the picture) is kept.
    #[test]
    fn smooths_small_steps_and_keeps_edges() {
        let row = |l: u8, r: u8| -> Vec<u8> { (0..16).map(|x| if x < 8 { l } else { r }).collect() };
        let mut small: Vec<u8> = (0..8).flat_map(|_| row(100, 104)).collect();
        intra_plane(&mut small, 16, 8, 8);
        assert!(small[7] > 100 && small[8] < 104, "{:?}", &small[..16]);
        let mut big: Vec<u8> = (0..8).flat_map(|_| row(20, 200)).collect();
        intra_plane(&mut big, 16, 8, 8);
        assert_eq!((big[7], big[8]), (20, 200));
    }
}
