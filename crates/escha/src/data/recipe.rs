//! Synthesis recipes, which Escha & Logy DX compiles into its executable instead of a data
//! file (see `docs/tables.md`, "Recipes").
//!
//! The table is an array of 288-byte records (72 little-endian i32), one per synthesizable
//! item, mostly in item order. Its address differs between builds, so [`find`] looks for it by
//! shape: the first item's number, then the next items' numbers every 288 bytes.

use barrel_formats::{Error, u32_at};

const RECORD: usize = 288;
const MAX_RECIPES: usize = 1024;

/// What an ingredient slot takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ingredient {
    /// This item (its number in the item enum).
    Item(u16),
    /// Any item of this category (its number in the category enum).
    Category(u16),
}

/// One step of a slot's effect line: `effect` (an item effect number, or none) holds while
/// the slot's value is below `up_to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub effect: Option<u16>,
    /// -1 when the step is not used.
    pub up_to: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub ingredient: Ingredient,
    pub count: u32,
}

/// One element's effect line (lines 0..3 follow Fire, Water, Wind and Earth).
pub type Line = [Step; 4];

/// The effect `line` gives at `value`, as the game picks it: the value is capped at 99, and
/// the first step whose "up to" is above it wins. If none is, the fourth step's effect is
/// used.
pub fn effect_at(line: &Line, value: i32) -> Option<u16> {
    let v = value.min(99);
    line.iter().find(|s| v < s.up_to).unwrap_or(&line[3]).effect
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// The item it makes.
    pub item: u16,
    /// The item's kind (0..7).
    pub kind: u32,
    pub cost: u32,
    /// Synthesis time in days.
    pub days: f32,
    /// How many uses an item made by it starts with (word 8, `0x1400e3220`).
    pub uses: i32,
    /// The ingredients, unused slots left out.
    pub slots: Vec<Slot>,
    /// The four effect lines, one per element. The record keeps line k with ingredient
    /// slot k, but the game feeds it element k's value, whatever that slot holds.
    pub lines: [Line; 4],
}

impl Recipe {
    /// The recipe's effects in the game's order (`0x1400e32d0`): line by line, step by step,
    /// the steps with an effect. The save's `mix_recipe` flags (`save::Summary::recipe_effects`)
    /// follow this order: flag j says whether the j-th effect is revealed (its name shown, else
    /// "???").
    pub fn effect_list(&self) -> Vec<u16> {
        self.lines.iter().flatten().filter_map(|s| s.effect).collect()
    }
}

fn i32_at(data: &[u8], at: usize) -> Result<i32, Error> {
    u32_at(data, at).map(|v| v as i32)
}

fn effect(n: i32) -> Option<u16> {
    u16::try_from(n).ok()
}

/// Read the records from `data[at..]` until one no longer looks like a recipe (its item or
/// kind is out of range). The records are mostly, but not always, in item order.
pub fn parse(data: &[u8], at: usize) -> Result<Vec<Recipe>, Error> {
    let mut out: Vec<Recipe> = Vec::new();
    while out.len() < MAX_RECIPES {
        let r = at + RECORD * out.len();
        if r + RECORD > data.len() {
            break;
        }
        let item = i32_at(data, r)?;
        let kind = i32_at(data, r + 4)?;
        if !(0..0x10000).contains(&item) || !(0..16).contains(&kind) {
            break;
        }
        let mut slots = Vec::new();
        let mut lines = [[Step { effect: None, up_to: -1 }; 4]; 4];
        for s in 0..4 {
            let w = r + 96 + 48 * s;
            for (k, step) in lines[s].iter_mut().enumerate() {
                let p = w + 16 + 8 * k;
                *step = Step { effect: effect(i32_at(data, p)?), up_to: i32_at(data, p + 4)? };
            }
            let (kind, id, count) = (i32_at(data, w)?, i32_at(data, w + 4)?, i32_at(data, w + 8)?);
            let Ok(id) = u16::try_from(id) else { continue };
            let ingredient = match kind {
                0 => Ingredient::Item(id),
                1 => Ingredient::Category(id),
                _ => return Err(Error::Corrupt("recipe ingredient kind")),
            };
            slots.push(Slot { ingredient, count: count.max(0) as u32 });
        }
        out.push(Recipe {
            item: item as u16,
            kind: kind as u32,
            cost: i32_at(data, r + 20)?.max(0) as u32,
            days: f32::from_bits(u32_at(data, r + 24)?),
            uses: i32_at(data, r + 32)?,
            slots,
            lines,
        });
    }
    if out.is_empty() {
        return Err(Error::NotThisFormat);
    }
    Ok(out)
}

/// Find the recipe table in an executable's bytes, given the first synthesizable item's
/// number (195, Craft, in Escha & Logy DX): a 4-aligned `first` followed by `first + 1` and
/// `first + 2` at the next two records.
pub fn find(exe: &[u8], first: u32) -> Option<usize> {
    let mut at = 0;
    while at + 3 * RECORD <= exe.len() {
        if u32_at(exe, at).ok() == Some(first)
            && u32_at(exe, at + RECORD).ok() == Some(first + 1)
            && u32_at(exe, at + 2 * RECORD).ok() == Some(first + 2)
        {
            return Some(at);
        }
        at += 4;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(item: i32, slots: &[[i32; 12]]) -> Vec<u8> {
        let mut w = [0i32; 72];
        w[0] = item;
        w[1] = 1;
        w[5] = 500;
        w[6] = 3.0f32.to_bits() as i32;
        for s in 0..4 {
            w[24 + 12 * s + 1] = -1;
        }
        for (s, slot) in slots.iter().enumerate() {
            w[24 + 12 * s..36 + 12 * s].copy_from_slice(slot);
        }
        w.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn reads_and_finds() {
        // Craft's thorny-nuts slot: none to 20, effect 101 to 80, effect 102 to 100.
        let thorns = [1, 92, 1, 0, -1, 20, 101, 80, 102, 100, -1, -1];
        let mut exe = vec![0u8; 12];
        for item in 195..198 {
            exe.extend(record(item, &[thorns]));
        }
        exe.extend(record(100, &[])); // out of order, still a recipe
        exe.extend(record(0x10000, &[])); // not an item: the table ends
        assert_eq!(find(&exe, 195), Some(12));
        let r = parse(&exe, 12).unwrap();
        assert_eq!(r.len(), 4);
        assert_eq!((r[0].item, r[0].cost, r[0].days), (195, 500, 3.0));
        let s = &r[0].slots[0];
        assert_eq!((s.ingredient, s.count), (Ingredient::Category(92), 1));
        let line = &r[0].lines[0];
        assert_eq!(effect_at(line, 0), None);
        assert_eq!(effect_at(line, 19), None);
        assert_eq!(effect_at(line, 20), Some(101));
        assert_eq!(effect_at(line, 80), Some(102));
        assert_eq!(effect_at(line, 500), Some(102)); // capped at 99
        assert_eq!(r[0].slots.len(), 1);
    }
}
