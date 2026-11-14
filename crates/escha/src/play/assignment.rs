//! The main menu's Assignment page, from the game's layout 430 (`a15_menu.g1t`): its book (layout
//! 428, with the Tasks and Bonus tabs) showing the period's card (layout 425: 25 squares, layout
//! 419 each, in five columns A to E and five rows 1 to 5, the centre square's emblem part 0xf
//! cut 12 drawn by the card itself) and the details panel (layout 418: the chosen task's name,
//! a kind 22 text, its icon and its description), with the rank box (layout 527).
//!
//! The card is the period's (`calendar::period`: card k for period k, `SubjectCardData.xml`'s
//! `CARD_0k`), its tasks `SubjectData.xml`'s rows with that `CardTag`, each at its `Pos` (`A1`
//! column A row 1). A square's icon is part 0xe: cut 5 "?" (12 lit), and by `Type` a sword for
//! `SUBJECT_CATEGORY_BTL` (6, lit 13), a basket for `ADV` (8, lit 14), a pot for `MIX` (10, lit
//! 15); cuts 7, 9 and 11 are the same in orange (likely cleared). `OTHER` (the centre square's
//! main assignment) has none, the card's emblem shows there.
//!
//! Not yet the game's: the task shown on a square shared by several rows (the first whose
//! `isHiddenEvent` has been seen, else the first, as "?"); "?" with the name and description
//! hidden while that event is unseen; every task uncleared (Barrel does not count them yet: no
//! stamps, progress numbers, completion stars or points); the lit icon on the chosen square
//! with no cursor frame; the description wrapped by character count with its markup dropped;
//! the Bonus tab, the past cards list (layout 429) and the lines overlay (424) left out.

use std::path::Path;

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::Element;
use crate::title::Button;

pub const LAYOUT: u32 = 430;
const BONUS_PAGE: u32 = 426;
const PAST: u32 = 429;
const SQUARE: u32 = 419;
const LINES: u32 = 424;
const PANEL: u32 = 418;
/// The cleared stamps (layouts 949, "OK", and 704).
const STAMPS: [u32; 2] = [0x3b5, 0x2c0];
const ICON: i32 = 0xe;
const PLATE: i32 = 0xf;
const SIDE: usize = 5;
/// Characters a description line holds before it is wrapped.
const WRAP: usize = 40;
const LINE_STEP: i32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Battle,
    Adventure,
    Synthesis,
    Other,
}

/// A task on the card.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub name: String,
    pub category: Category,
    pub text: String,
    /// Its event is unseen: shown as "?".
    pub hidden: bool,
}

impl Task {
    /// Its icon cut (lit when chosen), if it has one.
    fn icon(&self, lit: bool) -> Option<i32> {
        let (plain, white) = match (self.hidden, self.category) {
            (true, _) => (5, 12),
            (false, Category::Battle) => (6, 13),
            (false, Category::Adventure) => (8, 14),
            (false, Category::Synthesis) => (10, 15),
            (false, Category::Other) => return None,
        };
        Some(if lit { white } else { plain })
    }
}

/// The period's card for day `today`: 25 squares, row by row (`seen` says whether an event
/// has been seen).
pub fn card(root: &Path, today: u32, seen: impl Fn(&str) -> bool) -> crate::Result<Vec<Option<Task>>> {
    let mut squares: Vec<Option<Task>> = vec![None; SIDE * SIDE];
    let Some(k) = barrel_escha::calendar::period(today) else { return Ok(squares) };
    let tag = format!("CARD_{k:02}");
    let table = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/subject/SubjectData.xml"))?)?;
    for row in table.children.iter().filter(|r| r.attr("CardTag") == Some(tag.as_str())) {
        let Some(slot) = row.attr("Pos").and_then(square_index) else { continue };
        let event = row.attr("isHiddenEvent").unwrap_or("");
        let hidden = !event.is_empty() && !seen(event);
        // A shared square shows its first task already open, else its first.
        if squares[slot].as_ref().is_some_and(|t| !t.hidden || hidden) {
            continue;
        }
        let category = match row.attr("Type").unwrap_or("") {
            "SUBJECT_CATEGORY_BTL" => Category::Battle,
            "SUBJECT_CATEGORY_ADV" => Category::Adventure,
            "SUBJECT_CATEGORY_MIX" => Category::Synthesis,
            _ => Category::Other,
        };
        let text = [row.attr("Text1"), row.attr("Text2")].into_iter().flatten().map(plain).collect::<Vec<_>>().join(" ");
        squares[slot] = Some(Task { name: row.attr("Name").unwrap_or("").to_string(), category, text, hidden });
    }
    Ok(squares)
}

/// `A1` to `E5` as a square number, row by row.
fn square_index(pos: &str) -> Option<usize> {
    let mut c = pos.chars();
    let col = (c.next()? as usize).checked_sub('A' as usize).filter(|&v| v < SIDE)?;
    let row = c.next()?.to_digit(10)?.checked_sub(1).filter(|&v| (v as usize) < SIDE)? as usize;
    Some(row * SIDE + col)
}

