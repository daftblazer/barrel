//! Pieces the menu pages are built from: layout nodes moved to where the game sets them, texts
//! as the game draws them, numbers from digit sprites, and the stretched panels and bars.

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_formats::pe::Pe;

use crate::hud::{Element, TEXT_RISE};

/// The layout of the cursor's glow (part 0x9f): one flipbook node of eight cuts, eight ticks each.
const CURSOR_LAYOUT: u32 = 691;

/// The cursor glow's flipbook, read from its layout (the one the pages, the lists and the menu
/// share: all their glows turn together).
pub fn cursor_glow(pe: &Pe) -> Option<layout::Flip> {
    layout::placed(pe, CURSOR_LAYOUT).iter().find_map(|p| p.node.flip())
}

const NUMBER_ENDS: i32 = 28;

/// Every cut of `part`'s flipbook, for loading the pictures a screen may show.
pub fn cuts_of(part: i32, flip: layout::Flip) -> impl Iterator<Item = Element> {
    (0..flip.cuts).map(move |k| Element::Sprite { part, cut: flip.first + k, at: [0, 0] })
}

pub fn shifted(p: &Placed, by: [i32; 2]) -> Placed {
    Placed { at: [p.at[0] + by[0], p.at[1] + by[1]], ..p.clone() }
}

/// A text of a node as the game draws it: raised by `TEXT_RISE`, opaque (the layouts store the
/// ink's alpha as 0xe1, the game draws it whole).
pub fn raised(t: Text, text: &str) -> Element {
    let colour = if t.colour[0] == 0 { t.colour } else { [0xff, t.colour[1], t.colour[2], t.colour[3]] };
    Element::Label(Text { at: [t.at[0], t.at[1] - TEXT_RISE], text: text.into(), colour, ..t })
}

/// A number node's ink (its word 7, stored r, g, b, a).
pub fn ink(pe: &Pe, p: &Placed) -> [u8; 3] {
    let [r, g, b, _] = layout::word(pe, p, 7).unwrap_or(0).to_le_bytes();
    [r, g, b]
}

/// A number from digit sprites of the node's part in the node's ink: centred on the node (kind
/// 29) or ending at it (kind 28, the last digit just left of the node).
pub fn number(pe: &Pe, p: &Placed, value: i32) -> Vec<Element> {
    let part = p.node.part;
    let advance = layout::sprite(pe, part, 0, [0, 0]).map_or(20, |s| i32::from(s.src[2] - s.src[0]));
    // The node's word 5 is the largest value it shows.
    let most = layout::word(pe, p, 5).filter(|&m| m > 0).unwrap_or(i32::MAX);
    let digits = value.clamp(0, most).to_string();
    let n = digits.len() as i32;
    let first = if p.node.kind == NUMBER_ENDS { p.at[0] - advance * n } else { p.at[0] - advance * n / 2 };
    // A node with no ink (word 7 clear) shows its digits as the sheet has them.
    let plain = layout::word(pe, p, 7).unwrap_or(0) == 0;
    let rgb = ink(pe, p);
    digits
        .bytes()
        .enumerate()
        .map(|(i, d)| {
            let (cut, at) = (i32::from(d - b'0'), [first + advance * i as i32, p.at[1]]);
            if plain { Element::Sprite { part, cut, at } } else { Element::Tinted { part, cut, at, rgb } }
        })
        .collect()
}

