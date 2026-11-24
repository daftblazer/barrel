//! Ryza as the player walks her: where she stands, which way she faces, and which of her
//! field motions plays (`motion`). The input is a lean on the ground (game space x, z, length
//! up to 1: the stick against the camera with its dead zone out, `crate::walk`); a light lean
//! walks, a full one runs. She moves along the lean from the first frame at a speed
//! proportional to it, her facing turning toward it at `TURN_RATE`. Setting off plays the
//! gait's start motion and then its loop, letting go its stop motion and then the stand; walk
//! and run change straight from loop to loop. Each change mixes the last motion out over
//! `BLEND`. Stand-ins, not read from the game: the lean that starts a run, her size against
//! the walls and the mixing time (`docs/ryza/open.md`, "Ryza in the field").

use barrel_engine::walk::{Ground, Walls};
use glam::Vec3;

use crate::motion;

// Measured: walk-30 frames 1900-2019 (a full lean moves her 10.0 units a frame along the stick).
/// Her speed at a full lean, units (centimetres) a second; it is proportional to the lean.
pub const FULL_SPEED: f32 = 600.0;
// STAND-IN: The lean that starts a run is chosen, not read from the game.
/// The lean at and above which she runs.
pub const RUN_LEAN: f32 = 0.6;
// Measured: walk-30 frames 2150-2170 (her facing turns 0.2 rad, 11.46 degrees, every frame).
/// How fast she turns toward where she goes, radians a second (0.2 a frame at 60 a second).
pub const TURN_RATE: f32 = 12.0;
// STAND-IN: The fallback collision size and step height are chosen; the real thing is the character tables' values and the game's own step rule.
/// Her size against the walls when the character tables give none (`chara::Chara`), and the
/// highest step she climbs.
pub const RADIUS: f32 = 30.0;
pub const HEIGHT: f32 = 170.0;
pub const STEP_UP: f32 = 60.0;
// Measured: walk-30 frames 2922-3164 (every other frame she climbs exactly 0.14).
/// The most she climbs in a frame after one that climbed more.
pub const CLIMB_CAP: f32 = 0.1435;

// STAND-IN: The mixing time between motions is chosen; the real thing is the game's own blend times.
/// How long the last motion takes to mix out, seconds.
pub const BLEND: f32 = 0.2;

// STAND-IN: How far into its loop the run motion is when it takes over from the start motion,
// seconds; the real thing is the game's own hand-over between the two, not read. Fitted on
// `walk-rasen` (the stick up from frame 1900): the pelvis's pinned cloth points, in the pelvis's
// space, are 3.1 cm rms off over the run with 0 and 1.0 cm off with 0.35 (0.32 to 0.38 within
// 0.2 cm of it; the loop is 0.75 s long). The walking loop's hand-over is not fitted.
const RUN_LOOP_PHASE: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Run,
}

impl Gait {
    /// Its start, loop and stop motions.
    fn motions(self) -> (u32, u32, u32) {
        match self {
            Gait::Stand => (motion::STAND, motion::STAND, motion::STAND),
            Gait::Walk => (motion::WALK_START, motion::WALK, motion::WALK_STOP),
            Gait::Run => (motion::RUN_START, motion::RUN, motion::RUN_STOP),
        }
    }
}

pub struct Player {
    /// Her feet, game space.
    pub at: Vec3,
    /// Which way she faces, radians about y (0 faces +z, -pi/2 faces -x).
    pub turn: f32,
    /// The gait asked for (she may still be starting or stopping).
    pub gait: Gait,
    /// The motion playing and the seconds into it.
    pub motion: u32,
    pub clock: f32,
    /// The motion before it, held where it was left, and how long ago that was.
    pub from: Option<(u32, f32)>,
    pub since: f32,
    /// The motions' lengths, seconds (one second for a motion not listed).
    lengths: Vec<(u32, f32)>,
    /// The gait she moves at (the last moving one while she stops).
    moving: Gait,
    /// Her size against the walls (radius, height).
    pub radius: f32,
    pub height: f32,
    /// Her last move climbed more than `CLIMB_CAP`: the next one may climb only that much.
    climbed: bool,
    /// She was held by a wall last frame / is to stand this frame.
    touching: bool,
    stall: bool,
}

impl Player {
    pub fn new(at: Vec3, turn: f32, lengths: &[(u32, f32)]) -> Player {
        Player { at, turn, gait: Gait::Stand, motion: motion::STAND, clock: 0.0, from: None, since: 0.0, lengths: lengths.to_vec(), moving: Gait::Stand, radius: RADIUS, height: HEIGHT, climbed: false, touching: false, stall: false }
    }

