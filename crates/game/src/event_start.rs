//! Which event starts when something happens (`0x140214d70`, called from 31 places with a trigger
//! group): the events listed under that group, highest priority first, whose gate, group 0 and
//! triggering group all pass; of those at the first passing priority, one at random.

use std::collections::HashMap;

use barrel_formats::event_check::{commands, EventCheck};

use crate::rng::Rng;

/// One event under a trigger group (`0x1400555d0`): the event, the group holding the trigger
/// and the event's priority (word 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listed {
    pub event: usize,
    pub group: usize,
    pub priority: i32,
}

/// The events under each trigger group, highest priority first. A group goes under the trigger
/// group of its first trigger. The game sorts with its library sort, which does not keep the
/// file order among equal priorities; this keeps it (the pick among equals can differ).
pub fn listings(checks: &[EventCheck]) -> HashMap<u8, Vec<Listed>> {
    let mut out: HashMap<u8, Vec<Listed>> = HashMap::new();
    for (event, check) in checks.iter().enumerate() {
        for (group, words) in check.groups.iter().enumerate() {
            let Ok(cmds) = commands(words) else { continue };
            if let Some(t) = cmds.iter().find_map(|c| c.trigger()) {
                out.entry(t).or_default().push(Listed { event, group, priority: check.words[1] });
            }
        }
    }
    for list in out.values_mut() {
        list.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
    out
}

/// What an event's gate (`0x140214c00`, before its groups) looks at.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gate {
    /// `0x1410782e2`, cleared when the day ends.
    pub day_flag: bool,
    /// The event has been seen (`0x14035f270`).
    pub seen: bool,
    /// Event flag 1 (`0x14035f460(1)`).
    pub flag_1: i32,
}

/// The gate's word checks: word 0 = 1 needs the day flag clear, a seen event needs word 2 set
/// (repeatable), word 4 set needs event flag 1. Words 8-10 and 11-13 are then a date range
/// (`0x14035c140`/`0x14035c160` against the date), not modelled here.
pub fn gate_open(words: &[i32; 14], g: &Gate) -> bool {
    !(words[0] == 1 && g.day_flag) && (!g.seen || words[2] != 0) && (words[4] == 0 || g.flag_1 != 0)
}

/// Picks the event to start from a trigger group's list. `passes(event, group)` is the full
/// check (gate, then every command of group 0 and of `group`, all needed). One generator number
/// is drawn when anything passes.
pub fn pick(list: &[Listed], mut passes: impl FnMut(usize, usize) -> bool, rng: &mut Rng) -> Option<usize> {
    let mut found: Vec<Listed> = Vec::new();
    for l in list {
        if !(passes(l.event, 0) && passes(l.event, l.group)) {
            continue;
        }
        if found.first().is_some_and(|f| f.priority != l.priority) || found.iter().any(|f| f.event == l.event) {
            continue;
        }
        found.push(*l);
    }
    if found.is_empty() {
        return None;
    }
    let n = rng.next_31() as usize % found.len();
    Some(found[n].event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(priority: i32, groups: Vec<Vec<i32>>) -> EventCheck {
        let mut words = [0; 14];
        words[1] = priority;
        EventCheck { words, groups }
    }

    #[test]
    fn lists_by_priority_and_picks_the_top() {
        let talk = 0x7000008c; // TRIGGER_OTHER_TALK_BEFORE_IS, group 12
        let checks = vec![
            check(10, vec![vec![], vec![talk, 5]]),
            check(500, vec![vec![], vec![talk, 5]]),
            check(500, vec![vec![], vec![talk, 6]]),
        ];
        let l = &listings(&checks)[&12];
        assert_eq!(l.iter().map(|l| l.event).collect::<Vec<_>>(), [1, 2, 0]);
        let mut rng = Rng::seeded(1);
        // Event 1 fails: only event 2 is at the top passing priority, event 0 is lower.
        assert_eq!(pick(l, |e, _| e != 1, &mut rng), Some(2));
        assert_eq!(pick(l, |e, _| e == 0, &mut rng), Some(0));
        assert_eq!(pick(l, |_, _| false, &mut rng), None);
    }

    #[test]
    fn gate() {
        let mut w = [0; 14];
        assert!(gate_open(&w, &Gate::default()));
        assert!(!gate_open(&w, &Gate { seen: true, ..Gate::default() }));
        w[2] = 1;
        assert!(gate_open(&w, &Gate { seen: true, ..Gate::default() }));
        w[0] = 1;
        assert!(!gate_open(&w, &Gate { day_flag: true, ..Gate::default() }));
        w[4] = 1;
        assert!(!gate_open(&w, &Gate::default()));
    }
}
