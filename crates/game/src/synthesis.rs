//! Synthesis: the cauldron's state as materials go in and skills are used, following the
//! rules read from Escha & Logy DX's executable (`docs/tables.md`, "Synthesis skills").
//!
//! The four elements are, in order, Fire, Water, Wind and Earth.

use barrel_formats::recipe::{Ingredient, Line, Recipe, effect_at};

/// Whether an item (its number and its categories' numbers) can go where `ingredient` asks.
pub fn fits(ingredient: Ingredient, item: u16, categories: &[u16]) -> bool {
    match ingredient {
        Ingredient::Item(i) => i == item,
        Ingredient::Category(c) => categories.contains(&c),
    }
}

/// The ingredients a recipe takes, one entry per material, in the order the game asks for
/// them without Freestyle: each slot as many times as its count.
pub fn order(recipe: &Recipe) -> Vec<Ingredient> {
    recipe.slots.iter().flat_map(|s| std::iter::repeat_n(s.ingredient, s.count as usize)).collect()
}

/// Attribute values stop at this.
pub const MAX_ATTRIBUTE: f32 = 999.0;
/// The effect value stops at this.
pub const MAX_EFFECT: f32 = 120.0;
/// Each element's point count stays between 0 and this.
pub const MAX_POINTS: i32 = 5;

/// What a material brings to the cauldron.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Which elements it has (`itemData` columns `fire`, `water`, `wind`, `soil`).
    pub elements: [bool; 4],
    /// Its element value (starts from `itemData`'s `value`).
    pub element_value: i32,
    /// Its effect value (starts from `itemData`'s `effect`).
    pub effect: f32,
    /// The cost points it uses up (where the game takes this from is still open).
    pub cp: i32,
}

impl Material {
    /// A material as an `itemData.xml` row describes it, before quality or traits change
    /// it. Missing or unreadable numbers count as 0.
    pub fn from_item(row: &barrel_formats::table::Element) -> Material {
        let num = |name: &str| row.attr(name).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
        Material {
            elements: ["fire", "water", "wind", "soil"].map(|e| num(e) > 0),
            element_value: num("value"),
            effect: num("effect") as f32,
            cp: 0,
        }
    }
}

/// A material's cost after Crafting Help (type 20): `percent` of it comes off, never
/// below 0. Crafting Help 3 takes 100 off.
pub fn crafting_help(cp: i32, percent: i32) -> i32 {
    (cp - cp * percent / 100).max(0)
}

/// The skills whose effect on the state is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Skill {
    /// Type 0: the element gains this many points.
    Conversion { points: i32 },
    /// Type 22: the cost points left grow by `amount`, with no limit.
    CpRecovery { amount: i32 },
    /// Type 4: the selected attribute goes to 0; each other one gains a third of it, times
    /// `percent` / 100.
    ResetAttribute { percent: i32 },
    /// Type 23: every attribute grows by `percent`, and the effect bonus by `effect`.
    FullPower { percent: i32, effect: i32 },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Synthesis {
    /// The four attribute values.
    pub attributes: [f32; 4],
    /// The last change to each attribute.
    pub changes: [f32; 4],
    /// The attributes' sum as the game keeps it (whole numbers, added as each one changes).
    pub total: i32,
    effect_sum: f32,
    materials: u32,
    /// Effect value added by skills.
    pub effect_bonus: i32,
    /// Each element's points, for skills.
    pub points: [i32; 4],
    /// Cost points left.
    pub cp: i32,
}

impl Synthesis {
    pub fn new() -> Synthesis {
        Synthesis::default()
    }

    /// A synthesis with `cp` cost points to spend.
    pub fn with_cp(cp: i32) -> Synthesis {
        Synthesis { cp, ..Synthesis::default() }
    }

    /// The effect value: the materials' average, plus the skill bonus, at most 120.
    pub fn effect_value(&self) -> f32 {
        let average = if self.materials == 0 { 0.0 } else { self.effect_sum / self.materials as f32 };
        (average + self.effect_bonus as f32).min(MAX_EFFECT)
    }

    /// The effect each of a recipe's four lines gives now: line k is read at attribute k's
    /// value (cut to a whole number).
    pub fn effects(&self, lines: &[Line; 4]) -> [Option<u16>; 4] {
        std::array::from_fn(|k| effect_at(&lines[k], self.attributes[k] as i32))
    }

    /// Put a material in: it uses up its cost points (the game refuses it, changing nothing,
    /// when it costs some and too few are left), then each element it has gains its element
    /// value. Returns whether it went in.
    pub fn add(&mut self, m: &Material) -> bool {
        // The game scales the cost by a percentage passed in by the synthesis screen; at
        // 100 it is the cost itself. What else it can be is still open.
        let used = m.cp;
        if used > 0 && self.cp - used < 0 {
            return false;
        }
        self.cp -= used;
        self.effect_sum += m.effect;
        self.materials += 1;
        for e in 0..4 {
            let change = if m.elements[e] { m.element_value as f32 } else { 0.0 };
            self.changes[e] = change;
            self.attributes[e] += change;
        }
        self.total = 0;
        for a in &mut self.attributes {
            *a = a.min(MAX_ATTRIBUTE);
            self.total += *a as i32;
        }
        true
    }

