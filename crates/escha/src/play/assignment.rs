//! The main menu's Assignment page, from the game's layout 430 (`a15_menu.g1t`) over the desk
//! (383) with the footer (372), the header (371) and the tab (531): its book (layout 428, with the
//! Tasks and Bonus tabs) showing the period's card (layout 425: 25 squares, layout 419 each, in
//! five columns A to E and five rows 1 to 5) and the details panel (layout 418: the chosen task's
//! name, a kind 22 text, its icon, its description, the page count and the reward), with the rank
//! box (layout 527).
//!
//! The card is the period's (`calendar::period`: card k for period k, `SubjectCardData.xml`'s
//! `CARD_0k`), its tasks `SubjectData.xml`'s rows with that `CardTag`, each at its `Pos` (`A1`
//! column A row 1); a square shared by the steps of a chain shows the one the save has reached.
//! The game draws the squares column by column. What a square shows follows its task's record in
//! the save (the state and the completion in hundredths of a percent, `data::subject`): a cleared
//! one (state 8) is only its stamp ("OK", part 0x10); an achieved one (7) has its plate (part
//! 0xf cut 6), icon, the "Report!" glow (part 0x9c) and "100%" in big digits (part 0xaf); an open
//! one (6, or 5 with a mark, part 0xd cut 15) its icon and its completion in percent (part 0x56)
//! unless a hidden event keeps it a "?" (icon cut 5, nothing else); the chosen square has the
//! cursor frame (part 0xd cut 11) and the centre's main assignment its own plate (part 0xf cut
//! 7, with no icon). A square's icon is part 0xe: cut 5 "?" (12 lit), and by `Type` a sword for
//! `SUBJECT_CATEGORY_BTL` (6, lit 13), a basket for `ADV` (8, lit 14), a pot for `MIX` (10, lit
//! 15); the main assignment (`OTHER`) is cut 21 (lit 22) in the details.
//!
//! The text of a task can start with a Shift-JIS symbol the game's font does not have: the game
//! shows two boxes ([`plain`]).
//!
//! The glow, the cursor frame, the mark and the cursor's glow flip through their cuts with the
//! screen's clock ([`layout::Flip`]). Not yet the game's: the pulse of a chosen square's plate, the stars of the reward's row, the Bonus tab and the past cards list
//! (layout 429), the lines overlay (424) and the stamps' variants; tasks never seen in a save
//! count as open at nothing; the "new" mark's meaning (state 5) is a guess from one square.

use std::path::Path;

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_escha::data::subject::SavedSubject;
use barrel_formats::pe::Pe;

use crate::play::hud::Element;
use crate::play::kit::{number, raised};
use crate::play::title::Button;

pub const LAYOUT: u32 = 430;
const BACKGROUND: u32 = 383;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const TAB: u32 = 531;
const BONUS_PAGE: u32 = 426;
const PAST_CARDS: u32 = 429;
const SQUARE: u32 = 419;
const LINES: u32 = 424;
const PANEL: u32 = 418;
const CARD: u32 = 425;
const PAGER: u32 = 459;
const REWARD_ROW: u32 = 413;
const STARS: u32 = 412;
const REWARD: u32 = 414;
const TASKS_TAB: u32 = 410;
const BONUS_TAB: u32 = 411;
const ICON: i32 = 0xe;
const PLATE: i32 = 0xf;
const CURSOR_FRAME: i32 = 0xd;
const GLOW: i32 = 0x9c;
const TEXT: i32 = 0x164;
const SIDE: usize = 5;
/// Characters a description line holds before it is wrapped.
const WRAP: usize = 40;
/// The description's lines are this far apart.
const LINE_STEP: i32 = 45;
/// The header's line in `SysInfo.xml` ("Select an assignment to check.") and the footer's in
/// `Menu_Line_Help.xml` ("Select, Change Entry, Return").
pub const HEADER_ROW: usize = 81;
pub const FOOTER_ROW: usize = 43;
/// The tab's cut for the Assignment command (the seventh of the menu's commands).
const TAB_CUT: i32 = 6;
const DESK_CUT: i32 = 1;
/// The tabs not open are drawn in this grey (the capture: 178 of 255).
const GREY: [u8; 3] = [178, 178, 178];
/// The square the cursor starts on: the centre, the main assignment.
pub const CENTRE: usize = 12;
/// The centre's plate (part 0xf cut 7, 104 pixels) from the square's place, fitted to the capture.
const CENTRE_PLATE_FROM_SQUARE: [i32; 2] = [8, 8];
/// The details' icon for the main assignment, lit.
const MAIN_ICON_LIT: i32 = 22;
/// The cursor's glow (part 0x9f, 96 pixels) from the chosen square's place, fitted to the capture.
const CURSOR: i32 = 0x9f;
const CURSOR_FROM_SQUARE: [i32; 2] = [-72, -72];

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
    /// The save's record of it (`data::subject::SavedSubject`): its state and its completion in
    /// hundredths of a percent; without a save a task is open and at nothing.
    pub state: i32,
    pub completion: i32,
    /// The points it pays (`Score`).
    pub score: i32,
}

