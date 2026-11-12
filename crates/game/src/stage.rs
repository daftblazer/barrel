//! An event's stage: where its characters and the camera are as the event runs, from the
//! player's timed steps ([`crate::event::Player::schedule`]), moved frame by frame by
//! [`Mover`]s. A frame can be any length (the game's are its measured frame times); see
//! [`FRAME`].
//!
//! Positions measured from a character start from one of its points ([`Measures::point`]),
//! when the stage knows the character's measures (its `Chara.xml` row); otherwise from where
//! it stands. A position whose anchor is not on the stage (an unknown kind, or a
//! character not placed yet) is skipped and counted in [`Stage::unresolved`].

use std::collections::BTreeMap;

use crate::camera::{Anchor, Change, Mover, Part, Position};
use crate::event::{Step, Timed};

/// The step [`Stage::run_until`] advances by: one tick at [`crate::clock::DEFAULT_RATE`]
/// (`docs/engine.md`, "Timing"). The original steps by each frame's measured time instead,
/// with exactly 1/60 s as its fixed fallback (`docs/events.md`, "Frame time"); Barrel always
/// steps by whole ticks and blends ticks for the screen ([`Snapshot::blend`]).
pub const FRAME: f32 = 1.0 / crate::clock::DEFAULT_RATE as f32;

/// What the screen needs of a stage at one tick: where everyone is and faces, and the camera.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// (position, facing) by character number.
    pub characters: BTreeMap<u32, ([f32; 3], f32)>,
    pub eye: Option<[f32; 3]>,
    pub target: Option<[f32; 3]>,
    /// The field camera's field of view (degrees), when it is the camera in use.
    pub fov: Option<f32>,
}

impl Snapshot {
    /// The state `alpha` (0 to 1) of the way from `before` to `after`, for drawing between
    /// ticks. Positions go straight, facings the short way round; a character or camera point
    /// that is new in `after` is drawn where it is there.
    pub fn blend(before: &Snapshot, after: &Snapshot, alpha: f32) -> Snapshot {
        let lerp = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha);
        let point = |a: Option<[f32; 3]>, b: Option<[f32; 3]>| match (a, b) {
            (Some(a), Some(b)) => Some(lerp(a, b)),
            (_, b) => b,
        };
        let characters = after
            .characters
            .iter()
            .map(|(&n, &(p, f))| {
                let v = match before.characters.get(&n) {
                    Some(&(p0, f0)) => (lerp(p0, p), f0 + short_way(f0, f) * alpha),
                    None => (p, f),
                };
                (n, v)
            })
            .collect();
        let fov = match (before.fov, after.fov) {
            (Some(a), Some(b)) => Some(a + (b - a) * alpha),
            (_, b) => b,
        };
        Snapshot { characters, eye: point(before.eye, after.eye), target: point(before.target, after.target), fov }
    }
}

/// A character's measures, from its `Chara.xml` row: its height and radius in field units and
/// the heights of its points as fractions of the height (`Foot`, `Body`, `Face`, `Head`, then
/// the three of `Extra`; Escha's are 0, 0.7, 0.91, 1 of 151).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measures {
    pub height: f32,
    pub radius: f32,
    pub fractions: [f32; 7],
}

impl Measures {
    pub fn from_row(row: &barrel_formats::table::Element) -> Option<Measures> {
        let num = |k: &str| row.attr(k)?.trim().parse::<f32>().ok();
        let mut extra = row.attr("Extra")?.split(',').map(|v| v.trim().parse::<f32>().ok());
        let mut e = || extra.next().flatten();
        Some(Measures {
            height: num("Height")?,
            radius: num("Radius")?,
            fractions: [num("Foot")?, num("Body")?, num("Face")?, num("Head")?, e()?, e()?, e()?],
        })
    }

