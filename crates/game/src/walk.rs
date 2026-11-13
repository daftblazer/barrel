//! A character walking against a field's walls (`<AREA>_col.g1co`, `barrel_formats::g1co`).
//!
//! Not yet the game's: the game moves characters through ktgl's collision engine
//! (`elixir::EXCollisionScene` over ktgl's broad and narrow phases), which is not read. This
//! is a plain collide-and-slide in its place: the character an upright cylinder, the walls the
//! collision mesh's upright triangles seen from above as segments, and a move pushed back out
//! of every wall it ends inside, so it slides along them (`docs/engine.md`, "Walking").

use barrel_formats::g1co::Collision;

/// The event whose having been seen makes the player run faster (`EVENT_XG05_110`).
pub const FASTER_RUN_EVENT: usize = 734;

/// How fast the player runs on a field, units a second: 500, or 625 once `FASTER_RUN_EVENT`
/// has been seen (the user brain's speed, `0x140234910`).
pub fn player_run_speed(faster: bool) -> f32 {
    if faster { 625.0 } else { 500.0 }
}

/// How much faster the player goes with the run toggle on.
pub const RUN_TOGGLE: f32 = 1.5;

/// How far the stick has to lean before the player moves at all.
pub const STICK_DEAD_ZONE: f32 = 0.2;

/// The player's move over `dt` seconds for a stick leaning `stick` (x, z; each -1 to 1): none
/// inside the dead zone, else along the stick at the run speed times how far it leans (at most
/// 1), half as fast again with the run toggle on (`run_on`: pad button 6, "Run On" / "Run
/// Off", off at the start), as the user brain does (`0x140234950`).
pub fn player_move(stick: [f32; 2], dt: f32, faster: bool, run_on: bool) -> [f32; 2] {
    let tilt = (stick[0] * stick[0] + stick[1] * stick[1]).sqrt();
    if tilt < STICK_DEAD_ZONE {
        return [0.0, 0.0];
    }
    let toggle = if run_on { RUN_TOGGLE } else { 1.0 };
    let scale = player_run_speed(faster) * dt * tilt.min(1.0) / tilt * toggle;
    [stick[0] * scale, stick[1] * scale]
}

/// The player's field motion: `MOTION_F_WAIT` (31), `MOTION_WALK` (32) or `MOTION_RUN` (33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Wait,
    Walk,
    Run,
}

/// The motion for the player moving at `speed` (units a second, as she moved this frame) and
/// the rate it plays at, as the user brain picks them (`0x140235410`): running from 0.42 of the
/// run speed, walking below it, waiting when still; played at the speed times 0.00225, never
/// slower than 1. (It blends into a new run or walk over 0.15 seconds.)
pub fn player_gait(speed: f32, faster: bool) -> (Gait, f32) {
    let gait = if speed >= player_run_speed(faster) * 0.42 {
        Gait::Run
    } else if speed > 0.0 {
        Gait::Walk
    } else {
        Gait::Wait
    };
    (gait, (speed * 0.01 * 0.225).max(1.0))
}

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

/// The ground a character stands on: the field's heightmap (`<AREA>.khm`) and the collision
/// mesh's floors (its triangles that are not upright).
pub struct Ground {
    heightmap: Option<barrel_formats::khm::Heightmap>,
    floors: Vec<[[f32; 3]; 3]>,
}

impl Ground {
    /// The floors are the collision mesh's triangles no steeper than `max_slope` (degrees).
    pub fn new(heightmap: Option<barrel_formats::khm::Heightmap>, c: &Collision, max_slope: f32) -> Ground {
        let floors = c.triangles.iter().map(|t| t.map(|i| c.vertices[i as usize])).filter(|v| wall(*v, max_slope).is_none()).collect();
        Ground { heightmap, floors }
    }

