//! A field creature driven by its `Script/ai_*.nut` (`squirrel`), the game's side of the calls
//! it makes. What is read from the executable is marked with the native function; the spots near
//! the player (`getPositionToPlayerBehind` and the like) are stand-ins until their functions are
//! read (`tools/sq_binds.py` lists them).

use crate::squirrel::{Error, Host, Value};

/// `SQ` is host object 0, the creature 1, the player 2.
const SQ: u32 = 0;
const SELF: u32 = 1;
const PLAYER: u32 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Creature {
    pub position: [f64; 3],
    pub facing: f64,
    /// The script's state (`setState`, `0x140131f60`).
    pub state: i64,
    /// Counts down every frame; the scripts wait for it to reach 0 (`setTimer`).
    pub timer: f64,
    /// The motion it plays and the seconds left of it (`setMotionTag`, `0x140231fa0`;
    /// `isEndMotin`, `0x140232360`, is true once it has ended).
    pub motion: Option<i64>,
    pub old_motion: Option<i64>,
    pub motion_left: f64,
    /// A straight move under way (`movePosition`, `0x140232000`): where to and how fast.
    pub moving: Option<([f64; 3], f64)>,
    /// Its base speed (`getMoveSpeed`) and idle time (`getWaitTime`, a virtual of its class).
    pub move_speed: f64,
    pub wait_time: f64,
    pub emotion: Option<i64>,
    pub custom_pos: [f64; 3],
    pub custom_rot: [f64; 3],
    pub custom_param: i64,
    pub judge_range: bool,
    /// The player's place, which the stage keeps current.
    pub player: [f64; 3],
    /// The length a motion is taken to last when the host has none for it.
    pub motion_seconds: f64,
    pub rng: crate::rng::Rng,
    /// The game's named constants (`MOTION_*`, `EMOTION_ICON_*`, ...).
    pub constants: std::collections::HashMap<String, i64>,
    /// The macro calls it made (`SQ.execProgramMacro` and the like), for whoever plays them.
    pub macros: Vec<String>,
}

impl Creature {
    pub fn new(position: [f64; 3], rng: crate::rng::Rng) -> Creature {
        Creature {
            position,
            facing: 0.0,
            state: 0,
            timer: 0.0,
            motion: None,
            old_motion: None,
            motion_left: 0.0,
            moving: None,
            move_speed: 150.0,
            wait_time: 2.0,
            emotion: None,
            custom_pos: position,
            custom_rot: [0.0; 3],
            custom_param: 0,
            judge_range: false,
            player: [0.0; 3],
            motion_seconds: 1.0,
            rng,
            macros: Vec::new(),
            constants: Default::default(),
        }
    }

    /// The frame's passing: the timer and the motion run down, a move goes on.
    pub fn frame(&mut self, dt: f64) {
        self.timer -= dt;
        self.motion_left -= dt;
        if let Some((to, speed)) = self.moving {
            let d = [to[0] - self.position[0], 0.0, to[2] - self.position[2]];
            let dist = (d[0] * d[0] + d[2] * d[2]).sqrt();
            let step = speed * dt;
            if dist <= step || dist == 0.0 {
                self.position[0] = to[0];
                self.position[2] = to[2];
                self.moving = None;
            } else {
                self.position[0] += d[0] / dist * step;
                self.position[2] += d[2] / dist * step;
                self.facing = d[0].atan2(d[2]);
            }
        }
    }

    fn distance_to_player(&self) -> f64 {
        let (dx, dz) = (self.player[0] - self.position[0], self.player[2] - self.position[2]);
        (dx * dx + dz * dz).sqrt()
    }
}

fn vec(v: &Value) -> [f64; 3] {
    match v {
        Value::Vec3(a) => *a,
        _ => [0.0; 3],
    }
}

fn num(args: &[Value], k: usize) -> f64 {
    args.get(k).and_then(|a| a.number()).unwrap_or(0.0)
}

impl Host for Creature {
    fn global(&mut self, name: &str) -> Option<Value> {
        if name == "SQ" {
            return Some(Value::Host(SQ));
        }
        self.constants.get(name).map(|&v| Value::Int(v))
    }

