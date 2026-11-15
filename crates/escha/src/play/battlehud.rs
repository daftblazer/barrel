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
//! state icons (layouts 873, 875), the plates' numbers (kind 28), the action banner (drawn on
//! its own, `banner`) and the
//! target panel left out. The Support gauge (`battle::SUPPORT_MAX`) shows its level (gauge /
//! 100) in layout 883's digit (part 0xe1) and the rest as the bar's fill (part 0xf7 cut 1, cut
//! from the left).

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::{Element, TEXT_RISE};

pub const LAYOUT: u32 = 886;
/// The Support gauge (layout 884), its bar (part 0xf7) and its level's digit (0xe1, layout 883).
const SUPPORT: u32 = 884;
const SUPPORT_BAR: i32 = 0xf7;
const SUPPORT_DIGITS: i32 = 0xe1;
/// Half the digit's width: its node is its centre.
const SUPPORT_DIGIT_HALF: i32 = 36;
const PLATE: u32 = 870;
const PORTRAIT: i32 = 0xf2;
const PLATE_PARTS: i32 = 0xee;
const PLATE_FRAME: i32 = 1;
/// The reserve row (layout 868): a panel for each member of the squad beyond the plates', in the
/// plates' order: its frame (part 0xee cut 8, the one that starts it), the two bars, the portrait's
/// ring (cut 0) and the face (part 0x42, the member's number as its cut).
const RESERVE: u32 = 868;
const RESERVE_FIRST_CUT: i32 = 8;
const RESERVE_FIRST: (i32, i32) = (PLATE_PARTS, RESERVE_FIRST_CUT);
const RESERVE_RING: i32 = 0;
const RESERVE_FACE: i32 = 0x42;
const PLATE_RING: i32 = 4;
/// The plate's HP and MP digits (part 0xdf, cuts 0 to 9, 28 wide), the MP row from y 856.
const PLATE_DIGITS: i32 = 0xdf;
const PLATE_DIGIT_ADVANCE: i32 = 28;
const PLATE_DIGITS_MP_FROM: i32 = 840;
const HP_BAR: i32 = 11;
const MP_BAR: i32 = 10;
/// The plate's first node (its frame), which starts each plate in the placed order.
const PLATE_FIRST: (i32, i32) = (PLATE_PARTS, 5);
/// The command row's layout (887): its places are the template `command_row` fills in.
const COMMAND_ROW: u32 = 887;
/// Left out: the template command row, the state icons (873, 875), the flipbook marker of layout 876 (not drawn at a turn's start), the target panel, the battle
/// cries' speech bubbles (layout 882: one over each plate, "…!!") and the action banner (885).
const LEFT_OUT: [u32; 7] = [873, 875, 876, 877, 882, BANNER, COMMAND_ROW];
/// The turn order bar and its slots (layout 860, the 16th the Action slot).
const TURN_BAR: u32 = 864;
const SLOT: u32 = 860;
/// The bar's end pieces (layout 862). Its first two, the dark frame behind the bar and its right
/// end, are never drawn in the captures (the frames of the party's turns and of a command row).
const BAR_ENDS: u32 = 862;
const BAR_ENDS_UNDRAWN: [(i32, i32); 2] = [(0xf4, 0), (0xf4, 1)];
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

/// A bar's fill: the share in whole percent, rounded down (the capture: 24 of 35 HP fills 68
/// of the bar's 100, 43 of 47 MP 91).
fn share((now, max): (i32, i32)) -> f32 {
    if max > 0 { (now.clamp(0, max) * 100 / max) as f32 / 100.0 } else { 0.0 }
}

/// Where the game draws a group of nodes against the place the layout gives (fitted to the
/// capture of a battle at 1920 × 1080: the turn bar a pixel right and down (its slots right only), the Fast Forward
/// label and the Support gauge a pixel up; the rest, the plates and the field information,
/// stand where the layout puts them).
const BAR_NUDGE: [i32; 2] = [1, 1];
/// The bar's slots a pixel right only.
const SLOT_NUDGE: [i32; 2] = [1, 0];
const SUPPORT_NUDGE: [i32; 2] = [0, -1];

