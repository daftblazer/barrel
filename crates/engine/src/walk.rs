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
    /// A wall made from a triangle: its three corners as (how far from a to b, 0 to 1; height),
    /// so the heights it spans are taken where along it a point is (a long sloping side, as a
    /// stair's rail, spans only its own slope's heights at each point). None: `low` to `high`
    /// all along.
    pub outline: Option<[[f32; 2]; 3]>,
}

impl Wall {
    /// The heights the wall spans at `t` along it (0 at a, 1 at b).
    pub fn span(&self, t: f32) -> (f32, f32) {
        let Some(o) = self.outline else { return (self.low, self.high) };
        let lo_t = o.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let hi_t = o.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let t = t.clamp(lo_t, hi_t);
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for (p, q) in [(o[0], o[1]), (o[1], o[2]), (o[2], o[0])] {
            let (t0, t1) = (p[0].min(q[0]), p[0].max(q[0]));
            if t < t0 || t > t1 {
                continue;
            }
            // An upright edge spans both its ends' heights; any other, its height at t.
            let (y0, y1) = if t1 - t0 < 1e-6 {
                (p[1].min(q[1]), p[1].max(q[1]))
            } else {
                let y = p[1] + (q[1] - p[1]) * (t - p[0]) / (q[0] - p[0]);
                (y, y)
            };
            low = low.min(y0);
            high = high.max(y1);
        }
        if low > high { (self.low, self.high) } else { (low, high) }
    }
}

/// The walls of a collision mesh, with the leaves' bounds to find them by.
pub struct Walls {
    leaves: Vec<([f32; 2], [f32; 2], Vec<Wall>)>,
}

impl Walls {
    /// The collision mesh's triangles steeper than `max_slope` (degrees: `walkable_slope`) as
    /// walls, grouped by the hierarchy's leaves.
    pub fn new(c: &Collision, max_slope: f32) -> Walls {
        let leaves = c
            .nodes
            .iter()
            .filter(|n| n.kind == 0)
            .map(|n| {
                let walls = (n.first..n.first + n.count)
                    .filter_map(|t| c.triangles.get(t as usize))
                    .filter_map(|t| wall(t.map(|i| c.vertices[i as usize]), max_slope))
                    .collect();
                ([n.min[0], n.min[2]], [n.max[0], n.max[2]], walls)
            })
            .collect();
        Walls { leaves }
    }

    /// Walls given as they are (the Ryza games' `_le.koc` quads), in square leaves of
    /// `leaf` units to find them by.
    pub fn from_walls(walls: Vec<Wall>, leaf: f32) -> Walls {
        let mut cells: std::collections::BTreeMap<(i32, i32), Vec<Wall>> = Default::default();
        for w in walls {
            let (lo, hi) = ([w.a[0].min(w.b[0]), w.a[1].min(w.b[1])], [w.a[0].max(w.b[0]), w.a[1].max(w.b[1])]);
            for cx in (lo[0] / leaf).floor() as i32..=(hi[0] / leaf).floor() as i32 {
                for cz in (lo[1] / leaf).floor() as i32..=(hi[1] / leaf).floor() as i32 {
                    cells.entry((cx, cz)).or_default().push(w);
                }
            }
        }
        let leaves = cells.into_iter().map(|((cx, cz), w)| ([cx as f32 * leaf, cz as f32 * leaf], [(cx + 1) as f32 * leaf, (cz + 1) as f32 * leaf], w)).collect();
        Walls { leaves }
    }

    /// These walls and `other`'s.
    pub fn join(mut self, other: Walls) -> Walls {
        self.leaves.extend(other.leaves);
        self
    }

