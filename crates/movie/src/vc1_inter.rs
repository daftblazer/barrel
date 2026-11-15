//! VC-1 progressive P and B pictures (SMPTE ST 421 8.3 and 8.4). P: 1-MV, 4-MV and skipped
//! macroblocks, motion compensated from the previous anchor. B: direct, forward, backward and
//! interpolated macroblocks between the two anchors around them. Both: inter blocks of 8 × 8,
//! 8 × 4, 4 × 8 or 4 × 4 transforms and intra blocks among them.

use crate::bitplane::Bitplane;
use crate::bits::{Bits, Vlc};
use crate::vc1::{Error, InterHeader};
use crate::vc1_intra::{dc_step, inverse_8x8, read_coefficients, read_dc_diff, AcSet, IntraTables, Picture};
use crate::vc1_loop::Cell;
use crate::vc1_mc::{bicubic, bilinear, inverse, Plane};
use crate::vc1_mv::{chroma, four_mv_luma_for_chroma, hybrid_needed, hybrid_pick, preliminary, read_mvdata, smod, Candidate, MvData, RANGES};
use crate::vc1_small::{Coded, Halves, MvMode, Transform, SUBBLKPAT2, SUBBLKPAT4, TTBLK, TTMB};
use crate::vc1_tables as t;

/// The tables P and B pictures read with, built once (beside [`IntraTables`]).
pub struct PTables {
    cbpcy: [Vlc; 4],
    mv: [Vlc; 4],
    ttmb: [Coded<(Transform, Halves, bool)>; 3],
    ttblk: [Coded<(Transform, Halves)>; 3],
    subblk4: [Coded<u8>; 3],
    subblk2: Coded<Halves>,
}

impl Default for PTables {
    fn default() -> Self {
        PTables {
            cbpcy: [Vlc::new(&t::PB_CBPCY_0), Vlc::new(&t::PB_CBPCY_1), Vlc::new(&t::PB_CBPCY_2), Vlc::new(&t::PB_CBPCY_3)],
            mv: [Vlc::new(&t::MV_DIFF_0), Vlc::new(&t::MV_DIFF_1), Vlc::new(&t::MV_DIFF_2), Vlc::new(&t::MV_DIFF_3)],
            ttmb: [0, 1, 2].map(|k| Coded::new(TTMB[k], |r| (r.0, (r.1, r.2, r.3)))),
            ttblk: [0, 1, 2].map(|k| Coded::new(TTBLK[k], |r| (r.0, (r.1, r.2)))),
            subblk4: [0, 1, 2].map(|k| Coded::new(SUBBLKPAT4[k], |r| *r)),
            subblk2: Coded::new(SUBBLKPAT2, |r| *r),
        }
    }
}

/// An intra block's prediction state (as in I pictures): quantized DC, first row and column.
#[derive(Clone, Copy, Default)]
struct IntraState {
    dc: i32,
    row: [i32; 7],
    col: [i32; 7],
}

/// Figure 67: the luma and colour-difference lookup tables of intensity compensation from
/// LUMSCALE and LUMSHIFT.
pub fn intensity_luts(lumscale: u8, lumshift: u8) -> ([u8; 256], [u8; 256]) {
    let (scale, shift) = (lumscale as i32, lumshift as i32);
    let (i_scale, i_shift) = if scale == 0 {
        (-64, 255 * 64 - shift * 2 * 64 + if shift > 31 { 128 * 64 } else { 0 })
    } else {
        (scale + 32, if shift > 31 { shift * 64 - 64 * 64 } else { shift * 64 })
    };
    let (mut y, mut uv) = ([0u8; 256], [0u8; 256]);
    for i in 0..256i32 {
        y[i as usize] = ((i_scale * i + i_shift + 32) >> 6).clamp(0, 255) as u8;
        uv[i as usize] = ((i_scale * (i - 128) + 128 * 64 + 32) >> 6).clamp(0, 255) as u8;
    }
    (y, uv)
}

/// `reference` remapped by intensity compensation (8.3.8).
pub fn remapped(reference: &Picture, lumscale: u8, lumshift: u8) -> Picture {
    let (luty, lutuv) = intensity_luts(lumscale, lumshift);
    Picture {
        width: reference.width,
        height: reference.height,
        y: reference.y.iter().map(|&p| luty[p as usize]).collect(),
        u: reference.u.iter().map(|&p| lutuv[p as usize]).collect(),
        v: reference.v.iter().map(|&p| lutuv[p as usize]).collect(),
    }
}

/// What a P picture leaves for the loop filter and for the B pictures before it.
pub struct PResult {
    /// The loop filter's cells: luma, Cb, Cr.
    pub cells: [Vec<Cell>; 3],
    /// Each macroblock's vector for direct mode (8.4.5.4): the 1-MV vector, the 4-MV median,
    /// zero for intra.
    pub colocated: Vec<[i32; 2]>,
}

