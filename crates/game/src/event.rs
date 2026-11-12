//! The event player: runs an event's bytecode and says what happens, step by step.
//!
//! Action 0 is the event's main script; its `w1 N` starts action N alongside it (the game runs
//! them at once; here a started action's steps come right where it starts). Each command
//! becomes a [`Step`]. It knows `u18` (0x75000012), which shows the event's messages
//! from its first operand to its second, inclusive, and the waits `p1` and `u1`.
//! Everything else is reported by name, with its operands, for the systems that will
//! handle it.

use std::collections::HashMap;

use barrel_formats::ebd::{self, Ebd, OperandTable};
use barrel_formats::ebm::Message;
use barrel_formats::Error;

/// `u18`: show messages first..=last.
pub const SHOW_MESSAGES: u32 = 0x7500_0012;
/// `p1` (`ECT_MACRO_TIME_WAIT`): wait, the operand in hundredths of a second.
pub const TIME_WAIT: u32 = 0x7000_0001;
/// `u1` (0x75000001): wait 0.3 seconds (the event player's constant).
pub const SHORT_WAIT: u32 = 0x7500_0001;
/// `p3` (`ECT_MACRO_CHARA_WAIT`): wait for a character (its number).
pub const CHARA_WAIT: u32 = 0x7000_0003;
/// `s3` (`ECT_CHARA_TIME_WAIT`): a character waits, `{character, hundredths}`.
pub const CHARA_TIME_WAIT: u32 = 0x7300_0003;
/// `s4` (`ECT_CHARA_POS_SET`): `{character, 0, 0, 0, x, y, z}`.
pub const CHARA_POS_SET: u32 = 0x7300_0004;
/// `s10` (`ECT_CHARA_MOTION_SET_LOOP`): `{character, motion, blend (hundredths), flag}`.
pub const CHARA_MOTION_LOOP: u32 = 0x7300_000a;
/// `q4` (`ECT_FIELDMAP_CHARA_ADDITION_EX`): `{character, 0, 0, 0, x, y, z, 0, angle, ...}` (14
/// operands): adds the character there, facing `angle` degrees.
pub const FIELD_CHARA_ADD: u32 = 0x7100_0004;
/// `q2` (`ECT_FIELDMAP_CHANGE`) and `q12` (`ECT_FIELDMAP_CHANGE_NOT_BGM`): `{map}`.
pub const FIELD_CHANGE: u32 = 0x7100_0002;
pub const FIELD_CHANGE_KEEP_BGM: u32 = 0x7100_000c;
/// `q5` (`ECT_FIELDMAP_CHARA_DELETE`): `{character}`.
pub const FIELD_CHARA_DELETE: u32 = 0x7100_0005;
/// `q6` (`ECT_FIELDMAP_CHARA_DELETE_ALL`): every field character is removed (an action usually
/// starts with it before placing the scene's cast).
pub const FIELD_CHARA_DELETE_ALL: u32 = 0x7100_0006;
/// `s5` (`ECT_CHARA_MOTION_SET`): `{character, motion, variant, blend ×100}`.
pub const CHARA_MOTION_SET: u32 = 0x7300_0005;
/// `u31` (`ECT_EVENT_VOICE_PLAY`): `{clip}`, a voice clip number (`docs/formats.md`, "Voices").
pub const EVENT_VOICE_PLAY: u32 = 0x7500_001f;
/// `s102` (`ECT_CHARA_VOICE_PLAY`): `{character, clip}`.
pub const CHARA_VOICE_PLAY: u32 = 0x7300_0066;
/// `u13` (`ECT_EVENT_SET_POST_EFFECT`): `{?, param, hundredths of a second, kind}`; the kinds
/// are in `docs/events.md`, "Screen effects" (3: a cross-fade from the screen as it was).
pub const EVENT_SET_POST_EFFECT: u32 = 0x7500_000d;
/// `u22` (`ECT_EVENT_CONTRAST_SET_MIX`): `{contrast id, preset, hundredths of a second}`: the
/// screen's colour overlay moves from the preset's first colour to its second over the time
/// (`docs/events.md`, "Fades").
pub const EVENT_CONTRAST_SET_MIX: u32 = 0x7500_0016;
/// `ECT_SOUND_SE_PLAY`: `{effect, a, volume %, pan ×100, d}` (`0x14005210c`): the volume and
/// pan (0 left, 64 centre, 128 right) are divided by 100; `a` and `d` go to settings not named
/// yet. The effect is a number for `0x140160070` (`barrel_audio::Library::effect`).
pub const SOUND_SE_PLAY: u32 = 0x7510_0001;
/// `s62` (`ECT_CHARA_SE_PLAY`): `{character, effect, a, volume %, pan ×100, d}` from a character
/// (18 uses, all in battle skill events, where the effect looks like another numbering: 151
/// would be the rain loop; its handler is not read).
pub const CHARA_SE_PLAY: u32 = 0x7300_003e;
/// `ECT_SOUND_BG_PLAY`: `{track, volume (percent), 0, 0}`; the track is a number in the
/// executable's music list (`barrel_formats::bgm`).
pub const SOUND_BG_PLAY: u32 = 0x7510_0003;
/// `ECT_SOUND_BG_STOP`: `{0}`.
pub const SOUND_BG_STOP: u32 = 0x7510_0004;
/// `ECT_SOUND_BG_FADEOUT`: `{hundredths of a second, 0}`.
pub const SOUND_BG_FADEOUT: u32 = 0x7510_0005;
/// `s23` (`ECT_CHARA_ROT_SET`): `{character, x, y, z}`, degrees.
pub const CHARA_ROT_SET: u32 = 0x7300_0017;
/// `s24` (`ECT_CHARA_ROT_SET_TARGET`): `{character, position (6)}`.
pub const CHARA_ROT_SET_TARGET: u32 = 0x7300_0018;
/// `s26` (`ECT_CHARA_ROT_MOVE_SET`): `{character, x, y, z, seconds ×100, curve}`.
pub const CHARA_ROT_MOVE: u32 = 0x7300_001a;
/// `s18` (`ECT_CHARA_MOVE_SET`): `{character, position (6), extra, type, ...}`.
pub const CHARA_MOVE: u32 = 0x7300_0012;
/// `w1` (0x77000001): start action N (its operand) alongside the one running. Action 0 is the
/// event's main script; it starts the others, each once, in order.
pub const START_ACTION: u32 = 0x7700_0001;
/// The old name of [`START_ACTION`], from when it was taken for a step marker.
pub const STEP: u32 = START_ACTION;

