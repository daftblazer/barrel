//! Leaving the Settings screen, and the same dialog on a tab change: LB or RB with something
//! changed asks first, and the answer decides the changes' fate before the tab turns (No: the
//! dialog slides out, the changes are dropped and the tab turns; Yes: the "Saving" cover, then
//! the tab turns; B closes the dialog and the tab stays, a guess: the capture has no B there).
//! Leaving the Settings screen. B after a change opens the dialog "Do you want to apply the
//! changes?" with a Yes / No box (the cursor starts on No); B closes it again, A on No throws the
//! changes away and leaves, A on Yes writes them, covers the screen in black with the "Saving..."
//! notice and then leaves. B with nothing changed leaves at once.
//!
//! Times are title frames (60 a second). The numbers are read off the capture `settings-leave`
//! (and `-gfx`, `-no`) of the original game: a frame's `age` counts from 1 on the frame of the
//! key. Offscreen the keys come from the script (`--title-keys`) and [`replay`] runs them again
//! from the screen's first frame to the one asked for, so a picture is a function of its frame.

use crate::options::State;
use crate::title_menu::{Hold, Input};

/// Frames the dialog's slide (`anm_common_slide_in_bottom`) takes, and the common fade.
pub const SLIDE: i32 = 30;
pub const FADE: i32 = 15;
/// The list's own help bar starts to fade out this many frames after the key that opens the
/// dialog, and comes back this many after the key that closes it; the dialog's own bar fades out
/// this many frames after a key that closes it and in this many after the one that opens it
/// (the capture's alphas 8/15 at +10 after an opening, 7/15 at +27 and +37 after a closing, 5/15
/// at +10 after a closing). The game times these by its own clock; the capture's frames are
/// sampled every ten, and one reopening comes a frame earlier than the other two.
// STAND-IN: the four delays are fitted to those alphas (each exact); the real thing is the help bar's own timer.
const LIST_HELP_OUT: i32 = 3;
const LIST_HELP_BACK: i32 = 33;
const DIALOG_HELP_IN: i32 = 33;
const DIALOG_HELP_OUT: i32 = 5;
/// The help plate shrinks as it fades out, over `PLATE_FRAMES`, from this many frames after the key.
const PLATE_SHRINK_AT: i32 = 3;
const PLATE_FRAMES: f32 = 6.4;
/// After A on Yes: the dialog slides out for `SLIDE` frames, the black covers the screen over the
/// next 30 (alpha (frame - key - 29) / 30), the notice fades in from `NOTICE_AT` and out from
/// `NOTICE_OUT` over the common fade, and the screen shows again until it leaves at `SAVED_AT`.
const COVER_FROM: i32 = 29;
const COVER_FRAMES: f32 = 30.0;
const NOTICE_AT: i32 = 31;
const NOTICE_OUT: i32 = 137;
// STAND-IN: the frame the saved screen is left at is the capture's (the list is back at +160 and the title at +164); the real thing is the save's own time.
const SAVED_AT: i32 = 163;
/// A tab change follows its dialog's answer after this many frames: No, the frame after the
/// dialog is out; Yes, the frame after the save is over (as the title follows a leave).
// STAND-IN: fitted to the capture settings-tabask (see docs/ryza/open.md); the real thing is the screen's own sequencing.
/// The list's help bar on the tab an answer turned to is at 7/15 of its opacity 9 frames after the turn.
// STAND-IN: fitted to that one alpha (settings-tabask 1660, the turn at 1651); the real thing is the bar's own timer.
const TAB_HELP_IN: i32 = 2;
const TAB_AFTER_NO: i32 = SLIDE + 1;
const TAB_AFTER_SAVE: i32 = SAVED_AT + 1;
/// The title comes back from black, 1/18 a frame (as the menu's cover rises), from the frame after the screen's last.
pub const RETURN_FADE: f32 = 18.0;

/// A box choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Yes,
    No,
}

/// What the dialog is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    /// B: the screen is left.
    Leave,
    /// LB (-1) or RB (1): the tab turns that many places.
    Tab(i32),
}

/// A tab change the answer let through, once its dialog is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Due {
    /// The frame the tab turns on.
    pub at: i32,
    pub way: i32,
    /// The answer was No: the changes are dropped before the tab turns.
    pub discard: bool,
    /// The frames the list's cursor stood still in, on the tab being left.
    pub stood: i32,
}

