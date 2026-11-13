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
    /// Today's day number.
    pub today: i32,
}

/// The gate's word checks: word 0 = 1 needs the day flag clear, a seen event needs word 2 set
/// (repeatable), word 4 set needs event flag 1, and today must lie from words 8-10 to words 11-13
/// (`0x14035c140`, `0x14035c160`, both inclusive; all -1 in most events).
pub fn gate_open(words: &[i32; 14], g: &Gate) -> bool {
    !(words[0] == 1 && g.day_flag)
        && (!g.seen || words[2] != 0)
        && (words[4] == 0 || g.flag_1 != 0)
        && date_order(g.today, [words[8], words[9], words[10]]) >= 0
        && date_order(g.today, [words[11], words[12], words[13]]) <= 0
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
    /// its region) is 0: true exactly for the twelve town maps, 0 to 11
    /// (`barrel_formats::gimmick::TOWN_MAPS`).
    fn region_zero(&self, map: i32) -> bool;
    /// The field map the player is in (`0x1400546c0` non-null, then `0x140236160`), None outside
    /// a field.
    fn field_map(&self) -> Option<i32>;
    /// The field area the player is in (`0x140236180`: `[0x140df9968]+0x9a4`, or +0x99c when that
    /// is -1), None outside a field.
    fn field_area(&self) -> Option<i32>;
    /// A character's level (its record's +4), None when it has no record (`0x140361ad0`).
    fn level(&self, chara: i32) -> Option<i32>;
    /// The 11-slot companion list at `0x141093530` (-1 empty; the save's `party/m_squad`).
    fn squad(&self) -> &[i32];
    /// `PLAYER_PARAM` n: 0 the money (`0x14036a990`), 1 the alchemy level (`0x14107c4c8`).
    fn money(&self) -> i32;
    fn alchemy_level(&self) -> i32;
    /// A character's friendship (`0x140361ad0` then `0x14032dba0`; characters 0 to 10 are the
    /// records' indices, `0x140330120`); None when the character has no
    /// record (the condition fails) or it is not known here (not modelled).
    fn friendship(&self, chara: i32) -> Option<Option<f32>>;
    /// The count (each slot's +0x22 summed) of an item, or of any item for None, in the basket and
    /// the container with quality from `low` to `high` (the lists' finder, `[+0x28]`); None when
    /// not known here.
    fn item_count(&self, item: Option<i32>, low: f32, high: f32) -> Option<i32>;
}

/// `0x1402f08a0` and its siblings: today against a (year, month, day) pattern, -1 fields
/// ignored (`0x1402f0a00` masks today's same fields, then both sides count -1 as 1 and encode
/// as `0x1402f02d0` does). Returns today's masked number minus the pattern's.
pub fn date_order(today: i32, pattern: [i32; 3]) -> i32 {
    let d = crate::calendar::Date::of(today.max(0) as u32);
    let now = [d.year as i32, d.month as i32, d.day as i32];
    let fix = |v: i32| if v < 0 { 1 } else { v };
    let enc = |t: [i32; 3]| {
        let (y, m, dd) = (fix(t[0]), fix(t[1]), fix(t[2]));
        let y = y + i32::from(m < 4 || (m == 4 && dd < 1));
        (y * 12 + m) * 30 + dd - 391
    };
    let masked = [0, 1, 2].map(|i| if pattern[i] < 0 { -1 } else { now[i] });
    enc(masked) - enc(pattern)
}

/// Matches anything in a trigger's operands.
pub const ANY: i32 = 0x6000_0002;
/// "Not a town map" in a map trigger's operands.
pub const NOT_REGION_ZERO: i32 = 0x6000_0001;

/// What fired (`0x140214d70`'s context): the trigger group, its values (+8), the day the
/// sleep, mix or trip ends on (+0x38) and the event being checked (`0x140213670`'s third
/// argument).
#[derive(Clone, Copy, Debug)]
pub struct Fired<'a> {
    pub group: u8,
    pub values: &'a [i32],
    pub day: i32,
    pub event: i32,
}

