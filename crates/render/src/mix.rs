//! The synthesis screen (the Atelier's cauldron): choose a recipe among those the basket can
//! make, see the ingredients it takes from the basket and the quality it would come out at, and
//! make it (`Session::synthesize`).
//!
//! Not yet the game's: the screen is plain text over the main menu's desk (layout 383); the
//! game's synthesis screens (layouts 720 to 851 on `a15_mix.g1t`: the recipe book, the
//! ingredient grid, the cauldron with its effect lines, the result) are not drawn yet; the
//! ingredients are the first that fit (no choosing among them); no skills, traits or count.

use std::path::Path;

use barrel_escha::data::layout::Text;
use barrel_escha::cauldron::Book;
use barrel_escha::gather::Got;

use crate::hud::{self, Element};
use crate::screen;
use crate::title::Button;
use crate::{Result, text};

const TITLE_AT: [i32; 2] = [180, 120];
const LIST_AT: [i32; 2] = [180, 200];
const STEP: i32 = 46;
const SIZE: f32 = 36.0;
/// Recipe lines shown at once.
const VISIBLE: usize = 16;
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const GOLD: [u8; 4] = [0xff, 0xff, 0xd0, 0x40];
const GREY: [u8; 4] = [0xff, 0xb0, 0xb0, 0xb0];

/// Where the screen is.
#[derive(Clone, Debug, PartialEq)]
pub enum Mix {
    /// The recipe list (the chosen line and the first shown).
    Recipes { chosen: usize, first: usize },
    /// A recipe's ingredients and the item it would make, waiting for confirm.
    Ingredients { chosen: usize, first: usize },
}

/// What the screen shows: the recipes the basket can make (recipe numbers) and, for each, its
/// lines (name and days; the ingredients picked with their qualities; the quality it would make).
pub struct Choices {
    pub recipes: Vec<usize>,
    pub names: Vec<String>,
    pub ingredients: Vec<Vec<String>>,
    pub picks: Vec<Vec<usize>>,
    pub outcome: Vec<String>,
}

impl Choices {
    pub fn new(book: &Book, basket: &[Got]) -> Choices {
        let recipes = book.makeable(basket);
        let mut c = Choices { recipes: recipes.clone(), names: Vec::new(), ingredients: Vec::new(), picks: Vec::new(), outcome: Vec::new() };
        for &r in &recipes {
            let recipe = &book.recipes[r];
            c.names.push(format!("{}   {} days", book.name(i32::from(recipe.item)), recipe.days));
            let picks = book.fill(r, basket).unwrap_or_default();
            c.ingredients.push(picks.iter().map(|&k| format!("{}   quality {:.0}", book.name(basket[k].item), basket[k].quality)).collect());
            c.outcome.push(match book.make(r, &picks, basket, &mut |lo, _| lo) {
                Ok(m) => format!("Makes {} at quality {:.0}", book.name(m.item), m.quality),
                Err(e) => format!("Cannot be made: {e}"),
            });
            c.picks.push(picks);
        }
        c
    }
}

impl Mix {
    /// A button: the list moves, confirm goes on to the ingredients and then makes it (the
    /// recipe and its picks returned), back goes back (from the list: the screen closes, `None`
    /// in the outer option).
    pub fn press(&mut self, b: Button, c: &Choices) -> Option<Option<(usize, Vec<usize>)>> {
        let n = c.recipes.len();
        match self.clone() {
            Mix::Recipes { chosen, first } => match b {
                Button::Up | Button::Down if n > 0 => {
                    let chosen = if b == Button::Up { (chosen + n - 1) % n } else { (chosen + 1) % n };
                    let first = if chosen < first { chosen } else if chosen >= first + VISIBLE { chosen + 1 - VISIBLE } else { first };
                    *self = Mix::Recipes { chosen, first };
                    None
                }
                Button::Confirm if n > 0 => {
                    *self = Mix::Ingredients { chosen, first };
                    None
                }
                Button::Back => Some(None),
                _ => None,
            },
            Mix::Ingredients { chosen, first } => match b {
                Button::Confirm => Some(Some((c.recipes[chosen], c.picks[chosen].clone()))),
                Button::Back => {
                    *self = Mix::Recipes { chosen, first };
                    None
                }
                _ => None,
            },
        }
    }

    pub fn elements(&self, c: &Choices) -> Vec<Element> {
        let line = |text: &str, k: i32, colour: [u8; 4]| Element::Label(Text { at: [LIST_AT[0], LIST_AT[1] + STEP * k], size: SIZE, colour, text: text.to_string(), kind: 21 });
        let title = |text: &str| Element::Label(Text { at: TITLE_AT, size: SIZE + 6.0, colour: WHITE, text: text.to_string(), kind: 21 });
        let mut out = Vec::new();
        match *self {
            Mix::Recipes { chosen, first } => {
                out.push(title("Synthesis: choose a recipe"));
                if c.recipes.is_empty() {
                    out.push(line("The basket holds what no recipe takes.", 0, GREY));
                }
                for (k, name) in c.names.iter().enumerate().skip(first).take(VISIBLE) {
                    out.push(line(name, (k - first) as i32, if k == chosen { GOLD } else { WHITE }));
                }
            }
            Mix::Ingredients { chosen, .. } => {
                out.push(title(&c.names[chosen]));
                for (k, i) in c.ingredients[chosen].iter().enumerate() {
                    out.push(line(i, k as i32, WHITE));
                }
                let k = c.ingredients[chosen].len() as i32 + 1;
                out.push(line(&c.outcome[chosen], k, GOLD));
                out.push(line("Confirm to make it, back for the list.", k + 1, GREY));
            }
        }
        out
    }
}

/// Runs the screen over the desk; the recipe and picks to make, if one was made.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &Path,
    book: &Book,
    basket: &[Got],
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<(usize, Vec<usize>)>> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("synthesis: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("synthesis: an empty font atlas")?.width as f32 };
    let desk: Vec<Element> = barrel_escha::data::layout::placed(&pe, crate::menu::BACKGROUND).iter().map(Element::stored).collect();
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
    let choices = Choices::new(book, basket);
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |m: &Mix| {
        let mut e = desk.clone();
        e.extend(m.elements(&choices));
        hud::quads(&pe, Some(&font), &sheets, &e, w, h)
    };
    let press = |m: &mut Mix, b: Button| m.press(b, &choices);
    let mut mix = Mix::Recipes { chosen: 0, first: 0 };
    Ok(screen::run(gpu, pool, target, window, offscreen, &images, &mut mix, quads, press, keys, out)?.flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_twice_makes_and_back_returns() {
        let c = Choices { recipes: vec![7, 9], names: vec!["a".into(), "b".into()], ingredients: vec![vec![], vec![]], picks: vec![vec![0], vec![1, 2]], outcome: vec![String::new(); 2] };
        let mut m = Mix::Recipes { chosen: 0, first: 0 };
        assert_eq!(m.press(Button::Down, &c), None);
        assert_eq!(m.press(Button::Confirm, &c), None);
        assert_eq!(m.press(Button::Back, &c), None);
        assert_eq!(m, Mix::Recipes { chosen: 1, first: 0 });
        m.press(Button::Confirm, &c);
        assert_eq!(m.press(Button::Confirm, &c), Some(Some((9, vec![1, 2]))));
        assert_eq!(Mix::Recipes { chosen: 0, first: 0 }.press(Button::Back, &c), Some(None));
    }
}