#[derive(Clone, Debug, PartialEq)]
pub enum Step<'a> {
    /// Action `action` starts here (`w1`), running alongside the action that started it; its
    /// steps follow, tagged with its number.
    StartAction { action: usize },
    /// A line of dialogue (or system text) to show.
    /// `expression` is the speaker's face from this line on and `gesture` a body gesture to
    /// play, when the line sets them (the message's fourth and third fields; -1 leaves them).
    Message { index: usize, speaker: i32, text: String, expression: Option<i32>, gesture: Option<i32> },
    /// Wait this many seconds before going on (`0x14005b310` sets the event's timer, which
    /// counts down by each frame's elapsed time).
    Wait { seconds: f32 },
    /// Wait until a character (by `Chara.xml` row) is ready (`p3`); what "ready" means is
    /// still open (`docs/events.md`, "Waits").
    WaitForCharacter { character: u32 },
    /// A character pauses its own commands (`s3`) while the event goes on.
    CharacterWait { action: usize, character: u32, seconds: f32 },
    /// A character is put at a point in field units (`s4`).
    Place { action: usize, character: u32, position: [f32; 3] },
    /// A character plays a looping motion (`s10`; a `MOTION_*` number), blending over `blend`
    /// seconds.
    /// `s10` loops it from its start (`0x1402036d0` with -1, -1 clears the play range); `s5`
    /// (`ECT_CHARA_MOTION_SET`, `{character, motion, variant, blend}`) plays it with
    /// `variant` (0 to 7, kept at the runner's +0x828; not read) and leaves the range as is.
    Motion { action: usize, character: u32, motion: u32, blend: f32, looping: bool, variant: u32 },
    /// The event moves to a field map (`q12`, `ECT_FIELDMAP_CHANGE_NOT_BGM`, `{map}`; `q2`,
    /// `ECT_FIELDMAP_CHANGE`, likewise): `map` is a `FIELDMAP_*` number ([`field_maps`]).
    FieldChange { action: usize, map: u32 },
    /// A field character is removed (`q5`, `ECT_FIELDMAP_CHARA_DELETE`, `{character}`).
    Remove { action: usize, character: u32 },
    /// Every field character is removed (`q6`).
    RemoveAll { action: usize },
    /// A voice clip plays (`u31` `{clip}`, or `s102` `{character, clip}` from a character).
    Voice { action: usize, character: Option<u32>, clip: u32 },
    /// A screen effect starts (`u13`): `kind` (`docs/events.md`, "Screen effects"), its
    /// parameter, over `seconds`.
    ScreenEffect { action: usize, kind: u32, param: i32, seconds: f32 },
    /// The screen's colour overlay starts a fade (`u22`): preset `preset` over `seconds`.
    Fade { action: usize, preset: u32, seconds: f32 },
    /// A sound effect plays (`ECT_SOUND_SE_PLAY`, or `s62` from a character) at `volume`
    /// (0 to 1) and `pan` (-1 left to 1 right).
    Effect { action: usize, character: Option<u32>, effect: u32, volume: f32, pan: f32 },
    /// The music changes: a track starts at `volume` percent, or (`track` None) stops, fading
    /// over `fade` seconds (0 at once).
    Music { action: usize, track: Option<u32>, volume: f32, fade: f32 },
    /// The event camera's eye or target is set or starts moving (`t3`, `t4`, `t6`, `t7`,
    /// `t9`; [`crate::camera`]).
    Camera { action: usize, part: crate::camera::Part, change: crate::camera::Change },
    /// The field-map camera changes (`t3/0x10` to `t20/0x10`, [`crate::field_camera`]).
    FieldCamera { action: usize, change: crate::field_camera::Change },
    /// A character's rotation is set (`s23`, `ECT_CHARA_ROT_SET`, `{character, x, y, z}` in
    /// degrees, here in radians): `rotation[1]` is the facing about the vertical axis, 0
    /// towards +z (the runner's +0x240, the same angle a move's facing writes).
    Turn { action: usize, character: u32, rotation: [f32; 3] },
    /// A character turns to a rotation over `seconds` (`s26`, `ECT_CHARA_ROT_MOVE_SET`,
    /// `{character, x, y, z (degrees), seconds ×100, curve}`; `0x1401f42e0`): each angle goes
    /// the short way round, along curve number `curve` (`0x1404e1770`; its shape is not read).
    TurnOver { action: usize, character: u32, rotation: [f32; 3], seconds: f32, curve: u32 },
    /// A character turns to face a position, on the ground (`s24`,
    /// `ECT_CHARA_ROT_SET_TARGET`; `0x1401f4f50`: yaw from the direction, 0 when the position
    /// is where the character stands).
    Face { action: usize, character: u32, toward: crate::camera::Position },
    /// A character starts moving (`s18`, `ECT_CHARA_MOVE_SET`; [`crate::camera::Move`]).
    CharacterMove { action: usize, character: u32, extra: u32, movement: crate::camera::Move },
    /// A command the player does not handle itself.
    Command { action: usize, word: u32, name: Option<&'static str>, operands: &'a [u32] },
}