/// The cursor's blink on each tab. A tab's cursor blinks on its own clock, which runs while the
/// tab is shown and not while the dialog is up: it starts with the tab's first visit and goes on
/// from where it was when the tab is shown again (the capture settings-tabask: the Game tab,
/// left at 1530 for the dialog and the Audio tab, is back at 1700 with its clock at 82, the frames
/// it had run).
#[derive(Clone, Debug, PartialEq)]
pub struct Blinks {
    /// The frame the screen's age counts from (its first frame).
    from: i32,
    clocks: [Option<i32>; crate::options::TABS.len()],
    /// The shown tab's clock when it was last turned to, and that frame (none before the first turn).
    base: i32,
    since: Option<i32>,
}

impl Blinks {
    pub fn new(from: i32) -> Blinks {
        Blinks { from, clocks: [None; crate::options::TABS.len()], base: 0, since: None }
    }

    /// The tab `leaving` turns to `to` on frame `at`, its cursor having stood still `stood` frames.
    pub fn turn(&mut self, leaving: usize, to: usize, at: i32, stood: i32) {
        let clock = match self.since {
            None => at - self.from + 1 - stood,
            Some(s) => self.base + (at - s + 1 - stood),
        };
        self.clocks[leaving] = Some(clock.max(0));
        self.base = self.clocks[to].unwrap_or(0);
        self.since = Some(at);
    }

    /// The shown tab's blink age on `frame` (none until the first turn: the screen's age is it).
    pub fn age(&self, frame: i32, stood: i32) -> Option<i32> {
        self.since.map(|s| self.base + (frame - s + 1 - stood).max(0))
    }
}

/// A tab button on `frame`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Nothing was changed: the tab turns at once.
    Now,
    /// The dialog is up.
    Asked,
    /// The list takes no keys at the moment.
    Ignored,
}

/// A key the leave reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Back,
    Confirm,
    /// Up or down: the box has two choices, so both flip it.
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Idle,
    /// The dialog came on `at` (the frame of the B), the cursor on `on`.
    Asking { at: i32, on: Choice },
    /// The dialog (`opened` at) slides out from `at` and the list stays.
    Closing { opened: i32, at: i32, on: Choice },
    /// No: the dialog slides out from `at`, then the screen is left.
    Discarding { opened: i32, at: i32 },
    /// Yes: the changes are written at `at`, then the black, the notice, and the screen is left.
    Saving { opened: i32, at: i32 },
    /// Left with nothing to ask.
    Left { at: i32 },
}

/// How the screen is leaving, as keys go by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leave {
    phase: Phase,
    /// Frames the dialog held the list still in earlier asks (the cursor's blink stands while it is up).
    stood: i32,
    /// The key that closed the dialog last (the list's help bar comes back after it).
    closed: Option<i32>,
    /// What the dialog in play (or last asked) leads to.
    then: Then,
    /// The frame of the A on Yes, once pressed (it stays after a tab change).
    saved: Option<i32>,
    /// The tab change the answer let through, until [`Leave::take_due`] hands it over.
    due: Option<Due>,
}

impl Default for Leave {
    fn default() -> Leave {
        Leave { phase: Phase::Idle, stood: 0, closed: None, then: Then::Leave, saved: None, due: None }
    }
}

/// What a key came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// The changes are to be written (A on Yes).
    Save,
    /// The changes are to be dropped (A on No).
    Discard,
}

/// The dialog's look on a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dialog {
    /// How far the panel's slide has come, 0 to 1 (its alpha and rise are the animation's easing of it).
    pub slide: f32,
    /// The box's opacity (a straight fade).
    pub box_alpha: f32,
    pub on: Choice,
    /// The frames the cursor's colour animation has run.
    pub clock: f32,
    /// The cursor is plain white (the dialog is on its way out).
    pub white: bool,
}

/// A help bar's opacity and how far its plate has grown (0 to 1).
pub type Plate = (f32, f32);

/// Everything the screen shows of the leaving on a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    pub dialog: Option<Dialog>,
    /// The list's help bar when the dialog changes it (`None`: as the screen's age gives it).
    pub list_help: Option<Plate>,
    /// The bar with Select, Back and Confirm.
    pub dialog_help: Option<Plate>,
    /// The black over the screen.
    pub cover: f32,
    /// The "Saving..." notice: its opacity and its frames.
    pub notice: Option<(f32, i32)>,
    /// The screen is not drawn any more (the title is back); the frame it was last drawn on.
    pub left: Option<i32>,
}

