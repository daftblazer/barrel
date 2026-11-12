//! The event camera's commands (`t3` to `t9`, handled by `CameraCManual`, `0x1401eb170` in
//! the English executable): the camera's eye and the point it looks at are each a
//! [`Position`], set at once or moved there over time ([`Move`]).
//!
//! Character moves (`s18`) build their moves with the same code (`docs/events.md`, "Event
//! positions and moves").

/// What a position is measured from: its first three operands `{kind, id, point}`
/// (`0x1402f0fa0`). Kinds 3 and the default are characters; the others are named by where
/// the code looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// Kind 0: the field's origin; the offset is the position itself.
    World,
    /// Kind 6: one of four saved points (`point`, clamped to 0..=3).
    Saved(u32),
    /// Any other kind: a thing found by `kind` and `id` (in the events, kind 3 with ids near
    /// 10050 and kind 5 with 10000 and up), at its point `point` (a per-model point such as
    /// the feet or the head; 27 of them, `0x1401f3b80`).
    Thing { kind: u32, id: u32, point: u32 },
}

/// A point: an anchor plus an offset in field units. Unless the anchor is the world or the
/// point is one of 14..=26, the offset is turned by the anchor's facing (about the vertical
/// axis) before it is added (`0x1402f1250`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub anchor: Anchor,
    pub offset: [f32; 3],
}

impl Position {
    /// Reads `{kind, id, point, x, y, z}` (`0x1402f13f0`); the rest of the operands follow.
    pub fn read(operands: &[u32]) -> Option<(Position, &[u32])> {
        let (head, rest) = operands.split_first_chunk::<6>()?;
        let [kind, id, point, x, y, z] = *head;
        let anchor = match kind {
            0 => Anchor::World,
            6 => Anchor::Saved(point.min(3)),
            _ => Anchor::Thing { kind, id, point },
        };
        // The game divides each by 100 and multiplies it back: whole field units stay as they are.
        let f = |v: u32| v as i32 as f32;
        Some((Position { anchor, offset: [f(x), f(y), f(z)] }, rest))
    }

    /// Whether the offset is turned by the anchor's facing.
    pub fn follows_facing(&self) -> bool {
        match self.anchor {
            Anchor::World | Anchor::Saved(_) => false,
            Anchor::Thing { point, .. } => !(14..=26).contains(&point),
        }
    }

    /// The point in the field, given the anchor's place and facing (radians about the
    /// vertical axis; `None` when the anchor is not there). The turn is the game's Y rotation
    /// matrix (`0x1400c2930`, from `0x140028d50`'s sine and cosine) applied to the offset as a
    /// row vector: x' = x·cos + z·sin, z' = z·cos − x·sin.
    pub fn resolve(&self, anchor: Option<([f32; 3], f32)>) -> [f32; 3] {
        let ([bx, by, bz], yaw) = match self.anchor {
            Anchor::World => ([0.0; 3], 0.0),
            _ => anchor.unwrap_or(([0.0; 3], 0.0)),
        };
        let [x, y, z] = self.offset;
        let (s, c) = if self.follows_facing() { yaw.sin_cos() } else { (0.0, 1.0) };
        [bx + x * c + z * s, by + y, bz - x * s + z * c]
    }
}

/// A move to a position over time (`0x1402f1a90`; a move command's operands after the
/// position are `{type, ...}`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Move {
    /// Type 4, nearly every camera move: straight there in `seconds`, eased by `ease`
    /// (`0x1402f3480`, updated by `0x1402f3530`; see [`eased`]).
    Straight { to: Position, seconds: f32, ease: i32 },
    /// Type 0, most character moves (`s18`): towards the position at `speed` field units a
    /// second (150 the commonest, a walk), its velocity the unit direction times the speed,
    /// plus `accel` (mostly zero) turned to the direction of travel (`0x1402f3720`).
    Speed { to: Position, speed: f32, accel: [f32; 3] },
    /// Type 3: a jump to the position (`0x1402f2680`; `0x1402f28f0` works out the launch):
    /// launched at `speed` (800 to 2,500 field units a second in the events) at the angle and
    /// with the gravity that make it peak `height` (50 to 300) above where it starts and come
    /// down on the position.
    Jump { to: Position, speed: f32, height: f32 },
    /// Type 1 (`MdCircle`, built by `0x1402f2e90`, updated by `0x1402f3090`): around the
    /// position, in a plane turned by `turn` (degrees about x, y, z; the anchor's facing is
    /// added to y), the angle going from `from` to `to` (degrees) at a turning speed going
    /// evenly from `speed0` to `speed1` (degrees a second), the radius from `r0` to `r1` and
    /// the height by `rise` (field units) in step with it.
    Circle { to: Position, turn: [f32; 3], r0: f32, r1: f32, rise: f32, from: f32, until: f32, speed0: f32, speed1: f32 },
    /// Type 2 (two operands, a handful of uses), not read yet.
    Other { to: Position, kind: u32 },
}

