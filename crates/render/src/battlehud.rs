//! The battle HUD, from the game's layout 886 (`a15_battle.g1t`): the turn order along the top
//! (layout 864), the party's plates along the bottom (layout 878: three of layout 870 at x 424,
//! 798 and 1168, each a portrait, layout 869, part 0xf2 cut = the member's number, and two bars,
//! part 0xee cut 11 for HP and cut 10 for MP, 124 pixels wide; the reserve row below, layout
//! 868), the battle cries' bubbles over them (layout 882), the Support gauge (layout 884, with
//! "Fast Forward"), and the target panel (layout 877).
//!
//! Not yet the game's: the plates in the layout's order (the first the lead's: the centre one),
//! a plate's bars cut to the member's share of HP and MP; the turn order, the reserve row, the
//! state icons (layouts 873, 875), the plates' numbers (kind 28), the action banner and the
//! target panel left out; the Support gauge as stored (0).

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::Element;

pub const LAYOUT: u32 = 886;
const PLATE: u32 = 870;
const PORTRAIT: i32 = 0xf2;
const PLATE_PARTS: i32 = 0xee;
const HP_BAR: i32 = 11;
const MP_BAR: i32 = 10;
/// The plate's first node (its frame), which starts each plate in the placed order.
const PLATE_FIRST: (i32, i32) = (PLATE_PARTS, 5);
/// Left out: the turn order, the reserve row, the state icons, the target panel, the battle
/// cries' speech bubbles (layout 882: one over each plate, "…!!") and the action banner (885).
const LEFT_OUT: [u32; 7] = [864, 868, 873, 875, 877, 882, 885];
const NUMBER_KIND: i32 = 28;

/// A party member as its plate shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    /// Its number (0 Escha, 1 Logy, ...): its portrait's cut.
    pub chara: usize,
    pub hp: (i32, i32),
    pub mp: (i32, i32),
}

fn share((now, max): (i32, i32)) -> f32 {
    if max > 0 { now.clamp(0, max) as f32 / max as f32 } else { 0.0 }
}

/// The elements to draw from layout 886's placed nodes (`texts` gives a node's text).
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, members: &[Member]) -> Vec<Element> {
    let mut out = Vec::new();
    // Which plate a node is on (counted as their first nodes come).
    let starts = |p: &&Placed| p.path.last() == Some(&PLATE) && (p.node.part, p.node.cut) == PLATE_FIRST;
    let mut plate: Option<usize> = None;
    for p in nodes {
        let n = p.node;
        if p.path.iter().any(|l| LEFT_OUT.contains(l)) {
            continue;
        }
        let on_plate = p.path.contains(&PLATE);
        if starts(&p) {
            plate = Some(plate.map_or(0, |k| k + 1));
        }
        if on_plate {
            let Some(m) = plate.and_then(|k| members.get(k)) else { continue };
            match (n.part, n.cut) {
                (PORTRAIT, _) => out.push(Element::Sprite { part: PORTRAIT, cut: m.chara as i32, at: p.at }),
                (PLATE_PARTS, HP_BAR) => out.push(Element::Cropped { part: PLATE_PARTS, cut: HP_BAR, at: p.at, fill: share(m.hp) }),
                (PLATE_PARTS, MP_BAR) => out.push(Element::Cropped { part: PLATE_PARTS, cut: MP_BAR, at: p.at, fill: share(m.mp) }),
                _ if n.kind == NUMBER_KIND => {}
                _ => out.push(Element::stored(p)),
            }
            continue;
        }
        if n.part == 0x164 {
            if let Some(t) = texts(p).filter(|t| !t.text.is_empty() && !t.filled_in()) {
                out.push(Element::Label(t));
            }
            continue;
        }
        if n.kind == NUMBER_KIND {
            continue;
        }
        out.push(Element::stored(p));
    }
    out
}

/// Where the command list stands and its lines' spacing (not the game's command layout, which
/// is not identified yet).
const LIST_AT: [i32; 2] = [1500, 360];
const LIST_STEP: i32 = 44;
const LIST_SIZE: f32 = 33.0;

/// A member's command list (`list`: each line and whether it can be given), the chosen line
/// gold and those that cannot be given grey, under a title (who acts, on whom): plain text right of centre.
pub fn command_list(list: &[(String, bool)], chosen: usize, title: &str) -> Vec<Element> {
    let line = |text: String, at: [i32; 2], colour: [u8; 4]| Element::Label(Text { at, size: LIST_SIZE, colour, text, kind: 21 });
    let mut out = vec![line(title.to_string(), LIST_AT, [0xff, 0xff, 0xff, 0xff])];
    for (k, (text, can)) in list.iter().enumerate() {
        let colour = if !*can { [0xff, 0x90, 0x90, 0x90] } else if k == chosen { [0xff, 0xff, 0xd0, 0x40] } else { [0xff, 0xff, 0xff, 0xff] };
        out.push(line(text.clone(), [LIST_AT[0], LIST_AT[1] + LIST_STEP * (k as i32 + 1)], colour));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_are_clamped() {
        assert_eq!(share((5, 10)), 0.5);
        assert_eq!(share((-3, 10)), 0.0);
        assert_eq!(share((3, 0)), 0.0);
    }
}