    /// How far along the line from `from` to `to` (0 to 1) it first passes through a wall: where
    /// it crosses the wall's segment on the ground at a height the wall spans. None if it does
    /// not.
    pub fn first_hit(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let (lo, hi) = ([from[0].min(to[0]), from[2].min(to[2])], [from[0].max(to[0]), from[2].max(to[2])]);
        let d = [to[0] - from[0], to[2] - from[2]];
        let mut best: Option<f32> = None;
        for (llo, lhi, walls) in &self.leaves {
            if hi[0] < llo[0] || lo[0] > lhi[0] || hi[1] < llo[1] || lo[1] > lhi[1] {
                continue;
            }
            for w in walls {
                let e = [w.b[0] - w.a[0], w.b[1] - w.a[1]];
                let den = d[0] * e[1] - d[1] * e[0];
                if den.abs() < 1e-9 {
                    continue;
                }
                let q = [w.a[0] - from[0], w.a[1] - from[2]];
                let t = (q[0] * e[1] - q[1] * e[0]) / den;
                let u = (q[0] * d[1] - q[1] * d[0]) / den;
                if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let y = from[1] + (to[1] - from[1]) * t;
                let (low, high) = w.span(u);
                if y >= low && y <= high && best.is_none_or(|b| t < b) {
                    best = Some(t);
                }
            }
        }
        best
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
                let (q, t) = closest(p, w.a, w.b);
                let (low, high) = w.span(t);
                if high < y || low > y + height {
                    continue;
                }
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

/// The ground a character stands on: the field's heightmap (`<AREA>.khm`) and the collision
/// mesh's floors (its triangles that are not upright).
pub struct Ground {
    heightmap: Option<barrel_formats::khm::Heightmap>,
    /// The heightmap is read as its cells' triangles (`Heightmap::at_faces`: the Ryza games),
    /// not bilinear between its vertices.
    faces: bool,
    floors: Vec<[[f32; 3]; 3]>,
    /// The floors by square cells of `FLOOR_CELL` on the ground (x, z): each floor in every
    /// cell its box (widened by `floor_height`'s tolerance) touches, so a height looks only at
    /// its own cell's floors.
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

/// The side of `Ground`'s floor cells, units.
const FLOOR_CELL: f32 = 256.0;

impl Ground {
    /// The floors are the collision mesh's triangles no steeper than `max_slope` (degrees).
    pub fn new(heightmap: Option<barrel_formats::khm::Heightmap>, c: &Collision, max_slope: f32) -> Ground {
        let mut g = Ground { heightmap, faces: false, floors: Vec::new(), cells: Default::default() };
        g.add_floors(c, max_slope);
        g
    }

    /// A ground of the heightmap alone (a field whose collision is not read: the Ryza games
    /// keep theirs in other files, `docs/ryza/open.md`), stood on by its cells' triangles as
    /// those games do.
    pub fn of_heightmap(heightmap: barrel_formats::khm::Heightmap) -> Ground {
        Ground { heightmap: Some(heightmap), faces: true, floors: Vec::new(), cells: Default::default() }
    }

    /// More floors: a collision mesh's triangles no steeper than `max_slope` (degrees).
    pub fn add_floors(&mut self, c: &Collision, max_slope: f32) {
        let first = self.floors.len();
        self.floors.extend(c.triangles.iter().map(|t| t.map(|i| c.vertices[i as usize])).filter(|v| wall(*v, max_slope).is_none()));
        for (k, t) in self.floors.iter().enumerate().skip(first) {
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for v in t {
                lo = [lo[0].min(v[0]), lo[1].min(v[2])];
                hi = [hi[0].max(v[0]), hi[1].max(v[2])];
            }
            let margin = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 1e-3 + 1.0;
            let cell = |v: f32| (v / FLOOR_CELL).floor() as i32;
            for cx in cell(lo[0] - margin)..=cell(hi[0] + margin) {
                for cz in cell(lo[1] - margin)..=cell(hi[1] + margin) {
                    self.cells.entry((cx, cz)).or_default().push(k as u32);
                }
            }
        }
    }

    /// The heightmap's height at (x, z): bilinear between its vertices, or on its cells'
    /// triangles (`faces`); none outside it.
    fn terrain(&self, x: f32, z: f32) -> Option<f32> {
        let map = self.heightmap.as_ref()?;
        if self.faces { map.at_faces(x, z) } else { map.at(x, z) }
    }

    /// Like `height`, but the floors take the character as a ball of `radius` resting on them
    /// (`ball_height`): on a slope it stands higher than the floor under its centre, as the
    /// Ryza games' do (docs/ryza/open.md, "Ryza in the field"). The heightmap is taken as it is.
    pub fn height_on_ball(&self, x: f32, z: f32, y: f32, step_up: f32, radius: f32) -> Option<f32> {
        let cell = ((x / FLOOR_CELL).floor() as i32, (z / FLOOR_CELL).floor() as i32);
        let floors = self.cells.get(&cell).into_iter().flatten().filter_map(|&k| ball_height(&self.floors[k as usize], x, z, radius));
        self.terrain(x, z).into_iter().chain(floors).filter(|&h| h <= y + step_up).fold(None, |best, h| Some(best.map_or(h, |b: f32| b.max(h))))
    }

    /// Where a character with its feet at `y` stands at (x, z): the highest of the heightmap and
    /// the floors under it that is no more than `step_up` above its feet (higher is a ledge it
    /// does not climb); none if there is nothing under it.
    pub fn height(&self, x: f32, z: f32, y: f32, step_up: f32) -> Option<f32> {
        let cell = ((x / FLOOR_CELL).floor() as i32, (z / FLOOR_CELL).floor() as i32);
        let floors = self.cells.get(&cell).into_iter().flatten().filter_map(|&k| floor_height(&self.floors[k as usize], x, z));
        self.terrain(x, z).into_iter().chain(floors).filter(|&h| h <= y + step_up).fold(None, |best, h| Some(best.map_or(h, |b: f32| b.max(h))))
    }
}

/// The height a character of `radius` stands at on a floor triangle under (x, z), whichever way
/// it is wound: a ball resting on a plane tilted by an angle stands `radius * (1 / cos - 1)`
/// above the plane's height under its centre (1.94 on a 21 degree slope for 28). Only the
/// triangle under the centre counts (not an edge or a corner within the radius): the game's
/// feet change floor, and jump by that amount, where the centre does (walk-30 frames 2967 and
/// 2978). None when (x, z) is not over the triangle.
fn ball_height(t: &[[f32; 3]; 3], x: f32, z: f32, radius: f32) -> Option<f32> {
    let h = floor_height(t, x, z)?;
    let [a, b, c] = *t;
    let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    // STAND-IN: measured on slopes of 4 to 21 degrees; a steeper floor (a cliff sliver the walk
    // crosses in a frame) is taken at its plain height.
    let up = if len > 1e-6 { n[1].abs() / len } else { 1.0 };
    Some(if up >= BALL_STEEPEST.cos() { h + radius * (1.0 / up - 1.0) } else { h })
}

/// The steepest slope (radians) a ball's rest is lifted for by `ball_height`.
const BALL_STEEPEST: f32 = std::f32::consts::FRAC_PI_6;

/// The height of a floor triangle at (x, z), if (x, z) is over it.
fn floor_height(t: &[[f32; 3]; 3], x: f32, z: f32) -> Option<f32> {
    let [a, b, c] = *t;
    let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if d.abs() < 1e-6 {
        return None;
    }
    let l1 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
    let l2 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
    let l3 = 1.0 - l1 - l2;
    (l1 >= -1e-4 && l2 >= -1e-4 && l3 >= -1e-4).then(|| l1 * a[1] + l2 * b[1] + l3 * c[1])
}

/// A field as the player walks it: its walls and its ground. (What stands on it, such as the
/// doors to other fields, is the game's: Escha's gimmicks, `barrel_escha::data::gimmick`.)
pub struct Field {
    pub name: String,
    pub walls: Walls,
    pub ground: Ground,
}

impl Field {
    /// Loads `<fields>/<name>/<name>.elixir.gz`'s collision and heightmap.
    pub fn load(fields: &std::path::Path, name: &str) -> Result<Field, String> {
        let path = fields.join(name).join(format!("{name}.elixir.gz"));
        let bytes = barrel_files::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let container = barrel_formats::elixir::Elixir::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let entry = container.entries.iter().find(|e| e.name.ends_with("_col.g1co")).ok_or(format!("{name} has no collision"))?;
        let collision = barrel_formats::g1co::parse(container.data(entry)).map_err(|e| format!("{name}: {e}"))?;
        let heightmap = container.entries.iter().find(|e| e.name.ends_with(".khm")).and_then(|e| barrel_formats::khm::parse(container.data(e)).ok());
        Ok(Field {
            name: name.to_string(),
            walls: Walls::new(&collision, DEFAULT_SLOPE),
            ground: Ground::new(heightmap, &collision, DEFAULT_SLOPE),
        })
    }
}

/// The steepest slope a character walks, degrees: the larger of the field's (the first float
/// of its record in `field_look::SKY_SETS_EN`, 75 in nearly all) and the character's
/// `CharaBase.xml` `FMslp` (75 for the player), as `0x14026c3c0` hands them to the collision.
pub fn walkable_slope(field: f32, chara: f32) -> f32 {
    field.max(chara)
}

/// The slope the walks here take when the field's is not looked up: 75 degrees, the player's
/// `FMslp` and nearly every field's.
pub const DEFAULT_SLOPE: f32 = 75.0;

/// A triangle steeper than `max_slope` degrees as a wall: its longest edge seen from above,
/// and its heights.
fn wall(v: [[f32; 3]; 3], max_slope: f32) -> Option<Wall> {
    let (e1, e2) = ([v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]], [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]]);
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-6 || (n[1] / len).abs() > max_slope.to_radians().cos() {
        return None;
    }
    let flat = v.map(|p| [p[0], p[2]]);
    let span = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let (a, b) = [(flat[0], flat[1]), (flat[1], flat[2]), (flat[2], flat[0])]
        .into_iter()
        .max_by(|x, y| span(x.0, x.1).total_cmp(&span(y.0, y.1)))?;
    let ys = v.map(|p| p[1]);
    let (ab, len) = ([b[0] - a[0], b[1] - a[1]], span(a, b));
    let along = |p: [f32; 2]| if len > 0.0 { ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len } else { 0.0 };
    let outline = [0, 1, 2].map(|i| [along(flat[i]), ys[i]]);
    Some(Wall { a, b, low: ys.iter().copied().fold(f32::MAX, f32::min), high: ys.iter().copied().fold(f32::MIN, f32::max), outline: Some(outline) })
}

/// The point of segment a-b nearest p, and how far along it that is (0 at a, 1 at b).
fn closest(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> ([f32; 2], f32) {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / len).clamp(0.0, 1.0) } else { 0.0 };
    ([a[0] + ab[0] * t, a[1] + ab[1] * t], t)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_sloping_wall_spans_its_own_heights_where_she_is() {
        // A rail's side, 1000 long, its top falling from 200 to 0 and its bottom at -400:
        // low at the far end, so a character standing at 50 passes over it there.
        let w = super::wall([[0.0, 200.0, 0.0], [0.0, 0.0, 1000.0], [0.0, -400.0, 1000.0]], 45.0).unwrap();
        // At z 900 (t along whichever way the wall's segment runs): the top at 20, the bottom
        // at -340 (the bottom edge falls from 200 to -400).
        let t = (900.0 - w.a[1]) / (w.b[1] - w.a[1]);
        let (low, high) = w.span(t);
        assert!((high - 20.0).abs() < 1e-3 && (low + 340.0).abs() < 1e-3, "{low} {high}");
        let walls = super::Walls::from_walls(vec![w], 2000.0);
        assert_eq!(walls.step([30.0, 900.0], [-60.0, 0.0], 50.0, 150.0, 20.0), [-30.0, 900.0]);
        assert_ne!(walls.step([30.0, 100.0], [-60.0, 0.0], 50.0, 150.0, 20.0), [-30.0, 100.0]);
    }

