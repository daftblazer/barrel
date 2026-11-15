//! The main menu's Formation page as the game draws it (the `menu-cmd-3` capture): over the desk
//! (383) the three plates of the Front and Back columns (part 0x14 cut 2), the members' slots
//! (layout 439: a plate with the face, LV, HP and MP, 434), the chosen slot's pulsing frame, the
//! "Front" and "Back" tabs (442, 443), the chosen member's status panel on the right (432: the
//! Status notebook's right page, 431, in a stretched panel), then the footer (372) and header
//! (371) lines, the command's tab (531) and the cursor's glow.
//!
//! The slots are layout 440's (three rows of two columns, Front from (116, 154) and Back from
//! (674, 154), 136 apart; three more rows of two, the Reserve, from y 616, when a member waits
//! there); the game fills them in the formation's order, first the Front column down, then the
//! Back. An empty slot draws nothing, and the Reserve's heading and plate only appear with a
//! member in it (none in the capture). The panel sits at (1228, 148) like layout 441 places 432.
//! Fitted to the capture: the panel's size, the cursor glow's place from the slot.
//!
//! Not yet the game's: the chosen member's figure (Escha's is drawn), the Reserve's rows, picking
//! a member up and putting it elsewhere (the cursor only moves), the mark at (126, 160), which is
//! drawn at the first slot (the lead's) here.

use barrel_escha::data::layout::{self, Placed};
use barrel_formats::pe::Pe;

use crate::play::hud::Element;
use crate::play::kit::{raised, tiled};
use crate::play::overview::Face;
use crate::play::status::Values;
use crate::play::title::Button;

const SCREEN: u32 = 440;
const PANEL: u32 = 432;
const SLOT: u32 = 439;
const RESERVE: u32 = 382;
const PLATE: i32 = 0x14;
const FRAME: i32 = 0x15;
const MARK_CUT: i32 = 4;
const PANEL_AT: [i32; 2] = [1228, 148];
/// The stretched panel behind the status page: its part and size.
const PANEL_PART: i32 = 0x61;
const PANEL_SIZE: [i32; 2] = [456, 820];
/// The cursor's glow from the chosen slot's place (the capture: the glow at (26, 64) over the
/// slot at (116, 154)).
const CURSOR_FROM_SLOT: [i32; 2] = [-90, -90];
const COLUMN: usize = 3;
/// Members past the six of the Front and Back columns wait in the Reserve.
const RESERVE_FROM: usize = 6;
/// The SysInfo.xml row ("Select character to change.") and the Menu_Line_Help.xml row of the lines.
pub const HEADER_ROW: usize = 42;
pub const FOOTER_ROW: usize = 8;
/// The command's cut in the tab's parts.
pub const TAB_CUT: i32 = 4;

/// One member of the formation as its slot and the panel show them.
pub struct Member {
    pub face: Face,
    pub values: Values,
}

/// The formation's cursor over `count` filled slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Formation {
    pub chosen: usize,
    pub count: usize,
}

impl Formation {
    /// A button press; whether the page closes. The cursor moves down a column and across to the
    /// Back (picking a member up to move is not there yet).
    pub fn press(&mut self, b: Button) -> bool {
        let n = self.count.max(1);
        match b {
            Button::Up => self.chosen = (self.chosen + n - 1) % n,
            Button::Down => self.chosen = (self.chosen + 1) % n,
            Button::Left if self.chosen >= COLUMN => self.chosen -= COLUMN,
            Button::Right if self.chosen + COLUMN < n => self.chosen += COLUMN,
            Button::Back => return true,
            _ => {}
        }
        false
    }
}

/// The layouts the page is made of, placed once.
pub struct Layouts {
    screen: Vec<Placed>,
    panel: Vec<Placed>,
    desk: Vec<Placed>,
    cursor: Option<layout::Flip>,
    pub header: Vec<Placed>,
    pub footer: Vec<Placed>,
    pub tab: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        // Layout 432 is nested in the screen: placed at PANEL_AT without a place of its own.
        let own = layout::nodes(pe, PANEL).map_or([0, 0], |(at, _)| at);
        let panel = get(PANEL).iter().map(|p| crate::play::kit::shifted(p, [PANEL_AT[0] - own[0], PANEL_AT[1] - own[1]])).collect();
        let pick = crate::play::picker::Layouts::load(pe);
        Layouts { screen: get(SCREEN), panel, desk: get(crate::play::picker::DESK), cursor: crate::play::kit::cursor_glow(pe), header: pick.header, footer: pick.footer, tab: pick.tab }
    }

    /// Every cut the page's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        let mut out = crate::play::kit::flip_elements(&self.screen);
        out.extend(self.cursor.into_iter().flat_map(|f| crate::play::kit::cuts_of(crate::play::kit::CURSOR, f)));
        out
    }
}

