//! The Adventure Equip. page of the main menu's character picker (the `menu-equip-adventure`
//! capture): the alchemist's field gear. Over the desk (383) the container screen's item grid
//! (layout 456 at (44, 252), as layout 458 places it: the backdrop and empty cells for the rows
//! no gear sits in), the gear packed along its five columns, the member's name between arrows
//! (457 at (110, 164)), the chosen gear's frame pulsing, the member's figure (361), the footer
//! (372), the card's panel and the card (752), then the header (371), the tab (531) and the
//! cursor's glow.
//!
//! The gear is the member's battle items (`explore_equip`). Each takes as many cells of a row as
//! its `EquipCost` (a three-cell piece, a one-cell piece), a row too full for the next piece
//! wraps to the following one. A piece's number is its uses left.

use barrel_escha::data::layout::{self, Placed};
use barrel_formats::pe::Pe;

use crate::basket::Item;
use crate::hud::Element;
use crate::kit::{number, raised, shifted, tiled};

const GRID: u32 = 456;
const NAME_TAB: u32 = 457;
/// The cell layouts by columns spanned: 451 spans one, 455 five.
const FIRST_CELL_LAYOUT: u32 = 451;
const CARD_PANEL: i32 = 0x61;
/// Node parts of the grid and of a cell.
const BACKDROP: i32 = 0x5;
const EMPTY_CELL: i32 = 0x17;
const EMPTY_CUT: i32 = 2;
const STRIP: i32 = 0x18;
const PICTURE: i32 = 0xd7;
const NUMBER: i32 = 0x54;
const NAME_TEXT: i32 = 0x164;
const COLUMNS: usize = 5;
const FRAME_FROM_CELL_Y: i32 = -14;
/// Where layout 458 (the container screen) puts layout 456 from its own place (100, 252).
const GRID_SHIFT: [i32; 2] = [-56, 0];
/// The name tab's place (the layout's own is (0, 0); 458 puts it at (164, 164)).
const NAME_TAB_AT: [i32; 2] = [110, 164];
/// The card's panel (place, size) and the card's place: the Basket page's are (616, 152) with
/// (600, 162): this page moves both 20 right and 16 up. Fitted to the capture.
const PANEL: ([i32; 2], [i32; 2]) = ([636, 136], [440, 820]);
const CARD_AT: [i32; 2] = [620, 146];
/// "Item Info" is one page here (the pager reads 1/1 without the turning button).
const ITEM_INFO_PAGES: i32 = 1;
/// A strip is a cell's step (100) less 4 wide for each cell the gear spans.
const CELL_STEP: i32 = 100;
const STRIP_TRIM: i32 = 4;
/// The item's picture is drawn at 60 % of the sheet's 96 pixels (58 square).
const PICTURE_PERCENT: i32 = 60;
/// The glow's place from the lit frame (the capture: (-12, 186) with the frame at (62, 262)).
const CURSOR_FROM_FRAME: [i32; 2] = [-74, -76];
/// The lit frame's pulse (`battleequip::pulse_with`), its period and the tick it is white at,
/// fitted to the nine captured frames (frames 2700 to 2860 every 20, tick = frame + 204; three
/// match exactly, the rest are within 2 %: the game's own phase jitters).
const PULSE_PERIOD: f32 = 62.1;
const PULSE_WHITE_AT: f32 = 6.4;
/// The SysInfo.xml row ("Select a slot to change.") and the Menu_Line_Help.xml row (Select,
/// Details, Character, Move, Return, Confirm) of the header and footer.
pub const HEADER_ROW: usize = 45;
pub const FOOTER_ROW: usize = 29;

/// One piece of gear as the page shows it: the item (its card), the uses left (the number on
/// its cell) and the cells it takes in a row (its `EquipCost`).
#[derive(Clone, Debug)]
pub struct Gear {
    pub item: Item,
    pub uses: i32,
    pub span: usize,
}

/// What the page shows of the chosen member: their name and their gear.
pub struct Worn {
    pub name: String,
    pub gear: Vec<Gear>,
}

/// The layouts the page is made of, placed once.
pub struct Layouts {
    desk: Vec<Placed>,
    grid: Vec<Placed>,
    /// The cell layouts 451 to 455.
    cells: Vec<Vec<Placed>>,
    /// The cells' places in the screen, row by row.
    places: Vec<[i32; 2]>,
    name: Vec<Placed>,
    figure: Vec<Placed>,
    card: crate::basketpage::Layouts,
    cursor: Option<layout::Flip>,
    tab: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        let pick = crate::picker::Layouts::load(pe);
        let grid = get(GRID);
        // Each cell layout sits at its nodes' place less the node's own; rows tell by their y.
        let mut places: Vec<[i32; 2]> = Vec::new();
        for p in grid.iter().filter(|p| p.path.get(1).is_some_and(|c| (FIRST_CELL_LAYOUT..FIRST_CELL_LAYOUT + 5).contains(c))) {
            let origin = [p.at[0] - p.node.at[0] + GRID_SHIFT[0], p.at[1] - p.node.at[1] + GRID_SHIFT[1]];
            if !places.contains(&origin) {
                places.push(origin);
            }
        }
        places.sort_by_key(|o| (o[1], o[0]));
        Layouts {
            desk: get(crate::picker::DESK),
            grid,
            cells: (0..5).map(|k| get(FIRST_CELL_LAYOUT + k)).collect(),
            places,
            name: get(NAME_TAB),
            figure: get(crate::picker::FIGURE),
            card: crate::basketpage::Layouts::load(pe),
            cursor: crate::kit::cursor_glow(pe),
            tab: pick.tab,
            header: pick.header,
            footer: pick.footer,
        }
    }

    /// Every cut the page's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        self.cursor.into_iter().flat_map(|f| crate::kit::cuts_of(crate::kit::CURSOR, f)).collect()
    }
}

