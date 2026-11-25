//! The collision of a field's placed models: the objects container holds, beside each model's
//! `<model>_hi.g1m`, its collision as `<model>_cbvh*.g1co` (a triangle mesh in the same `HVB_`
//! form as Escha's field collision, `barrel_formats::g1co`), `_cplh*` (`HLP_`, likely convex
//! hulls), `_cbox*` (boxes) and `_ccap*` (capsules), in the model's own space. Here the meshes,
//! and the boxes and capsules made into meshes (a capsule as an eight-sided prism), each placed
//! as its placement puts the model, give the walk its walls and the floors on models (stairs,
//! raised floors, rocks, crates); tree trunks are capsules, and the hulls (`HLP_`, read by
//! `hull`) cover the cliffs and the trees' roots. A shape is not always in its model's space:
//! the placement list gives each collision file a rotation and an offset within the model
//! (stairs and floors moved, the light poles' boxes turned from z-up). Docs: docs/formats.md,
//! "Ryza model collision".

use barrel_formats::elixir::Elixir;
use barrel_formats::g1co::{self, Collision};
use barrel_formats::opl2;
use glam::Vec3;

/// How the placed shapes lie in their models (`placed_meshes`): the shapes placed, and those of
/// them lying mostly outside their model's bounds (a sign of a transform read wrong).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fit {
    pub placed: usize,
    pub outside: usize,
}

/// A shape counts as outside its model when under this share of its box lies in the model's.
const OUTSIDE: f32 = 0.2;

/// Each placed model's collision shapes as meshes, in game space, and how they fit. A type's
/// collision files are the ones its placement header names (`opl2::CollisionFile`, by entry in
/// the objects container); each shape is first put where its file's rotation and offset say
/// within the model (`field::collision_within`: the game does the same when it builds a
/// placement's collision, 0x140750ca0), then placed as the model is.
pub fn placed_meshes(placements: &opl2::Placements, objects: &Elixir) -> (Vec<Collision>, Fit) {
    // The models' bounds (`obj-<model>_hi.g1m`'s geometry: min, then max), to check the fit.
    let mut model_bounds: std::collections::HashMap<String, [f32; 6]> = Default::default();
    for e in objects.entries.iter().filter(|e| !e.is_dummy()) {
        let Some(model) = e.name.strip_suffix("_hi.g1m") else { continue };
        if let Some(b) = barrel_formats::g1m::G1m::parse(objects.data(e)).ok().and_then(|m| m.geometry.map(|g| g.bounds)) {
            model_bounds.insert(model.to_ascii_lowercase(), b);
        }
    }
    let (mut out, mut fit) = (Vec::new(), Fit::default());
    for t in &placements.objects {
        let bounds = model_bounds.get(&t.name.to_ascii_lowercase());
        for file in &t.collisions {
            let Some(e) = objects.entries.get(file.entry as usize).filter(|e| !e.is_dummy()) else { continue };
            let Some(kind) = e.name.strip_suffix(".g1co").and_then(|stem| stem.rsplit_once('_')).map(|(_, kind)| kind) else { continue };
            let data = objects.data(e);
            let shape = if kind.starts_with("cbvh") {
                g1co::parse(data).ok()
            } else if kind.starts_with("cbox") || kind.starts_with("ccap") {
                shape(data)
            } else if kind.starts_with("cplh") {
                hull(data)
            } else {
                None
            };
            let Some(shape) = shape else { continue };
            let within = barrel_engine::field::collision_within(file);
            let shape = placed(&shape, |v| within.transform_point3(Vec3::from(v)).to_array());
            let outside = bounds.is_some_and(|b| inside(&shape, b) < OUTSIDE);
            for i in &t.instances {
                let m = barrel_engine::field::placement_transform(i);
                out.push(placed(&shape, |v| m.transform_point3(Vec3::from(v)).to_array()));
                fit.placed += 1;
                fit.outside += outside as usize;
            }
            if outside && std::env::var_os("BARREL_VERBOSE").is_some() {
                eprintln!("collision outside its model: {} in {}, turned {:?}, moved {:?}", e.name, t.name, file.rotation, file.offset);
            }
        }
    }
    (out, fit)
}

