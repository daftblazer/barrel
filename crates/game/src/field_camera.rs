//! The field-map camera that most events drive (`t3/0x10` to `t20/0x10`, words `0x74100003` to
//! `0x74100014`, handled by `CameraCFieldMap`, `0x1401e7a10` in the English executable): an
//! orbit around a target point, set by yaw, pitch, distance and a fourth angle.
//!
//! Every operand is a whole number divided by 100 (`docs/events.md`, "The event camera").
//! Each value is held as `{start, goal, now, seconds}` (yaw at +0x20..+0x2c, pitch +0x50..,
//! the fourth angle +0x80.., distance +0x98..; setters `0x1401eca20`, `0x1401ec7a0`,
//! `0x1401ec940`, `0x1401ec890`): a command moves it from where it is to the goal over its
//! seconds, at once when they are 0. The odd words set the goal; the even ones (`t4`, `t8`,
//! ...) add to the value now. `t20` sets everything at once.
//!
//! Ours until the game's update is read: values move linearly; the fourth angle (`t9`, and
//! `t20`'s fourth operand) is kept but not used; the roll-like angle (`t5`, `t20`'s second) is
//! ignored.

/// A value on its way to a goal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub seconds: f32,
    pub elapsed: f32,
}

impl Tween {
    pub fn at(v: f32) -> Tween {
        Tween { from: v, to: v, seconds: 0.0, elapsed: 0.0 }
    }

    pub fn now(&self) -> f32 {
        if self.seconds <= 0.0 || self.elapsed >= self.seconds {
            self.to
        } else {
            self.from + (self.to - self.from) * (self.elapsed / self.seconds)
        }
    }

    /// Heads for `to` over `seconds` from the value now.
    pub fn retarget(&mut self, to: f32, seconds: f32) {
        *self = Tween { from: self.now(), to, seconds: seconds.max(0.0), elapsed: 0.0 };
    }

    pub fn step(&mut self, dt: f32) {
        self.elapsed += dt;
    }
}

/// What one command does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// `t20`: yaw, pitch (degrees), the fourth angle, distance, target and the offset added to
    /// it.
    All { yaw: f32, pitch: f32, angle: f32, distance: f32, target: [f32; 3], offset: [f32; 3] },
    Yaw { value: f32, relative: bool, seconds: f32 },
    Pitch { value: f32, relative: bool, seconds: f32 },
    Angle { value: f32, relative: bool, seconds: f32 },
    Distance { value: f32, relative: bool, seconds: f32 },
    Target { value: [f32; 3], relative: bool, seconds: f32 },
}

pub const FIRST: u32 = 0x7410_0003;
pub const ALL: u32 = 0x7410_0014;

impl Change {
    /// The change a field-camera command word makes, if it is one this model follows.
    pub fn read(word: u32, operands: &[u32]) -> Option<Change> {
        let v = |k: usize| operands.get(k).map(|&o| o as i32 as f32 / 100.0);
        // Positions are in field units: divided by 100 and multiplied back (as the game does).
        let unit = |k: usize| operands.get(k).map(|&o| o as i32 as f32);
        let relative = word & 1 == 0;
        let one = |value: f32, k: usize| (value, v(k).unwrap_or(0.0));
        match word {
            ALL => Some(Change::All {
                yaw: v(0)?,
                pitch: v(2)?,
                angle: v(3)?,
                distance: unit(4)?,
                target: [unit(5)?, unit(6)?, unit(7)?],
                offset: [unit(8)?, unit(9)?, unit(10)?],
            }),
            0x7410_0003 | 0x7410_0004 => {
                let (value, seconds) = one(v(0)?, 1);
                Some(Change::Yaw { value, relative, seconds })
            }
            0x7410_0007 | 0x7410_0008 => {
                let (value, seconds) = one(v(0)?, 1);
                Some(Change::Pitch { value, relative, seconds })
            }
            0x7410_0009 | 0x7410_000a => {
                let (value, seconds) = one(v(0)?, 1);
                Some(Change::Angle { value, relative, seconds })
            }
            0x7410_000b | 0x7410_000c => {
                let (value, seconds) = one(unit(0)?, 1);
                Some(Change::Distance { value, relative, seconds })
            }
            0x7410_000d | 0x7410_000e => Some(Change::Target { value: [unit(0)?, unit(1)?, unit(2)?], relative, seconds: v(3).unwrap_or(0.0) }),
            _ => None,
        }
    }
}

