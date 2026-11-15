//! The battle HUD, from the game's layout 886 (`a15_battle.g1t`): the turn order along the top
//! (layout 864), the party's plates along the bottom (layout 878: three of layout 870 at x 424,
//! 798 and 1168, each a portrait, layout 869, part 0xf2 cut = the member's number, and two bars,
//! part 0xee cut 11 for HP and cut 10 for MP, 124 pixels wide; the reserve row below, layout
//! 868), the battle cries' bubbles over them (layout 882), the Support gauge (layout 884, with
//! "Fast Forward"), and the target panel (layout 877).
//!
//! The turn order bar (864, placed at (320, 8)): 15 hanging slots, the next to act in the
//! rightmost, and the Action slot (its node of kind 4 at 120%) with the one acting now; a slot
//! in use shows layout 860's frame (0xf4 cut 7) and the fighter's portrait (0xe6, cut = the
//! party member's number, 0 Escha to 10 Nio; a monster's 11 + its monster number: Stray Dog's
//! wolf at 15, Fairydrake at 21, Ore Rat at 47, Fake Ruin's blue blocks at 55 as the recording).
//!
//! Not yet the game's: the plates in the layout's order (the first the lead's: the centre one),
//! a plate's bars cut to the member's share of HP and MP; the reserve row, the
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
const LEFT_OUT: [u32; 6] = [868, 873, 875, 877, 882, 885];
/// The turn order bar and its slots (layout 860, the 16th the Action slot).
const TURN_BAR: u32 = 864;
const SLOT: u32 = 860;
const SLOTS: usize = 15;
const SLOT_FIRST: (i32, i32) = (0xf4, 7);
/// A slot's target marks, shown on the target's next turn while a member picks it (the
/// ruins-entrance-battle-stats recording at 322 s): the gold frame (layout 861 nested at
/// [-6, -4], part 0xf3) and the arrow under the slot (part 0x111 at [16, 76]).
const SLOT_FRAME: u32 = 861;
/// The next-turn mark while a member chooses (layout 865, nested in the bar at [938, 130]): the
/// line (part 0x10a, spanned) and the glint (part 0xf3 cut 4, at [-16, -18] off it). The game
/// (`0x1401900c0` → `0x1401cdf20`) puts it at bar x 1012 − 63 × n + 60 (n the entries due
/// before the actor's next turn; n 0: x 1032 + 60), and spans the line to bar x 1071 + 15
/// under the Action slot (the recording at 322 s: from 636 to 1406 on the screen).
const NEXT_MARK: u32 = 865;
const NEXT_LINE: (i32, i32) = (0x10a, 0);
const NEXT_GLINT: (i32, i32) = (0xf3, 4);
const NEXT_LINE_END: i32 = 1071 + 15;

/// The next-turn mark's x in the bar, for `n` entries due before it.
pub fn next_mark_x(n: usize) -> i32 {
    if n == 0 { 1032 + 60 } else { 1012 - 63 * n as i32 + 60 }
}
const SLOT_ARROW: i32 = 0x111;
const FACE: i32 = 0xe6;
/// The first monster's portrait cut (the party's 11 before it).
pub const MONSTER_FACES: usize = 11;
/// The Action slot's scale (its node's word 5) about its corner, (1032, 12) in the bar.
const ACTION_PERCENT: i32 = 120;
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
/// `order`: the turn order, the one acting first, each a portrait cut (`None`: no portrait).
/// The battle's Fast Forward: the game's global `0x141077da0` (0 off, 1, 2), stepped by L3 as
/// `(n + 1) % 3` (`0x14019ee90`), the battle's speed from the table at `0x140d772c0` and the
/// label's English text from `0x1401cffa0`'s choice (the layout's own "Fast Forward(×1.5)" is a
/// placeholder).
pub const FAST_FORWARD_SPEEDS: [f32; 3] = [1.0, 1.5, 2.0];
pub const FAST_FORWARD_LABELS: [&str; 3] = ["Fast Forward Off", "Fast Forward(x1.5)", "Fast Forward(x2)"];

