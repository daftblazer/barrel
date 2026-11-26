//! The synthesis rules: the ring graph of a recipe (`saves/mix/mixfielddata.xml`), the materials placed in its rings, which rings are open,
//! what the rings give (effect tiers, quality bonus, ...) and the made item's quality. Docs: docs/ryza/open.md "Synthesis (P6)" part 3,
//! docs/tables.md "mixfielddata", the addresses in docs/engine.md.
//!
//! The data is read by tag (`ITEM_MIX_*`) and can grow: a mod's file adds or replaces recipes' graphs ([`Rings::extend`]).
//!
//! A ring has an element (fire 0, ice 1, thunder 2, air 3), a `type` (what its steps give), the recipe row whose materials it takes
//! (`restrict`), an optional link (`Connect`: it opens once another ring holds `val` of an element) and `Param` steps (the highest step
//! whose threshold the ring's element value reaches gives its value). The state the game keeps per ring is `(value, lit)`: a lit ring's
//! value is the amount of its element placed in it; an unlit ring's is how far its link is from opening, as a number below zero.

use barrel_formats::table;

/// How many rings a recipe's field has (the data lists 16 `<Ring>` each; the empty ones have no element).
pub const RINGS: usize = 16;
/// The element names' order (`elemFire`, `elemIce`, `elemThunder`, `elemAir` of `itemdata.xml`).
pub const ELEMENTS: usize = 4;

/// One `Param` step of a ring: the value it gives when the ring's element value reaches `at`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// `vN`: a number, or None when the data names a tag (the unlock a type-6 ring gives).
    pub value: Option<i32>,
    /// `vN` as a tag (a recipe or an event the ring unlocks), for the rings of type 6.
    pub tag: Option<String>,
    /// `eN`: the element value the step needs.
    pub at: i32,
}

/// A ring's link to the ring that opens it (`Connect`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    /// The ring it hangs on (`idx`); none for the root.
    pub from: Option<usize>,
    /// The element value that ring must hold (`val`, 0 when the data has none).
    pub need: i32,
    /// The element of that ring counted (`elem`).
    pub elem: usize,
    /// The direction the link leaves its ring by (`dir`: 0 to 5), for the drawing.
    pub dir: Option<u8>,
}

/// One ring of a recipe's field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ring {
    /// Its element, or none for an empty slot of the 16.
    pub elem: Option<usize>,
    /// What its steps give: 0 to 3 an effect slot's tier, 4 the quality bonus, 5 and the rest as [`Totals`] says.
    pub kind: u32,
    /// The recipe row whose materials it takes (checked against the key: ring 0 takes the flowers of row 0, the ice rings the liquid of row 1).
    pub restrict: u32,
    /// A material the ring is for (`ex_material`: an item tag). Not read by the rules yet.
    pub ex_material: Option<String>,
    /// The story event that shows the ring (`ev_start`). Not read by the rules yet.
    pub ev_start: Option<String>,
    /// Its place on the graph's grid (`x`, `y`, 0 when absent).
    pub at: [i32; 2],
    /// The six neighbours by direction (`Child indexes`, -1 for none).
    pub children: [i32; 6],
    pub link: Option<Link>,
    pub steps: Vec<Step>,
}

impl Ring {
    /// The step the element value `value` reaches: the highest whose threshold it meets.
    pub fn reached(&self, value: i32) -> Option<&Step> {
        self.steps.iter().rev().find(|s| value >= s.at)
    }
}

/// The recipes' fields by tag.
#[derive(Clone, Debug, Default)]
pub struct Rings {
    fields: Vec<(String, Vec<Ring>)>,
}

fn number(e: &table::Element, name: &str) -> Option<i32> {
    e.attr(name).and_then(|t| t.trim().parse().ok())
}

impl Rings {
    /// Reads a `mixfielddata.xml`.
    pub fn read(bytes: &[u8]) -> Result<Rings, String> {
        let root = table::parse(bytes).map_err(|e| format!("mixfielddata: {e}"))?;
        let mut out = Rings::default();
        for field in root.children.iter().filter(|f| f.name == "FieldData") {
            let tag = field.attr("tag").ok_or("mixfielddata: a FieldData without a tag")?;
            let rings = field.children.iter().filter(|r| r.name == "Ring").map(read_ring).collect::<Result<Vec<_>, _>>().map_err(|e| format!("{tag}: {e}"))?;
            out.fields.push((tag.to_string(), rings));
        }
        Ok(out)
    }