pub struct Player<'a> {
    names: HashMap<u32, &'static str>,
    /// Each action's commands.
    actions: Vec<Vec<ebd::Command<'a>>>,
    /// The actions being run, innermost last: (action, next command).
    running: Vec<(usize, usize)>,
    started: Vec<bool>,
    messages: &'a [Message],
    pending: std::collections::VecDeque<Step<'a>>,
    /// How long [`Player::schedule`] holds each line: none by default, since the game waits
    /// for the player; [`Player::reading_time`] sets one for unattended playback.
    reading: Option<fn(&str) -> f32>,
}

/// A reading time for unattended playback (not the game's: it waits for a button): one
/// second plus 0.08 s a character, newlines and spaces not counted.
/// An effect's pan operand (hundredths of 0 left to 128 right) as -1 to 1 (`0x140160230`:
/// `(clamp(x, 0, 128) / 128 - 0.5) × 2`).
fn se_pan(hundredths: u32) -> f32 {
    ((hundredths as i32 as f32 / 100.0).clamp(0.0, 128.0) / 128.0 - 0.5) * 2.0
}

pub fn reading_time(text: &str) -> f32 {
    1.0 + 0.08 * text.chars().filter(|c| !c.is_whitespace()).count() as f32
}

impl<'a> Player<'a> {
    /// A player for Escha & Logy DX's events.
    pub fn escha(event: &'a Ebd, messages: &'a [Message], table: &OperandTable) -> Result<Self, Error> {
        let actions = event.actions.iter().map(|a| ebd::decode(&a.words, table)).collect::<Result<Vec<_>, _>>()?;
        let mut started = vec![false; actions.len()];
        let running = if actions.is_empty() { vec![] } else { vec![(0, 0)] };
        if let Some(s) = started.first_mut() {
            *s = true;
        }
        Ok(Player { names: crate::names::ESCHA.iter().copied().collect(), actions, running, started, messages, pending: Default::default(), reading: None })
    }