    #[test]
    fn a_line_through_a_wall_hits_it() {
        let w = super::Walls::from_walls(vec![super::Wall { a: [-100.0, 50.0], b: [100.0, 50.0], low: 0.0, high: 200.0, outline: None }], 1000.0);
        assert_eq!(w.first_hit([0.0, 100.0, 0.0], [0.0, 100.0, 100.0]), Some(0.5));
        // Over its top, or beside it: no.
        assert_eq!(w.first_hit([0.0, 300.0, 0.0], [0.0, 300.0, 100.0]), None);
        assert_eq!(w.first_hit([200.0, 100.0, 0.0], [200.0, 100.0, 100.0]), None);
    }

    use super::*;
    use barrel_formats::g1co::Node;

    /// One wall along z = 0 from x = -100 to 100, 0 to 200 high (two triangles), in one leaf.
    fn one_wall() -> Walls {
        let c = Collision {
            nodes: vec![Node { max: [100.0, 200.0, 0.0], min: [-100.0, 0.0, 0.0], kind: 0, link: 65535, count: 2, first: 0 }],
            triangles: vec![[0, 1, 2], [2, 1, 3]],
            vertices: vec![[-100.0, 0.0, 0.0], [100.0, 0.0, 0.0], [-100.0, 200.0, 0.0], [100.0, 200.0, 0.0]],
        };
        Walls::new(&c, DEFAULT_SLOPE)
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
    fn the_ground_is_the_highest_floor_within_a_step() {
        // A floor 100 high over x 0..100, z 0..100 (two triangles), no heightmap.
        let c = Collision {
            nodes: vec![],
            triangles: vec![[0, 1, 2], [2, 1, 3]],
            vertices: vec![[0.0, 100.0, 0.0], [100.0, 100.0, 0.0], [0.0, 100.0, 100.0], [100.0, 100.0, 100.0]],
        };
        let g = Ground::new(None, &c, DEFAULT_SLOPE);
        assert_eq!(g.height(50.0, 50.0, 90.0, 30.0), Some(100.0), "a step up onto it");
        assert_eq!(g.height(50.0, 50.0, 0.0, 30.0), None, "too high a ledge from the bottom");
        assert_eq!(g.height(150.0, 50.0, 100.0, 30.0), None, "off its edge");
    }

    #[test]
    fn a_character_stands_higher_on_a_slope_than_the_floor_under_its_centre() {
        // A floor rising 1 in 3 along x (18.4 degrees): a ball of 28 stands 28 * (1 / cos - 1) = 1.49 higher.
        let c = Collision { nodes: vec![], triangles: vec![[0, 1, 2], [2, 1, 3]], vertices: vec![[0.0, 0.0, 0.0], [300.0, 100.0, 0.0], [0.0, 0.0, 300.0], [300.0, 100.0, 300.0]] };
        let g = Ground::new(None, &c, DEFAULT_SLOPE);
        let (plain, ball) = (g.height(150.0, 100.0, 0.0, 200.0).unwrap(), g.height_on_ball(150.0, 100.0, 0.0, 200.0, 28.0).unwrap());
        assert!((plain - 50.0).abs() < 1e-3 && (ball - plain - 28.0 * ((1.0f32 + 1.0 / 9.0).sqrt() - 1.0)).abs() < 1e-3, "{plain} {ball}");
        // Off the floor there is nothing, as before.
        assert_eq!(g.height_on_ball(400.0, 100.0, 0.0, 200.0, 28.0), None);
    }

    #[test]
    fn a_ramp_is_a_floor_up_to_the_slope() {
        // A ramp rising 100 over 50 (about 63 degrees): a floor at 75, a wall at 60.
        let v = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 100.0, 50.0]];
        assert!(wall(v, 75.0).is_none());
        assert!(wall(v, 60.0).is_some());
        assert_eq!(walkable_slope(75.0, 80.0), 80.0);
    }

    #[test]
    fn a_wall_above_the_character_lets_it_pass() {
        let p = one_wall().step([0.0, 50.0], [0.0, -100.0], 300.0, 150.0, 30.0);
        assert!(p[0].abs() < 1e-3 && (p[1] + 50.0).abs() < 1e-3, "{p:?}");
    }
}