impl Move {
    /// Where it goes.
    pub fn to_mut(&mut self) -> &mut Position {
        match self {
            Move::Straight { to, .. } | Move::Speed { to, .. } | Move::Jump { to, .. } | Move::Circle { to, .. } | Move::Other { to, .. } => to,
        }
    }

    /// Reads `{position, type, ...}`.
    pub fn read(operands: &[u32]) -> Option<Move> {
        let (to, rest) = Position::read(operands)?;
        Self::read_type(to, rest)
    }

    /// Reads `{position, extra, type, ...}`, the form of `s18` (`ECT_CHARA_MOVE_SET`), `s20` and
    /// `s41`: the extra operand (0, 1 or 2 in `s18`) goes to the move's +0x38, which its update
    /// checks (what it switches is not read yet).
    pub fn read_with_extra(operands: &[u32]) -> Option<(u32, Move)> {
        let (to, rest) = Position::read(operands)?;
        let (&extra, rest) = rest.split_first()?;
        Some((extra, Self::read_type(to, rest)?))
    }

    fn read_type(to: Position, rest: &[u32]) -> Option<Move> {
        let f = |v: u32| v as i32 as f32;
        let (&kind, rest) = rest.split_first()?;
        Some(match (kind, rest) {
            (4, [hundredths, ease, ..]) => Move::Straight { to, seconds: f(*hundredths) / 100.0, ease: *ease as i32 },
            (0, [speed, x, y, z, ..]) => Move::Speed { to, speed: f(*speed), accel: [f(*x), f(*y), f(*z)] },
            (0, [speed]) => Move::Speed { to, speed: f(*speed), accel: [0.0; 3] },
            (3, [speed, height, ..]) => Move::Jump { to, speed: f(*speed), height: f(*height) },
            (1, [ax, ay, az, r0, r1, rise, from, until, speed0, speed1, ..]) => Move::Circle {
                to,
                turn: [f(*ax) / 100.0, f(*ay) / 100.0, f(*az) / 100.0],
                r0: f(*r0),
                r1: f(*r1),
                rise: f(*rise),
                from: f(*from),
                until: f(*until),
                speed0: f(*speed0) / 100.0,
                speed1: f(*speed1) / 100.0,
            },
            _ => Move::Other { to, kind },
        })
    }

    pub fn to(&self) -> &Position {
        match self {
            Move::Straight { to, .. } | Move::Speed { to, .. } | Move::Jump { to, .. } | Move::Circle { to, .. } | Move::Other { to, .. } => to,
        }
    }
}

/// How far along a straight move is, 0 to 1, after `elapsed` of `seconds`: with u the part of
/// the time gone, u · (1 + ease/100 · (1 − u)). Ease 100 starts fast and slows to a stop
/// (u·(2 − u)), −100 starts slow (u²), 0 is even; the events use 100 most, then −100 and 0.
pub fn eased(elapsed: f32, seconds: f32, ease: i32) -> f32 {
    if seconds <= 0.0 || elapsed >= seconds {
        return 1.0;
    }
    let u = (elapsed / seconds).max(0.0);
    u * (1.0 + ease as f32 / 100.0 * (1.0 - u))
}

/// A move under way: a camera point or a character going from where it was to a resolved
/// point, one frame at a time, as the game's move objects do (their update is vtable slot 1:
/// `0x1402f3530` for [`Move::Straight`], `0x1402f3a70` for [`Move::Speed`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Mover {
    from: [f32; 3],
    to: [f32; 3],
    kind: MoverKind,
    /// Seconds left (the move's +0x28).
    left: f32,
    position: [f32; 3],
    /// The extra operand of `s18`: 1 turns the mover to face the way it goes (on the ground).
    face: u32,
    facing: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
