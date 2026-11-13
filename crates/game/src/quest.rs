//! Requests, from `Saves/quest/QuestAlc.xml` (synthesis), `QuestBtl.xml` (monsters) and
//! `QuestGat.xml` (gathering), as the executable's schema types them (`docs/tables.md`,
//! "Requests"): the target, the points, two prizes, the days and events that bound it.
//!
//! The points and the prize counts are floats in the tables (5.12, 1.7067); a posted request
//! carries them rolled into whole numbers ([`rewards`]).

use barrel_formats::table::Element;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Synthesis,
    Monster,
    Gathering,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub kind: Kind,
    /// The item to make or bring, or the monster to beat.
    pub target: String,
    pub points: f32,
    pub prizes: Vec<(String, f32)>,
    pub from_event: Option<String>,
    pub until_event: Option<String>,
    /// The days it is offered, from and to (game days).
    pub from_day: i32,
    pub to_day: i32,
    pub priority: f32,
    pub table: i32,
    pub num: i32,
    /// `isClearSubject`: the assignment that must be cleared first.
    pub clear_subject: Option<String>,
}

/// The requests of one quest table.
pub fn requests(table: &Element, kind: Kind) -> Vec<Request> {
    let f = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
    let i = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
    let opt = |e: &Element, k: &str| e.attr(k).filter(|v| !v.is_empty()).map(str::to_string);
    table
        .children
        .iter()
        .filter(|e| e.attr("TargetTag").is_some())
        .map(|e| Request {
            kind,
            target: e.attr("TargetTag").unwrap_or("").to_string(),
            points: f(e, "GetPoint"),
            prizes: (1..=2)
                .filter_map(|n| opt(e, &format!("PrizeItemTag{n}")).map(|t| (t, f(e, &format!("PrizeItemNum{n}")))))
                .collect(),
            from_event: opt(e, "StartEvent"),
            until_event: opt(e, "EndEvent"),
            from_day: i(e, "StartDate"),
            to_day: i(e, "EndDate"),
            priority: f(e, "Priority"),
            table: i(e, "Table"),
            // QuestAlc spells it "num", the others "Num".
            num: e.attr("Num").or(e.attr("num")).and_then(|v| v.trim().parse().ok()).unwrap_or(0),
            clear_subject: opt(e, "isClearSubject"),
        })
        .collect()
}

/// How many a posted request asks for (the slot's count, `0x14033ab30`): its `Num`, at least 1;
/// a monster request no more than half of `seen` (the count the game asks the monster table for,
/// `0x1410bcaf0`'s slot `+0x70`, not read) and 10.
pub fn wanted(r: &Request, seen: i32) -> i32 {
    let n = match r.kind {
        Kind::Monster => r.num.min((seen / 2).min(10)),
        _ => r.num,
    };
    n.max(1)
}

/// Days from `today` to the end of the current assignment (`0x14035c070`): the last day of
/// period `assignment` (0 to 8), or April 1st of school year 5 after the ninth.
pub fn days_to_deadline(assignment: u32, today: u32) -> i32 {
    let end = if assignment < crate::calendar::PERIODS {
        crate::calendar::period_days(assignment).1
    } else {
        crate::calendar::Date { year: 5, month: 4, day: 1 }.number()
    };
    end as i32 - today as i32
}

/// A request's points and prize counts as posted for `n` wanted (`0x14033ab30`, each truncated):
/// the points and the first prize `GetPoint` (or its count) × n × m + 1 + a, the second prize
/// count × n + a, where (m, a) is (1.5, 1) when the assignment's end is 1 to 30 days away
/// ([`days_to_deadline`]) and (1, 0) otherwise. So 5.12 points for one item post as 6.
pub fn rewards(r: &Request, n: i32, days_left: i32) -> (i32, Vec<(String, i32)>) {
    let (m, a) = if (1..=30).contains(&days_left) { (1.5f32, 1.0f32) } else { (1.0, 0.0) };
    let n = n as f32;
    let points = (m * r.points * n + 1.0 + a) as i32;
    let prizes = r
        .prizes
        .iter()
        .enumerate()
        .map(|(k, (t, c))| (t.clone(), if k == 0 { (m * c * n + 1.0 + a) as i32 } else { (c * n + a) as i32 }))
        .collect();
    (points, prizes)
}