/// The camera's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldCamera {
    pub yaw: Tween,
    pub pitch: Tween,
    pub angle: Tween,
    pub distance: Tween,
    pub target: [Tween; 3],
    pub offset: [f32; 3],
}

impl FieldCamera {
    /// A camera as `t20` sets it.
    pub fn new(change: &Change) -> Option<FieldCamera> {
        let Change::All { yaw, pitch, angle, distance, target, offset } = *change else { return None };
        Some(FieldCamera { yaw: Tween::at(yaw), pitch: Tween::at(pitch), angle: Tween::at(angle), distance: Tween::at(distance), target: target.map(Tween::at), offset })
    }

    pub fn apply(&mut self, change: &Change) {
        let go = |t: &mut Tween, value: f32, relative: bool, seconds: f32| {
            let to = if relative { t.now() + value } else { value };
            t.retarget(to, seconds);
        };
        match *change {
            Change::All { .. } => *self = FieldCamera::new(change).unwrap(),
            Change::Yaw { value, relative, seconds } => go(&mut self.yaw, value, relative, seconds),
            Change::Pitch { value, relative, seconds } => go(&mut self.pitch, value, relative, seconds),
            Change::Angle { value, relative, seconds } => go(&mut self.angle, value, relative, seconds),
            Change::Distance { value, relative, seconds } => go(&mut self.distance, value, relative, seconds),
            Change::Target { value, relative, seconds } => {
                for (t, v) in self.target.iter_mut().zip(value) {
                    go(t, v, relative, seconds);
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        for t in [&mut self.yaw, &mut self.pitch, &mut self.angle, &mut self.distance].into_iter().chain(self.target.iter_mut()) {
            t.step(dt);
        }
    }

    /// The point looked at (target + offset) and the eye, in game space: the eye sits at
    /// (-sin yaw · cos pitch, sin pitch, -cos yaw · cos pitch) × distance from it (the yaw's
    /// sign is settled in `docs/events.md`).
    pub fn eye_target(&self) -> ([f32; 3], [f32; 3]) {
        let at: [f32; 3] = std::array::from_fn(|i| self.target[i].now() + self.offset[i]);
        let (yaw, pitch) = (self.yaw.now().to_radians(), self.pitch.now().to_radians());
        let d = self.distance.now();
        let back = [-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()];
        (std::array::from_fn(|i| at[i] + back[i] * d), at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_moves() {
        // The opening of EVENT_MM01_OP1_111: t20, then t3 / t7 / t11 / t13 over 5 s.
        let ops = |v: &[i32]| v.iter().map(|&x| x as u32).collect::<Vec<_>>();
        let all = Change::read(ALL, &ops(&[13789, 0, 2250, 0, 1000, 607, -21, 1808, 0, 150, 0])).unwrap();
        let mut c = FieldCamera::new(&all).unwrap();
        let (eye, at) = c.eye_target();
        assert_eq!(at, [607.0, 129.0, 1808.0]);
        assert!((eye[1] - (129.0 + 1000.0 * 22.5f32.to_radians().sin())).abs() < 1e-2);
        c.apply(&Change::read(0x7410_0003, &ops(&[-16397, 500])).unwrap());
        c.apply(&Change::read(0x7410_000b, &ops(&[1000, 500])).unwrap());
        c.apply(&Change::read(0x7410_000d, &ops(&[1077, -245, 1815, 500])).unwrap());
        c.step(2.5);
        assert!((c.yaw.now() - (137.89 - 163.97) / 2.0).abs() < 1e-3);
        assert_eq!(c.target[0].now(), (607.0 + 1077.0) / 2.0);
        c.step(10.0);
        assert_eq!(c.target.map(|t| t.now()), [1077.0, -245.0, 1815.0]);
        // A relative change adds to the value now; no time means at once.
        c.apply(&Change::read(0x7410_0004, &ops(&[1000])).unwrap());
        assert!((c.yaw.now() - (-163.97 + 10.0)).abs() < 1e-3);
        assert_eq!(Change::read(0x7410_0005, &[0, 0]), None);
    }
}
