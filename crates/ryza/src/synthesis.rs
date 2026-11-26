//! The synthesis flow as one state machine: BACK in free play asks "Return to Room", Yes loads the attic, the cauldron's prompt
//! "Synthesis" (the `MIX` gimmick) opens Select Recipe (with its tutorial pages), the recipe's list opens Add Materials, the
//! last loop makes the item, the result card and Complete / Level UP follow and B goes back to the room. It is stepped a frame
//! at a time with the pad's presses and says which screen is up and which sounds the game cues (docs/ryza/open.md, "Synthesis
//! (P6)"; the answer key is the `synth-atelier` scenario of ryza-recomp).
//!
//! Every frame number below is an offset from the press that caused it, read from the key's `play` lines (the hook log's sounds,
//! `tools/ryza_cues.py`). The screens' timings that have no sound (how long the opening takes) come from the key's draws.

use crate::mix;
use crate::save::Worn;
use crate::sound::Cue;

/// The screen the machine is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Free play in a field.
    Field,
    /// The "Return to Room" Yes / No prompt (`yes` is the cursor).
    Prompt { yes: bool },
    /// The attic loading, then the room's notice before she can move.
    Loading,
    Notice,
    /// The room: she walks and the cauldron's prompt shows when she is near.
    Room,
    /// Select Recipe's opening, its tutorial pages (1-based) and the list.
    RecipeOpening,
    Tutorial(u32),
    Recipes { cursor: usize },
    /// Add Materials: how many presses have gone into it.
    Materials { step: usize },
    /// The synthesis running, the result card coming in, and the Complete screen's panels.
    Making,
    Card,
    Result,
    Complete,
    /// The Complete screen going out.
    Leaving,
}

/// What the pad did this frame: the buttons pressed (edges) and the left stick as held (-1 to 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    pub a: bool,
    pub b: bool,
    pub back: bool,
    pub down: bool,
    pub stick: [f32; 2],
}

/// Where a voice line's random pick (`irand(lo, hi)`, `0x1402de19b`) comes from.
pub trait Picks {
    fn pick(&mut self, lo: i32, hi: i32) -> i32;
}

impl Picks for crate::random::Random {
    fn pick(&mut self, lo: i32, hi: i32) -> i32 {
        self.irand(lo, hi)
    }
}

/// The picks the key's run made, by the line's range (docs/ryza/open.md): the lowest of a range, except (4, 5) gave 5 and (0x18, 0x1b) 0x1a.
pub struct KeyPicks;

impl Picks for KeyPicks {
    fn pick(&mut self, lo: i32, hi: i32) -> i32 {
        match (lo, hi) {
            (4, 5) => 5,
            (0x18, 0x1b) => 0x1a,
            _ => lo,
        }
    }
}

/// The voice clips whose hash the key shows, by the clip's number (the pick).
const VOICE_CLIPS: [(i32, u32); 8] = [(0, 0x6b21_1790), (5, 0x8bb5_cdea), (6, 0xe591_382b), (8, 0x9948_0cad), (0x14, 0x2edc_aace), (0x16, 0xe293_7f50), (0x1a, 0xc525_f1b4), (0x1c, 0x78dc_c636)];

// The sounds the key names by hash.
const CLICK: u32 = 0x09c3_999b;
const MESSAGE: u32 = 0x54ee_d79d;
const CURSOR: u32 = 0x5e3d_ae22;
const CANCEL: u32 = 0x22bd_a2fa;
const TUTORIAL_END: u32 = 0x3027_bfbd;
const WALK_START: u32 = 0xc1e8_9d60;
const FOOTSTEP: u32 = 0xb704_f2c1;
const IDLE: u32 = 0xfb78_b7fb;
const ARRIVAL_JINGLE: u32 = 0x451e_4770;
const NOTICE_JINGLE: u32 = 0x414c_7000;
const MATERIAL_ADDED: u32 = 0x49bc_3781;
const RING_A: u32 = 0x3356_d4c4;
const RING_B: u32 = 0x4f13_9bfa;
const RING_C: u32 = 0x7e18_5a3a;
const LOOP_DONE: u32 = 0x8865_c5c1;
const SYNTHESIS: u32 = 0x3e78_ab99;
const MAKING_DONE: u32 = 0xc0d3_83b0;
const CARD_A: u32 = 0xb86b_c79c;
const CARD_B: u32 = 0x11e2_5c7c;
const LEVEL_UP: u32 = 0x67c2_431d;

