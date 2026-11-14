//! The field HUD, drawn from the game's own layout (`barrel_escha::data::layout`, layout 625):
//! the assignment panel at the top left (605: "Left", the days, the year disc), the time dial
//! (609) and the button list at the bottom left (620), filled with the calendar's values.
//!
//! What fills which node was matched against a screenshot of the game ("Left 100", "8 /20",
//! the year disc "1" on August 20th of year 1), not read from the HUD code: the empty text
//! after "Left" is the days left in the assignment period (its last day minus today); the two
//! big numbers (part 0x57, kind 29, cut = digit) are the month and the day; the year disc's
//! digit is part 0x8a cut 2 + year. Both numbers are centred on their node, 36 pixels a digit
//! (the glyph's width): "8 /20" as the game draws it (the colseit-arrival recording's area map,
//! measured by patch search: the month's "8" and the day's "20" each centred on its node). The dial's ten segments (part 0x8d cuts 2 to 11, in the
//! layout's order) light as the day's field time is spent, from the top clockwise (checked in the
//! running game, 2026-11-13: none on arrival, one after a battle, five after a gathering); its
//! key-shaped hand (layout 610, part 0x8d cut 1, 120 × 24), pointing at the time spent (up for
//! none, across for half), is left out (the HUD draws no turned sprites yet). Layout 609 lists
//! the segments clockwise from the top (cuts 2 to 6, then 11 down to 7). Not yet the game's: the button list (620)
//! moved up 121 pixels, as the screenshot has it (the layout puts the list at y 792, past the
//! screen's foot); the days left are a kind 22 text, centred on their node; the text size (the node's size × 1.47
//! over the font's line); white for texts
//! without a colour and for the days left. Under the date, the ten request cards (layout 603
//! at the panel's ten 0x25b nodes, five a row, the second row staggered): one per slot of the
//! requests taken, part 0x8b cut 3 (a scroll) while open, cut 5 (pink) with the red tick
//! (cut 1, the layout's second node) once done, cut 6 (blank) for an empty slot, each with its
//! pin (cut 12); the user's save at 8/20 (one gathering done, two open) against the
//! colseit-arrival recording. Only six slots are shown there (`CARDS_SHOWN`). Not yet seen: a failed request (likely cut 4 grey with the blue
//! cross, cut 2) or one of another kind. Under them, the field
//! effect's gauge (layout 614, see `GAUGE`). The arrows (602), the "Free Time" tab (604), the
//! small count (part 0xe5) and the pop-ups (the poster, gathering notices) are left out until
//! their conditions are known.

use barrel_escha::data::layout::{self, Placed};

use crate::text::{self, Quad};