/// A cell's strip: its first and last cut as they are, the middle one stretched across.
fn strip(pe: &Pe, at: [i32; 2], width: i32) -> Vec<Element> {
    let Some(cuts) = (0..3).map(|c| layout::sprite(pe, STRIP, c, [0, 0])).collect::<Option<Vec<_>>>() else { return Vec::new() };
    let wide = |c: usize| i32::from(cuts[c].src[2] - cuts[c].src[0]);
    let slice = |c: usize, from: i32, to: i32| {
        let p = &cuts[c];
        let [l, t, r, b] = p.src.map(f32::from);
        Element::Slice { pack: p.texture.clone(), index: p.index, src: [l, t, r, b], rect: [at[0] + from, at[1], at[0] + to, at[1] + (b - t) as i32] }
    };
    vec![slice(0, 0, wide(0)), slice(1, wide(0), width - wide(2)), slice(2, width - wide(2), width)]
}

/// Which cell of the grid (row * 5 + column) each piece of gear starts at: packed along a row,
/// a row too full for the next piece wraps.
fn cells_of(gear: &[Gear]) -> Vec<usize> {
    let (mut row, mut col) = (0, 0);
    gear.iter()
        .map(|g| {
            if col + g.span > COLUMNS {
                row += 1;
                col = 0;
            }
            let cell = row * COLUMNS + col;
            col += g.span;
            cell
        })
        .collect()
}

/// All the page's elements for `w` with gear `chosen` lit, `tick` the clock's 60 Hz ticks.
pub fn elements(pe: &Pe, l: &Layouts, words: &crate::picker::Words, w: &Worn, chosen: usize, tick: u32) -> Vec<Element> {
    let cells = cells_of(&w.gear);
    let rows_used: Vec<usize> = cells.iter().map(|c| c / COLUMNS).collect();
    let mut out: Vec<Element> = l.desk.iter().map(|p| Element::Sprite { part: p.node.part, cut: crate::picker::DESK_CUT, at: p.at }).collect();
    // The grid's backdrop and the empty cells of the rows no gear sits in.
    for p in l.grid.iter().filter(|p| p.path.len() == 1) {
        let p = shifted(p, GRID_SHIFT);
        match p.node.part {
            BACKDROP => out.push(Element::stored(&p)),
            EMPTY_CELL if p.node.cut == EMPTY_CUT => {
                let row = l.places.iter().position(|o| (o[1] - p.at[1]).abs() < 10).unwrap_or(0) / COLUMNS;
                if !rows_used.contains(&row) {
                    out.push(Element::stored(&p));
                }
            }
            _ => {}
        }
    }
    // Each piece of gear: its strip, picture and uses (the nodes of its cell layout).
    let mut lit = None;
    for (k, (g, &cell)) in w.gear.iter().zip(&cells).enumerate() {
        let origin = l.places[cell];
        for p in &l.cells[g.span.clamp(1, COLUMNS) - 1] {
            let p = shifted(p, origin);
            match p.node.part {
                STRIP => out.extend(strip(pe, p.at, CELL_STEP * g.span as i32 - STRIP_TRIM)),
                PICTURE => out.push(Element::Scaled { part: PICTURE, cut: g.item.picture as i32, at: p.at, percent: PICTURE_PERCENT, about: p.at }),
                NUMBER => out.extend(number(pe, &p, g.uses)),
                // The lit frame sits 14 above the cell whatever its layout says (453 to 455 are 2 low).
                EMPTY_CELL if k == chosen => lit = Some(Placed { at: [p.at[0], origin[1] + FRAME_FROM_CELL_Y], ..p }),
                _ => {}
            }
        }
    }
    // The member's name tab.
    for p in &l.name {
        let p = shifted(p, NAME_TAB_AT);
        match layout::text(pe, &p) {
            Some(t) if p.node.part == NAME_TEXT => out.push(raised(t, &w.name)),
            _ => out.push(Element::stored(&p)),
        }
    }
    let mut cursor = None;
    if let Some(p) = lit {
        out.push(Element::Tinted { part: p.node.part, cut: p.node.cut, at: p.at, rgb: crate::battleequip::pulse_with(tick, PULSE_PERIOD, PULSE_WHITE_AT) });
        cursor = Some([p.at[0] + CURSOR_FROM_FRAME[0], p.at[1] + CURSOR_FROM_FRAME[1]]);
    }
    out.extend(l.figure.iter().map(Element::stored));
    out.extend(crate::picker::chrome(pe, (&l.footer, &words.footer), (&[], ""), &[], 0));
    if let Some(g) = w.gear.get(chosen) {
        out.extend(tiled(pe, CARD_PANEL, PANEL.0, PANEL.1));
        out.extend(crate::basketpage::card_with_cost(pe, &l.card, &g.item, CARD_AT, ITEM_INFO_PAGES, Some((g.span as i32, g.uses))));
    }
    out.extend(crate::picker::chrome(pe, (&[], ""), (&l.header, &words.header), &l.tab, crate::picker::Kind::AdventureEquip.tab_cut()));
    out.extend(cursor.map(|at| Element::Sprite { part: crate::kit::CURSOR, cut: l.cursor.map_or(0, |f| f.cut(tick)), at }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gear(span: usize) -> Gear {
        Gear { item: Item::default(), uses: 0, span }
    }

    #[test]
    fn gear_packs_along_the_row_and_wraps() {
        // The capture: a three-cell piece, one that does not fit after it, then a one-cell piece.
        let cells = cells_of(&[gear(3), gear(3), gear(1), gear(1), gear(1)]);
        assert_eq!(cells, vec![0, 5, 8, 9, 10]);
    }
}