    fn length(&self, m: u32) -> f32 {
        self.lengths.iter().find(|(n, _)| *n == m).map_or(1.0, |&(_, l)| l.max(1e-3))
    }

    fn play(&mut self, m: u32) {
        if m != self.motion {
            self.from = Some((self.motion, self.time()));
            self.since = 0.0;
            self.motion = m;
            self.clock = if m == motion::RUN { RUN_LOOP_PHASE } else { 0.0 };
        }
    }

    /// One step of `dt` seconds with the lean `lean` (game x, z; its length 0 to 1); how far she
    /// went (x, z).
    pub fn step(&mut self, lean: [f32; 2], dt: f32, ground: Option<&Ground>, walls: Option<&Walls>) -> [f32; 2] {
        let amount = (lean[0] * lean[0] + lean[1] * lean[1]).sqrt().min(1.0);
        self.gait = if amount <= 0.0 { Gait::Stand } else if amount < RUN_LEAN { Gait::Walk } else { Gait::Run };
        self.clock += dt;
        self.since += dt;
        let done = self.clock >= self.length(self.motion);
        let (_, walk, walk_stop) = Gait::Walk.motions();
        let (_, run, run_stop) = Gait::Run.motions();
        let stopping = self.motion == walk_stop || self.motion == run_stop;
        // Which motion now (her position does not follow it: she moves at the lean's speed).
        match self.gait {
            Gait::Stand => {
                if self.motion == motion::STAND || (stopping && done) {
                    self.play(motion::STAND);
                    self.moving = Gait::Stand;
                } else {
                    let (_, _, stop) = self.moving.motions();
                    self.play(stop);
                }
                return [0.0; 2];
            }
            g => {
                let (start, looped, _) = g.motions();
                if self.motion == looped || (self.motion == start && done) || self.motion == walk || self.motion == run {
                    // Into the loop, or from one gait's loop straight to the other's.
                    self.play(looped);
                } else if self.motion != start {
                    self.play(start);
                }
            }
        }
        self.moving = self.gait;
        // Along the lean, at once, at its speed.
        let delta = [lean[0] * FULL_SPEED * dt, lean[1] * FULL_SPEED * dt];
        let free = [self.at.x + delta[0], self.at.z + delta[1]];
        let mut to = match walls {
            // A wall whose top is within a step of her feet (a floor's edge, a kerb) she steps
            // over: the walls see her from a step up.
            Some(w) => w.step([self.at.x, self.at.z], delta, self.at.y + STEP_UP, self.height - STEP_UP, self.radius),
            None => free,
        };
        let before = self.at;
        let slid = (to[0] - free[0]).abs() + (to[1] - free[1]).abs() > 1e-3;
        if let (Some(w), true, false) = (walls, slid, self.touching) {
            // Measured: walk-30 frames 2915-2918: she runs up to the wall and stops where she
            // meets it, loses the rest of the move, and stands the next frame too.
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            for _ in 0..16 {
                let mid = 0.5 * (lo + hi);
                let part = [delta[0] * mid, delta[1] * mid];
                let end = w.step([self.at.x, self.at.z], part, self.at.y + STEP_UP, self.height - STEP_UP, self.radius);
                if (end[0] - self.at.x - part[0]).abs() + (end[1] - self.at.z - part[1]).abs() > 1e-3 { hi = mid } else { lo = mid }
            }
            to = [self.at.x + delta[0] * lo, self.at.z + delta[1] * lo];
            self.stall = true;
        } else if self.stall {
            to = [self.at.x, self.at.z];
            self.stall = false;
        }
        self.touching = slid;
        match ground {
            // Off the ground, or up a ledge higher than a step: she stays.
            Some(g) => {
                if self.climbed {
                    // Cut the move where the ground has risen CLIMB_CAP.
                    let reach = |t: f32| g.height(before.x + (to[0] - before.x) * t, before.z + (to[1] - before.z) * t, before.y, STEP_UP);
                    if reach(1.0).is_some_and(|y| y - before.y > CLIMB_CAP) {
                        let (mut lo, mut hi) = (0.0f32, 1.0f32);
                        for _ in 0..24 {
                            let mid = 0.5 * (lo + hi);
                            if reach(mid).is_some_and(|y| y - before.y > CLIMB_CAP) { hi = mid } else { lo = mid }
                        }
                        to = [before.x + (to[0] - before.x) * lo, before.z + (to[1] - before.z) * lo];
                    }
                }
                if let Some(y) = g.height(to[0], to[1], self.at.y, STEP_UP) {
                    self.climbed = slid && y - before.y > CLIMB_CAP;
                    self.at = Vec3::new(to[0], y, to[1]);
                }
            }
            None => self.at = Vec3::new(to[0], self.at.y, to[1]),
        }
        let moved = [self.at.x - before.x, self.at.z - before.z];
        // Turn toward where the stick points (a wall she slides along does not turn her, nor does
        // being held), the short way round.
        // Measured: walk-30 frames 2917-3060 (against the fence her facing keeps the stick's
        // direction, the camera's yaw a frame late, as she slides 70 degrees across it and when she
        // stands stuck in its corner).
        {
            let want = lean[0].atan2(lean[1]);
            let mut d = (want - self.turn).rem_euclid(std::f32::consts::TAU);
            // Measured: walk-30 frames 2560-2575 and 2780-2795 (179.2 degrees the negative way
            // and 179.99 the positive way: the short way, however near half a turn).
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            self.turn += d.clamp(-TURN_RATE * dt, TURN_RATE * dt);
        }
        moved
    }