    /// The heightmap's height at (x, z), bilinear between its vertices; none outside it.
    fn terrain(&self, x: f32, z: f32) -> Option<f32> {
        let h = self.heightmap.as_ref()?;
        let (fx, fz) = (x / h.cell_size, z / h.cell_size);
        if fx < 0.0 || fz < 0.0 || fx >= h.cells_x as f32 || fz >= h.cells_z as f32 {
            return None;
        }
        let (ix, iz) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let row = |z: usize| h.height(ix, z) * (1.0 - tx) + h.height(ix + 1, z) * tx;
        Some(row(iz) * (1.0 - tz) + row(iz + 1) * tz)
    }

    /// Where a character with its feet at `y` stands at (x, z): the highest of the heightmap and
    /// the floors under it that is no more than `step_up` above its feet (higher is a ledge it
    /// does not climb); none if there is nothing under it.
    pub fn height(&self, x: f32, z: f32, y: f32, step_up: f32) -> Option<f32> {
        let floors = self.floors.iter().filter_map(|t| floor_height(t, x, z));
        self.terrain(x, z).into_iter().chain(floors).filter(|&h| h <= y + step_up).fold(None, |best, h| Some(best.map_or(h, |b: f32| b.max(h))))
    }
}

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

/// A field as the player walks it: its walls, its ground and its gimmicks (the doors among
/// them lead to other fields, `Gimmick::destination`).
pub struct Field {
    pub name: String,
    pub walls: Walls,
    pub ground: Ground,
    pub gimmicks: Vec<barrel_formats::gimmick::Gimmick>,
}

impl Field {
    /// Loads `<fields>/<name>/<name>.elixir.gz`'s collision and heightmap; `gimmicks` are the
    /// map's (from the executable's table, `barrel_formats::gimmick::areas`).
    pub fn load(fields: &std::path::Path, name: &str, gimmicks: Vec<barrel_formats::gimmick::Gimmick>) -> Result<Field, String> {
        let path = fields.join(name).join(format!("{name}.elixir.gz"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let container = barrel_formats::elixir::Elixir::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let entry = container.entries.iter().find(|e| e.name.ends_with("_col.g1co")).ok_or(format!("{name} has no collision"))?;
        let collision = barrel_formats::g1co::parse(container.data(entry)).map_err(|e| format!("{name}: {e}"))?;
        let heightmap = container.entries.iter().find(|e| e.name.ends_with(".khm")).and_then(|e| barrel_formats::khm::parse(container.data(e)).ok());
        Ok(Field {
            name: name.to_string(),
            walls: Walls::new(&collision, DEFAULT_SLOPE),
            ground: Ground::new(heightmap, &collision, DEFAULT_SLOPE),
            gimmicks,
        })
    }

    /// The gimmicks a point (field units) stands inside.
    pub fn inside(&self, p: [f32; 3]) -> Vec<usize> {
        (0..self.gimmicks.len()).filter(|&i| self.gimmicks[i].contains(p.map(|v| v / 100.0))).collect()
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
    Some(Wall { a, b, low: ys.iter().copied().fold(f32::MAX, f32::min), high: ys.iter().copied().fold(f32::MIN, f32::max) })
}

/// The point of segment a-b nearest p.
fn closest(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / len).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + ab[0] * t, a[1] + ab[1] * t]
}

/// The player's jump (pad button 1 in the user brain, `0x140234cbf`): `MOTION_JUMP` (49) at 1.5,
/// sound 0x23, and sometimes a voice. The chance of a voice starts at 5 (`0x14023429f`); each jump
/// rolls it (`0x1404ca5f0`): a hit plays a voice clip from 11315 to 11317 for Escha (hero 0) or
/// 11318 to 11320 for Logy (`0x1404ca660`, both ends included; `0x140160960`) and sets the chance
/// back to 5, a miss raises it by 10.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JumpVoice {
    pub chance: i32,
}

pub const JUMP_MOTION: u32 = 49;
pub const JUMP_SOUND: u32 = 0x23;