/// A picture's block coding state, shared by P and B macroblocks: quantizer, tables, the
/// intra blocks' prediction state and escape mode 3's sizes.
struct Blocks<'a> {
    it: &'a IntraTables,
    pt: &'a PTables,
    mquant: i32,
    double_quant: i32,
    uniform: bool,
    rate: usize,
    intra_set: &'a AcSet,
    inter_set: &'a AcSet,
    dc_vlc: (&'a Vlc, &'a Vlc),
    frame_transform: Option<u8>,
    cols: usize,
    /// Intra prediction state by luma block (2 × cols across), and by Cb / Cr block.
    intra_y: Vec<Option<IntraState>>,
    intra_c: [Vec<Option<IntraState>>; 2],
    mode3: Option<(u32, u32)>,
}

/// An inter block's transform layout for the loop filter: middle edges and the sub-blocks with
/// coefficients (top-left, top-right, bottom-left, bottom-right).
#[derive(Default)]
struct Layout {
    split_h: bool,
    split_v: bool,
    nz: [bool; 4],
}

impl<'a> Blocks<'a> {
    fn new(it: &'a IntraTables, pt: &'a PTables, h: &InterHeader, cols: usize, rows: usize) -> Self {
        let set_index = match h.ac_set {
            0 if h.pqindex <= 8 => 0,
            0 => 3,
            1 => 1,
            _ => 2,
        };
        let mquant = h.pquant as i32;
        Blocks {
            it,
            pt,
            mquant,
            double_quant: 2 * mquant + h.half_qp as i32,
            uniform: h.uniform,
            rate: if h.pquant < 5 { 0 } else if h.pquant < 13 { 1 } else { 2 },
            intra_set: &it.intra[set_index],
            inter_set: &it.inter[set_index],
            dc_vlc: if h.dc_high { (&it.dc[2], &it.dc[3]) } else { (&it.dc[0], &it.dc[1]) },
            frame_transform: h.frame_transform,
            cols,
            intra_y: vec![None; 4 * cols * rows],
            intra_c: [vec![None; cols * rows], vec![None; cols * rows]],
            mode3: None,
        }
    }

    fn dequant(&self, v: i32) -> i32 {
        if v == 0 { 0 } else { v * self.double_quant + if self.uniform { 0 } else { v.signum() * self.mquant } }
    }

    /// The intra state of block `k`'s grid at (x, y) (luma: block grid; Cb / Cr: macroblocks).
    fn state(&self, k: usize, x: isize, y: isize) -> Option<IntraState> {
        if x < 0 || y < 0 {
            return None;
        }
        let (x, y) = (x as usize, y as usize);
        match k {
            0..=3 => self.intra_y.get(y * 2 * self.cols + x).copied().flatten(),
            _ => self.intra_c[k - 4][y * self.cols + x],
        }
    }

    fn set_state(&mut self, k: usize, x: usize, y: usize, s: Option<IntraState>) {
        match k {
            0..=3 => self.intra_y[y * 2 * self.cols + x] = s,
            _ => self.intra_c[k - 4][y * self.cols + x] = s,
        }
    }

    /// Whether the block at (x, y) of block `k`'s grid is intra (as decoded so far).
    fn is_intra(&self, k: usize, x: isize, y: isize) -> bool {
        self.state(k, x, y).is_some()
    }

    /// An intra block in a P or B picture (8.3.6.1): DC / AC prediction from intra neighbours
    /// only, the inter 8 × 8 scan; the pixels into `out`.
    fn intra(&mut self, b: &mut Bits, k: usize, gx: usize, gy: usize, coded: bool, ac_pred: bool, out: &mut [u8; 64]) -> Result<(), Error> {
        let is_luma = k < 4;
        let (ix, iy) = (gx as isize, gy as isize);
        let (a, c) = (self.state(k, ix, iy - 1), self.state(k, ix - 1, iy));
        let diff = read_dc_diff(b, if is_luma { self.dc_vlc.0 } else { self.dc_vlc.1 }, self.mquant).ok_or_else(|| Error("DC".into()))?;
        let (top, predictor) = match (a, c) {
            (Some(a), Some(c)) => {
                let bdc = self.state(k, ix - 1, iy - 1).map(|s| s.dc).unwrap_or(0);
                if (bdc - a.dc).abs() <= (bdc - c.dc).abs() { (false, c.dc) } else { (true, a.dc) }
            }
            (Some(a), None) => (true, a.dc),
            (None, Some(c)) => (false, c.dc),
            (None, None) => (false, 0),
        };
        let dc = predictor + diff;
        let mut q = [0i32; 64];
        if coded {
            let set = if is_luma { self.intra_set } else { self.inter_set };
            read_coefficients(b, self.it, set, self.mquant, &mut self.mode3, &t::INTER_8X8_SCAN, 1, &mut q)?;
        }
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
        let mut s = IntraState { dc, ..Default::default() };
        for i in 0..7 {
            s.row[i] = q[1 + i];
            s.col[i] = q[(1 + i) * 8];
        }
        self.set_state(k, gx, gy, Some(s));
        let mut d = [0i32; 64];
        d[0] = dc * dc_step(self.mquant);
        for i in 1..64 {
            d[i] = self.dequant(q[i]);
        }
        let r = inverse_8x8(&d);
        for (o, v) in out.iter_mut().zip(r.iter()) {
            *o = (v + 128).clamp(0, 255) as u8;
        }
        Ok(())
    }

