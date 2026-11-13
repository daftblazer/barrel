//! The main menu's Status page, from the game's layout 400 (`a15_menu.g1t`): its book branch,
//! layout 399 (the open notebook; layout 431 on its right page: the character's name, "LV",
//! "Next", "HP", "MP", "ATK", "DEF", "SPD", four "RES" rows with their element icons and
//! "Friend", each with a number, and the bookmark with the figure, layout 393).
//!
//! A number is a row of digit sprites (part 0x4e, cut = digit, 20 pixels wide) on a node of
//! kind 28; HP and MP are two numbers each with a slash (part 0x4e cut 12) between. Read from
//! the layout: the pictures, the labels, the numbers' places and order. The values: the lead's
//! base stats at the session's level from the party growth table (`battle_data::party_growth`)
//! plus its equipment's, and the experience still needed from the experience table
//! (`growth::to_next`).
//!
//! Not yet the game's: the numbers ending at their node (right-aligned; the first of the HP
//! and MP pairs ending at the slash after it); HP and MP full; the equipment as freshly made items
//! (its own quality and traits not kept); Friend 0 (no friendship yet); the name the lead's short name;
//! the digits drawn white as stored (the game's number widget, kind 28, likely tints them: white
//! is hard to read on the page); the page's other branches (equipment effects 398, skills 388 and 387) and its tabs left out;
//! Escha's figure on the bookmark for both leads.

use barrel_formats::layout::{Placed, Text};

use crate::hud::Element;

pub const LAYOUT: u32 = 400;
const BOOK: u32 = 399;
const DIGITS: i32 = 0x4e;
const NUMBER_KIND: i32 = 28;
const SLASH: i32 = 12;
const DIGIT_ADVANCE: i32 = 20;
const NAME_FILLER: &str = "アイウエオカ";

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Values {
    pub name: String,
    pub level: i32,
    pub next: i32,
    pub hp: (i32, i32),
    pub mp: (i32, i32),
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub resist: [i32; 4],
    pub friend: i32,
}

impl Values {
    /// The lead (Logy when `logy`) from the executable's tables: as `member` has it (the
    /// session's level, experience and equipment; level 1 with none and no kit without it), its
    /// base stats at that level plus what its equipment gives (`items`, `itemData.xml`'s rows;
    /// each item as freshly made, quality 50).
    pub fn lead(pe: &barrel_formats::pe::Pe, logy: bool, member: Option<&barrel_game::session::Member>, items: &[barrel_formats::table::Element]) -> Option<Values> {
        use barrel_formats::battle_data as bd;
        let growth = bd::party_growth(pe, bd::PARTY_GROWTH_EN)?;
        let exp = bd::exp_table(pe, bd::EXP_TABLE_EN)?;
        let (level, experience) = member.map_or((1, 0), |m| (m.level.max(1), m.experience));
        let s = *growth.get(usize::from(logy))?.get((level - 1) as usize)?;
        let rows: Vec<barrel_game::equipment::ItemRow> = items.iter().map(barrel_game::equipment::ItemRow::from_row).collect();
        let t = barrel_game::equipment::Tables { items: &rows, effects: &[], traits: &[], acts: &[] };
        let mut add = [0; 9];
        for &item in member.map_or(&[][..], |m| &m.equipment[..]).iter().filter(|&&i| i >= 0) {
            let unit = barrel_formats::save::ItemUnit { serial: 1, item: item as i16, quality: 50.0, traits: [-1; 3], word_e: 0, effects: [-1; 4], equip_cost: 0, word_1a: 0, counts: [0; 4] };
            for (k, a) in add.iter_mut().enumerate() {
                *a += barrel_game::equipment::item_stat(&t, &unit, k, 0, &mut |lo, _| lo);
            }
        }
        let (hp, mp) = (s.hp + add[0], s.mp + add[1]);
        Some(Values {
            name: if logy { "Logy" } else { "Escha" }.into(),
            level,
            next: barrel_game::growth::to_next(&exp, level, experience, 0)?,
            hp: (hp, hp),
            mp: (mp, mp),
            attack: s.attack + add[2],
            defense: s.defense + add[3],
            speed: s.speed + add[4],
            resist: [add[5], add[6], add[7], add[8]],
            friend: 0,
        })
    }

    /// The numbers in the order the page's number nodes come.
    fn numbers(&self) -> [i32; 14] {
        let [r0, r1, r2, r3] = self.resist;
        [self.level, self.next, self.hp.0, self.hp.1, self.mp.0, self.mp.1, self.attack, self.defense, self.speed, r0, r1, r2, r3, self.friend]
    }
}

/// The elements to draw from layout 400's placed nodes (`texts` gives a node's text).
pub fn elements(nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, v: &Values) -> Vec<Element> {
    let numbers = v.numbers();
    let mut k = 0;
    let mut out = Vec::new();
    for p in nodes.iter().filter(|p| p.path.get(1) == Some(&BOOK)) {
        let n = p.node;
        if n.part == 0x164 {
            match texts(p) {
                Some(t) if t.text == NAME_FILLER => out.push(Element::Label(Text { text: v.name.clone(), ..t })),
                Some(t) if !t.text.is_empty() && !t.filled_in() => out.push(Element::Label(t)),
                _ => {}
            }
            continue;
        }
        if n.part == DIGITS && n.kind == NUMBER_KIND {
            let Some(&value) = numbers.get(k) else { continue };
            k += 1;
            let digits: Vec<i32> = value.max(0).to_string().bytes().map(|b| i32::from(b - b'0')).collect();
            // The last digit on the node, or, with the slash of a pair just after it on the
            // row (HP, MP), ending at the slash.
            let slash = nodes.iter().find(|q| q.node.part == DIGITS && q.node.cut == SLASH && (q.at[1] - p.at[1]).abs() < 10 && (0..DIGIT_ADVANCE).contains(&(q.at[0] - p.at[0])));
            let end = slash.map_or(p.at[0], |q| q.at[0] - DIGIT_ADVANCE);
            let last = digits.len() as i32 - 1;
            for (i, d) in digits.into_iter().enumerate() {
                out.push(Element::Sprite { part: DIGITS, cut: d, at: [end - DIGIT_ADVANCE * (last - i as i32), p.at[1]] });
            }
            continue;
        }
        out.push(Element::stored(p));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::layout::Node;

    #[test]
    fn numbers_end_at_their_nodes() {
        let node = |part, kind, at| Placed { path: vec![LAYOUT, BOOK], node: Node { part, cut: 0, at, kind }, at, va: 0 };
        let mut slash = node(DIGITS, 0, [950, 140]);
        slash.node.cut = SLASH;
        let nodes = vec![node(DIGITS, NUMBER_KIND, [944, 94]), node(DIGITS, NUMBER_KIND, [944, 140]), slash];
        let v = Values { name: "Escha".into(), level: 1, next: 10, hp: (0, 0), mp: (0, 0), attack: 0, defense: 0, speed: 0, resist: [0; 4], friend: 0 };
        let e = elements(&nodes, |_| None, &v);
        let s = |cut, at| Element::Sprite { part: DIGITS, cut, at };
        // The second number ends at the slash just after it.
        assert_eq!(e, vec![s(1, [944, 94]), s(1, [910, 140]), s(0, [930, 140]), s(SLASH, [950, 140])]);
    }
}
