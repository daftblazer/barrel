//! The main menu's Journal page as the game draws it (the `menu-cmd-7` capture): over the desk
//! (383) the book's panel (part 0x19 of layout 491, at (114, 116)), five library tabs (layouts
//! 486 to 490: each a plate, its name and a "NEW" badge; the chosen one's plate also carries a
//! second, pulsing plate over it), the header line (371), the footer line (372), the title tab
//! (531) and the cursor's glow, last.
//!
//! The five tabs are the list's rows; what is behind them (layouts 492 to 499, a text each, and
//! the library's own pages) is not read yet: Confirm on a tab does nothing here.
//!
//! The pulsing plate (part 0x1a cut 4) is drawn in a colour that runs between white and a tan.
//! The capture has four frames of it (20 frames apart), too few to read the curve: [`pulse`] is
//! a fit that is right at the frame the check compares (2560) and near at the others.
//!
//! The badges: the capture draws one on every tab, as the save has library marks clear
//! (`Summary::library_new`); which tab a mark belongs to is not told by one save, so the badge
//! is on all five or on none.

use barrel_escha::data::layout::{self, Placed};
use barrel_formats::pe::Pe;

use crate::hud::Element;
use crate::kit::raised;
use crate::title::Button;

/// `SysInfo.xml`'s "Check game progression." (the header line).
pub const HEADER_ROW: usize = 14;
/// A `Menu_Line_Help.xml` row with the footer's words ("Select, Return, Confirm"; several rows
/// read alike, which one the game takes is not told).
pub const FOOTER_ROW: usize = 13;
const DESK: u32 = 383;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const TAB: u32 = 531;
const PAGE: u32 = 485;
/// The tab layouts, one per row.
const ROW_LAYOUTS: std::ops::RangeInclusive<u32> = 486..=490;
const DESK_CUT: i32 = 1;
const BOOK: i32 = 0x19;
const PLATE: i32 = 0x1a;
const PULSE_CUT: i32 = 4;
const BADGE: i32 = 0x4a;
const BADGE_LAST_CUT: i32 = 3;
const TEXT: i32 = 0x164;
/// The title tab's command cut (Journal is the ninth of the ten commands).
const TAB_CUT: i32 = 8;
/// The cursor's glow from the chosen plate's corner (the capture: the glow at (24, 104) with the
/// first plate at (84, 194)).
const CURSOR_FROM_PLATE: [i32; 2] = [-60, -90];
/// The rows the page lists.
pub const ROWS: usize = 5;

/// The pulse's far colour (the capture's frame 2560: the plate at full tan) and its period in
/// ticks, with the clock phase where it is white (the capture's 2520, clock 2724 = 4 mod 80).
const TAN: [f32; 3] = [227.0 / 255.0, 187.0 / 255.0, 110.0 / 255.0];
const PULSE_PERIOD: f32 = 80.0;
const PULSE_WHITE_AT: f32 = 4.0;
/// The curve's exponent: the frames 20 before and after the white one read 0.91 and 0.82 of the
/// way to tan, a sine's square would give 0.5.
const PULSE_SHAPE: f32 = 0.21;

/// The page's cursor: the chosen tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Journal {
    pub chosen: usize,
}

impl Journal {
    /// Moves over the tabs; Back returns true (the page closes).
    pub fn press(&mut self, b: Button) -> bool {
        match b {
            Button::Up => self.chosen = (self.chosen + ROWS - 1) % ROWS,
            Button::Down => self.chosen = (self.chosen + 1) % ROWS,
            Button::Back => return true,
            Button::Left | Button::Right | Button::Confirm => {}
        }
        false
    }
}