fn nudged(p: &Placed) -> [i32; 2] {
    let d = if p.path.contains(&SLOT) {
        SLOT_NUDGE
    } else if p.path.contains(&TURN_BAR) {
        BAR_NUDGE
    } else if p.path.contains(&SUPPORT) {
        SUPPORT_NUDGE
    } else {
        [0, 0]
    };
    [p.at[0] + d[0], p.at[1] + d[1]]
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
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, members: &[Member], reserve: &[Member], order: &[Option<usize>], fast_forward: usize, marked: Option<usize>, next: Option<usize>, support: i32, row: (Vec<Element>, Vec<Element>)) -> Vec<Element> {
    let mut out = Ranked::default();
    // How many of the bar's frames came already.
    let mut frames = 0;
    // The turn order's slot a node is in (counted as their first nodes come), and that slot's
    // corner (the Action slot's scaling centre).
    let mut slot: Option<usize> = None;
    let mut slot_at = [0, 0];
    // Which plate a node is on (counted as their first nodes come).
    let starts = |p: &&Placed| p.path.last() == Some(&PLATE) && (p.node.part, p.node.cut) == PLATE_FIRST;
    let mut plate: Option<usize> = None;
    // Whether the Support gauge's level digit is drawn already.
    let mut digit_drawn = false;
    // Which reserve panel a node is on, likewise.
    let mut reserved: Option<usize> = None;
    for placed in nodes {
        if placed.path.iter().any(|l| LEFT_OUT.contains(l)) {
            continue;
        }
        let moved = Placed { at: nudged(placed), ..placed.clone() };
        let p = &moved;
        let n = p.node;
        out.rank = rank(p, slot, frames);
        if p.path.last() == Some(&TURN_BAR) {
            frames += 1;
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
                if p.path.last() == Some(&BAR_ENDS) && BAR_ENDS_UNDRAWN.contains(&(n.part, n.cut)) {
                    continue;
                }
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
                // The Action slot's corner is a pixel lower than the others' (the capture).
                Element::Scaled { part, cut, at: [p.at[0], p.at[1] + 1], percent: ACTION_PERCENT, about: [slot_at[0], slot_at[1] + 1] }
            } else {
                Element::Sprite { part, cut, at: p.at }
            });
            continue;
        }
        if p.path.contains(&SUPPORT) {
            let level = support.clamp(0, barrel_escha::battle::SUPPORT_MAX) / barrel_escha::battle::SUPPORT_LEVEL;
            match (n.part, n.cut) {
                (SUPPORT_BAR, 1) => {
                    let fill = if level * barrel_escha::battle::SUPPORT_LEVEL >= barrel_escha::battle::SUPPORT_MAX { 1.0 } else { (support % barrel_escha::battle::SUPPORT_LEVEL) as f32 / barrel_escha::battle::SUPPORT_LEVEL as f32 };
                    out.push(Element::Rising { part: SUPPORT_BAR, cut: 1, at: p.at, fill });
                    continue;
                }
                // (The layout holds the digit twice, at one place; the game draws it once.)
                (SUPPORT_DIGITS, _) => {
                    if !std::mem::replace(&mut digit_drawn, true) {
                        out.push(Element::Sprite { part: SUPPORT_DIGITS, cut: level, at: [p.at[0] - SUPPORT_DIGIT_HALF, p.at[1]] });
                    }
                    continue;
                }
                _ => {}
            }
        }
        if p.path.contains(&RESERVE) {
            if (n.part, n.cut) == RESERVE_FIRST {
                reserved = Some(reserved.map_or(0, |k| k + 1));
            }
            let Some(m) = reserved.and_then(|k| reserve.get(k)) else { continue };
            match (n.part, n.cut) {
                (PLATE_PARTS, RESERVE_FIRST_CUT | RESERVE_RING) => out.push(Element::stored(p)),
                (PLATE_PARTS, HP_BAR) => out.push(Element::Cropped { part: PLATE_PARTS, cut: HP_BAR, at: p.at, fill: share(m.hp) }),
                (PLATE_PARTS, MP_BAR) => out.push(Element::Cropped { part: PLATE_PARTS, cut: MP_BAR, at: p.at, fill: share(m.mp) }),
                (RESERVE_FACE, _) => out.push(Element::Sprite { part: RESERVE_FACE, cut: m.chara as i32, at: p.at }),
                _ => {}
            }
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
                // The HP and MP numbers (the layout's two digit nodes, HP first), the
                // digits right-aligned on the node.
                (PLATE_DIGITS, _) => {
                    let value = if p.at[1] < PLATE_DIGITS_MP_FROM { m.hp.0 } else { m.mp.0 };
                    let digits = value.max(0).to_string();
                    let start = p.at[0] - PLATE_DIGIT_ADVANCE * digits.len() as i32;
                    for (k, d) in digits.bytes().enumerate() {
                        out.push(Element::Sprite { part: PLATE_DIGITS, cut: i32::from(d - b'0'), at: [start + PLATE_DIGIT_ADVANCE * k as i32, p.at[1]] });
                    }
                }
                // The frame and the portrait's ring; the rest of the plate (the action
                // percentage's badge and digits, the flipbooks that mark a member's state) is
                // not drawn when a member's turn begins.
                (PLATE_PARTS, PLATE_FRAME | PLATE_RING) => out.push(Element::stored(p)),
                _ => {}
            }
            continue;
        }
        if n.part == 0x164 {
            // The field information's label is empty in the layout: the game fills it in.
            let field_info = p.path.get(1) == Some(&FIELD_INFO_LAYOUT);
            if let Some(t) = texts(p).filter(|t| field_info || (!t.text.is_empty() && !t.filled_in())) {
                let text = if t.text.starts_with("Fast Forward") {
                    FAST_FORWARD_LABELS[fast_forward.min(2)].to_string()
                } else if field_info {
                    FIELD_INFO_LABEL.to_string()
                } else {
                    t.text.clone()
                };
                out.push(Element::Label(Text { text, at: [t.at[0], t.at[1] - TEXT_RISE], ..t }));
            }
            continue;
        }
        if n.kind == NUMBER_KIND {
            continue;
        }
        if p.path.get(1) == Some(&FIELD_INFO_LAYOUT) && (n.part, n.cut) == (FIELD_INFO_BUTTON.0, FIELD_INFO_BUTTON.1) {
            out.push(Element::Sprite { part: n.part, cut: FIELD_INFO_BUTTON.2, at: p.at });
            continue;
        }
        out.push(Element::stored(p));
    }
    out.items.extend(row.0.into_iter().map(|e| (ROW_BEFORE, e)));
    out.items.extend(row.1.into_iter().map(|e| (ROW_AFTER, e)));
    out.items.sort_by_key(|&(rank, _)| rank);
    out.items.into_iter().map(|(_, e)| e).collect()
}

