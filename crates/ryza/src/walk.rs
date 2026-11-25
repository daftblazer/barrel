//! The field's walk, one step a frame: the sticks (or keys mapped onto them) move Ryza
//! against the camera, and the camera turns, pitches and follows her. The window and the
//! headless walk (`barrel-tool walk`) both step through [`step`], so what the walk tests see
//! is what the game plays.
//!
//! Directions are game space: yaw in degrees with the forward axis `(sin yaw, cos yaw)` on
//! (x, z), so the camera's right is `(-cos yaw, sin yaw)`. Sticks are -1 to 1, x right and y up.
//! The rules are measured on a 30 second walk of the original (`walk-30`, the frames named in
//! each constant's comment, `docs/ryza/open.md`, "Walking her, measured").

use barrel_engine::walk::{Ground, Wall, Walls};
use barrel_formats::elixir::Elixir;
use glam::Vec3;

use crate::install::Install;
use crate::player::Player;

// Measured: walk-30 frame 2040-2130 (a lean of 12000/32767 moves her 108.9 units a second of 600),
// frames 2640-2700 (6000/32767: not at all); a straight line through both, with the stick read as
// below (12000 is 46 of 127: 0.3622, 108.9 of 600 is 0.1815; 28/127 is 0.2205).
/// The left stick's dead zone, of its full throw; beyond it the lean rises linearly to 1.
pub const MOVE_DEAD_ZONE: f32 = 0.2208;
// Measured: walk-30 frames 2236-2262 and 2580-2584 (the left stick pushed fully left or down she
// moves 10.08 units a frame, 1.008 of the 10.00 of a full push right or up; at the corner of
// left and up too: 10.080, 10.082, 10.079) in the stick's direction. The stick's axes are read as
// signed bytes (the high byte of the 16-bit value, over 127): full right or up 32767 is 127/127,
// full left or down -32767 is -128/127 = 1.00787. A lean longer than that does not go on: the cap
// below. Only the corner of left and up is sampled; the corner of right and up is taken to be
// capped the same (STAND-IN: not walked).
/// The longest the left stick's lean gets, the full negative throw's byte over the full positive's.
pub const MOVE_LONGEST: f32 = 128.0 / 127.0;
// Measured: walk-30 frames 2340-2400 (right stick x full turns the camera 1.5 degrees a frame) and
// frames 2420-2540 (-20000/32767 turns it 0.8 a frame), a straight line through both.
/// The right stick's dead zone, of its full throw.
pub const LOOK_DEAD_ZONE: f32 = 0.1651;
// Measured: walk-30 frames 2340-2400.
/// The camera's turn at a full right stick, degrees a second (1.5 a frame at 60).
pub const CAMERA_TURN_RATE: f32 = 90.0;
// Measured: walk-30 frames 2150-2170 (full lean to the camera's right: 1.0125 degrees a frame) and
// frames 2230-2330 (to its front left, 45 degrees: 0.71 times that). Only full leans were
// walked, so how it depends on the lean's length is not measured; it is taken as none.
// Measured: walk-30 frames 2918-3017 (sliding along the fence at 180 to 520 units a second, the camera
// turned 1.19 times what her speed over 600 a second gives: 1.19 times 1/600 is 1/504).
/// The speed (units a second) from which the camera's follow is at its full rate; below it the rate
/// falls in proportion (a slide along a wall is slow; a run is full speed).
pub const FOLLOW_FULL_SPEED: f32 = 504.0;
// Measured: walk-30 frames 2250-2310 (the lean at 45 degrees to the camera, which follows her 0.712
// a frame: her facing, which keeps the stick's direction, is 0.22 degrees ahead of the camera as
// the frame began), frames 2210-2229 (at 90 degrees, 1.01 a frame: exactly the camera as the frame
// began) and 2430-2540 (the right stick turning it 0.8 a frame, the lean ahead: exactly it). The
// lead is taken as 0.44 times sin(a) cos(a) for a lean at the angle `a` to the camera: the
// two angles sampled, its form between them (and behind her) is a stand-in. Why it is there is
// not found.
/// How far her lean is taken ahead of the camera as the frame began, degrees, at 45 degrees across it.
pub const FOLLOW_LEAD: f32 = 0.44;
// FITTED: with `FOLLOW_LEAD` the turn is 0.7121 a frame at 45 degrees (the 0.7125 measured) when the
// across angle is taken that part of the turn short; 0.5 without the lead.
/// The part of its own turn the camera's follow is taken across less.
pub const FOLLOW_MID: f32 = 0.77;
/// How fast the camera turns toward where she goes, at a right angle to its view, degrees a second.
pub const FOLLOW_RATE: f32 = 60.75;
// Measured: walk-30 frames 2720-2754 (right stick y 20000/32767: 0.376 degrees a frame for the
// first 8 frames of holding, then 0.50; with the dead zone out the stick is 0.533).
/// The camera's pitch at a full right stick y, degrees a second after the ramp (0.9375 a frame).
pub const PITCH_RATE: f32 = 56.25;
/// While the stick has been held for `PITCH_RAMP` seconds or less, the pitch moves at `PITCH_RAMP_GAIN` of its rate.
pub const PITCH_RAMP: f32 = 8.0 / 60.0;
pub const PITCH_RAMP_GAIN: f32 = 0.75;
// Measured: walk-30 frames 2728-2760: from 5 degrees down the eye comes in and the point it looks
// at rises, linearly with the pitch (400 and 128.5 at 5 degrees; 335.0 and 152.7 at -9.12, taking
// the view's pitch and the eye's offset from her). In the data the pull and the pitch both stop
// at frame 2760 and the pitch slows over its last 5 frames; whether this is the pitch's rule or
// the ground pulling the camera in (a collision matter) is not split: the ground is flat there.
/// The pitch below which the camera comes in and up.
pub const PULL_PITCH: f32 = 5.0;
/// How much the eye comes in and the look point rises, units a degree below `PULL_PITCH`.
pub const PULL_IN: f32 = 4.60;
pub const PULL_UP: f32 = 1.71;
// Measured: walk-30 frame 2760 on (the pitch stays there through frame 3680).
/// The lowest pitch of the follow camera, degrees (positive looks down), where the walk's stopped; whether it is a limit or the release is not split.
pub const PITCH_FLOOR: f32 = -9.12;
// Measured: walk-30 frames 1900-2700, the eye's height above her feet and its distance at pitch 8
// (184.2 at 400 along the view: the look point is 128.5 above her feet).
/// How high above her feet the camera looks at pitches of `PULL_PITCH` and above.
pub const LOOK_HEIGHT: f32 = 128.5;