    /// Every row of `Chara.xml`, by row number (the character numbers of story events).
    pub fn table(chara: &barrel_formats::table::Element) -> BTreeMap<u32, Measures> {
        chara.children.iter().enumerate().filter_map(|(i, r)| Some((i as u32, Measures::from_row(r)?))).collect()
    }

    /// Point `point` of a character standing at `position` facing `facing` (`0x1401f3b80`):
    ///
    /// - 1 to 7 (and again 17 to 23): up by the height times `Foot`, `Body`, `Face`, `Head`,
    ///   `Extra` x, y, z;
    /// - 8 and 24: the same place at height 0;
    /// - 10 and 12: at height 0, the radius in front of or behind the character;
    /// - 0 and 26 (the character's +0x3d8), 9 and 11 (the radius along the camera's direction)
    ///   and 13 to 16, 25: not read, the place itself.
    pub fn point(&self, point: u32, position: [f32; 3], facing: f32) -> [f32; 3] {
        let [x, y, z] = position;
        match point {
            1..=7 => [x, y + self.height * self.fractions[point as usize - 1], z],
            17..=23 => [x, y + self.height * self.fractions[point as usize - 17], z],
            8 | 24 => [x, 0.0, z],
            10 | 12 => {
                let r = if point == 10 { self.radius } else { -self.radius };
                let (s, c) = facing.sin_cos();
                [x + r * s, 0.0, z + r * c]
            }
            _ => position,
        }
    }
}

/// A motion a character plays: its `MOTION_*` number, since when (stage seconds), whether
/// it loops, and over how many seconds it blends in from the motion before.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playing {
    pub motion: u32,
    pub since: f32,
    pub looping: bool,
    pub blend: f32,
    /// The division of the motion it plays (`s5`, `s11`; `barrel_formats::chara_div`), or the
    /// whole clip.
    pub division: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub position: [f32; 3],
    /// Radians about the vertical axis, 0 towards +z.
    pub facing: f32,
    mover: Option<Mover>,
    /// The motion it plays; `None` until a command sets one.
    pub motion: Option<Playing>,
    /// Its face: the expression its latest line set (`None` before any), and whether it is
    /// speaking (its line is the one shown).
    pub expression: Option<i32>,
    pub talking: bool,
    /// The gesture its latest line set (a gesture number, `docs/events.md`) and when.
    pub gesture: Option<(i32, f32)>,
    /// Its head gesture (`s116`): segment, rate, weight, and the stage time it started.
    pub head: Option<(usize, f32, f32, f32)>,
    /// Whether it collides (`q8`; kept, not acted on: scenes move characters on set paths).
    pub solid: bool,
    /// A timed turn: (from, change, seconds, elapsed, curve).
    turn: Option<(f32, f32, f32, f32, u32)>,
}

