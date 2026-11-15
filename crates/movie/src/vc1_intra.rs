//! VC-1 intra picture decoding (SMPTE ST 421 8.1, advanced profile, progressive): macroblocks of
//! six 8 × 8 blocks, the DC coded against a neighbour, the AC as run / level / last with three
//! escape modes, AC prediction from the block above or left, dequantization and the inverse
//! transform (Annex A). Overlap smoothing and the loop filter run on the result afterwards.

use crate::bitplane::Bitplane;
use crate::bits::{Bits, Vlc};
use crate::vc1::{IntraHeader, Error};
use crate::vc1_tables as t;

/// A decoded picture, 4:2:0, its planes the macroblock-aligned size.
#[derive(Clone)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl Picture {
    pub fn new(cols: usize, rows: usize) -> Self {
        let (w, h) = (cols * 16, rows * 16);
        Picture { width: w, height: h, y: vec![0; w * h], u: vec![128; w * h / 4], v: vec![128; w * h / 4] }
    }
}

/// One AC coding set: its code, run / level rows, deltas.
struct AcSet {
    vlc: Vlc,
    run_level: Vec<(u8, u8)>,
    last_from: usize,
    delta_level: [&'static [u8]; 2],
    delta_run: [&'static [u8]; 2],
}

impl AcSet {
    #[allow(clippy::too_many_arguments)]
    fn new(vlc: &[(u32, u8)], rl0: &[(u8, u8)], rl1: &[(u8, u8)], dl0: &'static [u8], dl1: &'static [u8], dr0: &'static [u8], dr1: &'static [u8]) -> Self {
        AcSet { vlc: Vlc::new(vlc), run_level: rl0.iter().chain(rl1).copied().collect(), last_from: rl0.len(), delta_level: [dl0, dl1], delta_run: [dr0, dr1] }
    }
}

/// The tables an intra picture reads with, built once.
pub struct IntraTables {
    cbpcy: Vlc,
    /// Low-motion luma, chroma; high-motion luma, chroma.
    dc: [Vlc; 4],
    /// Intra sets: high rate, high motion, mid rate, low motion; then the inter sets so.
    intra: [AcSet; 4],
    inter: [AcSet; 4],
    escape_mode: Vlc,
    level_size_conservative: Vlc,
    level_size_efficient: Vlc,
}

macro_rules! set {
    ($p:ident) => {
        paste_set!($p)
    };
}

macro_rules! paste_set {
    (HIGH_RATE_INTRA) => { AcSet::new(&t::HIGH_RATE_INTRA_VLC, &t::HIGH_RATE_INTRA_RUN_LEVEL_0, &t::HIGH_RATE_INTRA_RUN_LEVEL_1, &t::HIGH_RATE_INTRA_DELTA_LEVEL_0, &t::HIGH_RATE_INTRA_DELTA_LEVEL_1, &t::HIGH_RATE_INTRA_DELTA_RUN_0, &t::HIGH_RATE_INTRA_DELTA_RUN_1) };
    (HIGH_MOTION_INTRA) => { AcSet::new(&t::HIGH_MOTION_INTRA_VLC, &t::HIGH_MOTION_INTRA_RUN_LEVEL_0, &t::HIGH_MOTION_INTRA_RUN_LEVEL_1, &t::HIGH_MOTION_INTRA_DELTA_LEVEL_0, &t::HIGH_MOTION_INTRA_DELTA_LEVEL_1, &t::HIGH_MOTION_INTRA_DELTA_RUN_0, &t::HIGH_MOTION_INTRA_DELTA_RUN_1) };
    (MID_RATE_INTRA) => { AcSet::new(&t::MID_RATE_INTRA_VLC, &t::MID_RATE_INTRA_RUN_LEVEL_0, &t::MID_RATE_INTRA_RUN_LEVEL_1, &t::MID_RATE_INTRA_DELTA_LEVEL_0, &t::MID_RATE_INTRA_DELTA_LEVEL_1, &t::MID_RATE_INTRA_DELTA_RUN_0, &t::MID_RATE_INTRA_DELTA_RUN_1) };
    (LOW_MOTION_INTRA) => { AcSet::new(&t::LOW_MOTION_INTRA_VLC, &t::LOW_MOTION_INTRA_RUN_LEVEL_0, &t::LOW_MOTION_INTRA_RUN_LEVEL_1, &t::LOW_MOTION_INTRA_DELTA_LEVEL_0, &t::LOW_MOTION_INTRA_DELTA_LEVEL_1, &t::LOW_MOTION_INTRA_DELTA_RUN_0, &t::LOW_MOTION_INTRA_DELTA_RUN_1) };
    (HIGH_RATE_INTER) => { AcSet::new(&t::HIGH_RATE_INTER_VLC, &t::HIGH_RATE_INTER_RUN_LEVEL_0, &t::HIGH_RATE_INTER_RUN_LEVEL_1, &t::HIGH_RATE_INTER_DELTA_LEVEL_0, &t::HIGH_RATE_INTER_DELTA_LEVEL_1, &t::HIGH_RATE_INTER_DELTA_RUN_0, &t::HIGH_RATE_INTER_DELTA_RUN_1) };
    (HIGH_MOTION_INTER) => { AcSet::new(&t::HIGH_MOTION_INTER_VLC, &t::HIGH_MOTION_INTER_RUN_LEVEL_0, &t::HIGH_MOTION_INTER_RUN_LEVEL_1, &t::HIGH_MOTION_INTER_DELTA_LEVEL_0, &t::HIGH_MOTION_INTER_DELTA_LEVEL_1, &t::HIGH_MOTION_INTER_DELTA_RUN_0, &t::HIGH_MOTION_INTER_DELTA_RUN_1) };
    (MID_RATE_INTER) => { AcSet::new(&t::MID_RATE_INTER_VLC, &t::MID_RATE_INTER_RUN_LEVEL_0, &t::MID_RATE_INTER_RUN_LEVEL_1, &t::MID_RATE_INTER_DELTA_LEVEL_0, &t::MID_RATE_INTER_DELTA_LEVEL_1, &t::MID_RATE_INTER_DELTA_RUN_0, &t::MID_RATE_INTER_DELTA_RUN_1) };
    (LOW_MOTION_INTER) => { AcSet::new(&t::LOW_MOTION_INTER_VLC, &t::LOW_MOTION_INTER_RUN_LEVEL_0, &t::LOW_MOTION_INTER_RUN_LEVEL_1, &t::LOW_MOTION_INTER_DELTA_LEVEL_0, &t::LOW_MOTION_INTER_DELTA_LEVEL_1, &t::LOW_MOTION_INTER_DELTA_RUN_0, &t::LOW_MOTION_INTER_DELTA_RUN_1) };
}

impl Default for IntraTables {
    fn default() -> Self {
        IntraTables {
            cbpcy: Vlc::new(&t::I_CBPCY),
            dc: [Vlc::new(&t::DC_LOW_LUMA), Vlc::new(&t::DC_LOW_CHROMA), Vlc::new(&t::DC_HIGH_LUMA), Vlc::new(&t::DC_HIGH_CHROMA)],
            intra: [set!(HIGH_RATE_INTRA), set!(HIGH_MOTION_INTRA), set!(MID_RATE_INTRA), set!(LOW_MOTION_INTRA)],
            inter: [set!(HIGH_RATE_INTER), set!(HIGH_MOTION_INTER), set!(MID_RATE_INTER), set!(LOW_MOTION_INTER)],
            // Table 58: mode 1 1b, mode 2 01b, mode 3 00b.
            escape_mode: Vlc::new(&[(1, 1), (1, 2), (0, 2)]),
            // Table 59: sizes 1 to 7 as 001b to 111b, 8 to 11 as 00000b to 00011b (by size − 1).
            level_size_conservative: Vlc::new(&[(1, 3), (2, 3), (3, 3), (4, 3), (5, 3), (6, 3), (7, 3), (0, 5), (1, 5), (2, 5), (3, 5)]),
            // Table 60: sizes 2 to 8 (by size − 2).
            level_size_efficient: Vlc::new(&[(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6), (0, 6)]),
        }
    }
}

/// Annex A's 8-point inverse transform matrix (rows are the basis functions).
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

/// The 8 × 8 inverse transform (Figure 159): rows `E = (D·T8 + 4) >> 3`, then columns
/// `R = (T8ᵀ·E + C8·1 + 64) >> 7`, C8 adding 1 to the lower four rows.
pub fn inverse_8x8(d: &[i32; 64]) -> [i32; 64] {
    let mut e = [0i32; 64];
    for r in 0..8 {
        for x in 0..8 {
            let mut s = 0;
            for k in 0..8 {
                s += d[r * 8 + k] * T8[k][x];
            }
            e[r * 8 + x] = (s + 4) >> 3;
        }
    }
    let mut out = [0i32; 64];
    for y in 0..8 {
        for x in 0..8 {
            let mut s = 0;
            for k in 0..8 {
                s += T8[k][y] * e[k * 8 + x];
            }
            out[y * 8 + x] = (s + (y >= 4) as i32 + 64) >> 7;
        }
    }
    out
}

/// What a decoded block leaves for its neighbours: its quantized DC, first row and first
/// column of AC (after prediction), and whether it had AC coded (for CBPCY prediction).
#[derive(Clone, Copy, Default)]
struct Neighbour {
    dc: i32,
    row: [i32; 7],
    col: [i32; 7],
    coded: bool,
}

fn dc_step(mquant: i32) -> i32 {
    match mquant {
        1 | 2 => 2 * mquant,
        3 | 4 => 8,
        _ => mquant / 2 + 6,
    }
}

/// Decodes an intra picture's macroblocks into `out`.
pub fn decode(tables: &IntraTables, header: &IntraHeader, data: &[u8], cols: usize, rows: usize, out: &mut Picture) -> Result<(), Error> {
    let mut b = Bits::new(data);
    b.skip(header.mb_start as u32);
    let mquant = header.pquant as i32;
    let step = dc_step(mquant);
    let double_quant = 2 * mquant + header.half_qp as i32;
    let uniform = header.uniform;
    // Coding sets: index 0 is high rate (PQINDEX ≤ 8) or low motion.
    let pick = |index: u8| -> usize {
        match index {
            0 if header.pqindex <= 8 => 0,
            0 => 3,
            1 => 1,
            _ => 2,
        }
    };
    let luma_set = &tables.intra[pick(header.ac_set_luma)];
    let chroma_set = &tables.inter[pick(header.ac_set_chroma)];
    let dc_tables = if header.dc_high { (&tables.dc[2], &tables.dc[3]) } else { (&tables.dc[0], &tables.dc[1]) };
    // Neighbour state: luma blocks in a (2·cols) × (2·rows) grid, chroma in cols × rows each.
    let (lw, lh) = (2 * cols, 2 * rows);
    let mut luma = vec![Neighbour::default(); lw * lh];
    let mut cb = vec![Neighbour::default(); cols * rows];
    let mut cr = vec![Neighbour::default(); cols * rows];
    let mut mode3: Option<(u32, u32)> = None;
    for my in 0..rows {
        for mx in 0..cols {
            // CBPCY and its prediction from the neighbouring luma blocks' coded status.
            let decoded = tables.cbpcy.read(&mut b).ok_or_else(|| Error(format!("CBPCY at macroblock {mx},{my}")))? as u32;
            let coded = |bx: isize, by: isize| -> bool { bx >= 0 && by >= 0 && luma[by as usize * lw + bx as usize].coded };
            let (bx, by) = (2 * mx as isize, 2 * my as isize);
            let (l1, l3, lt3, t2, t3) = (coded(bx - 1, by), coded(bx - 1, by + 1), coded(bx - 1, by - 1), coded(bx, by - 1), coded(bx + 1, by - 1));
            let p0 = (if lt3 == t2 { l1 } else { t2 }) ^ ((decoded >> 5) & 1 != 0);
            let p1 = (if t2 == t3 { p0 } else { t3 }) ^ ((decoded >> 4) & 1 != 0);
            let p2 = (if l1 == p0 { l3 } else { p0 }) ^ ((decoded >> 3) & 1 != 0);
            let p3 = (if p0 == p1 { p2 } else { p1 }) ^ ((decoded >> 2) & 1 != 0);
            let cbp = [p0, p1, p2, p3, decoded & 2 != 0, decoded & 1 != 0];
            let ac_pred = match &header.ac_pred {
                Bitplane::Raw => b.bit(),
                p => p.get(mx, my, cols).unwrap(),
            };
            if header.cond_over == Some(2) && matches!(header.over_flags, Some(Bitplane::Raw)) {
                b.bit(); // OVERFLAGMB: kept for overlap smoothing, not done yet.
            }
            for k in 0..6 {
                let (grid, gw, gx, gy) = match k {
                    0..=3 => (&mut luma, lw, 2 * mx + (k & 1), 2 * my + (k >> 1)),
                    4 => (&mut cb, cols, mx, my),
                    _ => (&mut cr, cols, mx, my),
                };
                let (dc_vlc, set) = if k < 4 { (dc_tables.0, luma_set) } else { (dc_tables.1, chroma_set) };
                // DC differential (Figure 37).
                let mut diff = dc_vlc.read(&mut b).ok_or_else(|| Error(format!("DC at macroblock {mx},{my} block {k}")))? as i32;
                if diff != 0 {
                    if diff == 119 {
                        diff = b.read(match mquant { 1 => 10, 2 => 9, _ => 8 }) as i32;
                    } else if mquant == 1 {
                        diff = diff * 4 + b.read(2) as i32 - 3;
                    } else if mquant == 2 {
                        diff = diff * 2 + b.read(1) as i32 - 1;
                    }
                    if b.bit() {
                        diff = -diff;
                    }
                }
                // DC prediction (Figure 40): A above, B above-left, C left.
                let at = |x: usize, y: usize| grid[y * gw + x];
                let a = (gy > 0).then(|| at(gx, gy - 1));
                let c = (gx > 0).then(|| at(gx - 1, gy));
                let (top, predictor) = match (a, c) {
                    (Some(a), Some(c)) => {
                        let bdc = at(gx - 1, gy - 1).dc;
                        if (bdc - a.dc).abs() <= (bdc - c.dc).abs() { (false, c.dc) } else { (true, a.dc) }
                    }
                    (Some(a), None) => (true, a.dc),
                    (None, Some(c)) => (false, c.dc),
                    (None, None) => (false, 0),
                };
                let dc = predictor + diff;
                // AC run / level / last into the scan chosen by the prediction.
                let scan: &[u8; 64] = if !ac_pred { &t::INTRA_NORMAL_SCAN } else if top { &t::INTRA_HORIZONTAL_SCAN } else { &t::INTRA_VERTICAL_SCAN };
                let mut q = [0i32; 64];
                if cbp[k] {
                    let escape = set.run_level.len();
                    let mut pos = 1usize;
                    loop {
                        let index = set.vlc.read(&mut b).ok_or_else(|| Error(format!("AC at macroblock {mx},{my} block {k}")))?;
                        let (run, level, last);
                        if index != escape {
                            let (r, l) = set.run_level[index];
                            last = index >= set.last_from;
                            run = r as usize;
                            level = if b.bit() { -(l as i32) } else { l as i32 };
                        } else {
                            let m = tables.escape_mode.read(&mut b).ok_or_else(|| Error("escape mode".into()))?;
                            if m < 2 {
                                let i2 = set.vlc.read(&mut b).ok_or_else(|| Error("escaped AC".into()))?;
                                if i2 == escape {
                                    return Err(Error("escape inside an escape".into()));
                                }
                                let (mut r, mut l) = set.run_level[i2];
                                last = i2 >= set.last_from;
                                let li = last as usize;
                                if m == 0 {
                                    l += set.delta_level[li].get(r as usize).copied().ok_or_else(|| Error("escape mode 1 run past its table".into()))?;
                                } else {
                                    r += set.delta_run[li].get(l as usize - 1).copied().ok_or_else(|| Error("escape mode 2 level past its table".into()))? + 1;
                                }
                                run = r as usize;
                                level = if b.bit() { -(l as i32) } else { l as i32 };
                            } else {
                                last = b.bit();
                                let (lsize, rsize) = match mode3 {
                                    Some(s) => s,
                                    None => {
                                        let l = if mquant <= 7 {
                                            tables.level_size_conservative.read(&mut b).ok_or_else(|| Error("ESCLVLSZ".into()))? as u32 + 1
                                        } else {
                                            tables.level_size_efficient.read(&mut b).ok_or_else(|| Error("ESCLVLSZ".into()))? as u32 + 2
                                        };
                                        let r = 3 + b.read(2);
                                        mode3 = Some((l, r));
                                        (l, r)
                                    }
                                };
                                run = b.read(rsize) as usize;
                                let negative = b.bit();
                                let l = b.read(lsize) as i32;
                                level = if negative { -l } else { l };
                            }
                        }
                        pos += run;
                        if pos >= 64 {
                            return Err(Error(format!("AC run past the block at macroblock {mx},{my} block {k}")));
                        }
                        q[scan[pos] as usize] = level;
                        pos += 1;
                        if last {
                            break;
                        }
                    }
                }
                // AC prediction (Figure 48) from the block in the DC's direction.
                if ac_pred {
                    if top {
                        if let Some(a) = a {
                            for i in 0..7 {
                                q[1 + i] += a.row[i];
                            }
                        }
                    } else if let Some(c) = c {
                        for i in 0..7 {
                            q[(1 + i) * 8] += c.col[i];
                        }
                    }
                }
                let mut n = Neighbour { dc, coded: cbp[k], ..Default::default() };
                for i in 0..7 {
                    n.row[i] = q[1 + i];
                    n.col[i] = q[(1 + i) * 8];
                }
                grid[gy * gw + gx] = n;
                // Dequantization (8.1.3.3, 8.1.3.8).
                let mut d = [0i32; 64];
                d[0] = dc * step;
                for i in 1..64 {
                    let v = q[i];
                    if v != 0 {
                        d[i] = v * double_quant + if uniform { 0 } else { v.signum() * mquant };
                    }
                }
                let r = inverse_8x8(&d);
                let (plane, stride, px, py) = match k {
                    0..=3 => (&mut out.y, out.width, mx * 16 + (k & 1) * 8, my * 16 + (k >> 1) * 8),
                    4 => (&mut out.u, out.width / 2, mx * 8, my * 8),
                    _ => (&mut out.v, out.width / 2, mx * 8, my * 8),
                };
                for y in 0..8 {
                    for x in 0..8 {
                        plane[(py + y) * stride + px + x] = (r[y * 8 + x] + 128).clamp(0, 255) as u8;
                    }
                }
            }
        }
    }
    if b.over() {
        return Err(Error("macroblocks run out".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lone DC: the transform spreads it flat (DC·12·12 / 1024, rounded).
    #[test]
    fn a_dc_alone_is_flat() {
        let mut d = [0i32; 64];
        d[0] = 64;
        let r = inverse_8x8(&d);
        assert!(r.iter().all(|&v| v == r[0]), "{r:?}");
        assert_eq!(r[0], ((64 * 12 + 4) >> 3) * 12 / 128);
    }
}

#[cfg(test)]
mod movie_tests {
    use super::*;
    use barrel_formats::asf::{Asf, Kind};

    /// Decodes `avantitle`'s key frame `BARREL_KEY` (default 0; `BARREL_MOVIE`) to `BARREL_YUV` (I420, 1920 × 1080).
    #[test]
    #[ignore]
    fn first_frame_to_yuv() {
        let (Ok(path), Ok(out)) = (std::env::var("BARREL_MOVIE"), std::env::var("BARREL_YUV")) else { return };
        let bytes = std::fs::read(path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let video = asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).unwrap();
        let Kind::Video { extra, .. } = &video.kind else { unreachable!() };
        let (s, e) = crate::vc1::headers(extra).unwrap();
        let nth: usize = std::env::var("BARREL_KEY").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let o = asf.objects().unwrap().into_iter().filter(|o| o.stream == video.number && o.key).nth(nth).unwrap();
        let data = crate::bits::unescape(&o.data);
        let h = s.intra_header(&e, &data, &crate::bitplane::Tables::default()).unwrap();
        let (cols, rows) = s.macroblocks(&e);
        let mut p = Picture::new(cols, rows);
        let t0 = std::time::Instant::now();
        decode(&IntraTables::default(), &h, &data, cols, rows, &mut p).unwrap();
        eprintln!("decoded in {:?} (pqindex {}, condover {:?}, sets {} {}, dc high {})", t0.elapsed(), h.pqindex, h.cond_over, h.ac_set_luma, h.ac_set_chroma, h.dc_high);
        let mut f = Vec::new();
        for y in 0..1080 {
            f.extend_from_slice(&p.y[y * p.width..y * p.width + 1920]);
        }
        for plane in [&p.u, &p.v] {
            for y in 0..540 {
                f.extend_from_slice(&plane[y * p.width / 2..y * p.width / 2 + 960]);
            }
        }
        std::fs::write(out, f).unwrap();
    }
}
