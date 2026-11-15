//! A town shop's screen: buy one of the wares on sale (`ShopData.xml`, `shop::on_sale`) at its
//! list price (`shop::list_price`), or sell one of the basket's items at its worth
//! (`Session::sell`); left and right change between the two lists.
//!
//! Buying is drawn from the game's shop screen, layout 664 (`docs/formats.md`; not checked
//! against the running game, which the copied save cannot reach a shop in): over the desk
//! (383), the title bar with the shop's name (371), the help bar (372), the money box (370,
//! part 0x4d digits), the product list (662: the panel with its "Product Name / Inventory /
//! Possession / Price" heading, 657, and a row, 661, repeated every 52 pixels: the lit bar,
//! part 0x94, on the chosen one, the name, the count held, part 0x4e, and the price, part 0x50,
//! with the coin) and the Price / Total panel (659).
//!
//! Not yet the game's: the numbers on rows not chosen are white on the parchment (the game
//! presumably tints them); a five-digit price runs into the Possession column; the Inventory column (no stock kept or refilled) and the "New" tags are
//! left out; the item card (752), the party's "E" marks (446) and the Amount dialog (660) are
//! not drawn (one item a deal); the help line's wording; selling is plain text over the desk
//! (the game's grid, 568, not drawn); the bought items at the ware's quality with none of its
//! effects or traits.
//!
//! Selling is drawn from the same layout: the bars and the money box, the grid (568: its frame,
//! 565, without the "Use" tabs, 558, or the counts under it, 559; 30 cells, 561, five a row,
//! 82 pixels apart, the rows at y 270, 336, 402, 472, 536 and 604: a frame, part 0x44, the item's
//! picture, part 0xd7, and its quality, part 0x54) with the basket's items, and the chosen one's
//! worth in Price / Total. Not yet the game's: the pictures at 64 pixels from the cell's corner
//! + (12, 6) (the layout's own are 96, which the game must scale), the other items at half
//! strength, every cell's frame lit (part 0x44 cut 0 as the layout has it), no check marks or
//! page count, more than 30 items not reached.

use std::path::Path;

use barrel_escha::data::layout::{Placed, Text};
use barrel_escha::shop::Ware;

use crate::hud::{self, Element};
use crate::screen;
use crate::title::Button;
use crate::{Result, text};

const TITLE_AT: [i32; 2] = [180, 120];
const LIST_AT: [i32; 2] = [180, 220];
const STEP: i32 = 46;
const SIZE: f32 = 36.0;
const VISIBLE: usize = 15;
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const GOLD: [u8; 4] = [0xff, 0xff, 0xd0, 0x40];
const GREY: [u8; 4] = [0xff, 0xb0, 0xb0, 0xb0];

/// What a visit does.
#[derive(Clone, Debug, PartialEq)]
pub enum Deal {
    /// Buys a ware (its index among the wares shown).
    Buy(usize),
    /// Sells a basket item (its index in the basket).
    Sell(usize),
}

/// One line of a list: its text and whether it can be taken.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub can: bool,
}

/// A ware's row in the product list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub held: i32,
    pub price: i32,
}

/// What the screen shows.
pub struct Lists {
    pub shop: String,
    pub money: i32,
    pub buy: Vec<Line>,
    pub rows: Vec<Row>,
    pub sell: Vec<Line>,
    /// The basket's items as the grid shows them, and each one's worth.
    pub items: Vec<crate::basket::Item>,
    pub worth: Vec<i32>,
}

const GRID: u32 = 568;
const GRID_FRAME: u32 = 565;
const TABS: u32 = 558;
const COUNTS: u32 = 559;
const CELL: u32 = 561;
const CELL_FRAME: i32 = 0x44;
const CELLS: usize = 30;
const PICTURE_AT: [i32; 2] = [12, 6];
const PICTURE_SIZE: f32 = 64.0;
const QUALITY_DIGITS: i32 = 0x54;
const QUALITY_END: [i32; 2] = [80, 44];

