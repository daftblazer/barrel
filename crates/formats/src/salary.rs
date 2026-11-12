//! The monthly report comments (`Saves/rank/SalaryMessData.xml`, loaded by `0x14014f6c0`): 22
//! rows, each a name, a group, a start and end (by the values, 1 to 48 for most and 100/101
//! for the two ending rows) and up to seven counts each with its own MORE or LESS: battles
//! (`battle`/`btlmore`), gathering (`gather`/`gthmore`), items supplied (`item`/`itmmore`),
//! synthesis (`mix`/`mixmore`), refining (`train`/`trnmore`), artifacts (`artifact`/`atfmore`)
//! and requests (`quest`/`qstmore`), then the text (0x88-byte rows at `[0x140e00f00]` +0x30;
//! the pick is `barrel_game::rank::comment`).

use crate::table::Element;

/// One count a comment asks for: the threshold and whether the month's count must be more
/// (true) or less.
pub type Count = Option<(i32, bool)>;

#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    pub name: String,
    pub group: i32,
    pub start: i32,
    pub end: i32,
    /// Battles, gathering, items, synthesis, refining, artifacts, requests.
    pub counts: [Count; 7],
    pub text: String,
}

const COUNTS: [(&str, &str); 7] = [
    ("battle", "btlmore"),
    ("gather", "gthmore"),
    ("item", "itmmore"),
    ("mix", "mixmore"),
    ("train", "trnmore"),
    ("artifact", "atfmore"),
    ("quest", "qstmore"),
];

pub fn comments(t: &Element) -> Vec<Comment> {
    let num = |e: &Element, n: &str| e.attr(n).and_then(|v| v.trim().parse().ok());
    t.children
        .iter()
        .filter(|e| e.name == "SalaryMessData")
        .map(|e| Comment {
            name: e.attr("SalaryName").unwrap_or("").to_string(),
            group: num(e, "group").unwrap_or(0),
            start: num(e, "start").unwrap_or(0),
            end: num(e, "end").unwrap_or(0),
            counts: COUNTS.map(|(v, m)| num(e, v).map(|n| (n, e.attr(m) == Some("MORE")))),
            text: e.attr("text").unwrap_or("").to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_a_row() {
        let x = br#"<Root><SalaryMessData SalaryName="a" group="3" start="1" end="48" battle="5" btlmore="MORE" train="0" trnmore="LESS" text="t"/></Root>"#;
        let c = super::comments(&crate::table::parse(x).unwrap());
        assert_eq!((c[0].group, c[0].end, c[0].counts[0], c[0].counts[1]), (3, 48, Some((5, true)), None));
        assert_eq!(c[0].counts[4], Some((0, false)));
    }
}