/// `marked`: the place in `order` of the target's next turn, while one is being picked.
/// `next`: the next-turn mark's entry count while a member chooses.
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, members: &[Member], order: &[Option<usize>], fast_forward: usize, marked: Option<usize>, next: Option<usize>) -> Vec<Element> {
    let mut out = Vec::new();
    // The turn order's slot a node is in (counted as their first nodes come), and that slot's
    // corner (the Action slot's scaling centre).
    let mut slot: Option<usize> = None;
    let mut slot_at = [0, 0];
    // Which plate a node is on (counted as their first nodes come).
    let starts = |p: &&Placed| p.path.last() == Some(&PLATE) && (p.node.part, p.node.cut) == PLATE_FIRST;
    let mut plate: Option<usize> = None;
    for p in nodes {
        let n = p.node;
        if p.path.iter().any(|l| LEFT_OUT.contains(l)) {
            continue;
        }
        if p.path.contains(&TURN_BAR) {
            // The order's place slot k shows: the Action slot the one acting, slot k of the 15
            // the (15 - k)th after it.
            let place = |k: usize| if k >= SLOTS { 0 } else { SLOTS - k };
            if p.path.last() == Some(&NEXT_MARK) {
                if let Some(n) = next {
                    // The layout puts the mark at its node; the game moves it by n.
                    // The mark's own origin (the layout's at bar x 938), moved to the game's x.
                    let at = [p.at[0] - 938 + next_mark_x(n), p.at[1]];
                    match (p.node.part, p.node.cut) {
                        NEXT_LINE => out.push(Element::Spanned { part: NEXT_LINE.0, cut: NEXT_LINE.1, at, width: NEXT_LINE_END - next_mark_x(n) }),
                        NEXT_GLINT => out.push(Element::Sprite { part: NEXT_GLINT.0, cut: NEXT_GLINT.1, at }),
                        _ => {}
                    }
                }
                continue;
            }
            if p.path.last() != Some(&SLOT) {
                if !p.path.contains(&SLOT) {
                    out.push(Element::stored(p));
                } else if p.path.last() == Some(&SLOT_FRAME) && slot.is_some_and(|k| marked == Some(place(k))) {
                    out.push(Element::Sprite { part: n.part, cut: n.cut.max(0), at: p.at });
                }
                continue;
            }
            if (n.part, n.cut) == SLOT_FIRST {
                slot = Some(slot.map_or(0, |k| k + 1));
                slot_at = p.at;
            }
            let Some(k) = slot else { continue };
            if n.part == SLOT_ARROW {
                if marked == Some(place(k)) {
                    out.push(Element::Sprite { part: n.part, cut: n.cut.max(0), at: p.at });
                }
                continue;
            }
            let Some(&face) = order.get(place(k)) else { continue };
            // A free entry (no fighter) leaves its slot empty, the bar showing through.
            let (part, cut) = match (n.part, n.cut) {
                SLOT_FIRST if face.is_none() => continue,
                SLOT_FIRST => SLOT_FIRST,
                (FACE, _) => match face {
                    Some(c) => (FACE, c as i32),
                    None => continue,
                },
                _ => continue,
            };
            out.push(if k >= SLOTS {
                let at = [0, 1].map(|i| slot_at[i] + (p.at[i] - slot_at[i]) * ACTION_PERCENT / 100);
                Element::Scaled { part, cut, at, percent: ACTION_PERCENT }
            } else {
                Element::Sprite { part, cut, at: p.at }
            });
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
                let text = if t.text.starts_with("Fast Forward") { FAST_FORWARD_LABELS[fast_forward.min(2)].to_string() } else { t.text.clone() };
                out.push(Element::Label(Text { text, ..t }));
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

/// The command row (layout 887: nine places of layout 859 at y 584, 104 to 106 apart; a
/// member's commands take the first ones, centred on the screen: the battle-stats recording at
/// about 300 s, Katla's six centred at x 958). Each command's icon is part 0xe7 and its label
/// part 0xec, cut by command in this order.
pub mod command {
    pub const ATTACK: i32 = 0;
    pub const ITEM: i32 = 1;
    pub const DRAW: i32 = 2;
    pub const SKILL: i32 = 3;
    pub const SPECIAL: i32 = 4;
    pub const MOVE: i32 = 5;
    pub const WAIT: i32 = 6;
    pub const CHANGE: i32 = 7;
    pub const RUN: i32 = 8;
}
const ROW_X: [i32; 9] = [420, 524, 630, 736, 840, 944, 1050, 1156, 1260];
const ROW_Y: i32 = 584;
/// A place's centre from its node (layout 859's icon, 856, at (98, 56)).
const ROW_CENTRE: [i32; 2] = [98, 56];
const ICON: i32 = 0xe7;
const ROW_PARTS: i32 = 0xe8;
const LABEL: i32 = 0xec;

/// The command row: `commands` (icon cuts) in their places, the chosen one on the diamond
/// (layout 857) with the arrows either side and its label on a plate below (858), the others
/// in their dark frames (855).
///
/// The diamond and the arrows are nodes of kind 36, a flipbook of word 5 cuts from the node's
/// (4: the diamond cuts 2 to 5, the arrows 6 to 9 and 10 to 13); drawn at their first frame
/// until the HUD has a clock.
pub fn command_row(commands: &[i32], chosen: usize) -> Vec<Element> {
    let n = commands.len().clamp(1, ROW_X.len());
    let shift = 960 - ((ROW_X[0] + ROW_X[n - 1]) / 2 + ROW_CENTRE[0]);
    let mut out = Vec::new();
    // The chosen one last: its arrows lie over its neighbours (the recording's right arrow over
    // the Skill icon).
    let order = (0..n).filter(|&k| k != chosen).chain((chosen < n).then_some(chosen));
    for k in order {
        let c = commands[k];
        let node = [ROW_X[k] + shift, ROW_Y];
        let at = |x: i32, y: i32| [node[0] + x, node[1] + y];
        if k == chosen {
            out.push(Element::Sprite { part: ROW_PARTS, cut: 2, at: at(96 - 66, 50 - 72) });
            out.push(Element::Sprite { part: ICON, cut: c, at: at(ROW_CENTRE[0] - 48, ROW_CENTRE[1] - 48) });
            out.push(Element::Sprite { part: ROW_PARTS, cut: 6, at: at(-12, 20) });
            out.push(Element::Sprite { part: ROW_PARTS, cut: 10, at: at(154, 20) });
            out.push(Element::Sprite { part: LABEL, cut: 9, at: at(4, 100) });
            out.push(Element::Sprite { part: LABEL, cut: c, at: at(48, 112) });
        } else {
            out.push(Element::Sprite { part: ROW_PARTS, cut: 0, at: at(96 - 48, 54 - 48) });
            out.push(Element::Sprite { part: ICON, cut: c, at: at(ROW_CENTRE[0] - 48, ROW_CENTRE[1] - 48) });
        }
    }
    out
}

/// What a member's turn shows: the command row (icon cuts) and the place on it, the skills'
/// list with its choice once Skill is picked, and a title (who acts, on whom while choosing).
#[derive(Clone, Debug, PartialEq)]
pub struct Commands {
    pub row: Vec<i32>,
    pub at: usize,
    pub skills: Option<(Vec<(String, bool)>, usize)>,
    pub title: String,
    /// The target's plate while choosing it, and the place in the turn order of its next turn.
    pub plate: Option<Plate>,
    pub marked: Option<usize>,
    /// The next-turn mark's entry count (`Turn::next_place`).
    pub next: usize,
}

/// The target plate (layout 920: the frame, part 0x102 cut 3, the red HP bar, cut 6, and the
/// name): over the target while a member chooses it (the ruins-entrance-battle-stats recording
/// at 322 and 354 s: "Fake Ruin" with its HP, at the same height whichever monster, its left
/// edge near the monster's place on screen).
pub const TARGET_PLATE: u32 = 920;
/// The plate's top on the 1920 × 1080 screen, and its left edge off the target's place (both
/// measured on the recording; the game's own placing code not read yet).
pub const PLATE_TOP: i32 = 186;
pub const PLATE_LEFT: i32 = -8;

#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub name: String,
    pub hp: (i32, i32),
    /// The target's place on the 1920 × 1080 screen.
    pub at: [i32; 2],
}

/// The target plate's elements from layout 920's placed nodes (`texts` gives a node's text). Its
/// state icons (nested layouts 917 to 919) and the strip under them (cut 4) wait for states.
pub fn target_plate(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, plate: &Plate) -> Vec<Element> {
    let origin = [plate.at[0] + PLATE_LEFT, PLATE_TOP];
    let at = |p: &Placed| [origin[0] + p.at[0], origin[1] + p.at[1]];
    let mut out = Vec::new();
    for p in nodes.iter().filter(|p| p.path.len() == 1) {
        match (p.node.part, p.node.cut) {
            (0x102, 3) => out.push(Element::Sprite { part: 0x102, cut: 3, at: at(p) }),
            (0x102, 6) => {
                let fill = if plate.hp.1 > 0 { (plate.hp.0.max(0) as f32 / plate.hp.1 as f32).min(1.0) } else { 0.0 };
                out.push(Element::Cropped { part: 0x102, cut: 6, at: at(p), fill });
            }
            (0x164, _) => {
                if let Some(t) = texts(p) {
                    out.push(Element::Label(Text { at: at(p), text: plate.name.clone(), ..t }));
                }
            }
            _ => {}
        }
    }
    out
}

/// Where the command list stands and its lines' spacing (the skills' list, until the game's
/// skill window is in).
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

/// The battle's numbers over a fighter hit or healed: part 0xe0 (`a15_battle.g1t` picture 1, a
/// row of white brush digits with a dark edge, 36 × 60 cuts: 0 to 9, then 10 `+`, 11 `-`, 12
/// `/`, 13 `%`, 14 "MP"), the sign first, the glyphs 36 apart and centred on the fighter's
/// place (as a kind 29 node centres its number). Measured on the ruins-entrance-battle-result
/// recording (149.8 s, "-27" on a Neck Hunt): drawn at full size (the "2"'s white 26 × 34, 32
/// tall there; "-27" 91 wide), it fades in over 0.1 s while shrinking from about 160% to its
/// size by 0.2 s, and stays until the camera moves on, some 0.85 s; the game's drawing code is
/// not found yet (`docs/escha/open.md`).
pub const NUMBER_DIGITS: i32 = 0xe0;
const NUMBER_PLUS: i32 = 10;
const NUMBER_MINUS: i32 = 11;
const NUMBER_ADVANCE: f32 = 36.0;
/// How long a number shows, and its fade in, pop and fade out (seconds).
pub const NUMBER_SECONDS: f32 = 1.0;
const NUMBER_FADE_IN: f32 = 0.1;
const NUMBER_POP: f32 = 0.2;
const NUMBER_FADE_OUT: f32 = 0.15;
const NUMBER_POP_PERCENT: f32 = 160.0;

/// `value` (negative: damage, "-27"; positive: a heal, "+12") centred on `centre` (the 1920 ×
/// 1080 screen), `age` seconds after it came up; nothing once [`NUMBER_SECONDS`] have passed.
pub fn number(value: i32, centre: [i32; 2], age: f32) -> Vec<Element> {
    if !(0.0..NUMBER_SECONDS).contains(&age) {
        return Vec::new();
    }
    let mut cuts = vec![if value < 0 { NUMBER_MINUS } else { NUMBER_PLUS }];
    cuts.extend(value.unsigned_abs().to_string().bytes().map(|b| i32::from(b - b'0')));
    let pop = (1.0 - age / NUMBER_POP).max(0.0);
    let scale = (100.0 + (NUMBER_POP_PERCENT - 100.0) * pop * pop) / 100.0;
    let alpha = (age / NUMBER_FADE_IN).min(1.0) * ((NUMBER_SECONDS - age) / NUMBER_FADE_OUT).min(1.0);
    let advance = NUMBER_ADVANCE * scale;
    let left = centre[0] as f32 - advance * (cuts.len() as f32 - 1.0) / 2.0;
    cuts.iter()
        .enumerate()
        .map(|(k, &cut)| Element::Popped {
            part: NUMBER_DIGITS,
            cut,
            centre: [(left + advance * k as f32).round() as i32, centre[1]],
            percent: (scale * 100.0).round() as i32,
            alpha: (alpha * 255.0).round() as u8,
        })
        .collect()
}

#[cfg(test)]
mod number_tests {
    use super::*;

    #[test]
    fn a_hit_shows_its_sign_and_digits_centred_then_goes() {
        let e = number(-27, [500, 400], 0.5);
        let cuts: Vec<(i32, [i32; 2], i32, u8)> = e.iter().map(|e| match *e {
            Element::Popped { cut, centre, percent, alpha, .. } => (cut, centre, percent, alpha),
            _ => unreachable!(),
        }).collect();
        assert_eq!(cuts, vec![(11, [464, 400], 100, 255), (2, [500, 400], 100, 255), (7, [536, 400], 100, 255)]);
        // Popping in: larger and faint at first.
        let Element::Popped { percent, alpha, .. } = number(12, [0, 0], 0.02)[0] else { unreachable!() };
        assert!(percent > 140 && alpha < 60, "{percent} {alpha}");
        assert!(number(-5, [0, 0], NUMBER_SECONDS).is_empty());
    }
}