    /// A coded inter block's residual (8.3.6.2 to 8.3.6.4) into `err`: its transform from TTFRM,
    /// the macroblock's TTMB or its own TTBLK, then SUBBLKPAT where it applies.
    fn residual(&mut self, b: &mut Bits, ttmb: Option<(Transform, Halves, bool)>, first_inter: &mut bool, err: &mut [i32; 64]) -> Result<Layout, Error> {
        let (tt, halves) = if let Some(f) = self.frame_transform {
            ([Transform::T8x8, Transform::T8x4, Transform::T4x8, Transform::T4x4][f as usize], None)
        } else if let Some((t, hv, mb_level)) = ttmb {
            if *first_inter {
                (t, Some(hv))
            } else if mb_level {
                (t, None)
            } else {
                let (t2, hv2) = self.pt.ttblk[self.rate].read(b).ok_or_else(|| Error("TTBLK".into()))?;
                (t2, Some(hv2))
            }
        } else {
            (Transform::T8x8, None)
        };
        *first_inter = false;
        let mut sub = |this: &mut Self, b: &mut Bits, n: usize, m: usize, scan: &[u8], ox: usize, oy: usize| -> Result<(), Error> {
            let mut q = vec![0i32; n * m];
            read_coefficients(b, this.it, this.inter_set, this.mquant, &mut this.mode3, scan, 0, &mut q)?;
            for v in q.iter_mut() {
                *v = this.dequant(*v);
            }
            let r = inverse(&q, n, m);
            for y in 0..m {
                for x in 0..n {
                    err[(oy + y) * 8 + ox + x] = r[y * n + x];
                }
            }
            Ok(())
        };
        let mut layout = Layout::default();
        match tt {
            Transform::T8x8 => {
                sub(self, b, 8, 8, &t::INTER_8X8_SCAN, 0, 0)?;
                layout.nz = [true; 4];
            }
            Transform::T8x4 | Transform::T4x8 => {
                let hv = match halves.filter(|&v| v != 0) {
                    Some(v) => v,
                    None => self.pt.subblk2.read(b).ok_or_else(|| Error("SUBBLKPAT".into()))?,
                };
                let (first, second) = (hv & 2 != 0, hv & 1 != 0);
                if tt == Transform::T8x4 {
                    layout.split_h = true;
                    layout.nz = [first, first, second, second];
                } else {
                    layout.split_v = true;
                    layout.nz = [first, second, first, second];
                }
                for j in 0..2 {
                    if hv & (2 >> j) != 0 {
                        if tt == Transform::T8x4 {
                            sub(self, b, 8, 4, &t::INTER_8X4_SCAN, 0, 4 * j)?;
                        } else {
                            sub(self, b, 4, 8, &t::INTER_4X8_SCAN, 4 * j, 0)?;
                        }
                    }
                }
            }
            Transform::T4x4 => {
                let pat = self.pt.subblk4[self.rate].read(b).ok_or_else(|| Error("SUBBLKPAT".into()))?;
                (layout.split_h, layout.split_v) = (true, true);
                layout.nz = std::array::from_fn(|j| pat & (8 >> j) != 0);
                for j in 0..4 {
                    if pat & (8 >> j) != 0 {
                        sub(self, b, 4, 4, &t::INTER_4X4_SCAN, 4 * (j & 1), 4 * (j >> 1))?;
                    }
                }
            }
        }
        Ok(layout)
    }

    /// TTMB, when blocks signal their transforms and the macroblock has a coded inter block.
    fn ttmb(&self, b: &mut Bits, mb_transform: bool, any_coded_inter: bool) -> Result<Option<(Transform, Halves, bool)>, Error> {
        if mb_transform && any_coded_inter {
            Ok(Some(self.pt.ttmb[self.rate].read(b).ok_or_else(|| Error("TTMB".into()))?))
        } else {
            Ok(None)
        }
    }

    fn cbpcy(&self, b: &mut Bits, table: u8) -> Result<[bool; 6], Error> {
        let c = self.pt.cbpcy[table as usize].read(b).ok_or_else(|| Error("CBPCY".into()))?;
        Ok(std::array::from_fn(|k| (c >> (5 - k)) & 1 != 0))
    }
}

