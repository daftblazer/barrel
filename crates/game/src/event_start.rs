//! Which event starts when something happens (`0x140214d70`, called from 31 places with a trigger
//! group): the events listed under that group, highest priority first, whose gate, group 0 and
//! triggering group all pass; of those at the first passing priority, one at random.

use std::collections::HashMap;

use barrel_formats::event_check::{commands, Command, EventCheck};

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

/// The game state the modelled conditions read.
pub trait World {
    /// The event has been seen (`0x14035f270`).
    fn seen(&self, event: i32) -> bool;
    /// The day an event was seen (`0x14035f3a0`).
    fn seen_day(&self, event: i32) -> i32;
    /// Today (`0x14035c040`).
    fn today(&self) -> i32;
    /// An event flag (`0x14035f460`).
    fn flag(&self, flag: i32) -> i32;
    /// `0x140236070`: the first word of the map's runtime record (`[0x140df9a08]+0x10e8+0x40×map`,
    /// the region for a field area) is 0.
    fn region_zero(&self, map: i32) -> bool;
}

/// Matches anything in a trigger's operands.
pub const ANY: i32 = 0x6000_0002;
/// "Not a region-0 map" in a map trigger's operands.
pub const NOT_REGION_ZERO: i32 = 0x6000_0001;

/// One command of a group (`0x140213670`). `fired` is the trigger group that fired and its
/// values. None for the codes not read yet. `MT` and `LT` include equality.
pub fn command(c: &Command, fired: (u8, &[i32]), w: &impl World) -> Option<bool> {
    if let Some(t) = c.trigger() {
        if t != fired.0 {
            return Some(false);
        }
    }
    let op = |i: usize| c.operands.get(i).copied().unwrap_or(0);
    let cmp = |v: i32, kind: usize| match kind {
        0 => v >= op(1),
        1 => v <= op(1),
        _ => v == op(1),
    };
    Some(match c.code {
        0x1f => w.seen(op(0)),
        0x20 => !w.seen(op(0)),
        0x24..=0x26 => w.seen(op(0)) && cmp(w.today() - w.seen_day(op(0)), c.code - 0x24),
        0x27..=0x29 => cmp(w.flag(op(0)), c.code - 0x27),
        0x7e | 0x7f => trigger_values(c.operands, fired.1, 4, Mode::FromTo, w),
        0x80 => trigger_values(c.operands, fired.1, 1, Mode::Region, w),
        _ => match c.name() {
            "TRIGGER_MAP_COLLECT" | "TRIGGER_ITEM_GET" | "TRIGGER_MIX_SUCCESS" | "TRIGGER_MIX_FAILED"
            | "TRIGGER_EVENT_END" | "TRIGGER_OTHER_TALK_BEFORE_IS" | "TRIGGER_OTHER_TALK_AFTER_IS"
            | "TRIGGER_OTHER_PLAYER_PUSH_BUTTON" | "TRIGGER_GIMMICK_ACCESS_BEFORE"
            | "TRIGGER_GIMMICK_ACCESS_AFTER" | "TRIGGER_ENEMY_KILL_IS" | "TRIGGER_RECLAIM_TARGET_CLEAR"
            | "TRIGGER_RECLAIM_FACILITY_BUILT" | "TRIGGER_SUBJECT_CLEAR" => {
                trigger_values(c.operands, fired.1, 1, Mode::Plain, w)
            }
            "TRIGGER_MIX_HP_EMPTY" | "TRIGGER_MIX_ACCESS" | "TRIGGER_BATTLE_ANNIHILATION"
            | "TRIGGER_BATTLE_ANNIHILATION_ATELIER" | "TRIGGER_RECLAIM_KINGDOM_LV_UP"
            | "TRIGGER_ATELIER_RETURN" | "TRIGGER_ATELIER_SLEEP_MIX" | "TRIGGER_MAP_WORLDMAP_ROUTE_MOVE"
            | "TRIGGER_MEMORIES_LV_UP" => true,
            // The codes the switch sends to its default (license, reclaim, DLC, quests ...):
            // always false, and the check data never uses them.
            n if DEFAULT_FALSE.contains(&n) => false,
            _ => return None,
        },
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Plain,
    FromTo,
    Region,
}

const DEFAULT_FALSE: &[&str] = &[
    "CONDITION_CHIMU_IS_COLLECTTING", "CONDITION_CHIMU_IS_NOT_COLLECTTING",
    "CONDITION_MAP_TRAVERSAL_RITU_MT", "CONDITION_MAP_TRAVERSAL_RITU_LT", "CONDITION_MAP_TRAVERSAL_RITU_EQ",
    "CONDITION_LICENSE_RANK_MT", "CONDITION_LICENSE_RANK_LT", "CONDITION_LICENSE_RANK_EQ",
    "CONDITION_LICENSE_RANK_UP_OK", "CONDITION_QUEST_ACCEPT_NUM_MT", "CONDITION_QUEST_ACCEPT_NUM_LT",
    "CONDITION_QUEST_ACCEPT_NUM_EQ", "CONDITION_RECLAIM_BONUS_ENTRY_MT", "CONDITION_RECLAIM_BONUS_ENTRY_LT",
    "CONDITION_RECLAIM_BONUS_ENTRY_EQ", "CONDITION_RECLAIM_TARGET_CLEAR", "CONDITION_RECLAIM_TARGET_NOT_CLEAR",
    "CONDITION_RECLAIM_FACILITY_BUILT", "CONDITION_RECLAIM_FACILITY_NOT_BUILT",
    "CONDITION_RECLAIM_POPULARITY_MT", "CONDITION_RECLAIM_POPULARITY_LT", "CONDITION_RECLAIM_POPULARITY_EQ",
    "CONDITION_RECLAIM_KINGDOM_LV_MT", "CONDITION_RECLAIM_KINGDOM_LV_LT", "CONDITION_RECLAIM_KINGDOM_LV_EQ",
    "CONDITION_RECLAIM_KINGDOM_LV_UP_OK", "CONDITION_RECLAIM_SEED_UNKNOWN", "CONDITION_RECLAIM_SEED_FIND",
    "CONDITION_RECLAIM_SEED_OPEN", "CONDITION_RECLAIM_POPULATION_MT", "CONDITION_RECLAIM_POPULATION_LT",
    "CONDITION_RECLAIM_POPULATION_EQ", "CONDITION_DLC_NO", "CONDITION_DLC_PARTY_MARION",
    "CONDITION_DLC_PARTY_ODILEIA", "CONDITION_DLC_EXTRA_MAP_00", "CONDITION_DLC_EXTRA_MAP_01",
    "CONDITION_CHARA_WMPOINTER_BE_WITH_PLAYER", "CONDITION_QUEST_OCCURRENCE",
    "CONDITION_QUEST_NOT_OCCURRENCE", "TRIGGER_ATELIER_ITEM_RETURN",
];

/// `0x140213540`: the first `count` operands against the fired values. `ANY` matches anything.
/// `NOT_REGION_ZERO` matches a map that is not region 0. For the from-to triggers it applies only
/// at operands 1 and 3 after a kind operand of 0; in the plain triggers it is an ordinary value.
fn trigger_values(ops: &[i32], fired: &[i32], count: usize, mode: Mode, w: &impl World) -> bool {
    if ops.len() < count || fired.len() < count {
        return false;
    }
    (0..count).all(|i| match ops[i] {
        ANY => true,
        NOT_REGION_ZERO if mode == Mode::FromTo => {
            (i == 1 || i == 3) && ops[i - 1] == 0 && !w.region_zero(fired[i])
        }
        NOT_REGION_ZERO if mode == Mode::Region => !w.region_zero(fired[i]),
        v => fired[i] == v,
    })
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

    struct W;
    impl World for W {
        fn seen(&self, e: i32) -> bool {
            e == 15
        }
        fn seen_day(&self, _: i32) -> i32 {
            10
        }
        fn today(&self) -> i32 {
            13
        }
        fn flag(&self, f: i32) -> i32 {
            f
        }
        fn region_zero(&self, map: i32) -> bool {
            map == 9
        }
    }

    #[test]
    fn conditions_and_map_triggers() {
        let group = [0x7000001f, 15, 0x70000020, 31, 0x70000027, 4, 4, 0x70000025, 15, 3];
        let cmds = commands(&group).unwrap();
        assert!(cmds.iter().all(|c| command(c, (0, &[]), &W) == Some(true)));
        let lt = commands(&[0x70000028, 4, 3]).unwrap();
        assert_eq!(command(&lt[0], (0, &[]), &W), Some(false));
        let t = commands(&[0x7000007f, 0, 4, 0, 5]).unwrap();
        assert_eq!(command(&t[0], (1, &[0, 4, 0, 5]), &W), Some(true));
        assert_eq!(command(&t[0], (1, &[0, 4, 0, 6]), &W), Some(false));
        assert_eq!(command(&t[0], (0, &[0, 4, 0, 5]), &W), Some(false));
        let any = commands(&[0x7000007f, 0, NOT_REGION_ZERO, 0, ANY]).unwrap();
        assert_eq!(command(&any[0], (1, &[0, 4, 0, 77]), &W), Some(true));
        assert_eq!(command(&any[0], (1, &[0, 9, 0, 77]), &W), Some(false));
        let talk = commands(&[0x7000008c, 10050]).unwrap();
        assert_eq!(command(&talk[0], (12, &[10050]), &W), Some(true));
        assert_eq!(command(&talk[0], (12, &[10051]), &W), Some(false));
        let sleep = commands(&[0x7000009f]).unwrap();
        assert_eq!(command(&sleep[0], (28, &[]), &W), Some(true));
        let quest = commands(&[0x70000075, 3]).unwrap();
        assert_eq!(command(&quest[0], (0, &[]), &W), Some(false));
    }
}
