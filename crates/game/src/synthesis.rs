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

/// What a material brings to the cauldron: the synthesis state's slot entry (+0x418 + 40 ×
/// slot: the item, its cost points, its effect value, its element count, its element value and
/// a bonus), as read from the running game at the start of a synthesis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Which elements it has (`itemData` columns `fire`, `water`, `wind`, `soil`).
    pub elements: [bool; 4],
    /// Its element value (`itemData`'s `value`).
    pub element_value: i32,
    /// Its effect value: the item's own quality (the save's item unit +4).
    pub effect: f32,
    /// The cost points it uses up: `itemData`'s `level`.
    pub cp: i32,
}

impl Material {
    /// A material as an `itemData.xml` row describes it, its effect value the row's `effect`
    /// (an item's own quality replaces it: [`Material::with_quality`]). Missing or unreadable
    /// numbers count as 0.
    pub fn from_item(row: &barrel_formats::table::Element) -> Material {
        let num = |name: &str| row.attr(name).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
        Material {
            elements: ["fire", "water", "wind", "soil"].map(|e| num(e) > 0),
            element_value: num("value"),
            effect: num("effect") as f32,
            cp: num("level"),
        }
    }

    /// The same material with an item's own quality as its effect value.
    pub fn with_quality(self, quality: f32) -> Material {
        Material { effect: quality, ..self }
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
            uses: 1,
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
            r#"<Root><itemData level="6" effect="50" attribute="ELEMENT_FIRE" value="10" fire="1" water="0" wind="2" soil="0"/></Root>"#,
        )
        .unwrap();
        let m = Material::from_item(&root.children[0]);
        // The game's slot for a Spiky Bud (level 6) costs 6 CP.
        assert_eq!(m, Material { elements: [true, false, true, false], element_value: 10, effect: 50.0, cp: 6 });
        assert_eq!(m.with_quality(68.5).effect, 68.5);
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
        let t = Trait { name: "Quality+".into(), group: String::new(), level: 0, point: 0, kinds: [false; 9], slots: vec![TraitSlot { kind: "ITEM_SUB_ITM_ADD_QUALITY".into(), min: 15, max: 15 }] };
        let other = Trait { name: "Power".into(), group: String::new(), level: 0, point: 0, kinds: [false; 9], slots: vec![TraitSlot { kind: "ITEM_SUB_ITM_CHANGE_POWER".into(), min: 30, max: 30 }] };
        let mut roll = |lo: i32, _: i32| lo;
        assert_eq!(super::finished_quality(50.0, &[&t, &other], &mut roll), 65.0);
        assert_eq!(super::finished_quality(110.0, &[&t], &mut roll), 120.0);
    }
}

/// The sum of a kind's amounts over an item's traits (each trait's two slots), as
/// `0x14029ef10` and `0x14029f030` add them up.
fn trait_sum(traits: &[&barrel_formats::battle_data::Trait], kind: &str, roll: &mut dyn FnMut(i32, i32) -> i32) -> i32 {
    traits.iter().flat_map(|t| t.slots.iter().take(2)).filter(|s| s.kind == kind).map(|s| roll(s.min, s.max)).sum()
}

/// A made item's uses (`0x14029ef10`, the unit's +0x1e): its recipe's use count (record word
/// 8, +0x20; 0 for an item no recipe makes) plus its `ITEM_SUB_ITM_ADD_USECOUNT` traits, at
/// least 1. The uses left (+0x1c) are this plus +0x20.
pub fn uses(recipe_uses: i32, traits: &[&barrel_formats::battle_data::Trait], roll: &mut dyn FnMut(i32, i32) -> i32) -> i32 {
    (recipe_uses + trait_sum(traits, "ITEM_SUB_ITM_ADD_USECOUNT", roll)).max(1)
}

/// A made item's equip cost (`0x14029f030`, the unit's +0x18, the squares it takes on the
/// battle item grid): `itemData`'s `EquipCost` less its `ITEM_SUB_EQUIP_COST` traits and the
/// unit's +0x1a, kept within 1 and 5; 0 for an item whose `EquipCost` is 0.
pub fn equip_cost(item_cost: i32, traits: &[&barrel_formats::battle_data::Trait], less: i32, roll: &mut dyn FnMut(i32, i32) -> i32) -> i32 {
    if item_cost <= 0 {
        return 0;
    }
    (item_cost - trait_sum(traits, "ITEM_SUB_EQUIP_COST", roll) - less).clamp(1, 5)
}