impl Leave {
    /// The frame the changes are written (A on Yes), once it was pressed.
    pub fn saved_at(&self) -> Option<i32> {
        self.saved
    }

    /// The tab turned: the list's cursor has not stood still on the new one.
    pub fn new_tab(&mut self) {
        self.stood = 0;
    }

    /// The dialog leads to leaving the screen (not to a tab).
    pub fn leaving(&self) -> bool {
        self.then == Then::Leave
    }

    /// The tab change a finished answer lets through, once its frame is reached.
    pub fn take_due(&mut self, frame: i32) -> Option<Due> {
        self.settle(frame);
        self.due.take()
    }

    fn settle(&mut self, frame: i32) {
        if let Then::Tab(way) = self.then {
            // The tab turns when the dialog is out (No) or the save is over (Yes); the list is the screen's own again.
            let done = match self.phase {
                Phase::Discarding { opened, at } if frame >= at + TAB_AFTER_NO => Some((opened, at + TAB_AFTER_NO, true)),
                Phase::Saving { opened, at } if frame >= at + TAB_AFTER_SAVE => Some((opened, at + TAB_AFTER_SAVE, false)),
                _ => None,
            };
            if let Some((opened, at, discard)) = done {
                let stood = self.stood + at - opened;
                self.new_tab();
                // The new tab's help bar fades in from a couple of frames after the turn (the list's own bar after a dialog: `closed` plus its delay).
                self.closed = Some(at + TAB_HELP_IN - LIST_HELP_BACK);
                self.phase = Phase::Idle;
                self.due = Some(Due { at, way, discard, stood });
            }
        }
        match self.phase {
            Phase::Closing { opened, at, .. } if frame >= at + SLIDE => {
                self.stood += at + SLIDE - opened;
                self.closed = Some(at);
                self.phase = Phase::Idle;
            }
            _ => {}
        }
    }

    /// The frames up to `frame` the list's cursor stood still in: from the key that asked to the
    /// end of the dialog's slide out (the capture's list cursor keeps its opacity while the dialog
    /// is on and goes on from there once it is gone: settings-leave 1530 to 1650).
    pub fn stood(&self, frame: i32) -> i32 {
        let mut me = *self;
        me.settle(frame);
        match me.phase {
            Phase::Asking { at, .. } => me.stood + (frame - at).max(0),
            Phase::Closing { opened, at, .. } => me.stood + (frame.min(at + SLIDE) - opened).max(0),
            Phase::Discarding { opened, .. } | Phase::Saving { opened, .. } => me.stood + (frame - opened).max(0),
            _ => me.stood,
        }
    }

    /// A key on `frame`; `changed` is whether anything changed since the screen opened.
    pub fn press(&mut self, key: Key, changed: bool, frame: i32) -> Outcome {
        self.settle(frame);
        match (self.phase, key) {
            (Phase::Idle, Key::Back) => {
                self.then = Then::Leave;
                self.phase = if changed { Phase::Asking { at: frame, on: Choice::No } } else { Phase::Left { at: frame } };
            }
            (Phase::Asking { at, on }, Key::Back) => self.phase = Phase::Closing { opened: at, at: frame, on },
            (Phase::Asking { at, on }, Key::Toggle) => self.phase = Phase::Asking { at, on: if on == Choice::Yes { Choice::No } else { Choice::Yes } },
            (Phase::Asking { at: opened, on: Choice::Yes }, Key::Confirm) => {
                self.phase = Phase::Saving { opened, at: frame };
                self.saved = Some(frame);
                return Outcome::Save;
            }
            (Phase::Asking { at: opened, on: Choice::No }, Key::Confirm) => {
                self.phase = Phase::Discarding { opened, at: frame };
                return Outcome::Discard;
            }
            _ => {}
        }
        Outcome::Nothing
    }

    /// LB or RB on `frame`: the tab turns at once when nothing changed, else the dialog asks.
    pub fn tab(&mut self, way: i32, changed: bool, frame: i32) -> Tab {
        self.settle(frame);
        if self.phase != Phase::Idle {
            return Tab::Ignored;
        }
        if !changed {
            return Tab::Now;
        }
        self.then = Then::Tab(way);
        self.phase = Phase::Asking { at: frame, on: Choice::No };
        Tab::Asked
    }