/// Whether a request is offered on `day`, given which events have been seen.
pub fn offered(r: &Request, day: i32, seen: impl Fn(&str) -> bool) -> bool {
    (r.from_day..=r.to_day).contains(&day)
        && r.from_event.as_deref().is_none_or(&seen)
        && !r.until_event.as_deref().is_some_and(&seen)
}

/// Whether a request may go on the board on `day` (`0x14033b450`, English Escha executable):
/// its start event seen and its end event not (each when it has one); its `isClearSubject`
/// assignment cleared (the assignment's state, `0x141099e10` + 0x14 × its number, at least 7);
/// and, when either date is above 0, `StartDate <= day` and `day <= EndDate` (an end below 0
/// meaning none). Unlike [`offered`], a request whose dates are both 0 or less is not bounded
/// by them.
pub fn board_eligible(r: &Request, day: i32, seen: impl Fn(&str) -> bool, cleared: impl Fn(&str) -> bool) -> bool {
    if r.from_event.as_deref().is_some_and(|e| !seen(e)) || r.until_event.as_deref().is_some_and(&seen) {
        return false;
    }
    if r.clear_subject.as_deref().is_some_and(|s| !cleared(s)) {
        return false;
    }
    if r.from_day > 0 || r.to_day > 0 {
        return r.from_day <= day && (r.to_day < 0 || day <= r.to_day);
    }
    true
}

/// One kind's slots of a redraw: slot k gathers, in table order, the requests of `kind` whose
/// `Table` is k (or -1), that are [`board_eligible`] and pass `known` (the game's further tests,
/// not modelled here), sorts them by `key` from highest to lowest when given (stably), and draws
/// one by [`board_pick`]. Returns the chosen requests' indices.
#[allow(clippy::too_many_arguments)]
fn draw_slots<const N: usize>(
    requests: &[Request],
    kind: Kind,
    day: i32,
    seen: &dyn Fn(&str) -> bool,
    cleared: &dyn Fn(&str) -> bool,
    known: &dyn Fn(&Request) -> bool,
    key: Option<&dyn Fn(&Request) -> i32>,
    distinct: bool,
    rng: &mut BoardRng,
) -> [Option<usize>; N] {
    let mut out: [Option<usize>; N] = [None; N];
    for k in 0..N {
        let mut candidates: Vec<usize> = (0..requests.len())
            .filter(|&i| {
                let r = &requests[i];
                let taken = distinct && out[..k].iter().flatten().any(|&j| requests[j].target == r.target);
                r.kind == kind && (r.table == k as i32 || r.table == -1) && !taken && board_eligible(r, day, seen, cleared) && known(r)
            })
            .collect();
        if let Some(key) = key {
            candidates.sort_by_key(|&i| std::cmp::Reverse(key(&requests[i])));
        }
        let chances: Vec<f32> = candidates.iter().map(|&i| requests[i].priority).collect();
        out[k] = board_pick(&chances, rng).map(|j| candidates[j]);
    }
    out
}

/// The three synthesis slots (`0x14033b5e0`, board places 0 to 2, `Table` 0 to 2). `known`
/// stands for the game's tests of the target: a count of at least 6 in `0x141079fe0` and its
/// recipe flagged (`0x1400e31a0`, `0x141079fe0` + 0x8c0).
pub fn draw_synthesis(requests: &[Request], day: i32, seen: impl Fn(&str) -> bool, cleared: impl Fn(&str) -> bool, known: impl Fn(&Request) -> bool, rng: &mut BoardRng) -> [Option<usize>; 3] {
    draw_slots(requests, Kind::Synthesis, day, &seen, &cleared, &known, None, false, rng)
}

