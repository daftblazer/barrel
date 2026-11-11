//! An event's stage: where its characters and the camera are as the event runs, from the
//! player's timed steps ([`crate::event::Player::schedule`]), moved frame by frame by
//! [`Mover`]s.
//!
//! Positions measured from a character use the character's place only: the 27 points on a
//! character (feet, head, ...; `docs/events.md`) are not read yet, so `point` is taken as the
//! character's position. A position whose anchor is not on the stage (an unknown kind, or a
//! character not placed yet) is skipped and counted in [`Stage::unresolved`].

use std::collections::BTreeMap;

use crate::camera::{Anchor, Change, Mover, Part, Position};
use crate::event::{Step, Timed};

/// The game's frame time in events (`1/30` s, the constant at `0x14077ba78`).
pub const FRAME: f32 = 1.0 / 30.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub position: [f32; 3],
    /// Radians about the vertical axis, 0 towards +z.
    pub facing: f32,
    mover: Option<Mover>,
}

impl Body {
    pub fn moving(&self) -> bool {
        self.mover.as_ref().is_some_and(|m| !m.done())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraPoint {
    pub position: Option<[f32; 3]>,
    mover: Option<Mover>,
}

#[derive(Clone, Debug, Default)]
pub struct Stage {
    /// By the number the event's commands use (a `Chara.xml` row in story events, 10000 and
    /// up in skill events).
    pub characters: BTreeMap<u32, Body>,
    pub eye: CameraPoint,
    pub target: CameraPoint,
    pub time: f32,
    /// Positions that could not be placed (see the module notes).
    pub unresolved: usize,
}

impl Stage {
    fn resolve(&self, p: &Position) -> Option<[f32; 3]> {
        match p.anchor {
            Anchor::World => Some(p.resolve(None)),
            // Kind 3 names a thing whose character gives the point; its ids are not
            // character numbers, so only the other kinds are looked up here.
            Anchor::Thing { kind, id, .. } if kind != 2 && kind != 3 && kind != 4 => {
                let b = self.characters.get(&id)?;
                Some(p.resolve(Some((b.position, b.facing))))
            }
            _ => None,
        }
    }

    /// Applies a step at the stage's current time.
    pub fn apply(&mut self, step: &Step) {
        match step {
            Step::Place { character, position, .. } => {
                let b = self.characters.entry(*character).or_insert(Body { position: *position, facing: 0.0, mover: None });
                b.position = *position;
                b.mover = None;
            }
            Step::CharacterMove { character, extra, movement, .. } => {
                let Some(to) = self.resolve(movement.to()) else {
                    self.unresolved += 1;
                    return;
                };
                let Some(b) = self.characters.get_mut(character) else {
                    self.unresolved += 1;
                    return;
                };
                b.mover = Mover::new(b.position, to, movement, *extra);
            }
            Step::Camera { part, change, .. } => {
                let (to, movement) = match change {
                    Change::Set(p) => (self.resolve(p), None),
                    Change::Move(m) => (self.resolve(m.to()), Some(*m)),
                };
                let Some(to) = to else {
                    self.unresolved += 1;
                    return;
                };
                let point = match part {
                    Part::Eye => &mut self.eye,
                    Part::Target => &mut self.target,
                };
                match (movement, point.position) {
                    (Some(m), Some(from)) => point.mover = Mover::new(from, to, &m, 0),
                    _ => {
                        point.position = Some(to);
                        point.mover = None;
                    }
                }
            }
            _ => {}
        }
    }

    /// Runs the movers one frame.
    pub fn frame(&mut self) {
        for b in self.characters.values_mut() {
            if let Some(m) = b.mover.as_mut().filter(|m| !m.done()) {
                m.step(FRAME);
                b.position = m.position();
                if let Some(f) = m.facing() {
                    b.facing = f;
                }
            }
        }
        for p in [&mut self.eye, &mut self.target] {
            if let Some(m) = p.mover.as_mut().filter(|m| !m.done()) {
                m.step(FRAME);
                p.position = Some(m.position());
            }
        }
        self.time += FRAME;
    }

    /// Runs whole frames until `time`.
    pub fn run_until(&mut self, time: f32) {
        while self.time + FRAME * 0.5 < time {
            self.frame();
        }
    }

    /// Plays a timed event: each step is applied at its time, the movers run in between, and
    /// `each` sees the stage after every step.
    pub fn play<'a>(steps: &[Timed<'a>], mut each: impl FnMut(&Timed<'a>, &Stage)) -> Stage {
        let mut stage = Stage::default();
        for t in steps {
            stage.run_until(t.time);
            stage.apply(&t.step);
            each(t, &stage);
        }
        stage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Move;

    #[test]
    fn a_character_walks_and_the_camera_follows() {
        let world = |x: f32, z: f32| Position { anchor: Anchor::World, offset: [x, 0.0, z] };
        let on = |id: u32, z: f32| Position { anchor: Anchor::Thing { kind: 5, id, point: 0 }, offset: [0.0, 150.0, z] };
        let steps = vec![
            Timed { time: 0.0, action: 0, step: Step::Place { action: 0, character: 7, position: [0.0, 0.0, 0.0] } },
            Timed {
                time: 0.0,
                action: 0,
                step: Step::CharacterMove {
                    action: 0,
                    character: 7,
                    extra: 1,
                    movement: Move::Speed { to: world(300.0, 0.0), speed: 150.0, accel: [0.0; 3] },
                },
            },
            Timed { time: 3.0, action: 0, step: Step::Camera { action: 0, part: Part::Target, change: Change::Set(on(7, 0.0)) } },
            Timed { time: 3.0, action: 0, step: Step::Camera { action: 0, part: Part::Eye, change: Change::Set(on(9, 0.0)) } },
        ];
        let stage = Stage::play(&steps, |_, _| {});
        let b = &stage.characters[&7];
        assert_eq!(b.position, [300.0, 0.0, 0.0]);
        assert!((b.facing - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert_eq!(stage.target.position, Some([300.0, 150.0, 0.0]));
        // Character 9 was never placed.
        assert_eq!((stage.eye.position, stage.unresolved), (None, 1));
    }
}