/// One cue of a step: the frames after the press, what it is. A voice names its pick range.
#[derive(Clone, Copy)]
enum Sound {
    Effect(u32),
    Voice(i32, i32),
    Stop,
}

use Sound::{Effect, Stop, Voice};

/// What each A on Add Materials does, by its number since the screen opened (the 11 of `ADD_STEPS`): the first three loops open the materials' list with A on a ring and
/// place the material under its cursor with the next A; the rest are the loops' own presses.
// STAND-IN: read off the key (`synth-atelier` 2920 to 3520: the lists open on the 1st, 3rd and 7th press); the game's rule for the roles is not read.
pub const ADD_PRESS_ROLES: [PressRole; 11] = [
    PressRole::Ring,
    PressRole::Material,
    PressRole::Ring,
    PressRole::Material,
    PressRole::Loop,
    PressRole::Loop,
    PressRole::Ring,
    PressRole::Material,
    PressRole::Loop,
    PressRole::Loop,
    PressRole::Loop,
];

/// What an A on Add Materials does to the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressRole {
    /// Opens the materials' list on the ring under the cursor.
    Ring,
    /// Places the material under the list's cursor (the list goes out).
    Material,
    /// One of the loops' own presses.
    Loop,
}

/// What each A in Add Materials does, by its number since the screen opened, as `(frames after the press, sound)`.
// STAND-IN: the key's 12 presses of the loops (`synth-atelier` 2920 to 3520) with the sounds each made: the sounds follow the recipe's
// rows (a material added, a loop's ring lighting and closing), but the game's rule for which press does which is not read; this is
// the run of recipe 92 with the first listed material each time (4 materials over 3 loops).
const ADD_STEPS: [&[(i64, Sound)]; 11] = [
    &[(0, Effect(MATERIAL_ADDED)), (1, Voice(6, 7))],
    &[(0, Stop), (0, Effect(CLICK)), (19, Effect(RING_A)), (19, Effect(RING_A))],
    &[(0, Effect(MATERIAL_ADDED)), (1, Voice(6, 7)), (6, Effect(RING_B)), (12, Effect(RING_B))],
    &[(0, Stop), (0, Effect(CLICK)), (19, Effect(RING_A)), (19, Effect(RING_A)), (25, Effect(RING_C))],
    &[(7, Effect(RING_B)), (37, Effect(LOOP_DONE)), (38, Voice(8, 9))],
    &[],
    &[(0, Effect(MATERIAL_ADDED)), (1, Voice(6, 7))],
    &[(0, Stop), (0, Effect(CLICK)), (19, Effect(RING_A)), (19, Effect(RING_A))],
    &[(7, Effect(RING_B)), (12, Effect(RING_B)), (37, Effect(LOOP_DONE)), (38, Voice(8, 9))],
    &[(40, Voice(0x14, 0x15))],
    // The last press makes the item: the synthesis 16 frames later, the "done" sound 122 after the press.
    &[(0, Effect(CLICK)), (16, Voice(0x16, 0x17)), (16, Effect(SYNTHESIS)), (122, Effect(MAKING_DONE))],
];