/// The three reference planes of a picture, edges repeated past the coded size.
struct Refs<'a> {
    y: Plane<'a>,
    u: Plane<'a>,
    v: Plane<'a>,
}

impl<'a> Refs<'a> {
    fn new(p: &'a Picture, coded: (usize, usize)) -> Self {
        let (cw, ch) = (coded.0.div_ceil(2) as i32, coded.1.div_ceil(2) as i32);
        Refs {
            y: Plane { px: &p.y, stride: p.width, width: coded.0 as i32, height: coded.1 as i32 },
            u: Plane { px: &p.u, stride: p.width / 2, width: cw, height: ch },
            v: Plane { px: &p.v, stride: p.width / 2, width: cw, height: ch },
        }
    }

    /// Block `k`'s 8 × 8 prediction at pixel (x, y): luma at `mv` (bicubic, or bilinear in the
    /// half-pel bilinear mode), colour difference bilinear at the colour-difference `cmv`.
    fn predict(&self, k: usize, x: usize, y: usize, mv: [i32; 2], cmv: [i32; 2], bilinear_luma: bool, rnd: i32, out: &mut [u8; 64]) {
        match k {
            0..=3 if bilinear_luma => bilinear(&self.y, x as i32, y as i32, mv, 8, 8, rnd, out),
            0..=3 => bicubic(&self.y, x as i32, y as i32, mv, 8, 8, rnd, out),
            4 => bilinear(&self.u, x as i32, y as i32, cmv, 8, 8, rnd, out),
            _ => bilinear(&self.v, x as i32, y as i32, cmv, 8, 8, rnd, out),
        }
    }
}

/// Block `k` of macroblock (mx, my): its top-left pixel in its plane.
fn block_pos(k: usize, mx: usize, my: usize) -> (usize, usize) {
    match k {
        0..=3 => (mx * 16 + (k & 1) * 8, my * 16 + (k >> 1) * 8),
        _ => (mx * 8, my * 8),
    }
}

/// Writes block `k` of macroblock (mx, my).
fn put(out: &mut Picture, k: usize, mx: usize, my: usize, px: &[u8; 64]) {
    let (x, y) = block_pos(k, mx, my);
    let (dst, stride) = match k {
        0..=3 => (&mut out.y, out.width),
        4 => (&mut out.u, out.width / 2),
        _ => (&mut out.v, out.width / 2),
    };
    for j in 0..8 {
        dst[(y + j) * stride + x..(y + j) * stride + x + 8].copy_from_slice(&px[j * 8..j * 8 + 8]);
    }
}

fn add(pred: &[u8; 64], err: &[i32; 64]) -> [u8; 64] {
    std::array::from_fn(|i| (pred[i] as i32 + err[i]).clamp(0, 255) as u8)
}

fn bitplane_bit(b: &mut Bits, p: &Bitplane, mx: usize, my: usize, cols: usize) -> bool {
    match p {
        Bitplane::Raw => b.bit(),
        p => p.get(mx, my, cols).unwrap(),
    }
}