    /// Pay a skill's cost in element points; a count never goes below 0.
    pub fn pay(&mut self, cost: [i32; 4]) {
        for (p, c) in self.points.iter_mut().zip(cost) {
            *p = (*p - c).max(0);
        }
    }

    /// Use `skill` on element `e`, after paying for it.
    pub fn use_skill(&mut self, skill: Skill, e: usize) {
        match skill {
            Skill::CpRecovery { amount } => self.cp += amount,
            Skill::Conversion { points } => {
                self.points[e] = (self.points[e] + points).clamp(0, MAX_POINTS);
            }
            Skill::ResetAttribute { percent } => {
                let v = self.attributes[e] as i32;
                let share = ((v / 3 * percent) as f32 / 100.0) as i32 as f32;
                self.attributes[e] = 0.0;
                self.changes[e] = -(v as f32);
                self.total = 0;
                for o in (0..4).filter(|&o| o != e) {
                    self.changes[o] = share;
                    self.attributes[o] = (self.attributes[o] + share).min(MAX_ATTRIBUTE);
                    self.total += self.attributes[o] as i32;
                }
            }
            Skill::FullPower { percent, effect } => {
                self.total = 0;
                for o in 0..4 {
                    let change = (self.attributes[o] / 100.0 * percent as f32) as i32 as f32;
                    self.changes[o] = change;
                    self.attributes[o] += change;
                    // The game adds to the sum before the cap.
                    self.total += self.attributes[o] as i32;
                    self.attributes[o] = self.attributes[o].min(MAX_ATTRIBUTE);
                }
                self.effect_bonus += effect;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRE_WATER: Material = Material { elements: [true, true, false, false], element_value: 10, effect: 50.0, cp: 0 };
    const EARTH: Material = Material { elements: [false, false, false, true], element_value: 15, effect: 60.0, cp: 0 };

    #[test]
    fn materials_fill_their_elements() {
        let mut s = Synthesis::new();
        s.add(&FIRE_WATER);
        s.add(&EARTH);
        assert_eq!(s.attributes, [10.0, 10.0, 0.0, 15.0]);
        assert_eq!(s.total, 35);
        assert_eq!(s.effect_value(), 55.0);
        let big = Material { element_value: 2000, ..EARTH };
        s.add(&big);
        assert_eq!(s.attributes[3], MAX_ATTRIBUTE);
    }

    #[test]
    fn ingredient_order() {
        use barrel_formats::recipe::{Slot, Step};
        let none = Step { effect: None, up_to: -1 };
        let r = Recipe {
            item: 201,
            kind: 1,
            cost: 600,
            days: 4.0,
            slots: vec![
                Slot { ingredient: Ingredient::Item(54), count: 2 },
                Slot { ingredient: Ingredient::Category(31), count: 1 },
            ],
            lines: [[none; 4]; 4],
        };
        assert_eq!(order(&r), [Ingredient::Item(54), Ingredient::Item(54), Ingredient::Category(31)]);
        assert!(fits(Ingredient::Category(31), 7, &[2, 31]));
        assert!(!fits(Ingredient::Item(54), 55, &[31]));
    }

    #[test]
    fn from_item_row() {
        let root = barrel_formats::table::parse_str(
            r#"<Root><itemData effect="50" attribute="ELEMENT_FIRE" value="10" fire="1" water="0" wind="2" soil="0"/></Root>"#,
        )
        .unwrap();
        let m = Material::from_item(&root.children[0]);
        assert_eq!(m, Material { elements: [true, false, true, false], element_value: 10, effect: 50.0, cp: 0 });
    }

    #[test]
    fn lines_follow_elements() {
        use barrel_formats::recipe::Step;
        let none = Step { effect: None, up_to: -1 };
        let mut lines = [[none; 4]; 4];
        lines[0] = [Step { effect: None, up_to: 20 }, Step { effect: Some(7), up_to: 100 }, none, none];
        lines[3] = [Step { effect: Some(9), up_to: 100 }, none, none, none];
        let mut s = Synthesis::new();
        assert_eq!(s.effects(&lines), [None, None, None, Some(9)]);
        s.add(&FIRE_WATER);
        s.add(&FIRE_WATER);
        assert_eq!(s.effects(&lines)[0], Some(7)); // Fire at 20
    }

    #[test]
    fn cost_points() {
        let m = Material { cp: 6, ..EARTH };
        let mut s = Synthesis::with_cp(10);
        assert!(s.add(&m));
        assert_eq!(s.cp, 4);
        assert!(!s.add(&m)); // 4 left, costs 6: refused
        assert_eq!((s.cp, s.attributes[3]), (4, 15.0));
        s.use_skill(Skill::CpRecovery { amount: 25 }, 0);
        assert!(s.add(&m));
        assert_eq!(s.cp, 23);
        assert_eq!((crafting_help(6, 50), crafting_help(6, 75), crafting_help(6, 100)), (3, 2, 0));
    }

    #[test]
    fn skills() {
        let mut s = Synthesis::new();
        s.add(&Material { elements: [true; 4], element_value: 30, effect: 100.0, cp: 0 });
        s.use_skill(Skill::ResetAttribute { percent: 100 }, 0);
        assert_eq!(s.attributes, [0.0, 40.0, 40.0, 40.0]);
        assert_eq!(s.total, 120);
        s.use_skill(Skill::FullPower { percent: 50, effect: 50 }, 0);
        assert_eq!(s.attributes, [0.0, 60.0, 60.0, 60.0]);
        assert_eq!(s.effect_value(), MAX_EFFECT);
        s.points = [1, 0, 0, 0];
        s.pay([2, 0, 0, 0]);
        assert_eq!(s.points[0], 0);
        s.use_skill(Skill::Conversion { points: 7 }, 1);
        assert_eq!(s.points[1], MAX_POINTS);
    }
}

/// The alchemy experience a finished synthesis gives (`0x14030ef30`). `level` is the made
/// item's `itemData` level, `days` the recipe's synthesis time less the days bonuses take off
/// (`0x1400fe4e0`, at least 1), `count` how many items it made, `first` whether the recipe
/// had never been made (its knowledge below 6; the game raises it to 6 then), `rate` the
/// synthesis experience bonus (bonus value 3, a percentage) and `alchemy_level` the current
/// level: at 50 no more experience comes.
///
/// The first item gives `2 × trunc(level × (trunc(days / 10) + 1) / 3)`; each further item
/// half what the one before gave, back to the full amount once that would fall below a tenth
/// of it; a first synthesis adds `level`. The sum is scaled by `(100 + rate) / 100`
/// (truncated) and is at least `count`.
pub fn experience(level: i32, days: i32, count: i32, first: bool, rate: i32, alchemy_level: i32) -> i32 {
    let full = level * (days / 10 + 1) / 3 * 2;
    let mut sum = full;
    let mut step = full;
    for _ in 1..count {
        let half = step / 2;
        step = if half < full / 10 { full } else { half };
        sum += step;
    }
    if first {
        sum += level;
    }
    let exp = ((rate as f32 + 100.0) / 100.0 * sum as f32) as i32;
    if alchemy_level >= 50 { 0 } else { exp.max(count) }
}

#[cfg(test)]
mod experience_tests {
    use super::experience;

    #[test]
    fn experience_halves_per_copy_and_rewards_a_first_synthesis() {
        // Level 6, 2 days: 2 × trunc(6 / 3) = 4 for the first item.
        assert_eq!(experience(6, 2, 1, false, 0, 12), 4);
        // Three items: 4 + 2 + 1.
        assert_eq!(experience(6, 2, 3, false, 0, 12), 7);
        // Level 60, 1 day: 40, then 20, 10, 5, and 2 is below 4, so 40 again.
        assert_eq!(experience(60, 1, 5, false, 0, 12), 40 + 20 + 10 + 5 + 40);
        assert_eq!(experience(6, 2, 1, true, 0, 12), 10);
        assert_eq!(experience(6, 2, 1, true, 25, 12), 12);
        assert_eq!(experience(1, 1, 3, false, 0, 12), 3);
        assert_eq!(experience(30, 5, 1, false, 0, 50), 0);
    }
}

/// The finished item's quality (`0x14030baf0` and on): the cauldron's effect value (its
/// quality so far, at most [`MAX_EFFECT`]) plus, for each trait the item takes, the amount of
/// its `ITEM_SUB_ITM_ADD_QUALITY` slots (`0x1402a1fd0` asks for the kind, `0x1402a1ed0` gives
/// the amount, a roll in the slot's range), kept within 0 and 120 (`0x1407272a0`).
pub fn finished_quality(effect_value: f32, traits: &[&barrel_formats::battle_data::Trait], roll: &mut dyn FnMut(i32, i32) -> i32) -> f32 {
    let mut q = effect_value;
    for t in traits {
        for s in t.slots.iter().filter(|s| s.kind == "ITEM_SUB_ITM_ADD_QUALITY") {
            q += roll(s.min, s.max) as f32;
        }
    }
    q.clamp(0.0, MAX_EFFECT)
}

#[cfg(test)]
mod quality_tests {
    use barrel_formats::battle_data::{Trait, TraitSlot};

    #[test]
    fn quality_traits_add_and_cap_at_120() {
        let t = Trait { name: "Quality+".into(), group: String::new(), slots: vec![TraitSlot { kind: "ITEM_SUB_ITM_ADD_QUALITY".into(), min: 15, max: 15 }] };
        let other = Trait { name: "Power".into(), group: String::new(), slots: vec![TraitSlot { kind: "ITEM_SUB_ITM_CHANGE_POWER".into(), min: 30, max: 30 }] };
        let mut roll = |lo: i32, _: i32| lo;
        assert_eq!(super::finished_quality(50.0, &[&t, &other], &mut roll), 65.0);
        assert_eq!(super::finished_quality(110.0, &[&t], &mut roll), 120.0);
    }
}