    /// Adds `other`'s recipes; a tag already here takes the new graph (a mod's file replaces a recipe's).
    pub fn extend(&mut self, other: Rings) {
        for (tag, rings) in other.fields {
            match self.fields.iter_mut().find(|(t, _)| *t == tag) {
                Some(f) => f.1 = rings,
                None => self.fields.push((tag, rings)),
            }
        }
    }

    /// The rings of the recipe that makes `tag`.
    pub fn get(&self, tag: &str) -> Option<&[Ring]> {
        self.fields.iter().find(|(t, _)| t == tag).map(|(_, r)| &r[..])
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

fn read_ring(r: &table::Element) -> Result<Ring, String> {
    let mut ring = Ring {
        elem: number(r, "elem").map(|e| e as usize),
        kind: number(r, "type").unwrap_or(0) as u32,
        restrict: number(r, "restrict").unwrap_or(0) as u32,
        ex_material: r.attr("ex_material").map(str::to_string),
        ev_start: r.attr("ev_start").map(str::to_string),
        at: [number(r, "x").unwrap_or(0), number(r, "y").unwrap_or(0)],
        children: [-1; 6],
        link: None,
        steps: Vec::new(),
    };
    for c in &r.children {
        match c.name.as_str() {
            "Child" => {
                for (i, t) in c.attr("indexes").unwrap_or_default().split(',').filter(|t| !t.trim().is_empty()).take(6).enumerate() {
                    ring.children[i] = t.trim().parse().map_err(|_| format!("Child indexes {t}?"))?;
                }
            }
            "Connect" => {
                ring.link = Some(Link {
                    from: number(c, "idx").map(|i| i as usize).filter(|&i| i < RINGS),
                    need: number(c, "val").unwrap_or(0),
                    elem: number(c, "elem").unwrap_or(0) as usize % ELEMENTS,
                    dir: number(c, "dir").map(|d| d as u8),
                });
            }
            "Param" => {
                for i in 0.. {
                    let Some(v) = c.attr(&format!("v{i}")) else { break };
                    let at = number(c, &format!("e{i}")).ok_or_else(|| format!("Param v{i} without e{i}"))?;
                    let n = v.trim().parse().ok();
                    ring.steps.push(Step { value: n, tag: if n.is_none() { Some(v.to_string()) } else { None }, at });
                }
            }
            _ => {}
        }
    }
    Ok(ring)
}

/// A material placed in a ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The ring it went into.
    pub ring: usize,
    /// The material's quality.
    pub quality: f32,
    /// Its element values (fire, ice, thunder, air).
    pub elements: [i32; ELEMENTS],
}

/// What the lit rings give together (`0x1402caa50`'s totals, as far as the key shows them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Totals {
    /// Words 0 to 15 of the game's totals: word 0 the quality bonus (type 4), word 1 the sum of the type-5 steps, words 6 and 7 the types 7 and 8,
    /// word `type - 1` for the types 9 and 11 to 16 (the game's jump table, unchecked beyond the key's zeros).
    pub words: [i32; 16],
    /// The effect slots' tiers (types 0 to 3): the highest tier a lit ring reaches, -1 for none.
    pub tiers: [i32; 4],
    /// The recipes and events the type-6 rings reached unlock (tags, see [`Step::tag`]); the game keeps their largest id, which is not read.
    pub unlocks: Vec<String>,
}

impl Totals {
    fn none() -> Totals {
        Totals { words: [0; 16], tiers: [-1; 4], unlocks: Vec::new() }
    }

    /// The quality bonus the rings give.
    pub fn bonus(&self) -> i32 {
        self.words[0]
    }

    fn add(&mut self, ring: &Ring, step: &Step) {
        let v = step.value.unwrap_or(0);
        match ring.kind {
            0..=3 => self.tiers[ring.kind as usize] = self.tiers[ring.kind as usize].max(v),
            4 => self.words[0] += v,
            5 => self.words[1] += v,
            6 => self.unlocks.extend(step.tag.clone()),
            10 => {}
            k @ 7..=16 => self.words[k as usize - 1] += v,
            _ => {}
        }
    }
}

/// The rings' state for the placed materials: `(value, lit)` per ring (the game's 32 words), and what the lit rings give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eval {
    pub cells: [(i32, bool); RINGS],
    pub totals: Totals,
}

impl Eval {
    /// The game's 32 state words.
    pub fn words(&self) -> Vec<i32> {
        self.cells.iter().flat_map(|&(v, l)| [v, i32::from(l)]).collect()
    }
}

