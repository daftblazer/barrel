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
    /// The player's place and facing (radians), which the stage keeps current.
    pub player: [f64; 3],
    pub player_facing: f64,
    /// The length a motion is taken to last when the host has none for it.
    pub motion_seconds: f64,
    pub rng: crate::rng::Rng,
    /// Its field enemy record's numbers (`battle_data::FieldEnemy`): walking and running speed
    /// (m/s), longest wait (s), sight and reach (m), field of view (degrees), and where it stands.
    pub walk: f64,
    pub run: f64,
    pub sight: f64,
    pub reach: f64,
    pub view: f64,
    pub home: [f64; 3],
    /// Whether the player is in its sight (its brain's +0xd4, `setIsRange`/`getIsRange`, set by
    /// `utility.nut`'s `isRangeInOut` when the sight check changes): it runs rather than walks,
    /// and its brain plays its record's first script (`ai_*_in`) rather than its second.
    pub aware: bool,
    /// Take `wait_time` as it is rather than drawing (for tests).
    pub fixed_wait: bool,
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
            // 0x140235b74 starts a brain with its sight flag clear and range judging on.
            judge_range: true,
            player: [0.0; 3],
            player_facing: 0.0,
            motion_seconds: 1.0,
            rng,
            macros: Vec::new(),
            constants: Default::default(),
            walk: 1.5,
            run: 1.5,
            sight: 8.0,
            reach: 20.0,
            view: 180.0,
            home: position,
            aware: false,
            fixed_wait: false,
        }
    }

    /// A creature for a field enemy record, standing where the record puts it (metres in the
    /// record, field units here).
    pub fn from_field_enemy(e: &barrel_formats::battle_data::FieldEnemy, rng: crate::rng::Rng) -> Creature {
        let at = e.position.map(|v| v as f64 * 100.0);
        Creature {
            walk: e.speeds.0 as f64,
            run: e.speeds.1 as f64,
            wait_time: e.wait as f64,
            sight: e.sight as f64,
            reach: e.reach as f64,
            view: e.view as f64,
            ..Creature::new(at, rng)
        }
    }

    /// Whether `p` is within its sight and inside half its field of view either side of its
    /// facing (`isRangeIn`, `0x140131b60` → `0x140232c00`).
    pub fn sees(&self, p: [f64; 3]) -> bool {
        let (dx, dz) = (p[0] - self.position[0], p[2] - self.position[2]);
        let d = (dx * dx + dz * dz).sqrt();
        if d > self.sight * 100.0 {
            return false;
        }
        if d == 0.0 || self.view >= 360.0 {
            return true;
        }
        let toward = dx.atan2(dz);
        let mut off = (toward - self.facing).abs() % std::f64::consts::TAU;
        if off > std::f64::consts::PI {
            off = std::f64::consts::TAU - off;
        }
        off.to_degrees() <= self.view * 0.5
    }

    /// A spot to wander to (`getRandomRangePosition`, `0x1401317f0` → `0x140232f30`): where it
    /// stands, moved on x and z each by a draw of -32768 to 32767 over 32767 times its sight,
    /// kept when both are within its reach of where it stands; up to 64 draws, the last kept.
    pub fn random_spot(&mut self) -> [f64; 3] {
        let (sight, reach) = (self.sight * 100.0, self.reach * 100.0);
        let mut spot = self.home;
        for _ in 0..64 {
            let dx = self.rng.range(-32768, 32767) as f64 / 32767.0 * sight;
            let dz = self.rng.range(-32768, 32767) as f64 / 32767.0 * sight;
            spot = [self.home[0] + dx, self.home[1], self.home[2] + dz];
            if (spot[0] - self.home[0]).abs() <= reach && (spot[2] - self.home[2]).abs() <= reach {
                break;
            }
        }
        spot
    }

    /// A spot just past the player, on the far side from the creature (`getPositionToPlayerBehind`,
    /// `0x140131690` → `0x1402330d0`): the player's place plus the direction from the creature to
    /// the player times a draw of 1 to 3 (taken as metres; the function mixes the creature's
    /// metres with the player's field units, not settled).
    pub fn behind_player(&mut self) -> [f64; 3] {
        let (dx, dz) = (self.player[0] - self.position[0], self.player[2] - self.position[2]);
        let d = (dx * dx + dz * dz).sqrt().max(1e-6);
        let k = 100.0 + 200.0 * (self.rng.range(0, 10000) as f64 / 10000.0);
        [self.player[0] + dx / d * k, self.player[1], self.player[2] + dz / d * k]
    }

    /// A spot to flee to (`getPositionToPlayerEscape (angle)`, `0x140131720` → `0x140233240`):
    /// from where it is, in a direction within `angle` degrees centred on the way the player
    /// faces, a draw of distance up to its sight, kept when within its reach of where it is on
    /// both axes; up to 64 draws. (The lower bound of the distance draw is not settled; 0 here.)
    pub fn escape_spot(&mut self, angle: f64) -> [f64; 3] {
        let (sight, reach) = (self.sight * 100.0, self.reach * 100.0);
        let mut spot = self.position;
        for _ in 0..64 {
            let a = self.player_facing + (self.rng.range(0, 10000) as f64 / 10000.0 - 0.5) * angle.to_radians();
            let d = sight * (self.rng.range(0, 10000) as f64 / 10000.0);
            spot = [self.position[0] + a.sin() * d, self.position[1], self.position[2] + a.cos() * d];
            if (spot[0] - self.position[0]).abs() <= reach && (spot[2] - self.position[2]).abs() <= reach {
                break;
            }
        }
        spot
    }

    /// Whether `p` is within its reach of where it stands (`isMaxRangeIn`, `0x140131a60` →
    /// `0x140232e30`).
    pub fn within_reach(&self, p: [f64; 3]) -> bool {
        let (dx, dz) = (p[0] - self.home[0], p[2] - self.home[2]);
        (dx * dx + dz * dz).sqrt() <= self.reach * 100.0
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
            // 0x140131900: a random time from half the record's wait to all of it.
            (SELF, "getWaitTime") if self.fixed_wait => Value::Float(self.wait_time),
            (SELF, "getWaitTime") => {
                let w = self.wait_time;
                Value::Float(w * 0.5 + w * 0.5 * (self.rng.range(0, 10000) as f64 / 10000.0))
            }
            // 0x1401314d0: running once aware, else walking; metres a second to field units.
            (SELF, "getMoveSpeed") => Value::Float(if self.aware { self.run } else { self.walk } * 100.0),
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
            (SELF, "getIsRange") => Value::Bool(self.aware),
            (SELF, "setIsRange") => {
                self.aware = args.first().is_some_and(|a| a.truthy());
                Value::Null
            }
            (SELF, "setIsJudgeRange") => {
                self.judge_range = args.first().is_some_and(|a| a.truthy());
                Value::Null
            }
            (SELF, "getIsJudgeRange") => Value::Bool(self.judge_range),
            // Stand-ins (not read yet): the spots near the player, 300 units off.
            (SELF, "isRangeIn") => Value::Bool(self.sees(args.first().map_or(self.player, vec))),
            (SELF, "isMaxRangeIn") => Value::Bool(self.within_reach(args.first().map_or(self.player, vec))),
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
    fn spots() {
        let mut c = Creature::new([1000.0, 0.0, 1000.0], crate::rng::Rng::from_state([1, 2, 3, 4]));
        c.sight = 8.0;
        c.reach = 5.0;
        for _ in 0..50 {
            let s = c.random_spot();
            assert!((s[0] - 1000.0).abs() <= 800.0 && (s[2] - 1000.0).abs() <= 800.0);
        }
        c.player = [1000.0, 0.0, 2000.0];
        c.player_facing = 0.0;
        for _ in 0..50 {
            // Facing +z with 90°: never more than 45° off +z.
            let e = c.escape_spot(90.0);
            let (dx, dz) = (e[0] - 1000.0, e[2] - 1000.0);
            assert!(dz >= -1e-6 && dx.abs() <= dz + 1e-6, "{e:?}");
        }
        let b = c.behind_player();
        assert!(b[2] >= 2100.0 && b[2] <= 2300.0 && (b[0] - 1000.0).abs() < 1e-6, "{b:?}");
    }

    #[test]
    fn sight_cone_and_reach() {
        let mut c = Creature::new([0.0; 3], crate::rng::Rng::from_state([1, 2, 3, 4]));
        c.sight = 12.0;
        c.view = 220.0;
        c.reach = 25.0;
        // Facing +z: ahead within 12 m, beside within the 110° half-cone, not behind.
        assert!(c.sees([0.0, 0.0, 1000.0]));
        assert!(!c.sees([0.0, 0.0, 1300.0]));
        assert!(c.sees([1000.0, 0.0, 0.0]));
        assert!(!c.sees([0.0, 0.0, -500.0]));
        assert!(c.within_reach([2400.0, 0.0, 0.0]) && !c.within_reach([2600.0, 0.0, 0.0]));
    }

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
        c.wait_time = 2.0;
        c.fixed_wait = true;
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