/// What the player holds: both sticks, x right and y up, each of a throw of 1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sticks {
    pub left: [f32; 2],
    pub right: [f32; 2],
}

/// A stick's throw with its radial dead zone taken out: 0 inside it, rising to 1 at the edge
/// (a square corner, longer than 1, counts as the edge).
pub fn dead_zone(stick: [f32; 2], zone: f32) -> [f32; 2] {
    let len = (stick[0] * stick[0] + stick[1] * stick[1]).sqrt();
    if len <= zone {
        return [0.0; 2];
    }
    let k = ((len - zone) / (1.0 - zone)).min(1.0) / len;
    [stick[0] * k, stick[1] * k]
}

/// How the Camera tab's rows scale the right stick: a factor on the turn and on the pitch,
/// negative for an inverted axis. The rows' default (normal, speed 5) is 1 each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feel {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Feel {
    fn default() -> Feel {
        Feel { yaw: 1.0, pitch: 1.0 }
    }
}

impl Feel {
    /// The speed rows (1 to 10, default 5) are linear in the rate, 5 being the measured rate below.
    // STAND-IN: the game's speed curve is not measured; only the default (5) is the captures'.
    pub fn of(invert_horizontal: bool, invert_vertical: bool, horizontal_speed: i32, vertical_speed: i32) -> Feel {
        let axis = |inverted: bool, speed: i32| speed.clamp(1, 10) as f32 / 5.0 * if inverted { -1.0 } else { 1.0 };
        Feel { yaw: axis(invert_horizontal, horizontal_speed), pitch: axis(invert_vertical, vertical_speed) }
    }
}