enum MoverKind {
    Straight { seconds: f32, ease: i32 },
    Speed { velocity: [f32; 3], accel: [f32; 3] },
    Jump { velocity: [f32; 3], gravity: f32 },
    Circle(Circle),
}

/// A circle under way (`MdCircle`; angles in radians).
#[derive(Clone, Debug, PartialEq)]
struct Circle {
    centre: [f32; 3],
    turn: [f32; 3],
    r0: f32,
    r1: f32,
    rise: f32,
    from: f32,
    until: f32,
    angle: f32,
    speed: f32,
    accel: f32,
}

impl Circle {
    /// The point at the current angle: (radius, height, 0) turned about y by the angle, then
    /// about x, z and y by the plane's turns (the game's matrices, row vectors: y·x·z, then
    /// the y turn), from the centre.
    fn point(&self) -> [f32; 3] {
        let span = self.until - self.from;
        let f = if span == 0.0 { 1.0 } else { (self.angle - self.from) / span };
        let r = self.r0 + (self.r1 - self.r0) * f;
        let h = self.rise * f;
        let yaw = |[x, y, z]: [f32; 3], a: f32| {
            let (s, c) = a.sin_cos();
            [x * c + z * s, y, z * c - x * s]
        };
        let pitch = |[x, y, z]: [f32; 3], a: f32| {
            let (s, c) = a.sin_cos();
            [x, y * c - z * s, y * s + z * c]
        };
        let roll = |[x, y, z]: [f32; 3], a: f32| {
            let (s, c) = a.sin_cos();
            [x * c - y * s, x * s + y * c, z]
        };
        let v = yaw(roll(pitch(yaw([r, h, 0.0], self.angle), self.turn[0]), self.turn[2]), self.turn[1]);
        [self.centre[0] + v[0], self.centre[1] + v[1], self.centre[2] + v[2]]
    }
}

impl Mover {
    /// A mover from `from` to `to` (the move's position, already resolved). `None` for the
    /// type not read yet (type 2).
    pub fn new(from: [f32; 3], to: [f32; 3], movement: &Move, face: u32) -> Option<Mover> {
        Self::anchored(from, to, 0.0, movement, face)
    }

