//! The field's gathering points: which one is in reach, and what A gives there (the rules and
//! tables are `barrel_ryza::gather`; the popups are the HUD's). Docs: docs/ryza/open.md,
//! "Gathering (P6)".

use barrel_ryza::field_hud::Found;
use barrel_ryza::gather::{Boost, Context, Gathered, Tables};
use barrel_ryza::gimmick::{self, Spot};
use barrel_ryza::install::Install;
use barrel_ryza::random::Random;
use barrel_ryza::story::Story;

/// The gimmick types that gather by hand (A): the point is the place (`ITEM_D_MULTIPLE`); the ones the staff swings at
/// (X: `_WAND_ACTION`) are the other kind; the indirect ones are left out.
const HAND_POINT: &str = "ITEM_D_MULTIPLE";
const STAFF_POINT: &str = "ITEM_D_MULTIPLE_WAND_ACTION";

/// What gathers: her hand or the staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tool {
    Hand,
    Staff,
}

impl Tool {
    fn point(self) -> &'static str {
        match self {
            Tool::Hand => HAND_POINT,
            Tool::Staff => STAFF_POINT,
        }
    }
}
/// How much of a facing a point has to be in front of her to be in reach (the cosine of the angle
/// between her facing and the way to the point).
// STAND-IN: between the capture's two frames: 0.65 where the prompt is not up yet (`gather-rasen`
// 1994) and 0.70 where it still is (2010); the game's own test is not read.
const FACING: f32 = 0.67;
/// The reach of a point, in game units, from its `formParam` (metres).
const METRE: f32 = 100.0;
/// How near the staff's point has to be for its prompt and its swing (game units).
// STAND-IN: the prompt of `gather-rasen2` came up when she was 2.0 m from the box (3120 to 3121: 2.04, 1.97 m; at 3081 she
// was 2.9 m away and facing it with no prompt) and the swing hit from 1.6 m; the point's `formParam` is 3 and the game's
// own test (a box of that size turned by the row's `rotationY`) is not read.
const STAFF_REACH: f32 = 200.0;

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
        let spots = spots.into_iter().filter(|s| (s.kind == HAND_POINT || s.kind == STAFF_POINT) && story.within(&s.story, false) && s.values.iter().any(|v| tables.place(v).is_some())).collect();
        Some(Points { tables, spots })
    }

    /// The point of `tool`'s kind nearest her within its reach and in front of her, for her feet `at` (game units) and her turn
    /// in degrees (0 toward +z); `faced` false takes the nearest whichever way she faces. The staff's swing reaches a point
    /// whichever way she faces.
    // STAND-IN: `gather-rasen2` swung from 1.6 m at 62 degrees off her facing and hit; the game's own hit test (a cone, an arc,
    // the box) is not read, so any point of the staff's kind within `STAFF_REACH` counts.
    pub(super) fn in_reach(&self, at: [f32; 3], turn: f32, faced: bool, tool: Tool) -> Option<&Spot> {
        let (s, c) = turn.to_radians().sin_cos();
        let faced = faced && tool == Tool::Hand;
        self.spots
            .iter()
            .filter(|p| p.kind == tool.point())
            .filter_map(|p| {
                let (dx, dz) = (p.position[0] - at[0], p.position[2] - at[2]);
                let far = dx.hypot(dz);
                let ahead = far < 1e-3 || (dx * s + dz * c) / far >= FACING;
                let reach = if tool == Tool::Staff { STAFF_REACH } else { p.reach.max(1.0) * METRE };
                (far <= reach && (ahead || !faced)).then_some((far, p))
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
    pub tool: Tool,
    /// The point it was at (a swing at nothing has none).
    pub spot: Option<u32>,
    /// What she said as it began (a voice clip's hash), if she spoke and the clip's hash is known.
    pub voice: Option<u32>,
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
    pub(super) fn plan(&self, at: [f32; 3], turn: f32, frames: (f32, f32), tool: Tool, spent: &[u32], rng: &mut Random, knowledge: &mut Vec<i32>, events: &[usize]) -> Option<Plan> {
        let spot = self.in_reach(at, turn, false, tool).filter(|s| !spent.contains(&s.id));
        match spot {
            Some(spot) => self.plan_at(spot, frames, tool, rng, knowledge, events),
            None => Some(Plan::swing_only(frames, tool, rng)).filter(|_| tool == Tool::Staff),
        }
    }

    /// The gathering at `spot`: the draws in the game's order (the staff's clip before the gathering's, her voice after it).
    fn plan_at(&self, spot: &Spot, frames: (f32, f32), tool: Tool, rng: &mut Random, knowledge: &mut Vec<i32>, events: &[usize]) -> Option<Plan> {
        use barrel_ryza::sound::GATHER_VOICES;
        let mut voice = if tool == Tool::Staff { swing_voice(rng) } else { None };
        let gave = self.gather(spot, rng, knowledge, events);
        if tool == Tool::Hand {
            voice = barrel_ryza::gather::hand_clip(rng).and_then(|clip| GATHER_VOICES[clip]);
        }
        barrel_ryza::gather::learn(knowledge, &gave);
        let found = gave.iter().map(|g| Found { name: barrel_ryza::item_info::NAME_BASE + g.item as u32, new: g.new }).collect();
        let images = gave.iter().map(|g| self.image(g.item)).collect::<Option<Vec<_>>>().filter(|i| !i.is_empty())?;
        Some(Plan { near: frames.0, press: frames.1, tool, spot: Some(spot.id), voice, found, images, gave })
    }
}

/// The staff's swing clip (`irand(0, 1)`).
fn swing_voice(rng: &mut Random) -> Option<u32> {
    Some(barrel_ryza::sound::SWING_VOICES[barrel_ryza::gather::swing_clip(rng)])
}

impl Plan {
    /// A swing that hits nothing: her clip and the swing's sounds only.
    fn swing_only(frames: (f32, f32), tool: Tool, rng: &mut Random) -> Plan {
        Plan { near: frames.0, press: frames.1, tool, spot: None, voice: swing_voice(rng), found: Vec::new(), images: Vec::new(), gave: Vec::new() }
    }

    /// The sounds it makes, from the frame its button went down.
    pub(super) fn cues(&self, press: i64) -> Vec<barrel_ryza::sound::Cue> {
        barrel_ryza::sound::gather_cues(press, self.tool == Tool::Staff, self.voice, self.found.iter().any(|f| f.new))
    }

    /// Frames after the press the prompt starts to slide out.
    pub(super) fn leave_at(&self) -> f32 {
        match self.tool {
            Tool::Hand => 0.0,
            Tool::Staff => barrel_ryza::field_hud::STAFF_LEAVE_AT,
        }
    }

    /// The popups' clock (`Gathering::notice_at`): frames after the press the first notice starts.
    pub(super) fn notice_at(&self) -> f32 {
        match self.tool {
            Tool::Hand => barrel_ryza::field_hud::HAND_NOTICE_AT,
            Tool::Staff => barrel_ryza::field_hud::STAFF_NOTICE_AT,
        }
    }
}

/// Frames after X the swing's action begins and how long it runs (3166 to 3226 from X at 3160).
const SWING_ACTION_FROM: f32 = 6.0;
const SWING_ACTION: f32 = 60.0;
/// The prompt's slide-in (`anm_common_slide_in_bottom`).
const PROMPT_SLIDE: f32 = 30.0;
/// The frames after A the popups are over in (the discovery log's fade-out ends 328 frames after it).
const POPUPS_OVER: f32 = 330.0;

/// The gathering in a window: whether a point is in reach and faced, and the one begun by A.
#[derive(Default)]
pub(super) struct Live {
    /// The tick the point came in reach (the prompt began to slide in).
    near: Option<f32>,
    /// Whether the point in reach is the staff's (the prompt's word).
    staff_near: bool,
    plan: Option<Plan>,
    /// The points gathered from (they give nothing more: `gather-rasen` 2200 and 2360 got nothing).
    spent: Vec<u32>,
    rng: Option<Random>,
}

impl Live {
    /// One frame at `tick` (capture-style frames): her feet `at`, her turn, and whether A (by hand) or X (the staff's swing)
    /// went down. Returns the picture numbers of the items gathered when this frame's press gathered some; a swing
    /// that hits nothing makes the plan too (`plan()`: its sounds), with no pictures.
    pub(super) fn step(&mut self, points: &Points, at: [f32; 3], turn: f32, tick: f32, buttons: (bool, bool), knowledge: &mut Vec<i32>, events: &[usize]) -> Option<Vec<usize>> {
        let (confirm, swing) = buttons;
        // A swing that hit nothing is over with its action; a gathering when its popups are.
        if self.plan.as_ref().is_some_and(|p| tick - p.press > if p.found.is_empty() { SWING_ACTION_FROM + SWING_ACTION } else { POPUPS_OVER }) {
            self.plan = None;
        }
        if self.plan.is_some() {
            return None;
        }
        let hand = points.in_reach(at, turn, true, Tool::Hand).filter(|s| !self.spent.contains(&s.id));
        let staff = points.in_reach(at, turn, false, Tool::Staff).filter(|s| !self.spent.contains(&s.id));
        match (hand, staff) {
            (None, None) => self.near = None,
            _ => {
                self.near.get_or_insert(tick);
            }
        }
        self.staff_near = hand.is_none() && staff.is_some();
        let tool = match (confirm && hand.is_some(), swing) {
            (true, _) => Tool::Hand,
            (false, true) => Tool::Staff,
            _ => return None,
        };
        let rng = self.rng.get_or_insert_with(generator);
        let frames = (self.near.unwrap_or(tick), tick);
        let plan = match hand.filter(|_| tool == Tool::Hand) {
            Some(spot) => points.plan_at(spot, frames, tool, rng, knowledge, events)?,
            None => points.plan(at, turn, frames, tool, &self.spent, rng, knowledge, events)?,
        };
        for g in &plan.gave {
            eprintln!("gathered: item {} quality {} potentials {:?}{}", g.item, g.quality, g.potentials, if g.new { " (new discovery)" } else { "" });
        }
        self.spent.extend(plan.spot);
        let images = plan.images.clone();
        self.plan = Some(plan);
        Some(images)
    }

    /// Whether the help bar's rest count is held at 0 at `tick`: while the prompt slides in and while a swing's action runs.
    // STAND-IN: read off `gather-rasen2`: the bar's fade-in came 121 frames after the prompt's slide-in ended (the hand's 2440 + 30)
    // and after the swing's action (60 frames from its first command: 3166 and 3266 to 3325), not after her stop or her presses.
    pub(super) fn busy(&self, tick: f32) -> bool {
        let swinging = self.plan.as_ref().is_some_and(|p| p.tool == Tool::Staff && (SWING_ACTION_FROM..SWING_ACTION_FROM + SWING_ACTION).contains(&(tick - p.press)));
        swinging || self.near.is_some_and(|n| (0.0..PROMPT_SLIDE).contains(&(tick - n)))
    }

    /// The popups at `tick`.
    pub(super) fn show(&self, tick: f32) -> barrel_ryza::field_hud::Gathering {
        match &self.plan {
            Some(g) if !g.found.is_empty() => barrel_ryza::field_hud::Gathering { near: Some(tick - g.near), pressed: Some(tick - g.press), found: g.found.clone(), notice_at: g.notice_at(), leave_at: g.leave_at(), staff: g.tool == Tool::Staff },
            _ => barrel_ryza::field_hud::Gathering { near: self.near.map(|n| tick - n), staff: self.staff_near, ..Default::default() },
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
