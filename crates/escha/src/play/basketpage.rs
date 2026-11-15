//! The main menu's Basket page as the game draws it (the `menu-cmd-0` capture): over the desk
//! (383) the footer line (372), the item grid (layout 568: its scroll bar, category tabs, the
//! "Empty" count of free places and the cells, 30 of them, six rows of five), the chosen item's
//! card (752: name, Alchemy LV, the EFF gauge, Stats, the four elements, the "Item Info" lines
//! with their page count), the "Possessed" box (706: how many of it the basket and the container
//! hold), then the header line (371) and the tab (531).
//!
//! The game puts the screen together at run time from these layouts at places of its own:
//! the grid from (74, 152) of its layout, the card at (600, 162) with its "Item Info" part 2 right
//! and 18 up, the "Possessed" box at (1110, 144), and the two panels behind them (parts 0x61,
//! stretched). Fitted to the capture: those places and the panels' sizes.
//!
//! What a node shows is the item's: the cell's picture is part 0xd7 cut `ImageNo` and the card's
//! part 0xd6 (the item picture sheets the layouts index), both at 70%; the card's Alchemy LV is
//! `itemData`'s `level`, its Stats number the element value (`value`, the bar fills to 50), the
//! four element icons light for the elements the item has (`fire`, `water`, `wind`, `soil`), the
//! EFF gauge fills to the quality over 120 (the quality's cap, docs/tables.md); the lines under
//! "Item Info" are the item's properties (`item_status.xml`'s ESP tags, named from
//! `item_mat_pot.xml`), its effects (the unit's effect rows) and its traits (`item_potential.xml`).
//!
//! Not yet the game's: the cursor glow's cut and place (time), the tabs other than the first (the
//! page lists "all items" only), the pages of "Item Info" beyond the first, the effect lines'
//! values, and equipment's "Equip. Cost".

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_formats::pe::Pe;

use crate::basket::{Basket, Item, COLUMNS};
use crate::hud::{Element, TEXT_RISE};

/// The header's line in  ("Please select an item.").
pub const HEADER_ROW: usize = 36;
const DESK: u32 = 383;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const TAB: u32 = 531;
pub const GRID: u32 = 568;
pub const CARD: u32 = 752;
pub const POSSESSED: u32 = 706;
/// The nested layouts' ids inside the grid and the card.
const SCROLL: u32 = 563;
const TABS: u32 = 558;
const COUNT: u32 = 559;
const FRAME: u32 = 565;
const CELL: u32 = 561;
const INFO: u32 = 748;
const SECTIONS: u32 = 742;
const OTHER_SECTIONS: u32 = 744;
const PAGER: u32 = 751;
const HIDDEN_IN_CARD: [u32; 1] = [OTHER_SECTIONS];
const BADGES: u32 = 719;
const NAME: u32 = 733;
const LEVEL: u32 = 727;
const EFFECT: u32 = 731;
const STATS: u32 = 739;
const STAT_BAR: u32 = 737;
const BAR_KIND: i32 = 20;
const STRETCHED: i32 = 15;
const NUMBER_ENDS: i32 = 28;
const NUMBER_CENTRED: i32 = 29;
const TEXT: i32 = 0x164;
const DESK_CUT: i32 = 1;

const GRID_AT: [i32; 2] = [74, 152];
/// Where the second count box (the container's) starts in the grid's layout.
const SECOND_BOX: i32 = 246;
const CARD_AT: [i32; 2] = [600, 162];
/// Where "Item Info" sits from the card's layout (part 2 right, 18 up).
const INFO_SHIFT: [i32; 2] = [2, -18];
const POSSESSED_AT: [i32; 2] = [1110, 144];
/// The panels behind the card and the box: place and size.
const CARD_PANEL: ([i32; 2], [i32; 2]) = ([616, 152], [440, 820]);
const POSSESSED_PANEL: [i32; 2] = [300, 192];
const PANEL: i32 = 0x61;
/// The box's heading is drawn 70% as wide as its layout (the bar and the places along it).
const HEADING_SCALE: f32 = 0.7;