/// The HUD's top layout and the parts of it that are drawn.
pub const FIELD_HUD: u32 = 625;
const SHOWN: [u32; 4] = [605, 609, 614, 620];
/// The field effect's gauge (layout 614, at (32, 246) in 625) and its pieces: the notice's
/// plate (0x69 cut 4) and text (0x6e), the box (cut 2), the fill (cut 6, red at the top to
/// green at the foot, shown from the foot up to the percentage over 200), the tube over it
/// (cut 17, its notches at 100), the percentage (0xe5, kind 28) and its sign (0xe5 cut 13),
/// "MAX" (0xf0) in their place from 200 (`0x1402823fa`). Left out: the effect's choice
/// panel (cut 5 and layouts 611 to 613, shown in the field at 100) and the glows (0x6f).
const GAUGE: u32 = 614;
/// The gauge rests 4 pixels right of its node (the colseit-arrival recording's area map, a
/// shift fitted at 1080p against the 720p frame: box, fill and notice alike), as the panel
/// rests off its own.
const GAUGE_SHIFT: [i32; 2] = [4, 0];
const GAUGE_PARTS: i32 = 0x69;
const GAUGE_PLATE: i32 = 4;
const GAUGE_CHOICE: i32 = 5;
const GAUGE_FILL: i32 = 6;
const GAUGE_FULL: i32 = 200;
const NOTICE: i32 = 0x6e;
const GLOW: i32 = 0x6f;
const MAX: i32 = 0xf0;
const PERCENT: i32 = 13;
/// The small digits' advance (their glyph's width).
const SMALL_ADVANCE: i32 = 18;
const LEFT_OUT: [u32; 2] = [602, 604];
/// A request card (layout 603) and its sprites.
const CARD: u32 = 603;
const CARD_OPEN: i32 = 3;
const CARD_DONE: i32 = 5;
const CARD_EMPTY: i32 = 6;
const CARD_TICK: i32 = 1;
/// The slots the game shows: six on the user's save (rank 3: the first row and the second
/// row's first card), the rule (rank, story) not yet read; the rest are not drawn.
const CARDS_SHOWN: usize = 6;
const DIGITS: i32 = 0x57;
const SMALL_DIGITS: i32 = 0xe5;
const YEAR: i32 = 0x8a;
const DIAL: i32 = 0x8d;
const DIGIT_ADVANCE: i32 = 36;
/// Half a digit glyph (36 wide): numbers are centred on their node.
const DIGIT_CENTRE: i32 = 18;
const BUTTON_LIST: u32 = 620;
const BUTTON_LIST_SHIFT: i32 = -121;
/// The assignment panel (605) rests 20 right and 8 down of its node in 625 (-20, -40): at its
/// own place (0, -32), as the colseit-arrival recording's area map has it (patch search on the
/// date's digits). Reused layouts sit at their nodes (776 four times in 788), so this is not
/// the general rule; likely where the HUD's slide-in ends.
const PANEL: u32 = 605;
const PANEL_SHIFT: [i32; 2] = [20, 8];

/// What the HUD shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    pub left: u32,
    pub month: u32,
    pub day: u32,
    pub year: u32,
    pub tenths_spent: u32,
    /// The request cards, by slot.
    pub cards: [Card; 10],
    /// The field effect's gauge (layout 614), when shown.
    pub gauge: Option<Gauge>,
}

/// The field effect's gauge under the request cards: its percentage (0 to 200, the field
/// effect's `fieldmap/m_userDataFieldMapData` gauge) and the effect's mode (its notice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gauge {
    pub percent: i32,
    pub mode: i32,
}

impl Gauge {
    /// The notice's cut (part 0x6e: Collection Quantity, Rare Material, Relic, Battle, Land,
    /// Report): modes 3 to 8 in order (`0x140283a60`'s jump table); any other mode, none.
    pub fn notice(self) -> Option<i32> {
        (3..=8).contains(&self.mode).then_some(self.mode - 3)
    }
}

/// The request cards for the requests taken.
pub fn cards(taken: &[Option<barrel_escha::session::Taken>; 10]) -> [Card; 10] {
    taken.map(|t| match t {
        None => Card::Empty,
        Some(t) if t.done() => Card::Done,
        Some(_) => Card::Open,
    })
}

/// A request card under the date: an empty slot, a request taken and open, or done.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Card {
    #[default]
    Empty,
    Open,
    Done,
}

impl Values {
    /// The values on day `today` with `tenths` of it spent in the field.
    pub fn on(today: u32, tenths: i32) -> Values {
        use barrel_escha::calendar::{period, period_days, Date};
        let left = period(today).map_or(0, |k| period_days(k).1 - today);
        let date = Date::of(today);
        Values { left, month: date.month, day: date.day, year: date.year, tenths_spent: tenths.clamp(0, 10) as u32, cards: [Card::Empty; 10], gauge: None }
    }
}

const WHITE: [u8; 4] = [0xff; 4];

