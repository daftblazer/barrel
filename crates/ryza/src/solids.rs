//! The collision of a field's placed models: the objects container holds, beside each model's
//! `<model>_hi.g1m`, its collision as `<model>_cbvh*.g1co` (a triangle mesh in the same `HVB_`
//! form as Escha's field collision, `barrel_formats::g1co`), `_cplh*` (`HLP_`, likely convex
//! hulls), `_cbox*` (boxes) and `_ccap*` (capsules), in the model's own space. Here the meshes,
//! each placed as its placement puts the model, give the walk its walls and the floors on
//! models (stairs, raised floors, rocks); the hulls, boxes and capsules are not read
//! (`docs/ryza/open.md`). Docs: docs/formats.md, "Ryza model collision".

use barrel_formats::elixir::Elixir;
use barrel_formats::g1co::{self, Collision};
use barrel_formats::opl2;
use glam::Vec3;

/// Each placed model's collision meshes, in game space.
pub fn placed_meshes(placements: &opl2::Placements, objects: &Elixir) -> Vec<Collision> {
    // The meshes by model name (the entries' names drop the placement's `obj-`).
    let mut meshes: std::collections::HashMap<String, Vec<Collision>> = Default::default();
    for e in objects.entries.iter().filter(|e| !e.is_dummy()) {
        let Some(stem) = e.name.strip_suffix(".g1co") else { continue };
        let Some((model, kind)) = stem.rsplit_once('_') else { continue };
        if !kind.starts_with("cbvh") {
            continue;
        }
        if let Ok(c) = g1co::parse(objects.data(e)) {
            meshes.entry(model.to_ascii_lowercase()).or_default().push(c);
        }
    }
    let mut out = Vec::new();
    for t in &placements.objects {
        let name = t.name.strip_prefix("obj-").unwrap_or(&t.name).to_ascii_lowercase();
        let Some(list) = meshes.get(&name) else { continue };
        for i in &t.instances {
            let m = barrel_engine::field::placement_transform(i);
            for c in list {
                out.push(placed(c, |v| m.transform_point3(Vec3::from(v)).to_array()));
            }
        }
    }
    out
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