/// The elements as they are made, each with its place in the game's drawing order.
#[derive(Default)]
struct Ranked {
    items: Vec<(u32, Element)>,
    rank: u32,
}

impl Ranked {
    fn push(&mut self, e: Element) {
        self.items.push((self.rank, e));
    }
}

/// The order the game draws the HUD's parts in (the capture's draws; the layout's own order
/// differs): the bar's ends; each of its frames with the slot hanging from it; the Action slot;
/// the plates with the reserve row; the Support gauge and Fast Forward; the Action slot's
/// overlay; the command row's places; the field information; the chosen command's decorations.
const BAR_FRAMES: u32 = 1000;
const ACTION_SLOT: u32 = 2000;
const NEXT_MARK_RANK: u32 = 2100;
const PLATES: u32 = 3000;
const SUPPORT_RANK: u32 = 4000;
const OVERLAY: u32 = 4500;
const ROW_BEFORE: u32 = 5000;
const FIELD_INFO: u32 = 6000;
const ROW_AFTER: u32 = 7000;
/// The Action slot's overlay (layout 863, nested in the bar).
const OVERLAY_LAYOUT: u32 = 863;
const BAR_ENDS_RANK: u32 = 900;

/// `p`'s place in the drawing order: `slot` the slot counted so far, `frames` the bar's frames.
fn rank(p: &Placed, slot: Option<usize>, frames: u32) -> u32 {
    let first_of_slot = p.path.last() == Some(&SLOT) && (p.node.part, p.node.cut) == SLOT_FIRST;
    if p.path.contains(&SLOT) {
        let k = if first_of_slot { slot.map_or(0, |k| k + 1) } else { slot.unwrap_or(0) };
        if k >= SLOTS { ACTION_SLOT } else { BAR_FRAMES + 10 * k as u32 + 5 }
    } else if p.path.last() == Some(&NEXT_MARK) {
        NEXT_MARK_RANK
    } else if p.path.contains(&OVERLAY_LAYOUT) {
        OVERLAY
    } else if p.path.last() == Some(&BAR_ENDS) {
        BAR_ENDS_RANK
    } else if p.path.last() == Some(&TURN_BAR) {
        BAR_FRAMES + 10 * frames
    } else if p.path.contains(&SUPPORT) {
        SUPPORT_RANK
    } else if p.path.get(1) == Some(&FIELD_INFO_LAYOUT) {
        FIELD_INFO
    } else {
        PLATES
    }
}