#[cfg(test)]
mod count_tests {
    use barrel_formats::battle_data::{Trait, TraitSlot};

    #[test]
    fn traits_add_uses_and_take_off_cost() {
        let t = |kind: &str, v| Trait { name: String::new(), group: String::new(), level: 0, point: 0, kinds: [false; 9], slots: vec![TraitSlot { kind: kind.into(), min: v, max: v }] };
        let (more, light) = (t("ITEM_SUB_ITM_ADD_USECOUNT", 1), t("ITEM_SUB_EQUIP_COST", 2));
        let mut roll = |lo: i32, _: i32| lo;
        assert_eq!(super::uses(3, &[&more], &mut roll), 4);
        assert_eq!(super::uses(0, &[], &mut roll), 1);
        assert_eq!(super::equip_cost(3, &[&light], 0, &mut roll), 1);
        assert_eq!(super::equip_cost(3, &[&light, &light], 0, &mut roll), 1);
        assert_eq!(super::equip_cost(9, &[], 0, &mut roll), 5);
        assert_eq!(super::equip_cost(0, &[], 0, &mut roll), 0);
    }
}

/// What the finished synthesis needs besides the cauldron.
pub struct Finish<'a> {
    /// The recipe's number (its place in the recipe table) and record.
    pub recipe: (usize, &'a Recipe),
    /// The made item's `itemData` level and `EquipCost`.
    pub level: i32,
    pub equip_cost: i32,
    /// The traits the player picked (up to three `item_potential` rows) and the trait table.
    pub traits: [i16; 3],
    pub trait_table: &'a [barrel_formats::battle_data::Trait],
    /// How many items it makes.
    pub count: i32,
    /// Bonus value 3 (synthesis experience %) and the alchemy level.
    pub exp_rate: i32,
    pub alchemy_level: i32,
    /// The made item's four category numbers (`itemData`'s `category_00`..`_03`, -1 none) and
    /// the synthesis bonus totals (`bonus::State::mix`, empty for none).
    pub categories: [i32; 4],
    pub mix: &'a [i32],
    /// What the Quantity skill added to the uses (state +0x4d4) and Compression to the cost
    /// reduction (state +0x4c8).
    pub extra_uses: i32,
    pub compression: i32,
}

impl Finish<'_> {
    /// A per-category synthesis bonus summed over the item's categories (`0x14036c3d0`).
    fn per_category(&self, kind: usize) -> i32 {
        self.categories
            .iter()
            .filter(|&&c| c >= 0)
            .filter_map(|&c| crate::bonus::mix_slot(kind, c))
            .filter_map(|slot| self.mix.get(slot))
            .sum()
    }
}

/// Finishes a synthesis: the made item (its effects from the recipe's lines at the
/// attribute values, the picked traits, the quality, uses and equip cost), what it teaches
/// (the traits' and effects' knowledge to 10, the effects revealed on the recipe, the item
/// itself at 10 and the recipe raised to 6, made) and the alchemy experience. `serial` is
/// the new item's serial number.
pub fn finish(s: &Synthesis, f: &Finish, serial: u16, k: &mut barrel_formats::save::Knowledge, revealed: &mut [[bool; 10]], roll: &mut dyn FnMut(i32, i32) -> i32) -> (barrel_formats::save::ItemUnit, i32) {
    let (r, recipe) = f.recipe;
    let picked: Vec<&barrel_formats::battle_data::Trait> = f.traits.iter().filter_map(|&t| usize::try_from(t).ok().and_then(|t| f.trait_table.get(t))).collect();
    let effects = s.effects(&recipe.lines).map(|e| e.map_or(-1, |e| e as i16));
    // The result builder (`0x1402fa700`): the quality takes the PASSIVE_EFFECT_ADD bonuses of
    // the item's categories (within 0 and 120), the extra uses Quantity's and the
    // USE_ITEM_ADD bonuses', the cost reduction Compression's.
    let base = (s.effect_value() + f.per_category(13) as f32).clamp(0.0, MAX_EFFECT);
    let uses = uses(recipe.uses, &picked, roll) as i16;
    let extra = (f.extra_uses + f.per_category(12)) as i16;
    let unit = barrel_formats::save::ItemUnit {
        serial,
        item: recipe.item as i16,
        quality: finished_quality(base, &picked, roll),
        traits: f.traits,
        word_e: 0,
        effects,
        equip_cost: equip_cost(f.equip_cost, &picked, f.compression, roll) as i16,
        word_1a: f.compression as i16,
        counts: [uses + extra, uses, extra, 0],
    };
    for &t in &f.traits {
        if let Some(v) = usize::try_from(t).ok().and_then(|t| k.potential.get_mut(t)) {
            *v = (*v).max(crate::library::KNOWN);
        }
    }
    for &e in &effects {
        crate::library::reveal_effect(revealed, k, Some((r, recipe)), e.into());
    }
    let first = k.recipe.get(r).is_some_and(|&v| v < 6);
    if let Some(v) = k.recipe.get_mut(r) {
        *v = (*v).max(6);
    }
    crate::library::learn_item(k, &unit, crate::library::KNOWN, false, [-1, -1]);
    // The days less MIX_DAY_DEC, at least 1 (`0x1400fe4e0`).
    let mix_days = crate::bonus::mix_slot(11, 0).and_then(|slot| f.mix.get(slot)).copied().unwrap_or(0);
    let days = (recipe.days as i32 - mix_days).max(1);
    let exp = experience(f.level, days, f.count, first, f.exp_rate, f.alchemy_level);
    (unit, exp)
}