/// The follow camera: where it looks from, behind and above her.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// The Camera tab's scaling of the right stick.
    pub feel: Feel,
    /// Degrees; its forward axis is `(sin yaw, cos yaw)` on (x, z).
    pub yaw: f32,
    /// Degrees, positive looking down.
    pub pitch: f32,
    /// The distance asked for (the field's, zoomed), units.
    pub length: f32,
    pub pitch_range: (f32, f32),
    /// Seconds the right stick has been held up or down.
    held: f32,
    /// Whether looking up brings it in (the follow camera).
    pulls: bool,
}

impl Camera {
    pub fn new(yaw: f32, pitch: f32, length: f32, pitch_range: (f32, f32)) -> Camera {
        Camera { feel: Feel::default(), yaw, pitch, length, pitch_range, held: 0.0, pulls: false }
    }

    /// The camera that follows her: it does not go below `PITCH_FLOOR` and looking up brings it in.
    pub fn following(mut self) -> Camera {
        self.pitch_range.0 = self.pitch_range.0.max(PITCH_FLOOR);
        self.pulls = true;
        self
    }

    /// The right stick turns and pitches the camera (`right` as held, the dead zone not yet out).
    pub fn steer(&mut self, right: [f32; 2], dt: f32) {
        let [x, y] = dead_zone(right, LOOK_DEAD_ZONE);
        self.yaw -= x * CAMERA_TURN_RATE * self.feel.yaw * dt;
        if y == 0.0 {
            self.held = 0.0;
        } else {
            self.held += dt;
            let gain = if self.held <= PITCH_RAMP { PITCH_RAMP_GAIN } else { 1.0 };
            self.pitch = (self.pitch - y * PITCH_RATE * self.feel.pitch * gain * dt).clamp(self.pitch_range.0, self.pitch_range.1);
        }
    }

    /// While she moves along `way` (game x, z), taken against a camera of yaw `from`, the camera
    /// turns toward it, the more the more it is across that view.
    pub fn follow(&mut self, way: [f32; 2], from: f32, dt: f32) {
        let speed = way[0].hypot(way[1]) / dt;
        if speed < 1e-3 {
            return;
        }
        let across = (way[0].atan2(way[1]).to_degrees() - from).to_radians();
        // Measured: walk-30 frames 2230-2320 (the lean at 45 degrees to the camera, which turns
        // 0.7121 a frame, not 0.716): the turn is taken against the view as it is halfway
        // through it, so it is the fixed point of `turn = rate * sin(across - turn / 2)`.
        let rate = FOLLOW_RATE * (speed / FOLLOW_FULL_SPEED).min(1.0) * dt;
        let mut turn = rate * across.sin();
        for _ in 0..4 {
            turn = rate * (across - FOLLOW_MID * turn.to_radians()).sin();
        }
        self.yaw += turn;
    }

    /// How far below `PULL_PITCH` the view is looking up, degrees.
    fn below(&self) -> f32 {
        if self.pulls { (PULL_PITCH - self.pitch).max(0.0) } else { 0.0 }
    }

    /// How high above her feet the camera looks now.
    pub fn look_height(&self) -> f32 {
        LOOK_HEIGHT + PULL_UP * self.below()
    }

    /// How far the eye is from the point it looks at, before the ground's pull-in.
    pub fn distance(&self) -> f32 {
        self.length - PULL_IN * self.below()
    }