/// The field information panel (layout 911), its label and its button, as the game shows
/// them: the layout's cut 6 is a stand-in, the panel's button is part 0x41's cut 20 (49 wide).
const FIELD_INFO_LAYOUT: u32 = 911;
const FIELD_INFO_LABEL: &str = "Field Information";
const FIELD_INFO_BUTTON: (i32, i32, i32) = (BUTTONS, 6, 20);


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
/// The row's places, 105 apart (the layout's nine, 104 to 106 apart, are that rounded), the
/// row centred on x 961 and its icons at y 585 (the capture of a turn's command row at
/// 1920 × 1080: five places, the first icon at x 703, the second at 808, y 593).
/// The most commands a row holds (layout 887's places).
const MAX_COMMANDS: usize = 9;
const ROW_PITCH: f32 = 105.0;
const ROW_CENTRE_X: f32 = 961.0;
const ROW_Y: i32 = 585;
/// A place's centre from its node (layout 859's icon, 856, at (98, 56)).
const ROW_CENTRE: [i32; 2] = [98, 56];
const ICON: i32 = 0xe7;
const ROW_PARTS: i32 = 0xe8;
const LABEL: i32 = 0xec;

/// The command row's flipbooks: their cuts (node word 5) and cuts a second (node word 6, 10: the
/// area map's cursor glow, word 6 = 8, turns at the 8 cuts a second the game was seen to cycle).
pub const FLIPBOOK_CUTS: usize = 4;
pub const FLIPBOOK_RATE: f32 = 10.0;

/// The flipbooks' step at `time` seconds of the battle's clock (it counts from the battle's
/// start; the game's start is found by `PARITY_ALIGN`, docs/escha/parity.md).
pub fn flip_step(time: f32) -> usize {
    (time * FLIPBOOK_RATE) as usize % FLIPBOOK_CUTS
}

/// The first stage of a card turn: the chosen place's dark frame and diamond turn edge-on, 0 to
/// 3 px wide and about 1.4 times as tall, their v flipped, and the game holds them so while
/// the row waits (battle-open capture, escha-recomp `scenarios/battle-open`: the row's quads
/// appear at frame 3267, the pieces settle by 3278 and stay until the A press at 3300).
///
/// Each key is the pieces' rectangles (frame, diamond: left, top, right, bottom on the
/// 1920 x 1080 screen) at that many frames after the row opened, read off the capture's every
/// second frame (3268, 3270 ... 3278 are 1, 3, ... 11); the first key is the line back from
/// the next two to the rest place. The game's own curve is not read (the layout's node words
/// carry no such spec), so it is fitted between the keys.
const TURN_KEYS: [(u32, [i32; 4], [i32; 4]); 7] = [
    (0, [706, 587, 796, 692], [691, 568, 811, 711]),
    (1, [710, 587, 792, 692], [696, 567, 814, 712]),
    (3, [719, 581, 783, 698], [708, 560, 801, 721]),
    (5, [725, 578, 777, 701], [717, 556, 793, 726]),
    (7, [735, 575, 767, 705], [730, 552, 778, 732]),
    (9, [745, 574, 756, 707], [744, 550, 763, 736]),
    (11, [752, 574, 749, 707], [753, 550, 753, 737]),
];
/// The turn's frames at the game's 60 a second, and the frame it holds from.
pub const TURN_RATE: f32 = 60.0;
pub const TURN_FRAMES: u32 = TURN_KEYS[TURN_KEYS.len() - 1].0;
/// The row node's x the keys' rectangles belong to (the capture's four commands, first place).
const TURN_NODE_X: i32 = 653;