/// The element values the placed materials give each ring.
fn held(rings: &[Ring], placed: &[Placed]) -> [[i32; ELEMENTS]; RINGS] {
    let mut v = [[0; ELEMENTS]; RINGS];
    for p in placed {
        if let (Some(r), true) = (rings.get(p.ring), p.ring < RINGS) {
            if let Some(e) = r.elem {
                v[p.ring][e % ELEMENTS] += p.elements[e % ELEMENTS];
            }
        }
    }
    v
}

/// Which rings are open: a root ring (no link) and a ring whose link's ring is open and holds its element value.
fn open(rings: &[Ring], held: &[[i32; ELEMENTS]; RINGS]) -> [bool; RINGS] {
    let mut lit = [false; RINGS];
    for _ in 0..RINGS {
        for (i, r) in rings.iter().enumerate().take(RINGS) {
            if r.elem.is_none() {
                continue;
            }
            lit[i] = match r.link {
                None | Some(Link { from: None, .. }) => true,
                Some(l) => l.from.is_some_and(|s| lit[s] && held[s][l.elem] >= l.need),
            };
        }
    }
    lit
}

/// Evaluates a field with `placed` in it.
pub fn evaluate(rings: &[Ring], placed: &[Placed]) -> Eval {
    let held = held(rings, placed);
    let lit = open(rings, &held);
    let mut cells = [(0, false); RINGS];
    let mut totals = Totals::none();
    for (i, r) in rings.iter().enumerate().take(RINGS) {
        let Some(e) = r.elem else { continue };
        if lit[i] {
            let value = held[i][e % ELEMENTS];
            cells[i] = (value, true);
            if let Some(step) = r.reached(value) {
                totals.add(r, step);
            }
        } else if let Some(Link { from: Some(s), need, elem, .. }) = r.link {
            // How far the link is from opening: its ring's value of the element (0 when that ring is shut) less what it needs.
            let have = if lit[s] { held[s][elem] } else { 0 };
            cells[i] = ((have - need).min(0), false);
        }
    }
    Eval { cells, totals }
}

/// The product's numbers for the materials placed: what `0x1402cbb50` recomputes on every change.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub count: u32,
    /// The materials' qualities added as floats.
    pub sum: f32,
    /// The mean plus the materials' `0x157` bonus (not read: 0).
    pub mean: f32,
    /// `mean` plus the rings' quality bonus less the state's `B`, clamped to 0 to 999.
    pub clamped: f32,
    /// The made quality.
    pub quality: u32,
    pub eval: Eval,
}

/// The made quality: `trunc(min(999, max(0, sum / count + bonus + (a - b))))`; returns the mean plus bonus, the clamped value and the
/// quality. A cauldron with no material makes 0.
pub fn quality(sum: f32, count: u32, bonus: f32, a: i32, b: i32) -> (f32, f32, u32) {
    let mean = (if count == 0 { 0.0 } else { sum / count as f32 }) + bonus;
    let value = (mean + (a - b) as f32).clamp(0.0, 999.0);
    (mean, value, value as u32)
}

/// A cauldron's field with its materials.
pub struct Cauldron<'a> {
    rings: &'a [Ring],
    placed: Vec<Placed>,
}