    /// The camera's eye for her feet at `feet`, game space, with the eye `dist` from the point
    /// it looks at (`distance`, or what the ground leaves of it).
    pub fn eye(&self, feet: Vec3, dist: f32) -> Vec3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        let look = feet + Vec3::new(0.0, self.look_height(), 0.0);
        look + Vec3::new(-y.sin() * p.cos(), p.sin(), -y.cos() * p.cos()) * dist
    }
}

/// A stick axis as the game reads it: the pad's 16-bit value (a throw of 1 is 32767) by its high
/// byte, over 127, so a full push left or down reads -128/127.
pub fn byte_axis(v: f32) -> f32 {
    f32::from((v * 32767.0).round().clamp(-32768.0, 32767.0) as i16 >> 8) / 127.0
}

/// The left stick's throw (x, y) with the dead zone taken out, its length 0 to `MOVE_LONGEST`: the
/// length from the axes as bytes, the direction the stick's own (a corner of left and up is
/// unskewed, walk-30 frames 2236-2419; read as bytes it would lean 0.2 degrees toward the left).
fn move_throw(left: [f32; 2]) -> [f32; 2] {
    let raw = left[0].hypot(left[1]);
    let len = byte_axis(left[0]).hypot(byte_axis(left[1]));
    if len <= MOVE_DEAD_ZONE || raw == 0.0 {
        return [0.0; 2];
    }
    let k = ((len - MOVE_DEAD_ZONE) / (1.0 - MOVE_DEAD_ZONE)).min(MOVE_LONGEST) / raw;
    [left[0] * k, left[1] * k]
}

/// The lean on the ground (game x, z; its length 0 to 1) of the left stick against a camera of
/// yaw `yaw` degrees.
pub fn lean(left: [f32; 2], yaw: f32) -> [f32; 2] {
    let [x, y] = move_throw(left);
    let (s, c) = yaw.to_radians().sin_cos();
    [s * y - c * x, c * y + s * x]
}

/// One frame of `dt` seconds: the sticks move her and the camera, which follows her.
pub fn step(player: &mut Player, cam: &mut Camera, sticks: &Sticks, dt: f32, ground: Option<&Ground>, walls: Option<&Walls>) {
    // She goes where the stick pointed against the camera as the frame began.
    // Measured: walk-30 frames 2420-2559 (her facing keeps a frame behind a camera turning, which
    // turns exactly the stick's 0.8 degrees a frame: the follow is taken against the same yaw).
    let from = cam.yaw;
    let across = lean(sticks.left, 0.0);
    let a = across[0].atan2(across[1]);
    let way = lean(sticks.left, from + FOLLOW_LEAD * a.sin() * a.cos() * (across[0].hypot(across[1])).min(1.0));
    cam.steer(sticks.right, dt);
    let moved = player.step(way, dt, ground, walls);
    // The camera follows where she went, which against a wall is along it.
    cam.follow(moved, from, dt);
}

/// What she walks on and against, loaded from a field.
pub struct Terrain {
    pub ground: Option<Ground>,
    pub walls: Option<Walls>,
    /// The gimmicks' bodies, already among the walls, by `fixID` (to take one out when its point is spent).
    pub bodies: Vec<(u32, Vec<Wall>)>,
    /// The point asked for with its height from the heightmap (as given when there is none).
    pub start: Vec3,
}

/// A field's heightmap alone as a ground (`<AREA>.khm`).
pub fn heightmap_ground(main: &Elixir, area: &str) -> Option<Ground> {
    barrel_engine::field::named(main, &format!("{}.khm", area.to_ascii_uppercase()))
        .or_else(|| main.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".khm")).map(|e| main.data(e)))
        .and_then(|b| barrel_formats::khm::parse(b).ok())
        .map(Ground::of_heightmap)
}

