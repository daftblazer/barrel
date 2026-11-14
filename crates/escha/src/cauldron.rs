//! Making an item from the basket: a recipe (the executable's table, `crate::data::recipe`),
//! its ingredients picked from what the player holds, put into the cauldron in the recipe's
//! order ([`crate::synthesis::Synthesis`]) and finished ([`crate::synthesis::finish`]).
//!
//! Not yet the game's: every recipe open (the game's recipe books and ideas are not followed),
//! the cost points a fixed [`COST_POINTS`] (the alchemist's own, `synthesis::start_cp`, needs
//! the save's alchemy level and skills), one item made, no traits carried over or skills used,
//! the alchemy level 1 and no experience kept.

use std::path::Path;

use crate::data::recipe::Recipe;
use barrel_formats::table::Element;

use crate::gather::Got;
use crate::synthesis::{Material, Synthesis};

/// The first recipe's item (the table is found by it).
const FIRST_RECIPE: u32 = 195;
/// The cost points a synthesis may spend (more than any material list here needs).
pub const COST_POINTS: i32 = 99;

/// The recipes and the item table they read.
pub struct Book {
    pub recipes: Vec<Recipe>,
    /// `ITEM_CATEGORY_*` names in enum order.
    categories: Vec<String>,
    /// `itemData.xml`'s rows, by item number.
    items: Vec<Element>,
}

/// An item made: what it is, its quality and effects (item effect numbers), and the days it
/// took.
#[derive(Clone, Debug, PartialEq)]
pub struct Made {
    pub item: i32,
    pub quality: f32,
    pub effects: Vec<i32>,
    pub days: f32,
}

impl Book {
    /// From the English executable's bytes and the install (`Saves_EN/item/itemData.xml`).
    pub fn load(exe: &[u8], root: &Path) -> Result<Book, String> {
        let at = crate::data::recipe::find(exe, FIRST_RECIPE).ok_or("no recipe table")?;
        let recipes = crate::data::recipe::parse(exe, at).map_err(|e| format!("recipes: {e}"))?;
        let pe = barrel_formats::pe::Pe::parse(exe).map_err(|e| e.to_string())?;
        let (names, first) = pe.enum_names("ITEM_CATEGORY_PLANT").ok_or("no item category enum")?;
        let categories = names[first..].iter().map(|n| n.to_string()).collect();
        let bytes = std::fs::read(root.join("Saves_EN/item/itemData.xml")).map_err(|e| format!("itemData.xml: {e}"))?;
        let items = barrel_formats::table::parse(&bytes).map_err(|e| e.to_string())?.children;
        Ok(Book { recipes, categories, items })
    }

    /// An item's name.
    pub fn name(&self, item: i32) -> &str {
        usize::try_from(item).ok().and_then(|i| self.items.get(i)).and_then(|r| r.attr("ItemName")).unwrap_or("?")
    }

    /// What an ingredient asks for, in words: the item's name or the category's tag.
    pub fn ingredient(&self, i: crate::data::recipe::Ingredient) -> String {
        match i {
            crate::data::recipe::Ingredient::Item(n) => self.name(i32::from(n)).to_string(),
            crate::data::recipe::Ingredient::Category(c) => self.categories.get(c as usize).map_or("?".into(), |t| format!("({})", t.trim_start_matches("ITEM_CATEGORY_"))),
        }
    }

    /// The items of a category (its enum number).
    pub fn of_category(&self, c: u16) -> Vec<i32> {
        (0..self.items.len() as i32).filter(|&i| self.categories_of(i).contains(&c)).collect()
    }

    fn categories_of(&self, item: i32) -> Vec<u16> {
        let Some(row) = usize::try_from(item).ok().and_then(|i| self.items.get(i)) else { return Vec::new() };
        (0..4).filter_map(|k| row.attr(&format!("category_{k:02}"))).filter_map(|tag| self.categories.iter().position(|c| c == tag)).map(|n| n as u16).collect()
    }

    /// Recipe `r`'s ingredients filled from `basket`: for each, in the recipe's order, the
    /// first entry not yet picked that fits (its index in `basket`); none when one cannot be.
    pub fn fill(&self, r: usize, basket: &[Got]) -> Option<Vec<usize>> {
        let recipe = self.recipes.get(r)?;
        let mut picks: Vec<usize> = Vec::new();
        for want in crate::synthesis::order(recipe) {
            let k = (0..basket.len()).find(|&k| !picks.contains(&k) && crate::synthesis::fits(want, basket[k].item as u16, &self.categories_of(basket[k].item)))?;
            picks.push(k);
        }
        Some(picks)
    }

    /// The recipes `basket` holds every ingredient for (the table's records without
    /// ingredients, items such as "Poly. Poison S" that take 0 days, left out).
    pub fn makeable(&self, basket: &[Got]) -> Vec<usize> {
        (0..self.recipes.len()).filter(|&r| !self.recipes[r].slots.is_empty() && self.fill(r, basket).is_some()).collect()
    }

    /// Recipe `r` made from `basket`'s entries `picks` (each material's effect value its own
    /// quality); `roll(lo, hi)` draws the finish's random numbers.
    pub fn make(&self, r: usize, picks: &[usize], basket: &[Got], roll: &mut dyn FnMut(i32, i32) -> i32) -> Result<Made, String> {
        let recipe = self.recipes.get(r).ok_or("no such recipe")?;
        let mut s = Synthesis::with_cp(COST_POINTS);
        for &k in picks {
            let got = basket.get(k).ok_or("a material is not in the basket")?;
            let row = usize::try_from(got.item).ok().and_then(|i| self.items.get(i)).ok_or("a material has no item row")?;
            if !s.add(&Material::from_item(row).with_quality(got.quality)) {
                return Err(format!("{}: too few cost points left", self.name(got.item)));
            }
        }
        let int = |attr: &str| self.items.get(recipe.item as usize).and_then(|row| row.attr(attr)).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let f = crate::synthesis::Finish {
            recipe: (r, recipe),
            level: int("level"),
            equip_cost: int("EquipCost"),
            traits: [-1; 3],
            trait_table: &[],
            count: 1,
            exp_rate: 0,
            alchemy_level: 1,
            categories: [-1; 4],
            mix: &[],
            extra_uses: 0,
            compression: 0,
        };
        let mut k = crate::data::save::Knowledge { item: vec![0; 560], recipe: vec![0; 265], eff: vec![0; 698], potential: vec![0; 400], mix_esp: vec![0; 90], ..Default::default() };
        let mut revealed = vec![[false; 10]; self.recipes.len()];
        let (unit, _) = crate::synthesis::finish(&s, &f, 0, &mut k, &mut revealed, roll);
        Ok(Made { item: i32::from(recipe.item), quality: f32::from(unit.quality), effects: unit.effects.iter().copied().filter(|&e| e >= 0).map(i32::from).collect(), days: recipe.days })
    }
}