    /// Holds each line for `time(text)` seconds in [`Player::schedule`] (the action showing
    /// it waits that long), as an unattended playback would.
    pub fn reading_time(mut self, time: fn(&str) -> f32) -> Self {
        self.reading = Some(time);
        self
    }

    /// The name the executable gives a command word, if any.
    pub fn name(&self, word: u32) -> Option<&'static str> {
        self.names.get(&word).copied()
    }
}

impl<'a> Player<'a> {
    /// What command `c` of action `action` does: steps to report (in order), and an action
    /// to start, if any.
    fn translate(&self, action: usize, c: ebd::Command<'a>, out: &mut Vec<Step<'a>>) -> Option<usize> {
        let f = |v: u32| v as i32 as f32;
        match (c.word, c.operands) {
            (START_ACTION, [n, ..]) => return Some(*n as usize),
            (START_ACTION, []) => {}
            (TIME_WAIT, [hundredths, ..]) => out.push(Step::Wait { seconds: f(*hundredths) / 100.0 }),
            (TIME_WAIT, []) => out.push(Step::Wait { seconds: 0.0 }),
            (SHORT_WAIT, _) => out.push(Step::Wait { seconds: 0.3 }),
            (CHARA_WAIT, [character, ..]) => out.push(Step::WaitForCharacter { character: *character }),
            (CHARA_TIME_WAIT, [character, hundredths, ..]) => {
                out.push(Step::CharacterWait { action, character: *character, seconds: f(*hundredths) / 100.0 })
            }
            (CHARA_POS_SET, [character, _, _, _, x, y, z, ..]) => {
                out.push(Step::Place { action, character: *character, position: [f(*x), f(*y), f(*z)] })
            }
            (CHARA_MOTION_LOOP, [character, motion, rest @ ..]) => {
                let blend = rest.first().map_or(0.0, |&b| f(b) / 100.0);
                out.push(Step::Motion { action, character: *character, motion: *motion, blend, looping: true, variant: 0 });
            }
            (CHARA_MOTION_SET, [character, motion, variant, hundredths, ..]) => {
                let blend = f(*hundredths) / 100.0;
                out.push(Step::Motion { action, character: *character, motion: *motion, blend, looping: false, variant: *variant });
            }
            (FIELD_CHANGE | FIELD_CHANGE_KEEP_BGM, [map, ..]) => out.push(Step::FieldChange { action, map: *map }),
            (FIELD_CHARA_DELETE, [character, ..]) => out.push(Step::Remove { action, character: *character }),
            (FIELD_CHARA_DELETE_ALL, _) => out.push(Step::RemoveAll { action }),
            (EVENT_VOICE_PLAY, [clip, ..]) => out.push(Step::Voice { action, character: None, clip: *clip }),
            (SOUND_BG_PLAY, [track, volume, ..]) => out.push(Step::Music { action, track: Some(*track), volume: *volume as f32 / 100.0, fade: 0.0 }),
            (SOUND_BG_STOP, _) => out.push(Step::Music { action, track: None, volume: 0.0, fade: 0.0 }),
            (SOUND_BG_FADEOUT, [hundredths, ..]) => out.push(Step::Music { action, track: None, volume: 0.0, fade: *hundredths as f32 / 100.0 }),
            (EVENT_CONTRAST_SET_MIX, [_, preset, hundredths, ..]) => out.push(Step::Fade { action, preset: *preset, seconds: *hundredths as i32 as f32 / 100.0 }),
            (EVENT_SET_POST_EFFECT, [_, param, hundredths, kind, ..]) => {
                out.push(Step::ScreenEffect { action, kind: *kind, param: *param as i32, seconds: *hundredths as i32 as f32 / 100.0 })
            }
            (SOUND_SE_PLAY, [effect, _, volume, pan, ..]) => out.push(Step::Effect { action, character: None, effect: *effect, volume: *volume as f32 / 100.0, pan: se_pan(*pan) }),
            (CHARA_SE_PLAY, [character, effect, _, volume, pan, ..]) => {
                out.push(Step::Effect { action, character: Some(*character), effect: *effect, volume: *volume as f32 / 100.0, pan: se_pan(*pan) })
            }
            (CHARA_VOICE_PLAY, [character, clip, ..]) => out.push(Step::Voice { action, character: Some(*character), clip: *clip }),
            (SHOW_MESSAGES, [first, last]) => {
                for i in *first as usize..=(*last as usize) {
                    if let Some(m) = self.messages.get(i) {
                        let field = |k: usize| m.fields.get(k).copied().filter(|&v| v >= 0);
                        out.push(Step::Message { index: i, speaker: m.speaker, text: m.repaired_text(), expression: field(3), gesture: field(2) });
                    }
                }
            }
            (FIELD_CHARA_ADD, [character, _, _, _, x, y, z, _, angle, ..]) => {
                out.push(Step::Place { action, character: *character, position: [f(*x), f(*y), f(*z)] });
                out.push(Step::Turn { action, character: *character, rotation: [0.0, f(*angle).to_radians(), 0.0] });
            }
            (CHARA_ROT_SET, [character, x, y, z, ..]) => {
                let rotation = [*x, *y, *z].map(|v| (v as i32 as f32).to_radians());
                out.push(Step::Turn { action, character: *character, rotation })
            }
            (CHARA_ROT_MOVE, [character, x, y, z, hundredths, curve, ..]) => {
                let rotation = [*x, *y, *z].map(|v| (v as i32 as f32).to_radians());
                let seconds = *hundredths as i32 as f32 / 100.0;
                out.push(Step::TurnOver { action, character: *character, rotation, seconds, curve: *curve })
            }
            (CHARA_ROT_SET_TARGET, [character, rest @ ..]) if crate::camera::Position::read(rest).is_some() => {
                let (toward, _) = crate::camera::Position::read(rest).unwrap();
                out.push(Step::Face { action, character: *character, toward })
            }
            (CHARA_MOVE, [character, rest @ ..]) if crate::camera::Move::read_with_extra(rest).is_some() => {
                let (extra, movement) = crate::camera::Move::read_with_extra(rest).unwrap();
                out.push(Step::CharacterMove { action, character: *character, extra, movement })
            }
            (word, operands) if crate::field_camera::Change::read(word, operands).is_some() => {
                out.push(Step::FieldCamera { action, change: crate::field_camera::Change::read(word, operands).unwrap() })
            }
            (word, operands) if !crate::camera::changes(word, operands).is_empty() => {
                out.extend(crate::camera::changes(word, operands).into_iter().map(|(part, change)| Step::Camera { action, part, change }))
            }
            (word, operands) => out.push(Step::Command { action, word, name: self.name(word), operands }),
        }
        None
    }