/// The cost points a synthesis starts with for `user` (0 Escha, 1 Logy; `0x1400fe640`):
/// the alchemy level + 10 + the `INIT_COST_ADD` synthesis bonus + the first parameter of
/// every passive of type 8 (Expert Skill, +15) the user has learnt, at least 0. `learnt` is
/// the save's `bonus/skill` flags, one per synthesis skill record.
pub fn start_cp(user: i32, alchemy_level: i32, mix: &[i32], learnt: &[bool], skills: &[barrel_formats::battle_data::MixSkill]) -> i32 {
    let bonus = crate::bonus::mix_slot(7, 0).and_then(|s| mix.get(s)).copied().unwrap_or(0);
    let expert: i32 = skills
        .iter()
        .zip(learnt)
        .filter(|(s, l)| **l && s.kind == 8 && (s.user == user || s.user == -1))
        .map(|(s, _)| s.params[0])
        .sum();
    (alchemy_level + 10 + bonus + expert).max(0)
}

#[cfg(test)]
mod start_tests {
    use barrel_formats::battle_data::MixSkill;

    #[test]
    fn starting_cp_counts_level_bonus_and_expert_skill() {
        let skill = |user, kind, p| MixSkill { name: String::new(), kind, params: [p, 0], user, level: 30, cost: [0; 4], description: String::new() };
        let skills = [skill(0, 8, 15), skill(1, 8, 15), skill(0, 22, 10)];
        let mut mix = vec![0; 0x220];
        mix[5] = 30;
        assert_eq!(super::start_cp(0, 12, &mix, &[true, true, true], &skills), 12 + 10 + 30 + 15);
        assert_eq!(super::start_cp(1, 12, &[], &[false, false, true], &skills), 22);
    }
}

/// The property points a synthesis starts with (state +0x378, set at `0x140302640` and on):
/// 1 + the first parameter of every learnt type-10 passive of the user (Inherit Apprentice,
/// Journeyman, Master: 4, 5 and 10; their descriptions say 5, 10 and 20) + the `INIT_PP_ADD`
/// synthesis bonus + the `GET_PP_ADD` bonuses of the made item's four categories.
pub fn start_pp(user: i32, mix: &[i32], learnt: &[bool], skills: &[barrel_formats::battle_data::MixSkill], categories: &[i32; 4]) -> i32 {
    let bonus = |kind, cat| crate::bonus::mix_slot(kind, cat).and_then(|s| mix.get(s)).copied().unwrap_or(0);
    let inherit: i32 = skills.iter().zip(learnt).filter(|(s, l)| **l && s.kind == 10 && (s.user == user || s.user == -1)).map(|(s, _)| s.params[0]).sum();
    1 + inherit + bonus(10, 0) + categories.iter().filter(|&&c| c >= 0).map(|&c| bonus(9, c)).sum::<i32>()
}

#[cfg(test)]
mod pp_tests {
    use barrel_formats::battle_data::MixSkill;