    /// The dialog is up (keys go to it, not to the list).
    pub fn asking(&self) -> bool {
        matches!(self.phase, Phase::Asking { .. })
    }

    /// The list takes no keys: a dialog is up or on its way, or the screen is on its way out.
    pub fn busy(&self, frame: i32) -> bool {
        let mut me = *self;
        me.settle(frame);
        me.phase != Phase::Idle
    }

    /// Frames the title's clock runs ahead of the screen's last frame: after a B that asked nothing the
    /// title's fade and cursor are one frame on (the capture settings-tabask 2130 to 2160).
    // STAND-IN: one frame, read off those four frames; the dialogs' leaves are fitted without it.
    pub fn lead(&self) -> i32 {
        i32::from(matches!(self.phase, Phase::Left { .. }))
    }

    /// The frame the screen was last drawn on, once it is being left.
    pub fn left_at(&self) -> Option<i32> {
        match self.phase {
            _ if self.then != Then::Leave => None,
            Phase::Left { at } => Some(at),
            Phase::Discarding { at, .. } => Some(at + SLIDE),
            Phase::Saving { at, .. } => Some(at + SAVED_AT),
            _ => None,
        }
    }

    /// The screen's look on `frame`.
    pub fn look(&self, frame: i32) -> Look {
        let mut me = *self;
        me.settle(frame);
        let mut look = Look { left: me.left_at().filter(|&l| frame > l), ..Look::default() };
        let plate = |age: i32| ((age as f32 / FADE as f32).clamp(0.0, 1.0), (age as f32 / PLATE_FRAMES).clamp(0.0, 1.0));
        // Fading out: the alpha falls from `at + delay`, the plate shrinks from `PLATE_SHRINK_AT`.
        let out = |at: i32, delay: i32| -> Plate {
            let fade = 1.0 - ((frame - at - delay).max(0) as f32 / FADE as f32);
            let grow = 1.0 - (frame - at - PLATE_SHRINK_AT).max(0) as f32 / PLATE_FRAMES;
            (fade.clamp(0.0, 1.0), grow.clamp(0.0, 1.0))
        };
        let shown = |p: Plate| (p.0 > 0.0).then_some(p);
        match me.phase {
            Phase::Idle | Phase::Left { .. } => {
                if let Some(c) = me.closed {
                    look.list_help = Some(plate(frame - (c + LIST_HELP_BACK)));
                }
            }
            Phase::Asking { at, on } => {
                look.dialog = Some(Dialog { slide: ((frame - at + 1) as f32 / SLIDE as f32).clamp(0.0, 1.0), box_alpha: ((frame - at + 1) as f32 / FADE as f32).clamp(0.0, 1.0), on, clock: (frame - at + 1) as f32, white: false });
                // (the list's bar fades with its plate at full width)
                look.list_help = Some((out(at, LIST_HELP_OUT).0, 1.0));
                look.dialog_help = shown(plate(frame - (at + DIALOG_HELP_IN) + 1)).filter(|_| frame >= at + DIALOG_HELP_IN);
            }
            _ => {}
        }
        // The closing phases (B, and A on No / Yes) all slide the dialog out from `at`.
        let closing = match me.phase {
            Phase::Closing { opened, at, on } => Some((opened, at, on)),
            Phase::Discarding { opened, at } => Some((opened, at, Choice::No)),
            Phase::Saving { opened, at } => Some((opened, at, Choice::Yes)),
            _ => None,
        };
        if let Some((opened, at, on)) = closing {
            let age = frame - at + 1;
            // A dialog closed before it was all the way in goes out from where it was.
            let came = (at - opened).min(SLIDE - 1) + 1;
            let slide = ((came - age).max(0) as f32 / SLIDE as f32).min(1.0);
            if age <= SLIDE {
                look.dialog = Some(Dialog { slide, box_alpha: (1.0 - age as f32 / FADE as f32).max(0.0), on, clock: 0.0, white: true });
            }
            look.dialog_help = shown(out(at, DIALOG_HELP_OUT));
            let back = frame - (at + LIST_HELP_BACK);
            if matches!(me.phase, Phase::Closing { .. }) {
                // The list's bar comes back once the dialog is gone.
                look.list_help = Some(if back > 0 { plate(back) } else { (0.0, 0.0) });
            } else {
                look.list_help = Some((0.0, 0.0));
            }
        }
        if let Phase::Saving { at, .. } = me.phase {
            look.cover = ((frame - at - COVER_FROM) as f32 / COVER_FRAMES).clamp(0.0, 1.0);
            let age_in = frame - (at + NOTICE_AT);
            let alpha = if frame < at + NOTICE_OUT + 1 { (age_in as f32 / FADE as f32).clamp(0.0, 1.0) } else { (1.0 - (frame - at - NOTICE_OUT) as f32 / FADE as f32).clamp(0.0, 1.0) };
            if alpha > 0.0 {
                look.notice = Some((alpha, frame - (at + NOTICE_AT)));
            }
            // After its notice the screen shows again until it is left.
            if frame > at + NOTICE_OUT + FADE {
                look.cover = 0.0;
            }
        }
        look
    }
}

