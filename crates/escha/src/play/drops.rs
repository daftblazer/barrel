//! A won battle's drops, shown after its Result screen (the game: `docs/formats.md`, "The
//! battle result"): over the desk, the items in an item grid, the chosen one's count in the
//! basket in the "Possessed" box (layout 706: the basket's and the container's icons with a
//! number each), and "OK"; confirm (or back) closes it.
//!
//! Not yet the game's: the grid is the Basket page's (`basket.rs`, layout 456, its tab titled
//! "Drops") where the game has a narrower one with a scroll bar and a check mark on each item;
//! no item card (the game shows the chosen item's, as the shop's); the container's count is 0
//! (Barrel keeps no container); "OK" is a plain line under the grid; the box's parchment (part
//! 0x61, a 96-pixel piece the game stretches) is not stretched to the box and its numbers are
//! white, not dark. The game's own screen is put
//! together at run time (no layout holds 706).

use std::path::Path;

use barrel_escha::data::layout::{Placed, Text};

use crate::basket::{Basket, Item};
use crate::hud::{self, Element};
use crate::screen;
use crate::title::Button;
use crate::{Result, text};

const POSSESSED: u32 = 706;
/// Where the box sits on the 1920 × 1080 screen (the recording: x 801, y 97 of 1280 × 720).
const POSSESSED_AT: [i32; 2] = [1202, 146];
const COUNT_DIGITS: (i32, i32) = (0x4c, 24);
const OK_AT: [i32; 2] = [470, 900];

/// The screen's elements past the desk: the grid with the drops, the box with the chosen one's
/// counts (`possessed[k]`: how many of drop `k`'s item the basket holds now) and "OK".
pub fn elements(grid: &[Placed], boxed: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, b: &Basket, items: &[Item], possessed: &[i32]) -> Vec<Element> {
    let mut out: Vec<Element> = b.elements(grid, &texts, items).into_iter().map(|e| match e {
        Element::Label(t) if t.text == "Basket" => Element::Label(Text { text: "Drops".into(), ..t }),
        e => e,
    }).collect();
    // The chosen item's line under the grid gives way to "OK".
    if let Some(Element::Label(t)) = out.last().cloned() {
        out.pop();
        out.push(Element::Label(Text { text: "OK".into(), at: OK_AT, kind: 22, ..t }));
    }
    let counts = [possessed.get(b.chosen).copied().unwrap_or(0), 0];
    let mut number = 0;
    for p in boxed {
        let at = [p.at[0] + POSSESSED_AT[0], p.at[1] + POSSESSED_AT[1]];
        match p.node.part {
            0x164 => {
                if let Some(t) = texts(p).filter(|t| !t.text.is_empty() && !t.filled_in()) {
                    out.push(Element::Label(Text { at, ..t }));
                }
            }
            part if part == COUNT_DIGITS.0 && p.node.kind == 28 => {
                let digits = counts.get(number).copied().unwrap_or(0).max(0).to_string();
                let n = digits.len() as i32;
                for (i, d) in digits.bytes().enumerate() {
                    out.push(Element::Sprite { part, cut: i32::from(d - b'0'), at: [at[0] - COUNT_DIGITS.1 * (n - 1 - i as i32), at[1]] });
                }
                number += 1;
            }
            0x4d => {}
            _ => out.push(match Element::stored(p) {
                Element::Mirrored { part, cut, axis, .. } => Element::Mirrored { part, cut, at, axis },
                e => e.sprite().map_or(e, |(part, cut, _)| Element::Sprite { part, cut, at }),
            }),
        }
    }
    out
}

/// Runs the screen over the desk until it is closed.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &Path,
    drops: &[(i32, f32)],
    possessed: &[i32],
    keys: &[Button],
    out: Option<&Path>,
) -> Result<()> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("drops: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("drops: an empty font atlas")?.width as f32 };
    let item_rows = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/item/itemData.xml"))?)?;
    let items = crate::basket::items(drops, &item_rows.children);
    let desk: Vec<Element> = barrel_escha::data::layout::placed(&pe, crate::menu::BACKGROUND).iter().map(Element::stored).collect();
    let grid = barrel_escha::data::layout::placed(&pe, crate::basket::GRID);
    let boxed = barrel_escha::data::layout::placed(&pe, POSSESSED);
    let texts = |p: &Placed| barrel_escha::data::layout::text(&pe, p);
    let all = |b: &Basket| {
        let mut e = desk.clone();
        e.extend(elements(&grid, &boxed, texts, b, &items, possessed));
        e
    };
    let mut every = Vec::new();
    for chosen in 0..items.len().max(1) {
        every.extend(all(&Basket { chosen, first: chosen / 25 * 5 }));
    }
    every.extend((0..10).map(|cut| Element::Sprite { part: COUNT_DIGITS.0, cut, at: [0, 0] }));
    every.extend((0..10).map(|cut| Element::Sprite { part: 0x54, cut, at: [0, 0] }));
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
    let quads = |b: &Basket| hud::quads(&pe, Some(&font), &sheets, &all(b), w, h);
    let count = items.len();
    let press = |b: &mut Basket, k: Button| match k {
        Button::Confirm | Button::Back => Some(()),
        k => {
            b.press(k, count);
            None
        }
    };
    let mut b = Basket::default();
    screen::run(gpu, pool, target, window, offscreen, &images, &mut b, quads, press, keys, out)?;
    Ok(())
}