    /// The same, with the facing of the thing the position is anchored to (radians), which
    /// turns a circle's plane.
    pub fn anchored(from: [f32; 3], to: [f32; 3], anchor_yaw: f32, movement: &Move, face: u32) -> Option<Mover> {
        let (kind, left) = match *movement {
            Move::Jump { speed, height, .. } => {
                // 0x1402f28f0: with D the distance across the ground and dy the rise to the
                // point, the launch angle is atan(2 (√H·√(H − dy) + H) / D) for a peak H above
                // the start; the time is D over the speed across the ground, and the gravity
                // brings the climb to a stop at the peak, after √H / (√H + √(H − dy)) of it.
                // A peak below the point, or no distance across, and the jump never runs.
                let d = sub(to, from);
                let across = (d[0] * d[0] + d[2] * d[2]).sqrt();
                if height - d[1] < 0.0 || across == 0.0 {
                    return Some(Mover::arrived(from, from, face));
                }
                let (rise, fall) = (height.sqrt(), (height - d[1]).sqrt());
                let angle = (2.0 * (rise * fall + height) / across).atan();
                let (ground, up) = (angle.cos() * speed, angle.sin() * speed);
                let t = across / ground;
                let gravity = up / (t * rise / (rise + fall));
                let velocity = [d[0] / across * ground, up, d[2] / across * ground];
                (MoverKind::Jump { velocity, gravity }, t)
            }
            Move::Circle { turn, r0, r1, rise, from: a0, until: a1, speed0, speed1, .. } => {
                // 0x1402f2e90: the time is the angle to go over the mean speed, the speed
                // changing evenly over it; a time not above 0 leaves the speed as it is.
                let (a0, a1, s0, s1) = (a0.to_radians(), a1.to_radians(), speed0.to_radians(), speed1.to_radians());
                let mean = (s0 + s1) * 0.5;
                let t = if mean == 0.0 { 0.0 } else { (a1 - a0) / mean };
                let accel = if t > 0.0 { (s1 - s0) / t } else { 0.0 };
                let turn = [turn[0].to_radians(), turn[1].to_radians() + anchor_yaw, turn[2].to_radians()];
                let c = Circle { centre: to, turn, r0, r1, rise, from: a0, until: a1, angle: a0, speed: s0, accel };
                let (start, end) = (c.point(), Circle { angle: a1, ..c.clone() }.point());
                if t <= 0.0 {
                    // The game's circle then never runs: the mover stays where it is.
                    return Some(Mover::arrived(from, from, face));
                }
                return Some(Mover { from: start, to: end, kind: MoverKind::Circle(c), left: t, position: start, face, facing: None });
            }
            Move::Straight { seconds, ease, .. } => (MoverKind::Straight { seconds, ease }, seconds),
            Move::Speed { speed, accel, .. } => {
                // 0x1402f3720: the velocity is the direction times the speed, the time the
                // distance over the speed. The extra vector is turned to the direction of travel
                // (yaw from the ground-plane direction) and taken off the velocity every second;
                // half of it times the time is added at the start, so the move still ends on
                // the point, bowed.
                let d = sub(to, from);
                let distance = len(d);
                if distance == 0.0 {
                    return Some(Mover::arrived(from, to, face));
                }
                let speed = if speed == 0.0 { 1.0 } else { speed };
                let dir = d.map(|v| v / distance);
                let yaw = if d[0].abs() < 0.01 && d[2].abs() < 0.01 { 0.0 } else { d[0].atan2(d[2]) };
                let (s, c) = yaw.sin_cos();
                let [x, y, z] = accel;
                let accel = [x * c + z * s, y, z * c - x * s];
                let t = distance / speed.abs();
                let velocity = std::array::from_fn(|i| dir[i] * speed + 0.5 * t * accel[i]);
                (MoverKind::Speed { velocity, accel }, t)
            }
            _ => return None,
        };
        if left <= 0.0 {
            // The game's move is never started then; snapping to the end is assumed here.
            return Some(Mover::arrived(from, to, face));
        }
        Some(Mover { from, to, kind, left, position: from, face, facing: None })
    }

    fn arrived(from: [f32; 3], to: [f32; 3], face: u32) -> Mover {
        Mover { from, to, kind: MoverKind::Straight { seconds: 0.0, ease: 0 }, left: 0.0, position: to, face, facing: None }
    }

    pub fn position(&self) -> [f32; 3] {
        self.position
    }

    /// The facing (radians about the vertical axis, 0 towards +z) from the last frame's
    /// travel, when the mover turns (`face` 1) and moved far enough on the ground
    /// (`0x1402f21a0`: more than 0.01 along x or z).
    pub fn facing(&self) -> Option<f32> {
        self.facing
    }

    /// Seconds left (the move's +0x28, which `s21` compares with).
    pub fn seconds_left(&self) -> f32 {
        self.left
    }

    pub fn done(&self) -> bool {
        self.left <= 0.0
    }

    /// Advances by `dt` seconds: the time left goes down first, then the point moves; once
    /// the time is used up the point is the end.
    pub fn step(&mut self, dt: f32) {
        if self.done() {
            return;
        }
        let before = self.position;
        self.left -= dt;
        match &mut self.kind {
            MoverKind::Straight { seconds, ease } => {
                let f = eased(*seconds - self.left, *seconds, *ease);
                self.position = std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * f);
            }
            MoverKind::Speed { velocity, accel } => {
                for i in 0..3 {
                    self.position[i] += velocity[i] * dt;
                    velocity[i] -= accel[i] * dt;
                }
            }
            MoverKind::Jump { velocity, gravity } => {
                // 0x1402f2760: gravity on the climb first, then the move.
                velocity[1] -= *gravity * dt;
                for i in 0..3 {
                    self.position[i] += velocity[i] * dt;
                }
            }
            MoverKind::Circle(c) => {
                // 0x1402f3090: the speed changes, then the angle moves on, stopping at the end.
                c.speed += c.accel * dt;
                c.angle += c.speed * dt;
                if (c.until - c.from) * (c.angle - c.until) >= 0.0 {
                    c.angle = c.until;
                    self.left = 0.0;
                }
                self.position = c.point();
            }
        }
        if self.left <= 0.0 {
            self.position = self.to;
            self.left = 0.0;
        }
        let d = sub(self.position, before);
        if self.face == 1 && (d[0].abs() >= 0.01 || d[2].abs() >= 0.01) {
            self.facing = Some(d[0].atan2(d[2]));
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Which of the camera's two points a command changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Eye,
    Target,
}

/// A change to the camera from one command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// `t3` (eye) and `t6` (target), and both from `t9`.
    Set(Position),
    /// `t4` (eye) and `t7` (target).
    Move(Move),
}