/// The scripted keys of an offscreen run, run again from the screen's first frame (`from`) to
/// `frame`: the state after them, how the screen is leaving, and the directions that went to the
/// list (the ones pressed while the dialog was up went to its box).
pub struct Replay {
    pub state: State,
    pub leave: Leave,
    pub holds: Vec<Hold>,
    /// The frame the tab last turned on.
    pub tab_change: Option<i32>,
    /// The shown tab's blink age, once the tab has turned (see [`Blinks`]).
    pub blink: Option<i32>,
}

/// Runs `input`'s scripted keys (tabs, changes, B, A and the directions) up to `frame` on a
/// screen that opened with `initial`.
pub fn replay(initial: &State, input: &Input, from: i32, frame: i32) -> Replay {
    #[derive(Clone, Copy)]
    enum Event {
        Tab(i32),
        Hold(usize),
        Change(i32),
        Back,
        Confirm,
    }
    let mut events: Vec<(i32, u8, Event)> = Vec::new();
    // At a frame the directions come first, then the tabs and the changes, then B and A.
    events.extend(input.list_holds.iter().enumerate().map(|(i, h)| (h.from, 0, Event::Hold(i))));
    events.extend(input.tabs.iter().map(|&(f, w)| (f, 1, Event::Tab(w))));
    events.extend(input.changes.iter().map(|&(f, w)| (f, 2, Event::Change(w))));
    events.extend(input.backs.iter().map(|&f| (f, 3, Event::Back)));
    events.extend(input.answers.iter().map(|&f| (f, 4, Event::Confirm)));
    events.retain(|e| e.0 >= from && e.0 <= frame);
    events.sort_by_key(|e| (e.0, e.1));
    let mut initial = initial.clone();
    let mut state = initial.clone();
    state.tab = crate::options::FIRST_TAB;
    let mut leave = Leave::default();
    let mut holds: Vec<Hold> = Vec::new();
    let mut tab_change: Option<i32> = None;
    // A tab change an answer let through turns where its dialog ends (the state is the screen's, not the first one's).
    let mut blinks = Blinks::new(from);
    let turn = |leave: &mut Leave, state: &mut State, initial: &State, tab_change: &mut Option<i32>, blinks: &mut Blinks, f: i32| {
        if let Some(due) = leave.take_due(f) {
            if due.discard {
                state.discard_changes(initial);
            }
            let was = state.tab;
            state.tab_by(due.way);
            blinks.turn(was, state.tab, due.at, due.stood);
            *tab_change = Some(due.at);
        }
    };
    for (f, _, event) in events {
        turn(&mut leave, &mut state, &initial, &mut tab_change, &mut blinks, f);
        match event {
            Event::Hold(i) => {
                if leave.busy(f) {
                    leave.press(Key::Toggle, state.changed, f);
                } else {
                    holds.push(input.list_holds[i]);
                }
            }
            Event::Tab(way) => match leave.tab(way, state.changed, f) {
                crate::leave::Tab::Now => {
                    let was = state.tab;
                    state.tab_by(way);
                    blinks.turn(was, state.tab, f, leave.stood(f));
                    leave.new_tab();
                    tab_change = Some(f);
                }
                _ => {}
            },
            Event::Change(way) if !leave.busy(f) => {
                let n = state.entries(state.tab).len().max(1);
                let held: Vec<Hold> = holds.iter().filter(|h| tab_change.is_none_or(|t| h.from >= t)).copied().collect();
                state.cursor[state.tab] = crate::options::cursor_on(&Input { list_holds: held, ..Input::default() }, 0, n, f);
                state.change(way);
            }
            Event::Back => {
                leave.press(Key::Back, state.changed, f);
            }
            Event::Confirm => {
                if leave.press(Key::Confirm, state.changed, f) == Outcome::Save {
                    initial = state.clone();
                    initial.changed = false;
                    state.changed = false;
                }
            }
            _ => {}
        }
    }
    turn(&mut leave, &mut state, &initial, &mut tab_change, &mut blinks, frame);
    let blink = blinks.age(frame, leave.stood(frame));
    Replay { state, leave, holds, tab_change, blink }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::Settings;
    use barrel_render::settings::RenderSettings;

    #[test]
    fn b_with_nothing_changed_leaves_at_once() {
        let mut l = Leave::default();
        assert_eq!(l.press(Key::Back, false, 100), Outcome::Nothing);
        assert_eq!(l.left_at(), Some(100));
        assert!(l.look(100).dialog.is_none());
    }

    #[test]
    fn b_after_a_change_asks_on_no_and_b_closes_it() {
        let mut l = Leave::default();
        l.press(Key::Back, true, 100);
        let d = l.look(100).dialog.unwrap();
        assert_eq!(d.on, Choice::No);
        assert!((d.slide - 1.0 / 30.0).abs() < 1e-6 && (d.box_alpha - 1.0 / 15.0).abs() < 1e-6);
        l.press(Key::Toggle, true, 120);
        assert_eq!(l.look(120).dialog.unwrap().on, Choice::Yes);
        l.press(Key::Toggle, true, 121);
        assert_eq!(l.look(121).dialog.unwrap().on, Choice::No);
        // B slides it out over 30 frames and the list takes keys again.
        l.press(Key::Back, true, 200);
        assert!(l.busy(220) && !l.busy(230));
        assert!(l.look(229).dialog.is_some() && l.look(230).dialog.is_none());
        // ... and B opens it once more.
        l.press(Key::Back, true, 300);
        assert!(l.asking());
    }

    #[test]
    fn a_on_no_discards_and_a_on_yes_saves() {
        let mut l = Leave::default();
        l.press(Key::Back, true, 10);
        assert_eq!(l.press(Key::Confirm, true, 50), Outcome::Discard);
        assert_eq!(l.left_at(), Some(80));
        let mut l = Leave::default();
        l.press(Key::Back, true, 10);
        l.press(Key::Toggle, true, 20);
        assert_eq!(l.press(Key::Confirm, true, 50), Outcome::Save);
        // The black rises 30 frames after the dialog is out; the notice comes over it.
        assert_eq!(l.look(79).cover, 0.0);
        assert!((l.look(80).cover - 1.0 / 30.0).abs() < 1e-6);
        assert_eq!(l.look(110).cover, 1.0);
        assert!(l.look(80).notice.is_none() && l.look(90).notice.is_some());
        assert_eq!(l.left_at(), Some(50 + SAVED_AT));
    }

    #[test]
    fn the_replay_changes_the_row_under_the_cursor_and_sends_directions_to_the_box() {
        let s = State::new(Settings::default(), RenderSettings::DESKTOP);
        let input = Input {
            changes: vec![(100, 1)],
            backs: vec![110],
            list_holds: vec![Hold { from: 150, until: Some(156), way: 1 }, Hold { from: 20, until: Some(26), way: 1 }],
            answers: vec![210],
            ..Input::default()
        };
        // Before the first B: the hold at 20 moved the cursor to the second row of the Game tab (Difficulty), and the change at 100 is its.
        let r = replay(&s, &input, 0, 105);
        assert!(r.state.changed && r.state.option("difficulty") != s.option("difficulty") && r.state.option("vibration") == s.option("vibration"));
        // The hold at 150 came while the dialog was up: it is the box's, and A on Yes saves.
        let r = replay(&s, &input, 0, 205);
        assert!(r.leave.asking() && r.holds.len() == 1);
        assert_eq!(r.leave.look(205).dialog.map(|d| d.on), Some(Choice::Yes));
        let r = replay(&s, &input, 0, 215);
        assert_eq!(r.leave.left_at(), Some(210 + SAVED_AT));
    }

    #[test]
    fn a_tab_button_turns_the_tab_at_once_unless_something_changed() {
        let mut l = Leave::default();
        assert_eq!(l.tab(1, false, 10), Tab::Now);
        assert!(!l.asking());
        assert_eq!(l.tab(1, true, 20), Tab::Asked);
        assert!(l.asking() && !l.leaving());
        assert_eq!(l.look(20).dialog.map(|d| d.on), Some(Choice::No));
        // Another tab button while the dialog is up goes nowhere.
        assert_eq!(l.tab(-1, true, 30), Tab::Ignored);
    }

    #[test]
    fn no_on_a_tab_ask_drops_the_changes_and_turns_the_tab_after_the_dialog() {
        let mut l = Leave::default();
        l.tab(1, true, 100);
        assert_eq!(l.press(Key::Confirm, true, 120), Outcome::Discard);
        assert_eq!(l.left_at(), None);
        assert_eq!(l.take_due(120 + TAB_AFTER_NO - 1), None);
        let due = l.take_due(120 + TAB_AFTER_NO).expect("the tab turns");
        assert_eq!((due.way, due.discard, due.at), (1, true, 120 + TAB_AFTER_NO));
        // The list is its own again, and the turn comes once.
        assert!(!l.busy(120 + TAB_AFTER_NO));
        assert_eq!(l.take_due(400), None);
    }

    #[test]
    fn yes_on_a_tab_ask_saves_and_turns_the_tab_when_the_save_is_over() {
        let mut l = Leave::default();
        l.tab(-1, true, 100);
        l.press(Key::Toggle, true, 110);
        assert_eq!(l.press(Key::Confirm, true, 120), Outcome::Save);
        assert_eq!(l.saved_at(), Some(120));
        assert_eq!(l.take_due(120 + TAB_AFTER_SAVE - 1), None);
        assert!(l.busy(120 + 100));
        let due = l.take_due(120 + TAB_AFTER_SAVE).expect("the tab turns");
        assert_eq!((due.way, due.discard), (-1, false));
        // The save stays on record after the tab turned.
        assert_eq!(l.saved_at(), Some(120));
    }

    #[test]
    fn b_on_a_tab_ask_closes_it_and_the_tab_stays() {
        let mut l = Leave::default();
        l.tab(1, true, 100);
        l.press(Key::Back, true, 110);
        assert_eq!(l.take_due(500), None);
        assert!(!l.busy(500));
    }

    #[test]
    fn a_tab_keeps_its_own_blink_clock_while_it_is_away() {
        // Screen from 0; the first tab runs 100 frames (20 of them held still), then another is shown for 50, then back.
        let mut b = Blinks::new(0);
        assert_eq!(b.age(10, 0), None);
        b.turn(1, 2, 100, 20);
        assert_eq!(b.age(100, 0), Some(1));
        assert_eq!(b.age(110, 0), Some(11));
        b.turn(2, 1, 150, 0);
        // The first tab went on from its 81 (100 + 1 - 20).
        assert_eq!(b.age(150, 0), Some(82));
    }

    #[test]
    fn the_replay_turns_the_tab_after_a_no_and_after_a_save() {
        let s = State::new(Settings::default(), RenderSettings::DESKTOP);
        let input = Input { changes: vec![(100, 1)], tabs: vec![(110, 1), (400, 1)], answers: vec![150, 300], ..Input::default() };
        // The ask on the first tab button, the cursor on No: A throws the change away, the tab turns after the dialog.
        let r = replay(&s, &input, 0, 140);
        assert!(r.leave.asking() && r.state.tab == s.tab && r.state.changed);
        let r = replay(&s, &input, 0, 150 + TAB_AFTER_NO);
        assert_eq!((r.state.tab, r.state.changed), (s.tab + 1, false));
        assert_eq!(r.state.option("vibration"), s.option("vibration"));
        assert_eq!(r.tab_change, Some(150 + TAB_AFTER_NO));
        assert!(!r.leave.busy(150 + TAB_AFTER_NO));
        // Nothing changed now: the next tab button turns at once.
        let r = replay(&s, &input, 0, 400);
        assert_eq!(r.state.tab, s.tab + 2);
    }
}
