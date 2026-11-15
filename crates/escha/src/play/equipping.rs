//! The Equip page: choose a party member (left / right), one of their four slots (weapon,
//! armour, two accessories) and then a basket item that fits it (`ItemRow::fits`); confirm puts
//! it on (`Session::equip`, what the slot held back into the basket).
//!
//! Drawn from the game's equipment screen, layout 404 (its pieces fit throughout; not seen in the
//! running game yet): over the desk (383), the member's Status notebook (399, as the Status
//! page draws it, `status.rs`), the slot panel (403: four rows, layout 390 each, an item frame,
//! part 0xb cut 4, a name bar, part 0x3d, and the item's name, 56 pixels or so apart from y
//! 206) and, while an item is being chosen, the new values beside the notebook (406 / 405: for
//! HP, MP, ATK, DEF, SPD and the four RES, an arrow, part 0x68, cut 0 up in red, cut 2 down in
//! blue, and the value, part 0x4e digits ending at x 1678).
//!
//! Not yet the game's: the slot names white on their yellow bars (hard to read; the game likely
//! tints them); the items' pictures in the slot frames; the fitting items as text lines
//! under the slot panel (the game's grid, 568, and the item card, 752, not drawn); the help line;
//! one change a visit.

use std::path::Path;

use barrel_escha::data::layout::{Placed, Text};

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
    /// The Status page's values as worn, and with each fitting item put on instead.
    pub status: Option<crate::status::Values>,
    pub previews: [Vec<Option<crate::status::Values>>; 4],
}

pub const LAYOUT: u32 = 404;
const SLOT_PANEL: u32 = 403;
const SLOT_ROW: u32 = 390;
const NAME_BAR: i32 = 0x3d;
const PREVIEW: u32 = 406;
const ARROW: i32 = 0x68;
const UP: i32 = 0;
const DOWN: i32 = 2;
const DIGITS: i32 = 0x4e;
const DIGIT_ADVANCE: i32 = 20;
const ITEMS_AT: [i32; 2] = [120, 520];

/// The nine values the preview column shows, in its order.
fn shown(v: &crate::status::Values) -> [i32; 9] {
    let [r0, r1, r2, r3] = v.resist;
    [v.hp.1, v.mp.1, v.attack, v.defense, v.speed, r0, r1, r2, r3]
}

/// The page from layout 404's placed nodes (`texts` gives a node's text).
pub fn view(e: &Equip, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, m: &[Member]) -> Vec<Element> {
    let (member, slot, item) = match *e {
        Equip::Slots { member, slot } => (member, slot, None),
        Equip::Items { member, slot, chosen, first } => (member, slot, Some((chosen, first))),
    };
    let Some(x) = m.get(member) else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(v) = &x.status {
        out.extend(crate::status::elements(nodes, &texts, v));
    }
    // The slot panel: its rows in order are the four slots.
    let mut row = 0usize;
    for p in nodes.iter().filter(|p| p.path.get(1) == Some(&SLOT_PANEL)) {
        let n = p.node;
        if p.path.last() == Some(&SLOT_ROW) {
            match n.part {
                0x164 => {
                    if let Some(t) = texts(p) {
                        let colour = if row == slot { [0xff, 0xff, 0xd0, 0x40] } else if x.fitting[row].is_empty() { [0xff, 0xb0, 0xb0, 0xb0] } else { [0xff; 4] };
                        out.push(Element::Label(Text { text: x.worn[row].clone(), colour, ..t }));
                    }
                    row = (row + 1).min(3);
                }
                _ => out.push(Element::stored(p)),
            }
        } else if n.part != 0x164 {
            out.push(Element::stored(p));
        }
    }
    let label = |text: String, at: [i32; 2], colour: [u8; 4]| Element::Label(Text { at, size: 30.0, colour, text, kind: 21 });
    out.push(label(format!("{}   (left / right: member, up / down: slot, Enter: change)", x.name), [120, 1000], [0xff; 4]));
    // Choosing an item: the fitting ones under the panel, and the values it would give.
    if let Some((chosen, first)) = item {
        for (k, (_, text)) in x.fitting[slot].iter().enumerate().skip(first).take(VISIBLE) {
            out.push(label(text.clone(), [ITEMS_AT[0], ITEMS_AT[1] + 34 * (k - first) as i32], if k == chosen { [0xff, 0xff, 0xd0, 0x40] } else { [0xff; 4] }));
        }
        if let (Some(now), Some(Some(then))) = (&x.status, x.previews[slot].get(chosen)) {
            let (now, then) = (shown(now), shown(then));
            let mut k = 0;
            for p in nodes.iter().filter(|p| p.path.get(1) == Some(&PREVIEW)) {
                let n = p.node;
                match n.part {
                    ARROW if n.cut == 0 => {
                        if let (Some(a), Some(b)) = (now.get(k), then.get(k)) && a != b {
                            out.push(Element::Sprite { part: ARROW, cut: if b > a { UP } else { DOWN }, at: p.at });
                        }
                    }
                    DIGITS if n.kind == 28 => {
                        let v = then.get(k).copied().unwrap_or(0).max(0).to_string();
                        let len = v.len() as i32;
                        for (i, d) in v.bytes().enumerate() {
                            out.push(Element::Sprite { part: DIGITS, cut: i32::from(d - b'0'), at: [p.at[0] - DIGIT_ADVANCE * (len - i as i32), p.at[1]] });
                        }
                        k += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    out
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
    let exe = barrel_files::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("equip: no glyph table")?;
    let atlas_bytes = barrel_files::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("equip: an empty font atlas")?.width as f32 };
    let desk: Vec<Element> = barrel_escha::data::layout::placed(&pe, crate::menu::BACKGROUND).iter().map(Element::stored).collect();
    let nodes = barrel_escha::data::layout::placed(&pe, LAYOUT);
    let texts = |p: &Placed| barrel_escha::data::layout::text(&pe, p);
    let mut every = desk.clone();
    for member in 0..members.len() {
        every.extend(view(&Equip::Slots { member, slot: 0 }, &nodes, texts, members));
    }
    every.extend((0..13).map(|cut| Element::Sprite { part: DIGITS, cut, at: [0, 0] }));
    every.extend([UP, DOWN].map(|cut| Element::Sprite { part: ARROW, cut, at: [0, 0] }));
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
    let quads = |e: &Equip| {
        let mut all = desk.clone();
        all.extend(view(e, &nodes, texts, members));
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
        let m = [Member { name: "Escha".into(), worn: Default::default(), fitting, status: None, previews: Default::default() }, Member { name: "Logy".into(), worn: Default::default(), fitting: none(), status: None, previews: Default::default() }];
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