impl<'a> Cauldron<'a> {
    pub fn new(rings: &'a [Ring]) -> Cauldron<'a> {
        Cauldron { rings, placed: Vec::new() }
    }

    pub fn placed(&self) -> &[Placed] {
        &self.placed
    }

    /// Places a material of recipe row `row` into ring `ring`: the ring must be open and take that row.
    pub fn place(&mut self, ring: usize, row: u32, quality: f32, elements: [i32; ELEMENTS]) -> Result<(), String> {
        let r = self.rings.get(ring).filter(|r| r.elem.is_some()).ok_or_else(|| format!("no ring {ring}"))?;
        if r.restrict != row {
            return Err(format!("ring {ring} takes row {}, not {row}", r.restrict));
        }
        if !evaluate(self.rings, &self.placed).cells[ring].1 {
            return Err(format!("ring {ring} is not open"));
        }
        self.placed.push(Placed { ring, quality, elements });
        Ok(())
    }

    /// The numbers now. With `blank` the rings' state is the game's empty one (every ring shut, as it is for a frame when a loop's press
    /// swaps the state; the materials still count).
    pub fn outcome(&self, blank: bool) -> Outcome {
        let eval = if blank { Eval { cells: [(0, false); RINGS], totals: Totals::none() } } else { evaluate(self.rings, &self.placed) };
        let count = self.placed.len() as u32;
        // STAND-IN: the materials' potential values of kinds 0x157 and 0x15b (0 in the key) and the state's B (0) are not read.
        let sum = self.placed.iter().fold(0.0f32, |s, p| s + p.quality);
        let (mean, clamped, quality) = quality(sum, count, 0.0, eval.totals.bonus(), 0);
        Outcome { count, sum, mean, clamped, quality, eval }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recipe 92's field, cut down: ring 0 (fire, effect slot 0, no link), ring 1 (ice, quality bonus, opens at fire 2 in ring 0), ring 2 (ice, slot 1,
    /// opens at ice 2 in ring 1).
    const FIELD: &str = r#"<Root><FieldData tag="ITEM_MIX_T">
<Ring elem="0" type="0" restrict="0"><Param v0="0" v1="1" v2="2" v3="3" e0="2" e1="3" e2="4" e3="5"/></Ring>
<Ring elem="1" type="4" restrict="1" y="2"><Child indexes="-1,-1,2,-1,-1,-1,"/><Connect idx="0" val="2" elem="0"/><Param v0="10" v1="15" e0="1" e1="2"/></Ring>
<Ring elem="1" type="1" restrict="1"><Connect idx="1" val="2" elem="1" dir="5"/><Param v0="0" e0="3"/></Ring>
<Ring/>
</FieldData></Root>"#;

    fn field() -> Vec<Ring> {
        let r = Rings::read(FIELD.as_bytes()).unwrap();
        r.get("ITEM_MIX_T").unwrap().to_vec()
    }

    const FIRE: [i32; 4] = [1, 0, 0, 0];
    const ICE: [i32; 4] = [0, 1, 0, 0];

    #[test]
    fn the_field_reads_its_rings() {
        let f = field();
        assert_eq!(f.len(), 4);
        assert_eq!(f[1].children, [-1, -1, 2, -1, -1, -1]);
        assert_eq!(f[1].link, Some(Link { from: Some(0), need: 2, elem: 0, dir: None }));
        assert_eq!(f[2].link.unwrap().dir, Some(5));
        assert_eq!(f[3].elem, None);
        assert_eq!(f[0].reached(1), None);
        assert_eq!(f[0].reached(4).unwrap().value, Some(2));
    }

    #[test]
    fn rings_open_by_their_link_and_show_how_far_they_are() {
        let f = field();
        let mut c = Cauldron::new(&f);
        assert_eq!(c.outcome(false).eval.cells[..3], [(0, true), (-2, false), (-2, false)]);
        c.place(0, 0, 35.0, FIRE).unwrap();
        assert_eq!(c.outcome(false).eval.cells[1], (-1, false));
        assert!(c.place(1, 1, 40.0, ICE).is_err(), "still shut");
        c.place(0, 0, 42.0, FIRE).unwrap();
        assert_eq!(c.outcome(false).eval.cells[..3], [(2, true), (0, true), (-2, false)]);
        assert!(c.place(1, 0, 40.0, ICE).is_err(), "the ring takes row 1's materials");
    }

    #[test]
    fn steps_apply_by_the_rings_type() {
        let f = field();
        let mut c = Cauldron::new(&f);
        c.place(0, 0, 35.0, FIRE).unwrap();
        assert_eq!(c.outcome(false).eval.totals.tiers, [-1; 4]);
        c.place(0, 0, 42.0, FIRE).unwrap();
        assert_eq!(c.outcome(false).eval.totals.tiers, [0, -1, -1, -1]);
        c.place(1, 1, 40.0, ICE).unwrap();
        let o = c.outcome(false);
        assert_eq!(o.eval.totals.bonus(), 10);
        // (35 + 42 + 40) / 3 = 39, plus the ring's 10.
        assert_eq!((o.count, o.sum, o.mean, o.quality), (3, 117.0, 39.0, 49));
        let blank = c.outcome(true);
        assert_eq!((blank.quality, blank.eval.totals.tiers), (39, [-1; 4]));
    }

    #[test]
    fn the_quality_formula_truncates_and_clamps() {
        assert_eq!(quality(77.0, 2, 0.0, 0, 0), (38.5, 38.5, 38));
        assert_eq!(quality(0.0, 0, 0.0, 0, 0), (0.0, 0.0, 0));
        assert_eq!(quality(990.0, 1, 5.0, 100, 0).2, 999);
        assert_eq!(quality(10.0, 1, 0.0, 0, 30).2, 0);
        assert_eq!(quality(40.0, 1, 2.0, 10, 3).2, 49);
    }
}
