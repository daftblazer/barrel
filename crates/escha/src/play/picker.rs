//! The character picker the main menu's Adventure Equip., Battle Equip. and Status commands open
//! first (the `menu-cmd-1`, `-2` and `-4` captures are the same picker): over the desk (383) the
//! lead's figure (361), then one row a member, each layout 362 (the plate and the tab of 359 and
//! 358 with the face, name, LV, HP, MP and Next, and "Friend" for those besides the lead), the
//! chosen row's pulsing frame (the flipbook node of 362), the footer (372) and header (371)
//! lines, the command's tab (531) and the cursor's glow (part 0x9f), drawn last.
//!
//! The rows go where layout 365 puts its six party plates, down and to the right (from (78, 152),
//! 136 down and about 45 right a row). The members are listed from the first of those places
//! when there are five or more, and from the later ones as there are fewer: the captures'
//! two-member list (Adventure Equip. lists only the two alchemists, Escha and Logy) starts at the
//! second, so the first is `(5 - n) / 2` places down (fitted to the 2 and 5 member lists).
//!
//! Not yet the game's: the figure of another chosen member (Escha's is drawn for everyone; the
//! game draws the chosen member's), the opening and closing motion.

use barrel_escha::data::layout::{self, Placed};
use barrel_formats::pe::Pe;

use crate::hud::Element;
use crate::kit::{raised, shifted};
use crate::overview::Face;
use crate::title::Button;

pub const DESK: u32 = 383;
pub const FIGURE: u32 = 361;
const ROW: u32 = 362;
const SLOTS: u32 = 365;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const TAB: u32 = 531;
const DESK_CUT: i32 = 1;
const ROW_PART: i32 = 0x16a;
/// The chosen row's frame (part 0x2, the flipbook node of layout 362) and the Friend label and
/// number nodes the lead's row leaves out.
const FRAME: i32 = 0x2;
const FRIEND_NUMBER: usize = 6;
/// The cursor's glow from the chosen row's place (the capture: the glow at (63, 69) with the first
/// row at (78, 152)).
const CURSOR_FROM_ROW: [i32; 2] = [-15, -83];
/// The SysInfo.xml row ("Please select a character.") and the Menu_Line_Help.xml row ("Select,
/// Return, Confirm") of the header and footer lines.
pub const HEADER_ROW: usize = 37;
pub const FOOTER_ROW: usize = 8;

/// The command the picker opens for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    AdventureEquip,
    BattleEquip,
    Status,
}

impl Kind {
    /// The command's cut in the tab's title and icon parts (the command's place in the list).
    pub fn tab_cut(self) -> i32 {
        match self {
            Kind::AdventureEquip => 2,
            Kind::BattleEquip => 3,
            Kind::Status => 5,
        }
    }

    /// The command the picker belongs to.
    pub fn command(self) -> crate::menu::Command {
        use crate::menu::Command;
        match self {
            Kind::AdventureEquip => Command::AdventureEquip,
            Kind::BattleEquip => Command::BattleEquip,
            Kind::Status => Command::Status,
        }
    }

    /// How many of the `party` members it lists.
    pub fn count(self, party: usize) -> usize {
        match self {
            Kind::AdventureEquip => party.min(2),
            _ => party,
        }
    }

    /// The characters it lists out of the party in `members`: Adventure Equip. only the two
    /// alchemists (Escha, 0, and Logy, 1).
    pub fn lists(self, members: &[usize]) -> Vec<usize> {
        match self {
            Kind::AdventureEquip => members.iter().copied().filter(|&c| c < 2).collect(),
            _ => members.to_vec(),
        }
    }
}

/// The picker's cursor over the `count` members listed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Picker {
    pub chosen: usize,
    pub count: usize,
}

impl Picker {
    /// A button press; whether the page closes.
    pub fn press(&mut self, b: Button) -> bool {
        let n = self.count.max(1);
        match b {
            Button::Up => self.chosen = (self.chosen + n - 1) % n,
            Button::Down => self.chosen = (self.chosen + 1) % n,
            Button::Back => return true,
            _ => {}
        }
        false
    }
}

/// The page's two lines of words.
pub struct Words {
    pub header: String,
    pub footer: String,
}