/// The selling view from layout 664's placed nodes.
pub fn sell_elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, l: &Lists, chosen: usize) -> Vec<Element> {
    let mut out = Vec::new();
    let worth = l.worth.get(chosen).copied().unwrap_or(0);
    let mut cells: Vec<[i32; 2]> = Vec::new();
    for p in nodes {
        let n = p.node;
        let Some(&branch) = p.path.get(1) else { continue };
        match branch {
            TITLE | HELP if n.part == 0x164 => {
                if let Some(t) = texts(p) {
                    let text = if branch == TITLE { l.shop.clone() } else { "Up / Down / Left / Right: choose   Enter: sell   Esc: leave".into() };
                    out.push(Element::Label(Text { text, ..t }));
                }
            }
            MONEY if n.part == MONEY_DIGITS && n.kind == 28 => out.extend(digits(l.money, MONEY_DIGITS, p.at[0], p.at[1])),
            TOTALS if n.part == MONEY_DIGITS && n.kind == 28 => out.extend(digits(worth, MONEY_DIGITS, p.at[0], p.at[1])),
            GRID if p.path.contains(&CELL) => {
                if n.part == CELL_FRAME && p.path.last() == Some(&CELL) {
                    cells.push(p.at);
                }
            }
            GRID if p.path.contains(&GRID_FRAME) && !p.path.contains(&TABS) && !p.path.contains(&COUNTS) && n.part != 0x164 => out.push(Element::stored(p)),
            TITLE | HELP | MONEY | TOTALS if n.part != 0x164 => out.push(Element::stored(p)),
            _ => {}
        }
    }
    let first = chosen / CELLS * CELLS;
    for (slot, at) in cells.iter().enumerate() {
        out.push(Element::Sprite { part: CELL_FRAME, cut: 0, at: *at });
        let k = first + slot;
        let Some(it) = l.items.get(k) else { continue };
        out.push(Element::Picture { pack: crate::basket::PICTURES.into(), index: it.picture, size: [PICTURE_SIZE; 2], at: [at[0] + PICTURE_AT[0], at[1] + PICTURE_AT[1]], dim: k != chosen });
        out.extend(digits(it.quality, QUALITY_DIGITS, at[0] + QUALITY_END[0], at[1] + QUALITY_END[1]));
    }
    out
}

pub const LAYOUT: u32 = 664;
const TITLE: u32 = 371;
const HELP: u32 = 372;
const MONEY: u32 = 370;
const LIST: u32 = 662;
const ROW: u32 = 661;
const TOTALS: u32 = 659;
/// The rows' spacing and how many the panel holds.
const ROW_STEP: i32 = 52;
const ROWS: usize = 11;
const LIT_BAR: i32 = 0x94;
const HELD_DIGITS: i32 = 0x4e;
const PRICE_DIGITS: i32 = 0x50;
const MONEY_DIGITS: i32 = 0x4d;
const DIGIT_ADVANCE: i32 = 24;
/// The Inventory column's number (x 418) and the "New" tag (part 0x95 cut 4), left out.
const INVENTORY_X: i32 = 418;

/// A number's digits ending at `end` (as the game's kind 28 numbers).
fn digits(value: i32, part: i32, end: i32, y: i32) -> Vec<Element> {
    let d: Vec<i32> = value.max(0).to_string().bytes().map(|b| i32::from(b - b'0')).collect();
    let n = d.len() as i32;
    d.into_iter().enumerate().map(|(i, c)| Element::Sprite { part, cut: c, at: [end - DIGIT_ADVANCE * (n - i as i32), y] }).collect()
}