    fn call(&mut self, object: u32, name: &str, args: &[Value]) -> Result<Value, Error> {
        Ok(match (object, name) {
            (SQ, "getPlayer") => Value::Host(PLAYER),
            (SQ, "getActiveChara") => Value::Host(SELF),
            (SQ, "randRange") => Value::Int(self.rng.range(num(args, 0) as i32, num(args, 1) as i32) as i64),
            (SQ, "randRangeF") => {
                let (a, b) = (num(args, 0), num(args, 1));
                Value::Float(a + (b - a) * (self.rng.range(0, 10000) as f64 / 10000.0))
            }
            (SQ, "getVector3") => Value::Vec3([num(args, 0), num(args, 1), num(args, 2)]),
            (SQ, "getXByVector3") => Value::Float(vec(&args[0])[0]),
            (SQ, "getYByVector3") => Value::Float(vec(&args[0])[1]),
            (SQ, "getZByVector3") => Value::Float(vec(&args[0])[2]),
            (SQ, n) if n.starts_with("execProgramMacro") => {
                if let Some(Value::Str(m)) = args.first() {
                    self.macros.push(m.to_string());
                }
                Value::Null
            }
            (PLAYER, "getPosition") => Value::Vec3(self.player),
            (SELF, "getState") => Value::Int(self.state),
            (SELF, "setState") => {
                self.state = num(args, 0) as i64;
                Value::Null
            }
            (SELF, "setTimer") => {
                self.timer = num(args, 0);
                Value::Null
            }
            (SELF, "getTimer") => Value::Float(self.timer),
            (SELF, "getWaitTime") => Value::Float(self.wait_time),
            (SELF, "getMoveSpeed") => Value::Float(self.move_speed),
            (SELF, "setMotionTag") => {
                self.old_motion = self.motion;
                self.motion = Some(num(args, 0) as i64);
                self.motion_left = self.motion_seconds;
                Value::Null
            }
            (SELF, "getOldMotionTag") => Value::Int(self.old_motion.unwrap_or(-1)),
            (SELF, "isEndMotin") => Value::Bool(self.motion_left <= 0.0),
            (SELF, "movePosition") => {
                let speed = if args.len() > 1 { num(args, 1) } else { self.move_speed };
                self.moving = Some((vec(&args[0]), speed));
                Value::Null
            }
            (SELF, "stopMovePosition") => {
                self.moving = None;
                Value::Null
            }
            (SELF, "isMovePosition") => Value::Bool(self.moving.is_some()),
            (SELF, "getPosition") => Value::Vec3(self.position),
            (SELF, "setPosition") => {
                self.position = vec(&args[0]);
                Value::Null
            }
            (SELF, "setEmotionIcon") => {
                self.emotion = Some(num(args, 0) as i64);
                Value::Null
            }
            (SELF, "unsetEmotionIcon") => {
                self.emotion = None;
                Value::Null
            }
            (SELF, "setCustomPos") => {
                self.custom_pos = vec(&args[0]);
                Value::Null
            }
            (SELF, "getCustomPos") => Value::Vec3(self.custom_pos),
            (SELF, "setCustomRot") => {
                self.custom_rot = vec(&args[0]);
                Value::Null
            }
            (SELF, "setCustomParam") => {
                self.custom_param = num(args, 0) as i64;
                Value::Null
            }
            (SELF, "getCustomParam") => Value::Int(self.custom_param),
            (SELF, "setIsJudgeRange") => {
                self.judge_range = args.first().is_some_and(|a| a.truthy());
                Value::Null
            }
            (SELF, "getIsJudgeRange") => Value::Bool(self.judge_range),
            // Stand-ins (not read yet): range checks against a radius given as the argument, the
            // spots near the player 300 units off.
            (SELF, "isRangeIn" | "isMaxRangeIn") => Value::Bool(self.distance_to_player() <= num(args, 0).max(1.0)),
            (SELF, "getPositionToPlayerBehind" | "getPositionToPlayerEscape" | "getRandomRangePosition") => {
                let (dx, dz) = (self.position[0] - self.player[0], self.position[2] - self.player[2]);
                let d = (dx * dx + dz * dz).sqrt().max(1.0);
                Value::Vec3([self.position[0] + dx / d * 300.0, self.position[1], self.position[2] + dz / d * 300.0])
            }
            (_, n) if n.starts_with("is") => Value::Bool(false),
            (_, n) if n.starts_with("get") => Value::Float(0.0),
            _ => Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creature_waits_then_walks_to_its_spot() {
        let src = crate::squirrel::parse(
            r#"{
            local pcOwn = SQ.getActiveChara();
            switch (pcOwn.getState()) {
                case 0: pcOwn.setTimer(pcOwn.getWaitTime()); pcOwn.setState(1); break;
                case 1: if (pcOwn.getTimer() <= 0.0) { pcOwn.movePosition(SQ.getVector3(300, 0, 0), pcOwn.getMoveSpeed()); pcOwn.setState(2); } break;
                case 2: if (!pcOwn.isMovePosition()) { pcOwn.setState(3); } break;
            }
        }"#,
        )
        .unwrap();
        let mut c = Creature::new([0.0; 3], crate::rng::Rng::from_state([1, 2, 3, 4]));
        let mut vm = crate::squirrel::Interp::new();
        let mut frames = 0;
        while c.state != 3 && frames < 600 {
            vm.run(&src, &mut c).unwrap();
            c.frame(1.0 / 60.0);
            frames += 1;
        }
        // Two seconds' wait, then 300 units at 150 a second.
        assert_eq!(c.state, 3);
        assert!((240..=250).contains(&frames), "{frames}");
        assert_eq!(c.position, [300.0, 0.0, 0.0]);
    }
}