    /// The event on a clock: every started action runs at once, each held only by its own
    /// waits (`p1`, `u1`). The game keeps one wait timer per event, not per action
    /// (`docs/events.md`), so overlapping waits may run differently there; not settled. Messages take no time here
    /// (the game waits for the player to read them), nor do waits for characters or the
    /// camera, whose lengths depend on what they wait for.
    pub fn schedule(mut self) -> Vec<Timed<'a>> {
        // (ready at, action, next command), in start order.
        let mut threads: Vec<(f32, usize, usize)> = vec![];
        if !self.actions.is_empty() {
            threads.push((0.0, 0, 0));
        }
        let mut out = Vec::new();
        let mut steps = Vec::new();
        while let Some(t) = (0..threads.len()).filter(|&i| threads[i].2 < self.actions[threads[i].1].len()).min_by(|&i, &j| {
            threads[i].0.partial_cmp(&threads[j].0).unwrap().then(i.cmp(&j))
        }) {
            let (now, action, pc) = threads[t];
            threads[t].2 = pc + 1;
            let c = self.actions[action][pc];
            steps.clear();
            if let Some(n) = self.translate(action, c, &mut steps) {
                if n < self.actions.len() && !self.started[n] {
                    self.started[n] = true;
                    threads.push((now, n, 0));
                    out.push(Timed { time: now, action, step: Step::StartAction { action: n } });
                }
            }
            for s in steps.drain(..) {
                if let Step::Wait { seconds } = s {
                    threads[t].0 += seconds.max(0.0);
                }
                let held = match (&s, self.reading) {
                    (Step::Message { text, .. }, Some(f)) => f(&text.replace("<CR>", "")).max(0.0),
                    _ => 0.0,
                };
                let shown_at = threads[t].0;
                threads[t].0 += held;
                out.push(Timed { time: if held > 0.0 { shown_at } else { now }, action, step: s });
            }
        }
        out
    }
}