/// A field's own collision shapes, `<FIELD>_coll.bin` in its main container: a u32 count of
// STAND-IN: The field collision's box and cylinder sizes are read as half-sizes and an upward height without being checked against the game; the real thing is the game's own interpretation.
/// boxes, each a position, a rotation (radians, turned as placements are), three sizes and two
/// words (2 and 15 in all of Rasenboden's: not read), 44 bytes; then a u32 count of cylinders,
/// each a position, a rotation, a radius and a height and the same two words, 40 bytes. Taken
/// as half-sizes about the position, a cylinder's height upward from it (`docs/ryza/open.md`:
/// not checked against the game).
pub fn field_shapes(b: &[u8]) -> Result<Vec<Collision>, String> {
    let f = |o: usize| b.get(o..o + 4).map(|v| f32::from_le_bytes(v.try_into().unwrap())).ok_or("coll.bin: truncated");
    let n = |o: usize| b.get(o..o + 4).map(|v| u32::from_le_bytes(v.try_into().unwrap()) as usize).ok_or("coll.bin: truncated");
    let turn = |o: usize| -> Result<glam::Mat4, String> {
        let i = opl2::Instance { position: [f(o)?, f(o + 4)?, f(o + 8)?], rotation: [f(o + 12)?, f(o + 16)?, f(o + 20)?], scale: [1.0; 3], tags: String::new(), word: 0 };
        Ok(barrel_engine::field::placement_transform(&i))
    };
    let mut out = Vec::new();
    let boxes = n(0)?;
    let mut o = 4;
    for _ in 0..boxes {
        let (m, half) = (turn(o)?, Vec3::new(f(o + 24)?, f(o + 28)?, f(o + 32)?));
        let ring = [Vec3::new(half.x, 0.0, half.z), Vec3::new(half.x, 0.0, -half.z), Vec3::new(-half.x, 0.0, -half.z), Vec3::new(-half.x, 0.0, half.z)];
        out.push(placed(&prism(Vec3::ZERO, Vec3::new(0.0, half.y, 0.0), &ring), |v| m.transform_point3(Vec3::from(v)).to_array()));
        o += 44;
    }
    let cylinders = n(o)?;
    o += 4;
    for _ in 0..cylinders {
        let (m, radius, height) = (turn(o)?, f(o + 24)?, f(o + 28)?);
        let ring: Vec<Vec3> = (0..8).map(|k| (k as f32 * std::f32::consts::FRAC_PI_4).sin_cos()).map(|(s, c)| Vec3::new(c * radius, 0.0, s * radius)).collect();
        out.push(placed(&prism(Vec3::new(0.0, height * 0.5, 0.0), Vec3::new(0.0, height * 0.5, 0.0), &ring), |v| m.transform_point3(Vec3::from(v)).to_array()));
        o += 40;
    }
    Ok(out)
}

/// A box or capsule file (`OC1G`, 16 bytes, a kind byte, then floats) as a triangle mesh:
/// kind 0 a box (centre, three axes, half-sizes, each four floats), kind 3 a capsule (centre,
/// axis, then its length and radius; taken as the distance between its ends' centres, the
/// prism reaching a radius past each end). None for anything else.
fn shape(d: &[u8]) -> Option<Collision> {
    if d.get(..4) != Some(b"OC1G") {
        return None;
    }
    let f = |k: usize| d.get(17 + 4 * k..21 + 4 * k).map(|b| f32::from_le_bytes(b.try_into().unwrap()));
    let v = |k: usize| Some(Vec3::new(f(k)?, f(k + 1)?, f(k + 2)?));
    match *d.get(16)? {
        0 if d.len() >= 17 + 80 => {
            let (c, axes, half) = (v(0)?, [v(4)?, v(8)?, v(12)?], v(16)?);            Some(prism(c, axes[1] * half.y, &[axes[0] * half.x + axes[2] * half.z, axes[0] * half.x - axes[2] * half.z, -axes[0] * half.x - axes[2] * half.z, -axes[0] * half.x + axes[2] * half.z]))
        }
        3 if d.len() >= 17 + 40 => {
            let (c, axis, length, radius) = (v(0)?, v(4)?.normalize_or_zero(), f(8)?, f(9)?);
            // Two directions square to the axis, and eight points around it.
            let side = if axis.y.abs() < 0.9 { axis.cross(Vec3::Y).normalize() } else { axis.cross(Vec3::X).normalize() };
            let other = axis.cross(side);
            let ring: Vec<Vec3> = (0..8).map(|k| (k as f32 * std::f32::consts::FRAC_PI_4).sin_cos()).map(|(s, co)| (side * co + other * s) * radius).collect();
            Some(prism(c, axis * (length * 0.5 + radius), &ring))
        }
        _ => None,
    }
}

