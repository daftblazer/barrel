//! Ryza as the player walks her: where she stands, which way she faces, and which of her
//! field motions plays (`motion`). The input is a lean on the ground (game space x, z, length
//! up to 1); a light lean walks, a full one runs. Stand-ins, not read from the game: the
//! speeds, the lean that starts a run, how fast she turns, her size against the walls, and no
//! start or stop motions yet (`docs/ryza/open.md`, "Ryza in the field").

use barrel_engine::walk::{Ground, Walls};
use glam::Vec3;

use crate::motion;

/// Walking and running speeds, units (centimetres) a second.
pub const WALK_SPEED: f32 = 150.0;
pub const RUN_SPEED: f32 = 480.0;
/// The lean at and above which she runs.
pub const RUN_LEAN: f32 = 0.6;
/// How fast she turns toward where she goes, radians a second.
pub const TURN_RATE: f32 = 12.0;
/// Her size against the walls, and the highest step she climbs.
pub const RADIUS: f32 = 30.0;
pub const HEIGHT: f32 = 170.0;
pub const STEP_UP: f32 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Run,
}

pub struct Player {
    /// Her feet, game space.
    pub at: Vec3,
    /// Which way she faces, radians about y (0 faces +z, -pi/2 faces -x).
    pub turn: f32,
    pub gait: Gait,
    /// Seconds into the gait's motion.
    pub clock: f32,
}

impl Player {
    pub fn new(at: Vec3, turn: f32) -> Player {
        Player { at, turn, gait: Gait::Stand, clock: 0.0 }
    }

    /// One step of `dt` seconds with the lean `lean` (game x, z).
    pub fn step(&mut self, lean: [f32; 2], dt: f32, ground: Option<&Ground>, walls: Option<&Walls>) {
        let amount = (lean[0] * lean[0] + lean[1] * lean[1]).sqrt().min(1.0);
        let gait = if amount < 0.05 { Gait::Stand } else if amount < RUN_LEAN { Gait::Walk } else { Gait::Run };
        if gait != self.gait {
            self.gait = gait;
            self.clock = 0.0;
        }
        self.clock += dt;
        if gait == Gait::Stand {
            return;
        }
        // Turn toward the lean, the short way round.
        let want = lean[0].atan2(lean[1]);
        let mut d = (want - self.turn).rem_euclid(std::f32::consts::TAU);
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        self.turn += d.clamp(-TURN_RATE * dt, TURN_RATE * dt);
        let speed = if gait == Gait::Run { RUN_SPEED } else { WALK_SPEED };
        let delta = [lean[0] / amount * speed * dt, lean[1] / amount * speed * dt];
        let to = match walls {
            Some(w) => w.step([self.at.x, self.at.z], delta, self.at.y, HEIGHT, RADIUS),
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

    /// The motion playing now: its number (`pc00a_<N>`) and the time into it.
    pub fn motion(&self) -> (u32, f32) {
        let m = match self.gait {
            Gait::Stand => motion::STAND,
            Gait::Walk => motion::WALK,
            Gait::Run => motion::RUN,
        };
        (m, self.clock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_lean_runs_and_a_light_one_walks() {
        let mut p = Player::new(Vec3::ZERO, 0.0);
        p.step([0.0, 1.0], 1.0, None, None);
        assert_eq!((p.gait, p.at.z), (Gait::Run, RUN_SPEED));
        p.step([0.0, 0.3], 1.0, None, None);
        assert_eq!(p.gait, Gait::Walk);
        assert_eq!(p.motion().0, motion::WALK);
        p.step([0.0, 0.0], 1.0, None, None);
        assert_eq!(p.motion().0, motion::STAND);
    }

    #[test]
    fn she_turns_toward_the_lean() {
        let mut p = Player::new(Vec3::ZERO, 0.0);
        for _ in 0..60 {
            p.step([-1.0, 0.0], 1.0 / 60.0, None, None);
        }
        assert!((p.turn + std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{}", p.turn);
    }
}
