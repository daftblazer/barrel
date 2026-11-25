//! Synthesis at the cauldron: what a recipe asks for, which materials the Add Materials screen takes when the player takes the
//! first one listed, what comes out, and the random draws the game makes around it. Docs: docs/ryza/open.md, "Synthesis (P6)".
//!
//! The recipe table is `saves/item/itemrecipedata.xml`: a head row per recipe (`ItemTag`, `MakeNum` units made, `Hour` the clock
//! moves on, `NeedNum` the units its first material takes, `MatTag` that material's category, `AddEff*` the effects its loop
//! unlocks) followed by the rows of the rest of the loops (a `MatTag` and `NeedNum` of their own, or only effects). The recipe
//! numbers the save's `knowledge/recipe` uses are the head rows' order.
//!
//! The game's random draws around a synthesis are not the synthesis's own: the result is not random. The 48 draws of the effect
//! (`0x1401e2fad`) and a restocking pass over the world when the hours go by (`0x14059f9b0` to `0x14059fdaf`: the per-group
//! shuffles and the random potentials the stock gets, `0x14059f210`) and the voice picks (`0x1402de19b`) are what the key's
//! 3500 to 3536 draws are; [`Step`] lists them.

use barrel_formats::table;

use crate::random::Random;
use crate::save::Worn;

/// One row of a recipe: a material it takes and the effects that loop unlocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// The category of material the row takes (`ITEM_CATEGORY_*`); none for a row of effects only.
    pub category: Option<String>,
    /// How many units of it (`NeedNum`; 1 when the row names none).
    pub need: u32,
    /// The effects its loop unlocks (`AddEff0`, `AddEff1`, ...).
    pub effects: Vec<String>,
}

/// One recipe of `itemrecipedata.xml`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recipe {
    /// The item it makes (`ItemTag`: `ITEM_MIX_*`).
    pub tag: String,
    /// Units made at once.
    pub make: u32,
    /// Hours it takes.
    pub hours: u32,
    /// The head row's and the loops' rows.
    pub rows: Vec<Row>,
}

/// The recipe table and the items' categories.
#[derive(Clone, Debug, Default)]
pub struct Tables {
    pub recipes: Vec<Recipe>,
    /// For each item of `itemdata.xml`, its categories (`cat_0`, `cat_1`, ...).
    categories: Vec<Vec<String>>,
}

impl Tables {
    /// Reads `itemrecipedata.xml` and `itemdata.xml`.
    pub fn read(recipes: &[u8], items: &[u8]) -> Result<Tables, String> {
        let root = table::parse(recipes).map_err(|e| format!("itemrecipedata: {e}"))?;
        let mut out: Vec<Recipe> = Vec::new();
        for e in &root.children {
            let n = |k: &str, or: u32| e.attr(k).and_then(|t| t.trim().parse().ok()).unwrap_or(or);
            let row = Row {
                category: e.attr("MatTag").map(str::to_string),
                need: n("NeedNum", 1),
                effects: (0..).map_while(|i| e.attr(&format!("AddEff{i}"))).map(str::to_string).collect(),
            };
            match e.attr("ItemTag") {
                Some(tag) => out.push(Recipe { tag: tag.to_string(), make: n("MakeNum", 1), hours: n("Hour", 0), rows: vec![row] }),
                None => out.last_mut().ok_or("itemrecipedata: a loop row before any recipe")?.rows.push(row),
            }
        }
        let items = table::parse(items).map_err(|e| format!("itemdata: {e}"))?;
        let categories = items.children.iter().map(|e| (0..).map_while(|i| e.attr(&format!("cat_{i}"))).map(str::to_string).collect()).collect();
        Ok(Tables { recipes: out, categories })
    }

    /// Whether item `item` is in `category`.
    pub fn in_category(&self, item: usize, category: &str) -> bool {
        self.categories.get(item).is_some_and(|c| c.iter().any(|c| c == category))
    }
}

