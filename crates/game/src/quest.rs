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
        })
        .collect()
}

/// Whether a request is offered on `day`, given which events have been seen.
pub fn offered(r: &Request, day: i32, seen: impl Fn(&str) -> bool) -> bool {
    (r.from_day..=r.to_day).contains(&day)
        && r.from_event.as_deref().is_none_or(&seen)
        && !r.until_event.as_deref().is_some_and(&seen)
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
}