/// The frame's and the diamond's rectangles `frames` after the row opened (held after the last
/// key) for the place whose node is at `node_x`.
fn turn_rects(frames: u32, node_x: i32) -> ([i32; 4], [i32; 4]) {
    let shift = node_x - TURN_NODE_X;
    let at = |r: [i32; 4]| [r[0] + shift, r[1], r[2] + shift, r[3]];
    let k = TURN_KEYS.iter().position(|key| key.0 >= frames).unwrap_or(TURN_KEYS.len() - 1);
    if k == 0 || TURN_KEYS[k].0 <= frames {
        let (_, f, d) = TURN_KEYS[k];
        return (at(f), at(d));
    }
    let (a, b) = (&TURN_KEYS[k - 1], &TURN_KEYS[k]);
    let t = (frames - a.0) as f32 / (b.0 - a.0) as f32;
    let mix = |x: [i32; 4], y: [i32; 4]| [0, 1, 2, 3].map(|i| (x[i] as f32 + (y[i] - x[i]) as f32 * t).round() as i32);
    (at(mix(a.1, b.1)), at(mix(a.2, b.2)))
}

/// The command row: `commands` (icon cuts) in their places, the chosen one on the diamond
/// (layout 857) with the arrows either side and its label on a plate below (858), the others
/// in their dark frames (855).
///
/// The diamond and the arrows are nodes of kind 36, a flipbook of word 5 cuts from the node's
/// (4: the diamond cuts 2 to 5, the arrows 6 to 9 and 10 to 13); `step` is the clock's step
/// ([`flip_step`]), taken modulo the cuts. `turn` is the frames since the row opened while the
/// chosen place is still in the card turn's first stage ([`TURN_KEYS`]), else `None`.
pub fn command_row(pe: &barrel_formats::pe::Pe, commands: &[i32], chosen: usize, step: usize, turn: Option<u32>) -> (Vec<Element>, Vec<Element>) {
    let step = (step % FLIPBOOK_CUTS) as i32;
    let n = commands.len().clamp(1, MAX_COMMANDS);
    // The places in order, drawn before the field information; the chosen one's arrows and
    // label after it (the arrows lie over the neighbours, the recording's right arrow over the
    // Skill icon).
    let (mut places, mut marks) = (Vec::new(), Vec::new());
    for k in 0..n {
        let c = commands[k];
        let node = [(ROW_CENTRE_X - ROW_CENTRE[0] as f32 + ROW_PITCH * (k as f32 - (n - 1) as f32 / 2.0)).round() as i32, ROW_Y];
        let at = |x: i32, y: i32| [node[0] + x, node[1] + y];
        let edge_on = turn.filter(|_| k == chosen).and_then(|frames| {
            let flipped = |cut: i32, rect: [i32; 4]| {
                let p = barrel_escha::data::layout::sprite(pe, ROW_PARTS, cut, [0, 0])?;
                let [l, t, r, b] = p.src.map(f32::from);
                Some(Element::Slice { pack: p.texture, index: p.index, src: [l, b, r, t], rect })
            };
            let (frame, diamond) = turn_rects(frames, node[0]);
            Some((flipped(0, frame)?, flipped(2 + step, diamond)?))
        });
        if let Some((frame, diamond)) = edge_on {
            places.push(frame);
            places.push(Element::Sprite { part: ICON, cut: c, at: at(ROW_CENTRE[0] - 48, ROW_CENTRE[1] - 48) });
            marks.push(diamond);
            marks.push(Element::Sprite { part: ROW_PARTS, cut: 6 + step, at: at(-12, 20) });
            marks.push(Element::Sprite { part: ROW_PARTS, cut: 10 + step, at: at(154, 20) });
            marks.push(Element::Sprite { part: LABEL, cut: 9, at: at(4, 100) });
            marks.push(Element::Sprite { part: LABEL, cut: c, at: at(48, 112) });
        } else if k == chosen {
            places.push(Element::Sprite { part: ROW_PARTS, cut: 2 + step, at: at(96 - 66, 50 - 72) });
            places.push(Element::Sprite { part: ICON, cut: c, at: at(ROW_CENTRE[0] - 48, ROW_CENTRE[1] - 48) });
            marks.push(Element::Sprite { part: ROW_PARTS, cut: 6 + step, at: at(-12, 20) });
            marks.push(Element::Sprite { part: ROW_PARTS, cut: 10 + step, at: at(154, 20) });
            marks.push(Element::Sprite { part: LABEL, cut: 9, at: at(4, 100) });
            marks.push(Element::Sprite { part: LABEL, cut: c, at: at(48, 112) });
        } else {
            places.push(Element::Sprite { part: ROW_PARTS, cut: 0, at: at(96 - 48, 54 - 48) });
            places.push(Element::Sprite { part: ICON, cut: c, at: at(ROW_CENTRE[0] - 48, ROW_CENTRE[1] - 48) });
        }
    }
    (places, marks)
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
    /// The next-turn mark's entry count (`Turn::next_place`), while one is shown.
    pub next: Option<usize>,
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
        assert_eq!(share((24, 35)), 0.68);
        assert_eq!(share((-3, 10)), 0.0);
        assert_eq!(share((3, 0)), 0.0);
    }
}