/// The frames the Select Recipe screen takes to open before its first tutorial page (A at 2300, the page up by 2336), and the frames a
/// page takes before A is accepted.
// STAND-IN: 36 from the draws; the lock is between 24 (the key's A at 2360 is ignored on the page that came up at 2336) and 60 (the next
// A is taken): 30.
const RECIPE_OPENING: i64 = 36;
const PAGE_LOCK: i64 = 30;
/// The tutorial's pages (the key's A at 2420 to 2660 turn five, the A at 2720 ends the sixth).
const TUTORIAL_PAGES: u32 = 6;
/// The frames the synthesis takes before A is taken for the result card (the key ignores A at 3580 to 3760, 240 after the confirm, and takes
/// the one at 3820, 300 after).
// STAND-IN: between those, 270.
const MAKING_FRAMES: i64 = 270;
/// The frames after the press that opened the Complete screen going out (the CANCEL sound of 4240 + 30).
const LEAVE_AFTER: i64 = 30;
/// The frames from the first B in the room to her idle sounds (4300 to 4360), and between them (every 60, the second sound 56 on).
const IDLE_AFTER: i64 = 60;
const IDLE_EVERY: i64 = 60;
const IDLE_OTHER: i64 = 56;
/// Loading the attic: the frames from Yes to the arrival jingle (1960 to 2012), the stop of the field's sounds (to 1980), and from the
/// notice's A to its jingle (2100 to 2133).
const LOAD_JINGLE: i64 = 52;
const LOAD_STOP: i64 = 20;
const NOTICE_JINGLE_AFTER: i64 = 33;
/// The frames from the room's notice up to the walk being accepted are not modelled: she moves from the stick's first frame.
/// Her footsteps: the first sound a frame after the stick, then one every 22.5 frames while she moves (2161, 2183, 2206).
const FOOTSTEP_EVERY: f32 = 22.5;
/// The Level UP's gain without the game's level table (`Machine::with_alchemy` gives the rule): the alchemy level goes up one.
pub const LEVEL_GAIN: u32 = 1;

/// What the experience rule needs besides the save: the executable's level table, the made item's level (`itemdata.xml`'s `lv`), the
/// synthesis's batches and how many times the recipe was made before. STAND-IN: `made_before` is the caller's (the game's count comes from
/// the recipe list of the synthesis state, `0x1401cab30`; for the key's first synthesis both of its paths give 1).
pub struct Alchemy {
    pub levels: crate::alchemy::Levels,
    pub item_level: u32,
    pub batches: u32,
    pub made_before: u32,
}

/// A second unit of what the machine changed in the save: the clock word, the container and the alchemy level.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub clock: u32,
    pub container: Vec<Worn>,
    pub level: u32,
    /// The alchemy experience (`mix_lv/m_exp`).
    pub exp: u32,
}

/// The made item's units, as the caller builds them (their quality and effects are the rings' rules: `rings::Cauldron`).
pub type Maker<'a> = &'a dyn Fn(&mix::Made) -> Vec<Worn>;

pub struct Machine<'a> {
    screen: Screen,
    /// The frame of the last screen change.
    since: i64,
    frame: i64,
    picks: Box<dyn Picks + 'a>,
    world: World,
    tables: &'a mix::Tables,
    recipe: usize,
    maker: Maker<'a>,
    alchemy: Option<Alchemy>,
    /// Her place (x, z) and facing (radians), from the walk.
    pub at: [f32; 2],
    pub turn: f32,
    /// The frame the stick first went down in the room, and the footsteps cued so far.
    walk_from: Option<i64>,
    steps_cued: u32,
    /// Whether the machine has a recipe's units to take (`make` found the container's materials).
    made: Option<mix::Made>,
    /// The frame the room was reached from the Complete screen (her idle sounds count from it).
    idle_from: Option<i64>,
    /// The frames of each A on Add Materials, and of the A on Select Recipe that opens it (`recipe_press`).
    presses: Vec<i64>,
    recipe_press: Option<i64>,
}

/// Where she arrives in the attic and which way the room's camera looks, for the walk (game units, degrees).
// STAND-IN: the key's dumps start at 2250, after the walk (2160 to 2220: LX 16000, LY 26000), so neither the arrival nor the room camera's
// yaw is known. The yaw is fitted so that her last move's direction is the key's facing at 2250 (14.35 degrees), and the arrival so that
// 60 frames of the stick end at the key's place (3772.95, 4140.70): the end place and facing are fitted, not checked, and the way
// between them is not (she stops by 2200 in the key: a wall or the cauldron may stop her there; walls are not modelled).
pub const ARRIVAL: [f32; 2] = [3637.05, 3609.3];
pub const ROOM_YAW: f32 = 45.95;

impl<'a> Machine<'a> {
    pub fn new(tables: &'a mix::Tables, recipe: usize, world: World, maker: Maker<'a>, picks: impl Picks + 'a) -> Machine<'a> {
        Machine { screen: Screen::Field, since: 0, frame: 0, picks: Box::new(picks), world, tables, recipe, maker, alchemy: None, at: ARRIVAL, turn: 0.0, walk_from: None, steps_cued: 0, made: None, idle_from: None, presses: Vec::new(), recipe_press: None }
    }

    /// The experience rule for the Level UP (without it the level goes up `LEVEL_GAIN`).
    pub fn with_alchemy(mut self, alchemy: Alchemy) -> Machine<'a> {
        self.alchemy = Some(alchemy);
        self
    }