/// A piece cut in nine (`part`'s cuts 0 to 8 in rows) tiled over `size` from `at`, in the game's
/// order: the top row, the left column, the right column, the bottom row, then the middle row by
/// row. The edge and middle tiles repeat at their size, the last one cut short; a middle there is
/// no room for is still drawn, empty.
pub fn tiled(pe: &Pe, part: i32, at: [i32; 2], size: [i32; 2]) -> Vec<Element> {
    let Some(cuts) = (0..9).map(|c| layout::sprite(pe, part, c, [0, 0])).collect::<Option<Vec<_>>>() else { return Vec::new() };
    let wide = |c: usize| i32::from(cuts[c].src[2] - cuts[c].src[0]);
    let high = |c: usize| i32::from(cuts[c].src[3] - cuts[c].src[1]);
    // The pieces along one axis: (cut column or row, start, length).
    let along = |first: i32, mid: i32, last: i32, total: i32| {
        let mut out = vec![(0, 0, first)];
        let mut x = first;
        loop {
            out.push((1, x, mid.min((total - last - x).max(0))));
            x += mid;
            if x >= total - last {
                break;
            }
        }
        out.push((2, total - last, last));
        out
    };
    let xs = along(wide(0), wide(1), wide(2), size[0]);
    let ys = along(high(0), high(3), high(6), size[1]);
    let slice = |c: usize, x: (usize, i32, i32), y: (usize, i32, i32)| {
        let p = &cuts[c];
        let [l, t, ..] = p.src.map(f32::from);
        Element::Slice { pack: p.texture.clone(), index: p.index, src: [l, t, l + x.2 as f32, t + y.2 as f32], rect: [at[0] + x.1, at[1] + y.1, at[0] + x.1 + x.2, at[1] + y.1 + y.2] }
    };
    let (top, bottom) = (ys[0], *ys.last().unwrap());
    let (left, right) = (xs[0], *xs.last().unwrap());
    let mut out: Vec<Element> = xs.iter().map(|&x| slice(x.0, x, top)).collect();
    let middle = &ys[1..ys.len() - 1];
    out.extend(middle.iter().map(|&y| slice(3, left, y)));
    out.extend(middle.iter().map(|&y| slice(5, right, y)));
    out.extend(xs.iter().map(|&x| slice(6 + x.0, x, bottom)));
    for &y in middle {
        out.extend(xs[1..xs.len() - 1].iter().map(|&x| slice(4, x, y)));
    }
    out
}

/// A bar of three cuts down (`part`'s cuts 0 to 2: top end, middle, bottom end; node kind 20)
/// over `length` from `at`: the middle repeated, its last repeat cut short.
pub fn vertical_bar(pe: &Pe, part: i32, at: [i32; 2], length: i32) -> Vec<Element> {
    let Some(cuts) = (0..3).map(|c| layout::sprite(pe, part, c, [0, 0])).collect::<Option<Vec<_>>>() else { return Vec::new() };
    let high = |c: usize| i32::from(cuts[c].src[3] - cuts[c].src[1]);
    let slice = |c: usize, y: i32, h: i32| {
        let p = &cuts[c];
        let [l, t, r, _] = p.src.map(f32::from);
        Element::Slice { pack: p.texture.clone(), index: p.index, src: [l, t, r, t + h as f32], rect: [at[0], at[1] + y, at[0] + (r - l) as i32, at[1] + y + h] }
    };
    let mut out = vec![slice(0, 0, high(0))];
    let mut y = high(0);
    while y < length - high(2) {
        out.push(slice(1, y, high(1).min(length - high(2) - y)));
        y += high(1);
    }
    out.push(slice(2, length - high(2), high(2)));
    out
}

/// A bar of three cuts (`part`'s cuts 0 to 2: left end, middle, right end) over `width` from `at`:
/// the middle repeated, its last repeat cut short.
pub fn horizontal_bar(pe: &Pe, part: i32, at: [i32; 2], width: i32) -> Vec<Element> {
    let Some(cuts) = (0..3).map(|c| layout::sprite(pe, part, c, [0, 0])).collect::<Option<Vec<_>>>() else { return Vec::new() };
    let wide = |c: usize| i32::from(cuts[c].src[2] - cuts[c].src[0]);
    let slice = |c: usize, x: i32, w: i32| {
        let p = &cuts[c];
        let [l, t, _, b] = p.src.map(f32::from);
        Element::Slice { pack: p.texture.clone(), index: p.index, src: [l, t, l + w as f32, b], rect: [at[0] + x, at[1], at[0] + x + w, at[1] + (b - t) as i32] }
    };
    let mut out = vec![slice(0, 0, wide(0))];
    let mut x = wide(0);
    while x < width - wide(2) {
        let w = wide(1).min(width - wide(2) - x);
        out.push(slice(1, x, w));
        x += wide(1);
    }
    out.push(slice(2, width - wide(2), wide(2)));
    out
}