/// The change from angle `from` to `to` the short way round (`0x1401f42e0`: one turn of 2π is
/// added or taken off when that makes the change smaller).
pub fn short_way(from: f32, to: f32) -> f32 {
    use std::f32::consts::TAU;
    let d = to - from;
    if d < 0.0 && d + TAU < -d {
        d + TAU
    } else if d > 0.0 && TAU - d < d {
        d - TAU
    } else {
        d
    }
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

/// A sound the event asks for, for whoever plays the stage (it has no sound of its own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// A music track starts at `volume` (0 to 1), or (`track` None) the music stops over
    /// `fade` seconds.
    Music { track: Option<u32>, volume: f32, fade: f32 },
    /// A voice clip plays.
    Voice { clip: u32 },
    /// A sound effect plays at `volume` and `pan` (-1 to 1).
    Effect { effect: u32, volume: f32, pan: f32 },
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
    /// The characters' measures, by character number (see [`Measures::table`]).
    pub measures: BTreeMap<u32, Measures>,
    /// The curves timed turns follow (`curve_data.xml`, [`crate::curve::Curve::table`]).
    pub curves: BTreeMap<u32, crate::curve::Curve>,
    /// Sounds asked for and not yet taken (the player drains them).
    pub cues: Vec<Cue>,
    /// The field-map camera, once an event's `t20` has set it; the latest of it and the
    /// manual camera ([`Stage::eye`], [`Stage::target`]) is the one in use.
    pub field_camera: Option<crate::field_camera::FieldCamera>,
    /// The field map the event is in (its latest `FieldChange`).
    pub field: Option<u32>,
    pub manual_camera: bool,
    /// The latest screen effect (`u13`): kind, parameter, seconds, and the stage time it started.
    pub screen_effect: Option<(u32, i32, f32, f32)>,
    /// The latest fade (`u22`): preset, seconds, and the stage time it started.
    pub fade: Option<(u32, f32, f32)>,
    /// The narration card on screen (`u55`) and its opacity (`u56`).
    pub telop: Option<u32>,
    /// The still picture shown (`u1/0x30`, `u2/0x30`): it, the one before it, the stage time it
    /// came and the seconds it fades in over.
    pub still: Option<(u32, Option<u32>, f32, f32)>,
    pub telop_alpha: crate::field_camera::Tween,
    /// The latest line ([`Stage::line_shown`] says whether it is still up).
    pub line: Option<Line>,
    /// How long a line stays up, for playback nobody reads along with: the reading time the
    /// schedule holds each line for (`event::Player::reading_time`). None keeps each line up
    /// until the next.
    pub reading: Option<fn(&str) -> f32>,
    /// The field object versions the event shows (`q15`); objects tagged with a version not
    /// here are hidden (`event::FIELD_MAP_VISIBLE`).
    pub versions_shown: std::collections::BTreeSet<u32>,
    /// The camera quake running (`t14`): quake number, the stage time it started and its
    /// seconds; `t16` or the end of its time stops it.
    pub quake: Option<(u32, f32, f32)>,
}

/// A line of the event's text.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The character number of whoever says it (-1 for none: narration, system text).
    pub speaker: i32,
    /// The name shown (an entry of the speaker-name table), when the message gives one.
    pub name: Option<i32>,
    /// As the message file has it (markup included).
    pub text: String,
    /// The stage time it goes down at, if it does before the next line.
    pub until: Option<f32>,
}

impl Stage {
    fn resolve(&self, p: &Position) -> Option<[f32; 3]> {
        match p.anchor {
            Anchor::World => Some(p.resolve(None)),
            // Kind 3 names a thing whose character gives the point; its ids are not
            // character numbers, so only the other kinds are looked up here.
            Anchor::Thing { kind, id, point } if kind != 2 && kind != 3 && kind != 4 => {
                let b = self.characters.get(&id)?;
                let base = self.measures.get(&id).map_or(b.position, |m| m.point(point, b.position, b.facing));
                Some(p.resolve(Some((base, b.facing))))
            }
            _ => None,
        }
    }

    /// Applies a step at the stage's current time.
    /// The line on screen now, if any.
    pub fn line_shown(&self) -> Option<&Line> {
        self.line.as_ref().filter(|l| l.until.is_none_or(|u| self.time < u))
    }

    /// How many seconds the character's move has left, while it moves.
    pub fn move_left(&self, character: u32) -> Option<f32> {
        self.characters.get(&character)?.mover.as_ref().map(Mover::seconds_left)
    }

