//! The hero choice after the title's New Game, from the game's layout 1023
//! (`WinXls_EN/a15_selection.g1t` on the menu background): the two figures, the two profile
//! cards (Logy's, layout 1025, at the left; Escha's, 1024, at the right), the arrows and
//! "Select your character.", then the Yes / No tags (layouts 1013 and 1014) to confirm.
//!
//! Read: the pictures and their places. Known from the game (the oracle runs' button path, A on
//! Escha's card, A, D-pad left for Yes, A): the choice opens on Escha, and Yes is the left tag.
//! Not yet the game's: the chosen card drawn whole and the other at half strength, the arrows
//! and left / right switching, the tags appearing only to confirm, the step the oracle path's
//! second A passes (left out), and the "Cleared" stamp (cut 3 of the cards) never shown.

use barrel_formats::layout::Placed;

use crate::hud::Element;
use crate::title::Button;

pub const LAYOUT: u32 = 1023;
const ESCHA_CARD: u32 = 1024;
const LOGY_CARD: u32 = 1025;
const YES: u32 = 1013;
const NO: u32 = 1014;
const CARD_PART: i32 = 0x141;
const STAMP_CUT: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hero {
    /// Choosing a card: Logy's when true.
    Choose { logy: bool },
    /// Asked to confirm: Yes lit when true.
    Confirm { logy: bool, yes: bool },
}

/// What a button did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The lead is chosen: Logy when true.
    Chosen(bool),
    /// Back to the title.
    Back,
}

impl Hero {
    pub fn new() -> Hero {
        Hero::Choose { logy: false }
    }

    pub fn press(&mut self, b: Button) -> Option<Outcome> {
        match (*self, b) {
            (Hero::Choose { logy }, Button::Left | Button::Right | Button::Up | Button::Down) => *self = Hero::Choose { logy: !logy },
            (Hero::Choose { logy }, Button::Confirm) => *self = Hero::Confirm { logy, yes: false },
            (Hero::Choose { .. }, Button::Back) => return Some(Outcome::Back),
            (Hero::Confirm { logy, .. }, Button::Left) => *self = Hero::Confirm { logy, yes: true },
            (Hero::Confirm { logy, .. }, Button::Right) => *self = Hero::Confirm { logy, yes: false },
            (Hero::Confirm { .. }, Button::Up | Button::Down) => {}
            (Hero::Confirm { logy, yes: true }, Button::Confirm) => return Some(Outcome::Chosen(logy)),
            (Hero::Confirm { logy, .. }, Button::Confirm | Button::Back) => *self = Hero::Choose { logy },
        }
        None
    }

    /// The elements to draw, from layout 1023's placed nodes (`texts` gives a node's text).
    pub fn elements(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<barrel_formats::layout::Text>) -> Vec<Element> {
        let (logy, yes) = match *self {
            Hero::Choose { logy } => (logy, None),
            Hero::Confirm { logy, yes } => (logy, Some(yes)),
        };
        let mut out = Vec::new();
        for p in nodes {
            let n = p.node;
            if n.part == 0x164 {
                out.extend(texts(p).filter(|t| !t.text.is_empty()).map(Element::Label));
                continue;
            }
            let (part, cut, at) = (n.part, n.cut.max(0), p.at);
            let branch = p.path.get(1).copied();
            let lit = match branch {
                Some(ESCHA_CARD) | Some(LOGY_CARD) if part == CARD_PART && cut == STAMP_CUT => continue,
                Some(ESCHA_CARD) => !logy,
                Some(LOGY_CARD) => logy,
                Some(YES) => match yes {
                    None => continue,
                    Some(y) => y,
                },
                Some(NO) => match yes {
                    None => continue,
                    Some(y) => !y,
                },
                _ => true,
            };
            out.push(if lit { Element::Sprite { part, cut, at } } else { Element::Dim { part, cut, at } });
        }
        out
    }
}

impl Default for Hero {
    fn default() -> Hero {
        Hero::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::layout::Node;

    #[test]
    fn choose_then_confirm_yes_on_the_left() {
        // The oracle path: A on Escha's card, D-pad left (Yes), A.
        let mut h = Hero::new();
        assert_eq!(h.press(Button::Confirm), None);
        assert_eq!(h.press(Button::Left), None);
        assert_eq!(h.press(Button::Confirm), Some(Outcome::Chosen(false)));
        // Logy: switch, confirm, No goes back.
        let mut h = Hero::new();
        h.press(Button::Right);
        h.press(Button::Confirm);
        assert_eq!(h.press(Button::Confirm), None);
        assert_eq!(h, Hero::Choose { logy: true });
        assert_eq!(h.press(Button::Back), Some(Outcome::Back));
    }

    #[test]
    fn cards_and_tags() {
        let node = |path: &[u32], part, cut| Placed { path: path.to_vec(), node: Node { part, cut, at: [0, 0], kind: 0 }, at: [0, 0], va: 0 };
        let nodes = vec![
            node(&[LAYOUT, ESCHA_CARD], CARD_PART, 0),
            node(&[LAYOUT, ESCHA_CARD], CARD_PART, STAMP_CUT),
            node(&[LAYOUT, LOGY_CARD], CARD_PART, 0),
            node(&[LAYOUT, YES], 0x13d, 0),
        ];
        let e = Hero::new().elements(&nodes, |_| None);
        assert_eq!(e, vec![Element::Sprite { part: CARD_PART, cut: 0, at: [0, 0] }, Element::Dim { part: CARD_PART, cut: 0, at: [0, 0] }]);
        let e = Hero::Confirm { logy: false, yes: true }.elements(&nodes, |_| None);
        assert_eq!(e.last(), Some(&Element::Sprite { part: 0x13d, cut: 0, at: [0, 0] }));
    }
}