    #[test]
    fn starting_pp() {
        let skill = |user, p| MixSkill { name: String::new(), kind: 10, params: [p, 0], user, level: 18, cost: [0; 4], description: String::new() };
        let skills = [skill(0, 4), skill(1, 4), skill(0, 5)];
        let mut mix = vec![0; 0x220];
        mix[0x87] = 3;
        mix[7 + 2] = 3;
        assert_eq!(super::start_pp(0, &mix, &[true, true, true], &skills, &[2, -1, -1, -1]), 1 + 4 + 5 + 3 + 3);
        assert_eq!(super::start_pp(1, &[], &[false; 3], &skills, &[-1; 4]), 1);
    }
}

/// An item's price traits applied to a price (`0x1402a2ef0`): each `ITEM_SUB_ITM_ADD_PRICE`
/// slot of each trait in turn adds `trunc(price / 100 × amount)` (its upper bound), the next
/// one working from the raised price.
pub fn price_traits(mut price: i32, traits: &[&barrel_formats::battle_data::Trait]) -> i32 {
    for t in traits {
        for s in t.slots.iter().take(2).filter(|s| s.kind == "ITEM_SUB_ITM_ADD_PRICE") {
            price += (price as f32 / 100.0 * s.max as f32) as i32;
        }
    }
    price
}

/// An item's worth by its quality (`0x1402a3130`, which the item lists compare items by):
/// `trunc(cost × (quality + 100) / 200)` (`itemData`'s `cost`), then its price traits, at
/// least 1; 0 for an item with no cost.
pub fn worth(cost: i32, quality: f32, traits: &[&barrel_formats::battle_data::Trait]) -> i32 {
    if cost <= 0 {
        return 0;
    }
    price_traits(((quality + 100.0) * cost as f32 / 200.0) as i32, traits).max(1)
}

/// The figure the two table prices start from (`0x14029e030`): two thirds of the quality,
/// cut to a whole number. (The game adds each trait record's `valuePoint` and each effect
/// record's +0x24; both are 0 in every shipped record.)
pub fn price_figure(quality: f32) -> i32 {
    (quality / 3.0 * 2.0) as i32
}

/// One of the two prices reached through tables (`0x1402a2fa0`; which of buying and selling
/// it is, is still open): `2 × trunc((2q + 100) / 100) × cost + 5`, then the price traits,
/// within 10 and 9,999; 0 for an item with no cost.
pub fn table_price_a(cost: i32, quality: f32, traits: &[&barrel_formats::battle_data::Trait]) -> i32 {
    if cost <= 0 {
        return 0;
    }
    let q = price_figure(quality);
    price_traits((2 * q + 100) / 100 * cost * 2 + 5, traits).max(10).min(9999)
}

/// The other (`0x1402a3070`): `100 + trunc(2 × cost × (q + 50) / 10)`, then the price traits,
/// within 100 and 9,999; 100 for an item with no cost.
pub fn table_price_b(cost: i32, quality: f32, traits: &[&barrel_formats::battle_data::Trait]) -> i32 {
    if cost <= 0 {
        return 100;
    }
    let q = price_figure(quality);
    price_traits(100 + 2 * cost * (q + 50) / 10, traits).min(9999).max(100)
}

#[cfg(test)]
mod price_tests {
    #[test]
    fn table_prices() {
        // Craft (cost 30) at quality 50: q 33.
        assert_eq!(super::price_figure(50.0), 33);
        assert_eq!(super::table_price_a(30, 50.0, &[]), 65);
        assert_eq!(super::table_price_b(30, 50.0, &[]), 598);
        assert_eq!((super::table_price_a(0, 50.0, &[]), super::table_price_b(0, 50.0, &[])), (0, 100));
        assert_eq!(super::table_price_b(999, 120.0, &[]), 9999);
    }

    use barrel_formats::battle_data::{Trait, TraitSlot};

    #[test]
    fn price_traits_compound() {
        let t = Trait { name: "High Price".into(), group: String::new(), level: 0, point: 0, kinds: [false; 9], slots: vec![TraitSlot { kind: "ITEM_SUB_ITM_ADD_PRICE".into(), min: 50, max: 50 }] };
        assert_eq!(super::price_traits(100, &[&t, &t]), 225);
        // Quality 50, cost 46: 46 × 150 / 200 = 34.5 → 34.
        assert_eq!(super::worth(46, 50.0, &[]), 34);
        assert_eq!(super::worth(46, 50.0, &[&t]), 51);
        assert_eq!(super::worth(0, 50.0, &[]), 0);
    }
}