/// How much of a mesh's box lies inside `bounds` (min, then max), 0 to 1.
fn inside(c: &Collision, bounds: &[f32; 6]) -> f32 {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &c.vertices {
        for a in 0..3 {
            lo[a] = lo[a].min(v[a]);
            hi[a] = hi[a].max(v[a]);
        }
    }
    (0..3)
        .map(|a| {
            let span = (hi[a] - lo[a]).max(1e-3);
            ((hi[a].min(bounds[a + 3]) - lo[a].max(bounds[a])).max(0.0) / span).min(1.0)
        })
        .product()
}

/// A hull file (`OC1G`, then an `HLP_` chunk) as a triangle mesh. In the chunk (offsets from
/// its start): at 16 the triangle count, the triangles' offset and the vertex count (u32 each),
/// then the vertices' offset and the adjacency's (u16 each); at 0x40 the centre (3 floats); the
/// triangles as u16 triples; the vertices as (x, y, z, 0) floats from the centre. The rest
/// (each vertex's neighbours, for the game's hull tests) is not needed here.
fn hull(d: &[u8]) -> Option<Collision> {
    let c = d.get(16..)?;
    if c.get(..4) != Some(b"HLP_") {
        return None;
    }
    let u32_at = |o: usize| c.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    let u16_at = |o: usize| c.get(o..o + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap()));
    let f32_at = |o: usize| c.get(o..o + 4).map(|b| f32::from_le_bytes(b.try_into().unwrap()));
    let (count, at, vertex_count) = (u32_at(16)?, u32_at(20)?, u32_at(24)?);
    let vertex_at = u16_at(28)? as usize;
    let centre = [f32_at(0x40)?, f32_at(0x44)?, f32_at(0x48)?];
    let mut vertices = Vec::with_capacity(vertex_count);
    for k in 0..vertex_count {
        let o = vertex_at + 16 * k;
        vertices.push([f32_at(o)? + centre[0], f32_at(o + 4)? + centre[1], f32_at(o + 8)? + centre[2]]);
    }
    let mut triangles = Vec::with_capacity(count);
    for k in 0..count {
        let t = [u16_at(at + 6 * k)?, u16_at(at + 6 * k + 2)?, u16_at(at + 6 * k + 4)?];
        if t.iter().any(|&i| i as usize >= vertex_count) {
            return None;
        }
        triangles.push(t);
    }
    // A hull is a wall for her, not a floor: she does not stand on it (the stall she makes on the placed stairs' hull at 32690, 30068 is
    // not the game's: `gather-rasen2` 2960 to 2972 she runs on past it), so only its steep triangles stay.
    // STAND-IN: that no hull is a floor is read off that one run (`A21-S09-STAIR-03`, the only hull she crossed); the game's own use of a hull
    // (a trigger, the camera's) is not read.
    let steep = barrel_engine::walk::DEFAULT_SLOPE.to_radians().cos();
    triangles.retain(|t| {
        let [a, b, c] = t.map(|i| Vec3::from(vertices[i as usize]));
        (b - a).cross(c - a).normalize_or_zero().y.abs() <= steep
    });
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &vertices {
        for a in 0..3 {
            lo[a] = lo[a].min(v[a]);
            hi[a] = hi[a].max(v[a]);
        }
    }
    let node = g1co::Node { max: hi, min: lo, kind: 0, link: 65535, count: triangles.len() as u32, first: 0 };
    Some(Collision { nodes: vec![node], triangles, vertices })
}

