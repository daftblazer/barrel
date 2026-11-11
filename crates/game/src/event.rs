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
    Message { index: usize, speaker: i32, text: String },
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
    Motion { action: usize, character: u32, motion: u32, blend: f32 },
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
        Ok(Player { names: crate::names::ESCHA.iter().copied().collect(), actions, running, started, messages, pending: Default::default() })
    }

    /// The name the executable gives a command word, if any.
    pub fn name(&self, word: u32) -> Option<&'static str> {
        self.names.get(&word).copied()
    }
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
            match (c.word, c.operands) {
                (START_ACTION, [n, ..]) => {
                    let n = *n as usize;
                    if n < self.actions.len() && !self.started[n] {
                        self.started[n] = true;
                        self.running.push((n, 0));
                        return Some(Step::StartAction { action: n });
                    }
                }
                (START_ACTION, []) => {}
                (TIME_WAIT, [hundredths, ..]) => return Some(Step::Wait { seconds: *hundredths as i32 as f32 / 100.0 }),
                (TIME_WAIT, []) => return Some(Step::Wait { seconds: 0.0 }),
                (SHORT_WAIT, _) => return Some(Step::Wait { seconds: 0.3 }),
                (CHARA_WAIT, [character, ..]) => return Some(Step::WaitForCharacter { character: *character }),
                (CHARA_TIME_WAIT, [character, hundredths, ..]) => {
                    return Some(Step::CharacterWait { action, character: *character, seconds: *hundredths as i32 as f32 / 100.0 })
                }
                (CHARA_POS_SET, [character, _, _, _, x, y, z, ..]) => {
                    let f = |v: u32| v as i32 as f32;
                    return Some(Step::Place { action, character: *character, position: [f(*x), f(*y), f(*z)] });
                }
                (CHARA_MOTION_LOOP, [character, motion, rest @ ..]) => {
                    let blend = rest.first().map_or(0.0, |&b| b as i32 as f32 / 100.0);
                    return Some(Step::Motion { action, character: *character, motion: *motion, blend });
                }
                (SHOW_MESSAGES, [first, last]) => {
                    for i in *first as usize..=(*last as usize) {
                        if let Some(m) = self.messages.get(i) {
                            self.pending.push_back(Step::Message { index: i, speaker: m.speaker, text: m.repaired_text() });
                        }
                    }
                }
                (word, operands) => {
                    return Some(Step::Command { action, word, name: self.name(word), operands });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
            })
            .collect();
        assert_eq!(
            texts,
            ["a", "b", "ECT_CHARA_MOTION_WAIT", "wait 0.5", "c", "0 at [100.0, -5.0, 200.0]", "0 motion 32 blend 0.4", "1 waits 1.5"]
        );
    }
}