/// What a body's wall stands out from its radius (units).
// STAND-IN: fitted to `gather-rasen2`: she slid round the first honeycomb (radius 0.5 m, scale 1.2) on a circle 91.2 units from its centre
// (her x along the slide 33354.0 to 33354.8 over z 30022 to 30041 is that circle's), 60 of them the body's and 28 hers; what the
// remaining 3.2 is (a contact margin, a larger shape) is not known.
const BODY_SKIN: f32 = 3.2;

/// The bodies of the field's gimmicks as walls, by the gimmick's `fixID`: each model that has a collision radius in `charabase.xml` (`FMrad`, in metres,
/// times the placement's scale; `FMhig` its height) is a cylinder she cannot walk through (a post: a wall of no length and the body's
/// radius). The field's own tables do not place these models (`<AREA>.opl2` has the trees and rocks), so the game's own collision for
/// them is the cylinder.
// STAND-IN: the boxes (`Fmdepth`, the wooden boxes) are cylinders of their `FMrad` too; the game's own shape is not read. Checked against
// `gather-rasen2`'s walks by the honeycombs (radius 0.5, scale 1.2): she stopped 91 and 100 units from their centres, and walked through
// the first one's place once it was gathered (a point that is spent has no body: the callers take it out with `Walls::remove`).
pub fn gimmick_walls(charabase: &[u8], gimmicks: &[crate::gimmick::Gimmick]) -> Vec<(u32, Vec<Wall>)> {
    let Ok(root) = barrel_formats::table::parse(charabase) else { return Vec::new() };
    let number = |tag: &str, key: &str| {
        let row = root.children.iter().find(|r| r.attr("BaseTag") == Some(tag))?;
        row.attr(key)?.trim().parse::<f32>().ok()
    };
    let mut out = Vec::new();
    for g in gimmicks {
        let (Some(radius), Some(height)) = (number(&g.tag, "FMrad"), number(&g.tag, "FMhig")) else { continue };
        let high = g.position[1] + height * 100.0 * g.scale[1];
        let at = [g.position[0], g.position[2]];
        let thick = radius * 100.0 * g.scale[0].max(g.scale[2]) + BODY_SKIN;
        out.push((g.id, vec![Wall { a: at, b: at, low: g.position[1] - 100.0, high, outline: None, thick }]));
    }
    out
}