pub const EYE_POS_SET: u32 = 0x7400_0003;
pub const EYE_MOVE_SET: u32 = 0x7400_0004;
pub const AT_POS_SET: u32 = 0x7400_0006;
pub const AT_MOVE_SET: u32 = 0x7400_0007;
/// `t9` (`ECT_CAMERA_EYEAT_POS_SET`): the target's position, then the eye's (in that order,
/// despite the name: the first goes where `t6` puts the target).
pub const EYEAT_POS_SET: u32 = 0x7400_0009;

/// The camera changes a command makes (none if it is not one of these).
pub fn changes(word: u32, operands: &[u32]) -> Vec<(Part, Change)> {
    match word {
        EYE_POS_SET => Position::read(operands).map(|(p, _)| vec![(Part::Eye, Change::Set(p))]).unwrap_or_default(),
        AT_POS_SET => Position::read(operands).map(|(p, _)| vec![(Part::Target, Change::Set(p))]).unwrap_or_default(),
        EYE_MOVE_SET => Move::read(operands).map(|m| vec![(Part::Eye, Change::Move(m))]).unwrap_or_default(),
        AT_MOVE_SET => Move::read(operands).map(|m| vec![(Part::Target, Change::Move(m))]).unwrap_or_default(),
        EYEAT_POS_SET => {
            let Some((at, rest)) = Position::read(operands) else { return vec![] };
            let Some((eye, _)) = Position::read(rest) else { return vec![] };
            vec![(Part::Target, Change::Set(at)), (Part::Eye, Change::Set(eye))]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn jump_move() {
        // A jump 300 across and 50 down, peaking 100 above the start, launched at 800.
        let to = Position { anchor: Anchor::World, offset: [0.0; 3] };
        let m = Move::read_type(to, &[3, 800, 100]).unwrap();
        let mut j = Mover::new([0.0; 3], [300.0, -50.0, 0.0], &m, 0).unwrap();
        let mut top = 0f32;
        for _ in 0..600 {
            j.step(1.0 / 600.0);
            top = top.max(j.position()[1]);
        }
        assert!(j.done());
        assert!((top - 100.0).abs() < 1.0, "{top}");
        assert_eq!(j.position(), [300.0, -50.0, 0.0]);
        // A peak below the point: it never runs.
        let m = Move::read_type(to, &[3, 800, 10]).unwrap();
        assert!(Mover::new([0.0; 3], [300.0, 50.0, 0.0], &m, 0).unwrap().done());
    }

    #[test]
    fn circle_move() {
        // A circle the events use (s18 around the hero): a half turn from 90° to 270° at
        // radius 800, falling 2000, at 150 to 300 degrees a second (so 180 / 225 = 0.8 s).
        let to = Position { anchor: Anchor::World, offset: [0.0; 3] };
        let m = Move::read_type(to, &[1, 0, 0, 0, 800, 800, (-2000i32) as u32, 90, 270, 15000, 30000]).unwrap();
        let mut c = Mover::new([5.0; 3], [100.0, 0.0, 0.0], &m, 0).unwrap();
        // The start: (800, 0, 0) turned 90° about y is (0, 0, -800), from the centre.
        let p = c.position();
        assert!((p[0] - 100.0).abs() < 1e-3 && p[1].abs() < 1e-3 && (p[2] + 800.0).abs() < 1e-3, "{p:?}");
        assert!((c.seconds_left() - 0.8).abs() < 1e-4);
        for _ in 0..60 {
            c.step(1.0 / 60.0);
        }
        assert!(c.done());
        let p = c.position();
        assert!((p[0] - 100.0).abs() < 1e-3 && (p[1] + 2000.0).abs() < 1e-3 && (p[2] - 800.0).abs() < 1e-3, "{p:?}");
    }
    use super::*;

    #[test]
    fn positions_and_moves_read() {
        // A t4 from the events: Escha (kind 5, 10000), her point 2, 3 m out, one second, eased.
        let ops = [5, 10000, 2, 0, 100, 300, 4, 100, 100];
        let Some(Move::Straight { to, seconds, ease }) = Move::read(&ops) else { panic!() };
        assert_eq!(to.anchor, Anchor::Thing { kind: 5, id: 10000, point: 2 });
        assert_eq!((to.offset, seconds, ease), ([0.0, 100.0, 300.0], 1.0, 100));
        let neg = (-200i32) as u32;
        let ops = [0, 0, 1, 10, 20, neg, 7];
        let (p, rest) = Position::read(&ops).unwrap();
        assert_eq!((p.anchor, p.offset, rest), (Anchor::World, [10.0, 20.0, -200.0], &[7u32][..]));
        // An s18 after its character: Escha's place + (0, 0, 500), extra 1, walk at 150.
        let (extra, m) = Move::read_with_extra(&[5, 10000, 0, 0, 0, 500, 1, 0, 150, 0, 0, 0]).unwrap();
        assert_eq!(extra, 1);
        assert!(matches!(m, Move::Speed { speed: 150.0, accel: [0.0, 0.0, 0.0], .. }));
        assert_eq!(changes(EYEAT_POS_SET, &[0, 0, 0, 1, 2, 3, 0, 0, 0, 4, 5, 6]).len(), 2);
    }

    #[test]
    fn offsets_turn_with_the_anchor() {
        let p = Position { anchor: Anchor::Thing { kind: 5, id: 10000, point: 2 }, offset: [0.0, 100.0, 300.0] };
        let q = p.resolve(Some(([10.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2)));
        assert!((q[0] - 310.0).abs() < 1e-3 && (q[1] - 100.0).abs() < 1e-3 && q[2].abs() < 1e-3);
        let bone = Position { anchor: Anchor::Thing { kind: 5, id: 10000, point: 14 }, ..p };
        assert_eq!(bone.resolve(Some(([10.0, 0.0, 0.0], 1.0))), [10.0, 100.0, 300.0]);
    }

    #[test]
    fn movers_arrive() {
        let to = Position { anchor: Anchor::World, offset: [300.0, 0.0, 400.0] };
        // A walk of 500 units at 150 a second takes 10/3 s, facing the way it goes.
        let mut m = Mover::new([0.0; 3], [300.0, 0.0, 400.0], &Move::Speed { to, speed: 150.0, accel: [0.0; 3] }, 1).unwrap();
        for _ in 0..100 {
            m.step(1.0 / 30.0);
        }
        assert!(!m.done());
        let p = m.position();
        assert!((p[0] - 300.0 * 100.0 / 30.0 * 150.0 / 500.0).abs() < 0.1, "{p:?}");
        assert!((m.facing().unwrap() - 0.6435).abs() < 1e-3);
        m.step(1.0 / 30.0);
        assert!(m.done());
        assert_eq!(m.position(), [300.0, 0.0, 400.0]);
        // A bowed walk ends on the point too.
        let mut b = Mover::new([0.0; 3], [0.0, 0.0, 600.0], &Move::Speed { to, speed: 150.0, accel: [0.0, 100.0, 0.0] }, 0).unwrap();
        let mut top: f32 = 0.0;
        while !b.done() {
            b.step(1.0 / 30.0);
            top = top.max(b.position()[1]);
        }
        assert!(top > 150.0 && b.position() == [0.0, 0.0, 600.0]);
        // An eased camera move is three quarters there at half time.
        let mut e = Mover::new([0.0; 3], [100.0, 0.0, 0.0], &Move::Straight { to, seconds: 1.0, ease: 100 }, 0).unwrap();
        e.step(0.5);
        assert!((e.position()[0] - 75.0).abs() < 1e-3);
    }

    #[test]
    fn easing() {
        assert_eq!(eased(0.5, 1.0, 0), 0.5);
        assert_eq!(eased(0.5, 1.0, 100), 0.75);
        assert_eq!(eased(0.5, 1.0, -100), 0.25);
        assert_eq!(eased(2.0, 1.0, 100), 1.0);
        assert_eq!(eased(0.0, 0.0, 100), 1.0);
    }
}
