//! Requests, from `Saves/quest/QuestAlc.xml` (synthesis), `QuestBtl.xml` (monsters) and
//! `QuestGat.xml` (gathering), as the executable's schema types them (`docs/tables.md`,
//! "Requests"): the target, the points, two prizes, the days and events that bound it.
//!
//! The points and the prize counts are floats in the tables (5.12, 1.7067): how the game turns
//! them into whole numbers is not read yet, so they are kept as given.

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
    rng: &mut BoardRng,
) -> [Option<usize>; N] {
    let mut out = [None; N];
    for (k, slot) in out.iter_mut().enumerate() {
        let mut candidates: Vec<usize> = (0..requests.len())
            .filter(|&i| {
                let r = &requests[i];
                r.kind == kind && (r.table == k as i32 || r.table == -1) && board_eligible(r, day, seen, cleared) && known(r)
            })
            .collect();
        if let Some(key) = key {
            candidates.sort_by_key(|&i| std::cmp::Reverse(key(&requests[i])));
        }
        let chances: Vec<f32> = candidates.iter().map(|&i| requests[i].priority).collect();
        *slot = board_pick(&chances, rng).map(|j| candidates[j]);
    }
    out
}

/// The three synthesis slots (`0x14033b5e0`, board places 0 to 2, `Table` 0 to 2). `known`
/// stands for the game's tests of the target: a count of at least 6 in `0x141079fe0` and its
/// recipe flagged (`0x1400e31a0`, `0x141079fe0` + 0x8c0).
pub fn draw_synthesis(requests: &[Request], day: i32, seen: impl Fn(&str) -> bool, cleared: impl Fn(&str) -> bool, known: impl Fn(&Request) -> bool, rng: &mut BoardRng) -> [Option<usize>; 3] {
    draw_slots(requests, Kind::Synthesis, day, &seen, &cleared, &known, None, rng)
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
    draw_slots(requests, Kind::Monster, day, &seen, &cleared, &known, Some(&level), rng)
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

#[cfg(test)]
mod tests {
    use super::*;

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
