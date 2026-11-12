//! The field-map camera that most events drive (`t3/0x10` to `t20/0x10`, words `0x74100003` to
//! `0x74100014`, handled by `CameraCFieldMap`, `0x1401e7a10` in the English executable): an
//! orbit around a target point, set by yaw, pitch, distance and the field of view.
//!
//! Every operand is a whole number divided by 100 (`docs/events.md`, "The event camera").
//! Each value is held as `{start, goal, now, seconds}` (yaw at +0x20..+0x2c, the field of view
//! +0x50.., pitch +0x80.., distance +0x98..; setters `0x1401eca20`, `0x1401ec7a0`,
//! `0x1401ec940`, `0x1401ec890`): a command moves it from where it is to the goal over its
//! seconds, at once when they are 0. The odd words set the goal; the even ones (`t4`, `t8`,
//! ...) add to the value now. `t20` sets everything at once.
//!
//! Which value is the field of view and which the pitch was read from the events, not the code:
//! `t20`'s third operand is 22.5° in 140 of the 168 story events' first `t20` (else 30°, 45°,
//! 40°), the fields' own camera tables give `fov="22.5"`, and taking the fourth as the pitch
//! frames the nearest character a median 6.7° off centre (9.7° the other way round).
//!
//! The values move linearly: each frame the update (`0x1401e6ee0`) moves the value now by
//! `(goal - now) × dt / time left` and counts the time left down, landing on the goal when it
//! runs out. The angle `t5` and `t20`'s second operand set (+0x38..+0x44) is ignored.

/// A value on its way to a goal.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    /// `t20`: yaw, field of view, pitch (degrees), distance, target and the offset added to
    /// it.
    All { yaw: f32, fov: f32, pitch: f32, distance: f32, target: [f32; 3], offset: [f32; 3] },
    Yaw { value: f32, relative: bool, seconds: f32 },
    Fov { value: f32, relative: bool, seconds: f32 },
    Pitch { value: f32, relative: bool, seconds: f32 },
    Distance { value: f32, relative: bool, seconds: f32 },
    Target { value: [f32; 3], relative: bool, seconds: f32 },
    /// `t19`: the field's camera data on or off (events turn it off at their start and back on
    /// at their end, handing the camera back to play, so a scene never draws with it).
    DataEffect { on: bool },
}

pub const FIRST: u32 = 0x7410_0003;
/// `t19/0x10` (`ECT_FIELDMAP_CAMERA_DATA_EFFECT_SET`, `{on}`): whether the field's own camera
/// data steers the camera (`docs/events.md`, "The event camera").
pub const DATA_EFFECT: u32 = 0x7410_0013;
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
                fov: v(2)?,
                pitch: v(3)?,
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
                Some(Change::Fov { value, relative, seconds })
            }
            0x7410_0009 | 0x7410_000a => {
                let (value, seconds) = one(v(0)?, 1);
                Some(Change::Pitch { value, relative, seconds })
            }
            0x7410_000b | 0x7410_000c => {
                let (value, seconds) = one(unit(0)?, 1);
                Some(Change::Distance { value, relative, seconds })
            }
            DATA_EFFECT => Some(Change::DataEffect { on: operands.first().is_some_and(|&v| v != 0) }),
            0x7410_000d | 0x7410_000e => Some(Change::Target { value: [unit(0)?, unit(1)?, unit(2)?], relative, seconds: v(3).unwrap_or(0.0) }),
            _ => None,
        }
    }
}

/// The camera's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldCamera {
    pub yaw: Tween,
    pub fov: Tween,
    pub pitch: Tween,
    pub distance: Tween,
    pub target: [Tween; 3],
    pub offset: [f32; 3],
    /// `t19`'s flag (+0x138), kept but not drawn: the camera data it switches on is not read.
    pub data_effect: bool,
}