/// One thing to draw: a sprite (part, cut, top-left) or a label.
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    Sprite { part: i32, cut: i32, at: [i32; 2] },
    /// A sprite at half strength (a choice not taken).
    Dim { part: i32, cut: i32, at: [i32; 2] },
    /// A sprite at `alpha` of 255 (the title's pulsing prompt).
    Faded { part: i32, cut: i32, at: [i32; 2], alpha: u8 },
    /// A sprite mirrored (node kind 2): `axis` 0 left to right, 1 top to bottom, 2 both.
    Mirrored { part: i32, cut: i32, at: [i32; 2], axis: i32 },
    /// A sprite scaled to `percent` about its node `at` (a layout node of kind 4, its word 5:
    /// the title's logo at 80).
    Scaled { part: i32, cut: i32, at: [i32; 2], percent: i32 },
    /// A sprite turned a quarter clockwise about its node `at` (a layout node of kind 7: the
    /// area map labels' ornaments), so it lies left of and below the node.
    Turned { part: i32, cut: i32, at: [i32; 2] },
    /// A sprite spanned to `width` (a layout node of kind 17, sized by the game's code: the turn
    /// bar's next-turn line, an L corner): the sprite at the left end, mirrored at the right
    /// end, its last column stretched between.
    Spanned { part: i32, cut: i32, at: [i32; 2], width: i32 },
    /// A sprite's left `fill` (0 to 1) only: a bar.
    Cropped { part: i32, cut: i32, at: [i32; 2], fill: f32 },
    /// A sprite's bottom `fill` (0 to 1) only, at its place: a gauge filling upwards.
    Rising { part: i32, cut: i32, at: [i32; 2], fill: f32 },
    /// A whole picture of a pack chosen at run time (an item's picture), `size` on the
    /// 1920 × 1080 screen, at half strength when `dim`.
    Picture { pack: String, index: u32, size: [f32; 2], at: [i32; 2], dim: bool },
    Label(layout::Text),
    /// A rectangle (left, top, right, bottom on the 1920 × 1080 screen) of one colour.
    Fill { rect: [i32; 4], colour: [f32; 4] },
    /// A region `src` (texture pixels: left, top, right, bottom) of a pack's picture, drawn
    /// into `rect` (the 1920 × 1080 screen): the pieces the game stretches (windows, panels).
    Slice { pack: String, index: u32, src: [f32; 4], rect: [i32; 4] },
}

/// A piece stretched into `rect` keeping its `edge` texture pixels at each side as they are
/// (the corners whole, the edges stretched one way, the middle both ways).
pub fn nine(pack: &str, index: u32, src: [f32; 4], rect: [i32; 4], edge: f32) -> Vec<Element> {
    let e = edge as i32;
    let xs = [(src[0], rect[0]), (src[0] + edge, rect[0] + e), (src[2] - edge, rect[2] - e), (src[2], rect[2])];
    let ys = [(src[1], rect[1]), (src[1] + edge, rect[1] + e), (src[3] - edge, rect[3] - e), (src[3], rect[3])];
    let mut out = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            out.push(Element::Slice { pack: pack.into(), index, src: [xs[i].0, ys[j].0, xs[i + 1].0, ys[j + 1].0], rect: [xs[i].1, ys[j].1, xs[i + 1].1, ys[j + 1].1] });
        }
    }
    out
}

/// Node kind of a sprite drawn mirrored left to right (the right halves of symmetric pieces:
/// the save list's and hero choice's parchment, the right-hand rope).
pub const MIRRORED: i32 = 2;

impl Element {
    /// A placed node's sprite as the layout stores it (mirrored for kind 2).
    pub fn stored(p: &Placed) -> Element {
        let (part, cut, at) = (p.node.part, p.node.cut.max(0), p.at);
        if p.node.kind == MIRRORED { Element::Mirrored { part, cut, at, axis: p.node.mirror } } else { Element::Sprite { part, cut, at } }
    }

    /// The sprite's part, cut and place, if it is one.
    pub fn sprite(&self) -> Option<(i32, i32, [i32; 2])> {
        match *self {
            Element::Sprite { part, cut, at } | Element::Dim { part, cut, at } | Element::Mirrored { part, cut, at, .. } | Element::Scaled { part, cut, at, .. } | Element::Faded { part, cut, at, .. } | Element::Cropped { part, cut, at, .. } | Element::Spanned { part, cut, at, .. } | Element::Rising { part, cut, at, .. } | Element::Turned { part, cut, at } => Some((part, cut, at)),
            Element::Label(_) | Element::Picture { .. } | Element::Fill { .. } | Element::Slice { .. } => None,
        }
    }
}