    /// Seconds into the motion playing: around again for the loops, held at the end otherwise.
    fn time(&self) -> f32 {
        let l = self.length(self.motion);
        if [motion::STAND, motion::WALK, motion::RUN].contains(&self.motion) { self.clock % l } else { self.clock.min(l) }
    }

    /// The motion playing now and the time into it, and the one mixing out with how much of
    /// it is left (0 to 1).
    pub fn pose(&self) -> ((u32, f32), Option<((u32, f32), f32)>) {
        let left = 1.0 - (self.since / BLEND).min(1.0);
        ((self.motion, self.time()), self.from.filter(|_| left > 0.0).map(|f| (f, left)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTHS: [(u32, f32); 7] = [(0, 2.5), (10, 0.6), (11, 1.35), (12, 0.65), (20, 0.25), (21, 0.75), (22, 0.65)];

    fn run(p: &mut Player, lean: [f32; 2], seconds: f32, walls: Option<&Walls>) {
        for _ in 0..(seconds * 60.0) as usize {
            p.step(lean, 1.0 / 60.0, None, walls);
        }
    }

    #[test]
    fn she_starts_loops_and_stops() {
        let mut p = Player::new(Vec3::ZERO, 0.0, &LENGTHS);
        p.step([0.0, 1.0], 1.0 / 60.0, None, None);
        assert_eq!(p.motion, motion::RUN_START);
        run(&mut p, [0.0, 1.0], 1.0, None);
        assert_eq!(p.motion, motion::RUN);
        let z = p.at.z;
        run(&mut p, [0.0, 0.3], 0.1, None);
        assert_eq!(p.motion, motion::WALK);
        run(&mut p, [0.0, 0.0], 0.1, None);
        assert_eq!(p.motion, motion::WALK_STOP);
        run(&mut p, [0.0, 0.0], 1.0, None);
        assert_eq!(p.motion, motion::STAND);
        assert!(p.at.z > z);
        // Mixing out the stop motion, then not.
        assert!(p.pose().1.is_none());
    }

    #[test]
    fn a_change_mixes_the_last_motion_out() {
        let mut p = Player::new(Vec3::ZERO, 0.0, &LENGTHS);
        run(&mut p, [0.0, 1.0], 0.1, None);
        let (_, from) = p.pose();
        let ((m, _), left) = from.unwrap();
        assert_eq!(m, motion::STAND);
        assert!(left > 0.0 && left < 1.0);
    }

    #[test]
    fn a_wall_stops_her() {
        let wall = barrel_engine::walk::Wall { a: [-500.0, 200.0], b: [500.0, 200.0], low: -100.0, high: 300.0, outline: None };
        let walls = Walls::from_walls(vec![wall], 1000.0);
        let mut p = Player::new(Vec3::ZERO, 0.0, &LENGTHS);
        run(&mut p, [0.0, 1.0], 2.0, Some(&walls));
        assert!((p.at.z - (200.0 - RADIUS)).abs() < 1.0, "{:?}", p.at);
    }

    #[test]
    fn she_steps_over_a_low_wall() {
        let kerb = barrel_engine::walk::Wall { a: [-500.0, 200.0], b: [500.0, 200.0], low: -100.0, high: STEP_UP * 0.5, outline: None };
        let walls = Walls::from_walls(vec![kerb], 1000.0);
        let mut p = Player::new(Vec3::ZERO, 0.0, &LENGTHS);
        run(&mut p, [0.0, 1.0], 2.0, Some(&walls));
        assert!(p.at.z > 300.0, "{:?}", p.at);
    }

    #[test]
    fn she_turns_toward_the_lean() {
        let mut p = Player::new(Vec3::ZERO, 0.0, &LENGTHS);
        run(&mut p, [-1.0, 0.0], 1.0, None);
        assert!((p.turn + std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{}", p.turn);
    }
}
