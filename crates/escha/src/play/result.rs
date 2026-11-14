//! A won battle's Result screen, from the game's layout 898 (`a15_battle.g1t`, in layout 904):
//! the "Result" heading (layout 892), one row per party member in the battle (layout 895: a
//! plate, part 0x101 cut 7; the member's portrait, part 0x4, cut = its number; "Lv" and "Next"
//! with their numbers, part 0x4c digits, 24 wide, each number ending at its node; "Level Up!",
//! layout 896, beside a member that went up), then the battle's experience (layout 893) and
//! money ("Cole", layout 894, with the coin), part 0x4b digits, 36 wide.
//!
//! As the running game shows it (`docs/formats.md`, "The battle result"): over the battle
//! scene, without the desk (layout 901) or the level-up panel (layout 900).
//!
//! Not yet the game's: the lead's victory pose, the rows' and numbers' motion, "Skill Get!"
//! (layout 897, no skills learnt yet), the level-up panel, the drops' screen that follows.

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::Element;

pub const LAYOUT: u32 = 904;
const LIST: u32 = 898;
const ROW: u32 = 895;
const LEVEL_UP: u32 = 896;
const SKILL_GET: u32 = 897;
const ROW_PLATE: (i32, i32) = (0x101, 7);
const PORTRAIT: i32 = 0x4;
const SMALL_DIGITS: (i32, i32) = (0x4c, 24);
const LARGE_DIGITS: (i32, i32) = (0x4b, 36);
const NUMBER_KIND: i32 = 28;

/// One party member's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Its number (0 Escha, 1 Logy, ...): its portrait's cut.
    pub chara: usize,
    pub level: i32,
    /// The experience still needed for the next level.
    pub next: i32,
    pub level_up: bool,
}

/// What the screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Values {
    pub rows: Vec<Row>,
    pub exp: i32,
    pub money: i32,
}

/// A number's digit sprites, the last one ending at `end`.
fn digits(value: i32, (part, advance): (i32, i32), end: i32, y: i32) -> Vec<Element> {
    let digits: Vec<i32> = value.max(0).to_string().bytes().map(|b| i32::from(b - b'0')).collect();
    let n = digits.len() as i32;
    digits.into_iter().enumerate().map(|(i, d)| Element::Sprite { part, cut: d, at: [end - advance * (n - i as i32), y] }).collect()
}

/// The elements to draw from layout 904's placed nodes (`texts` gives a node's text).
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, v: &Values) -> Vec<Element> {
    let mut out = Vec::new();
    // Which row a node is on (counted as the rows' plates come), and which of a row's two
    // numbers comes next.
    let mut row: Option<usize> = None;
    let mut number = 0;
    for p in nodes.iter().filter(|p| p.path.get(1) == Some(&LIST)) {
        let n = p.node;
        let on_row = p.path.contains(&ROW);
        if on_row && p.path.last() == Some(&ROW) && (n.part, n.cut) == ROW_PLATE {
            row = Some(row.map_or(0, |k| k + 1));
            number = 0;
        }
        if on_row {
            let Some(r) = row.and_then(|k| v.rows.get(k)) else { continue };
            if p.path.contains(&SKILL_GET) || (p.path.contains(&LEVEL_UP) && !r.level_up) {
                continue;
            }
            if n.part == PORTRAIT {
                out.push(Element::Sprite { part: PORTRAIT, cut: r.chara as i32, at: p.at });
                continue;
            }
            if n.part == SMALL_DIGITS.0 && n.kind == NUMBER_KIND {
                out.extend(digits(if number == 0 { r.level } else { r.next }, SMALL_DIGITS, p.at[0], p.at[1]));
                number += 1;
                continue;
            }
        } else if n.part == LARGE_DIGITS.0 && n.kind == NUMBER_KIND {
            // The experience's number, then the money's (their layouts' order).
            let value = if p.path.contains(&893) { v.exp } else { v.money };
            out.extend(digits(value, LARGE_DIGITS, p.at[0], p.at[1]));
            continue;
        }
        if n.part == 0x164 {
            if let Some(t) = texts(p).filter(|t| !t.text.is_empty() && !t.filled_in()) {
                out.push(Element::Label(t));
            }
            continue;
        }
        out.push(Element::stored(p));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_end_at_their_node() {
        let d = digits(72, SMALL_DIGITS, 510, 192);
        assert_eq!(d.len(), 2);
        assert!(matches!(d[0], Element::Sprite { cut: 7, at: [462, 192], .. }));
        assert!(matches!(d[1], Element::Sprite { cut: 2, at: [486, 192], .. }));
    }
}
