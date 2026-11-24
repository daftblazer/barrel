//! The Options screen, one implementation for both doors to it. [`Screen`] is the screen itself (the
//! values, the leaving dialog, the cursor's blink) and what keys do to it; the title
//! (`play::run_once`, on the title's clock, its cursor from the hold record, its offscreen keys
//! replayed by `leave::replay`) and the field's menu ([`Run`], on a clock of its own, its cursor
//! stepped by keys) each drive a `Screen` and keep only what differs.

use crate::leave::{Blinks, Key as LeaveKey, Leave, Look, Outcome, Tab};
use crate::options::{self, State};
use crate::options_view::Moment;

/// A key on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A press of up or down: the cursor wraps at the list's ends (or the dialog's box turns).
    Up,
    Down,
    /// A held direction's repeat: the cursor stops at the ends.
    RepeatUp,
    RepeatDown,
    Left,
    Right,
    Confirm,
    Back,
    TabPrev,
    TabNext,
}

/// A sound a frame asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Cursor,
    Cancel,
    Choose,
    Page,
}

impl Cue {
    /// The sound's name and its effect id.
    pub fn sound(self) -> (&'static str, u32) {
        match self {
            Cue::Cursor => ("cursor", crate::sound::CURSOR),
            Cue::Cancel => ("cancel", crate::sound::CANCEL),
            Cue::Choose => ("choose", crate::sound::CLICK),
            Cue::Page => ("page", crate::sound::PAGE),
        }
    }
}

/// What a frame did outside the screen.
#[derive(Debug, Default, PartialEq)]
pub struct Effects {
    pub cues: Vec<Cue>,
    /// A on Yes: the changes are to be written and applied.
    pub saved: bool,
    /// The frame a tab turned on (the new tab's cursor starts again).
    pub turned: Option<i32>,
}

/// What the Options screen is, whoever drives it: the values, the leaving dialog, the blink. The
/// title and the field's menu both feed it the keys of a frame ([`Screen::press`]) and let it
/// settle ([`Screen::settle`]); each keeps what is its own: where the keys come from, the clock,
/// and how the list's cursor moves (the title's from its hold record, the menu's [`Run`] by steps).
#[derive(Clone, Debug)]
pub struct Screen {
    pub state: State,
    /// The values when the screen opened (or were last saved): A on No goes back to them.
    pub initial: State,
    pub leave: Leave,
    pub blinks: Blinks,
    /// The screen is being left with the changes dropped.
    pub discard: bool,
}

impl Screen {
    /// A screen on `state`, its blink counted from frame `from`.
    pub fn new(state: State, from: i32) -> Screen {
        Screen { initial: state.clone(), state, leave: Leave::default(), blinks: Blinks::new(from), discard: false }
    }

    /// The screen opens again on the values it has: nothing changed, the dialog and blink fresh.
    pub fn enter(&mut self, from: i32) {
        self.state.changed = false;
        self.initial = self.state.clone();
        self.leave = Leave::default();
        self.blinks = Blinks::new(from);
        self.discard = false;
    }

    /// The values are `state` now, with nothing to go back to.
    pub fn set(&mut self, state: State) {
        self.initial = state.clone();
        self.state = state;
    }

    /// The list takes keys: no dialog is up or on its way and the screen is not on its way out.
    pub fn takes_keys(&self, frame: i32) -> bool {
        !self.leave.asking() && !self.leave.busy(frame)
    }

    /// The keys pressed on `frame`. The dialog takes up and down (its box), B and A while it is up;
    /// otherwise left and right change the row's value, the tab keys turn the tab (the dialog asks
    /// first when something changed) and B leaves. The cursor's own keys are the caller's.
    pub fn press(&mut self, frame: i32, keys: &[Key], fx: &mut Effects) {
        let has = |k: Key| keys.contains(&k);
        if self.leave.asking() {
            if has(Key::Up) || has(Key::Down) {
                self.leave.press(LeaveKey::Toggle, self.state.changed, frame);
                fx.cues.push(Cue::Cursor);
            }
            if has(Key::Back) {
                self.leave.press(LeaveKey::Back, self.state.changed, frame);
                fx.cues.push(Cue::Cancel);
            } else if has(Key::Confirm) {
                fx.cues.push(Cue::Choose);
                match self.leave.press(LeaveKey::Confirm, self.state.changed, frame) {
                    Outcome::Save => {
                        fx.saved = true;
                        self.state.changed = false;
                        self.initial = self.state.clone();
                    }
                    Outcome::Discard => self.discard = self.leave.leaving(),
                    Outcome::Nothing => {}
                }
            }
        } else if !self.leave.busy(frame) {
            for (key, way) in [(Key::Left, -1), (Key::Right, 1)] {
                if has(key) {
                    self.state.change(way);
                }
            }
            for (key, way) in [(Key::TabPrev, -1), (Key::TabNext, 1)] {
                if has(key) {
                    if self.leave.tab(way, self.state.changed, frame) == Tab::Now {
                        self.turn_tab(frame, way, self.leave.stood(frame), fx);
                        self.leave.new_tab();
                    } else {
                        fx.cues.push(Cue::Cancel);
                    }
                }
            }
            if has(Key::Back) {
                self.leave.press(LeaveKey::Back, self.state.changed, frame);
                fx.cues.push(Cue::Cancel);
            }
        }
    }