/// The four battle slots (`0x14033b990`, board places 3 to 6, `Table` 0 to 3; nothing has
/// `Table` 3, so the fourth takes only -1). The candidates come from the manager's second list
/// (+0x18), need a count of at least 2 for the monster in `0x141079fe0` (`known`), and are sorted
/// by `level` (`0x14035cc80` of the target, the monster's level presumably; comparator
/// `0x14033c1d0`) from highest to lowest before the draw.
pub fn draw_battle(
    requests: &[Request],
    day: i32,
    seen: impl Fn(&str) -> bool,
    cleared: impl Fn(&str) -> bool,
    known: impl Fn(&Request) -> bool,
    level: impl Fn(&Request) -> i32,
    rng: &mut BoardRng,
) -> [Option<usize>; 4] {
    draw_slots(requests, Kind::Monster, day, &seen, &cleared, &known, Some(&level), false, rng)
}

/// The three gathering slots (`0x14033bd10`, board places 7 to 9, `Table` 0 to 2; the rows with
/// `Table` 3 come up only through -1, which none has). The candidates come from the manager's
/// third list (+0x30); a target already on an earlier gathering slot is skipped; no count is
/// asked for.
pub fn draw_gathering(requests: &[Request], day: i32, seen: impl Fn(&str) -> bool, cleared: impl Fn(&str) -> bool, rng: &mut BoardRng) -> [Option<usize>; 3] {
    draw_slots(requests, Kind::Gathering, day, &seen, &cleared, &|_| true, None, true, rng)
}

/// A whole redraw on `day` (`0x14033ab20`): the generator seeded from the day plus `global`
/// (the global at `0x1410989f0`), then the three synthesis, four battle and three gathering
/// slots in that order, all from one list of requests.
#[allow(clippy::too_many_arguments)]
pub fn draw_board(
    requests: &[Request],
    day: i32,
    global: i32,
    seen: impl Fn(&str) -> bool,
    cleared: impl Fn(&str) -> bool,
    known: impl Fn(&Request) -> bool,
    level: impl Fn(&Request) -> i32,
) -> [Option<usize>; 10] {
    let mut rng = BoardRng::new(crate::calendar::board_seed(day.max(0) as u32, global));
    let a = draw_synthesis(requests, day, &seen, &cleared, &known, &mut rng);
    let b = draw_battle(requests, day, &seen, &cleared, &known, &level, &mut rng);
    let c = draw_gathering(requests, day, &seen, &cleared, &mut rng);
    let mut out = [None; 10];
    out[..3].copy_from_slice(&a);
    out[3..7].copy_from_slice(&b);
    out[7..].copy_from_slice(&c);
    out
}

/// The request board's generator (the object at `0x140dfc880`, English Escha executable): two
/// words, stepped like the battle's Xorshift but on two: `t = x ^ (x << 11)`, `x = y`,
/// `y = y ^ (y >> 19) ^ (t >> 8) ^ t` with the shift by 19 arithmetic and by 8 logical; a
/// number is the new `y`'s low 31 bits. Seeded on each redraw by [`crate::calendar::board_seed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardRng {
    pub state: [i32; 2],
}

impl BoardRng {
    pub fn new(state: [i32; 2]) -> BoardRng {
        BoardRng { state }
    }

    pub fn next(&mut self) -> i32 {
        let [x, y] = self.state;
        let t = x ^ (x << 11);
        let y2 = y ^ (y >> 19) ^ (((t as u32) >> 8) as i32) ^ t;
        self.state = [y, y2];
        y2 & 0x7fff_ffff
    }

    /// A number below `n` (`0x14033c210`).
    pub fn below(&mut self, n: i32) -> i32 {
        self.next() % n.max(1)
    }
}