/// Cells in the grid: six rows of five.
pub const CELLS: usize = 30;
/// The category tabs the page shows, the lit one first, and their icons (part 0x45's cuts).
const TAB_ICONS: [i32; 3] = [29, 32, 30];
const TAB_LIT: i32 = 1;
/// The scroll bar's thumb: the track's length less this much for each row past five.
const THUMB_PER_ROW: i32 = 30;
/// The EFF gauge's full quality, the Stats bar's full value.
const QUALITY_CAP: f32 = 120.0;
const STAT_CAP: f32 = 50.0;
const PICTURE_PERCENT: i32 = 70;
/// A line's step in the "Item Info" groups.
const LINE_STEP: i32 = 36;
/// The groups' lines: properties, effects and traits.
const GROUPS: [usize; 3] = [2, 4, 3];

/// The layouts the page is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
    tab: Vec<Placed>,
    grid: Vec<Placed>,
    card: Vec<Placed>,
    possessed: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        Layouts { desk: get(DESK), header: get(HEADER), footer: get(FOOTER), tab: get(TAB), grid: get(GRID), card: get(CARD), possessed: get(POSSESSED) }
    }
}

/// The page's two lines of words: what the header says and the footer's button help.
pub struct Words {
    pub header: String,
    pub footer: String,
}

fn shifted(p: &Placed, by: [i32; 2]) -> Placed {
    Placed { at: [p.at[0] + by[0], p.at[1] + by[1]], ..p.clone() }
}

/// A text of a node as the game draws it: raised by `TEXT_RISE`, opaque (the layouts store the
/// ink's alpha as 0xe1, the game draws it whole).
fn raised(t: Text, text: &str) -> Element {
    let colour = if t.colour[0] == 0 { t.colour } else { [0xff, t.colour[1], t.colour[2], t.colour[3]] };
    Element::Label(Text { at: [t.at[0], t.at[1] - TEXT_RISE], text: text.into(), colour, ..t })
}

/// A number node's ink (its word 7, stored r, g, b, a).
fn ink(pe: &Pe, p: &Placed) -> [u8; 3] {
    let [r, g, b, _] = layout::word(pe, p, 7).unwrap_or(0).to_le_bytes();
    [r, g, b]
}

/// A number from digit sprites of the node's part in the node's ink: centred on the node (kind
/// 29) or ending at it (kind 28, the last digit just left of the node).
fn number(pe: &Pe, p: &Placed, value: i32) -> Vec<Element> {
    let part = p.node.part;
    let advance = layout::sprite(pe, part, 0, [0, 0]).map_or(20, |s| i32::from(s.src[2] - s.src[0]));
    let digits = value.max(0).to_string();
    let n = digits.len() as i32;
    let first = if p.node.kind == NUMBER_ENDS { p.at[0] - advance * n } else { p.at[0] - advance * n / 2 };
    let rgb = ink(pe, p);
    digits.bytes().enumerate().map(|(i, d)| Element::Tinted { part, cut: i32::from(d - b'0'), at: [first + advance * i as i32, p.at[1]], rgb }).collect()
}