    /// What `frame` brings by itself: the tab change an answered dialog lets through.
    pub fn settle(&mut self, frame: i32, fx: &mut Effects) {
        if let Some(due) = self.leave.take_due(frame) {
            if due.discard {
                self.state.discard_changes(&self.initial);
            }
            self.turn_tab(due.at, due.way, due.stood, fx);
        }
    }

    /// The frame the screen was last drawn on, once its leaving is over.
    pub fn left(&self, frame: i32) -> Option<i32> {
        self.leave.look(frame).left
    }

    /// The screen is over: the changes the player did not keep are dropped.
    pub fn finish_leaving(&mut self) {
        if self.discard {
            self.state = self.initial.clone();
            self.discard = false;
        }
    }

    /// The age the cursor's blink runs on at `frame`, `age` frames into the screen: it stands still while the dialog is up.
    pub fn blink_age(&self, frame: i32, age: i32) -> i32 {
        let stood = self.leave.stood(frame);
        self.blinks.age(frame, stood).unwrap_or(age - stood)
    }

    fn turn_tab(&mut self, at: i32, way: i32, stood: i32, fx: &mut Effects) {
        let was = self.state.tab;
        self.state.tab_by(way);
        self.blinks.turn(was, self.state.tab, at, stood);
        fx.turned = Some(at);
        fx.cues.push(Cue::Page);
    }
}

/// The screen from its first frame (frame 1) until it is left, on a clock of its own.
#[derive(Clone, Debug)]
pub struct Run {
    pub screen: Screen,
    /// The frames stepped so far (the screen's age).
    frame: i32,
    left: Option<i32>,
}

impl Run {
    /// The screen opening on `state` (the values it shows; the first tab, the cursors at the top).
    pub fn open(mut state: State) -> Run {
        state.changed = false;
        state.tab = options::FIRST_TAB;
        state.cursor = [0; options::TABS.len()];
        state.first = [0; options::TABS.len()];
        Run { screen: Screen::new(state, 1), frame: 0, left: None }
    }

    /// The frames the screen has run.
    pub fn frame(&self) -> i32 {
        self.frame
    }

    /// The frame the screen was last drawn on, once it is being left.
    pub fn left_at(&self) -> Option<i32> {
        self.screen.leave.left_at()
    }

    /// The screen is over: it was left on a frame before this one.
    pub fn is_over(&self) -> bool {
        self.left.is_some()
    }

    /// The values after the screen: the changes the player did not keep dropped.
    pub fn finish(mut self) -> State {
        self.screen.finish_leaving();
        self.screen.state
    }

    /// Runs one frame with the keys pressed on it. The first frame's keys count (the cover before
    /// it takes none: the caller starts the run on its first frame).
    pub fn step(&mut self, keys: &[Key]) -> Effects {
        self.frame += 1;
        let frame = self.frame;
        let mut fx = Effects::default();
        if self.left.is_some() {
            return fx;
        }
        if self.screen.takes_keys(frame) {
            for (key, way) in [(Key::Up, -1), (Key::Down, 1), (Key::RepeatUp, -1), (Key::RepeatDown, 1)] {
                if keys.contains(&key) {
                    self.move_cursor(way, matches!(key, Key::RepeatUp | Key::RepeatDown));
                    fx.cues.push(Cue::Cursor);
                }
            }
        }
        self.screen.press(frame, keys, &mut fx);
        self.screen.settle(frame, &mut fx);
        // Left once the leaving is over.
        if self.screen.left(frame).is_some() {
            self.left = Some(frame);
        }
        fx
    }

    fn move_cursor(&mut self, way: i32, repeat: bool) {
        let state = &mut self.screen.state;
        if repeat {
            let n = state.entries(state.tab).len().max(1) as i32;
            let tab = state.tab;
            state.cursor[tab] = (state.cursor[tab] as i32 + way).clamp(0, n - 1) as usize;
        } else {
            state.step(way);
        }
        state.follow();
    }

    /// The leaving's look on the frame stepped last.
    pub fn look(&self) -> Look {
        self.screen.leave.look(self.frame)
    }