/// The battle's numbers over a fighter hit or healed: part 0xe0 (`a15_battle.g1t` picture 1, a
/// row of white brush digits with a dark edge, 36 × 60 cuts: 0 to 9, then 10 `+`, 11 `-`, 12
/// `/`, 13 `%`, 14 "MP"), the sign first, the glyphs 36 apart and centred on the fighter's
/// place (as a kind 29 node centres its number). Measured on the ruins-entrance-battle-result
/// recording (149.8 s, "-27" on a Neck Hunt): the "2"'s white is 26 × 34 in the texture, 32
/// tall at 720p, so 48 at 1080p; "-27" 136 wide), drawn at about 140%; it fades in over 0.1 s
/// while shrinking from about 1.6 times that to it by 0.2 s, and stays until the camera moves
/// on, some 0.85 s; the game's drawing code is not found yet (`docs/escha/open.md`; the
/// recording is 1280 × 720).
pub const NUMBER_DIGITS: i32 = 0xe0;
const NUMBER_PLUS: i32 = 10;
const NUMBER_MINUS: i32 = 11;
const NUMBER_ADVANCE: f32 = 36.0;
/// How long a number shows, and its fade in, pop and fade out (seconds).
pub const NUMBER_SECONDS: f32 = 1.0;
const NUMBER_FADE_IN: f32 = 0.1;
const NUMBER_POP: f32 = 0.2;
const NUMBER_FADE_OUT: f32 = 0.15;
const NUMBER_PERCENT: f32 = 140.0;
const NUMBER_POP_PERCENT: f32 = 1.6 * NUMBER_PERCENT;

/// `value` (negative: damage, "-27"; positive: a heal, "+12") centred on `centre` (the 1920 ×
/// 1080 screen), `age` seconds after it came up; nothing once [`NUMBER_SECONDS`] have passed.
pub fn number(value: i32, centre: [i32; 2], age: f32) -> Vec<Element> {
    if !(0.0..NUMBER_SECONDS).contains(&age) {
        return Vec::new();
    }
    let mut cuts = vec![if value < 0 { NUMBER_MINUS } else { NUMBER_PLUS }];
    cuts.extend(value.unsigned_abs().to_string().bytes().map(|b| i32::from(b - b'0')));
    let pop = (1.0 - age / NUMBER_POP).max(0.0);
    let scale = (NUMBER_PERCENT + (NUMBER_POP_PERCENT - NUMBER_PERCENT) * pop * pop) / 100.0;
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
        assert_eq!(cuts, vec![(11, [450, 400], 140, 255), (2, [500, 400], 140, 255), (7, [550, 400], 140, 255)]);
        // Popping in: larger and faint at first.
        let Element::Popped { percent, alpha, .. } = number(12, [0, 0], 0.02)[0] else { unreachable!() };
        assert!(percent > 200 && alpha < 60, "{percent} {alpha}");
        assert!(number(-5, [0, 0], NUMBER_SECONDS).is_empty());
    }
}

