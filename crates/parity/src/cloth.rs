//! `barrel-parity cloth`: a character's cloth control points in the capture against Barrel's.
//!
//! The game's vertex program for a cloth piece reads its control points from a constant array
//! (`vCtrl`, the points in the piece's parent joint's space, `x y z 1` each). Barrel's solver
//! writes the same points to a trace (`BARREL_TRACE_CLOTH`, `barrel_ryza::play::cloth`), so a
//! walk's pieces compare point by point at the same frames, without the walk's placement.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use super::draws3d::{folder, game_draws, key};
use super::{index, Result, Trace};

/// Control points deeper than this inside a collider (game units, centimetres) count as
/// penetrating: the solver leaves points on the surface to rounding.
const PENETRATION_MIN: f32 = 0.05;

/// A collider as Barrel's trace gives it: shape (5 a capsule along y, 6 a sphere), radius, half
/// length, and its matrix into the piece's parent space (column major).
struct Collider {
    shape: u64,
    r: f32,
    h: f32,
    m: [f32; 16],
}

impl Collider {
    /// How deep a point (parent space) lies inside the collider, 0 when outside.
    fn depth(&self, p: [f32; 3]) -> f32 {
        let m = &self.m;
        let (a, b, c) = ([m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]);
        let cross = |u: [f32; 3], v: [f32; 3]| [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        let det = dot(a, cross(b, c));
        if det.abs() < 1e-12 {
            return 0.0;
        }
        // The point in the collider's space by Cramer's rule (the matrix may carry a scale).
        let d = [p[0] - m[12], p[1] - m[13], p[2] - m[14]];
        let q = [dot(d, cross(b, c)) / det, dot(a, cross(d, c)) / det, dot(a, cross(b, d)) / det];
        let depth = if self.shape == 5 {
            let e = (q[1].abs() - self.h).max(0.0);
            self.r - (q[0] * q[0] + q[2] * q[2] + e * e).sqrt()
        } else {
            self.r - (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt()
        };
        depth.max(0.0)
    }
}

/// A piece in Barrel's trace at one frame.
struct Piece {
    cloth: u64,
    parent: u64,
    pins: usize,
    pts: Vec<[f32; 3]>,
    cols: Vec<Collider>,
}

fn floats(v: &Value) -> Vec<f32> {
    v.as_array().into_iter().flatten().filter_map(|x| x.as_f64().map(|x| x as f32)).collect()
}

/// Barrel's cloth trace: the pieces by frame.
fn trace(path: &Path) -> Result<BTreeMap<u64, Vec<Piece>>> {
    let mut out = BTreeMap::new();
    for line in std::fs::read_to_string(path)?.lines() {
        let v: Value = serde_json::from_str(line)?;
        let pieces = v["pieces"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| Piece {
                cloth: p["cloth"].as_u64().unwrap_or(0),
                parent: p["parent"].as_u64().unwrap_or(0),
                pins: p["pins"].as_u64().unwrap_or(0) as usize,
                pts: floats(&p["pts"]).chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
                cols: p["cols"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| Some(Collider { shape: c["shape"].as_u64()?, r: c["r"].as_f64()? as f32, h: c["h"].as_f64()? as f32, m: floats(&c["m"]).try_into().ok()? }))
                    .collect(),
            })
            .collect();
        out.insert(v["frame"].as_u64().unwrap_or(0), pieces);
    }
    Ok(out)
}

/// A piece's numbers over the frames.
#[derive(Default)]
struct Stat {
    parent: u64,
    n: usize,
    frames: usize,
    sum2: f64,
    count: usize,
    /// The squared error of the pinned points (the body's pose) and how many.
    pin2: f64,
    pinned: usize,
    /// The worst point's error, its frame and its number.
    worst: (f32, u64, usize),
    /// The worst frame's rms over the piece and its frame.
    worst_frame: (f32, u64),
    /// Points inside a collider (game, Barrel) and the deepest.
    inside: [usize; 2],
    deepest: [f32; 2],
}

/// Each Barrel piece takes the game's draw whose points are nearest its own (the biggest pieces
/// first); draws of one piece repeat its array, so a draw is taken again only by a piece that
/// differs. Returns for each piece the game's array.
fn pair<'a>(pieces: &[Piece], ctrl: &[&'a Vec<f32>]) -> Vec<Option<&'a Vec<f32>>> {
    let mut taken: Vec<&Vec<f32>> = Vec::new();
    let mut out = vec![None; pieces.len()];
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(pieces[i].pts.len()));
    for i in order {
        let p = &pieces[i];
        let n = p.pts.len();
        let err = |c: &Vec<f32>| p.pts.iter().enumerate().map(|(k, b)| (0..3).map(|a| (c[k * 4 + a] - b[a]).powi(2)).sum::<f32>()).sum::<f32>();
        let best = ctrl.iter().filter(|c| c.len() >= n * 4 && !taken.iter().any(|t| t[..n * 4] == c[..n * 4])).min_by(|a, b| err(a).total_cmp(&err(b)));
        if let Some(&c) = best {
            taken.push(c);
            out[i] = Some(c);
        }
    }
    out
}

