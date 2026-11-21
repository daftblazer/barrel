//! A character's cloth pieces (hair, skirt, jacket hem: the model's `NUNO` chunk) moving: each
//! piece's solver (`barrel_engine::cloth`) fed the pose, the wind and the colliders each frame,
//! and the cloth vertices it gives written to the scene's palette entries
//! (`barrel_render::ClothSlots`) the vertex shader reads them from.

use barrel_engine::cloth::ClothSim;
use barrel_formats::g1m::{Geometry, Skeleton};
use barrel_render::{ClothSlots, cloth_entry};
use glam::{DMat4, DVec3, Mat4, Vec3};

/// Seconds a step covers: the game steps its cloth at 60 Hz.
const STEP: f32 = 1.0 / 60.0;
/// Steps a piece takes at its first frame on the pose it stands in, so a scene starts with the
/// cloth settled and not falling from its rest shape (a deliberate difference, docs/engine.md).
const SETTLE_STEPS: usize = 120;
/// The most steps one frame catches up (a slow frame does not stall on the solver).
const MAX_CATCH_UP: usize = 4;

/// One cloth piece: its solver and what feeds and shows it.
struct Piece {
    sim: ClothSim,
    /// The piece in the geometry's list.
    cloth: usize,
    /// Its parent joint.
    parent: usize,
    /// Each pinned point: its place at rest in the model's space and the (joint, weight) pairs
    /// that carry it.
    pins: Vec<([f32; 3], Vec<(usize, f32)>)>,
    /// Each collider: its joint and offset from it (column-vector matrix).
    colliders: Vec<(usize, Mat4)>,
    /// The submeshes it shows and where their vertices' entries start in the palette.
    slots: Vec<ClothSlots>,
}

/// A character's moving cloth.
pub struct ClothDriver {
    pieces: Vec<Piece>,
    /// The skeleton's joints at bind pose, model space.
    bind: Vec<Mat4>,
    settled: bool,
    /// Time not yet covered by a step.
    owed: f32,
}

impl ClothDriver {
    /// The solvers for a character's placed cloth pieces, None when it has none to show.
    /// `slots` are the scene's `cloth_slots`; `local_gravity` the character's `localGravity`.
    pub fn new(geometry: &Geometry, skeleton: &Skeleton, slots: &[ClothSlots], instance: u32, local_gravity: f32) -> Option<ClothDriver> {
        let mut pieces = Vec::new();
        for (index, cloth) in geometry.cloths.iter().enumerate() {
            let Some((parent, _)) = cloth.placed else { continue };
            let parent = parent as usize;
            let mine: Vec<ClothSlots> = slots
                .iter()
                .filter(|s| s.instance == instance && geometry.submeshes.get(s.submesh).and_then(|m| geometry.cloth_of(m)).is_some_and(|c| std::ptr::eq(c, cloth)))
                .copied()
                .collect();
            if mine.is_empty() {
                continue;
            }
            let palette = geometry.bone_maps.get(cloth.palette as usize);
            let pins = cloth
                .pins
                .iter()
                .enumerate()
                .map(|(k, pin)| {
                    let carriers = (0..4).filter(|&i| pin.weights[i] > 0.0).filter_map(|i| Some((skeleton.joint(palette?.get(pin.slots[i] as usize)?.joint)?, pin.weights[i]))).collect();
                    (cloth.points[k], carriers)
                })
                .collect();
            let colliders: Vec<_> = cloth.collider_groups.iter().filter_map(|&g| geometry.collider_groups.get(g as usize)).flatten().collect();
            let bones = colliders.iter().map(|c| skeleton.joint(c.bone).map(|j| (j, Mat4::from_cols_array_2d(&c.offset)))).collect::<Option<Vec<_>>>()?;
            let sim = ClothSim::new(cloth, &colliders.iter().map(|&&c| c).collect::<Vec<_>>(), local_gravity);
            pieces.push(Piece { sim, cloth: index, parent, pins, colliders: bones, slots: mine });
        }
        (!pieces.is_empty()).then(|| ClothDriver { pieces, bind: barrel_engine::skin::bind_world(skeleton), settled: false, owed: 0.0 })
    }