/// The layouts the page is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
    tab: Vec<Placed>,
    page: Vec<Placed>,
    cursor: Option<layout::Flip>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        Layouts { desk: get(DESK), header: get(HEADER), footer: get(FOOTER), tab: get(TAB), page: get(PAGE), cursor: crate::kit::cursor_glow(pe) }
    }

    /// Every cut the page's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        let mut out = crate::kit::flip_elements(&self.page);
        out.extend(self.cursor.into_iter().flat_map(|f| crate::kit::cuts_of(crate::kit::CURSOR, f)));
        out
    }
}

/// The page's two lines of words.
pub struct Words {
    pub header: String,
    pub footer: String,
}

/// The pulsing plate's colour at clock tick `tick`: white to tan.
fn pulse(tick: u32) -> [f32; 3] {
    let phase = (tick as f32 - PULSE_WHITE_AT) / PULSE_PERIOD * std::f32::consts::TAU;
    let u = ((1.0 - phase.cos()) / 2.0).powf(PULSE_SHAPE);
    TAN.map(|t| 1.0 - (1.0 - t) * u)
}

/// All the page's elements; `new` puts the "NEW" badges on, `tick` the clock's 60 Hz ticks.
pub fn elements(pe: &Pe, l: &Layouts, words: &Words, view: &Journal, new: bool, tick: u32) -> Vec<Element> {
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: DESK_CUT, at: p.at }).collect();
    let mut cursor = None;
    for p in &l.page {
        let n = p.node;
        let row = p.path.iter().find_map(|id| ROW_LAYOUTS.contains(id).then(|| (id - ROW_LAYOUTS.start()) as usize));
        match (row, n.part) {
            (None, BOOK) => out.push(Element::Sprite { part: BOOK, cut: n.cut, at: p.at }),
            (Some(r), PLATE) if n.cut == PULSE_CUT => {
                if r == view.chosen {
                    out.push(Element::Tinted { part: PLATE, cut: n.cut, at: p.at, rgb: pulse(tick).map(|c| (c * 255.0).round() as u8) });
                    cursor = Some([p.at[0] + CURSOR_FROM_PLATE[0], p.at[1] + CURSOR_FROM_PLATE[1]]);
                }
            }
            (Some(_), PLATE) => out.push(Element::stored(p)),
            (Some(_), TEXT) => {
                if let Some(t) = layout::text(pe, p) {
                    out.push(raised(t.clone(), &t.text));
                }
            }
            (Some(_), BADGE) if new => out.push(Element::Sprite { part: BADGE, cut: n.flip().map_or(0, |f| f.cut(tick).min(BADGE_LAST_CUT)), at: p.at }),
            _ => {}
        }
    }
    let line = |nodes: &[Placed], text: &str| -> Vec<Element> {
        nodes
            .iter()
            .flat_map(|p| match layout::text(pe, p) {
                Some(t) => vec![raised(t, text)],
                None => vec![Element::stored(p)],
            })
            .collect()
    };
    out.extend(line(&l.header, &words.header));
    out.extend(line(&l.footer, &words.footer));
    for p in &l.tab {
        out.push(match Element::stored(p) {
            Element::Sprite { part, at, .. } if part == 0x2e || part == 0x2c => Element::Sprite { part, cut: TAB_CUT, at },
            e => e,
        });
    }
    out.extend(cursor.map(|at| Element::Sprite { part: crate::kit::CURSOR, cut: l.cursor.map_or(0, |f| f.cut(tick)), at }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_round_the_tabs_and_back_closes() {
        let mut j = Journal::default();
        assert!(!j.press(Button::Up));
        assert_eq!(j.chosen, ROWS - 1);
        j.press(Button::Down);
        assert_eq!(j.chosen, 0);
        assert!(j.press(Button::Back));
    }

    #[test]
    fn the_pulse_runs_white_to_tan() {
        assert_eq!(pulse(PULSE_WHITE_AT as u32), [1.0, 1.0, 1.0]);
        let far = pulse(PULSE_WHITE_AT as u32 + 40);
        assert!(far.iter().zip(TAN).all(|(a, b)| (a - b).abs() < 1e-5));
    }
}