/// The HUD's elements for `v`, from layout 625's placed nodes (`texts` gives a node's text).
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<layout::Text>, v: Values) -> Vec<Element> {
    let mut out = Vec::new();
    let mut numbers = 0;
    let mut segment = 0;
    let mut left_shown = false;
    let mut card = 0usize;
    for p in nodes {
        let Some(&branch) = p.path.get(1) else { continue };
        if !SHOWN.contains(&branch) || p.path.iter().any(|l| LEFT_OUT.contains(l)) {
            continue;
        }
        let shift = if p.path.contains(&BUTTON_LIST) {
            [0, BUTTON_LIST_SHIFT]
        } else if p.path.contains(&PANEL) && p.node.part != 0x164 {
            // Its texts stay at their nodes (measured: "Left" and the days left sit 8
            // pixels higher than the sprites' shift would put them).
            PANEL_SHIFT
        } else if branch == GAUGE {
            GAUGE_SHIFT
        } else {
            [0, 0]
        };
        let p = &Placed { at: [p.at[0] + shift[0], p.at[1] + shift[1]], ..p.clone() };
        let n = p.node;
        let at = p.at;
        if branch == GAUGE {
            let Some(g) = v.gauge else { continue };
            if p.path.len() > 2 {
                // The choice panel's icons (611 to 613).
                continue;
            }
            let full = g.percent >= GAUGE_FULL;
            match (n.part, n.cut) {
                (GAUGE_PARTS, GAUGE_PLATE) if g.notice().is_none() => {}
                (GAUGE_PARTS, GAUGE_CHOICE) | (GLOW, _) => {}
                (GAUGE_PARTS, GAUGE_FILL) => out.push(Element::Rising { part: n.part, cut: n.cut, at, fill: g.percent.clamp(0, GAUGE_FULL) as f32 / GAUGE_FULL as f32 }),
                (NOTICE, _) => {
                    if let Some(cut) = g.notice() {
                        out.push(Element::Sprite { part: n.part, cut, at });
                    }
                }
                (SMALL_DIGITS, _) if full => {}
                (SMALL_DIGITS, PERCENT) => out.push(Element::Sprite { part: n.part, cut: n.cut, at }),
                (SMALL_DIGITS, _) => {
                    // The percentage ends at its node.
                    let digits: Vec<i32> = g.percent.max(0).to_string().bytes().map(|b| i32::from(b - b'0')).collect();
                    let start = at[0] - SMALL_ADVANCE * digits.len() as i32;
                    for (k, d) in digits.into_iter().enumerate() {
                        out.push(Element::Sprite { part: SMALL_DIGITS, cut: d, at: [start + SMALL_ADVANCE * k as i32, at[1]] });
                    }
                }
                (MAX, _) if !full => {}
                _ => out.push(Element::Sprite { part: n.part, cut: n.cut.max(0), at }),
            }
            continue;
        }
        if p.path.last() == Some(&CARD) {
            if card >= CARDS_SHOWN {
                continue;
            }
            let c = v.cards.get(card).copied().unwrap_or_default();
            match n.cut {
                CARD_OPEN => out.push(Element::Sprite {
                    part: n.part,
                    cut: match c {
                        Card::Empty => CARD_EMPTY,
                        Card::Open => CARD_OPEN,
                        Card::Done => CARD_DONE,
                    },
                    at,
                }),
                CARD_TICK if c == Card::Done => out.push(Element::Sprite { part: n.part, cut: CARD_TICK, at }),
                CARD_TICK => {}
                _ => {
                    // The pin: a card's last node.
                    out.push(Element::Sprite { part: n.part, cut: n.cut, at });
                    card += 1;
                }
            }
            continue;
        }
        match n.part {
            0x164 => match texts(p) {
                // The text after "Left": the days left.
                Some(t) if t.text.is_empty() && p.path.len() == 2 && p.path[1] == 605 && n.kind == 22 && !left_shown => {
                    out.push(Element::Label(layout::Text { text: v.left.to_string(), colour: WHITE, at, ..t }));
                    left_shown = true;
                }
                // "Left" starts at its node (measured on the recording: x 78 to 122 at 1080p,
                // its node at 80), though kind 22 texts elsewhere are centred.
                Some(t) if p.path.len() == 2 && p.path[1] == PANEL && !t.text.is_empty() => out.push(Element::Label(layout::Text { at, kind: 21, ..t })),
                Some(t) if !t.text.is_empty() && !t.filled_in() => out.push(Element::Label(layout::Text { at, ..t })),
                _ => {}
            },
            DIGITS if n.kind == 29 => {
                let value = if numbers == 0 { v.month } else { v.day };
                let digits: Vec<i32> = value.to_string().bytes().map(|b| i32::from(b - b'0')).collect();
                let start = p.at[0] - DIGIT_CENTRE * digits.len() as i32;
                for (k, d) in digits.into_iter().enumerate() {
                    out.push(Element::Sprite { part: DIGITS, cut: d, at: [start + DIGIT_ADVANCE * k as i32, p.at[1]] });
                }
                numbers += 1;
            }
            SMALL_DIGITS if n.kind == 29 => {}
            YEAR if n.cut >= 3 => out.push(Element::Sprite { part: YEAR, cut: (v.year as i32 + 2).clamp(3, 7), at: p.at }),
            DIAL if (2..=11).contains(&n.cut) => {
                if segment < v.tenths_spent {
                    out.push(Element::Sprite { part: n.part, cut: n.cut, at: p.at });
                }
                segment += 1;
            }
            _ => out.push(match Element::stored(p) {
                Element::Mirrored { part, cut, axis, .. } => Element::Mirrored { part, cut, at, axis },
                e => e.sprite().map_or(e, |(part, cut, _)| Element::Sprite { part, cut, at }),
            }),
        }
    }
    out
}