/// The layouts the picker is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    figure: Vec<Placed>,
    row: Vec<Placed>,
    /// Where the rows go (layout 365's plates).
    slots: Vec<[i32; 2]>,
    pub header: Vec<Placed>,
    pub footer: Vec<Placed>,
    pub tab: Vec<Placed>,
    cursor: Option<layout::Flip>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let slots = layout::nodes(pe, SLOTS).map_or_else(Vec::new, |(_, nodes)| nodes.iter().filter(|n| n.part == ROW_PART).map(|n| n.at).collect());
        let get = |id| layout::placed(pe, id);
        Layouts { desk: get(DESK), figure: get(FIGURE), row: get(ROW), slots, header: get(HEADER), footer: get(FOOTER), tab: get(TAB), cursor: crate::kit::cursor_glow(pe) }
    }

    /// Every cut the picker's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        let mut out = crate::kit::flip_elements(&self.row);
        out.extend(self.cursor.into_iter().flat_map(|f| crate::kit::cuts_of(crate::kit::CURSOR, f)));
        out
    }
}

/// The footer, header and tab every page of this family ends with: the lines' plates with
/// their words, and the command's tab (its title and icon at `tab_cut`).
pub fn chrome(pe: &Pe, footer: (&[Placed], &str), header: (&[Placed], &str), tab: &[Placed], tab_cut: i32) -> Vec<Element> {
    let line = |nodes: &[Placed], text: &str| -> Vec<Element> {
        nodes.iter().flat_map(|p| match layout::text(pe, p) {
            Some(t) => vec![raised(t, text)],
            None => vec![Element::stored(p)],
        }).collect()
    };
    let mut out = line(footer.0, footer.1);
    out.extend(line(header.0, header.1));
    for p in tab {
        out.push(match Element::stored(p) {
            Element::Sprite { part, at, .. } if part == 0x2e || part == 0x2c => Element::Sprite { part, cut: tab_cut, at },
            e => e,
        });
    }
    out
}

/// The row of member `face` at `slot`: the nodes of layout 362 at the slot's place, the lead's
/// without "Friend", the chosen one's with its frame cut by `tick`.
fn row(pe: &Pe, l: &Layouts, slot: [i32; 2], face: &Face, lead: bool, chosen: bool, tick: u32) -> Vec<Element> {
    let nodes: Vec<Placed> = l.row.iter().map(|p| shifted(p, slot)).collect();
    let texts = |p: &Placed| layout::text(pe, p).filter(|t| !(lead && t.text == "Friend"));
    let own: Vec<&Placed> = nodes.iter().filter(|p| p.node.part != FRAME).collect();
    let numbers = [face.level, face.hp.0, face.hp.1, face.mp.0, face.mp.1, face.next, face.friend];
    let numbers = if lead { &numbers[..FRIEND_NUMBER] } else { &numbers[..] };
    let mut out = crate::overview::face(pe, &own, &texts, &face.name, Some(face.character), numbers);
    if chosen && let Some(p) = nodes.iter().find(|p| p.node.part == FRAME) {
        out.push(Element::Sprite { part: FRAME, cut: p.node.flip().map_or(0, |f| f.cut(tick)), at: p.at });
    }
    out
}

/// All the picker's elements: `faces` the members listed (in order), `lead` the lead's character
/// number, `tick` the clock's 60 Hz ticks.
pub fn elements(pe: &Pe, l: &Layouts, words: &Words, kind: Kind, view: &Picker, faces: &[Face], lead: usize, tick: u32) -> Vec<Element> {
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: DESK_CUT, at: p.at }).collect();
    out.extend(l.figure.iter().map(Element::stored));
    let first = 5usize.saturating_sub(faces.len()) / 2;
    let mut cursor = None;
    for (k, face) in faces.iter().enumerate() {
        let Some(&slot) = l.slots.get(first + k) else { break };
        let chosen = k == view.chosen;
        out.extend(row(pe, l, slot, face, face.character as usize == lead, chosen, tick));
        if chosen {
            cursor = Some([slot[0] + CURSOR_FROM_ROW[0], slot[1] + CURSOR_FROM_ROW[1]]);
        }
    }
    out.extend(chrome(pe, (&l.footer, &words.footer), (&l.header, &words.header), &l.tab, kind.tab_cut()));
    out.extend(cursor.map(|at| Element::Sprite { part: crate::kit::CURSOR, cut: l.cursor.map_or(0, |f| f.cut(tick)), at }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_round_the_list_and_backs_out() {
        let mut p = Picker { chosen: 0, count: 3 };
        assert!(!p.press(Button::Up));
        assert_eq!(p.chosen, 2);
        assert!(!p.press(Button::Down));
        assert_eq!(p.chosen, 0);
        assert!(p.press(Button::Back));
    }

    #[test]
    fn adventure_equip_lists_the_alchemists() {
        assert_eq!(Kind::AdventureEquip.lists(&[0, 1, 2, 3, 4]), vec![0, 1]);
        assert_eq!(Kind::Status.lists(&[0, 1, 2]), vec![0, 1, 2]);
    }
}