/// The buying view from layout 664's placed nodes (`texts` gives a node's text).
pub fn buy_elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, l: &Lists, chosen: usize, first: usize) -> Vec<Element> {
    let mut out = Vec::new();
    let price = l.rows.get(chosen).map_or(0, |r| r.price);
    for p in nodes {
        let n = p.node;
        let Some(&branch) = p.path.get(1) else { continue };
        match branch {
            TITLE | HELP if n.part == 0x164 => {
                if let Some(t) = texts(p) {
                    let text = if branch == TITLE { l.shop.clone() } else { "Up / Down: choose   Enter: buy   Left / Right: sell   Esc: leave".into() };
                    out.push(Element::Label(Text { text, ..t }));
                }
            }
            MONEY if n.part == MONEY_DIGITS && n.kind == 28 => out.extend(digits(l.money, MONEY_DIGITS, p.at[0], p.at[1])),
            TOTALS if n.part == MONEY_DIGITS && n.kind == 28 => {
                out.extend(digits(price, MONEY_DIGITS, p.at[0], p.at[1]));
            }
            LIST if p.path.contains(&ROW) => {
                for (slot, k) in (first..l.rows.len()).take(ROWS).enumerate() {
                    let r = &l.rows[k];
                    let dy = ROW_STEP * slot as i32;
                    let at = [p.at[0], p.at[1] + dy];
                    match n.part {
                        LIT_BAR if k != chosen => {}
                        // The row's dark backing goes with the lit bar (the chosen row only).
                        0x95 if n.cut == 2 && k != chosen => {}
                        0x95 if n.cut == 4 => {}
                        0x164 => {
                            if let Some(t) = texts(p) {
                                let can = l.buy.get(k).is_some_and(|b| b.can);
                                let colour = match (k == chosen, can) {
                                    (true, true) => [0xff; 4],
                                    (true, false) => [0xff, 0xb0, 0xb0, 0xb0],
                                    (false, true) => t.colour,
                                    (false, false) => [0xff, 0x80, 0x80, 0x80],
                                };
                                out.push(Element::Label(Text { text: r.name.clone(), at, colour, ..t }));
                            }
                        }
                        HELD_DIGITS if n.kind == 28 && p.at[0] == INVENTORY_X => {}
                        HELD_DIGITS if n.kind == 28 => out.extend(digits(r.held, HELD_DIGITS, at[0], at[1])),
                        PRICE_DIGITS if n.kind == 28 => out.extend(digits(r.price, PRICE_DIGITS, at[0], at[1])),
                        _ => out.push(Element::Sprite { part: n.part, cut: n.cut.max(0), at }),
                    }
                }
            }
            TITLE | HELP | MONEY | TOTALS | LIST if n.part != 0x164 => out.push(Element::stored(p)),
            _ => {}
        }
    }
    out
}

/// Where the screen is: the list (false buying, true selling), the chosen line and the first
/// shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Shop {
    pub selling: bool,
    pub chosen: usize,
    pub first: usize,
}

impl Shop {
    /// A button: up and down move, left and right change lists, confirm takes the chosen line
    /// when it can be taken (the deal returned), back closes (`None` in the outer option).
    pub fn press(&mut self, b: Button, l: &Lists) -> Option<Option<Deal>> {
        let list = if self.selling { &l.sell } else { &l.buy };
        let n = list.len();
        match b {
            // Selling: the grid's five columns (left and right move along a row there; Tab-free:
            // the lists change with left on the first column and right on the last).
            Button::Left | Button::Right if self.selling && n > 0 && !(b == Button::Left && self.chosen % 5 == 0) && !(b == Button::Right && (self.chosen % 5 == 4 || self.chosen + 1 >= n)) => {
                self.chosen = if b == Button::Left { self.chosen - 1 } else { self.chosen + 1 };
                None
            }
            Button::Up | Button::Down if self.selling && n > 0 => {
                self.chosen = if b == Button::Up { self.chosen.saturating_sub(5) } else { (self.chosen + 5).min(n - 1) };
                None
            }
            Button::Up | Button::Down if n > 0 => {
                self.chosen = if b == Button::Up { (self.chosen + n - 1) % n } else { (self.chosen + 1) % n };
                let visible = if self.selling { VISIBLE } else { ROWS };
                self.first = if self.chosen < self.first { self.chosen } else if self.chosen >= self.first + visible { self.chosen + 1 - visible } else { self.first };
                None
            }
            Button::Left | Button::Right => {
                *self = Shop { selling: !self.selling, chosen: 0, first: 0 };
                None
            }
            Button::Confirm if list.get(self.chosen).is_some_and(|line| line.can) => Some(Some(if self.selling { Deal::Sell(self.chosen) } else { Deal::Buy(self.chosen) })),
            Button::Back => Some(None),
            _ => None,
        }
    }

    /// The view: buying from the game's layout (its nodes given), selling as text.
    pub fn view(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, l: &Lists) -> Vec<Element> {
        if self.selling { sell_elements(nodes, texts, l, self.chosen) } else { buy_elements(nodes, texts, l, self.chosen, self.first) }
    }

    pub fn elements(&self, l: &Lists) -> Vec<Element> {
        let label = |text: String, at: [i32; 2], size: f32, colour: [u8; 4]| Element::Label(Text { at, size, colour, text, kind: 21 });
        let mut out = vec![
            label(format!("{}: {}   (left / right)", l.shop, if self.selling { "Sell" } else { "Buy" }), TITLE_AT, SIZE + 6.0, WHITE),
            label(format!("{} cole", l.money), [TITLE_AT[0], TITLE_AT[1] + 50], SIZE, GOLD),
        ];
        let list = if self.selling { &l.sell } else { &l.buy };
        if list.is_empty() {
            out.push(label(if self.selling { "The basket is empty." } else { "Nothing is on sale." }.into(), LIST_AT, SIZE, GREY));
        }
        for (k, line) in list.iter().enumerate().skip(self.first).take(VISIBLE) {
            let colour = if !line.can { GREY } else if k == self.chosen { GOLD } else { WHITE };
            out.push(label(line.text.clone(), [LIST_AT[0], LIST_AT[1] + STEP * (k - self.first) as i32], SIZE, colour));
        }
        out
    }
}