/// A step and when it happens, in seconds from the event's start, and the action it is in.
#[derive(Clone, Debug, PartialEq)]
pub struct Timed<'a> {
    pub time: f32,
    pub action: usize,
    pub step: Step<'a>,
}

impl<'a> Iterator for Player<'a> {
    type Item = Step<'a>;

    fn next(&mut self) -> Option<Step<'a>> {
        loop {
            if let Some(s) = self.pending.pop_front() {
                return Some(s);
            }
            let &mut (action, ref mut next) = self.running.last_mut()?;
            let Some(&c) = self.actions[action].get(*next) else {
                self.running.pop();
                continue;
            };
            *next += 1;
            let mut steps = Vec::new();
            if let Some(n) = self.translate(action, c, &mut steps) {
                if n < self.actions.len() && !self.started[n] {
                    self.started[n] = true;
                    self.running.push((n, 0));
                    return Some(Step::StartAction { action: n });
                }
            }
            self.pending.extend(steps);
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn effect_pan() {
        assert_eq!(se_pan(6400), 0.0);
        assert_eq!(se_pan(0), -1.0);
        assert_eq!(se_pan(12800), 1.0);
        assert_eq!(se_pan(99999), 1.0);
        assert_eq!(se_pan(u32::MAX), -1.0);
    }

    use super::*;
    use barrel_formats::ebd::{Action, END};

    #[test]
    fn actions_start_where_the_main_script_says() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // Action 0 waits, starts 2, waits, starts 1 (and 2 again, which does nothing).
        let event = Ebd {
            actions: vec![
                act(vec![TIME_WAIT, 10, START_ACTION, 2, TIME_WAIT, 20, START_ACTION, 1, START_ACTION, 2, END]),
                act(vec![TIME_WAIT, 100, END]),
                act(vec![TIME_WAIT, 200, END]),
                act(vec![TIME_WAIT, 300, END]),
            ],
            resources: vec![],
        };
        let table = OperandTable::new(&[(START_ACTION, Some(1)), (TIME_WAIT, Some(1))]);
        let steps: Vec<String> = Player::escha(&event, &[], &table)
            .unwrap()
            .map(|s| match s {
                Step::StartAction { action } => format!("start {action}"),
                Step::Wait { seconds } => format!("wait {seconds}"),
                other => format!("{other:?}"),
            })
            .collect();
        // Action 3 is never started, so it never runs.
        assert_eq!(steps, ["wait 0.1", "start 2", "wait 2", "wait 0.2", "start 1", "wait 1"]);
    }