impl FieldCamera {
    /// A camera as `t20` sets it.
    pub fn new(change: &Change) -> Option<FieldCamera> {
        let Change::All { yaw, fov, pitch, distance, target, offset } = *change else { return None };
        Some(FieldCamera { yaw: Tween::at(yaw), fov: Tween::at(fov), pitch: Tween::at(pitch), distance: Tween::at(distance), target: target.map(Tween::at), offset, data_effect: false })
    }

    pub fn apply(&mut self, change: &Change) {
        let go = |t: &mut Tween, value: f32, relative: bool, seconds: f32| {
            let to = if relative { t.now() + value } else { value };
            t.retarget(to, seconds);
        };
        match *change {
            Change::All { .. } => *self = FieldCamera::new(change).unwrap(),
            Change::Yaw { value, relative, seconds } => go(&mut self.yaw, value, relative, seconds),
            Change::Fov { value, relative, seconds } => go(&mut self.fov, value, relative, seconds),
            Change::Pitch { value, relative, seconds } => go(&mut self.pitch, value, relative, seconds),
            Change::Distance { value, relative, seconds } => go(&mut self.distance, value, relative, seconds),
            Change::DataEffect { on } => self.data_effect = on,
            Change::Target { value, relative, seconds } => {
                for (t, v) in self.target.iter_mut().zip(value) {
                    go(t, v, relative, seconds);
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        for t in [&mut self.yaw, &mut self.fov, &mut self.pitch, &mut self.distance].into_iter().chain(self.target.iter_mut()) {
            t.step(dt);
        }
    }

    /// The point looked at (target + offset) and the eye, in game space: the eye sits at
    /// (sin yaw · cos pitch, sin pitch, cos yaw · cos pitch) × distance from it, as
    /// `0x1401e68f0` builds it (`(0, 0, distance)` turned by the angles `{x: -pitch, y: yaw}`,
    /// `0x14004fbd0`, row vectors).
    pub fn eye_target(&self) -> ([f32; 3], [f32; 3]) {
        let at: [f32; 3] = std::array::from_fn(|i| self.target[i].now() + self.offset[i]);
        let (yaw, pitch) = (self.yaw.now().to_radians(), self.pitch.now().to_radians());
        let d = self.distance.now();
        let back = [yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()];
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
        let all = Change::read(ALL, &ops(&[-17696, 0, 1750, 2972, 1000, 825, -245, 1660, 0, 150, 0])).unwrap();
        let mut c = FieldCamera::new(&all).unwrap();
        let (eye, at) = c.eye_target();
        assert_eq!(at, [825.0, -95.0, 1660.0]);
        assert_eq!(c.fov.now(), 17.5);
        assert!((eye[1] - (-95.0 + 1000.0 * 29.72f32.to_radians().sin())).abs() < 1e-2);
        // Yaw -176.96°: the eye is behind the target along -z (cos < 0), a little to the -x side.
        assert!(eye[2] < 1660.0 - 800.0 && eye[0] < 825.0);
        c.apply(&Change::read(0x7410_0003, &ops(&[-16397, 500])).unwrap());
        c.apply(&Change::read(0x7410_000b, &ops(&[1000, 500])).unwrap());
        c.apply(&Change::read(0x7410_000d, &ops(&[1077, -245, 1815, 500])).unwrap());
        c.step(2.5);
        assert!((c.yaw.now() - (-176.96 - 163.97) / 2.0).abs() < 1e-3);
        assert_eq!(c.target[0].now(), (825.0 + 1077.0) / 2.0);
        c.step(10.0);
        assert_eq!(c.target.map(|t| t.now()), [1077.0, -245.0, 1815.0]);
        // A relative change adds to the value now; no time means at once.
        c.apply(&Change::read(0x7410_0004, &ops(&[1000])).unwrap());
        assert!((c.yaw.now() - (-163.97 + 10.0)).abs() < 1e-3);
        assert_eq!(Change::read(0x7410_0005, &[0, 0]), None);
    }
}
