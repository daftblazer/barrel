//! The field's gathering points: which one is in reach, and what A gives there (the rules and
//! tables are `barrel_ryza::gather`; the popups are the HUD's). Docs: docs/ryza/open.md,
//! "Gathering (P6)".

use barrel_ryza::field_hud::Found;
use barrel_ryza::gather::{Boost, Context, Gathered, Tables};
use barrel_ryza::gimmick::{self, Spot};
use barrel_ryza::install::Install;
use barrel_ryza::random::Random;
use barrel_ryza::story::Story;

/// The gimmick types that gather by hand: the point is the place (`ITEM_D_MULTIPLE`); the ones
/// that need a tool (`_WAND_ACTION`) and the indirect ones are left out.
const HAND_POINT: &str = "ITEM_D_MULTIPLE";
/// How much of a facing a point has to be in front of her to be in reach (the cosine of the angle
/// between her facing and the way to the point).
// STAND-IN: between the capture's two frames: 0.65 where the prompt is not up yet (`gather-rasen`
// 1994) and 0.70 where it still is (2010); the game's own test is not read.
const FACING: f32 = 0.67;
/// The reach of a point, in game units, from its `formParam` (metres).
const METRE: f32 = 100.0;

/// The field's gathering points and the tables that say what they give.
pub(super) struct Points {
    tables: Tables,
    spots: Vec<Spot>,
}

impl Points {
    /// The points the story has opened in `field`; `None` without the tables.
    pub(super) fn read(install: &Install, exe: &[u8], field: &str, story: &Story) -> Option<Points> {
        let tables = Tables::read(exe, &install.read("\\saves\\item\\itemdata.xml").ok()?, &install.read("\\saves\\item\\item_potential.xml").ok()?)
            .map_err(|e| eprintln!("gathering: {e}"))
            .ok()?;
        let table = install.read(&format!("\\saves\\fieldmap\\gimmick\\gimmick_fieldmap_{}.xml", field.to_ascii_lowercase())).ok()?;
        let spots = gimmick::spots(&table).map_err(|e| eprintln!("gathering: {e}")).ok()?;
        let spots = spots.into_iter().filter(|s| s.kind == HAND_POINT && story.within(&s.story, false) && s.values.iter().any(|v| tables.place(v).is_some())).collect();
        Some(Points { tables, spots })
    }

