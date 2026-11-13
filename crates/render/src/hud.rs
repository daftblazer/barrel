//! The field HUD, drawn from the game's own layout (`barrel_formats::layout`, layout 625):
//! the assignment panel at the top left (605: "Left", the days, the year disc), the time dial
//! (609) and the button list at the bottom left (620), filled with the calendar's values.
//!
//! What fills which node was matched against a screenshot of the game ("Left 100", "8 /20",
//! the year disc "1" on August 20th of year 1), not read from the HUD code: the empty text
//! after "Left" is the days left in the assignment period (its last day minus today); the two
//! big numbers (part 0x57, kind 29, cut = digit) are the month and the day; the year disc's
//! digit is part 0x8a cut 2 + year. Not yet the game's: the dial's ten segments (part 0x8d cuts
//! 2 to 11, in the layout's order) as the tenths of the day still free; the button list (620)
//! moved up 121 pixels, as the screenshot has it (the layout puts the list at y 792, past the
//! screen's foot); the days left are a kind 22 text, centred on their node; the text size (the node's size × 1.47
//! over the font's line); how many-digit numbers sit (the first
//! right-aligned on its node, the second left-aligned, 32 pixels a digit); white for texts
//! without a colour and for the days left. The arrows (602), the assignment badges (603), the
//! "Free Time" tab (604), the small count (part 0xe5) and the pop-ups (the poster, gathering
//! notices, the gathering gauge) are left out until their conditions are known.

use barrel_formats::layout::{self, Placed};

use crate::text::{self, Quad};

/// The HUD's top layout and the parts of it that are drawn.
pub const FIELD_HUD: u32 = 625;
const SHOWN: [u32; 3] = [605, 609, 620];
const LEFT_OUT: [u32; 3] = [602, 603, 604];
const DIGITS: i32 = 0x57;
const SMALL_DIGITS: i32 = 0xe5;
const YEAR: i32 = 0x8a;
const DIAL: i32 = 0x8d;
const DIGIT_ADVANCE: i32 = 32;
const BUTTON_LIST: u32 = 620;
const BUTTON_LIST_SHIFT: i32 = -121;

/// What the HUD shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    pub left: u32,
    pub month: u32,
    pub day: u32,
    pub year: u32,
    pub tenths_free: u32,
}

impl Values {
    /// The values on day `today` with `tenths` of it spent in the field.
    pub fn on(today: u32, tenths: i32) -> Values {
        use barrel_game::calendar::{period, period_days, Date};
        let left = period(today).map_or(0, |k| period_days(k).1 - today);
        let date = Date::of(today);
        Values { left, month: date.month, day: date.day, year: date.year, tenths_free: (10 - tenths.clamp(0, 10)) as u32 }
    }
}

const WHITE: [u8; 4] = [0xff; 4];

/// One thing to draw: a sprite (part, cut, top-left) or a label.
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    Sprite { part: i32, cut: i32, at: [i32; 2] },
    /// A sprite at half strength (a choice not taken).
    Dim { part: i32, cut: i32, at: [i32; 2] },
    /// A sprite mirrored left to right (node kind 2).
    Mirrored { part: i32, cut: i32, at: [i32; 2] },
    Label(layout::Text),
}

/// Node kind of a sprite drawn mirrored left to right (the right halves of symmetric pieces:
/// the save list's and hero choice's parchment, the right-hand rope).
pub const MIRRORED: i32 = 2;

impl Element {
    /// A placed node's sprite as the layout stores it (mirrored for kind 2).
    pub fn stored(p: &Placed) -> Element {
        let (part, cut, at) = (p.node.part, p.node.cut.max(0), p.at);
        if p.node.kind == MIRRORED { Element::Mirrored { part, cut, at } } else { Element::Sprite { part, cut, at } }
    }

    /// The sprite's part, cut and place, if it is one.
    pub fn sprite(&self) -> Option<(i32, i32, [i32; 2])> {
        match *self {
            Element::Sprite { part, cut, at } | Element::Dim { part, cut, at } | Element::Mirrored { part, cut, at } => Some((part, cut, at)),
            Element::Label(_) => None,
        }
    }
}