/// The action banner (layout 885, nested in 886 at (690, 180)): the plate part 0xf8 (node kind
/// 19, 450 wide as stored: cut 0 the left end 144 wide, cut 1 a 48-wide middle, cut 2 the right
/// end 212 wide, 72 high) with the action's name (size 33, its node at (116, 14)). The game
/// sizes and centres it in code (not read); measured on the ruins-entrance-battle-result
/// recording (1280 × 720, scaled to 1080p; 104, 112 and 133 s: "Attack", "Neck Hunt",
/// "Charge"): at full size, its top at the node's 180, centred on the screen (x 960), as wide
/// as the name plus 306 (405, 467, 415), the middle stretched; the name centred on it at its
/// node's height. It shows while the action plays.
pub const BANNER: u32 = 885;
const BANNER_PART: i32 = 0xf8;
const BANNER_CENTRE: i32 = 960;
const BANNER_TOP: i32 = 180;
const BANNER_SCALE: f32 = 1.0;
const BANNER_PAD: f32 = 306.0;
/// The name's size (the node's) and its drop from the plate's top.
pub const BANNER_TEXT_SIZE: f32 = 33.0;
const BANNER_TEXT_DROP: i32 = 14;
/// How long the banner stays up for an action (seconds; Barrel plays an action a step).
pub const BANNER_SECONDS: f32 = 1.4;

/// The banner for action `name`; `width` is the name's width at [`BANNER_TEXT_SIZE`] on the
/// 1920 × 1080 screen.
pub fn banner(pe: &barrel_formats::pe::Pe, name: &str, width: f32) -> Vec<Element> {
    let total = (width + BANNER_PAD).round() as i32;
    let left = BANNER_CENTRE - total / 2;
    let piece = |cut: i32| barrel_escha::data::layout::sprite(pe, BANNER_PART, cut, [0, 0]);
    let (Some(l), Some(m), Some(r)) = (piece(0), piece(1), piece(2)) else { return Vec::new() };
    let size = |p: &barrel_escha::data::layout::Piece| ((f32::from(p.src[2] - p.src[0]) * BANNER_SCALE).round() as i32, (f32::from(p.src[3] - p.src[1]) * BANNER_SCALE).round() as i32);
    let ((lw, h), (rw, _)) = (size(&l), size(&r));
    let slice = |p: &barrel_escha::data::layout::Piece, x0: i32, x1: i32| Element::Slice { pack: p.texture.clone(), index: p.index, src: p.src.map(f32::from), rect: [x0, BANNER_TOP, x1, BANNER_TOP + h] };
    vec![
        slice(&l, left, left + lw),
        slice(&m, left + lw, left + total - rw),
        slice(&r, left + total - rw, left + total),
        Element::Label(Text { at: [BANNER_CENTRE, BANNER_TOP + BANNER_TEXT_DROP], size: BANNER_TEXT_SIZE, colour: [0; 4], text: name.to_string(), kind: 22 }),
    ]
}

/// The damage rate panel (layout 926 at (1364, 150)): "Damage Rate:" (921, part 0xff cut 1)
/// and "Total Damage:" (922, cut 2) left of their numbers, the rate as 100 + it in the purple
/// digits (923, part 0xe3, kind 28: ending at its node) with "%" (925, 0xe3 cut 13), the total in
/// the red ones (924, 0xe4). Each digit advances its glyph's width (42, 40). Measured on the
/// ruins-entrance-battle-result recording (104 s, "130%", "9"): the whole panel tilted, rising
/// some 7° to the right, and its numbers further right than their nodes; drawn level here
/// at the layout's places (`docs/escha/open.md`).
pub const RATE_PANEL: u32 = 926;
const RATE_DIGITS: (i32, i32) = (0xe3, 42);
const TOTAL_DIGITS: (i32, i32) = (0xe4, 40);
const RATE_PERCENT_CUT: i32 = 13;