    /// The point nearest her within its reach and in front of her, for her feet `at` (game units) and her turn
    /// in degrees (0 toward +z); `faced` false takes the nearest whichever way she faces.
    pub(super) fn in_reach(&self, at: [f32; 3], turn: f32, faced: bool) -> Option<&Spot> {
        let (s, c) = turn.to_radians().sin_cos();
        self.spots
            .iter()
            .filter_map(|p| {
                let (dx, dz) = (p.position[0] - at[0], p.position[2] - at[2]);
                let far = dx.hypot(dz);
                let ahead = far < 1e-3 || (dx * s + dz * c) / far >= FACING;
                (far <= p.reach.max(1.0) * METRE && (ahead || !faced)).then_some((far, p))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
    }

    /// The items a gathering at `spot` gives from the generator `rng`.
    pub(super) fn gather(&self, spot: &Spot, rng: &mut Random, knowledge: &[i32], events: &[usize]) -> Vec<Gathered> {
        let Some(place) = spot.values.iter().find_map(|v| self.tables.place(v)) else { return Vec::new() };
        let flag = |f: i32| usize::try_from(f).is_ok_and(|f| events.contains(&f));
        // The difficulty is Normal and the rank the story's first; the party has no effects to add (`barrel_ryza::gather::Context`).
        let ctx = Context { difficulty: 1, rank: 0, knowledge, flag: &flag, more_items: 0, boost: Boost::default() };
        self.tables.gather(&ctx, rng, place)
    }

    /// An item's picture number (the small and large pictures' places in their files).
    pub(super) fn image(&self, item: i32) -> Option<usize> {
        self.tables.image(item)
    }
}

/// A gathering the HUD replays against a capture (`--gather NEAR:PRESS`): the frames the prompt came up and A was pressed, and
/// what came of it.
pub(super) struct Plan {
    pub near: f32,
    pub press: f32,
    /// The items' names (string numbers) and whether the library did not know each.
    pub found: Vec<Found>,
    /// The items' picture numbers.
    pub images: Vec<usize>,
    /// The items the gathering gave, each with its quality and potentials, for the log.
    pub gave: Vec<Gathered>,
}

impl Points {
    /// The gathering at the point nearest `at`, from the generator `rng`: what the first item looks
    /// like in the HUD. `None` where no point is in reach or it gives nothing.
    pub(super) fn plan(&self, at: [f32; 3], turn: f32, frames: (f32, f32), rng: &mut Random, knowledge: &mut Vec<i32>, events: &[usize]) -> Option<Plan> {
        let spot = self.in_reach(at, turn, false)?;
        self.plan_at(spot, frames, rng, knowledge, events)
    }

    /// The gathering at `spot`.
    fn plan_at(&self, spot: &Spot, frames: (f32, f32), rng: &mut Random, knowledge: &mut Vec<i32>, events: &[usize]) -> Option<Plan> {
        let gave = self.gather(spot, rng, knowledge, events);
        barrel_ryza::gather::learn(knowledge, &gave);
        let found = gave.iter().map(|g| Found { name: barrel_ryza::item_info::NAME_BASE + g.item as u32, new: g.new }).collect();
        let images = gave.iter().map(|g| self.image(g.item)).collect::<Option<Vec<_>>>().filter(|i| !i.is_empty())?;
        Some(Plan { near: frames.0, press: frames.1, found, images, gave })
    }
}

/// The frames after A the popups are over in (the discovery log's fade-out ends 328 frames after it).
const POPUPS_OVER: f32 = 330.0;

/// The gathering in a window: whether a point is in reach and faced, and the one begun by A.
#[derive(Default)]
pub(super) struct Live {
    /// The tick the point came in reach (the prompt began to slide in).
    near: Option<f32>,
    plan: Option<Plan>,
    /// The points gathered from (they give nothing more: `gather-rasen` 2200 and 2360 got nothing).
    spent: Vec<u32>,
    rng: Option<Random>,
}

impl Live {
    /// One frame at `tick` (capture-style frames): her feet `at`, her turn, and whether A went down. Returns
    /// the picture numbers of the items gathered when this frame's A gathered some.
    pub(super) fn step(&mut self, points: &Points, at: [f32; 3], turn: f32, tick: f32, confirm: bool, knowledge: &mut Vec<i32>, events: &[usize]) -> Option<Vec<usize>> {
        if self.plan.as_ref().is_some_and(|p| tick - p.press > POPUPS_OVER) {
            self.plan = None;
        }
        if self.plan.is_some() {
            return None;
        }
        let Some(spot) = points.in_reach(at, turn, true).filter(|s| !self.spent.contains(&s.id)) else {
            self.near = None;
            return None;
        };
        let near = *self.near.get_or_insert(tick);
        if !confirm {
            return None;
        }
        let rng = self.rng.get_or_insert_with(generator);
        let plan = points.plan_at(spot, (near, tick), rng, knowledge, events)?;
        for g in &plan.gave {
            eprintln!("gathered: item {} quality {} potentials {:?}{}", g.item, g.quality, g.potentials, if g.new { " (new discovery)" } else { "" });
        }
        self.spent.push(spot.id);
        let images = plan.images.clone();
        self.plan = Some(plan);
        Some(images)
    }

    /// The popups at `tick`.
    pub(super) fn show(&self, tick: f32) -> barrel_ryza::field_hud::Gathering {
        match &self.plan {
            Some(g) => barrel_ryza::field_hud::Gathering { near: Some(tick - g.near), pressed: Some(tick - g.press), found: g.found.clone() },
            None => barrel_ryza::field_hud::Gathering { near: self.near.map(|n| tick - n), ..Default::default() },
        }
    }

    /// The plan, if a gathering is on.
    pub(super) fn plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }
}

/// The generator for a run: the game's state from `BARREL_RANDOM_STATE` (a dump of its window, to
/// run in step with a capture) or seeded from the clock as the game does at start-up.
pub(super) fn generator() -> Random {
    if let Some(bytes) = std::env::var_os("BARREL_RANDOM_STATE").and_then(|p| std::fs::read(p).ok()) {
        if let Some(r) = Random::from_dump(&bytes) {
            return r;
        }
        eprintln!("BARREL_RANDOM_STATE: not a generator dump; seeding from the clock");
    }
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
    // The game's generator has made no draws it keeps track of when a run starts here.
    Random::after(seed, 0)
}