/// Decodes a P picture from `reference` into `out`. `coded` is the coded size (the reference
/// repeats past it); `fast_uvmc` is the entry point's FASTUVMC.
#[allow(clippy::too_many_arguments)]
pub fn decode(it: &IntraTables, pt: &PTables, h: &InterHeader, data: &[u8], cols: usize, rows: usize, coded: (usize, usize), fast_uvmc: bool, reference: &Picture, out: &mut Picture) -> Result<PResult, Error> {
    // Intensity compensation (8.3.8): the reference remapped through LUTs first.
    let remap;
    let reference = match h.intensity {
        Some((scale, shift)) => {
            remap = remapped(reference, scale, shift);
            &remap
        }
        None => reference,
    };
    let mut b = Bits::new(data);
    b.skip(h.mb_start as u32);
    let mut blocks = Blocks::new(it, pt, h, cols, rows);
    let rnd = h.round_control as i32;
    let half = matches!(h.mv_mode, MvMode::OneHalf | MvMode::OneHalfBilinear);
    let bilinear_luma = h.mv_mode == MvMode::OneHalfBilinear;
    let (_, _, range_x, range_y) = RANGES[h.mv_range as usize];
    let (lw, lh) = (2 * cols, 2 * rows);
    // Each luma block's vector and whether it is intra, for prediction.
    let mut grid = vec![([0i32; 2], false); lw * lh];
    let refs = Refs::new(reference, coded);
    let frame = [(cols * 16) as i32, (rows * 16) as i32];
    let mut cells = [vec![Cell::default(); 16 * cols * rows], vec![Cell::default(); 4 * cols * rows], vec![Cell::default(); 4 * cols * rows]];
    let mut colocated = vec![[0i32; 2]; cols * rows];
    let at = |e: Error, mx: usize, my: usize| Error(format!("{} at macroblock {mx},{my}", e.0));
    for my in 0..rows {
        for mx in 0..cols {
            let four = match (&h.mv_type, h.mv_mode) {
                (Some(p), MvMode::Mixed) => bitplane_bit(&mut b, p, mx, my, cols),
                _ => false,
            };
            let skipped = bitplane_bit(&mut b, &h.skipped, mx, my, cols);
            let (bx, by) = (2 * mx, 2 * my);
            let cand = |grid: &[([i32; 2], bool)], x: isize, y: isize| -> Candidate {
                if x < 0 || y < 0 || x >= lw as isize || y >= lh as isize {
                    return None;
                }
                Some(grid[y as usize * lw + x as usize])
            };
            // The predictor for a block / macroblock, reading HYBRIDPRED when it is needed.
            let predict = |b: &mut Bits, a: Candidate, bb: Candidate, c: Candidate, at: [i32; 2], size: i32| -> [i32; 2] {
                let pre = preliminary(a, bb, c, at, size, frame);
                if hybrid_needed(pre, a, c) { hybrid_pick(b.bit(), a, c) } else { pre }
            };
            let read_mv = |b: &mut Bits| read_mvdata(b, &pt.mv[h.mv_table as usize], h.mv_range, half).ok_or_else(|| Error(format!("MVDATA at macroblock {mx},{my}")));
            let (x0, y0) = (bx as isize, by as isize);
            // Luma vectors and intra flags, the coded pattern and ACPRED.
            let mut cbp = [false; 6];
            let mut luma_intra = [false; 4];
            let mut luma_coded = [false; 4];
            let mut mvs = [[0i32; 2]; 4];
            let mut ac_pred = false;
            let mut any_coded_inter = false;
            if !four {
                let d = if skipped { MvData::default() } else { read_mv(&mut b)? };
                let mut mv = [0, 0];
                if !d.intra {
                    let a = cand(&grid, x0, y0 - 1);
                    let c = cand(&grid, x0 - 1, y0);
                    let bb = if mx + 1 < cols { cand(&grid, x0 + 2, y0 - 1) } else { cand(&grid, x0 - 1, y0 - 1) };
                    let p = predict(&mut b, a, bb, c, [(mx * 16) as i32, (my * 16) as i32], 16);
                    let scale = 1 + half as i32;
                    mv = [smod(d.dx * scale + p[0], range_x), smod(d.dy * scale + p[1], range_y)];
                }
                mvs = [mv; 4];
                luma_intra = [d.intra; 4];
                if !skipped {
                    if d.intra {
                        ac_pred = b.bit();
                    }
                    if d.more {
                        cbp = blocks.cbpcy(&mut b, h.cbp_table).map_err(|e| at(e, mx, my))?;
                    }
                    luma_coded = [cbp[0], cbp[1], cbp[2], cbp[3]];
                    any_coded_inter = !d.intra && cbp.iter().any(|&c| c);
                }
                for i in 0..4 {
                    grid[(by + (i >> 1)) * lw + bx + (i & 1)] = (mv, d.intra);
                }
            } else {
                if !skipped {
                    cbp = blocks.cbpcy(&mut b, h.cbp_table).map_err(|e| at(e, mx, my))?;
                }
                for i in 0..4 {
                    let d = if !skipped && cbp[i] { read_mv(&mut b)? } else { MvData::default() };
                    let (gx, gy) = (x0 + (i & 1) as isize, y0 + (i >> 1) as isize);
                    luma_intra[i] = d.intra;
                    luma_coded[i] = d.more;
                    if !d.intra {
                        let a = cand(&grid, gx, gy - 1);
                        let c = cand(&grid, gx - 1, gy);
                        let bb = match i {
                            0 => if mx == 0 { cand(&grid, gx + 1, gy - 1) } else { cand(&grid, gx - 1, gy - 1) },
                            1 => if mx + 1 == cols { cand(&grid, gx - 1, gy - 1) } else { cand(&grid, gx + 1, gy - 1) },
                            2 => cand(&grid, gx + 1, gy - 1),
                            _ => cand(&grid, gx - 1, gy - 1),
                        };
                        let p = predict(&mut b, a, bb, c, [(gx * 8) as i32, (gy * 8) as i32], 8);
                        let scale = 1 + half as i32;
                        mvs[i] = [smod(d.dx * scale + p[0], range_x), smod(d.dy * scale + p[1], range_y)];
                    }
                    // Store at once: the next block of this macroblock predicts from it.
                    grid[gy as usize * lw + gx as usize] = (mvs[i], d.intra);
                }
                any_coded_inter = (0..4).any(|i| !luma_intra[i] && luma_coded[i]);
            }
            let chroma_intra = if four { luma_intra.iter().filter(|&&v| v).count() >= 3 } else { luma_intra[0] };
            let chroma_luma = if four {
                four_mv_luma_for_chroma(&std::array::from_fn(|i| (mvs[i], luma_intra[i])))
            } else if chroma_intra {
                None
            } else {
                Some(mvs[0])
            };
            colocated[my * cols + mx] = chroma_luma.unwrap_or([0, 0]);
            let chroma_mv = chroma_luma.map(|l| chroma(l, fast_uvmc)).unwrap_or([0, 0]);
            if four && !skipped {
                any_coded_inter |= !chroma_intra && (cbp[4] || cbp[5]);
                let any_intra = luma_intra.iter().any(|&v| v) || chroma_intra;
                if any_intra || any_coded_inter {
                    // ACPRED when an intra block has an intra neighbour above or left.
                    let nonzero = (0..4).any(|i| {
                        let (gx, gy) = (x0 + (i & 1) as isize, y0 + (i >> 1) as isize);
                        luma_intra[i] && (cand(&grid, gx, gy - 1).is_some_and(|c| c.1) || cand(&grid, gx - 1, gy).is_some_and(|c| c.1))
                    }) || (chroma_intra && (blocks.is_intra(4, mx as isize, my as isize - 1) || blocks.is_intra(4, mx as isize - 1, my as isize)));
                    if nonzero {
                        ac_pred = b.bit();
                    }
                }
            }
            let ttmb = if skipped { None } else { blocks.ttmb(&mut b, h.mb_transform, any_coded_inter).map_err(|e| at(e, mx, my))? };
            let mut first_inter = true;
            for k in 0..6 {
                let is_luma = k < 4;
                let intra = if is_luma { luma_intra[k] } else { chroma_intra };
                let coded = if is_luma && four { luma_coded[k] } else { cbp[k] };
                let (gx, gy) = if is_luma { (bx + (k & 1), by + (k >> 1)) } else { (mx, my) };
                let (cell_grid, cell_w, cx0, cy0) = if is_luma { (0, 4 * cols, 2 * gx, 2 * gy) } else { (k - 3, 2 * cols, 2 * mx, 2 * my) };
                let mut cell = Cell::default();
                let mut nz = [true; 4];
                let mut px = [0u8; 64];
                if intra {
                    blocks.intra(&mut b, k, gx, gy, coded, ac_pred, &mut px).map_err(|e| Error(format!("{} at macroblock {mx},{my} block {k}", e.0)))?;
                    cell.intra = true;
                } else {
                    let (x, y) = block_pos(k, mx, my);
                    refs.predict(k, x, y, mvs[k.min(3)], chroma_mv, bilinear_luma, rnd, &mut px);
                    cell.mv = if is_luma { mvs[k] } else { chroma_mv };
                    nz = [false; 4];
                    if coded {
                        let mut err = [0i32; 64];
                        let l = blocks.residual(&mut b, ttmb, &mut first_inter, &mut err).map_err(|e| Error(format!("{} at macroblock {mx},{my} block {k}", e.0)))?;
                        (cell.split_h, cell.split_v, nz) = (l.split_h, l.split_v, l.nz);
                        px = add(&px, &err);
                    }
                }
                for (i, &z) in nz.iter().enumerate() {
                    cells[cell_grid][(cy0 + (i >> 1)) * cell_w + cx0 + (i & 1)] = Cell { coded: z, ..cell };
                }
                put(out, k, mx, my, &px);
            }
        }
    }
    if b.over() {
        return Err(Error("macroblocks run out".into()));
    }
    Ok(PResult { cells, colocated })
}

