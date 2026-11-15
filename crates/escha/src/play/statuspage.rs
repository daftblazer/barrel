//! The Status page of the main menu's character picker (the `menu-status-page` capture): over the
//! desk (383) and the chosen member's figure (361), layout 400's book (399: the right page,
//! 431, with the member's numbers, and the left page, 398, "Effect & Property" with its slot
//! icons and lines and the pager of 751), then the footer (372) and header (371) lines and the
//! command's tab (531), as the picker draws them.
//!
//! What the game does not draw there is left out: the bookmark (393, the Formation panel has it),
//! the skills pages (388, 387), the effect page's second group (395) and a trait's bar and text
//! when the slot's item has none. The pager shows the page and the page count ("1/2": the effect
//! page has two groups).
//!
//! Not yet the game's: the figure of the chosen member (Escha's is drawn), the effect texts of
//! the worn items (their traits are not listed; the lines are drawn empty), the page turning.

use barrel_escha::data::layout::{self, Placed};
use barrel_formats::pe::Pe;

use crate::formation::Member;
use crate::hud::Element;
use crate::kit::{number, raised};

const SCREEN: u32 = 400;
pub const RIGHT_PAGE: u32 = 431;
pub const BOOKMARK: u32 = 393;
const LEFT_PAGE: u32 = 398;
/// The left page's first group of effect rows (layout 396: 4 slots of 3 lines; 397 is the second)
/// and its pager.
const EFFECT_GROUP: u32 = 396;
const PAGER: u32 = 751;
/// The effect lines (part 0xb cut 7).
const LINE: i32 = 7;
/// The pager's values: page 1 of 2.
const PAGES: [i32; 2] = [1, 2];
/// The SysInfo.xml row ("View current character status.") and the Menu_Line_Help.xml row
/// ("Character, Return, Skills") of the header and footer.
pub const HEADER_ROW: usize = 9;
pub const FOOTER_ROW: usize = 37;

/// The layouts the page is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    figure: Vec<Placed>,
    screen: Vec<Placed>,
    pub tab: Vec<Placed>,
    pub header: Vec<Placed>,
    pub footer: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        let pick = crate::picker::Layouts::load(pe);
        Layouts { desk: get(crate::picker::DESK), figure: get(crate::picker::FIGURE), screen: get(SCREEN), tab: pick.tab, header: pick.header, footer: pick.footer }
    }
}

/// The right page's `nodes` (layout 431 in place) for `m`: the labels, the numbers (the lead has no
/// friendship), the element icons and the name plate, in the layout's order. The Equip page
/// draws the same page, with the bookmark.
pub fn right_page(pe: &Pe, nodes: &[&Placed], m: &Member, lead: bool) -> Vec<Element> {
    let texts = |p: &Placed| layout::text(pe, p).filter(|t| !(lead && t.text == "Friend"));
    let mut numbers = m.values.numbers().to_vec();
    if lead {
        numbers.pop();
    }
    crate::overview::face(pe, nodes, &texts, &m.values.name, None, &numbers)
}

/// The left page: its panel, slot icons and title, the effect rows' lines, the title tab and
/// the pager.
fn left_page(pe: &Pe, l: &Layouts) -> Vec<Element> {
    let mut out = Vec::new();
    let mut pages = PAGES.iter();
    for p in l.screen.iter().filter(|p| p.path.get(1) == Some(&LEFT_PAGE)) {
        let n = p.node;
        match p.path.get(2) {
            None if n.part == 0x164 => {
                if let Some(t) = layout::text(pe, p).filter(|t| !t.filled_in()) {
                    out.push(raised(t.clone(), &t.text));
                }
            }
            None => out.push(Element::stored(p)),
            // The first group's lines (a bar and text come with a trait).
            Some(&EFFECT_GROUP) if n.part == 0xb && n.cut == LINE => out.push(Element::stored(p)),
            // The pager: the page and the page count, a slash between, in the slash's ink.
            Some(&PAGER) if n.part == 0x4f && n.kind != 0 => out.extend(pages.next().into_iter().flat_map(|&v| number(pe, p, v))),
            Some(&PAGER) if n.part == 0x4f => out.push(Element::Tinted { part: n.part, cut: n.cut, at: p.at, rgb: crate::kit::ink(pe, p) }),
            Some(&PAGER) => out.push(Element::stored(p)),
            _ => {}
        }
    }
    out
}

/// All the page's elements for member `chosen` of `members` (`lead` the lead's character number).
pub fn elements(pe: &Pe, l: &Layouts, words: &crate::picker::Words, members: &[Member], chosen: usize, lead: usize) -> Vec<Element> {
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: crate::picker::DESK_CUT, at: p.at }).collect();
    out.extend(l.figure.iter().map(Element::stored));
    let Some(m) = members.get(chosen) else { return out };
    for p in l.screen.iter().filter(|p| p.path.len() == 2 && p.path[1] == 399) {
        out.push(Element::stored(p));
    }
    let page: Vec<&Placed> = l.screen.iter().filter(|p| p.path.get(2) == Some(&RIGHT_PAGE) && p.path.get(3) != Some(&BOOKMARK)).collect();
    out.extend(right_page(pe, &page, m, m.face.character as usize == lead));
    out.extend(left_page(pe, l));
    out.extend(crate::picker::chrome(pe, (&l.footer, &words.footer), (&l.header, &words.header), &l.tab, crate::picker::Kind::Status.tab_cut()));
    out
}