/// Picks one of `chances` (each candidate request's `Priority`, the float at its +0x38) the way
/// the board does (`0x14033b836`): in order, each draws a number below 100 and is taken when it
/// is under `trunc(chance + 0.5)`; when none is, the candidates are shuffled (for each place
/// k from 1, swapped with the one at `below(k + 1)`, the standard library's `random_shuffle`)
/// and the first is taken. Returns the candidate's index; `None` when there are none.
pub fn board_pick(chances: &[f32], rng: &mut BoardRng) -> Option<usize> {
    if chances.is_empty() {
        return None;
    }
    for (i, &c) in chances.iter().enumerate() {
        if rng.next() % 100 < (c + 0.5) as i32 {
            return Some(i);
        }
    }
    let mut order: Vec<usize> = (0..chances.len()).collect();
    for k in 1..order.len() {
        let j = rng.below(k as i32 + 1) as usize;
        order.swap(k, j);
    }
    Some(order[0])
}

/// The requests a save's board slots hold (`barrel_formats::save::BoardSlot`), as indices into
/// `requests`: each slot's target named through `items` (the `ITEM_*` names, item rows) or
/// `monsters` (the names from the first `MONSTER_` on), then the first request of that kind,
/// target and prize items. The slot keeps only rolled values, so requests that differ in
/// nothing else (two Craft requests differ in a prize count and a condition) fall on the first.
pub fn board_from_save(slots: &[Option<barrel_formats::save::BoardSlot>], requests: &[Request], items: &[&str], monsters: &[&str]) -> [Option<usize>; 10] {
    let mut board = [None; 10];
    for (b, s) in board.iter_mut().zip(slots) {
        let Some(s) = s else { continue };
        let kind = match s.kind {
            0 => Kind::Synthesis,
            1 => Kind::Monster,
            _ => Kind::Gathering,
        };
        let name = |n: i32| usize::try_from(n).ok();
        let target = if kind == Kind::Monster { name(s.target).and_then(|n| monsters.get(n)) } else { name(s.target).and_then(|n| items.get(n)) };
        let Some(&target) = target else { continue };
        let prize = |k: usize| name(s.prizes[k].0).and_then(|n| items.get(n)).copied();
        let same_prizes = |r: &Request| (0..2).all(|k| r.prizes.get(k).map(|p| p.0.as_str()) == prize(k));
        *b = requests
            .iter()
            .position(|r| r.kind == kind && r.target == target && same_prizes(r))
            .or_else(|| requests.iter().position(|r| r.kind == kind && r.target == target));
    }
    board
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_board_finds_its_requests() {
        use barrel_formats::save::BoardSlot;
        let req = |kind, target: &str, prize: &str| Request { kind, target: target.into(), points: 5.12, prizes: vec![(prize.into(), 2.0)], from_event: None, until_event: None, from_day: 0, to_day: 0, priority: 0.0, table: 0, num: 1, clear_subject: None };
        let requests = [req(Kind::Synthesis, "ITEM_MIX_CRAFT", "ITEM_A"), req(Kind::Synthesis, "ITEM_MIX_CRAFT", "ITEM_B"), req(Kind::Monster, "MONSTER_X", "ITEM_A")];
        let items = ["ITEM_A", "ITEM_B", "ITEM_MIX_CRAFT"];
        let slot = |kind, target, prize| Some(BoardSlot { kind, target, state: 1, posted: 220, until: 329, count: 1, value: 6.0, prizes: [(prize, 2.0), (-1, 0.0)] });
        let mut slots = vec![None; 10];
        slots[0] = slot(0, 2, 1);
        slots[6] = slot(1, 0, 0);
        let b = board_from_save(&slots, &requests, &items, &["MONSTER_X"]);
        // The Craft request with ITEM_B as its prize, and the monster one.
        assert_eq!((b[0], b[6], b[1]), (Some(1), Some(2), None));
    }

    #[test]
    fn rewards_roll_as_posted() {
        let r = Request {
            kind: Kind::Synthesis,
            target: "x".into(),
            points: 5.12,
            prizes: vec![("a".into(), 1.7067), ("b".into(), 1.5)],
            from_event: None,
            until_event: None,
            from_day: 0,
            to_day: 0,
            priority: 0.0,
            table: 0,
            num: 2,
            clear_subject: None,
        };
        assert_eq!(rewards(&r, 1, 100), (6, vec![("a".into(), 2), ("b".into(), 1)]));
        // Near the deadline: 5.12 × 2 × 1.5 + 2 = 17.36; 1.7067 × 3 + 2 = 7.12; 1.5 × 2 + 1 = 4.
        assert_eq!(rewards(&r, 2, 30), (17, vec![("a".into(), 7), ("b".into(), 4)]));
        assert_eq!(days_to_deadline(0, 200), 9);
        assert_eq!(days_to_deadline(9, 0), crate::calendar::Date { year: 5, month: 4, day: 1 }.number() as i32);
        assert_eq!(wanted(&Request { kind: Kind::Monster, num: 5, ..r.clone() }, 6), 3);
        assert_eq!(wanted(&Request { num: 0, ..r }, 0), 1);
    }

    #[test]
    fn reads_requests() {
        let t = barrel_formats::table::parse_str(
            r#"<Root><QuestBtl TargetTag="MONSTER_ROCKRAT_00" GetPoint="2.088" PrizeItemTag1="ITEM_A" PrizeItemNum1="0.5" StartEvent="EVENT_X" StartDate="91" EndDate="1530" Priority="20" Table="0" Num="2"/></Root>"#,
        )
        .unwrap();
        let r = requests(&t, Kind::Monster);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].prizes, [("ITEM_A".to_string(), 0.5)]);
        assert_eq!((r[0].from_day, r[0].to_day, r[0].num), (91, 1530, 2));
        assert!(!offered(&r[0], 100, |_| false));
        assert!(offered(&r[0], 100, |e| e == "EVENT_X"));
        assert!(!offered(&r[0], 50, |_| true));
    }
    #[test]
    fn the_board_generator() {
        let mut rng = BoardRng::new([1, 2]);
        // t = 1 ^ 2048 = 2049; y = 2 ^ 0 ^ (2049 >> 8 = 8) ^ 2049 = 2059.
        assert_eq!(rng.next(), 2059);
        assert_eq!(rng.state, [2, 2059]);
        // A sure candidate is taken at once; none gives a shuffled pick.
        let mut rng = BoardRng::new(crate::calendar::board_seed(90, 0));
        assert_eq!(board_pick(&[100.0, 0.0], &mut rng), Some(0));
        assert!(board_pick(&[0.0, 0.0, 0.0], &mut rng).is_some());
        assert_eq!(board_pick(&[], &mut rng), None);
    }

    #[test]
    fn board_eligibility() {
        let r = Request {
            kind: Kind::Synthesis,
            target: "ITEM_MIX_CRAFT".into(),
            points: 0.0,
            prizes: Vec::new(),
            from_event: None,
            until_event: None,
            from_day: 91,
            to_day: 1530,
            priority: 29.0,
            table: 0,
            num: 1,
            clear_subject: Some("SUBJECT_A".into()),
        };
        let none = |_: &str| false;
        let all = |_: &str| true;
        // April 1st (90) is before its start; the subject must be cleared.
        assert!(!board_eligible(&r, 90, none, all));
        assert!(board_eligible(&r, 91, none, all));
        assert!(!board_eligible(&r, 91, none, none));
        let mut rng = BoardRng::new(crate::calendar::board_seed(100, 0));
        let slots = draw_synthesis(std::slice::from_ref(&r), 100, none, all, |_| true, &mut rng);
        assert!(slots[1].is_none() && slots[2].is_none());
        // Battle: the higher level goes first, so a sure candidate there wins.
        let mut a = r.clone();
        a.kind = Kind::Monster;
        a.clear_subject = None;
        a.priority = 0.0;
        let mut b = a.clone();
        b.priority = 100.0;
        b.target = "B".into();
        let level = |q: &Request| if q.target == "B" { 9 } else { 1 };
        let slots = draw_battle(&[a, b], 100, none, all, |_| true, level, &mut rng);
        assert_eq!(slots[0], Some(1));
    }
}