    /// The moment the view draws: the age and the cursor's blink (it stands still while the dialog is up).
    pub fn moment(&self) -> Moment {
        let frame = self.frame;
        // (B with nothing changed: the cursor stands still from the key's own frame, which is the last drawn: `field-menu` 2400)
        let still = i32::from(self.screen.leave.left_at() == Some(frame));
        let age = self.screen.blink_age(frame, frame) - still;
        Moment { age: frame, blink: options::blink_clock(age, options::blink_speed(self.screen.state.tab)), from_menu: true }
    }

    /// Whether a value changed since the screen opened or was last saved.
    pub fn changed(&self) -> bool {
        self.screen.state.changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::Settings;
    use barrel_render::settings::RenderSettings;

    fn run() -> Run {
        Run::open(State::new(Settings::default(), RenderSettings::DESKTOP))
    }

    #[test]
    fn b_with_nothing_changed_leaves_on_that_frame() {
        let mut r = run();
        r.step(&[]);
        assert!(!r.is_over());
        r.step(&[Key::Back]);
        assert_eq!(r.left_at(), Some(2));
        assert!(!r.is_over(), "the screen is drawn on its last frame");
        r.step(&[]);
        assert!(r.is_over());
    }

    #[test]
    fn the_cursor_stands_still_on_the_frame_b_leaves() {
        let mut r = run();
        for _ in 0..10 {
            r.step(&[]);
        }
        let before = r.moment().blink;
        r.step(&[Key::Back]);
        // (the blink would have moved on by a frame's step; on the leaving frame it has not)
        assert_eq!(r.moment().age, 11);
        let mut other = run();
        for _ in 0..11 {
            other.step(&[]);
        }
        assert_ne!(r.moment().blink, other.moment().blink);
        assert!(before != other.moment().blink);
    }

    #[test]
    fn a_change_asks_before_leaving_and_no_drops_it() {
        let mut r = run();
        let before = r.screen.state.clone();
        r.step(&[]);
        r.step(&[Key::Right]);
        assert!(r.changed());
        r.step(&[Key::Back]);
        assert!(r.screen.leave.asking());
        // A on No (the box starts on No) drops the changes and slides out.
        r.step(&[Key::Confirm]);
        for _ in 0..crate::leave::SLIDE + 2 {
            r.step(&[]);
        }
        assert!(r.is_over());
        assert_eq!(r.finish(), before);
    }

    #[test]
    fn a_tab_with_a_change_asks_and_turns_after_the_answer() {
        let mut r = run();
        r.step(&[Key::Right]);
        let tab = r.screen.state.tab;
        let fx = r.step(&[Key::TabNext]);
        assert_eq!(fx.cues, vec![Cue::Cancel]);
        assert_eq!(r.screen.state.tab, tab);
        r.step(&[Key::Confirm]);
        for _ in 0..crate::leave::SLIDE + 2 {
            r.step(&[]);
        }
        assert_ne!(r.screen.state.tab, tab);
        assert!(!r.is_over());
    }

    #[test]
    fn the_cursor_wraps_on_a_press_and_stops_on_a_repeat() {
        let mut r = run();
        r.step(&[Key::Up]);
        let n = r.screen.state.entries(r.screen.state.tab).len();
        assert_eq!(r.screen.state.cursor[r.screen.state.tab], n - 1);
        r.step(&[Key::RepeatDown]);
        assert_eq!(r.screen.state.cursor[r.screen.state.tab], n - 1);
    }

    #[test]
    fn the_title_replay_and_the_menu_run_take_a_key_script_to_the_same_screen() {
        use crate::title_menu::Input;
        let s = State::new(Settings::default(), RenderSettings::DESKTOP);
        // A change, the tab key asking, A on No (the change goes, the tab turns), the tab key again, then B.
        let script: [(i32, Key); 5] = [(5, Key::Right), (10, Key::TabNext), (20, Key::Confirm), (200, Key::TabNext), (260, Key::Back)];
        let input = Input { changes: vec![(5, 1)], tabs: vec![(10, 1), (200, 1)], answers: vec![20], backs: vec![260], ..Input::default() };
        let mut r = Run::open(s.clone());
        let mut frame = 0;
        for at in [9, 19, 60, 199, 259, 300] {
            while frame < at {
                frame += 1;
                let keys: Vec<Key> = script.iter().filter(|k| k.0 == frame).map(|k| k.1).collect();
                r.step(&keys);
            }
            let t = crate::leave::replay(&s, &input, 0, at);
            assert_eq!((t.state.tab, t.state.changed, t.leave.asking(), t.leave.left_at()), (r.screen.state.tab, r.screen.state.changed, r.screen.leave.asking(), r.screen.leave.left_at()), "frame {at}");
            assert_eq!(t.state.option("vibration"), r.screen.state.option("vibration"), "frame {at}");
        }
    }
}