    /// Move the cloth `dt` seconds on: `skin` the pose's skinning matrices (bind to posed, game
    /// space), `model` the character's matrix, `wind` the world wind (a velocity). The vertices
    /// go to `palette`. The first call settles the cloth on the pose first.
    pub fn update(&mut self, geometry: &Geometry, skin: &[Mat4], model: DMat4, wind: Vec3, dt: f32, palette: &mut [Mat4]) {
        self.owed += dt;
        let mut steps = (self.owed / STEP).floor() as usize;
        self.owed -= steps as f32 * STEP;
        steps = steps.min(MAX_CATCH_UP);
        // A joint's posed world matrix: its skin matrix on its bind pose.
        let world = |j: usize| model * (skin.get(j).copied().unwrap_or(Mat4::IDENTITY) * self.bind.get(j).copied().unwrap_or(Mat4::IDENTITY)).as_dmat4();
        let first = !self.settled;
        self.settled = true;
        for piece in &mut self.pieces {
            let parent = world(piece.parent);
            let parent_inverse = (skin.get(piece.parent).copied().unwrap_or(Mat4::IDENTITY) * self.bind.get(piece.parent).copied().unwrap_or(Mat4::IDENTITY)).inverse();
            // The pinned points: skinned like a vertex, then into the parent joint's space.
            let pins: Vec<Vec3> = piece
                .pins
                .iter()
                .map(|(rest, carriers)| {
                    let p = Vec3::from(*rest);
                    let posed = carriers.iter().fold(Vec3::ZERO, |sum, &(j, w)| sum + skin.get(j).copied().unwrap_or(Mat4::IDENTITY).transform_point3(p) * w);
                    parent_inverse.transform_point3(posed)
                })
                .collect();
            let colliders: Vec<DMat4> = piece.colliders.iter().map(|&(j, offset)| world(j) * offset.as_dmat4()).collect();
            if first {
                piece.sim.reset(parent, &pins);
                for _ in 0..SETTLE_STEPS {
                    piece.sim.step(STEP, parent, &pins, &colliders, wind);
                }
            }
            for _ in 0..steps {
                piece.sim.step(STEP, parent, &pins, &colliders, wind);
            }
            // The vertices from the points, in the model's space.
            let cloth = &geometry.cloths[piece.cloth];
            let points: Vec<[f32; 3]> = piece.sim.points().iter().map(|p| cloth.to_model(p.to_array())).collect();
            for slots in &piece.slots {
                let Some(vertices) = geometry.submeshes.get(slots.submesh).and_then(|s| geometry.cloth_vertices(s, &points)) else { continue };
                for (k, v) in vertices.iter().flatten().enumerate() {
                    if let Some(entry) = palette.get_mut(slots.first as usize + k) {
                        *entry = cloth_entry(v);
                    }
                }
            }
        }
    }
}

impl ClothDriver {
    /// The pieces' control points at `frame` as one JSON line (`BARREL_TRACE_CLOTH`): per piece its
    /// parent joint, `cloth` (its number in the geometry) and the points in the parent's space,
    /// `x y z` each, for `barrel-parity cloth`.
    pub fn trace_line(&self, frame: i64) -> String {
        use std::fmt::Write as _;
        let mut line = format!("{{\"frame\":{frame},\"pieces\":[");
        for (k, piece) in self.pieces.iter().enumerate() {
            let _ = write!(line, "{}{{\"cloth\":{},\"parent\":{},\"pins\":{},\"pts\":[", if k > 0 { "," } else { "" }, piece.cloth, piece.parent, piece.pins.len());
            for (i, p) in piece.sim.points().iter().enumerate() {
                let _ = write!(line, "{}{:.4},{:.4},{:.4}", if i > 0 { "," } else { "" }, p.x, p.y, p.z);
            }
            line.push_str("],\"cols\":[");
            for (i, c) in piece.sim.colliders().iter().enumerate() {
                let m = c.local.to_cols_array().map(|v| format!("{v:.4}")).join(",");
                let _ = write!(line, "{}{{\"shape\":{},\"r\":{:.4},\"h\":{:.4},\"m\":[{m}]}}", if i > 0 { "," } else { "" }, c.shape, c.radius, c.half);
            }
            line.push_str("]}");
        }
        line.push_str("]}");
        line
    }
}

/// The world wind on a character's cloth at `clock` seconds: its `clothWindScale` times the
/// field's gust power times the gust's direction (the plants' captured bend,
/// `BARREL_PLANT_WIND`, stands in for the power when set).
pub fn world_wind(gust: Option<barrel_ryza::environment::Gust>, scale: f32, clock: f32) -> Vec3 {
    let Some(g) = gust else { return Vec3::ZERO };
    let captured = std::env::var("BARREL_PLANT_WIND").ok().and_then(|v| v.split_once(',').and_then(|(b, _)| b.trim().parse::<f32>().ok()));
    // `Gust::bend` is 5 sqrt(2 p): the power a bend came from.
    let power = captured.map_or_else(|| g.at(clock).0, |bend| (bend / 5.0).powi(2) / 2.0);
    Vec3::from(g.direction) * (scale * power)
}

/// The character's matrix in game space, in f64 (a field's coordinates near 32,000 need it).
pub fn model_matrix(size: [f32; 3], turn: f32, at: Vec3) -> DMat4 {
    DMat4::from_scale_rotation_translation(DVec3::new(size[0] as f64, size[1] as f64, size[2] as f64), glam::DQuat::from_rotation_y(turn as f64), at.as_dvec3())
}