/// A prism about `c`: the polygon `ring` (offsets from its middle, in order around) at `c - up`
/// and at `c + up`, closed at both ends; one leaf node over it all.
fn prism(c: Vec3, up: Vec3, ring: &[Vec3]) -> Collision {
    let n = ring.len();
    let mut vertices: Vec<[f32; 3]> = ring.iter().map(|&o| (c - up + o).to_array()).collect();
    vertices.extend(ring.iter().map(|&o| (c + up + o).to_array()));
    let mut triangles = Vec::new();
    for k in 0..n {
        let (a, b) = (k as u16, ((k + 1) % n) as u16);
        triangles.push([a, b, b + n as u16]);
        triangles.push([a, b + n as u16, a + n as u16]);
    }
    for k in 1..n - 1 {
        triangles.push([0, k as u16 + 1, k as u16]);
        triangles.push([n as u16, (n + k) as u16, (n + k + 1) as u16]);
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &vertices {
        for a in 0..3 {
            lo[a] = lo[a].min(v[a]);
            hi[a] = hi[a].max(v[a]);
        }
    }
    let node = g1co::Node { max: hi, min: lo, kind: 0, link: 65535, count: triangles.len() as u32, first: 0 };
    Collision { nodes: vec![node], triangles, vertices }
}

/// A mesh moved by `f`: its vertices and its nodes' bounds (each node's box made again from
/// its corners, so a turned mesh's boxes still hold it).
fn placed(c: &Collision, f: impl Fn([f32; 3]) -> [f32; 3]) -> Collision {
    let nodes = c
        .nodes
        .iter()
        .map(|n| {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for k in 0..8 {
                let corner = std::array::from_fn(|a| if k >> a & 1 == 0 { n.min[a] } else { n.max[a] });
                let p = f(corner);
                for a in 0..3 {
                    lo[a] = lo[a].min(p[a]);
                    hi[a] = hi[a].max(p[a]);
                }
            }
            g1co::Node { min: lo, max: hi, ..*n }
        })
        .collect();
    Collision { nodes, triangles: c.triangles.clone(), vertices: c.vertices.iter().map(|&v| f(v)).collect() }
}

#[cfg(test)]
mod tests {
    fn file(kind: u8, floats: &[f32]) -> Vec<u8> {
        let mut d = b"OC1G2\0\0\0\0\0\x01\0\0\0\0\0".to_vec();
        d.push(kind);
        for v in floats {
            d.extend(v.to_le_bytes());
        }
        d
    }

    #[test]
    fn field_shapes_are_boxes_then_cylinders() {
        let mut b = 1u32.to_le_bytes().to_vec();
        for v in [100.0f32, 0.0, 200.0, 0.0, 0.0, 0.0, 10.0, 50.0, 20.0] {
            b.extend(v.to_le_bytes());
        }
        b.extend(2u32.to_le_bytes());
        b.extend(15u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        for v in [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 30.0, 40.0] {
            b.extend(v.to_le_bytes());
        }
        b.extend(2u32.to_le_bytes());
        b.extend(15u32.to_le_bytes());
        let s = super::field_shapes(&b).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].nodes[0].min, s[0].nodes[0].max), ([90.0, -50.0, 180.0], [110.0, 50.0, 220.0]));
        assert!((s[1].nodes[0].max[1] - 40.0).abs() < 1e-3 && (s[1].nodes[0].max[0] - 30.0).abs() < 1e-3);
    }

    #[test]
    fn a_box_is_its_six_sides() {
        let d = file(0, &[0.0, 20.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 30.0, 20.0, 10.0, 0.0]);
        let c = super::shape(&d).unwrap();
        assert_eq!(c.triangles.len(), 12);
        let (lo, hi) = (c.nodes[0].min, c.nodes[0].max);
        assert_eq!((lo, hi), ([-30.0, 0.0, -10.0], [30.0, 40.0, 10.0]));
    }

    #[test]
    fn a_hull_is_its_triangles_about_its_centre() {
        let mut c = vec![0u8; 0x80];
        c[..4].copy_from_slice(b"HLP_");
        for (o, v) in [(16usize, 1u32), (20, 0x50), (24, 3)] {
            c[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        c[28..30].copy_from_slice(&0x60u16.to_le_bytes());
        for (k, v) in [10.0f32, 20.0, 30.0].iter().enumerate() {
            c[0x40 + 4 * k..0x44 + 4 * k].copy_from_slice(&v.to_le_bytes());
        }
        for (k, i) in [0u16, 1, 2].iter().enumerate() {
            c[0x50 + 2 * k..0x52 + 2 * k].copy_from_slice(&i.to_le_bytes());
        }
        c.resize(0x60 + 48, 0);
        // An upright triangle: a hull is walls only (a floor triangle goes: `a_hull_is_not_a_floor`).
        for (k, p) in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].iter().enumerate() {
            for (a, v) in p.iter().enumerate() {
                let o = 0x60 + 16 * k + 4 * a;
                c[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        let mut d = b"OC1G2\0\0\0\0\0\x01\x02\0\0\0\0".to_vec();
        d.extend(c);
        let h = super::hull(&d).unwrap();
        assert_eq!(h.triangles, vec![[0, 1, 2]]);
        assert_eq!(h.vertices[1], [11.0, 20.0, 30.0]);
    }

    #[test]
    fn a_hull_is_not_a_floor() {
        let mut c = vec![0u8; 0x80];
        c[..4].copy_from_slice(b"HLP_");
        for (o, v) in [(16usize, 1u32), (20, 0x50), (24, 3)] {
            c[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        c[28..30].copy_from_slice(&0x60u16.to_le_bytes());
        for (k, i) in [0u16, 1, 2].iter().enumerate() {
            c[0x50 + 2 * k..0x52 + 2 * k].copy_from_slice(&i.to_le_bytes());
        }
        c.resize(0x60 + 48, 0);
        // A triangle lying flat.
        for (k, p) in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]].iter().enumerate() {
            for (a, v) in p.iter().enumerate() {
                let o = 0x60 + 16 * k + 4 * a;
                c[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        let mut d = b"OC1G2\0\0\0\0\0\x01\x02\0\0\0\0".to_vec();
        d.extend(c);
        assert!(super::hull(&d).unwrap().triangles.is_empty());
    }

    #[test]
    fn a_z_up_box_is_turned_upright_by_its_collision_rotation() {
        // A pole along z in the file; the light poles' record turns it along y in the model.
        let d = file(0, &[0.0, 0.0, 500.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 50.0, 50.0, 500.0, 0.0]);
        let quarter = std::f32::consts::FRAC_PI_2;
        let file = barrel_formats::opl2::CollisionFile { entry: 0, rotation: [-quarter, -quarter, 0.0], offset: [0.0, 10.0, 0.0] };
        let within = barrel_engine::field::collision_within(&file);
        let c = super::placed(&super::shape(&d).unwrap(), |v| within.transform_point3(glam::Vec3::from(v)).to_array());
        let (lo, hi) = (c.nodes[0].min, c.nodes[0].max);
        assert!((lo[1] - 10.0).abs() < 1e-2 && (hi[1] - 1010.0).abs() < 1e-2, "{lo:?} {hi:?}");
        assert!((hi[0] - lo[0] - 100.0).abs() < 1e-2 && (hi[2] - lo[2] - 100.0).abs() < 1e-2, "{lo:?} {hi:?}");
    }

    #[test]
    fn an_upright_capsule_is_a_post() {
        let d = file(3, &[0.0, 100.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 200.0, 50.0]);
        let c = super::shape(&d).unwrap();
        let (lo, hi) = (c.nodes[0].min, c.nodes[0].max);
        assert!((lo[1] + 50.0).abs() < 1e-3 && (hi[1] - 250.0).abs() < 1e-3, "{lo:?} {hi:?}");
        assert!((hi[0] - 50.0).abs() < 1e-3);
    }

    #[test]
    fn a_moved_mesh_keeps_its_nodes_around_it() {
        let c = barrel_formats::g1co::Collision {
            nodes: vec![barrel_formats::g1co::Node { max: [1.0, 1.0, 1.0], min: [-1.0, -1.0, -1.0], kind: 0, link: 65535, count: 1, first: 0 }],
            triangles: vec![[0, 1, 2]],
            vertices: vec![[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, 1.0]],
        };
        let m = super::placed(&c, |v| [v[0] * 10.0 + 100.0, v[1], v[2] * 10.0]);
        assert_eq!(m.vertices[1], [110.0, 0.0, -10.0]);
        assert_eq!((m.nodes[0].min, m.nodes[0].max), ([90.0, -1.0, -10.0], [110.0, 1.0, 10.0]));
    }
}
