//! Synthesis: the cauldron's state as materials go in and skills are used, following the
//! rules read from Escha & Logy DX's executable (`docs/tables.md`, "Synthesis skills").
//!
//! The four elements are, in order, Fire, Water, Wind and Earth.

use barrel_formats::recipe::{Line, effect_at};

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
        }
    }
}

/// The skills whose effect on the state is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Skill {
    /// Type 0: the element gains this many points.
    Conversion { points: i32 },
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
}

impl Synthesis {
    pub fn new() -> Synthesis {
        Synthesis::default()
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

    /// Put a material in: each element it has gains its element value.
    pub fn add(&mut self, m: &Material) {
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

    const FIRE_WATER: Material = Material { elements: [true, true, false, false], element_value: 10, effect: 50.0 };
    const EARTH: Material = Material { elements: [false, false, false, true], element_value: 15, effect: 60.0 };

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
    fn from_item_row() {
        let root = barrel_formats::table::parse_str(
            r#"<Root><itemData effect="50" attribute="ELEMENT_FIRE" value="10" fire="1" water="0" wind="2" soil="0"/></Root>"#,
        )
        .unwrap();
        let m = Material::from_item(&root.children[0]);
        assert_eq!(m, Material { elements: [true, false, true, false], element_value: 10, effect: 50.0 });
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
    fn skills() {
        let mut s = Synthesis::new();
        s.add(&Material { elements: [true; 4], element_value: 30, effect: 100.0 });
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
