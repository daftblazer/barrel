//! The Equip page: choose a party member (left / right), one of their four slots (weapon,
//! armour, two accessories) and then a basket item that fits it (`ItemRow::fits`); confirm puts
//! it on (`Session::equip`, what the slot held back into the basket).
//!
//! Not yet the game's: plain text over the main menu's desk (layout 383); the game's equipment
//! screen is not drawn yet, and no stat changes are shown before choosing; one change a visit.

use std::path::Path;

use barrel_formats::layout::Text;

use crate::hud::{self, Element};
use crate::screen;
use crate::title::Button;
use crate::{Result, text};

const TITLE_AT: [i32; 2] = [180, 120];
const LIST_AT: [i32; 2] = [180, 220];
const STEP: i32 = 46;
const SIZE: f32 = 36.0;
const VISIBLE: usize = 14;
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const GOLD: [u8; 4] = [0xff, 0xff, 0xd0, 0x40];
const GREY: [u8; 4] = [0xff, 0xb0, 0xb0, 0xb0];
const SLOTS: [&str; 4] = ["Weapon", "Armor", "Accessory", "Accessory"];

/// One member as the page shows them: their name, what each slot holds, and for each slot the
/// basket items that fit it (basket index, line).
pub struct Member {
    pub name: String,
    pub worn: [String; 4],
    pub fitting: [Vec<(usize, String)>; 4],
}

/// Where the page is.
#[derive(Clone, Debug, PartialEq)]
pub enum Equip {
    Slots { member: usize, slot: usize },
    Items { member: usize, slot: usize, chosen: usize, first: usize },
}

impl Equip {
    /// A button; confirm on an item returns (member index, slot, basket index), back from the
    /// slots closes (`None` in the outer option).
    pub fn press(&mut self, b: Button, m: &[Member]) -> Option<Option<(usize, usize, usize)>> {
        match self.clone() {
            Equip::Slots { member, slot } => match b {
                Button::Left | Button::Right if !m.is_empty() => {
                    let n = m.len();
                    *self = Equip::Slots { member: if b == Button::Left { (member + n - 1) % n } else { (member + 1) % n }, slot };
                    None
                }
                Button::Up | Button::Down => {
                    *self = Equip::Slots { member, slot: if b == Button::Up { (slot + 3) % 4 } else { (slot + 1) % 4 } };
                    None
                }
                Button::Confirm if m.get(member).is_some_and(|x| !x.fitting[slot].is_empty()) => {
                    *self = Equip::Items { member, slot, chosen: 0, first: 0 };
                    None
                }
                Button::Back => Some(None),
                _ => None,
            },
            Equip::Items { member, slot, chosen, first } => {
                let n = m[member].fitting[slot].len();
                match b {
                    Button::Up | Button::Down => {
                        let chosen = if b == Button::Up { (chosen + n - 1) % n } else { (chosen + 1) % n };
                        let first = if chosen < first { chosen } else if chosen >= first + VISIBLE { chosen + 1 - VISIBLE } else { first };
                        *self = Equip::Items { member, slot, chosen, first };
                        None
                    }
                    Button::Confirm => Some(Some((member, slot, m[member].fitting[slot][chosen].0))),
                    Button::Back => {
                        *self = Equip::Slots { member, slot };
                        None
                    }
                    _ => None,
                }
            }
        }
    }

    pub fn elements(&self, m: &[Member]) -> Vec<Element> {
        let label = |text: String, at: [i32; 2], colour: [u8; 4]| Element::Label(Text { at, size: SIZE, colour, text, kind: 21 });
        let line = |k: usize| [LIST_AT[0], LIST_AT[1] + STEP * k as i32];
        let mut out = Vec::new();
        let Some(member) = m.get(match *self { Equip::Slots { member, .. } | Equip::Items { member, .. } => member }) else {
            return vec![label("Nobody is in the party.".into(), TITLE_AT, GREY)];
        };
        out.push(label(format!("Equip: {}   (left / right)", member.name), TITLE_AT, WHITE));
        match *self {
            Equip::Slots { slot, .. } => {
                for (k, (name, worn)) in SLOTS.iter().zip(&member.worn).enumerate() {
                    let colour = if k == slot { GOLD } else if member.fitting[k].is_empty() { GREY } else { WHITE };
                    out.push(label(format!("{name}:   {worn}"), line(k), colour));
                }
            }
            Equip::Items { slot, chosen, first, .. } => {
                out.push(label(format!("{} now: {}", SLOTS[slot], member.worn[slot]), [TITLE_AT[0], TITLE_AT[1] + 50], GREY));
                for (k, (_, text)) in member.fitting[slot].iter().enumerate().skip(first).take(VISIBLE) {
                    out.push(label(text.clone(), line(k - first), if k == chosen { GOLD } else { WHITE }));
                }
            }
        }
        out
    }
}

/// Runs the page over the desk; the change chosen, if any.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &Path,
    members: &[Member],
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<(usize, usize, usize)>> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_formats::font::glyphs(&pe, barrel_formats::font::FONT_EN).ok_or("equip: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("equip: an empty font atlas")?.width as f32 };
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
    let quads = |e: &Equip| {
        let mut all = desk.clone();
        all.extend(e.elements(members));
        hud::quads(&pe, Some(&font), &sheets, &all, w, h)
    };
    let press = |e: &mut Equip, b: Button| e.press(b, members);
    let mut page = Equip::Slots { member: 0, slot: 0 };
    Ok(screen::run(gpu, pool, target, window, offscreen, &images, &mut page, quads, press, keys, out)?.flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_then_an_item_that_fits() {
        let none = || [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut fitting = none();
        fitting[0] = vec![(4, "Staff".into()), (7, "Rod".into())];
        let m = [Member { name: "Escha".into(), worn: Default::default(), fitting }, Member { name: "Logy".into(), worn: Default::default(), fitting: none() }];
        let mut e = Equip::Slots { member: 0, slot: 0 };
        e.press(Button::Right, &m);
        assert_eq!(e.press(Button::Confirm, &m), None, "Logy has nothing to put on");
        assert_eq!(e, Equip::Slots { member: 1, slot: 0 });
        e.press(Button::Left, &m);
        e.press(Button::Confirm, &m);
        e.press(Button::Down, &m);
        assert_eq!(e.press(Button::Confirm, &m), Some(Some((0, 0, 7))));
    }
}
