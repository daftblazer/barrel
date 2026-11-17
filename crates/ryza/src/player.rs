//! Ryza as the player walks her: where she stands, which way she faces, and which of her
//! field motions plays (`motion`). The input is a lean on the ground (game space x, z, length
//! up to 1); a light lean walks, a full one runs. Setting off plays the gait's start motion
//! and then its loop, letting go its stop motion and then the stand; walk and run change
//! straight from loop to loop. Each change mixes the last motion out over `BLEND`. Stand-ins,
//! not read from the game: the speeds (eased in over the start motion and out over the stop
//! one), the lean that starts a run, how fast she turns, her size against the walls and the
//! mixing time (`docs/ryza/open.md`, "Ryza in the field").

use barrel_engine::walk::{Ground, Walls};
use glam::Vec3;

use crate::motion;

// STAND-IN: The walking and running speeds are chosen, not read from the game.
/// Walking and running speeds, units (centimetres) a second.
pub const WALK_SPEED: f32 = 150.0;
pub const RUN_SPEED: f32 = 480.0;
// STAND-IN: The lean that starts a run is chosen, not read from the game.
/// The lean at and above which she runs.
pub const RUN_LEAN: f32 = 0.6;
// STAND-IN: The turn rate is chosen, not read from the game.
/// How fast she turns toward where she goes, radians a second.
pub const TURN_RATE: f32 = 12.0;
// STAND-IN: The fallback collision size and step height are chosen; the real thing is the character tables' values and the game's own step rule.
/// Her size against the walls when the character tables give none (`chara::Chara`), and the
/// highest step she climbs.
pub const RADIUS: f32 = 30.0;
pub const HEIGHT: f32 = 170.0;
pub const STEP_UP: f32 = 60.0;

// STAND-IN: The mixing time between motions is chosen; the real thing is the game's own blend times.
/// How long the last motion takes to mix out, seconds.
pub const BLEND: f32 = 0.2;

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
    /// Her full walking and running speeds, units a second.
    pub walk_speed: f32,
    pub run_speed: f32,
}

impl Player {
    pub fn new(at: Vec3, turn: f32, lengths: &[(u32, f32)]) -> Player {
        Player { at, turn, gait: Gait::Stand, motion: motion::STAND, clock: 0.0, from: None, since: 0.0, lengths: lengths.to_vec(), moving: Gait::Stand, radius: RADIUS, height: HEIGHT, walk_speed: WALK_SPEED, run_speed: RUN_SPEED }
    }

    fn length(&self, m: u32) -> f32 {
        self.lengths.iter().find(|(n, _)| *n == m).map_or(1.0, |&(_, l)| l.max(1e-3))
    }

    fn play(&mut self, m: u32) {
        if m != self.motion {
            self.from = Some((self.motion, self.time()));
            self.since = 0.0;
            self.motion = m;
            self.clock = 0.0;
        }
    }

    /// One step of `dt` seconds with the lean `lean` (game x, z).
    pub fn step(&mut self, lean: [f32; 2], dt: f32, ground: Option<&Ground>, walls: Option<&Walls>) {
        let amount = (lean[0] * lean[0] + lean[1] * lean[1]).sqrt().min(1.0);
        self.gait = if amount < 0.05 { Gait::Stand } else if amount < RUN_LEAN { Gait::Walk } else { Gait::Run };
        self.clock += dt;
        self.since += dt;
        let done = self.clock >= self.length(self.motion);
        let (_, walk, walk_stop) = Gait::Walk.motions();
        let (_, run, run_stop) = Gait::Run.motions();
        let stopping = self.motion == walk_stop || self.motion == run_stop;
        // Which motion now, and how much of the gait's speed she moves at.
        let speed = match self.gait {
            Gait::Stand => {
                if self.motion == motion::STAND || (stopping && done) {
                    self.play(motion::STAND);
                    self.moving = Gait::Stand;
                    0.0
                } else {
                    let (_, _, stop) = self.moving.motions();
                    self.play(stop);
                    1.0 - (self.clock / self.length(stop)).min(1.0)
                }
            }
            g => {
                let (start, looped, _) = g.motions();
                if self.motion == looped || (self.motion == start && done) {
                    self.play(looped);
                    1.0
                } else if self.motion == start {
                    (self.clock / self.length(start)).min(1.0)
                } else if self.motion == walk || self.motion == run {
                    // From one gait's loop straight to the other's.
                    self.play(looped);
                    1.0
                } else {
                    self.play(start);
                    0.0
                }
            }
        };
        if self.gait != Gait::Stand {
            self.moving = self.gait;
            // Turn toward the lean, the short way round.
            let want = lean[0].atan2(lean[1]);
            let mut d = (want - self.turn).rem_euclid(std::f32::consts::TAU);
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            self.turn += d.clamp(-TURN_RATE * dt, TURN_RATE * dt);
        }
        if speed <= 0.0 || self.moving == Gait::Stand {
            return;
        }
        // Along where she faces (so a stop carries on her way).
        let full = if self.moving == Gait::Run { self.run_speed } else { self.walk_speed };
        let delta = [self.turn.sin() * full * speed * dt, self.turn.cos() * full * speed * dt];
        let to = match walls {
            // A wall whose top is within a step of her feet (a floor's edge, a kerb) she steps
            // over: the walls see her from a step up.
            Some(w) => w.step([self.at.x, self.at.z], delta, self.at.y + STEP_UP, self.height - STEP_UP, self.radius),
            None => [self.at.x + delta[0], self.at.z + delta[1]],
        };
        match ground {
            // Off the ground, or up a ledge higher than a step: she stays.
            Some(g) => {
                if let Some(y) = g.height(to[0], to[1], self.at.y, STEP_UP) {
                    self.at = Vec3::new(to[0], y, to[1]);
                }
            }
            None => self.at = Vec3::new(to[0], self.at.y, to[1]),
        }
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
        let wall = barrel_engine::walk::Wall { a: [-500.0, 200.0], b: [500.0, 200.0], low: -100.0, high: 300.0 };
        let walls = Walls::from_walls(vec![wall], 1000.0);
        let mut p = Player::new(Vec3::ZERO, 0.0, &LENGTHS);
        run(&mut p, [0.0, 1.0], 2.0, Some(&walls));
        assert!((p.at.z - (200.0 - RADIUS)).abs() < 1.0, "{:?}", p.at);
    }

    #[test]
    fn she_steps_over_a_low_wall() {
        let kerb = barrel_engine::walk::Wall { a: [-500.0, 200.0], b: [500.0, 200.0], low: -100.0, high: STEP_UP * 0.5 };
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