    #[test]
    fn actions_run_at_once_on_a_clock() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // Main: start 1, wait 1 s, mark; action 1: wait 0.5 s, mark, wait 1 s, mark.
        let mark = 0x7300_0006;
        let event = Ebd {
            actions: vec![
                act(vec![START_ACTION, 1, TIME_WAIT, 100, mark, 0, END]),
                act(vec![TIME_WAIT, 50, mark, 1, TIME_WAIT, 100, mark, 2, END]),
            ],
            resources: vec![],
        };
        let table = OperandTable::new(&[(START_ACTION, Some(1)), (TIME_WAIT, Some(1)), (mark, Some(1))]);
        let marks: Vec<(f32, u32)> = Player::escha(&event, &[], &table)
            .unwrap()
            .schedule()
            .into_iter()
            .filter_map(|t| match t.step {
                Step::Command { operands: [m], .. } => Some((t.time, *m)),
                _ => None,
            })
            .collect();
        assert_eq!(marks, [(0.5, 1), (1.0, 0), (1.5, 2)]);
    }

    #[test]
    fn messages_come_out_in_ranges() {
        let words = vec![
            STEP, 1, SHOW_MESSAGES, 0, 1, 0x7300_0006, 4, TIME_WAIT, 50, SHOW_MESSAGES, 2, 2,
            CHARA_POS_SET, 0, 0, 0, 0, 100, (-5i32) as u32, 200, CHARA_MOTION_LOOP, 0, 32, 40, 1, CHARA_TIME_WAIT, 1, 150, END,
        ];
        let event = Ebd { actions: vec![Action { flags: 0, name: vec![], description: vec![], words }], resources: vec![] };
        let msg = |t: &str| Message { kind: 1, speaker: 0, fields: vec![0; 6], after: Vec::new(), text: t.into() };
        let messages = vec![msg("a"), msg("b"), msg("c")];
        let table = OperandTable::new(&[
            (STEP, Some(1)),
            (SHOW_MESSAGES, Some(2)),
            (0x7300_0006, Some(1)),
            (TIME_WAIT, Some(1)),
            (CHARA_POS_SET, Some(7)),
            (CHARA_MOTION_LOOP, Some(4)),
            (CHARA_TIME_WAIT, Some(2)),
        ]);
        // Read unattended, each line holds its reading time: "a" at 0, "b" after it, the wait
        // and "c" after that.
        let timed = Player::escha(&event, &messages, &table).unwrap().reading_time(|_| 2.0).schedule();
        let at: Vec<(String, f32)> =
            timed.iter().filter_map(|t| match &t.step { Step::Message { text, .. } => Some((text.clone(), t.time)), _ => None }).collect();
        assert_eq!(at, [("a".into(), 0.0), ("b".into(), 2.0), ("c".into(), 4.5)]);
        let steps: Vec<Step> = Player::escha(&event, &messages, &table).unwrap().collect();
        let texts: Vec<String> = steps
            .iter()
            .map(|s| match s {
                Step::Message { text, .. } => text.clone(),
                Step::StartAction { action } => format!("start {action}"),
                Step::Command { name, .. } => name.unwrap_or("?").to_string(),
                Step::Wait { seconds } => format!("wait {seconds}"),
                Step::WaitForCharacter { character } => format!("wait for {character}"),
                Step::CharacterWait { character, seconds, .. } => format!("{character} waits {seconds}"),
                Step::Place { character, position, .. } => format!("{character} at {position:?}"),
                Step::Motion { character, motion, blend, .. } => format!("{character} motion {motion} blend {blend}"),
                Step::Camera { part, .. } => format!("camera {part:?}"),
                Step::FieldCamera { change, .. } => format!("field camera {change:?}"),
                Step::Remove { character, .. } => format!("{character} removed"),
                Step::RemoveAll { .. } => "everyone removed".to_string(),
                Step::Voice { clip, .. } => format!("voice {clip}"),
                Step::Effect { effect, .. } => format!("effect {effect}"),
                Step::ScreenEffect { kind, seconds, .. } => format!("screen effect {kind} over {seconds} s"),
                Step::Fade { preset, seconds, .. } => format!("fade {preset} over {seconds} s"),
                Step::Music { track, fade, .. } => match track {
                    Some(t) => format!("music {t}"),
                    None => format!("music stops over {fade} s"),
                },
                Step::FieldChange { map, .. } => format!("field {map}"),
                Step::CharacterMove { character, .. } => format!("{character} moves"),
                Step::Turn { character, .. } | Step::Face { character, .. } | Step::TurnOver { character, .. } => {
                    format!("{character} turns")
                }
            })
            .collect();
        assert_eq!(
            texts,
            ["a", "b", "ECT_CHARA_MOTION_WAIT", "wait 0.5", "c", "0 at [100.0, -5.0, 200.0]", "0 motion 32 blend 0.4", "1 waits 1.5"]
        );
    }
}

/// The field maps by `FIELDMAP_*` number, without the prefix: the names of the field
/// directories under `Data/x64/Field` (`TOWN_01` to `TOWN_12` are 0 to 11, then `AREA_01_01`
/// and on; 122 in Escha & Logy DX), from the executable's enum names.
pub fn field_maps(pe: &barrel_formats::pe::Pe) -> Vec<String> {
    pe.enum_names("FIELDMAP_TOWN_01")
        .map(|(names, first)| {
            names[first..].iter().take_while(|n| n.starts_with("FIELDMAP_")).map(|n| n["FIELDMAP_".len()..].to_string()).collect()
        })
        .unwrap_or_default()
}

/// The field an event plays in: the map of its first field change, if it has one.
pub fn first_field(steps: &[Timed]) -> Option<u32> {
    steps.iter().find_map(|t| match t.step {
        Step::FieldChange { map, .. } => Some(map),
        _ => None,
    })
}