/// A task's state (the save's): 5 and 6 open (5 with the "new" mark), 7 achieved and waiting to
/// be reported, 8 cleared; 1 a step of a chain already past and 9 one not yet reached.
const OPEN: i32 = 6;
const NEW: i32 = 5;
const ACHIEVED: i32 = 7;
const CLEARED: i32 = 8;
const PAST: i32 = 1;
const AHEAD: i32 = 9;

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

/// The period's card for day `today`: 25 squares, row by row (`seen` says whether an event has
/// been seen, `subjects` the save's records by `SubjectData` row).
pub fn card(root: &Path, today: u32, seen: impl Fn(&str) -> bool, subjects: &[SavedSubject]) -> crate::play::Result<Vec<Option<Task>>> {
    let mut squares: Vec<Option<Task>> = vec![None; SIDE * SIDE];
    let Some(k) = barrel_escha::calendar::period(today) else { return Ok(squares) };
    let tag = format!("CARD_{k:02}");
    let table = barrel_formats::table::parse(&barrel_files::read(root.join("Saves_EN/subject/SubjectData.xml"))?)?;
    for (index, row) in table.children.iter().enumerate().filter(|(_, r)| r.attr("CardTag") == Some(tag.as_str())) {
        let Some(slot) = row.attr("Pos").and_then(square_index) else { continue };
        let saved = subjects.get(index);
        let (state, completion) = saved.map_or((OPEN, 0), |s| (s.state, s.completion));
        let event = row.attr("isHiddenEvent").unwrap_or("");
        // A hidden task shows as "?" until it is achieved (or, without a save, its event seen).
        let hidden = !event.is_empty() && !seen(event) && !(saved.is_some() && state >= ACHIEVED);
        // A square shared by the steps of a chain shows the one the save has reached: the first
        // not past nor ahead (without a save, the first already open, else the first).
        if let Some(have) = squares[slot].as_ref() {
            let better = if saved.is_some() { matches!(have.state, PAST | AHEAD) && !matches!(state, PAST | AHEAD) } else { have.hidden && !hidden };
            if !better {
                continue;
            }
        }
        let category = match row.attr("Type").unwrap_or("") {
            "SUBJECT_CATEGORY_BTL" => Category::Battle,
            "SUBJECT_CATEGORY_ADV" => Category::Adventure,
            "SUBJECT_CATEGORY_MIX" => Category::Synthesis,
            _ => Category::Other,
        };
        let text = [row.attr("Text1"), row.attr("Text2")].into_iter().flatten().map(plain).collect::<Vec<_>>().join(" ");
        let score = row.attr("Score").and_then(|v| v.parse().ok()).unwrap_or(0);
        squares[slot] = Some(Task { name: row.attr("Name").unwrap_or("").to_string(), category, text, hidden, state, completion, score });
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

/// A table text without its markup (`<CLBL>` ...), and with a symbol the font lacks (a
/// Shift-JIS double-byte code) as the two boxes the game draws for its two bytes.
fn plain(t: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for ch in t.chars() {
        match ch {
            '<' => tag = true,
            '>' => tag = false,
            _ if tag => {}
            c if c.is_ascii() => out.push(c),
            _ => out.push_str("\u{25a0}\u{25a0}"),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub chosen: usize,
}

impl Default for Assignment {
    fn default() -> Assignment {
        Assignment { chosen: CENTRE }
    }
}

/// The layouts the page is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
    tab: Vec<Placed>,
    screen: Vec<Placed>,
    rank: Vec<Placed>,
    /// The cursor glow's flipbook (layout 691).
    cursor: Option<layout::Flip>,
}

/// Where the rank box (layout 527) sits, fitted to the capture.
const RANK_BOX_AT: [i32; 2] = [1426, 840];
const RANK_BOX: u32 = 527;

impl Layouts {
    /// Every cut the page's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        let mut out = crate::play::kit::flip_elements(&self.screen);
        out.extend(self.cursor.into_iter().flat_map(|f| crate::play::kit::cuts_of(CURSOR, f)));
        out
    }

    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        Layouts { desk: get(BACKGROUND), header: get(HEADER), footer: get(FOOTER), tab: get(TAB), screen: get(LAYOUT), rank: get(RANK_BOX), cursor: crate::play::kit::cursor_glow(pe) }
    }
}

/// The header's and the footer's lines.
pub struct Words {
    pub header: String,
    pub footer: String,
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

    /// All the page's elements over the period's `card`; `rank` the rank box's icon and the
    /// points to the next rank; `tick` the clock's 60 Hz ticks (the glows' and frames' flipbooks).
    pub fn elements(&self, pe: &Pe, l: &Layouts, words: &Words, card: &[Option<Task>], rank: crate::play::requests::Panel, tick: u32) -> Vec<Element> {
        let line = |nodes: &[Placed], text: &str| -> Vec<Element> {
            nodes.iter().flat_map(|p| match layout::text(pe, p) {
                Some(t) => vec![raised(t, text)],
                None => vec![Element::stored(p)],
            }).collect()
        };
        let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: DESK_CUT, at: p.at }).collect();
        out.extend(line(&l.footer, &words.footer));
        out.extend(line(&l.header, &words.header));
        for p in &l.tab {
            out.push(match Element::stored(p) {
                Element::Sprite { part, at, .. } if part == 0x2e || part == 0x2c => Element::Sprite { part, cut: TAB_CUT, at },
                e => e,
            });
        }
        let chosen = card.get(self.chosen).and_then(Option::as_ref);
        // The tabs come twice: first both greyed, last the open one (Tasks) over the rest.
        let (mut pass, mut in_tabs) = (0, false);
        let mut squares: Vec<Vec<&Placed>> = Vec::new();
        for p in &l.screen {
            let n = p.node;
            let inside = |id: u32| p.path.contains(&id);
            if inside(BONUS_PAGE) || inside(PAST_CARDS) || inside(LINES) {
                continue;
            }
            if inside(SQUARE) {
                // A square starts at its plate.
                if n.part == PLATE && p.path.last() == Some(&SQUARE) {
                    squares.push(Vec::new());
                }
                if let Some(s) = squares.last_mut() {
                    s.push(p);
                }
                continue;
            }
            // The squares are drawn where the first of them is: flush them (column by column).
            if !squares.is_empty() && p.path.get(2) != Some(&CARD) {
                out.extend(self.squares(pe, &squares, card, tick));
                squares.clear();
            }
            let tab = inside(TASKS_TAB) || inside(BONUS_TAB);
            if tab && !in_tabs {
                pass += 1;
            }
            in_tabs = tab;
            if tab {
                let plain_text = layout::text(pe, p).is_some_and(|t| t.colour[0] == 0);
                match (pass, n.part) {
                    // First both tabs, greyed: the plate and the plain text.
                    (1, TEXT) if plain_text => out.extend(layout::text(pe, p).map(|t| Element::Label(Text { at: [t.at[0], t.at[1] - crate::play::hud::TEXT_RISE], colour: [0xff, GREY[0], GREY[1], GREY[2]], ..t }))),
                    (1, TEXT) => {}
                    (1, _) => out.push(Element::Tinted { part: n.part, cut: n.cut, at: p.at, rgb: GREY }),
                    // Then the open tab, Tasks, with both its texts.
                    (_, TEXT) if inside(TASKS_TAB) => out.extend(layout::text(pe, p).map(|t| raised(t.clone(), &t.text))),
                    (_, _) if inside(TASKS_TAB) => out.push(Element::stored(p)),
                    _ => {}
                }
                continue;
            }
            if inside(PANEL) {
                out.extend(self.panel(pe, p, chosen));
            } else if n.part == 0xc {
                // The book (425) and the spine (428).
                out.push(Element::stored(p));
            }
        }
        if !squares.is_empty() {
            out.extend(self.squares(pe, &squares, card, tick));
        }
        // The rank box.
        let (mut icon, mut numbers) = (true, 0);
        for p in &l.rank {
            let at = [p.at[0] + RANK_BOX_AT[0], p.at[1] + RANK_BOX_AT[1]];
            let moved = Placed { at, ..p.clone() };
            match p.node.part {
                0x31 => {
                    if std::mem::take(&mut icon) {
                        out.push(Element::Sprite { part: 0x31, cut: rank.icon, at });
                    }
                }
                0x4e if p.node.kind == 28 => {
                    if numbers == 0 {
                        out.extend(number(pe, &moved, rank.to_next));
                    }
                    numbers += 1;
                }
                0x4e => {}
                TEXT => out.extend(layout::text(pe, &moved).map(|t| raised(t.clone(), &t.text))),
                _ => out.push(Element::stored(&moved)),
            }
        }
        // The cursor's glow (its eight cuts turn with the clock), drawn last.
        let (col, row) = (self.chosen % SIDE, self.chosen / SIDE);
        let origin = l.screen.iter().filter(|p| p.path.last() == Some(&SQUARE) && p.node.part == PLATE).nth(col * SIDE + row);
        out.extend(origin.map(|p| Element::Sprite { part: CURSOR, cut: l.cursor.map_or(0, |f| f.cut(tick)), at: [p.at[0] + CURSOR_FROM_SQUARE[0], p.at[1] + CURSOR_FROM_SQUARE[1]] }));
        out
    }

    /// The squares, column by column (`squares` in the layout's order).
    fn squares(&self, pe: &Pe, squares: &[Vec<&Placed>], card: &[Option<Task>], tick: u32) -> Vec<Element> {
        let mut out = Vec::new();
        for (g, nodes) in squares.iter().enumerate() {
            let (col, row) = (g / SIDE, g % SIDE);
            let k = row * SIDE + col;
            out.extend(self.square(pe, nodes, card.get(k).and_then(Option::as_ref), k == self.chosen, tick));
        }
        out
    }

    /// One square's elements from its layout's nodes (419): the plate, the cursor frame, the icon,
    /// the stamp, the percent, the mark, the glow, the big percent.
    fn square(&self, pe: &Pe, nodes: &[&Placed], task: Option<&Task>, chosen: bool, tick: u32) -> Vec<Element> {
        let Some(task) = task else { return Vec::new() };
        let mut out = Vec::new();
        let at = |i: usize| nodes.get(i).map_or([0, 0], |p| p.at);
        let (plate, frame, icon, stamp, percent, sign, mark, glow, big, big_sign) = (0, 1, 2, 3, 4, 5, 6, 7, 8, 9);
        // A piece that flips through its cuts (the frame, the mark, the glow): the cut of its
        // flipbook now, else the one given.
        let flipped = |i: usize, part: i32, cut: i32| Element::Sprite { part, cut: nodes.get(i).and_then(|p| p.node.flip()).map_or(cut, |f| f.cut(tick)), at: at(i) };
        let main = task.category == Category::Other;
        if task.state == CLEARED {
            out.push(Element::Sprite { part: 0x10, cut: 0, at: at(stamp) });
            return out;
        }
        if chosen {
            out.push(flipped(frame, CURSOR_FRAME, 11));
        }
        if task.state == ACHIEVED {
            out.push(Element::Sprite { part: PLATE, cut: 6, at: at(plate) });
        } else if main && chosen {
            let from = at(plate);
            out.push(Element::Sprite { part: PLATE, cut: 7, at: [from[0] + CENTRE_PLATE_FROM_SQUARE[0], from[1] + CENTRE_PLATE_FROM_SQUARE[1]] });
        }
        if let Some(cut) = task.icon(false) {
            out.push(Element::Sprite { part: ICON, cut, at: at(icon) });
        }
        let shown = !task.hidden;
        if task.state == ACHIEVED && shown {
            out.push(flipped(glow, GLOW, 0));
            if let Some(p) = nodes.get(big) {
                out.extend(number(pe, p, 100));
            }
            out.push(Element::Sprite { part: 0xaf, cut: 13, at: at(big_sign) });
        } else if shown && (task.state == OPEN || task.state == NEW) {
            if let Some(p) = nodes.get(percent) {
                out.extend(number(pe, p, task.completion / 100));
            }
            out.push(Element::Sprite { part: 0x56, cut: 13, at: at(sign) });
            if task.state == NEW {
                out.push(flipped(mark, CURSOR_FRAME, 15));
            }
        }
        out
    }

    /// A node of the details panel for the chosen task.
    fn panel(&self, pe: &Pe, p: &Placed, chosen: Option<&Task>) -> Vec<Element> {
        let n = p.node;
        let inside = |id: u32| p.path.contains(&id);
        // The reward row's stars are not drawn; its plate (413 itself) is.
        if inside(STARS) {
            return Vec::new();
        }
        if p.path.last() == Some(&REWARD_ROW) {
            return vec![Element::stored(p)];
        }
        if inside(PAGER) {
            return match (n.part, n.cut) {
                (0x63, _) => vec![Element::stored(p)],
                (0x4f, _) if n.kind == 29 => number(pe, p, 1),
                (0x4f, 12) => vec![Element::Tinted { part: 0x4f, cut: 12, at: p.at, rgb: crate::play::kit::ink(pe, p) }],
                _ => Vec::new(),
            };
        }
        if inside(REWARD) {
            return match (n.part, n.kind) {
                (TEXT, _) => layout::text(pe, p).map(|t| raised(t.clone(), &t.text)).into_iter().collect(),
                (0x5a, _) => chosen.map_or_else(Vec::new, |c| number(pe, p, c.score)),
                _ => vec![Element::stored(p)],
            };
        }
        if inside(949) {
            // The stamp of a cleared task.
            return chosen.filter(|c| c.state == CLEARED).map(|_| Element::stored(p)).into_iter().collect();
        }
        match n.part {
            TEXT => {
                let Some(t) = layout::text(pe, p) else { return Vec::new() };
                if t.kind == 22 {
                    let name = chosen.map_or(String::new(), |c| if c.hidden { "???".into() } else { c.name.clone() });
                    return vec![raised(t, &name)];
                }
                if t.text.starts_with("By pressing") {
                    let text = chosen.map_or(String::new(), |c| if c.hidden { "???".into() } else { c.text.clone() });
                    return wrap(&text, WRAP)
                        .into_iter()
                        .enumerate()
                        .map(|(i, line)| raised(Text { at: [t.at[0], t.at[1] + LINE_STEP * i as i32], ..t.clone() }, &line))
                        .collect();
                }
                Vec::new()
            }
            ICON => {
                let cut = chosen.and_then(|c| if c.category == Category::Other { Some(MAIN_ICON_LIT) } else { c.icon(true) });
                cut.map(|cut| Element::Sprite { part: ICON, cut, at: p.at }).into_iter().collect()
            }
            _ => vec![Element::stored(p)],
        }
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
        assert_eq!(plain("\u{2605}Go"), "\u{25a0}\u{25a0}Go");
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        let mut a = Assignment { chosen: 0 };
        a.press(Button::Down);
        a.press(Button::Right);
        assert_eq!(a.chosen, 6);
        a.press(Button::Up);
        a.press(Button::Up);
        assert_eq!(a.chosen, 1);
        assert!(a.press(Button::Back));
        assert_eq!(Assignment::default().chosen, CENTRE);
    }
}