/// The lines of a shop's lists: its wares on sale (name, price, whether the money covers it,
/// how many the basket holds: `held(ware)`) and the basket's items (name, quality, worth).
pub fn lists(shop: &str, wares: &[&Ware], money: i32, basket: &[(String, f32, i32)], held: impl Fn(&Ware) -> i32, items: Vec<crate::basket::Item>) -> Lists {
    let buy = wares.iter().map(|w| {
        let price = barrel_escha::shop::list_price(w);
        Line { text: format!("{}   {} cole", w.item_name, price), can: price <= money }
    });
    let rows = wares.iter().map(|w| Row { name: w.item_name.clone(), held: held(w), price: barrel_escha::shop::list_price(w) });
    let sell = basket.iter().map(|(name, quality, worth)| Line { text: format!("{name}   quality {quality:.0}   {worth} cole"), can: true });
    let worth = basket.iter().map(|b| b.2).collect();
    Lists { shop: shop.to_string(), money, buy: buy.collect(), rows: rows.collect(), sell: sell.collect(), items, worth }
}

/// Runs the screen over the desk; the deal made, if any.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &Path,
    l: &Lists,
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<Deal>> {
    let exe = barrel_files::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("shop: no glyph table")?;
    let atlas_bytes = barrel_files::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("shop: an empty font atlas")?.width as f32 };
    let desk: Vec<Element> = barrel_escha::data::layout::placed(&pe, crate::menu::BACKGROUND).iter().map(Element::stored).collect();
    let nodes = barrel_escha::data::layout::placed(&pe, LAYOUT);
    let texts = |p: &Placed| barrel_escha::data::layout::text(&pe, p);
    let mut every = desk.clone();
    every.extend(buy_elements(&nodes, texts, l, 0, 0));
    every.extend(sell_elements(&nodes, texts, l, 0));
    every.extend((0..10).map(|cut| Element::Sprite { part: QUALITY_DIGITS, cut, at: [0, 0] }));
    every.extend([HELD_DIGITS, PRICE_DIGITS, MONEY_DIGITS].iter().flat_map(|&part| (0..10).map(move |cut| Element::Sprite { part, cut, at: [0, 0] })));
    every.extend(nodes.iter().filter(|p| p.node.part == LIT_BAR).map(Element::stored));
    let keys_needed = screen::sheet_keys(&pe, every);
    let packs = screen::read_packs(root, &keys_needed)?;
    let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), barrel_formats::g1t::G1t::parse(b)?))).collect::<Result<Vec<_>>>()?;
    let mut images = vec![&atlas.textures[0]];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
    for (name, index) in keys_needed {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |s: &Shop| {
        let mut e = desk.clone();
        e.extend(s.view(&nodes, texts, l));
        hud::quads(&pe, Some(&font), &sheets, &e, w, h)
    };
    let press = |s: &mut Shop, b: Button| s.press(b, l);
    let mut shop = Shop { selling: false, chosen: 0, first: 0 };
    Ok(screen::run(gpu, pool, target, window, offscreen, &images, &mut shop, quads, press, keys, out)?.flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buying_needs_the_money_and_selling_is_a_side_step_away() {
        let l = Lists { shop: "Shop".into(), money: 50, rows: Vec::new(), items: Vec::new(), worth: Vec::new(), buy: vec![Line { text: "a".into(), can: false }, Line { text: "b".into(), can: true }], sell: vec![Line { text: "c".into(), can: true }] };
        let mut s = Shop { selling: false, chosen: 0, first: 0 };
        assert_eq!(s.press(Button::Confirm, &l), None, "too dear: nothing");
        s.press(Button::Down, &l);
        assert_eq!(s.press(Button::Confirm, &l), Some(Some(Deal::Buy(1))));
        s.press(Button::Right, &l);
        assert_eq!(s.press(Button::Confirm, &l), Some(Some(Deal::Sell(0))));
        assert_eq!(s.press(Button::Back, &l), Some(None));
    }
}
