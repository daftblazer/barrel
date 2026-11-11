//! The event player: runs an event's bytecode and says what happens, step by step.
//!
//! This first version runs the actions one after another in file order and turns each
//! command into a [`Step`]. It knows `u18` (0x75000012), which shows the event's messages
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
/// `w1`: a step marker.
pub const STEP: u32 = 0x7700_0001;

#[derive(Clone, Debug, PartialEq)]
pub enum Step<'a> {
    /// A line of dialogue (or system text) to show.
    Message { index: usize, speaker: i32, text: String },
    /// Wait this many seconds before going on (`0x14005b310` sets the event's timer, which
    /// counts down by each frame's elapsed time).
    Wait { seconds: f32 },
    /// A command the player does not handle itself.
    Command { action: usize, word: u32, name: Option<&'static str>, operands: &'a [u32] },
}

pub struct Player<'a> {
    names: HashMap<u32, &'static str>,
    commands: Vec<(usize, ebd::Command<'a>)>,
    messages: &'a [Message],
    next: usize,
    pending: std::collections::VecDeque<Step<'a>>,
}

impl<'a> Player<'a> {
    /// A player for Escha & Logy DX's events.
    pub fn escha(event: &'a Ebd, messages: &'a [Message], table: &OperandTable) -> Result<Self, Error> {
        let mut commands = Vec::new();
        for (i, a) in event.actions.iter().enumerate() {
            for c in ebd::decode(&a.words, table)? {
                commands.push((i, c));
            }
        }
        Ok(Player {
            names: crate::names::ESCHA.iter().copied().collect(),
            commands,
            messages,
            next: 0,
            pending: Default::default(),
        })
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
            let (action, c) = *self.commands.get(self.next)?;
            self.next += 1;
            match (c.word, c.operands) {
                (STEP, _) => continue,
                (TIME_WAIT, [hundredths, ..]) => return Some(Step::Wait { seconds: *hundredths as i32 as f32 / 100.0 }),
                (TIME_WAIT, []) => return Some(Step::Wait { seconds: 0.0 }),
                (SHORT_WAIT, _) => return Some(Step::Wait { seconds: 0.3 }),
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
    fn messages_come_out_in_ranges() {
        let words = vec![STEP, 1, SHOW_MESSAGES, 0, 1, 0x7300_0006, 4, TIME_WAIT, 50, SHOW_MESSAGES, 2, 2, END];
        let event = Ebd { actions: vec![Action { flags: 0, name: vec![], description: vec![], words }], resources: vec![] };
        let msg = |t: &str| Message { kind: 1, speaker: 0, fields: vec![0; 6], after: Vec::new(), text: t.into() };
        let messages = vec![msg("a"), msg("b"), msg("c")];
        let table = OperandTable::new(&[(STEP, Some(1)), (SHOW_MESSAGES, Some(2)), (0x7300_0006, Some(1)), (TIME_WAIT, Some(1))]);
        let steps: Vec<Step> = Player::escha(&event, &messages, &table).unwrap().collect();
        let texts: Vec<String> = steps
            .iter()
            .map(|s| match s {
                Step::Message { text, .. } => text.clone(),
                Step::Command { name, .. } => name.unwrap_or("?").to_string(),
                Step::Wait { seconds } => format!("wait {seconds}"),
            })
            .collect();
        assert_eq!(texts, ["a", "b", "ECT_CHARA_MOTION_WAIT", "wait 0.5", "c"]);
    }
}
