//! VC-1 progressive P pictures (SMPTE ST 421 8.3): 1-MV, 4-MV and skipped macroblocks, motion
//! compensated from the previous anchor picture, with inter blocks of 8 × 8, 8 × 4, 4 × 8 or
//! 4 × 4 transforms and intra blocks among them.

use crate::bitplane::Bitplane;
use crate::bits::{Bits, Vlc};
use crate::vc1::{Error, InterHeader};
use crate::vc1_intra::{dc_step, inverse_8x8, read_coefficients, read_dc_diff, IntraTables, Picture};
use crate::vc1_mc::{bicubic, bilinear, inverse, Plane};
use crate::vc1_mv::{chroma, four_mv_luma_for_chroma, hybrid_needed, hybrid_pick, preliminary, read_mvdata, smod, Candidate, RANGES};
use crate::vc1_small::{Coded, Halves, MvMode, Transform, SUBBLKPAT2, SUBBLKPAT4, TTBLK, TTMB};
use crate::vc1_tables as t;

/// The tables a P picture reads with, built once (beside [`IntraTables`]).
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

#[derive(Clone, Copy, Default)]
struct LumaBlock {
    mv: [i32; 2],
    /// Some for an intra block.
    intra: Option<IntraState>,
}

/// Decodes a P picture from `reference` into `out`. `fast_uvmc` is the entry point's FASTUVMC.
#[allow(clippy::too_many_arguments)]
pub fn decode(it: &IntraTables, pt: &PTables, h: &InterHeader, data: &[u8], cols: usize, rows: usize, coded: (usize, usize), fast_uvmc: bool, reference: &Picture, out: &mut Picture) -> Result<(), Error> {
    if h.intensity.is_some() {
        return Err(Error("intensity compensation: not supported yet".into()));
    }
    let mut b = Bits::new(data);
    b.skip(h.mb_start as u32);
    let mquant = h.pquant as i32;
    let double_quant = 2 * mquant + h.half_qp as i32;
    let rate = if h.pquant < 5 { 0 } else if h.pquant < 13 { 1 } else { 2 };
    let rnd = h.round_control as i32;
    let half = matches!(h.mv_mode, MvMode::OneHalf | MvMode::OneHalfBilinear);
    let bilinear_luma = h.mv_mode == MvMode::OneHalfBilinear;
    let (_, _, range_x, range_y) = RANGES[h.mv_range as usize];
    let set_index = match h.ac_set {
        0 if h.pqindex <= 8 => 0,
        0 => 3,
        1 => 1,
        _ => 2,
    };
    let (intra_set, inter_set) = (&it.intra[set_index], &it.inter[set_index]);
    let dc_vlc = if h.dc_high { (&it.dc[2], &it.dc[3]) } else { (&it.dc[0], &it.dc[1]) };
    let (lw, lh) = (2 * cols, 2 * rows);
    let mut luma = vec![LumaBlock::default(); lw * lh];
    let mut cb: Vec<Option<IntraState>> = vec![None; cols * rows];
    let mut cr: Vec<Option<IntraState>> = vec![None; cols * rows];
    let mut mode3 = None;
    let (pw, cw) = (out.width, out.width / 2);
    let ref_y = Plane { px: &reference.y, stride: pw, width: coded.0 as i32, height: coded.1 as i32 };
    let ref_u = Plane { px: &reference.u, stride: cw, width: coded.0.div_ceil(2) as i32, height: coded.1.div_ceil(2) as i32 };
    let ref_v = Plane { px: &reference.v, stride: cw, width: coded.0.div_ceil(2) as i32, height: coded.1.div_ceil(2) as i32 };
    let frame = [(cols * 16) as i32, (rows * 16) as i32];
    for my in 0..rows {
        for mx in 0..cols {
            let four = match (&h.mv_type, h.mv_mode) {
                (Some(Bitplane::Raw), MvMode::Mixed) => b.bit(),
                (Some(p), MvMode::Mixed) => p.get(mx, my, cols).unwrap(),
                _ => false,
            };
            let skipped = match &h.skipped {
                Bitplane::Raw => b.bit(),
                p => p.get(mx, my, cols).unwrap(),
            };
            let (bx, by) = (2 * mx, 2 * my);
            let cand = |luma: &[LumaBlock], x: isize, y: isize| -> Candidate {
                if x < 0 || y < 0 || x >= lw as isize || y >= lh as isize {
                    return None;
                }
                let l = luma[y as usize * lw + x as usize];
                Some((l.mv, l.intra.is_some()))
            };
            // The predictor for a block / macroblock, reading HYBRIDPRED when it is needed.
            let predict = |b: &mut Bits, a: Candidate, bb: Candidate, c: Candidate, at: [i32; 2], size: i32| -> [i32; 2] {
                let pre = preliminary(a, bb, c, at, size, frame);
                if hybrid_needed(pre, a, c) { hybrid_pick(b.bit(), a, c) } else { pre }
            };
            let (x0, y0) = (bx as isize, by as isize);
            // Luma vectors and intra flags, the coded pattern and ACPRED.
            let mut cbp = [false; 6];
            let mut luma_intra = [false; 4];
            let mut luma_coded = [false; 4];
            let mut mvs = [[0i32; 2]; 4];
            let mut ac_pred = false;
            let mut any_coded_inter = false;
            if !four {
                let d = if skipped { Default::default() } else { read_mvdata(&mut b, &pt.mv[h.mv_table as usize], h.mv_range, half).ok_or_else(|| Error(format!("MVDATA at {mx},{my}")))? };
                let mut mv = [0, 0];
                if !d.intra {
                    let a = cand(&luma, x0, y0 - 1);
                    let c = cand(&luma, x0 - 1, y0);
                    let bb = if mx + 1 < cols { cand(&luma, x0 + 2, y0 - 1) } else { cand(&luma, x0 - 1, y0 - 1) };
                    let p = predict(&mut b, a, bb, c, [(mx * 16) as i32, (my * 16) as i32], 16);
                    let scale = 1 + half as i32;
                    mv = [smod(d.dx * scale + p[0], range_x), smod(d.dy * scale + p[1], range_y)];
                }
                mvs = [mv; 4];
                luma_intra = [d.intra; 4];
                if !skipped {
                    if d.intra && !d.more {
                        ac_pred = b.bit();
                    } else if d.more {
                        if d.intra {
                            ac_pred = b.bit();
                        }
                        let c = pt.cbpcy[h.cbp_table as usize].read(&mut b).ok_or_else(|| Error(format!("CBPCY at {mx},{my}")))?;
                        for (k, v) in cbp.iter_mut().enumerate() {
                            *v = (c >> (5 - k)) & 1 != 0;
                        }
                    }
                    luma_coded = [cbp[0], cbp[1], cbp[2], cbp[3]];
                    any_coded_inter = !d.intra && cbp.iter().any(|&c| c);
                }
            } else {
                if !skipped {
                    let c = pt.cbpcy[h.cbp_table as usize].read(&mut b).ok_or_else(|| Error(format!("CBPCY at {mx},{my}")))?;
                    for (k, v) in cbp.iter_mut().enumerate() {
                        *v = (c >> (5 - k)) & 1 != 0;
                    }
                }
                for i in 0..4 {
                    let d = if !skipped && cbp[i] { read_mvdata(&mut b, &pt.mv[h.mv_table as usize], h.mv_range, half).ok_or_else(|| Error(format!("BLKMVDATA at {mx},{my}")))? } else { Default::default() };
                    let (gx, gy) = (x0 + (i & 1) as isize, y0 + (i >> 1) as isize);
                    luma_intra[i] = d.intra;
                    luma_coded[i] = d.more;
                    if !d.intra {
                        let a = cand(&luma, gx, gy - 1);
                        let c = cand(&luma, gx - 1, gy);
                        let bb = match i {
                            0 => if mx == 0 { cand(&luma, gx + 1, gy - 1) } else { cand(&luma, gx - 1, gy - 1) },
                            1 => if mx + 1 == cols { cand(&luma, gx - 1, gy - 1) } else { cand(&luma, gx + 1, gy - 1) },
                            2 => cand(&luma, gx + 1, gy - 1),
                            _ => cand(&luma, gx - 1, gy - 1),
                        };
                        let p = predict(&mut b, a, bb, c, [(gx * 8) as i32, (gy * 8) as i32], 8);
                        let scale = 1 + half as i32;
                        mvs[i] = [smod(d.dx * scale + p[0], range_x), smod(d.dy * scale + p[1], range_y)];
                    } else {
                        mvs[i] = [0, 0];
                    }
                    // Store at once: the next block of this macroblock predicts from it.
                    let l = &mut luma[gy as usize * lw + gx as usize];
                    l.mv = mvs[i];
                    l.intra = if d.intra { Some(IntraState::default()) } else { None };
                }
                any_coded_inter = (0..4).any(|i| !luma_intra[i] && luma_coded[i]);
            }
            let chroma_intra_count = luma_intra.iter().filter(|&&v| v).count();
            let chroma_intra = if four { chroma_intra_count >= 3 } else { luma_intra[0] };
            let chroma_mv = if four {
                let blocks: [([i32; 2], bool); 4] = std::array::from_fn(|i| (mvs[i], luma_intra[i]));
                four_mv_luma_for_chroma(&blocks).map(|l| chroma(l, fast_uvmc))
            } else if chroma_intra {
                None
            } else {
                Some(chroma(mvs[0], fast_uvmc))
            };
            if four && !skipped {
                any_coded_inter |= !chroma_intra && (cbp[4] || cbp[5]);
                let any_intra = luma_intra.iter().any(|&v| v) || chroma_intra;
                if any_intra || any_coded_inter {
                    // ACPRED when an intra block has an intra neighbour above or left.
                    let nonzero = (0..4).any(|i| {
                        if !luma_intra[i] {
                            return false;
                        }
                        let (gx, gy) = (x0 + (i & 1) as isize, y0 + (i >> 1) as isize);
                        cand(&luma, gx, gy - 1).is_some_and(|c| c.1) || cand(&luma, gx - 1, gy).is_some_and(|c| c.1)
                    }) || (chroma_intra && ((my > 0 && cb[(my - 1) * cols + mx].is_some()) || (mx > 0 && cb[my * cols + mx - 1].is_some())));
                    if nonzero {
                        ac_pred = b.bit();
                    }
                }
            }
            // The macroblock's transform type (TTMB), when blocks signal theirs.
            let ttmb = if h.mb_transform && any_coded_inter && !skipped {
                Some(pt.ttmb[rate].read(&mut b).ok_or_else(|| Error(format!("TTMB at {mx},{my}")))?)
            } else {
                None
            };
            let trace = std::env::var("BARREL_TRACE_MB").ok().is_some_and(|v| v == "all" || v == format!("{mx},{my}"));
            if trace {
                eprintln!("mb {mx},{my}: four {four} skipped {skipped} mvs {mvs:?} intra {luma_intra:?} coded {luma_coded:?} cbp {cbp:?} chroma {chroma_mv:?} ttmb {ttmb:?} acpred {ac_pred} frame tt {:?}", h.frame_transform);
            }
            let mut first_inter = true;
            for k in 0..6 {
                let is_luma = k < 4;
                let intra = if is_luma { luma_intra[k] } else { chroma_intra };
                let coded = if is_luma { if four { luma_coded[k] } else { cbp[k] } } else { cbp[k] };
                let (gx, gy) = if is_luma { (bx + (k & 1), by + (k >> 1)) } else { (mx, my) };
                let (dst, stride, px, py) = match k {
                    0..=3 => (&mut out.y, pw, gx * 8, gy * 8),
                    4 => (&mut out.u, cw, mx * 8, my * 8),
                    _ => (&mut out.v, cw, mx * 8, my * 8),
                };
                if intra {
                    // An intra block (8.3.6.1): neighbours count only when intra.
                    let state_at = |x: isize, y: isize| -> Option<IntraState> {
                        if x < 0 || y < 0 {
                            return None;
                        }
                        let (x, y) = (x as usize, y as usize);
                        match k {
                            0..=3 => luma.get(y * lw + x).and_then(|l| l.intra),
                            4 => cb[y * cols + x],
                            _ => cr[y * cols + x],
                        }
                    };
                    let (ix, iy) = (gx as isize, gy as isize);
                    let (a, c) = (state_at(ix, iy - 1), state_at(ix - 1, iy));
                    let diff = read_dc_diff(&mut b, if is_luma { dc_vlc.0 } else { dc_vlc.1 }, mquant).ok_or_else(|| Error(format!("DC at {mx},{my} block {k}")))?;
                    let (top, predictor) = match (a, c) {
                        (Some(a), Some(c)) => {
                            let bdc = state_at(ix - 1, iy - 1).map(|s| s.dc).unwrap_or(0);
                            if (bdc - a.dc).abs() <= (bdc - c.dc).abs() { (false, c.dc) } else { (true, a.dc) }
                        }
                        (Some(a), None) => (true, a.dc),
                        (None, Some(c)) => (false, c.dc),
                        (None, None) => (false, 0),
                    };
                    let dc = predictor + diff;
                    let mut q = [0i32; 64];
                    if coded {
                        let set = if is_luma { intra_set } else { inter_set };
                        read_coefficients(&mut b, it, set, mquant, &mut mode3, &t::INTER_8X8_SCAN, 1, &mut q).map_err(|e| Error(format!("{} at {mx},{my} block {k}", e.0)))?;
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
                    match k {
                        0..=3 => luma[gy * lw + gx].intra = Some(s),
                        4 => cb[my * cols + mx] = Some(s),
                        _ => cr[my * cols + mx] = Some(s),
                    }
                    let mut d = [0i32; 64];
                    d[0] = dc * dc_step(mquant);
                    for i in 1..64 {
                        let v = q[i];
                        if v != 0 {
                            d[i] = v * double_quant + if h.uniform { 0 } else { v.signum() * mquant };
                        }
                    }
                    let r = inverse_8x8(&d);
                    for y in 0..8 {
                        for x in 0..8 {
                            dst[(py + y) * stride + px + x] = (r[y * 8 + x] + 128).clamp(0, 255) as u8;
                        }
                    }
                    continue;
                }
                // An inter block: the prediction, then the residual if coded.
                let mut pred = [0u8; 64];
                if is_luma {
                    let mv = mvs[k];
                    if bilinear_luma {
                        bilinear(&ref_y, px as i32, py as i32, mv, 8, 8, rnd, &mut pred);
                    } else {
                        bicubic(&ref_y, px as i32, py as i32, mv, 8, 8, rnd, &mut pred);
                    }
                    luma[gy * lw + gx].mv = mv;
                    luma[gy * lw + gx].intra = None;
                } else {
                    let mv = chroma_mv.unwrap_or([0, 0]);
                    bilinear(if k == 4 { &ref_u } else { &ref_v }, px as i32, py as i32, mv, 8, 8, rnd, &mut pred);
                }
                let mut err = [0i32; 64];
                if coded {
                    let (tt, halves) = if let Some(f) = h.frame_transform {
                        ([Transform::T8x8, Transform::T8x4, Transform::T4x8, Transform::T4x4][f as usize], None)
                    } else if let Some((t, hv, mb_level)) = ttmb {
                        if first_inter {
                            (t, Some(hv))
                        } else if mb_level {
                            (t, None)
                        } else {
                            let (t2, hv2) = pt.ttblk[rate].read(&mut b).ok_or_else(|| Error(format!("TTBLK at {mx},{my} block {k}")))?;
                            (t2, Some(hv2))
                        }
                    } else {
                        (Transform::T8x8, None)
                    };
                    first_inter = false;
                    if trace {
                        eprintln!("  block {mx},{my},{k}: {tt:?} halves {halves:?}");
                    }
                    let set = inter_set;
                    let mut sub = |b: &mut Bits, n: usize, m: usize, scan: &[u8], ox: usize, oy: usize, mode3: &mut Option<(u32, u32)>| -> Result<(), Error> {
                        let mut q = vec![0i32; n * m];
                        read_coefficients(b, it, set, mquant, mode3, scan, 0, &mut q).map_err(|e| Error(format!("{} at {mx},{my} block {k}", e.0)))?;
                        for v in q.iter_mut() {
                            if *v != 0 {
                                *v = *v * double_quant + if h.uniform { 0 } else { v.signum() * mquant };
                            }
                        }
                        if std::env::var("BARREL_TRACE_MB").ok().is_some_and(|v| v == format!("{mx},{my}")) {
                            eprintln!("    {n}x{m} at {ox},{oy}: dequantized {q:?}");
                        }
                        let r = inverse(&q, n, m);
                        if std::env::var("BARREL_TRACE_MB").ok().is_some_and(|v| v == format!("{mx},{my}")) {
                            eprintln!("    residual {r:?}");
                        }
                        for y in 0..m {
                            for x in 0..n {
                                err[(oy + y) * 8 + ox + x] = r[y * n + x];
                            }
                        }
                        Ok(())
                    };
                    match tt {
                        Transform::T8x8 => sub(&mut b, 8, 8, &t::INTER_8X8_SCAN, 0, 0, &mut mode3)?,
                        Transform::T8x4 | Transform::T4x8 => {
                            let hv = match halves.filter(|&v| v != 0) {
                                Some(v) => v,
                                None => pt.subblk2.read(&mut b).ok_or_else(|| Error(format!("SUBBLKPAT at {mx},{my} block {k}")))?,
                            };
                            for j in 0..2 {
                                if hv & (2 >> j) != 0 {
                                    if tt == Transform::T8x4 {
                                        sub(&mut b, 8, 4, &t::INTER_8X4_SCAN, 0, 4 * j, &mut mode3)?;
                                    } else {
                                        sub(&mut b, 4, 8, &t::INTER_4X8_SCAN, 4 * j, 0, &mut mode3)?;
                                    }
                                }
                            }
                        }
                        Transform::T4x4 => {
                            let pat = pt.subblk4[rate].read(&mut b).ok_or_else(|| Error(format!("SUBBLKPAT at {mx},{my} block {k}")))?;
                            for j in 0..4 {
                                if pat & (8 >> j) != 0 {
                                    sub(&mut b, 4, 4, &t::INTER_4X4_SCAN, 4 * (j & 1), 4 * (j >> 1), &mut mode3)?;
                                }
                            }
                        }
                    }
                }
                for y in 0..8 {
                    for x in 0..8 {
                        dst[(py + y) * stride + px + x] = (pred[y * 8 + x] as i32 + err[y * 8 + x]).clamp(0, 255) as u8;
                    }
                }
            }
            if !four {
                for i in 0..4 {
                    let l = &mut luma[(by + (i >> 1)) * lw + bx + (i & 1)];
                    if luma_intra[i] {
                        if l.intra.is_none() {
                            l.intra = Some(IntraState::default());
                        }
                        l.mv = [0, 0];
                    } else {
                        l.mv = mvs[i];
                        l.intra = None;
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
mod movie_tests {
    use super::*;
    use crate::vc1::PictureType;
    use barrel_formats::asf::{Asf, Kind};

    /// Decodes `avantitle`'s first `BARREL_ANCHORS` anchor pictures (I, P, skipped; no loop
    /// filter) against `BARREL_REF`, ffmpeg's `-skip_frame nonref -skip_loop_filter all` output
    /// as I420, and prints how each compares.
    #[test]
    #[ignore]
    fn anchors_against_ffmpeg() {
        let (Ok(path), Ok(refs)) = (std::env::var("BARREL_MOVIE"), std::env::var("BARREL_REF")) else { return };
        let count: usize = std::env::var("BARREL_ANCHORS").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
        // `BARREL_REF=ffmpeg`: ffmpeg's frames streamed through a pipe (test oracle, PC only).
        let mut stream = (refs == "ffmpeg").then(|| {
            std::process::Command::new("ffmpeg")
                .args(["-loglevel", "error", "-skip_frame", "noref", "-skip_loop_filter", "all", "-i", &path, "-map", "0:v", "-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        });
        let reference = if stream.is_some() { Vec::new() } else { std::fs::read(&refs).unwrap() };
        let mut frame_buf = vec![0u8; 1920 * 1080 * 3 / 2];
        let (mut exact, mut tried) = (0, 0);
        let bytes = std::fs::read(path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let video = asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).unwrap();
        let Kind::Video { extra, .. } = &video.kind else { unreachable!() };
        let (s, e) = crate::vc1::headers(extra).unwrap();
        let (cols, rows) = s.macroblocks(&e);
        let (it, pt) = (IntraTables::default(), PTables::default());
        let (planes, inter) = (crate::bitplane::Tables::default(), crate::vc1::InterTables::default());
        let frame_len = 1920 * 1080 * 3 / 2;
        let mut anchor: Option<Picture> = None;
        let mut n = 0;
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == video.number) {
            if n >= count {
                break;
            }
            let kind = s.picture_start(&o.data).unwrap().kind;
            let data = crate::bits::unescape(&o.data);
            let t0 = std::time::Instant::now();
            let pic = match kind {
                PictureType::I => {
                    let hh = s.intra_header(&e, &data, &planes).unwrap();
                    let mut p = Picture::new(cols, rows);
                    crate::vc1_intra::decode(&it, &hh, &data, cols, rows, &mut p).unwrap();
                    p
                }
                PictureType::P => {
                    let hh = s.inter_header(&e, &data, &planes, &inter).unwrap();
                    let mut p = Picture::new(cols, rows);
                    if let Err(err) = decode(&it, &pt, &hh, &data, cols, rows, (1920, 1080), e.fast_uvmc, anchor.as_ref().unwrap(), &mut p) {
                        eprintln!("anchor {n}: P failed: {}", err.0);
                        break;
                    }
                    p
                }
                PictureType::Skipped => anchor.clone().unwrap(),
                _ => continue,
            };
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let want: &[u8] = match stream.as_mut() {
                Some(child) => {
                    use std::io::Read;
                    if child.stdout.as_mut().unwrap().read_exact(&mut frame_buf).is_err() {
                        break;
                    }
                    &frame_buf
                }
                None => match reference.get(n * frame_len..(n + 1) * frame_len) {
                    Some(w) => w,
                    None => break,
                },
            };
            let mut got = Vec::with_capacity(frame_len);
            for y in 0..1080 {
                got.extend_from_slice(&pic.y[y * pic.width..y * pic.width + 1920]);
            }
            for plane in [&pic.u, &pic.v] {
                for y in 0..540 {
                    got.extend_from_slice(&plane[y * pic.width / 2..y * pic.width / 2 + 960]);
                }
            }
            if std::env::var("BARREL_DUMP").ok().and_then(|v| v.parse::<usize>().ok()) == Some(n) {
                std::fs::write(std::env::var("BARREL_YUV").unwrap(), &got).unwrap();
            }
            let diff = got.iter().zip(want).filter(|(a, b)| a != b).count();
            let first = got.iter().zip(want).position(|(a, b)| a != b);
            tried += 1;
            exact += (diff == 0) as usize;
            if diff != 0 || std::env::var("BARREL_VERBOSE").is_ok() {
                eprintln!("anchor {n} {kind:?}: {diff} pixels differ (first at {first:?}), {ms:.1} ms");
            }
            anchor = Some(pic);
            n += 1;
        }
        eprintln!("{exact} of {tried} anchors bit-exact");
        if let Some(mut child) = stream {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