/// The assignment panel alone (605: "Left" and the days, the date, the year disc), as the
/// game shows it on white while a lost battle's day passes, and with the field effect's gauge
/// (when `v` has it) on the maps.
pub fn date_panel(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<layout::Text>, v: Values) -> Vec<Element> {
    let panel: Vec<Placed> = nodes.iter().filter(|p| matches!(p.path.get(1), Some(&PANEL) | Some(&GAUGE))).cloned().collect();
    elements(&panel, texts, v)
}

/// The maps' button lists (layout 636: Info, Assignment, System on the left, Requests and
/// Message Log on the right), placed by the game at run time where the recording has them.
pub const MAP_LIST: u32 = 636;
pub const MAP_LIST_AT: [i32; 2] = [6, 854];
/// The left list's first row as the game words it (strings in the executable next to each
/// other): on the world map, and on an area map.
pub const AREA_INFO: &str = "<IM04> Area Info";
pub const RETURN: &str = "<IM02> Return";

/// The maps' button lists (`MAP_LIST`'s placed nodes, as stored at its own place), the first
/// row's text `first`; with none (the world map's cursor on the town), no first row: the left
/// plate (part 0x8f cut 0, three 64-pixel rows) from its second row down, as the
/// colseit-arrival recording has it at 1:25 (Assignment and System where they always stand).
pub fn map_list(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<layout::Text>, first: Option<&str>) -> Vec<Element> {
    let mut out = Vec::new();
    let mut first_done = false;
    for p in nodes.iter().filter(|p| p.path.first() == Some(&MAP_LIST)) {
        let at = [p.at[0] + MAP_LIST_AT[0], p.at[1] + MAP_LIST_AT[1]];
        if p.node.part == 0x164 {
            match texts(p) {
                Some(t) if !t.text.is_empty() && !first_done => {
                    if let Some(first) = first {
                        out.push(Element::Label(layout::Text { at, text: first.to_string(), ..t }));
                    }
                    first_done = true;
                }
                Some(t) if !t.text.is_empty() => out.push(Element::Label(layout::Text { at, ..t })),
                _ => {}
            }
        } else if first.is_none() && p.node.part == LIST_PLATE && p.node.cut == 0 {
            out.push(Element::Rising { part: LIST_PLATE, cut: 0, at, fill: 2.0 / 3.0 });
        } else {
            out.push(match Element::stored(p) {
                Element::Mirrored { part, cut, axis, .. } => Element::Mirrored { part, cut, at, axis },
                e => e.sprite().map_or(e, |(part, cut, _)| Element::Sprite { part, cut, at }),
            });
        }
    }
    out
}