/// One command of a group (`0x140213670`). `fired` is the trigger group that fired and its
/// values. None for the codes not read yet. `MT` and `LT` include equality.
pub fn command(c: &Command, fired: &Fired, w: &impl World) -> Option<bool> {
    if let Some(t) = c.trigger() {
        if t != fired.group {
            return Some(false);
        }
    }
    let op = |i: usize| c.operands.get(i).copied().unwrap_or(0);
    let cmp = |v: i32, kind: usize| match kind {
        0 => v >= op(1),
        1 => v <= op(1),
        _ => v == op(1),
    };
    let date = |i: usize| [op(i), op(i + 1), op(i + 2)];
    Some(match c.code {
        0x00 => w.field_map().is_some_and(|m| match op(0) {
            NOT_REGION_ZERO => !w.region_zero(m),
            ANY => true,
            v => m == v,
        }),
        0x39..=0x3b => cmp(
            match op(0) {
                0 => w.money(),
                1 => w.alchemy_level(),
                _ => i32::MIN,
            },
            c.code - 0x39,
        ),
        0x0f..=0x11 => match w.friendship(op(0))? {
            None => false,
            Some(f) => {
                let v = op(1) as f32;
                [f >= v, f <= v, f == v][c.code - 0x0f]
            }
        },
        0x07..=0x09 => {
            let item = (op(0) != ANY).then_some(op(0));
            let n = w.item_count(item, op(1) as f32, op(2) as f32)?;
            [n >= op(3), n <= op(3), n == op(3)][c.code - 0x07]
        }
        0x02 => w.field_area() == Some(op(0)),
        0x16..=0x18 => w.level(op(0)).is_some_and(|l| cmp(l, c.code - 0x16)),
        // 0x140361fd0: the character's slot number (0x140330120, -1 past 10) in the list, so a
        // number past 10 finds an empty slot.
        0x0d | 0x0e => {
            let k = if (0..=10).contains(&op(0)) { op(0) } else { -1 };
            w.squad().contains(&k) == (c.code == 0x0d)
        }
        0x3c => date_order(w.today(), date(0)) == 0,
        0x3d => date_order(w.today(), date(0)) >= 0 && date_order(w.today(), date(3)) <= 0,
        0x3e => date_order(w.today(), date(0)) <= 0,
        0x3f => date_order(w.today(), date(0)) >= 0,
        0x1f => w.seen(op(0)),
        0x20 => !w.seen(op(0)),
        0x24..=0x26 => w.seen(op(0)) && cmp(w.today() - w.seen_day(op(0)), c.code - 0x24),
        0x27..=0x29 => cmp(w.flag(op(0)), c.code - 0x27),
        0x7e | 0x7f => trigger_values(c.operands, fired.values, 4, Mode::FromTo, w),
        0x80 => trigger_values(c.operands, fired.values, 1, Mode::Region, w),
        0x91..=0x93 => date_order(fired.day, date(0)) >= 0,
        0x94..=0x96 => w.seen(op(0)) && w.seen_day(op(0)) + op(1) <= fired.day,
        0xa0 => fired.values.first() == Some(&fired.event),
        _ => match c.name() {
            "TRIGGER_MAP_COLLECT" | "TRIGGER_ITEM_GET" | "TRIGGER_MIX_SUCCESS" | "TRIGGER_MIX_FAILED"
            | "TRIGGER_EVENT_END" | "TRIGGER_OTHER_TALK_BEFORE_IS" | "TRIGGER_OTHER_TALK_AFTER_IS"
            | "TRIGGER_OTHER_PLAYER_PUSH_BUTTON" | "TRIGGER_GIMMICK_ACCESS_BEFORE"
            | "TRIGGER_GIMMICK_ACCESS_AFTER" | "TRIGGER_ENEMY_KILL_IS" | "TRIGGER_RECLAIM_TARGET_CLEAR"
            | "TRIGGER_RECLAIM_FACILITY_BUILT" | "TRIGGER_SUBJECT_CLEAR" => {
                trigger_values(c.operands, fired.values, 1, Mode::Plain, w)
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

/// A story state for following events one after another: what has been seen, when, and the
/// event flags (a new game's by default). Only what events change through their process data
/// is kept; the rest of the world reads as a new game's.
#[derive(Clone, Debug)]
pub struct Story {
    pub seen: Vec<Option<i32>>,
    pub flags: Vec<i32>,
    pub today: i32,
    pub field_map: Option<i32>,
}

impl Default for Story {
    fn default() -> Story {
        Story { seen: vec![None; 1165], flags: vec![0; 151], today: 90, field_map: None }
    }
}

impl World for Story {
    fn seen(&self, e: i32) -> bool {
        usize::try_from(e).ok().and_then(|e| self.seen.get(e)).is_some_and(Option::is_some)
    }
    fn seen_day(&self, e: i32) -> i32 {
        usize::try_from(e).ok().and_then(|e| self.seen.get(e)).copied().flatten().unwrap_or(0)
    }
    fn today(&self) -> i32 {
        self.today
    }
    fn flag(&self, f: i32) -> i32 {
        usize::try_from(f).ok().and_then(|f| self.flags.get(f)).copied().unwrap_or(0)
    }
    fn region_zero(&self, map: i32) -> bool {
        (0..12).contains(&map)
    }
    fn field_map(&self) -> Option<i32> {
        self.field_map
    }
    fn field_area(&self) -> Option<i32> {
        None
    }
    fn money(&self) -> i32 {
        0
    }
    fn alchemy_level(&self) -> i32 {
        1
    }
    fn friendship(&self, _: i32) -> Option<Option<f32>> {
        Some(None)
    }
    fn item_count(&self, _: Option<i32>, _: f32, _: f32) -> Option<i32> {
        Some(0)
    }
    fn level(&self, _: i32) -> Option<i32> {
        None
    }
    fn squad(&self) -> &[i32] {
        &[]
    }
}

impl Story {
    /// An event is played: it is marked seen today, then its process data's changes apply
    /// (seen marks `0x77120001`, flag set `0x77130004` and flag add `0x77130002`; the rest is
    /// not kept here).
    pub fn play(&mut self, event: usize, process: &[Vec<u32>]) {
        if let Some(s) = self.seen.get_mut(event) {
            *s = Some(self.today);
        }
        for group in process {
            let Ok(cmds) = barrel_formats::event_process::commands(group) else { continue };
            for (word, ops) in cmds {
                let (a, b) = (ops.first().copied().unwrap_or(0) as i32, ops.get(1).copied().unwrap_or(0) as i32);
                match word {
                    0x7712_0001 => {
                        if let Some(s) = usize::try_from(a).ok().and_then(|e| self.seen.get_mut(e)) {
                            *s = (b != 0).then_some(self.today);
                        }
                    }
                    0x7713_0004 => {
                        if let Some(f) = usize::try_from(a).ok().and_then(|f| self.flags.get_mut(f)) {
                            *f = b;
                        }
                    }
                    0x7713_0002 => {
                        if let Some(f) = usize::try_from(a).ok().and_then(|f| self.flags.get_mut(f)) {
                            *f = f.wrapping_add(b);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The event that starts when trigger group `group` fires with `values` (none if no event
    /// passes): the gate (day flag clear), group 0 and the triggering group, as [`pick`].
    pub fn next(&self, checks: &[EventCheck], lists: &HashMap<u8, Vec<Listed>>, group: u8, values: &[i32], rng: &mut Rng) -> Option<usize> {
        let list = lists.get(&group)?;
        pick(
            list,
            |event, g| {
                let check = &checks[event];
                let gate = Gate { day_flag: false, seen: self.seen(event as i32), flag_1: self.flag(1), today: self.today };
                if !gate_open(&check.words, &gate) {
                    return false;
                }
                let fired = Fired { group, values, day: self.today, event: event as i32 };
                check.groups.get(g).is_some_and(|words| {
                    commands(words).is_ok_and(|cmds| cmds.iter().all(|c| command(c, &fired, self).unwrap_or(false)))
                })
            },
            rng,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(group: u8, values: &[i32]) -> Fired<'_> {
        Fired { group, values, day: 100, event: 7 }
    }

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
        w[8..14].copy_from_slice(&[-1; 6]);
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
        fn field_map(&self) -> Option<i32> {
            Some(4)
        }
        fn money(&self) -> i32 {
            500
        }
        fn field_area(&self) -> Option<i32> {
            Some(12)
        }
        fn level(&self, c: i32) -> Option<i32> {
            (c == 1).then_some(20)
        }
        fn squad(&self) -> &[i32] {
            &[0, 1, -1]
        }
        fn alchemy_level(&self) -> i32 {
            8
        }
        fn friendship(&self, c: i32) -> Option<Option<f32>> {
            Some((c == 3).then_some(10.0))
        }
        fn item_count(&self, item: Option<i32>, low: f32, _: f32) -> Option<i32> {
            Some(if item == Some(499) && low <= 40.0 { 2 } else { 0 })
        }
    }

    #[test]
    fn player_friendship_items() {
        let g = [0x70000039, 1, 8, 0x7000003a, 0, 500, 0x7000000f, 3, 10, 0x70000007, 499, 0, 999, 1];
        assert!(commands(&g).unwrap().iter().all(|c| command(c, &f(0, &[]), &W) == Some(true)));
        let no = [0x7000000f, 4, 0, 0x70000039, 2, -5, 0x70000007, ANY, 50, 999, 1];
        assert!(commands(&no).unwrap().iter().all(|c| command(c, &f(0, &[]), &W) == Some(false)));
    }

    #[test]
    fn dates() {
        // April 1st of year 1 is day 90; March 30th of year 1 is day 449.
        assert_eq!(date_order(90, [-1, -1, -1]), 0);
        assert_eq!(date_order(90, [1, 4, 1]), 0);
        assert!(date_order(90, [1, 3, 30]) < 0);
        assert_eq!(date_order(449, [1, 3, 30]), 0);
        // Day of month only: the 15th of any month.
        assert!(date_order(90 + 20, [-1, -1, 15]) > 0);
        let mut w = [0; 14];
        w[8..14].copy_from_slice(&[1, 5, 1, 1, 6, 30]);
        assert!(!gate_open(&w, &Gate { today: 90, ..Gate::default() }));
        assert!(gate_open(&w, &Gate { today: 125, ..Gate::default() }));
    }

    #[test]
    fn conditions_and_map_triggers() {
        let group = [0x7000001f, 15, 0x70000020, 31, 0x70000027, 4, 4, 0x70000025, 15, 3];
        let cmds = commands(&group).unwrap();
        assert!(cmds.iter().all(|c| command(c, &f(0, &[]), &W) == Some(true)));
        let lt = commands(&[0x70000028, 4, 3]).unwrap();
        assert_eq!(command(&lt[0], &f(0, &[]), &W), Some(false));
        let t = commands(&[0x7000007f, 0, 4, 0, 5]).unwrap();
        assert_eq!(command(&t[0], &f(1, &[0, 4, 0, 5]), &W), Some(true));
        assert_eq!(command(&t[0], &f(1, &[0, 4, 0, 6]), &W), Some(false));
        assert_eq!(command(&t[0], &f(0, &[0, 4, 0, 5]), &W), Some(false));
        let any = commands(&[0x7000007f, 0, NOT_REGION_ZERO, 0, ANY]).unwrap();
        assert_eq!(command(&any[0], &f(1, &[0, 4, 0, 77]), &W), Some(true));
        assert_eq!(command(&any[0], &f(1, &[0, 9, 0, 77]), &W), Some(false));
        let talk = commands(&[0x7000008c, 10050]).unwrap();
        assert_eq!(command(&talk[0], &f(12, &[10050]), &W), Some(true));
        assert_eq!(command(&talk[0], &f(12, &[10051]), &W), Some(false));
        let sleep = commands(&[0x7000009f]).unwrap();
        assert_eq!(command(&sleep[0], &f(28, &[]), &W), Some(true));
        let field = commands(&[0x70000000, 4, 0x70000000, ANY]).unwrap();
        assert!(field.iter().all(|c| command(c, &f(0, &[]), &W) == Some(true)));
        let after = commands(&[0x7000003c, -1, 1, 14, 0x7000003e, -1, 1, 20]).unwrap();
        assert!(after.iter().all(|c| command(c, &f(0, &[]), &W) == Some(true)));
        let more = commands(&[0x70000002, 12, 0x70000016, 1, 20, 0x7000000d, 1, 0x7000000e, 5, 0x7000000d, 40]).unwrap();
        assert!(more.iter().all(|c| command(c, &f(0, &[]), &W) == Some(true)));
        let not = commands(&[0x70000016, 2, 1, 0x7000000d, 5]).unwrap();
        assert!(not.iter().all(|c| command(c, &f(0, &[]), &W) == Some(false)));
        let over = commands(&[0x70000091, 1, 4, 10, 0x70000094, 15, 90]).unwrap();
        assert!(over.iter().all(|c| command(c, &f(17, &[]), &W) == Some(true)));
        let exec = commands(&[0x700000a0]).unwrap();
        assert_eq!(command(&exec[0], &f(29, &[7]), &W), Some(true));
        assert_eq!(command(&exec[0], &f(29, &[8]), &W), Some(false));
        let quest = commands(&[0x70000075, 3]).unwrap();
        assert_eq!(command(&quest[0], &f(0, &[]), &W), Some(false));
    }

    #[test]
    fn a_story_follows_event_ends() {
        let event = |groups: Vec<Vec<i32>>| {
            let mut words = [0; 14];
            words[1] = 500;
            words[8..14].copy_from_slice(&[-1; 6]);
            EventCheck { words, groups }
        };
        let end = |e: i32| vec![0x7000_008b_u32 as i32, e];
        let checks = vec![
            event(vec![vec![]]),
            event(vec![vec![0x7000_001f_u32 as i32, 0], end(0)]),
            event(vec![vec![0x7000_0027_u32 as i32, 5, 2], end(1)]),
        ];
        let lists = listings(&checks);
        let mut story = Story::default();
        let mut rng = Rng::seeded(1);
        story.play(0, &[]);
        assert_eq!(story.next(&checks, &lists, 11, &[0], &mut rng), Some(1));
        // Event 2 waits for flag 5 to reach 2: event 1's process data adds 2.
        assert_eq!(story.next(&checks, &lists, 11, &[1], &mut rng), None);
        story.play(1, &[vec![0x7713_0002, 5, 2, 0x7f00_0000]]);
        assert_eq!(story.flag(5), 2);
        assert_eq!(story.next(&checks, &lists, 11, &[1], &mut rng), Some(2));
    }

}