/// What a synthesis takes and gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    /// The container places (indexes into the filled places, in order) of the units taken, in the order the rows took them.
    pub taken: Vec<usize>,
    /// Units made.
    pub count: u32,
    /// The hours the clock moves on.
    pub hours: u32,
}

/// The container's units a synthesis of `recipe` takes when each row takes the first unit listed of its category (the screen's list is the
/// container's own order: the capture's lists at `synth-atelier` 3050 and 3300 fit it), or `None` when a row finds too few.
pub fn make(tables: &Tables, recipe: &Recipe, container: &[Worn]) -> Option<Made> {
    let mut taken: Vec<usize> = Vec::new();
    for row in &recipe.rows {
        let Some(category) = &row.category else { continue };
        for _ in 0..row.need {
            let at = (0..container.len()).find(|i| !taken.contains(i) && tables.in_category(usize::from(container[*i].item), category))?;
            taken.push(at);
        }
    }
    Some(Made { taken, count: recipe.make, hours: recipe.hours })
}

/// The container after a synthesis: the units taken go, the made ones come (the caller builds them: their quality and effects are
/// the result's, which is not derived yet).
pub fn after(container: &[Worn], made: &Made) -> Vec<Worn> {
    container.iter().enumerate().filter(|(i, _)| !made.taken.contains(i)).map(|(_, w)| *w).collect()
}

/// The clock word's step for an hour (`save::hour_of`: 50 to the hour, fitted to the capture's 493 at 10 AM; a synthesis of 14 hours from it
/// reads "AM 0" on the room's clock).
pub const CLOCK_PER_HOUR: u32 = 50;

/// The clock word after `hours` have gone by.
pub fn clock_after(word: u32, hours: u32) -> u32 {
    word + hours * CLOCK_PER_HOUR
}

/// The save's alchemy level and experience (`mix_lv`: `m_lv` and `m_exp`, one big-endian i32 each).
pub fn alchemy(state: &[u8]) -> Option<(u32, u32)> {
    let word = |name: &str| -> Option<u32> { crate::save::find(state, &["mix_lv", name])?.get(..4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])) };
    Some((word("m_lv")?, word("m_exp")?))
}

/// One of the game's draws a run makes, as a script names it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// `irand(a, b)`.
    Int(i32, i32),
    /// `frand(a, b)`.
    Float(f32, f32),
    /// `one_to(n)` (`r640`).
    OneTo(i32),
    /// `below(n)` (`r680`).
    Below(i32),
    /// A bare draw whose wrapper's arguments the hooks do not give: `around` (`0x1406ae2e0`, a float between two bounds: 2305, 2501
    /// ... of the key) and `chance` (`0x1406ae500`, a percent: her idle sound at 4360 ...). Only the generator's step matters.
    Raw,
}

impl Step {
    /// Makes the draw.
    pub fn draw(&self, rng: &mut Random) {
        match *self {
            Step::Int(a, b) => {
                rng.irand(a, b);
            }
            Step::Float(a, b) => {
                rng.frand(a, b);
            }
            Step::OneTo(n) => {
                rng.one_to(n);
            }
            Step::Below(n) => {
                rng.below(n);
            }
            Step::Raw => {
                rng.rand15();
            }
        }
    }