/// A B macroblock's prediction (8.4.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BMode {
    Direct,
    Forward,
    Backward,
    Interpolated,
    Intra,
}

/// Figure 70's reciprocals by denominator.
const INVERSE: [i32; 8] = [256, 128, 85, 64, 51, 43, 37, 32];

/// Decodes a B picture between `forward` (the previous anchor; remapped when the next anchor
/// used intensity compensation) and `backward` (the next anchor) into `out`. `colocated` is
/// the next anchor's direct-mode vectors ([`PResult::colocated`], zeros after an I or skipped
/// anchor).
#[allow(clippy::too_many_arguments)]
pub fn decode_b(it: &IntraTables, pt: &PTables, h: &InterHeader, data: &[u8], cols: usize, rows: usize, coded: (usize, usize), fast_uvmc: bool, forward: &Picture, backward: &Picture, colocated: &[[i32; 2]], out: &mut Picture) -> Result<(), Error> {
    let (num, den) = h.fraction.ok_or_else(|| Error("not a B picture header".into()))?;
    let direct_plane = h.direct.as_ref().ok_or_else(|| Error("not a B picture header".into()))?;
    let scale_factor = num as i32 * INVERSE[den as usize - 1];
    let backward_first = 2 * num >= den;
    let mut b = Bits::new(data);
    b.skip(h.mb_start as u32);
    let mut blocks = Blocks::new(it, pt, h, cols, rows);
    let rnd = h.round_control as i32;
    let half = h.mv_mode == MvMode::OneHalfBilinear;
    let (_, _, range_x, range_y) = RANGES[h.mv_range as usize];
    let (fwd_refs, bwd_refs) = (Refs::new(forward, coded), Refs::new(backward, coded));
    let frame = [(cols * 16) as i32, (rows * 16) as i32];
    // The forward and backward prediction contexts, one vector a macroblock (intra as zero).
    let mut contexts = [vec![[0i32; 2]; cols * rows], vec![[0i32; 2]; cols * rows]];
    let at = |e: Error, mx: usize, my: usize| Error(format!("{} at macroblock {mx},{my}", e.0));
    for my in 0..rows {
        for mx in 0..cols {
            let direct = bitplane_bit(&mut b, direct_plane, mx, my, cols);
            let skipped = bitplane_bit(&mut b, &h.skipped, mx, my, cols);
            let read_mv = |b: &mut Bits| read_mvdata(b, &pt.mv[h.mv_table as usize], h.mv_range, half).ok_or_else(|| Error(format!("BMV at macroblock {mx},{my}")));
            let mut d1 = MvData::default();
            let mut mode = BMode::Direct;
            if !direct {
                if !skipped {
                    d1 = read_mv(&mut b)?;
                }
                if !d1.intra || skipped {
                    // Table 57.
                    let (first, second) = if backward_first { (BMode::Backward, BMode::Forward) } else { (BMode::Forward, BMode::Backward) };
                    mode = if !b.bit() { first } else if !b.bit() { second } else { BMode::Interpolated };
                } else {
                    mode = BMode::Intra;
                }
            }
            // Direct-mode vectors (Figure 69), computed for every macroblock.
            let (qx, qy) = ((mx * 64) as i32, (my * 64) as i32);
            let pull = |v: [i32; 2]| [v[0].clamp(-60 - qx, cols as i32 * 64 - 4 - qx), v[1].clamp(-60 - qy, rows as i32 * 64 - 4 - qy)];
            let mv = colocated[my * cols + mx];
            let (direct_f, direct_b) = if half {
                (mv.map(|v| 2 * ((v * scale_factor + 255) >> 9)), mv.map(|v| 2 * ((v * (scale_factor - 256) + 255) >> 9)))
            } else {
                (mv.map(|v| (v * scale_factor + 128) >> 8), mv.map(|v| (v * (scale_factor - 256) + 128) >> 8))
            };
            let (mut fmv, mut bmv) = (pull(direct_f), pull(direct_b));
            // A predicted vector plus its differential, from context `dir` (0 forward).
            let decode_mv = |contexts: &[Vec<[i32; 2]>; 2], dir: usize, d: MvData| -> [i32; 2] {
                let ctx = &contexts[dir];
                let cand = |x: isize, y: isize| -> Candidate {
                    if x < 0 || y < 0 || x >= cols as isize || y >= rows as isize {
                        return None;
                    }
                    Some((ctx[y as usize * cols + x as usize], false))
                };
                let (x, y) = (mx as isize, my as isize);
                let bb = if mx + 1 < cols { cand(x + 1, y - 1) } else { cand(x - 1, y - 1) };
                let p = preliminary(cand(x, y - 1), bb, cand(x - 1, y), [(mx * 16) as i32, (my * 16) as i32], 16, frame);
                let scale = 1 + half as i32;
                [smod(d.dx * scale + p[0], range_x), smod(d.dy * scale + p[1], range_y)]
            };
            let mut more = !skipped;
            match mode {
                BMode::Direct => {}
                BMode::Intra => {
                    (fmv, bmv) = ([0, 0], [0, 0]);
                    more = d1.more;
                }
                BMode::Forward => {
                    fmv = decode_mv(&contexts, 0, d1);
                    more = d1.more;
                }
                BMode::Backward => {
                    bmv = decode_mv(&contexts, 1, d1);
                    more = d1.more;
                }
                BMode::Interpolated => {
                    let d2 = if d1.more { read_mv(&mut b)? } else { MvData::default() };
                    bmv = decode_mv(&contexts, 1, d1);
                    fmv = decode_mv(&contexts, 0, d2);
                    more = d2.more;
                }
            }
            contexts[0][my * cols + mx] = fmv;
            contexts[1][my * cols + mx] = bmv;
            let intra = mode == BMode::Intra;
            let mut cbp = [false; 6];
            let mut ac_pred = false;
            if !skipped {
                if intra {
                    ac_pred = b.bit();
                }
                if more {
                    cbp = blocks.cbpcy(&mut b, h.cbp_table).map_err(|e| at(e, mx, my))?;
                }
            }
            let ttmb = if skipped || intra { None } else { blocks.ttmb(&mut b, h.mb_transform, cbp.iter().any(|&c| c)).map_err(|e| at(e, mx, my))? };
            let (fcmv, bcmv) = (chroma(fmv, fast_uvmc), chroma(bmv, fast_uvmc));
            let mut first_inter = true;
            for k in 0..6 {
                let (gx, gy) = if k < 4 { (2 * mx + (k & 1), 2 * my + (k >> 1)) } else { (mx, my) };
                let (x, y) = block_pos(k, mx, my);
                let mut px = [0u8; 64];
                if intra {
                    blocks.intra(&mut b, k, gx, gy, cbp[k], ac_pred, &mut px).map_err(|e| Error(format!("{} at macroblock {mx},{my} block {k}", e.0)))?;
                } else {
                    match mode {
                        BMode::Forward => fwd_refs.predict(k, x, y, fmv, fcmv, half, rnd, &mut px),
                        BMode::Backward => bwd_refs.predict(k, x, y, bmv, bcmv, half, rnd, &mut px),
                        _ => {
                            let mut other = [0u8; 64];
                            fwd_refs.predict(k, x, y, fmv, fcmv, half, rnd, &mut px);
                            bwd_refs.predict(k, x, y, bmv, bcmv, half, rnd, &mut other);
                            for (p, o) in px.iter_mut().zip(other) {
                                *p = ((*p as u16 + o as u16 + 1) >> 1) as u8;
                            }
                        }
                    }
                    if cbp[k] {
                        let mut err = [0i32; 64];
                        blocks.residual(&mut b, ttmb, &mut first_inter, &mut err).map_err(|e| Error(format!("{} at macroblock {mx},{my} block {k}", e.0)))?;
                        px = add(&px, &err);
                    }
                }
                put(out, k, mx, my, &px);
            }
        }
    }
    if b.over() {
        return Err(Error("macroblocks run out".into()));
    }
    Ok(())
}