    /// Starts the clock at `frame` (the run's own frame count, so cues carry it).
    pub fn start_at(&mut self, frame: i64) {
        self.frame = frame;
        self.since = frame;
    }

    /// The frames of the A presses made on Add Materials so far, in order (what each does is `ADD_PRESS_ROLES`).
    pub fn add_presses(&self) -> &[i64] {
        &self.presses
    }

    /// The frame of the A on Select Recipe that opened Add Materials.
    pub fn recipe_press(&self) -> Option<i64> {
        self.recipe_press
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The frames since the screen changed.
    pub fn age(&self) -> i64 {
        self.frame - self.since
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.since = self.frame;
    }

    fn cue(&mut self, out: &mut Vec<Cue>, after: i64, sound: Sound) {
        let at = self.frame + after;
        match sound {
            Effect(hash) => out.push(Cue::effect(at, hash)),
            Stop => out.push(Cue::stop(at)),
            Voice(lo, hi) => {
                let pick = self.picks.pick(lo, hi);
                if let Some(&(_, hash)) = VOICE_CLIPS.iter().find(|(n, _)| *n == pick) {
                    out.push(Cue::voice(at, hash));
                }
            }
        }
    }

    /// Whether the cauldron's prompt is up: she stands in the `MIX` gimmick's box (centre, half sizes from its `formParam` 2.3 by 3 m).
    pub fn near_cauldron(&self) -> bool {
        const CENTRE: [f32; 2] = [3785.0, 4263.0];
        const HALF: [f32; 2] = [115.0, 150.0];
        self.screen == Screen::Room && (self.at[0] - CENTRE[0]).abs() <= HALF[0] && (self.at[1] - CENTRE[1]).abs() <= HALF[1]
    }

    /// One frame (60 a second): the pad's presses, and the cues the frame starts (each at its own frame, possibly later).
    pub fn step(&mut self, pad: Pad) -> Vec<Cue> {
        let mut out = Vec::new();
        let age = self.age();
        match self.screen {
            Screen::Field => {
                if pad.back {
                    self.go(Screen::Prompt { yes: true });
                }
            }
            Screen::Prompt { yes } => {
                if pad.down {
                    self.go(Screen::Prompt { yes: !yes });
                } else if pad.a && yes {
                    self.cue(&mut out, 0, Effect(CLICK));
                    self.cue(&mut out, 0, Effect(MESSAGE));
                    self.cue(&mut out, LOAD_STOP, Stop);
                    out.push(Cue::music(self.frame + LOAD_JINGLE, ARRIVAL_JINGLE));
                    self.go(Screen::Loading);
                } else if pad.b || (pad.a && !yes) {
                    self.cue(&mut out, 0, Effect(CANCEL));
                    self.go(Screen::Field);
                }
            }
            Screen::Loading => {
                if age >= LOAD_JINGLE {
                    self.at = ARRIVAL;
                    self.go(Screen::Notice);
                }
            }
            Screen::Notice => {
                if pad.a {
                    self.cue(&mut out, 0, Effect(MESSAGE));
                    out.push(Cue::music(self.frame + NOTICE_JINGLE_AFTER, NOTICE_JINGLE));
                    self.go(Screen::Room);
                }
            }
            Screen::Room => {
                self.walk(pad.stick, &mut out);
                if let Some(from) = self.idle_from {
                    let n = self.frame - from - IDLE_AFTER;
                    if n >= 0 && n % IDLE_EVERY == 0 {
                        self.cue(&mut out, 0, Effect(IDLE));
                    }
                    let m = n - IDLE_OTHER;
                    if m >= 0 && m % IDLE_EVERY == 0 {
                        self.cue(&mut out, 0, Effect(FOOTSTEP));
                    }
                }
                if pad.a && self.near_cauldron() {
                    self.cue(&mut out, 3, Voice(0, 1));
                    self.go(Screen::RecipeOpening);
                }
            }
            Screen::RecipeOpening => {
                if age >= RECIPE_OPENING {
                    self.go(Screen::Tutorial(1));
                }
            }
            Screen::Tutorial(page) => {
                if pad.a && age >= PAGE_LOCK {
                    if page < TUTORIAL_PAGES {
                        self.cue(&mut out, 0, Effect(MESSAGE));
                        self.go(Screen::Tutorial(page + 1));
                    } else {
                        self.cue(&mut out, 0, Effect(TUTORIAL_END));
                        self.go(Screen::Recipes { cursor: 0 });
                    }
                }
            }
            Screen::Recipes { cursor } => {
                if pad.down && cursor + 1 < 2 {
                    self.cue(&mut out, 0, Effect(CURSOR));
                    self.go(Screen::Recipes { cursor: cursor + 1 });
                } else if pad.a && cursor == 1 {
                    self.cue(&mut out, 0, Effect(CLICK));
                    self.cue(&mut out, 0, Voice(4, 5));
                    let recipe = &self.tables.recipes[self.recipe];
                    self.made = mix::make(self.tables, recipe, &self.world.container);
                    self.recipe_press = Some(self.frame);
                    self.go(Screen::Materials { step: 0 });
                } else if pad.b {
                    self.cue(&mut out, 0, Effect(CANCEL));
                    self.go(Screen::Room);
                }
            }
            Screen::Materials { step } => {
                if pad.a {
                    self.presses.push(self.frame);
                    for &(after, sound) in ADD_STEPS[step.min(ADD_STEPS.len() - 1)] {
                        self.cue(&mut out, after, sound);
                    }
                    if step + 1 >= ADD_STEPS.len() {
                        self.go(Screen::Making);
                    } else {
                        self.go(Screen::Materials { step: step + 1 });
                    }
                }
            }
            Screen::Making => {
                if pad.a && age >= MAKING_FRAMES {
                    self.cue(&mut out, 4, Voice(0x18, 0x1b));
                    self.cue(&mut out, 10, Effect(CARD_A));
                    self.cue(&mut out, 16, Effect(CARD_B));
                    self.go(Screen::Card);
                }
            }
            Screen::Card => {
                // The card's "Fast Forward" bar turns to "Next" 140 frames in; A then (and from the key's 4000, the 3880 and 3940 A's
                // are skipping the card in) goes on to the made item's card.
                if pad.a && age >= CARD_FAST {
                    self.cue(&mut out, 0, Effect(MESSAGE));
                    self.go(Screen::Result);
                }
            }
            Screen::Result => {
                if pad.a && age >= RESULT_LOCK {
                    self.cue(&mut out, 0, Effect(MESSAGE));
                    self.cue(&mut out, 41, Voice(0x1c, 0x1d));
                    self.cue(&mut out, 41, Effect(LEVEL_UP));
                    self.go(Screen::Complete);
                }
            }
            Screen::Complete => {
                if pad.a && age >= COMPLETE_LOCK {
                    self.cue(&mut out, LEAVE_AFTER, Effect(CANCEL));
                    self.go(Screen::Leaving);
                }
            }
            Screen::Leaving => {
                if pad.b {
                    self.cue(&mut out, 0, Effect(CANCEL));
                    self.cue(&mut out, 0, Stop);
                    self.finish();
                    self.idle_from = Some(self.frame);
                    self.go(Screen::Room);
                }
            }
        }
        self.frame += 1;
        out
    }

    /// The synthesis's changes to the save when the player is back in the room: the units taken go and the made ones come, the clock goes
    /// on by the hours and the alchemy level up.
    fn finish(&mut self) {
        if let Some(made) = self.made.take() {
            let mut container = mix::after(&self.world.container, &made);
            container.extend((self.maker)(&made));
            self.world.container = container;
            self.world.clock = mix::clock_after(self.world.clock, made.hours);
            match &self.alchemy {
                Some(a) => {
                    let w = &mut self.world;
                    w.exp += crate::alchemy::gain(w.level, a.batches, a.item_level, a.made_before);
                    w.level = w.level.max(a.levels.level_for(w.exp)).min(crate::alchemy::MAX_LEVEL);
                }
                None => self.world.level += LEVEL_GAIN,
            }
        }
    }

    /// Her walk in the room from the stick (the game's lean against the room camera, at her run speed), with her footsteps' sounds.
    fn walk(&mut self, stick: [f32; 2], out: &mut Vec<Cue>) {
        let lean = crate::walk::lean(stick, ROOM_YAW);
        if lean == [0.0; 2] {
            return;
        }
        let from = *self.walk_from.get_or_insert(self.frame);
        if self.steps_cued == 0 {
            self.cue(out, 1, Effect(WALK_START));
            self.steps_cued = 1;
        }
        // The footsteps run on from the first sound one every `FOOTSTEP_EVERY` frames while the stick is held.
        let due = ((self.frame - from) as f32 / FOOTSTEP_EVERY) as u32;
        while self.steps_cued <= due {
            let n = self.steps_cued;
            let at = (1.0 + FOOTSTEP_EVERY * n as f32) as i64;
            // (each step after the first ends the one before: a stop at the same frame, as the key's 2183 and 2206)
            out.push(Cue::stop(from + at));
            out.push(Cue::effect(from + at, FOOTSTEP));
            self.steps_cued += 1;
        }
        let k = crate::player::FULL_SPEED / 60.0;
        self.at = [self.at[0] + lean[0] * k, self.at[1] + lean[1] * k];
        self.turn = lean[0].atan2(lean[1]);
    }
}

/// Frames before the card takes A for the made item's card, the made item's card for Level UP, and Complete's for leaving.
// STAND-IN: the key's A presses are every 60 frames and each one after the card's coming in is taken from 4000 on (3880 and 3940 are
// ignored: the card was 60 and 120 frames in); the made item's card (4000) takes the next at 4060 (60 after); Complete (4101, 4120 and
// 4180 ignored) takes 4240. The locks are between the ignored and the taken presses.
const CARD_FAST: i64 = 150;
const RESULT_LOCK: i64 = 30;
const COMPLETE_LOCK: i64 = 130;
/// The pad script of a scenario (`pad.txt`: `FRAME BUTTON[+BUTTON...] HOLD` lines; `A`, `B`, `BACK`, `DOWN` are presses, `LX=n`, `LY=n` stick
/// values held for HOLD frames): the pad at `frame`.
pub fn script_pad(script: &str, frame: i64) -> Pad {
    let mut pad = Pad::default();
    for line in script.lines() {
        let mut w = line.split_whitespace();
        let (Some(f), Some(what), Some(hold)) = (w.next().and_then(|v| v.parse::<i64>().ok()), w.next(), w.next().and_then(|v| v.parse::<i64>().ok())) else { continue };
        for part in what.split('+') {
            match part {
                "A" if f == frame => pad.a = true,
                "B" if f == frame => pad.b = true,
                "BACK" if f == frame => pad.back = true,
                "DOWN" if f == frame => pad.down = true,
                _ => {
                    if let Some((axis, v)) = part.split_once('=') {
                        if frame >= f && frame < f + hold {
                            let v = v.parse::<f32>().unwrap_or(0.0) / 32767.0;
                            match axis {
                                "LX" => pad.stick[0] = v,
                                "LY" => pad.stick[1] = v,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    pad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_gives_presses_on_their_frame_and_sticks_for_their_hold() {
        let s = "1900 BACK 6\n2160 LX=16000+LY=26000 60\n";
        assert!(script_pad(s, 1900).back && !script_pad(s, 1901).back);
        assert_eq!(script_pad(s, 2159).stick, [0.0; 2]);
        assert!(script_pad(s, 2160).stick[1] > 0.79 && script_pad(s, 2219).stick[0] > 0.48);
        assert_eq!(script_pad(s, 2220).stick, [0.0; 2]);
    }

    const RECIPES: &str = r#"<Root>
<itemRecipeData ItemTag="ITEM_MIX_A" MakeNum="4" Hour="14" NeedNum="2" MatTag="ITEM_CATEGORY_FLOWER"/>
<itemRecipeData MatTag="ITEM_CATEGORY_LIQUID" NeedNum="1"/>
</Root>"#;
    const ITEMS: &str = r#"<Root><itemData cat_0="ITEM_CATEGORY_LIQUID"/><itemData cat_0="ITEM_CATEGORY_FLOWER"/></Root>"#;

    /// A run of the key's inputs (the synth-atelier pad script) on a container of two flowers and a liquid.
    fn run(script: &str, to: i64) -> (Vec<Screen>, Vec<Cue>, World, [f32; 2]) {
        let tables = mix::Tables::read(RECIPES.as_bytes(), ITEMS.as_bytes()).unwrap();
        let unit = |item| Worn { item, ..Default::default() };
        let world = World { clock: 493, container: vec![unit(1), unit(1), unit(0), unit(1)], level: 1, exp: 0 };
        let maker = |m: &mix::Made| vec![unit(7); m.count as usize];
        let mut machine = Machine::new(&tables, 0, world, &maker, KeyPicks);
        machine.start_at(1900);
        let (mut screens, mut cues) = (Vec::new(), Vec::new());
        for f in 1900..=to {
            cues.extend(machine.step(script_pad(script, f)));
            if screens.last() != Some(&machine.screen()) {
                screens.push(machine.screen());
            }
        }
        (screens, cues, machine.world().clone(), machine.at)
    }

    fn key_script() -> String {
        let mut s = String::from("1900 BACK 6
1960 A 6
2100 A 6
2160 LX=16000+LY=26000 60
2300 A 6
");
        for f in (2360..=2720).step_by(60) {
            s += &format!("{f} A 6
");
        }
        s += "2800 DOWN 6
";
        for f in (2860..=4240).step_by(60) {
            s += &format!("{f} A 6
");
        }
        s + "4300 B 6
"
    }

    #[test]
    fn the_keys_inputs_run_the_whole_flow_and_leave_the_save_changed() {
        let (screens, cues, world, at) = run(&key_script(), 4400);
        for want in [Screen::Prompt { yes: true }, Screen::Loading, Screen::Notice, Screen::Room, Screen::RecipeOpening, Screen::Tutorial(6), Screen::Recipes { cursor: 1 }, Screen::Materials { step: 10 }, Screen::Making, Screen::Card, Screen::Result, Screen::Complete, Screen::Leaving] {
            assert!(screens.contains(&want), "{want:?} not reached: {screens:?}");
        }
        assert_eq!(screens.last(), Some(&Screen::Room));
        // Back in the room: 14 hours on, the four units gone for the made ones, the level up; she stopped by the cauldron.
        assert_eq!(world.clock, 493 + 14 * 50);
        assert_eq!(world.container.iter().filter(|w| w.item == 7).count(), 4);
        assert_eq!(world.container.len(), 4 - 3 + 4 + 0);
        assert_eq!(world.level, 2);
        assert!((at[0] - 3772.95).abs() < 10.0 && (at[1] - 4140.70).abs() < 10.0, "{at:?}");
        // The sounds: the tutorial's MESSAGEs, the cursor, the clicks.
        let at_frame = |hash| cues.iter().filter(|c| c.hash == hash).map(|c| c.at_frame).collect::<Vec<_>>();
        assert_eq!(at_frame(MESSAGE), [1960, 2100, 2420, 2480, 2540, 2600, 2660, 4000, 4060]);
        assert_eq!(at_frame(CURSOR), [2800]);
        assert_eq!(at_frame(CANCEL), [4270, 4300]);
    }

    #[test]
    fn a_press_before_the_prompt_or_far_from_the_cauldron_opens_nothing() {
        // A in the room with no walk: she is at the arrival, far from the cauldron's box.
        let (screens, _, _, _) = run("1900 BACK 6
1960 A 6
2100 A 6
2300 A 6
", 2400);
        assert_eq!(screens.last(), Some(&Screen::Room));
        // B on the prompt goes back to free play with the cancel sound.
        let (screens, cues, _, _) = run("1900 BACK 6
1960 B 6
", 2000);
        assert_eq!(screens.last(), Some(&Screen::Field));
        assert!(cues.iter().any(|c| c.hash == CANCEL && c.at_frame == 1960));
    }

    #[test]
    fn a_tutorial_page_ignores_a_press_too_soon_after_it_came_up() {
        let (screens, _, _, _) = run(&key_script().replace("2420 A 6
", "").replace("2480 A 6
", ""), 2400);
        assert_eq!(screens.last(), Some(&Screen::Tutorial(1)));
    }
}
