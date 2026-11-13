//! A town shop's screen: buy one of the wares on sale (`ShopData.xml`, `shop::on_sale`) at its
//! list price (`shop::list_price`), or sell one of the basket's items at its worth
//! (`Session::sell`); left and right change between the two lists.
//!
//! Not yet the game's: plain text over the main menu's desk (layout 383); the game's shop screen
//! (likely layout 664 with the amount dialog 660, `docs/formats.md`, not checked against the
//! running game) is not drawn yet; one item a visit, no amounts, no stock kept or refilled, the
//! bought items at the ware's quality with none of its effects or traits.

use std::path::Path;

use barrel_formats::layout::Text;
use barrel_game::shop::Ware;

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

/// What the screen shows.
pub struct Lists {
    pub shop: String,
    pub money: i32,
    pub buy: Vec<Line>,
    pub sell: Vec<Line>,
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
            Button::Up | Button::Down if n > 0 => {
                self.chosen = if b == Button::Up { (self.chosen + n - 1) % n } else { (self.chosen + 1) % n };
                self.first = if self.chosen < self.first { self.chosen } else if self.chosen >= self.first + VISIBLE { self.chosen + 1 - VISIBLE } else { self.first };
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

/// The lines of a shop's lists: its wares on sale (name, price, whether the money covers it) and
/// the basket's items (name, quality, worth).
pub fn lists(shop: &str, wares: &[&Ware], money: i32, basket: &[(String, f32, i32)]) -> Lists {
    let buy = wares.iter().map(|w| {
        let price = barrel_game::shop::list_price(w);
        Line { text: format!("{}   {} cole", w.item_name, price), can: price <= money }
    });
    let sell = basket.iter().map(|(name, quality, worth)| Line { text: format!("{name}   quality {quality:.0}   {worth} cole"), can: true });
    Lists { shop: shop.to_string(), money, buy: buy.collect(), sell: sell.collect() }
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
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_formats::font::glyphs(&pe, barrel_formats::font::FONT_EN).ok_or("shop: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("shop: an empty font atlas")?.width as f32 };
    let desk: Vec<Element> = barrel_formats::layout::placed(&pe, crate::menu::BACKGROUND).iter().map(Element::stored).collect();
    let keys_needed = screen::sheet_keys(&pe, desk.clone());
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
        e.extend(s.elements(l));
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
        let l = Lists { shop: "Shop".into(), money: 50, buy: vec![Line { text: "a".into(), can: false }, Line { text: "b".into(), can: true }], sell: vec![Line { text: "c".into(), can: true }] };
        let mut s = Shop { selling: false, chosen: 0, first: 0 };
        assert_eq!(s.press(Button::Confirm, &l), None, "too dear: nothing");
        s.press(Button::Down, &l);
        assert_eq!(s.press(Button::Confirm, &l), Some(Some(Deal::Buy(1))));
        s.press(Button::Right, &l);
        assert_eq!(s.press(Button::Confirm, &l), Some(Some(Deal::Sell(0))));
        assert_eq!(s.press(Button::Back, &l), Some(None));
    }
}