/// A piece cut in nine (`part`'s cuts 0 to 8 in rows) tiled over `size` from `at`, in the game's
/// order: the top row, the left column, the right column, the bottom row, then the middle row by
/// row. The edge and middle tiles repeat at their size, the last one cut short; a middle there is
/// no room for is still drawn, empty.
fn tiled(pe: &Pe, part: i32, at: [i32; 2], size: [i32; 2]) -> Vec<Element> {
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
fn vertical_bar(pe: &Pe, part: i32, at: [i32; 2], length: i32) -> Vec<Element> {
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

/// The grid's elements: the scroll bar, the tabs, the free places, the frame and the cells.
fn grid(pe: &Pe, l: &Layouts, view: &Basket, items: &[Item], empty: i32, cursor: &mut Option<[i32; 2]>) -> Vec<Element> {
    let mut out = Vec::new();
    let rows = items.len().div_ceil(COLUMNS).max(1) as i32;
    let (mut tab, mut counted) = (0, false);
    let mut places: Option<Vec<Element>> = None;
    // Each cell's nodes: the frame (part 0x44) starts one.
    let mut cells: Vec<Vec<Placed>> = Vec::new();
    for p in l.grid.iter().map(|p| shifted(p, GRID_AT)) {
        let n = p.node;
        match p.path.get(1..).unwrap_or(&[]) {
            [_, SCROLL] if n.kind == BAR_KIND => {
                let track = layout::word(pe, &p, 6).unwrap_or(0);
                let length = (track - THUMB_PER_ROW * (rows - 5).max(0)).max(THUMB_PER_ROW);
                let top = if rows > 6 { (track - length) * view.first as i32 / (rows - 6) } else { 0 };
                out.extend(vertical_bar(pe, n.part, [p.at[0], p.at[1] + top], length));
            }
            [_, SCROLL] => out.push(Element::stored(&p)),
            [_, _, TABS] => {
                // A tab is a plate and then its icon; the page shows the first few.
                let k = tab / 2;
                if k < TAB_ICONS.len() {
                    let cut = if tab % 2 == 1 { TAB_ICONS[k] } else if k == 0 { TAB_LIT } else { 0 };
                    out.push(Element::Sprite { part: n.part, cut, at: p.at });
                }
                tab += 1;
            }
            // The basket's own count of free places (the first of the two boxes) and its label.
            [_, COUNT, ..] if p.at[0] < GRID_AT[0] + SECOND_BOX => match (n.part, n.kind) {
                (TEXT, _) => {
                    if let Some(t) = layout::text(pe, &p).filter(|t| t.text == "Empty") {
                        out.push(raised(t, "Empty"));
                        out.extend(places.take().unwrap_or_default());
                    }
                }
                (_, NUMBER_CENTRED) if !counted => {
                    counted = true;
                    places = Some(number(pe, &p, empty));
                }
                (0x45, _) => out.push(Element::stored(&p)),
                _ => {}
            },
            [FRAME] if n.part == 0x45 => out.push(Element::stored(&p)),
            [_, CELL, ..] => {
                if n.part == 0x44 {
                    cells.push(Vec::new());
                }
                if let Some(c) = cells.last_mut() {
                    c.push(p);
                }
            }
            _ => {}
        }
    }
    for (slot, cell) in cells.iter().enumerate() {
        let k = view.first * COLUMNS + slot;
        let Some(item) = items.get(k) else { continue };
        let Some(picture) = cell.iter().find(|p| p.node.part == 0xd7) else { continue };
        if k == view.chosen {
            out.push(Element::Sprite { part: 0x44, cut: 1, at: cell[0].at });
            *cursor = Some([cell[0].at[0] + CURSOR_FROM_CELL[0], cell[0].at[1] + CURSOR_FROM_CELL[1]]);
        }
        out.push(Element::Scaled { part: 0xd7, cut: item.picture as i32, at: picture.at, percent: PICTURE_PERCENT, about: picture.at });
    }
    out
}

/// Line `k` of the card's three groups (properties, effects, traits), if the item has it.
fn line_of(item: &Item, k: usize) -> Option<String> {
    let lists = [&item.properties, &item.effects, &item.traits];
    let mut start = 0;
    for (list, size) in lists.into_iter().zip(GROUPS) {
        if k < start + size {
            return list.get(k - start).cloned();
        }
        start += size;
    }
    None
}

/// The card's elements for `item`.
fn card(pe: &Pe, l: &Layouts, item: &Item) -> Vec<Element> {
    let mut out = tiled(pe, PANEL, CARD_PANEL.0, CARD_PANEL.1);
    let (mut line, mut bars) = (0, 0);
    let section_y: Vec<i32> = {
        // Each group's first bar, from the layout's: the bars follow in steps of `LINE_STEP`.
        let first = |g: usize| GROUPS[..g].iter().sum::<usize>();
        let ys: Vec<i32> = l.card.iter().filter(|p| p.path.get(2) == Some(&SECTIONS) && p.node.part == 0xbe && p.node.cut == 2).map(|p| p.at[1]).collect();
        (0..GROUPS.len()).filter_map(|g| ys.get(first(g)).copied()).collect()
    };
    for p in &l.card {
        let n = p.node;
        let inside = |id: u32| p.path.contains(&id);
        if HIDDEN_IN_CARD.iter().any(|&h| inside(h)) {
            continue;
        }
        let at = if inside(INFO) { [p.at[0] + CARD_AT[0] + INFO_SHIFT[0], p.at[1] + CARD_AT[1] + INFO_SHIFT[1]] } else { [p.at[0] + CARD_AT[0], p.at[1] + CARD_AT[1]] };
        let placed = Placed { at, ..p.clone() };
        match (n.part, n.cut) {
            (TEXT, _) => {
                let Some(t) = layout::text(pe, &placed) else { continue };
                let text = if inside(NAME) {
                    Some(item.name.clone())
                } else if inside(SECTIONS) {
                    line += 1;
                    line_of(item, line - 1)
                } else if t.filled_in() || t.text == "Equip. Cost" {
                    None
                } else {
                    Some(t.text.clone())
                };
                out.extend(text.map(|s| raised(t, &s)));
            }
            (0xbe, 2) if inside(SECTIONS) => {
                // Bars within a group are a line apart (the layout's fourth of the second group is 4 off).
                let g = GROUPS.iter().scan(0, |s, &n| { *s += n; Some(*s) }).position(|end| bars < end).unwrap_or(0);
                let k = bars - GROUPS[..g].iter().sum::<usize>();
                bars += 1;
                let y = section_y.get(g).map_or(at[1], |&y| y + CARD_AT[1] + INFO_SHIFT[1] + LINE_STEP * k as i32);
                out.push(Element::Sprite { part: 0xbe, cut: 2, at: [at[0], y] });
            }
            (0xbe, 10..=12) if inside(SECTIONS) => out.push(Element::stored(&placed)),
            (0xbe, 2) if inside(LEVEL) || p.path.contains(&725) => out.push(Element::stored(&placed)),
            (0x5, _) if inside(LEVEL) => out.push(Element::Scaled { part: 0x5, cut: n.cut, at, percent: layout::word(pe, &placed, 5).filter(|&v| v > 0).unwrap_or(100), about: at }),
            (0x4d, _) if inside(LEVEL) => out.extend(number(pe, &placed, item.level)),
            (0xbc, 10) | (0xbc, 16) if inside(EFFECT) => out.push(Element::stored(&placed)),
            (0xbc, 11) if inside(EFFECT) => out.push(Element::Cropped { part: 0xbc, cut: 11, at, fill: (item.quality as f32 / QUALITY_CAP).clamp(0.0, 1.0) }),
            (0xbe, 14) if inside(STAT_BAR) => out.push(Element::stored(&placed)),
            // The Stats bar's fill is the whole bar squeezed to the value's share, not cropped.
            (0xbe, 15) if inside(STAT_BAR) => {
                let Some(piece) = layout::sprite(pe, 0xbe, 15, at) else { continue };
                let [l0, t0, r0, b0] = piece.src.map(f32::from);
                let width = ((r0 - l0) * (item.value as f32 / STAT_CAP).clamp(0.0, 1.0)).round() as i32;
                out.push(Element::Slice { pack: piece.texture, index: piece.index, src: [l0, t0, r0, b0], rect: [at[0], at[1], at[0] + width, at[1] + (b0 - t0) as i32] });
            }
            (0x50, _) if inside(STATS) => out.extend(number(pe, &placed, item.value)),
            (0xc1, cut @ 25..=31) if inside(STATS) && cut % 2 == 1 => out.push(Element::Scaled { part: 0xc1, cut, at, percent: PICTURE_PERCENT, about: at }),
            (0xc1, cut @ 24..=30) if inside(STATS) => {
                if item.elements[((cut - 24) / 2) as usize] {
                    out.push(Element::Scaled { part: 0xc1, cut, at, percent: PICTURE_PERCENT, about: at });
                }
            }
            (0xbc, 0) if inside(BADGES) => out.push(Element::stored(&placed)),
            (0xd6, _) if inside(BADGES) => out.push(Element::Scaled { part: 0xd6, cut: item.picture as i32, at, percent: PICTURE_PERCENT, about: at }),
            (0xbc, 1) if inside(NAME) || p.path.len() == 2 => out.push(Element::stored(&placed)),
            (0x63, _) if inside(PAGER) => out.push(Element::stored(&placed)),
            (0x4f, cut) if inside(PAGER) => {
                if n.kind == NUMBER_CENTRED {
                    out.extend(number(pe, &placed, 1));
                } else if cut == 12 {
                    out.push(Element::Tinted { part: 0x4f, cut: 12, at, rgb: ink(pe, &placed) });
                }
            }
            _ => {}
        }
    }
    out
}

/// The "Possessed" box: how many of the item the basket and the container hold.
fn possessed(pe: &Pe, l: &Layouts, item: &Item) -> Vec<Element> {
    let mut out = Vec::new();
    let mut counts = [item.in_basket, item.in_container].into_iter();
    let origin = POSSESSED_AT;
    for p in &l.possessed {
        let n = p.node;
        let at = [p.at[0] + origin[0], p.at[1] + origin[1]];
        let placed = Placed { at, ..p.clone() };
        if n.part == PANEL && n.kind == STRETCHED {
            out.extend(tiled(pe, PANEL, origin, POSSESSED_PANEL));
        } else if p.path.get(1) == Some(&730) {
            // The heading's bar and its two ends, across 70% of the layout's width.
            let Some(piece) = layout::sprite(pe, n.part, n.cut, [0, 0]) else { continue };
            let [l0, t0, r0, b0] = piece.src.map(f32::from);
            let left = origin[0] as f32 + 4.0 + n.at[0] as f32 * HEADING_SCALE;
            let right = left + (r0 - l0) * HEADING_SCALE;
            out.push(Element::Slice { pack: piece.texture, index: piece.index, src: [l0, t0, r0, b0], rect: [left.round() as i32, at[1], right.round() as i32, at[1] + (b0 - t0) as i32] });
        } else if n.part == TEXT {
            if let Some(t) = layout::text(pe, &placed) {
                out.push(raised(t.clone(), &t.text));
            }
        } else if n.part == 0x4c {
            out.extend(number(pe, &placed, counts.next().unwrap_or(0)));
        } else if n.part == 0x2a {
            out.push(Element::Scaled { part: 0x2a, cut: n.cut, at, percent: layout::word(pe, &placed, 5).filter(|&v| v > 0).unwrap_or(100), about: at });
        }
    }
    out
}

/// All the page's elements: `view` the grid's cursor and scroll, `items` the basket's (the game's
/// order), `empty` the free places.
pub fn elements(pe: &Pe, l: &Layouts, words: &Words, view: &Basket, items: &[Item], empty: i32) -> Vec<Element> {
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: DESK_CUT, at: p.at }).collect();
    let line = |nodes: &[Placed], text: &str| -> Vec<Element> {
        nodes.iter().flat_map(|p| match layout::text(pe, p) {
            Some(t) => vec![raised(t, text)],
            None => vec![Element::stored(p)],
        }).collect()
    };
    out.extend(line(&l.footer, &words.footer));
    let mut cursor = None;
    out.extend(grid(pe, l, view, items, empty, &mut cursor));
    if let Some(item) = items.get(view.chosen) {
        out.extend(card(pe, l, item));
        out.extend(possessed(pe, l, item));
    }
    out.extend(line(&l.header, &words.header));
    for p in &l.tab {
        out.push(match Element::stored(p) {
            Element::Sprite { part, at, .. } if part == 0x2e || part == 0x2c => Element::Sprite { part, cut: TAB_CUT, at },
            e => e,
        });
    }
    // The cursor's glow, drawn last (it cycles its eight cuts by time: here the first).
    out.extend(cursor.map(|at| Element::Sprite { part: CURSOR, cut: 0, at }));
    out
}

/// The cursor's glow (part 0x9f) from the chosen cell's frame (the capture: the glow of 96
/// pixels at (36, 139) with the first cell, whose frame is at (104, 214)).
const CURSOR: i32 = 0x9f;
const CURSOR_FROM_CELL: [i32; 2] = [-68, -75];
/// The footer's row of `Menu_Line_Help.xml` ("Select, Details, Submenu, Return, Confirm").
pub const FOOTER_ROW: usize = 5;

/// The menu's Basket command's cut in the tab's title and icon parts (the second of the commands).
const TAB_CUT: i32 = 1;