    pub fn apply(&mut self, step: &Step) {
        match step {
            Step::Place { character, position, .. } => {
                let b = self.characters.entry(*character).or_insert(Body { position: *position, facing: 0.0, mover: None, turn: None, motion: None, expression: None, talking: false, gesture: None, head: None, solid: true });
                b.position = *position;
                b.mover = None;
            }
            Step::Motion { character, motion, looping, blend, division, .. } => match self.characters.get_mut(character) {
                Some(b) => b.motion = Some(Playing { motion: *motion, since: self.time, looping: *looping, blend: blend.max(0.0), division: *division }),
                None => self.unresolved += 1,
            },
            // A line: its speaker talks (and takes the line's expression, if it sets one); the
            // others fall quiet and keep their faces.
            Step::Message { speaker, name, text, expression, gesture, .. } => {
                let now = self.time;
                let until = self.reading.map(|f| now + f(&text.replace("<CR>", "")).max(0.0));
                self.line = Some(Line { speaker: *speaker, name: *name, text: text.clone(), until });
                for (&n, b) in self.characters.iter_mut() {
                    let speaking = i64::from(n) == i64::from(*speaker);
                    b.talking = speaking;
                    if speaking && expression.is_some() {
                        b.expression = *expression;
                    }
                    if let (true, Some(g)) = (speaking, gesture) {
                        b.gesture = Some((*g, now));
                    }
                }
            }
            Step::RemoveAll { .. } => self.characters.clear(),
            Step::Quake { quake, seconds, .. } => self.quake = Some((*quake, self.time, *seconds)),
            Step::QuakeStop { .. } => self.quake = None,
            Step::MapVisible { version, shown, .. } => {
                if *shown {
                    self.versions_shown.insert(*version);
                } else {
                    self.versions_shown.remove(version);
                }
            }
            Step::Collision { character, solid, .. } => {
                if let Some(b) = self.characters.get_mut(character) {
                    b.solid = *solid;
                }
            }
            Step::Remove { character, .. } => {
                self.characters.remove(character);
            }
            Step::Turn { character, rotation, .. } => match self.characters.get_mut(character) {
                Some(b) => {
                    b.facing = rotation[1];
                    b.turn = None;
                }
                None => self.unresolved += 1,
            },
            // Along its curve (`crate::curve`; evenly when the stage has no curves).
            Step::TurnOver { character, rotation, seconds, curve, .. } => match self.characters.get_mut(character) {
                Some(b) if *seconds > 0.0 => b.turn = Some((b.facing, short_way(b.facing, rotation[1]), *seconds, 0.0, *curve)),
                Some(b) => {
                    b.facing = b.facing + short_way(b.facing, rotation[1]);
                    b.turn = None;
                }
                None => self.unresolved += 1,
            },
            Step::Face { character, toward, .. } => {
                let Some(at) = self.resolve(toward) else {
                    self.unresolved += 1;
                    return;
                };
                let Some(b) = self.characters.get_mut(character) else {
                    self.unresolved += 1;
                    return;
                };
                let (dx, dz) = (at[0] - b.position[0], at[2] - b.position[2]);
                b.facing = if dx == 0.0 && dz == 0.0 { 0.0 } else { dx.atan2(dz) };
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
            Step::FieldChange { map, .. } => self.field = Some(*map),
            Step::Fade { preset, seconds, .. } => self.fade = Some((*preset, *seconds, self.time)),
            Step::Telop { card, .. } => self.telop = Some(*card),
            Step::Still { picture, seconds, .. } => self.still = Some((*picture, self.still.map(|s| s.0), self.time, *seconds)),
            Step::MotionDivision { character, division, .. } => {
                let now = self.time;
                if let Some(p) = self.characters.get_mut(character).and_then(|b| b.motion.as_mut()) {
                    *p = Playing { since: now, blend: 0.0, division: Some(*division), ..*p };
                }
            }
            Step::HeadGesture { character, segment, rate, weight, .. } => {
                let now = self.time;
                if let Some(b) = self.characters.get_mut(character) {
                    b.head = usize::try_from(*segment).ok().filter(|&s| s < crate::event::HEAD_SEGMENTS.len()).map(|s| (s, *rate, *weight, now));
                }
            }
            Step::HeadStop { character, .. } => {
                if let Some(b) = self.characters.get_mut(character) {
                    b.head = None;
                }
            }
            Step::TelopAlpha { from, to, seconds, .. } => self.telop_alpha = crate::field_camera::Tween { from: *from, to: *to, seconds: *seconds, elapsed: 0.0 },
            Step::ScreenEffect { kind, param, seconds, .. } => self.screen_effect = Some((*kind, *param, *seconds, self.time)),
            // t19 only flips a flag; it neither makes the field camera nor takes the view.
            Step::FieldCamera { change: crate::field_camera::Change::DataEffect { on }, .. } => {
                if let Some(c) = self.field_camera.as_mut() {
                    c.data_effect = *on;
                }
            }
            Step::FieldCamera { change, .. } => {
                match self.field_camera.as_mut() {
                    Some(c) => c.apply(change),
                    None => self.field_camera = crate::field_camera::FieldCamera::new(change),
                }
                self.manual_camera = false;
            }
            Step::Camera { part, change, .. } => {
                self.manual_camera = true;
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
            Step::Music { track, volume, fade, .. } => self.cues.push(Cue::Music { track: *track, volume: *volume, fade: *fade }),
            Step::Voice { clip, .. } => self.cues.push(Cue::Voice { clip: *clip }),
            // Only the event-wide command: `s62`'s numbers are not read (in battle skill events it
            // names 151, an ambience loop, which looks like another numbering).
            Step::Effect { character: None, effect, volume, pan, .. } => self.cues.push(Cue::Effect { effect: *effect, volume: *volume, pan: *pan }),
            _ => {}
        }
    }

    /// Runs the movers one frame of `dt` seconds.
    pub fn frame(&mut self, dt: f32) {
        if let Some(c) = self.field_camera.as_mut() {
            c.step(dt);
        }
        for b in self.characters.values_mut() {
            if let Some((from, change, seconds, elapsed, curve)) = b.turn.as_mut() {
                *elapsed += dt;
                let u = (*elapsed / *seconds).min(1.0);
                let f = if u >= 1.0 {
                    1.0
                } else {
                    self.curves.get(curve).map_or(u, |c| c.at(u * crate::curve::FRAMES))
                };
                b.facing = *from + *change * f;
                if u >= 1.0 {
                    b.turn = None;
                }
            }
            if let Some(m) = b.mover.as_mut().filter(|m| !m.done()) {
                m.step(dt);
                b.position = m.position();
                if let Some(f) = m.facing() {
                    b.facing = f;
                }
            }
        }
        for p in [&mut self.eye, &mut self.target] {
            if let Some(m) = p.mover.as_mut().filter(|m| !m.done()) {
                m.step(dt);
                p.position = Some(m.position());
            }
        }
        self.telop_alpha.step(dt);
        self.time += dt;
    }

    /// The state the screen needs now.
    pub fn snapshot(&self) -> Snapshot {
        // The camera in use: the field camera unless the manual one was set after it.
        let (eye, target, fov) = match (&self.field_camera, self.manual_camera) {
            (Some(c), false) => {
                let (e, t) = c.eye_target();
                (Some(e), Some(t), Some(c.fov.now()))
            }
            _ => (self.eye.position, self.target.position, None),
        };
        Snapshot { characters: self.characters.iter().map(|(&n, b)| (n, (b.position, b.facing))).collect(), eye, target, fov }
    }

    /// Runs whole [`FRAME`]s until `time`.
    pub fn run_until(&mut self, time: f32) {
        while self.time + FRAME * 0.5 < time {
            self.frame(FRAME);
        }
    }

    /// Plays a timed event: each step is applied at its time, the movers run in between, and
    /// `each` sees the stage after every step.
    /// (Set [`Stage::measures`] and [`Stage::curves`] first.)
    pub fn play<'a>(self, steps: &[Timed<'a>], mut each: impl FnMut(&Timed<'a>, &Stage)) -> Stage {
        let mut stage = self;
        for t in steps {
            stage.run_until(t.time);
            stage.apply(&t.step);
            each(t, &stage);
        }
        stage
    }
}

/// A stage played tick by tick, for a host driving it from [`crate::clock::Clock`]: each
/// [`Playback::tick`] applies the steps due by then and runs one [`FRAME`]. It gives the same
/// stage as [`Stage::play`] at the same tick.
pub struct Playback<'a> {
    pub stage: Stage,
    steps: Vec<Timed<'a>>,
    next: usize,
}