/// A table text without its markup (`<CLBL>` ...).
fn plain(t: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for ch in t.chars() {
        match ch {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// `text` in lines of at most `width` characters, broken at spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let last = lines.last_mut().expect("one line");
        if !last.is_empty() && last.chars().count() + 1 + word.chars().count() > width {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    lines
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Assignment {
    pub chosen: usize,
}

impl Assignment {
    /// Moves over the card; Back returns true (the page closes).
    pub fn press(&mut self, b: Button) -> bool {
        let (row, col) = (self.chosen / SIDE, self.chosen % SIDE);
        self.chosen = match b {
            Button::Left => row * SIDE + col.saturating_sub(1),
            Button::Right => row * SIDE + (col + 1).min(SIDE - 1),
            Button::Up => row.saturating_sub(1) * SIDE + col,
            Button::Down => (row + 1).min(SIDE - 1) * SIDE + col,
            Button::Back => return true,
            Button::Confirm => self.chosen,
        };
        false
    }

    /// The elements to draw from layout 430's placed nodes (`texts` gives a node's text).
    pub fn elements(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, card: &[Option<Task>]) -> Vec<Element> {
        // The squares' places: by column (x) and row (y).
        let mut origins: Vec<[i32; 2]> = Vec::new();
        for p in nodes.iter().filter(|p| p.path.last() == Some(&SQUARE)) {
            let o = [p.at[0] - p.node.at[0], p.at[1] - p.node.at[1]];
            if !origins.contains(&o) {
                origins.push(o);
            }
        }
        let rank = |v: i32, axis: usize| {
            let mut all: Vec<i32> = origins.iter().map(|o| o[axis]).collect();
            all.sort();
            all.dedup();
            all.iter().position(|&x| x == v).unwrap_or(0)
        };
        let chosen = card.get(self.chosen).and_then(Option::as_ref);
        let mut out = Vec::new();
        for p in nodes {
            let n = p.node;
            if p.path.iter().any(|&l| l == BONUS_PAGE || l == PAST || l == LINES) {
                continue;
            }
            // A square's stamps ("Report!", "OK") and counters are nested layouts: none show
            // while no task is counted.
            if p.path.contains(&SQUARE) && p.path.last() != Some(&SQUARE) {
                continue;
            }
            if p.path.iter().any(|l| STAMPS.contains(l)) {
                continue;
            }
            if p.path.last() == Some(&SQUARE) {
                let o = [p.at[0] - n.at[0], p.at[1] - n.at[1]];
                let k = rank(o[1], 1) * SIDE + rank(o[0], 0);
                match n.part {
                    PLATE => out.push(Element::stored(p)),
                    ICON => {
                        if let Some(cut) = card.get(k).and_then(Option::as_ref).and_then(|t| t.icon(k == self.chosen)) {
                            out.push(Element::Sprite { part: ICON, cut, at: p.at });
                        }
                    }
                    _ => {} // stamps, progress numbers and glows: no task is counted yet
                }
                continue;
            }
            let in_panel = p.path.contains(&PANEL);
            if n.part == 0x164 {
                let Some(t) = texts(p) else { continue };
                // Each tab label is there twice, plain (no colour: white) and dark: Tasks, the
                // open tab, white; Bonus dark.
                if !in_panel && (t.text == "Tasks" || t.text == "Bonus") && (t.colour[0] == 0) != (t.text == "Tasks") {
                    continue;
                }
                if in_panel && t.kind == 22 {
                    let name = chosen.map_or(String::new(), |c| if c.hidden { "???".into() } else { c.name.clone() });
                    out.push(Element::Label(Text { text: name, ..t }));
                } else if in_panel && t.text.starts_with("By pressing") {
                    let text = chosen.map_or(String::new(), |c| if c.hidden { "???".into() } else { c.text.clone() });
                    for (i, line) in wrap(&text, WRAP).into_iter().enumerate() {
                        out.push(Element::Label(Text { text: line, at: [t.at[0], t.at[1] + LINE_STEP * i as i32], ..t.clone() }));
                    }
                } else if !t.text.is_empty() && !t.filled_in() {
                    out.push(Element::Label(t));
                }
                continue;
            }
            if in_panel && n.part == ICON {
                if let Some(cut) = chosen.and_then(|c| c.icon(true)) {
                    out.push(Element::Sprite { part: ICON, cut, at: p.at });
                }
                continue;
            }
            out.push(Element::stored(p));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squares_wrap_and_plain_text() {
        assert_eq!(square_index("A1"), Some(0));
        assert_eq!(square_index("E1"), Some(4));
        assert_eq!(square_index("C3"), Some(12));
        assert_eq!(square_index("F1"), None);
        assert_eq!(plain("Increase your <CLBL>Adventurer Level<CLNR>!"), "Increase your Adventurer Level!");
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        let mut a = Assignment::default();
        a.press(Button::Down);
        a.press(Button::Right);
        assert_eq!(a.chosen, 6);
        a.press(Button::Up);
        a.press(Button::Up);
        assert_eq!(a.chosen, 1);
        assert!(a.press(Button::Back));
    }
}