/// The chosen member's status panel.
fn panel(pe: &Pe, l: &Layouts, m: &Member, lead: bool) -> Vec<Element> {
    let texts = |p: &Placed| layout::text(pe, p).filter(|t| !(lead && t.text == "Friend"));
    let mut numbers = m.values.numbers().to_vec();
    if lead {
        numbers.pop();
    }
    let mut out = Vec::new();
    let (frame, rest) = l.panel.split_first().map_or((None, &[][..]), |(f, r)| (Some(f), r));
    if let Some(f) = frame.filter(|f| f.node.part == PANEL_PART) {
        out.extend(tiled(pe, PANEL_PART, f.at, PANEL_SIZE));
    }
    let rest: Vec<&Placed> = rest.iter().collect();
    out.extend(crate::play::overview::face(pe, &rest, &texts, &m.values.name, None, &numbers));
    out
}

/// The nodes of each slot of layout 440 (a run of one place), in the order they come.
fn slot_groups(screen: &[Placed]) -> Vec<Vec<&Placed>> {
    let mut out: Vec<([i32; 2], Vec<&Placed>)> = Vec::new();
    for p in screen.iter().filter(|p| p.path.get(1) == Some(&SLOT)) {
        let origin = [p.at[0] - p.node.at[0], p.at[1] - p.node.at[1]];
        match out.last_mut() {
            Some(g) if g.0 == origin => g.1.push(p),
            _ => out.push((origin, vec![p])),
        }
    }
    out.into_iter().map(|g| g.1).collect()
}

/// One slot: the face's plate, picture, labels and numbers, then the frame when chosen.
fn slot(pe: &Pe, nodes: &[&Placed], m: &Member, chosen: bool, tick: u32) -> (Vec<Element>, Option<[i32; 2]>) {
    let (frame, body): (Vec<&Placed>, Vec<&Placed>) = nodes.iter().partition(|p| p.path.len() == 2);
    let f = &m.face;
    let texts = |p: &Placed| layout::text(pe, p);
    let mut out = crate::play::overview::face(pe, &body, &texts, &f.name, Some(f.character), &[f.level, f.hp.0, f.hp.1, f.mp.0, f.mp.1]);
    let mut at = None;
    if let (true, Some(p)) = (chosen, frame.iter().find(|p| p.node.flip().is_some())) {
        out.push(Element::Sprite { part: FRAME, cut: p.node.flip().map_or(0, |f| f.cut(tick)), at: p.at });
        at = Some([p.at[0] + 18, p.at[1] + 16]);
    }
    (out, at)
}

/// All the page's elements: `members` in the formation's order (Front column first), `lead` the
/// lead's character number, `tick` the clock's 60 Hz ticks.
pub fn elements(pe: &Pe, l: &Layouts, words: &crate::play::picker::Words, view: &Formation, members: &[Member], lead: usize, tick: u32) -> Vec<Element> {
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: 1, at: p.at }).collect();
    let groups = slot_groups(&l.screen);
    let (mut slots_done, mut cursor) = (false, None);
    // Layout 440's nodes in order: the plates, the slots, the mark, the Reserve and the two tabs.
    for p in &l.screen {
        let n = p.node;
        if p.path.get(1) == Some(&SLOT) {
            if !std::mem::replace(&mut slots_done, true) {
                for (k, (group, m)) in groups.iter().zip(members).enumerate() {
                    let (e, at) = slot(pe, group, m, k == view.chosen, tick);
                    out.extend(e);
                    cursor = cursor.or(at);
                }
            }
        } else if p.path.get(1) == Some(&RESERVE) && members.len() <= RESERVE_FROM {
            continue;
        } else if n.part == PLATE && n.cut == MARK_CUT {
            out.extend((!members.is_empty()).then(|| Element::stored(p)));
        } else if n.part == 0x164 {
            if let Some(t) = layout::text(pe, p).filter(|t| !t.filled_in()) {
                out.push(raised(t.clone(), &t.text));
            }
        } else {
            out.push(Element::stored(p));
        }
    }
    if let Some(m) = members.get(view.chosen) {
        out.extend(panel(pe, l, m, m.face.character as usize == lead));
    }
    out.extend(crate::play::picker::chrome(pe, (&l.footer, &words.footer), (&l.header, &words.header), &l.tab, TAB_CUT));
    out.extend(cursor.map(|at| Element::Sprite { part: crate::play::kit::CURSOR, cut: l.cursor.map_or(0, |f| f.cut(tick)), at: [at[0] + CURSOR_FROM_SLOT[0], at[1] + CURSOR_FROM_SLOT[1]] }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_goes_down_a_column_and_across() {
        let mut f = Formation { chosen: 0, count: 5 };
        f.press(Button::Down);
        assert_eq!(f.chosen, 1);
        f.press(Button::Right);
        assert_eq!(f.chosen, 4);
        f.press(Button::Right);
        assert_eq!(f.chosen, 4, "no Back slot to the right of the second Back member");
        f.press(Button::Left);
        assert_eq!(f.chosen, 1);
        assert!(f.press(Button::Back));
    }
}