#[cfg(test)]
mod movie_tests {
    use barrel_formats::asf::{Asf, Kind};

    /// Decodes `BARREL_MOVIE` (avantitle) through [`crate::stream::VideoStream`] in display
    /// order and compares each of the first `BARREL_FRAMES` pictures with ffmpeg's (a PC-only
    /// test oracle, streamed through a pipe; `-fps_mode passthrough`, loop filter as the stream
    /// asks).
    #[test]
    #[ignore]
    fn movie_against_ffmpeg() {
        use std::io::Read;
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let count: usize = std::env::var("BARREL_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        let mut child = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-i", &path, "-map", "0:v", "-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let video = asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).unwrap();
        let Kind::Video { extra, width, height, .. } = &video.kind else { unreachable!() };
        let (w, h) = (*width as usize, *height as usize);
        let mut stream = crate::stream::VideoStream::new(extra, w, h).unwrap();
        let mut want = vec![0u8; w * h * 3 / 2];
        let (mut shown, mut exact) = (0usize, 0usize);
        let mut by_kind = std::collections::BTreeMap::<String, (usize, usize)>::new();
        let t0 = std::time::Instant::now();
        let mut check = |s: crate::stream::Shown| -> bool {
            if shown >= count || child.stdout.as_mut().unwrap().read_exact(&mut want).is_err() {
                return false;
            }
            let p = &s.picture;
            let mut got = Vec::with_capacity(want.len());
            for y in 0..h {
                got.extend_from_slice(&p.y[y * p.width..y * p.width + w]);
            }
            for plane in [&p.u, &p.v] {
                for y in 0..h / 2 {
                    got.extend_from_slice(&plane[y * p.width / 2..y * p.width / 2 + w / 2]);
                }
            }
            let diff = got.iter().zip(&want).filter(|(a, b)| a != b).count();
            let e = by_kind.entry(format!("{:?}", s.kind)).or_default();
            e.0 += 1;
            if diff == 0 {
                exact += 1;
                e.1 += 1;
            } else if e.0 - e.1 <= 5 {
                eprintln!("picture {shown} ({:?}): {diff} values differ", s.kind);
            }
            shown += 1;
            true
        };
        'all: for o in asf.objects().unwrap().iter().filter(|o| o.stream == video.number) {
            for s in stream.push(&o.data, o.time_ms).unwrap() {
                if !check(s) {
                    break 'all;
                }
            }
        }
        if let Some(s) = stream.finish() {
            check(s);
        }
        let _ = child.kill();
        let _ = child.wait();
        eprintln!("{exact} of {shown} pictures bit-exact in {:.1} s; by kind (shown, exact): {by_kind:?}", t0.elapsed().as_secs_f64());
        assert_eq!(exact, shown);
    }
}
