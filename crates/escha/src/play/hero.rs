//! The hero choice after the title's New Game, from the game's layout 1023
//! (`WinXls_EN/a15_selection.g1t` on the menu background): the two figures (layouts 1021,
//! Escha's, at the left, and 1022, Logy's, at the right: parts 0x138 and 0x139, cut 0 a grey
//! sketch at half alpha, cut 1 in colour), the two profile
//! cards (Logy's, layout 1025, at the left; Escha's, 1024, at the right), the arrows, the
//! bottom panel (1018: a parchment whose right half is its left half mirrored, between two
//! ropes, one the other mirrored) and "Select your character.", then, to confirm, the Yes / No
//! tags (layouts 1013 and 1014) and "Do you want to use this character?" (`SysMess.xml` row 80).
//!
//! Matched against a screenshot of the game's confirm step (Escha chosen, No lit): the desk is
//! part 0 cut 1 (book at the left, scissors at the right) rather than the layout's cut 0 and
//! its desk overlay (layout 357), which the game evidently swaps at run time; the cards and
//! arrows are gone, the chosen figure is drawn in colour (cut 1) 300 pixels right
//! of its place (x 290 against the layout's -10; y as stored), the tags are at layout 1023's
//! places (No at [1212, 504] exactly), the lit tag is cut 1 of part 0x13d and the words are
//! cut 1 (plain) of parts 0x13e / 0x13f, the question takes the "Select your character." node
//! (kind 22, centred), and neither the tags' clips (part 0x140) nor the button mark (layout 694)
//! show. Known from the oracle runs' button path: the choice opens on Escha, Yes is the left tag.
//! Not yet the game's: Logy's figure when confirming (moved 300 pixels left, mirroring Escha's;
//! no screenshot), the Yes tag drawn level (kind 7: the game tilts it), the chosen card drawn
//! whole and the other at half strength, the step the oracle path's second A passes (left out),
//! the "Cleared" stamp (cut 3 of the cards) never shown, no motion.

use barrel_escha::data::layout::Placed;

use crate::hud::Element;
use crate::title::Button;