/// The panel for an action's damage `rate` (shown as 100 + it, percent) and `total` damage.
pub fn rate_panel(nodes: &[Placed], rate: i32, total: i32) -> Vec<Element> {
    let mut out = Vec::new();
    let digits = |value: i32, (part, advance): (i32, i32), end: [i32; 2], out: &mut Vec<Element>| {
        let text = value.max(0).to_string();
        let left = end[0] - advance * text.len() as i32;
        for (k, b) in text.bytes().enumerate() {
            out.push(Element::Sprite { part, cut: i32::from(b - b'0'), at: [left + advance * k as i32, end[1]] });
        }
    };
    for p in nodes {
        match (p.node.part, p.node.cut, p.node.kind) {
            (0xff, 1 | 2, _) => out.push(Element::Sprite { part: 0xff, cut: p.node.cut, at: p.at }),
            (part, _, 28) if part == RATE_DIGITS.0 => digits(100 + rate, RATE_DIGITS, p.at, &mut out),
            (part, _, 28) if part == TOTAL_DIGITS.0 => digits(total, TOTAL_DIGITS, p.at, &mut out),
            (part, RATE_PERCENT_CUT, _) if part == RATE_DIGITS.0 => out.push(Element::Sprite { part, cut: RATE_PERCENT_CUT, at: p.at }),
            _ => {}
        }
    }
    out
}

/// The support bubbles (layout 882): over each party plate, while a party hit plays, the
/// button that calls that member's Support Attack and its name: layout 879 over the left plate
/// (at (448, 750): LB, part 0x41 cut 8), 881 over the centre one ((828, 750): Y, cut 6), 880
/// over the right one ((1208, 750): RB, cut 12), each the tab 0xfb, the button, a rule (0x45 cut
/// 26) and the text (size 30); the second row (y 928) is the reserve's. The game's label is
/// "Support Attack" (the escha-support-attack recording).
pub const SUPPORT_BUBBLES: u32 = 882;
const BUBBLE_LAYOUTS: [u32; 3] = [879, 881, 880];
const BUBBLE_ROW_LIMIT: i32 = 900;
pub const SUPPORT_LABEL: &str = "Support Attack";
/// The offer while a monster's action on a member starts (the escha-support-attack recording,
/// Spitball on Katla).
pub const GUARD_LABEL: &str = "Support Guard";

/// The bubbles over the plates in `plates` (0 left, 1 centre, 2 right).
pub fn support_bubbles(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, plates: &[usize], label: &str) -> Vec<Element> {
    let mut out = Vec::new();
    for p in nodes {
        let Some(k) = p.path.get(1).and_then(|l| BUBBLE_LAYOUTS.iter().position(|b| b == l)) else { continue };
        if !plates.contains(&k) || p.at[1] >= BUBBLE_ROW_LIMIT {
            continue;
        }
        if p.node.part == 0x164 {
            if let Some(t) = texts(p) {
                out.push(Element::Label(Text { text: label.to_string(), ..t }));
            }
        } else if p.node.part == BUTTONS {
            // The button in the Western layout (`message::western_sprite`: the centre's cut 6
            // shows Y, as the recording).
            out.push(Element::Sprite { part: BUTTONS, cut: crate::message::western_sprite(p.node.cut.max(0) as u32) as i32, at: p.at });
        } else if (p.node.part, p.node.cut) == BUBBLE_RULE {
            // Not shown under the offer in the recording (a highlight for another state).
        } else if p.node.part >= 0 && p.node.part < 0x164 {
            out.push(Element::stored(p));
        }
    }
    out
}
const BUBBLE_RULE: (i32, i32) = (0x45, 26);
const BUTTONS: i32 = 0x41;
