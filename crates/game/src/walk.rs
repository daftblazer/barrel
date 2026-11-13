//! A character walking against a field's walls (`<AREA>_col.g1co`, `barrel_formats::g1co`).
//!
//! Not yet the game's: the game moves characters through ktgl's collision engine
//! (`elixir::EXCollisionScene` over ktgl's broad and narrow phases), which is not read. This
//! is a plain collide-and-slide in its place: the character an upright cylinder, the walls the
//! collision mesh's upright triangles seen from above as segments, and a move pushed back out
//! of every wall it ends inside, so it slides along them (`docs/engine.md`, "Walking").

use barrel_formats::g1co::Collision;

/// A wall seen from above: a segment on the ground (x, z) and the heights it spans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub low: f32,
    pub high: f32,
}

/// The walls of a collision mesh, with the leaves' bounds to find them by.
pub struct Walls {
    leaves: Vec<([f32; 2], [f32; 2], Vec<Wall>)>,
}

impl Walls {
    /// The collision mesh's upright triangles (their normal within about 25° of level) as
    /// walls, grouped by the hierarchy's leaves.
    pub fn new(c: &Collision) -> Walls {
        let leaves = c
            .nodes
            .iter()
            .filter(|n| n.kind == 0)
            .map(|n| {
                let walls = (n.first..n.first + n.count)
                    .filter_map(|t| c.triangles.get(t as usize))
                    .filter_map(|t| wall(t.map(|i| c.vertices[i as usize])))
                    .collect();
                ([n.min[0], n.min[2]], [n.max[0], n.max[2]], walls)
            })
            .collect();
        Walls { leaves }
    }

    /// The walls near (within `reach` of) the point (x, z).
    fn near(&self, p: [f32; 2], reach: f32) -> impl Iterator<Item = &Wall> {
        self.leaves
            .iter()
            .filter(move |(lo, hi, _)| p[0] >= lo[0] - reach && p[0] <= hi[0] + reach && p[1] >= lo[1] - reach && p[1] <= hi[1] + reach)
            .flat_map(|(_, _, w)| w.iter())
    }

    /// Moves a character of `radius` standing at (x, z) with its feet at `y` and `height` tall
    /// by `delta`, in steps of at most half its radius (so no step passes through a wall), each
    /// pushed back out of the walls it overlaps: a move into a wall slides along it.
    pub fn step(&self, from: [f32; 2], delta: [f32; 2], y: f32, height: f32, radius: f32) -> [f32; 2] {
        let length = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
        let steps = (length / (radius * 0.5).max(1.0)).ceil().max(1.0) as usize;
        let part = [delta[0] / steps as f32, delta[1] / steps as f32];
        let mut p = from;
        for _ in 0..steps {
            p = self.resolve([p[0] + part[0], p[1] + part[1]], part, y, height, radius);
        }
        p
    }

    /// The point `p` pushed back out of each wall it overlaps (a few rounds, for corners).
    fn resolve(&self, mut p: [f32; 2], delta: [f32; 2], y: f32, height: f32, radius: f32) -> [f32; 2] {
        for _ in 0..4 {
            let mut moved = false;
            for w in self.near(p, radius + delta[0].abs() + delta[1].abs()) {
                if w.high < y || w.low > y + height {
                    continue;
                }
                let q = closest(p, w.a, w.b);
                let (dx, dz) = (p[0] - q[0], p[1] - q[1]);
                let d = (dx * dx + dz * dz).sqrt();
                if d < radius && d > 1e-4 {
                    let push = (radius - d) / d;
                    p = [p[0] + dx * push, p[1] + dz * push];
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        p
    }
}

/// An upright triangle as a wall: its longest edge seen from above, and its heights.
fn wall(v: [[f32; 3]; 3]) -> Option<Wall> {
    let (e1, e2) = ([v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]], [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]]);
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-6 || (n[1] / len).abs() > 0.42 {
        return None;
    }
    let flat = v.map(|p| [p[0], p[2]]);
    let span = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let (a, b) = [(flat[0], flat[1]), (flat[1], flat[2]), (flat[2], flat[0])]
        .into_iter()
        .max_by(|x, y| span(x.0, x.1).total_cmp(&span(y.0, y.1)))?;
    let ys = v.map(|p| p[1]);
    Some(Wall { a, b, low: ys.iter().copied().fold(f32::MAX, f32::min), high: ys.iter().copied().fold(f32::MIN, f32::max) })
}

/// The point of segment a-b nearest p.
fn closest(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / len).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + ab[0] * t, a[1] + ab[1] * t]
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::g1co::Node;

    /// One wall along z = 0 from x = -100 to 100, 0 to 200 high (two triangles), in one leaf.
    fn one_wall() -> Walls {
        let c = Collision {
            nodes: vec![Node { max: [100.0, 200.0, 0.0], min: [-100.0, 0.0, 0.0], kind: 0, link: 65535, count: 2, first: 0 }],
            triangles: vec![[0, 1, 2], [2, 1, 3]],
            vertices: vec![[-100.0, 0.0, 0.0], [100.0, 0.0, 0.0], [-100.0, 200.0, 0.0], [100.0, 200.0, 0.0]],
        };
        Walls::new(&c)
    }

    #[test]
    fn a_wall_stops_a_walk_into_it() {
        // Walking straight at the wall from z = 50 by 100 ends a radius short of it.
        let p = one_wall().step([0.0, 50.0], [0.0, -100.0], 0.0, 150.0, 30.0);
        assert!((p[0] - 0.0).abs() < 1e-3 && (p[1] - 30.0).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn a_walk_along_a_wall_slides() {
        // Walking diagonally into it keeps the move along it.
        let p = one_wall().step([0.0, 40.0], [50.0, -50.0], 0.0, 150.0, 30.0);
        assert!((p[0] - 50.0).abs() < 1e-3 && (p[1] - 30.0).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn a_wall_above_the_character_lets_it_pass() {
        let p = one_wall().step([0.0, 50.0], [0.0, -100.0], 300.0, 150.0, 30.0);
        assert!(p[0].abs() < 1e-3 && (p[1] + 50.0).abs() < 1e-3, "{p:?}");
    }
}