impl<'a> Playback<'a> {
    pub fn new(stage: Stage, steps: Vec<Timed<'a>>) -> Playback<'a> {
        Playback { stage, steps, next: 0 }
    }

    /// Whether every step has been applied.
    pub fn finished(&self) -> bool {
        self.next >= self.steps.len()
    }

    /// One tick: the steps due now (within half a tick), then the movers.
    pub fn tick(&mut self) {
        while let Some(t) = self.steps.get(self.next).filter(|t| t.time <= self.stage.time + FRAME * 0.5) {
            self.stage.apply(&t.step);
            self.next += 1;
        }
        self.stage.frame(FRAME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Move;

    #[test]
    fn the_same_ticks_at_any_frame_rate() {
        use crate::camera::{Anchor, Move, Position};
        use crate::clock::Clock;
        let to = Position { anchor: Anchor::World, offset: [600.0, 0.0, 450.0] };
        let steps = vec![
            Timed { time: 0.0, action: 0, step: Step::Place { action: 0, character: 1, position: [0.0; 3] } },
            Timed {
                time: 0.5,
                action: 0,
                step: Step::CharacterMove { action: 0, character: 1, extra: 1, movement: Move::Speed { to, speed: 150.0, accel: [0.0, 80.0, 0.0] } },
            },
        ];
        let run = |hz: f64| {
            let mut clock = Clock::default();
            let mut p = Playback::new(Stage::default(), steps.clone());
            let mut before = p.stage.snapshot();
            let mut drawn = Snapshot::default();
            while clock.time() < 3.0 {
                let f = clock.advance(1.0 / hz);
                for _ in 0..f.ticks {
                    before = p.stage.snapshot();
                    p.tick();
                }
                drawn = Snapshot::blend(&before, &p.stage.snapshot(), f.alpha);
            }
            (clock.ticks, p.stage.snapshot(), drawn)
        };
        let (t30, s30, _) = run(30.0);
        let (t144, s144, d144) = run(144.0);
        // The logic lands on the same tick with the same state; only the drawn blend differs.
        assert_eq!((t30, &s30), (t144, &s144));
        assert!(d144.characters[&1].0[0] <= s144.characters[&1].0[0]);
        // And it matches playing the steps straight through.
        let mut straight = Stage::default().play(&steps, |_, _| {});
        straight.run_until(t30 as f32 / 60.0);
        assert_eq!(straight.snapshot().characters[&1].0, s30.characters[&1].0);
    }

    #[test]
    fn lines_set_faces() {
        let mut stage = Stage::default();
        let line = |speaker: i32, expression: Option<i32>| Step::Message { index: 0, speaker, name: None, text: String::new(), expression, gesture: None };
        stage.apply(&Step::Place { action: 0, character: 0, position: [0.0; 3] });
        stage.apply(&Step::Place { action: 0, character: 1, position: [0.0; 3] });
        stage.apply(&line(0, Some(4)));
        assert_eq!((stage.characters[&0].expression, stage.characters[&0].talking), (Some(4), true));
        stage.apply(&Step::Message { index: 1, speaker: 0, name: None, text: String::new(), expression: None, gesture: Some(7) });
        assert_eq!((stage.characters[&0].expression, stage.characters[&0].gesture), (Some(4), Some((7, 0.0))));
        stage.apply(&line(1, None));
        assert_eq!((stage.characters[&0].expression, stage.characters[&0].talking), (Some(4), false));
        assert_eq!((stage.characters[&1].expression, stage.characters[&1].talking), (None, true));
    }

    #[test]
    fn lines_stay_up_for_their_reading_time() {
        let mut stage = Stage { reading: Some(|t: &str| t.len() as f32), ..Stage::default() };
        stage.apply(&Step::Message { index: 0, speaker: 3, name: None, text: "ab<CR>c".into(), expression: None, gesture: None });
        // "abc" reads in 3 seconds.
        assert_eq!(stage.line_shown().map(|l| (l.speaker, l.until)), Some((3, Some(3.0))));
        stage.time = 2.9;
        assert!(stage.line_shown().is_some());
        stage.time = 3.0;
        assert!(stage.line_shown().is_none());
        // With no reading time a line stays until the next.
        let mut stage = Stage::default();
        stage.apply(&Step::Message { index: 0, speaker: 3, name: None, text: "ab".into(), expression: None, gesture: None });
        stage.time = 100.0;
        assert!(stage.line_shown().is_some());
    }

    #[test]
    fn ticks_blend_for_the_screen() {
        let a = Snapshot { characters: BTreeMap::from([(1, ([0.0; 3], 350f32.to_radians()))]), eye: None, target: Some([0.0; 3]), fov: None };
        let b = Snapshot {
            characters: BTreeMap::from([(1, ([10.0, 0.0, 0.0], 10f32.to_radians())), (2, ([5.0; 3], 0.0))]),
            eye: Some([1.0; 3]),
            target: Some([4.0, 0.0, 0.0]),
            fov: None,
        };
        let m = Snapshot::blend(&a, &b, 0.25);
        let (p, f) = m.characters[&1];
        assert_eq!(p, [2.5, 0.0, 0.0]);
        // 350° to 10° a quarter of the way is 355°, not 260°.
        assert!((f.to_degrees() - 355.0).abs() < 1e-3);
        assert_eq!(m.characters[&2], ([5.0; 3], 0.0));
        assert_eq!((m.eye, m.target), (Some([1.0; 3]), Some([1.0, 0.0, 0.0])));
    }

    #[test]
    fn a_character_walks_and_the_camera_follows() {
        let world = |x: f32, z: f32| Position { anchor: Anchor::World, offset: [x, 0.0, z] };
        let on = |id: u32, z: f32| Position { anchor: Anchor::Thing { kind: 5, id, point: 3 }, offset: [0.0, 10.0, z] };
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
        let escha = Measures { height: 151.0, radius: 27.0, fractions: [0.0, 0.7, 0.91, 1.0, 0.7, 1.0, 1.0] };
        let mut steps = steps;
        steps.push(Timed { time: 4.0, action: 0, step: Step::Place { action: 0, character: 8, position: [300.0, 0.0, -100.0] } });
        steps.push(Timed { time: 4.0, action: 0, step: Step::Face { action: 0, character: 8, toward: on(7, 0.0) } });
        let stage = Stage { measures: BTreeMap::from([(7, escha)]), ..Stage::default() }.play(&steps, |_, _| {});
        assert!(stage.characters[&8].facing.abs() < 1e-6);
        // From 350° to 10° is 20° the short way.
        assert!((short_way(350f32.to_radians(), 10f32.to_radians()) - 20f32.to_radians()).abs() < 1e-5);
        assert!((short_way(0.0, -3.0) + 3.0).abs() < 1e-6);
        let b = &stage.characters[&7];
        assert_eq!(b.position, [300.0, 0.0, 0.0]);
        assert!((b.facing - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        // At her face (0.91 of 151) and 10 up.
        let t = stage.target.position.unwrap();
        assert!((t[0] - 300.0).abs() < 1e-3 && (t[1] - 147.41).abs() < 1e-3 && t[2].abs() < 1e-3, "{t:?}");
        assert_eq!(escha.point(10, [0.0, 50.0, 0.0], 0.0), [0.0, 0.0, 27.0]);
        // Character 9 was never placed.
        assert_eq!((stage.eye.position, stage.unresolved), (None, 1));
    }
}