/// The HUD's elements for `v`, from layout 625's placed nodes (`texts` gives a node's text).
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<layout::Text>, v: Values) -> Vec<Element> {
    let mut out = Vec::new();
    let mut numbers = 0;
    let mut segment = 0;
    let mut left_shown = false;
    for p in nodes {
        let Some(&branch) = p.path.get(1) else { continue };
        if !SHOWN.contains(&branch) || p.path.iter().any(|l| LEFT_OUT.contains(l)) {
            continue;
        }
        let n = p.node;
        let shift = if p.path.contains(&BUTTON_LIST) { BUTTON_LIST_SHIFT } else { 0 };
        let at = [p.at[0], p.at[1] + shift];
        match n.part {
            0x164 => match texts(p) {
                // The text after "Left": the days left.
                Some(t) if t.text.is_empty() && p.path.len() == 2 && p.path[1] == 605 && n.kind == 22 && !left_shown => {
                    out.push(Element::Label(layout::Text { text: v.left.to_string(), colour: WHITE, at, ..t }));
                    left_shown = true;
                }
                Some(t) if !t.text.is_empty() && !t.filled_in() => out.push(Element::Label(layout::Text { at, ..t })),
                _ => {}
            },
            DIGITS if n.kind == 29 => {
                let value = if numbers == 0 { v.month } else { v.day };
                let digits: Vec<i32> = value.to_string().bytes().map(|b| i32::from(b - b'0')).collect();
                let start = if numbers == 0 { p.at[0] - DIGIT_ADVANCE * (digits.len() as i32 - 1) } else { p.at[0] };
                for (k, d) in digits.into_iter().enumerate() {
                    out.push(Element::Sprite { part: DIGITS, cut: d, at: [start + DIGIT_ADVANCE * k as i32, p.at[1]] });
                }
                numbers += 1;
            }
            SMALL_DIGITS if n.kind == 29 => {}
            YEAR if n.cut >= 3 => out.push(Element::Sprite { part: YEAR, cut: (v.year as i32 + 2).clamp(3, 7), at: p.at }),
            DIAL if (2..=11).contains(&n.cut) => {
                if segment < v.tenths_free {
                    out.push(Element::Sprite { part: n.part, cut: n.cut, at: p.at });
                }
                segment += 1;
            }
            _ => out.push(match Element::stored(p) {
                Element::Mirrored { part, cut, .. } => Element::Mirrored { part, cut, at },
                e => e.sprite().map_or(e, |(part, cut, _)| Element::Sprite { part, cut, at }),
            }),
        }
    }
    out
}

/// The HUD's textures in a text layer after the font atlas: each (pack, picture) and its size.
pub struct Sheets {
    pub keys: Vec<(String, u32)>,
    pub sizes: Vec<[f32; 2]>,
    /// The first texture slot of the sheets in the layer.
    pub first: usize,
}