/// A field's ground and walls as she walks on them: the heightmap (`<AREA>.khm`), the field's
/// own walls (`<field>_le.koc`), the placed models' collision meshes (`solids`) as more walls
/// and floors (added after the start is stood on the heightmap, so she does not start on a
/// tree's top) and the field's own shapes (`<AREA>_coll.bin`). `at` is where she starts.
pub fn load_terrain(install: &Install, field: &str, area: &str, main: &Elixir, objects: &Elixir, at: Vec3, gimmicks: &[crate::gimmick::Gimmick]) -> Result<Terrain, Box<dyn std::error::Error>> {
    let mut ground = heightmap_ground(main, area);
    let walls = install
        .read(&format!("\\data\\x64\\field\\{0}\\{0}_le.koc", field.to_ascii_lowercase()))
        .ok()
        .and_then(|b| crate::koc::parse(&b).ok())
        .map(|q| Walls::from_walls(crate::koc::walls(&q), 2000.0));
    // The walkable mesh's borders: walls as high as she can be (the fences' only walls).
    let borders = install
        .read(&format!("\\data\\x64\\field\\{0}\\{0}.g1pf", field.to_ascii_lowercase()))
        .ok()
        .and_then(|b| barrel_formats::g1pf::parse(&b).ok())
        .map(|m| Walls::from_walls(m.borders().into_iter().map(|(a, b)| Wall { a, b, low: f32::MIN, high: f32::MAX, outline: None, thick: 0.0 }).collect(), 2000.0));
    let walls = match (walls, borders) {
        (Some(w), Some(b)) => Some(w.join(b)),
        (w, b) => w.or(b),
    };
    let bodies = install.read("\\saves\\chara\\charabase.xml").map(|b| gimmick_walls(&b, gimmicks)).unwrap_or_default();
    let body_walls = || Walls::from_walls(bodies.iter().flat_map(|(_, w)| w.iter().copied()).collect(), 2000.0);
    let walls = match (walls, bodies.is_empty()) {
        (w, true) => w,
        (Some(w), false) => Some(w.join(body_walls())),
        (None, false) => Some(body_walls()),
    };
    let placements = barrel_engine::field::named(main, &format!("{area}.opl2")).map(barrel_formats::opl2::parse).transpose()?;
    let (mut solids, fit) = placements.map(|p| crate::solids::placed_meshes(&p, objects)).unwrap_or_default();
    eprintln!("model collision: {fit:?}");
    if let Some(b) = main.entries.iter().find(|e| !e.is_dummy() && e.name.to_ascii_lowercase().ends_with("_coll.bin")).map(|e| main.data(e)) {
        solids.extend(crate::solids::field_shapes(b)?);
    }
    let start = match ground.as_ref().and_then(|g| g.height(at.x, at.z, f32::MAX / 2.0, 0.0)) {
        Some(y) => Vec3::new(at.x, y, at.z),
        None => at,
    };
    let slope = barrel_engine::walk::DEFAULT_SLOPE;
    if let Some(g) = ground.as_mut() {
        for c in &solids {
            g.add_floors(c, slope);
        }
    }
    let walls = solids.iter().fold(walls, |w, c| {
        let more = Walls::new(c, slope);
        Some(match w {
            Some(w) => w.join(more),
            None => more,
        })
    });
    eprintln!("model collision: {} meshes, {} triangles", solids.len(), solids.iter().map(|c| c.triangles.len()).sum::<usize>());
    Ok(Terrain { ground, walls, bodies, start })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn a_gimmicks_model_is_a_body_of_its_radius_and_scale() {
        let base = br#"<Root><CharaBase BaseTag="FM_HONEYCOMB_01" FMrad="0.5" FMhig="10"/></Root>"#;
        let comb = crate::gimmick::Gimmick { id: 51, tag: "FM_HONEYCOMB_01".into(), position: [33445.0, 6198.0, 30027.0], yaw: 0.0, rotation: [0.0; 3], scale: [1.2; 3], story: Default::default(), span: None };
        let other = crate::gimmick::Gimmick { id: 7, tag: "NO_SUCH".into(), ..comb.clone() };
        let bodies = gimmick_walls(base, &[comb, other]);
        assert_eq!(bodies.len(), 1, "only a model with a collision radius has a body");
        let (id, walls) = &bodies[0];
        assert_eq!((*id, walls.len()), (51, 1));
        // A post at its place, standing 60 units and its skin out from the centre.
        assert_eq!((walls[0].a, walls[0].b), ([33445.0, 30027.0], [33445.0, 30027.0]));
        assert!((walls[0].thick - (60.0 + BODY_SKIN)).abs() < 0.01, "{:?}", walls[0]);
    }

    fn player() -> Player {
        Player::new(Vec3::ZERO, 0.0, &[])
    }

    fn cam() -> Camera {
        Camera::new(0.0, 8.0, 400.0, (-30.0, 45.0)).following()
    }

    #[test]
    fn the_dead_zone_is_taken_out() {
        assert_eq!(move_throw([6000.0 / 32767.0, 0.0]), [0.0; 2]);
        let m = move_throw([12000.0 / 32767.0, 0.0])[0];
        assert!((m * 600.0 - 108.9).abs() < 0.5, "{m}");
        let m = dead_zone([-20000.0 / 32767.0, 0.0], LOOK_DEAD_ZONE)[0];
        assert!((m * 90.0 / 60.0 + 0.8).abs() < 0.01, "{m}");
        // The axes are bytes: full right is 127/127, full left 128/127 (walk-30 frames 2236-2262, 3100-3299).
        assert_eq!(byte_axis(1.0), 1.0);
        assert_eq!(byte_axis(-1.0), -MOVE_LONGEST);
        assert_eq!(move_throw([1.0, 0.0]), [1.0, 0.0]);
        assert!((move_throw([-1.0, 0.0])[0] + MOVE_LONGEST).abs() < 1e-5);
        // A corner of left and up is the longest lean, along the stick (frames 2236-2262).
        let [x, y] = move_throw([-1.0, 1.0]);
        assert!((x.hypot(y) - MOVE_LONGEST).abs() < 1e-5);
    }

    #[test]
    fn she_runs_at_once_at_the_speed_of_the_lean() {
        let (mut p, mut c) = (player(), cam());
        step(&mut p, &mut c, &Sticks { left: [0.0, 1.0], right: [0.0; 2] }, DT, None, None);
        assert!((p.at.z - 10.0).abs() < 1e-3, "{:?}", p.at);
        step(&mut p, &mut c, &Sticks::default(), DT, None, None);
        assert!((p.at.z - 10.0).abs() < 1e-3, "no easing out: {:?}", p.at);
    }

    #[test]
    fn she_goes_where_the_stick_points_against_the_camera() {
        // The camera looks along +z: right is -x. Turned 90 degrees to look along +x, forward is +x.
        let (mut p, mut c) = (player(), cam());
        step(&mut p, &mut c, &Sticks { left: [1.0, 0.0], right: [0.0; 2] }, DT, None, None);
        assert!(p.at.x < -9.9 && p.at.z.abs() < 1.0, "{:?}", p.at);
        let (mut p, mut c) = (player(), cam());
        c.yaw = 90.0;
        step(&mut p, &mut c, &Sticks { left: [0.0, 1.0], right: [0.0; 2] }, DT, None, None);
        assert!(p.at.x > 9.9 && p.at.z.abs() < 1.0, "{:?}", p.at);
    }

    #[test]
    fn she_turns_a_fifth_of_a_radian_a_frame() {
        let (mut p, mut c) = (player(), cam());
        step(&mut p, &mut c, &Sticks { left: [-1.0, 0.0], right: [0.0; 2] }, DT, None, None);
        assert!((p.turn - 0.2).abs() < 1e-5, "{}", p.turn);
    }

    #[test]
    fn the_right_stick_turns_the_camera_a_degree_and_a_half_a_frame() {
        let (mut p, mut c) = (player(), cam());
        step(&mut p, &mut c, &Sticks { left: [0.0; 2], right: [1.0, 0.0] }, DT, None, None);
        assert!((c.yaw + 1.5).abs() < 1e-4, "{}", c.yaw);
    }

    #[test]
    fn the_camera_rows_flip_and_scale_the_right_stick() {
        let turn = |feel: Feel, right: [f32; 2]| {
            let (mut p, mut c) = (player(), cam());
            c.feel = feel;
            step(&mut p, &mut c, &Sticks { left: [0.0; 2], right }, DT, None, None);
            (c.yaw, c.pitch - 8.0)
        };
        let (yaw, pitch) = turn(Feel::default(), [1.0, 1.0]);
        let (flipped_yaw, flipped_pitch) = turn(Feel::of(true, true, 5, 5), [1.0, 1.0]);
        assert!((yaw + flipped_yaw).abs() < 1e-5 && yaw < 0.0);
        assert!((pitch + flipped_pitch).abs() < 1e-5 && pitch < 0.0);
        let (fast, _) = turn(Feel::of(false, false, 10, 5), [1.0, 1.0]);
        assert!((fast / yaw - 2.0).abs() < 1e-4);
        assert_eq!(Feel::of(false, false, 5, 5), Feel::default());
    }

    #[test]
    fn the_camera_follows_her_across_its_view_and_not_along_it() {
        let (mut p, mut c) = (player(), cam());
        step(&mut p, &mut c, &Sticks { left: [0.0, 1.0], right: [0.0; 2] }, DT, None, None);
        assert!(c.yaw.abs() < 1e-5);
        step(&mut p, &mut c, &Sticks { left: [1.0, 0.0], right: [0.0; 2] }, DT, None, None);
        assert!((c.yaw + 1.01).abs() < 1e-2, "{}", c.yaw);
    }

    #[test]
    fn the_camera_follows_a_diagonal_run_at_seven_tenths_of_a_degree_and_her_lean_leads_it() {
        // Walk-30 frames 2236-2262: the lean at 45 degrees to the camera, which turns 0.7125 a frame
        // (not 1.0125 times sin 45 degrees); she goes 0.22 degrees past 45 across the camera as the frame began.
        let (mut p, mut c) = (player(), cam());
        let diagonal = Sticks { left: [1.0, 1.0], right: [0.0; 2] };
        for _ in 0..4 {
            step(&mut p, &mut c, &diagonal, DT, None, None);
        }
        let before = (c.yaw, p.at);
        step(&mut p, &mut c, &diagonal, DT, None, None);
        assert!(((c.yaw - before.0).abs() - 0.7121).abs() < 0.004, "{}", c.yaw - before.0);
        let moved = (p.at.x - before.1.x, p.at.z - before.1.z);
        let way = moved.0.atan2(moved.1).to_degrees() - before.0;
        assert!((way.abs() - 45.22).abs() < 0.1, "{way}");
    }

    #[test]
    fn against_a_wall_she_slides_facing_the_stick_and_the_camera_follows_the_slide() {
        let wall = barrel_engine::walk::Wall { a: [-5000.0, 100.0], b: [5000.0, 100.0], low: -100.0, high: 300.0, outline: None, thick: 0.0 };
        let walls = Walls::from_walls(vec![wall], 1000.0);
        let (mut p, mut c) = (player(), cam());
        let diagonal = Sticks { left: [1.0, 1.0], right: [0.0; 2] };
        for _ in 0..60 {
            step(&mut p, &mut c, &diagonal, DT, None, Some(&walls));
        }
        // Along the wall toward the camera's right (-x); she faces the stick's direction (45 degrees
        // right of the camera's, which the follow keeps turning toward her slide), not her slide.
        assert!(p.at.x < -100.0 && p.at.z < 100.0, "{:?}", p.at);
        assert!(c.yaw < -1.0, "{}", c.yaw);
        assert!((p.turn - (c.yaw - 45.0).to_radians()).abs() < 0.05, "{} {}", p.turn, c.yaw);
    }

    #[test]
    fn the_camera_follows_a_slow_slide_less_than_a_run() {
        // The same direction across the view at 600 and at 120 units a second.
        let mut run = cam();
        run.follow([10.0, 0.0], 0.0, DT);
        let mut slide = cam();
        slide.follow([2.0, 0.0], 0.0, DT);
        assert!((run.yaw - 1.0125).abs() < 1e-3, "{}", run.yaw);
        assert!((slide.yaw / run.yaw - 120.0 / FOLLOW_FULL_SPEED).abs() < 1e-3, "{}", slide.yaw);
    }

    #[test]
    fn looking_up_brings_the_camera_in() {
        let mut c = cam();
        assert_eq!((c.distance(), c.look_height()), (400.0, LOOK_HEIGHT));
        c.pitch = PITCH_FLOOR;
        assert!((c.distance() - 335.0).abs() < 1.0 && (c.look_height() - 152.7).abs() < 0.5);
        // The pitch stops at its floor.
        for _ in 0..600 {
            c.steer([0.0, 1.0], DT);
        }
        assert_eq!(c.pitch, PITCH_FLOOR);
    }
}
