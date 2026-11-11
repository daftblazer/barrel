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
    /// Type 3: a jump to the position with gravity 9.8 (`0x1402f2680`; `0x1402f28f0` works
    /// out the velocity): `height` (50 to 300) is how high it rises; `first` (800 to 2,500
    /// in the events) is not named yet.
    Jump { to: Position, first: f32, height: f32 },
    /// Types 1 and 2 (type 1: eight more operands, 129 of the 2,006 `t4`s and 63 `s18`s),
    /// not read yet.
    Other { to: Position, kind: u32 },
}

impl Move {
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
            (3, [first, height, ..]) => Move::Jump { to, first: f(*first), height: f(*height) },
            _ => Move::Other { to, kind },
        })
    }

    pub fn to(&self) -> &Position {
        match self {
            Move::Straight { to, .. } | Move::Speed { to, .. } | Move::Jump { to, .. } | Move::Other { to, .. } => to,
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
    fn easing() {
        assert_eq!(eased(0.5, 1.0, 0), 0.5);
        assert_eq!(eased(0.5, 1.0, 100), 0.75);
        assert_eq!(eased(0.5, 1.0, -100), 0.25);
        assert_eq!(eased(2.0, 1.0, 100), 1.0);
        assert_eq!(eased(0.0, 0.0, 100), 1.0);
    }
}
