//! The Load Game list, from the game's own layout 1035 (`WinXls_EN/a15_saveload.g1t` on the
//! menu background): four slot plates (layout 1040, 200 pixels apart from y 200: the lit plate,
//! part 0x152, on the chosen slot, the plain one, 0x153, on the others; the stamp, part 0x151,
//! "NEW DATA" (cut 0) on an empty slot, the lead's "DATA" picture on a saved one (cut 1 Escha,
//! cut 2 Logy); the texts, layout 1042: "New SaveData", or "Game Data N", the date line and
//! the time it was saved), the heading (part 0x157: cut 0 "Save", 1 "Load", 2 "Delete") and the
//! details panel (layout 1044).
//!
//! Barrel's slots are its own saves (`barrel_escha::saves`). Not yet the game's: the date line's
//! wording ("Escha  Year 1, 4/1"; the layout's filler is the Japanese "【ロジー編】 １年目１２月２４日");
//! the details panel showing only the protagonist (Barrel's saves keep no play time, levels,
//! money or party); how many slots the list has (the saved ones and one empty after them, at
//! least four) and its scrolling (the chosen slot kept in view); the party rows (layout 1046)
//! left out; no help line.

use barrel_escha::data::layout::{Placed, Text};
use barrel_escha::saves::Slot;

use crate::hud::Element;
use crate::title::Button;

pub const LAYOUT: u32 = 1035;
const SLOT: u32 = 1040;
const SLOT_TEXTS: u32 = 1042;
const DETAILS: u32 = 1044;
const PARTY_ROW: u32 = 1046;
const HELP_BAR: u32 = 372;
const LIT_PLATE: i32 = 0x152;
const PLAIN_PLATE: i32 = 0x153;
const STAMP: i32 = 0x151;
const HEADING: i32 = 0x157;
const SLOT_TOP: i32 = 200;
const SLOT_STEP: i32 = 200;
const VISIBLE: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct LoadList {
    pub slots: Vec<Option<Slot>>,
    pub chosen: usize,
    pub first: usize,
}

/// What a button did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Load(std::path::PathBuf),
    Back,
}

impl LoadList {
    /// The list over the saved slots: each numbered slot up to the last saved one, then one
    /// empty, at least four.
    pub fn new(saved: Vec<Slot>) -> LoadList {
        let last = saved.iter().map(|s| s.number).max().unwrap_or(0) as usize;
        let mut slots: Vec<Option<Slot>> = vec![None; (last + 1).max(VISIBLE)];
        for s in saved {
            if s.number >= 1 {
                let k = s.number as usize - 1;
                slots[k] = Some(s);
            }
        }
        LoadList { slots, chosen: 0, first: 0 }
    }

    pub fn press(&mut self, b: Button) -> Option<Outcome> {
        let n = self.slots.len();
        match b {
            Button::Up => self.chosen = (self.chosen + n - 1) % n,
            Button::Down => self.chosen = (self.chosen + 1) % n,
            Button::Left | Button::Right => {}
            Button::Back => return Some(Outcome::Back),
            Button::Confirm => {
                if let Some(s) = &self.slots[self.chosen] {
                    return Some(Outcome::Load(s.path.clone()));
                }
            }
        }
        if self.chosen < self.first {
            self.first = self.chosen;
        } else if self.chosen >= self.first + VISIBLE {
            self.first = self.chosen + 1 - VISIBLE;
        }
        None
    }

    /// The elements to draw, from layout 1035's placed nodes (`texts` gives a node's text).
    pub fn elements(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>) -> Vec<Element> {
        use barrel_escha::calendar::Date;
        let mut out = Vec::new();
        let name = |logy: bool| if logy { "Logy" } else { "Escha" };
        for p in nodes {
            let n = p.node;
            let at = p.at;
            // Which visible row a slot's node is in.
            let row = p.path.iter().any(|&l| l == SLOT || l == SLOT_TEXTS).then(|| ((at[1] - SLOT_TOP).div_euclid(SLOT_STEP)).clamp(0, VISIBLE as i32 - 1) as usize);
            let shown = row.and_then(|r| self.slots.get(self.first + r).map(|s| (self.first + r, s)));
            if p.path.contains(&HELP_BAR) && n.part == 0x164 {
                continue;
            }
            if n.part == 0x164 {
                let Some(t) = texts(p) else { continue };
                if let (Some(_), Some((k, slot))) = (row, shown) {
                    // The slot's texts, told apart by the layout's own wording: "New SaveData",
                    // "Game Data 999", the date line ("【…"), the time saved ("9999/…").
                    let text = match (slot, t.text.as_str()) {
                        (None, "New SaveData") => Some(t.text.clone()),
                        (Some(_), w) if w.starts_with("Game Data") => Some(format!("Game Data {}", k + 1)),
                        (Some(s), w) if w.starts_with('【') => {
                            let d = Date::of(s.today.max(0) as u32);
                            Some(format!("{}  Year {}, {}/{}", name(s.logy), d.year, d.month, d.day))
                        }
                        (Some(s), w) if w.starts_with("9999/") => s.written.map(barrel_escha::saves::stamp),
                        _ => None,
                    };
                    out.extend(text.map(|text| Element::Label(Text { text, ..t })));
                    continue;
                }
                if p.path.contains(&DETAILS) {
                    // The details: the protagonist only.
                    let chosen = self.slots.get(self.chosen).and_then(Option::as_ref);
                    match (t.text.as_str(), chosen) {
                        ("Protagoist", Some(_)) => out.push(Element::Label(t)),
                        ("キャラ名", Some(s)) if t.at[1] == 200 => out.push(Element::Label(Text { text: name(s.logy).into(), ..t })),
                        _ => {}
                    }
                    continue;
                }
                if !t.text.is_empty() && !t.filled_in() {
                    out.push(Element::Label(t));
                }
                continue;
            }
            if p.path.contains(&PARTY_ROW) {
                continue;
            }
            let (part, cut) = (n.part, n.cut.max(0));
            match (part, shown) {
                (LIT_PLATE, Some((k, _))) if k != self.chosen => continue,
                (PLAIN_PLATE, Some((k, _))) if k == self.chosen => continue,
                (LIT_PLATE | PLAIN_PLATE, None) if row.is_some() => continue,
                (STAMP, Some((_, slot))) => {
                    let cut = slot.as_ref().map_or(0, |s| 1 + i32::from(s.logy));
                    out.push(Element::Sprite { part, cut, at });
                    continue;
                }
                (STAMP, None) if row.is_some() => continue,
                (HEADING, _) => {
                    out.push(Element::Sprite { part, cut: 1, at });
                    continue;
                }
                _ => {}
            }
            out.push(if n.kind == crate::hud::MIRRORED { Element::Mirrored { part, cut, at } } else { Element::Sprite { part, cut, at } });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(n: u32, logy: bool) -> Slot {
        Slot { number: n, path: format!("slot{n:02}.save").into(), logy, today: 90, written: Some(0) }
    }

    #[test]
    fn slots_and_moves() {
        let mut l = LoadList::new(vec![slot(2, true), slot(5, false)]);
        assert_eq!(l.slots.len(), 6);
        assert!(l.slots[0].is_none() && l.slots[1].is_some() && l.slots[4].is_some());
        assert_eq!(l.press(Button::Confirm), None);
        l.press(Button::Down);
        assert_eq!(l.press(Button::Confirm), Some(Outcome::Load("slot02.save".into())));
        for _ in 0..3 {
            l.press(Button::Down);
        }
        assert_eq!((l.chosen, l.first), (4, 1));
        assert_eq!(l.press(Button::Back), Some(Outcome::Back));
    }
}