    /// A step from a script's words: `irand A B` (hex), `frand A B`, `r640 N` (hex), `r680 N` (hex), the hook log's own spelling.
    pub fn parse(words: &[&str]) -> Result<Step, String> {
        let hex = |t: &str| i32::from_str_radix(t, 16).map_err(|e| format!("{t}: {e}"));
        let float = |t: &str| t.parse::<f32>().map_err(|e| format!("{t}: {e}"));
        match words {
            ["irand", a, b] => Ok(Step::Int(hex(a)?, hex(b)?)),
            ["frand", a, b] => Ok(Step::Float(float(a)?, float(b)?)),
            ["r640", n] => Ok(Step::OneTo(hex(n)?)),
            ["r680", n] => Ok(Step::Below(hex(n)?)),
            ["rand15"] => Ok(Step::Raw),
            _ => Err(format!("{}?", words.join(" "))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPES: &str = r#"<Root>
<itemRecipeData ItemTag="ITEM_MIX_A" MakeNum="4" Hour="14" NeedNum="2" MatTag="ITEM_CATEGORY_FLOWER" AddEff0="E1" AddEff1="E2"/>
<itemRecipeData MatTag="ITEM_CATEGORY_LIQUID" AddEff0="E3"/>
<itemRecipeData AddEff0="E4"/>
<itemRecipeData ItemTag="ITEM_MIX_B" MakeNum="1" Hour="7"/>
</Root>"#;
    const ITEMS: &str = r#"<Root>
<itemData cat_0="ITEM_CATEGORY_LIQUID"/>
<itemData cat_0="ITEM_CATEGORY_FLOWER" cat_1="ITEM_CATEGORY_MEDICAL"/>
<itemData cat_0="ITEM_CATEGORY_FLOWER"/>
<itemData cat_0="ITEM_CATEGORY_FLOWER"/>
</Root>"#;

    fn unit(item: u16) -> Worn {
        Worn { item, ..Default::default() }
    }

    #[test]
    fn rows_follow_their_head() {
        let t = Tables::read(RECIPES.as_bytes(), ITEMS.as_bytes()).unwrap();
        assert_eq!(t.recipes.len(), 2);
        let a = &t.recipes[0];
        assert_eq!((a.make, a.hours, a.rows.len()), (4, 14, 3));
        assert_eq!(a.rows[0], Row { category: Some("ITEM_CATEGORY_FLOWER".into()), need: 2, effects: vec!["E1".into(), "E2".into()] });
        assert_eq!(a.rows[1].need, 1);
        assert_eq!(a.rows[2].category, None);
        assert_eq!(t.recipes[1].hours, 7);
    }

    #[test]
    fn each_row_takes_the_first_units_listed() {
        let t = Tables::read(RECIPES.as_bytes(), ITEMS.as_bytes()).unwrap();
        let container = [unit(2), unit(0), unit(1), unit(3)];
        let made = make(&t, &t.recipes[0], &container).unwrap();
        // Two flowers (the first listed: places 0 and 2), then the water (place 1).
        assert_eq!(made.taken, [0, 2, 1]);
        assert_eq!((made.count, made.hours), (4, 14));
        let left = after(&container, &made);
        assert_eq!(left.iter().map(|w| w.item).collect::<Vec<_>>(), [3]);
        assert!(make(&t, &t.recipes[0], &container[..2]).is_none(), "too few flowers");
    }

    #[test]
    fn the_clock_moves_by_the_hours() {
        use crate::save::hour_of;
        // The capture's autosave: 493 reads 10 AM; the synthesis's 14 hours later the clock reads 0.
        assert_eq!(hour_of(493), 10);
        assert_eq!(hour_of(clock_after(493, 14)), 0);
    }

    #[test]
    fn steps_read_the_hook_logs_words() {
        assert_eq!(Step::parse(&["irand", "2", "4"]), Ok(Step::Int(2, 4)));
        assert_eq!(Step::parse(&["frand", "0.05", "0.2"]), Ok(Step::Float(0.05, 0.2)));
        assert_eq!(Step::parse(&["r640", "14"]), Ok(Step::OneTo(20)));
        assert_eq!(Step::parse(&["r680", "d"]), Ok(Step::Below(13)));
        assert_eq!(Step::parse(&["rand15"]), Ok(Step::Raw));
        assert!(Step::parse(&["shufA", "0"]).is_err());
        let mut r = Random::after(1, 0);
        r.record();
        Step::OneTo(20).draw(&mut r);
        Step::Below(13).draw(&mut r);
        let lines: Vec<String> = r.draws().iter().map(|d| d.to_string().split(" @").next().unwrap().to_string()).collect();
        assert_eq!(lines, ["r640 14", "r680 d"]);
    }
}