pub fn run(dir: &Path, (from, to, step): (u64, u64, u64), barrel: &Path, idx: Option<&str>, model: &str) -> Result<()> {
    let t = Trace { dir: dir.into() };
    let (names, _) = index(idx)?;
    let trace = trace(barrel)?;
    // `PARITY_CLOTH_SHIFT=N`: Barrel's frame N later than the game's is compared (to find a timing offset).
    let shift: i64 = std::env::var("PARITY_CLOTH_SHIFT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut stats: BTreeMap<u64, Stat> = BTreeMap::new();
    let mut frame = from;
    while frame <= to {
        let (Ok(draws), Some(pieces)) = (game_draws(&t, frame), trace.get(&((frame as i64 + shift).max(0) as u64))) else {
            frame += step;
            continue;
        };
        // The game's cloth draws of the model: its G-buffer draws that carry a control array.
        let ctrl: Vec<&Vec<f32>> = draws
            .iter()
            .filter(|d| d.rt.len() >= 4 && d.ds != "-")
            .filter(|d| key(&names, d.textures.iter().map(|x| x.1.clone()), d.count).0.iter().any(|n| folder(n).contains(model)))
            .filter_map(|d| d.vars.iter().find(|v| v.0 == "vCtrl").map(|v| &v.2))
            .collect();
        for (p, game) in pieces.iter().zip(pair(pieces, &ctrl)) {
            let Some(c) = game else { continue };
            let s = stats.entry(p.cloth).or_default();
            (s.parent, s.n) = (p.parent, p.pts.len());
            s.frames += 1;
            let (mut frame_sum, mut pin_sum) = (0f32, 0f32);
            for (k, b) in p.pts.iter().enumerate() {
                let g = [c[k * 4], c[k * 4 + 1], c[k * 4 + 2]];
                let e2: f32 = (0..3).map(|a| (g[a] - b[a]).powi(2)).sum();
                if std::env::var("PARITY_CLOTH_POINTS").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame) {
                    println!("piece {} point {k}{} game {g:.2?} barrel {b:.2?}", p.cloth, if k < p.pins { " (pin)" } else { "" });
                }
                if k < p.pins {
                    pin_sum += e2;
                    s.pin2 += e2 as f64;
                    s.pinned += 1;
                    continue;
                }
                s.sum2 += e2 as f64;
                s.count += 1;
                frame_sum += e2;
                if e2.sqrt() > s.worst.0 {
                    s.worst = (e2.sqrt(), frame, k);
                }
                for (side, pt) in [g, *b].into_iter().enumerate() {
                    let depth = p.cols.iter().map(|c| c.depth(pt)).fold(0.0, f32::max);
                    s.inside[side] += (depth > PENETRATION_MIN) as usize;
                    s.deepest[side] = s.deepest[side].max(depth);
                }
            }
            let rms = (frame_sum / (p.pts.len() - p.pins).max(1) as f32).sqrt();
            if std::env::var_os("PARITY_CLOTH_FRAMES").is_some() {
                println!("frame {frame} piece {} rms {rms:.2} pin {:.2}", p.cloth, (pin_sum / p.pins.max(1) as f32).sqrt());
            }
            if rms > s.worst_frame.0 {
                s.worst_frame = (rms, frame);
            }
        }
        frame += step;
    }
    println!("piece parent points pinned frames  pin rms | movable rms   max (frame, point) | worst frame rms | points inside a collider: game, Barrel (deepest)");
    let (mut sum2, mut count) = (0f64, 0usize);
    for (cloth, s) in &stats {
        sum2 += s.sum2;
        count += s.count;
        println!(
            "{cloth:5} {:6} {:6} {:6} {:6} {:7.2} | {:5.2} {:5.2} ({}, {}) | {:5.2} @{} | {}, {} ({:.2}, {:.2})",
            s.parent,
            s.n,
            s.pinned / s.frames.max(1),
            s.frames,
            (s.pin2 / s.pinned.max(1) as f64).sqrt(),
            (s.sum2 / s.count.max(1) as f64).sqrt(),
            s.worst.0,
            s.worst.1,
            s.worst.2,
            s.worst_frame.0,
            s.worst_frame.1,
            s.inside[0],
            s.inside[1],
            s.deepest[0],
            s.deepest[1]
        );
    }
    println!("all pieces: rms {:.3} cm over {count} points; points inside a collider (game, Barrel): {}, {}", (sum2 / count.max(1) as f64).sqrt(), stats.values().map(|s| s.inside[0]).sum::<usize>(), stats.values().map(|s| s.inside[1]).sum::<usize>());
    Ok(())
}