/// The quads for `elements` on a `width` × `height` screen (the 1920 × 1080 layout scaled to
/// the height and centred). Labels: the node's size taken as the text's height in pixels on
/// that screen, over the font's line (not read).
pub fn quads(pe: &barrel_formats::pe::Pe, font: Option<&text::Font>, sheets: &Sheets, elements: &[Element], width: f32, height: f32) -> Vec<Quad> {
    let k = height / 1080.0;
    let dx = (width - 1920.0 * k) / 2.0;
    let mut out = Vec::new();
    for e in elements {
        match e {
            &Element::Sprite { part, cut, at } | &Element::Dim { part, cut, at } | &Element::Mirrored { part, cut, at } => {
                let alpha = if matches!(e, Element::Dim { .. }) { 0.5 } else { 1.0 };
                let Some(piece) = layout::sprite(pe, part, cut, at) else { continue };
                let Some(slot) = sheets.keys.iter().position(|(n, i)| *n == piece.texture && *i == piece.index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let [l, t, r, b] = piece.src.map(f32::from);
                let [x, y] = piece.at.map(|v| v as f32);
                out.push(Quad {
                    rect: [dx + x * k, y * k, dx + (x + r - l) * k, (y + b - t) * k],
                    uv: if matches!(e, Element::Mirrored { .. }) { [r / w, t / h, l / w, b / h] } else { [l / w, t / h, r / w, b / h] },
                    color: [1.0, 1.0, 1.0, alpha],
                    texture: sheets.first + slot,
                });
            }
            Element::Label(t) => {
                let Some(font) = font else { continue };
                let [a, r, g, b] = t.colour;
                let colour = if a == 0 { [1.0; 4] } else { [r, g, b, a].map(|c| f32::from(c) / 255.0) };
                let scale = t.size * k / text::LINE * 1.47;
                // Kind 23 texts end at their node (the numbers at x 1800 in the save list's
                // details), kind 22 are centred on it ("Select your character." at x 954, the
                // HUD's days left), the others start there.
                let width = font.width_of(&t.text) * scale;
                let x = dx + t.at[0] as f32 * k - match t.kind {
                    23 => width,
                    22 => width / 2.0,
                    _ => 0.0,
                };
                font.line(&t.text, x, t.at[1] as f32 * k, scale, colour, &mut out);
            }
        }
    }
    out
}

/// The (pack, picture) keys the HUD's sprites use, for loading.
pub fn sheet_keys(pe: &barrel_formats::pe::Pe, nodes: &[Placed]) -> Vec<(String, u32)> {
    let mut keys = Vec::new();
    for p in nodes.iter().filter(|p| p.path.get(1).is_some_and(|b| SHOWN.contains(b))) {
        for cut in [p.node.cut.max(0), 0] {
            if let Some(piece) = layout::sprite(pe, p.node.part, cut, p.at) {
                let key = (piece.texture, piece.index);
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::layout::Node;

    fn node(path: &[u32], part: i32, cut: i32, kind: i32, at: [i32; 2]) -> Placed {
        Placed { path: path.to_vec(), node: Node { part, cut, at: [0, 0], kind }, at, va: 0 }
    }

    #[test]
    fn values_follow_the_calendar() {
        // April 1st of year 1: 119 days to the first period's last.
        assert_eq!(Values::on(90, 0), Values { left: 119, month: 4, day: 1, year: 1, tenths_free: 10 });
        // August 20th of year 1, as the game's screenshot: "Left 100", "8 /20", year 1.
        let aug20 = barrel_game::calendar::Date { year: 1, month: 8, day: 20 }.number();
        assert_eq!(Values::on(aug20, 3), Values { left: 100, month: 8, day: 20, year: 1, tenths_free: 7 });
    }

    #[test]
    fn numbers_year_and_dial() {
        let nodes = vec![
            node(&[625, 605], DIGITS, 0, 29, [120, 120]),
            node(&[625, 605], DIGITS, 0, 29, [214, 120]),
            node(&[625, 605], YEAR, 3, 0, [286, 116]),
            node(&[625, 605, 603], 0x8b, 3, 0, [0, 0]),
            node(&[625, 609], DIAL, 0, 0, [0, 0]),
            node(&[625, 609], DIAL, 2, 0, [1, 0]),
            node(&[625, 609], DIAL, 3, 0, [2, 0]),
            node(&[625, 617], 0x10, 0, 0, [0, 0]),
        ];
        let v = Values { left: 98, month: 12, day: 120, year: 2, tenths_free: 1 };
        let got = elements(&nodes, |_| None, v);
        let sprite = |part, cut, at| Element::Sprite { part, cut, at };
        assert_eq!(got, vec![
            sprite(DIGITS, 1, [88, 120]),
            sprite(DIGITS, 2, [120, 120]),
            sprite(DIGITS, 1, [214, 120]),
            sprite(DIGITS, 2, [246, 120]),
            sprite(DIGITS, 0, [278, 120]),
            sprite(YEAR, 4, [286, 116]),
            sprite(DIAL, 0, [0, 0]),
            sprite(DIAL, 2, [1, 0]),
        ]);
    }
}