impl Default for JumpVoice {
    fn default() -> Self {
        JumpVoice { chance: 5 }
    }
}

impl JumpVoice {
    /// One jump's voice, if any, for `hero` (0 Escha, else Logy).
    pub fn jump(&mut self, hero: u32, rng: &mut crate::rng::Rng) -> Option<u32> {
        if rng.chance(self.chance) {
            let top = if hero == 0 { 0x2c35 } else { 0x2c38 };
            let clip = rng.range(top - 2, top) as u32;
            self.chance = 5;
            Some(clip)
        } else {
            self.chance += 10;
            None
        }
    }
}

/// A character's gravity, metres a second squared (its +0x164, set to 0x416b5c0f by
/// `0x14026b9c9`).
pub const GRAVITY: f32 = 14.71;
/// The jump's launch speed upward, metres a second (`0x140234f80`: 5, or 25 when the character's
/// +0x12d is set), given as it leaves the ground (the jump's second phase).
pub const JUMP_SPEED: f32 = 5.0;
pub const JUMP_SPEED_HIGH: f32 = 25.0;

/// How far above where it left the ground a character launched at `speed` (metres a second) is
/// after `t` seconds, in field units (`0x14026bc80`: each frame it moves by v dt - a dt^2 / 2,
/// the height times 100, and its speed drops by a dt). Below 0 it has come down.
pub fn jump_height(speed: f32, t: f32) -> f32 {
    100.0 * (speed * t - GRAVITY * t * t / 2.0)
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
    fn the_stick_leans_the_speed() {
        assert_eq!(player_move([0.1, 0.1], 1.0, false, false), [0.0, 0.0], "inside the dead zone");
        let half = player_move([0.0, 0.5], 1.0, false, false);
        assert!((half[1] - 250.0).abs() < 1e-3 && half[0] == 0.0, "{half:?}");
        assert!((player_move([0.0, 1.0], 1.0, false, true)[1] - 750.0).abs() < 1e-3, "run toggle on");
        let full = player_move([3.0, 4.0], 0.5, true, false);
        assert!((full[0] - 187.5).abs() < 1e-3 && (full[1] - 250.0).abs() < 1e-3, "past full lean: {full:?}");
    }

    #[test]
    fn the_speed_picks_the_gait() {
        assert_eq!(player_gait(0.0, false), (Gait::Wait, 1.0));
        assert_eq!(player_gait(200.0, false), (Gait::Walk, 1.0), "under 0.42 of 500");
        assert_eq!(player_gait(500.0, false).0, Gait::Run);
        assert!((player_gait(500.0, false).1 - 1.125).abs() < 1e-6);
        assert_eq!(player_gait(250.0, true).0, Gait::Walk, "under 0.42 of 625");
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

    #[test]
    fn jump_voices_grow_likelier_until_one_plays() {
        let mut v = JumpVoice::default();
        let mut rng = crate::rng::Rng::from_state([1, 2, 3, 4]);
        let mut misses = 0;
        let clip = loop {
            match v.jump(0, &mut rng) {
                Some(c) => break c,
                None => misses += 1,
            }
            assert_eq!(v.chance, 5 + 10 * misses);
        };
        assert!((11315..=11317).contains(&clip) && v.chance == 5);
        // By 100 the voice is certain.
        let mut v = JumpVoice { chance: 100 };
        assert!(v.jump(1, &mut rng).is_some_and(|c| (11318..=11320).contains(&c)));
    }

    #[test]
    fn a_jump_rises_85_units_and_lands_in_0_68_s() {
        let peak = JUMP_SPEED / GRAVITY;
        assert!((jump_height(JUMP_SPEED, peak) - 85.0).abs() < 0.1);
        assert!(jump_height(JUMP_SPEED, 2.0 * peak).abs() < 1e-3 && (2.0 * peak - 0.68).abs() < 0.01);
    }
}