pub const LAYOUT: u32 = 1023;
const ESCHA_CARD: u32 = 1024;
const LOGY_CARD: u32 = 1025;
const ESCHA_FIGURE: u32 = 1021;
const LOGY_FIGURE: u32 = 1022;
const YES: u32 = 1013;
const NO: u32 = 1014;
const BUTTON_MARK: u32 = 694;
/// The desk overlay the layout nests (layout 357: `a15_menu.g1t` cuts 4 and 5 of part 0).
const DESK_OVERLAY: u32 = 357;
/// The desk picture the game shows under the hero choice: part 0 cut 1 (the layout names cut 0).
const DESK: i32 = 0;
const DESK_CUT: i32 = 1;
const CARD_PART: i32 = 0x141;
const STAMP_CUT: i32 = 3;
const ARROWS: [i32; 2] = [0x142, 0x143];
const TAG: i32 = 0x13d;
const TAG_WORDS: [i32; 2] = [0x13e, 0x13f];
const CLIP: i32 = 0x140;
/// How far the chosen figure moves right (Escha's; Logy's moves as far left) to confirm.
const FIGURE_SHIFT: i32 = 300;
/// The figures' cut in colour (cut 0 is a grey sketch at half alpha).
const COLOUR_FIGURE: i32 = 1;
/// `SysMess.xml`'s row asking to confirm.
pub const QUESTION_ROW: usize = 80;

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

    /// The elements to draw, from layout 1023's placed nodes (`texts` gives a node's text;
    /// `question` is the confirm step's line).
    pub fn elements(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<barrel_escha::data::layout::Text>, question: &str) -> Vec<Element> {
        let (logy, yes) = match *self {
            Hero::Choose { logy } => (logy, None),
            Hero::Confirm { logy, yes } => (logy, Some(yes)),
        };
        let confirming = yes.is_some();
        let mut out = Vec::new();
        for p in nodes {
            let n = p.node;
            if n.part == 0x164 {
                let Some(t) = texts(p).filter(|t| !t.text.is_empty()) else { continue };
                let text = if confirming { question.to_string() } else { t.text.clone() };
                out.push(Element::Label(barrel_escha::data::layout::Text { text, ..t }));
                continue;
            }
            let (part, cut, at) = (n.part, n.cut.max(0), p.at);
            let branch = p.path.get(1).copied();
            let e = match branch {
                Some(ESCHA_CARD) | Some(LOGY_CARD) if confirming || (part == CARD_PART && cut == STAMP_CUT) => continue,
                Some(ESCHA_CARD) => if logy { Element::Dim { part, cut, at } } else { Element::Sprite { part, cut, at } },
                Some(LOGY_CARD) => if logy { Element::Sprite { part, cut, at } } else { Element::Dim { part, cut, at } },
                Some(ESCHA_FIGURE) | Some(LOGY_FIGURE) if confirming => {
                    let escha = branch == Some(ESCHA_FIGURE);
                    if escha == logy {
                        continue;
                    }
                    let shift = if escha { FIGURE_SHIFT } else { -FIGURE_SHIFT };
                    Element::Sprite { part, cut: COLOUR_FIGURE, at: [at[0] + shift, at[1]] }
                }
                Some(YES) | Some(NO) => {
                    let Some(y) = yes else { continue };
                    let lit = y == (branch == Some(YES));
                    match part {
                        TAG => Element::Sprite { part, cut: i32::from(lit), at },
                        CLIP => continue,
                        w if TAG_WORDS.contains(&w) => Element::Sprite { part, cut: 1, at },
                        _ => Element::stored(p),
                    }
                }
                Some(BUTTON_MARK) | Some(DESK_OVERLAY) => continue,
                None if part == DESK => Element::Sprite { part, cut: DESK_CUT, at },
                _ if confirming && ARROWS.contains(&part) => continue,
                _ => Element::stored(p),
            };
            out.push(e);
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
    use barrel_escha::data::layout::Node;

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
            node(&[LAYOUT, ESCHA_FIGURE], 0x138, 0),
            node(&[LAYOUT, LOGY_FIGURE], 0x139, 0),
            node(&[LAYOUT, ESCHA_CARD], CARD_PART, 0),
            node(&[LAYOUT, ESCHA_CARD], CARD_PART, STAMP_CUT),
            node(&[LAYOUT, LOGY_CARD], CARD_PART, 0),
            node(&[LAYOUT], ARROWS[0], 0),
            node(&[LAYOUT, YES], TAG, 0),
            node(&[LAYOUT, YES], CLIP, 0),
            node(&[LAYOUT, YES], TAG_WORDS[0], 0),
            node(&[LAYOUT, NO], TAG, 0),
        ];
        let e = Hero::new().elements(&nodes, |_| None, "?");
        let s = |part, cut, at| Element::Sprite { part, cut, at };
        assert_eq!(e, vec![
            s(0x138, 0, [0, 0]),
            s(0x139, 0, [0, 0]),
            s(CARD_PART, 0, [0, 0]),
            Element::Dim { part: CARD_PART, cut: 0, at: [0, 0] },
            s(ARROWS[0], 0, [0, 0]),
        ]);
        // Confirming Escha with Yes lit: her figure whole and moved, the tags, no cards.
        let e = Hero::Confirm { logy: false, yes: true }.elements(&nodes, |_| None, "?");
        assert_eq!(e, vec![
            s(0x138, COLOUR_FIGURE, [FIGURE_SHIFT, 0]),
            s(TAG, 1, [0, 0]),
            s(TAG_WORDS[0], 1, [0, 0]),
            s(TAG, 0, [0, 0]),
        ]);
    }
}