/// The button lists' plates (layout 636's left one: cut 0, its three rows in one).
const LIST_PLATE: i32 = 0x8f;

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
            &Element::Sprite { part, cut, at } | &Element::Dim { part, cut, at } | &Element::Mirrored { part, cut, at, .. } | &Element::Scaled { part, cut, at, .. } | &Element::Faded { part, cut, at, .. } => {
                let scale = if let Element::Scaled { percent, .. } = *e { percent as f32 / 100.0 } else { 1.0 };
                let alpha = match *e {
                    Element::Dim { .. } => 0.5,
                    Element::Faded { alpha, .. } => f32::from(alpha) / 255.0,
                    _ => 1.0,
                };
                let Some(piece) = layout::sprite(pe, part, cut, at) else { continue };
                let Some(slot) = sheets.keys.iter().position(|(n, i)| *n == piece.texture && *i == piece.index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let [l, t, r, b] = piece.src.map(f32::from);
                let [x, y] = piece.at.map(|v| v as f32);
                out.push(Quad { turn: false,
                    rect: [dx + x * k, y * k, dx + (x + (r - l) * scale) * k, (y + (b - t) * scale) * k],
                    uv: match *e {
                        Element::Mirrored { axis: 1, .. } => [l / w, b / h, r / w, t / h],
                        Element::Mirrored { axis: 2, .. } => [r / w, b / h, l / w, t / h],
                        Element::Mirrored { .. } => [r / w, t / h, l / w, b / h],
                        _ => [l / w, t / h, r / w, b / h],
                    },
                    color: [1.0, 1.0, 1.0, alpha],
                    texture: sheets.first + slot,
                });
            }
            &Element::Spanned { part, cut, at, width } => {
                let Some(piece) = layout::sprite(pe, part, cut, at) else { continue };
                let Some(slot) = sheets.keys.iter().position(|(n, i)| *n == piece.texture && *i == piece.index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let [l, t, r, b] = piece.src.map(f32::from);
                let [x, y] = piece.at.map(|v| v as f32);
                let half = r - l;
                let width = (width as f32).max(half * 2.0);
                let (top, bottom) = (y * k, (y + b - t) * k);
                let mut quad = |x0: f32, x1: f32, u0: f32, u1: f32| out.push(Quad { turn: false, rect: [dx + x0 * k, top, dx + x1 * k, bottom], uv: [u0 / w, t / h, u1 / w, b / h], color: [1.0; 4], texture: sheets.first + slot });
                quad(x, x + half, l, l + half);
                quad(x + half, x + width - half, r - 1.0, r - 0.5);
                quad(x + width - half, x + width, r, l);
            }
            &Element::Turned { part, cut, at } => {
                let Some(piece) = layout::sprite(pe, part, cut, at) else { continue };
                let Some(slot) = sheets.keys.iter().position(|(n, i)| *n == piece.texture && *i == piece.index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let [l, t, r, b] = piece.src.map(f32::from);
                let [x, y] = piece.at.map(|v| v as f32);
                out.push(Quad { turn: true, rect: [dx + (x - (b - t)) * k, y * k, dx + x * k, (y + r - l) * k], uv: [l / w, t / h, r / w, b / h], color: [1.0; 4], texture: sheets.first + slot });
            }
            &Element::Cropped { part, cut, at, fill } => {
                let Some(piece) = layout::sprite(pe, part, cut, at) else { continue };
                let Some(slot) = sheets.keys.iter().position(|(n, i)| *n == piece.texture && *i == piece.index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let [l, t, r, b] = piece.src.map(f32::from);
                let r = l + (r - l) * fill.clamp(0.0, 1.0);
                let [x, y] = piece.at.map(|v| v as f32);
                out.push(Quad { turn: false, rect: [dx + x * k, y * k, dx + (x + r - l) * k, (y + b - t) * k], uv: [l / w, t / h, r / w, b / h], color: [1.0; 4], texture: sheets.first + slot });
            }
            &Element::Rising { part, cut, at, fill } => {
                let Some(piece) = layout::sprite(pe, part, cut, at) else { continue };
                let Some(slot) = sheets.keys.iter().position(|(n, i)| *n == piece.texture && *i == piece.index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let [l, t, r, b] = piece.src.map(f32::from);
                let top = b - (b - t) * fill.clamp(0.0, 1.0);
                let [x, y] = piece.at.map(|v| v as f32);
                out.push(Quad { turn: false, rect: [dx + x * k, (y + top - t) * k, dx + (x + r - l) * k, (y + b - t) * k], uv: [l / w, top / h, r / w, b / h], color: [1.0; 4], texture: sheets.first + slot });
            }
            Element::Picture { pack, index, size, at, dim } => {
                let Some(slot) = sheets.keys.iter().position(|(n, i)| n == pack && i == index) else { continue };
                let [x, y] = at.map(|v| v as f32);
                out.push(Quad { turn: false,
                    rect: [dx + x * k, y * k, dx + (x + size[0]) * k, (y + size[1]) * k],
                    uv: [0.0, 0.0, 1.0, 1.0],
                    color: [1.0, 1.0, 1.0, if *dim { 0.5 } else { 1.0 }],
                    texture: sheets.first + slot,
                });
            }
            Element::Slice { pack, index, src, rect: [l, t, r, b] } => {
                let Some(slot) = sheets.keys.iter().position(|(n, i)| n == pack && i == index) else { continue };
                let [w, h] = sheets.sizes[slot];
                let x = |v: i32| dx + v as f32 * k;
                out.push(Quad { turn: false,
                    rect: [x(*l), *t as f32 * k, x(*r), *b as f32 * k],
                    uv: [src[0] / w, src[1] / h, src[2] / w, src[3] / h],
                    color: [1.0; 4],
                    texture: sheets.first + slot,
                });
            }
            Element::Fill { rect: [l, t, r, b], colour } => {
                let x = |v: i32| dx + v as f32 * k;
                out.push(Quad { turn: false, rect: [x(*l), *t as f32 * k, x(*r), *b as f32 * k], uv: [-1.0, 0.0, 0.0, 0.0], color: *colour, texture: 0 });
            }
            Element::Label(t) => {
                let Some(font) = font else { continue };
                let [a, r, g, b] = t.colour;
                let colour = if a == 0 { [1.0; 4] } else { [r, g, b, a].map(|c| f32::from(c) / 255.0) };
                let scale = t.size * k / crate::message::LINE * 1.47;
                // Kind 23 texts end at their node (the numbers at x 1800 in the save list's
                // details), kind 22 are centred on it ("Select your character." at x 954, the
                // HUD's days left), the others start there.
                let width = crate::message::width_of(font, &t.text) * scale;
                let x = dx + t.at[0] as f32 * k - match t.kind {
                    23 => width,
                    22 => width / 2.0,
                    _ => 0.0,
                };
                crate::message::line(font, &t.text, x, t.at[1] as f32 * k, scale, colour, &mut out);
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
    use barrel_escha::data::layout::Node;

    fn node(path: &[u32], part: i32, cut: i32, kind: i32, at: [i32; 2]) -> Placed {
        Placed { path: path.to_vec(), node: Node { part, cut, at: [0, 0], kind, mirror: 0 }, at, va: 0 }
    }

    #[test]
    fn values_follow_the_calendar() {
        // April 1st of year 1: 119 days to the first period's last.
        assert_eq!(Values::on(90, 0), Values { left: 119, month: 4, day: 1, year: 1, tenths_spent: 0, cards: [Card::Empty; 10], gauge: None });
        // August 20th of year 1, as the game's screenshot: "Left 100", "8 /20", year 1.
        let aug20 = barrel_escha::calendar::Date { year: 1, month: 8, day: 20 }.number();
        assert_eq!(Values::on(aug20, 3), Values { left: 100, month: 8, day: 20, year: 1, tenths_spent: 3, cards: [Card::Empty; 10], gauge: None });
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
        let v = Values { left: 98, month: 12, day: 120, year: 2, tenths_spent: 1, cards: [Card::Empty; 10], gauge: None };
        let got = elements(&nodes, |_| None, v);
        let sprite = |part, cut, at| Element::Sprite { part, cut, at };
        assert_eq!(got, vec![
            sprite(DIGITS, 1, [104, 128]),
            sprite(DIGITS, 2, [140, 128]),
            sprite(DIGITS, 1, [180, 128]),
            sprite(DIGITS, 2, [216, 128]),
            sprite(DIGITS, 0, [252, 128]),
            sprite(YEAR, 4, [306, 124]),
            sprite(0x8b, 6, [20, 8]),
            sprite(DIAL, 0, [0, 0]),
            sprite(DIAL, 2, [1, 0]),
        ]);
    }

    #[test]
    fn request_cards_by_slot() {
        let card = |cut, at| node(&[625, 605, CARD], 0x8b, cut, 0, at);
        let nodes = vec![card(3, [0, 0]), card(1, [4, 12]), card(12, [20, -2]), card(3, [66, 0]), card(1, [70, 12]), card(12, [86, -2]), card(3, [132, 0]), card(1, [136, 12]), card(12, [152, -2])];
        let mut v = Values::on(90, 0);
        v.cards[0] = Card::Done;
        v.cards[1] = Card::Open;
        let got: Vec<i32> = elements(&nodes, |_| None, v).iter().filter_map(|e| e.sprite()).map(|(_, cut, _)| cut).collect();
        assert_eq!(got, vec![5, 1, 12, 3, 12, 6, 12]);
    }

    #[test]
    fn field_gauge_as_the_area_map_shows_it() {
        let g = |part, cut, kind, at| node(&[625, GAUGE], part, cut, kind, at);
        let nodes = vec![
            g(0x69, 4, 0, [120, 252]),
            g(0x69, 5, 0, [120, 314]),
            g(0x69, 2, 0, [32, 246]),
            g(0xe5, 0, 28, [96, 262]),
            g(0x69, 6, 0, [58, 310]),
            g(0x6e, 0, 0, [134, 258]),
            node(&[625, GAUGE, 611], 0x69, 3, 0, [132, 332]),
            g(0x6f, 0, 36, [50, 360]),
            g(0x69, 17, 0, [50, 298]),
            g(0xe5, 13, 0, [96, 266]),
            g(0xf0, 0, 36, [38, 256]),
        ];
        let mut v = Values::on(90, 0);
        assert!(elements(&nodes, |_| None, v).is_empty(), "no gauge, nothing drawn");
        // The user's save: 45, mode 4 ("Rare Material Changed").
        v.gauge = Some(Gauge { percent: 45, mode: 4 });
        let got = elements(&nodes, |_| None, v);
        let s = |part, cut, at: [i32; 2]| Element::Sprite { part, cut, at: [at[0] + 4, at[1]] };
        assert_eq!(got, vec![
            s(0x69, 4, [120, 252]),
            s(0x69, 2, [32, 246]),
            s(0xe5, 4, [60, 262]),
            s(0xe5, 5, [78, 262]),
            Element::Rising { part: 0x69, cut: 6, at: [62, 310], fill: 0.225 },
            s(0x6e, 1, [134, 258]),
            s(0x69, 17, [50, 298]),
            s(0xe5, 13, [96, 266]),
        ]);
        // No effect: no plate or notice; full: "MAX" for the number.
        v.gauge = Some(Gauge { percent: 200, mode: -1 });
        let got: Vec<(i32, i32)> = elements(&nodes, |_| None, v).iter().filter_map(|e| e.sprite()).map(|(p, c, _)| (p, c)).collect();
        assert_eq!(got, vec![(0x69, 2), (0x69, 6), (0x69, 17), (0xf0, 0)]);
    }
}
