//! The Settings screen as a run of its own: opened from the field's menu, it keeps its own clock
//! (frames since its first frame) and takes the keys of each frame, as the title's screen does
//! with the title's clock (`play::run_once`). The same state ([`State`]), leaving ([`Leave`]) and
//! blink ([`Blinks`]) rules run here, so both doors lead to one screen.

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

/// What a frame did outside the screen.
#[derive(Debug, Default, PartialEq)]
pub struct Effects {
    pub cues: Vec<Cue>,
    /// A on Yes: the changes are to be written and applied.
    pub saved: bool,
}

/// The screen from its first frame (frame 1) until it is left.
#[derive(Clone, Debug)]
pub struct Run {
    pub state: State,
    initial: State,
    leave: Leave,
    blinks: Blinks,
    discard: bool,
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
        Run { initial: state.clone(), state, leave: Leave::default(), blinks: Blinks::new(1), discard: false, frame: 0, left: None }
    }

    /// The frames the screen has run.
    pub fn frame(&self) -> i32 {
        self.frame
    }

    /// The frame the screen was last drawn on, once it is being left.
    pub fn left_at(&self) -> Option<i32> {
        self.leave.left_at()
    }

    /// The screen is over: it was left on a frame before this one.
    pub fn is_over(&self) -> bool {
        self.left.is_some()
    }

    /// The values after the screen: the changes the player did not keep dropped.
    pub fn finish(self) -> State {
        if self.discard { self.initial } else { self.state }
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
        let has = |k: Key| keys.contains(&k);
        if self.leave.asking() {
            // The dialog's box: up and down move its cursor, A answers, B closes it.
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
            for (key, way) in [(Key::Up, -1), (Key::Down, 1), (Key::RepeatUp, -1), (Key::RepeatDown, 1)] {
                if has(key) {
                    self.move_cursor(way, matches!(key, Key::RepeatUp | Key::RepeatDown));
                    fx.cues.push(Cue::Cursor);
                }
            }
            for (key, way) in [(Key::Left, -1), (Key::Right, 1)] {
                if has(key) {
                    self.state.change(way);
                }
            }
            for (key, way) in [(Key::TabPrev, -1), (Key::TabNext, 1)] {
                if has(key) {
                    // With something changed the dialog asks first; the tab turns once it is answered.
                    if self.leave.tab(way, self.state.changed, frame) == Tab::Now {
                        self.turn_tab(frame, way, self.leave.stood(frame), &mut fx);
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
        if let Some(due) = self.leave.take_due(frame) {
            if due.discard {
                self.state.discard_changes(&self.initial);
            }
            self.turn_tab(due.at, due.way, due.stood, &mut fx);
        }
        // Left once the leaving is over.
        if self.leave.look(frame).left.is_some() {
            self.left = Some(frame);
        }
        fx
    }

    fn move_cursor(&mut self, way: i32, repeat: bool) {
        if repeat {
            let n = self.state.entries(self.state.tab).len().max(1) as i32;
            let tab = self.state.tab;
            self.state.cursor[tab] = (self.state.cursor[tab] as i32 + way).clamp(0, n - 1) as usize;
        } else {
            self.state.step(way);
        }
        self.state.follow();
    }

    fn turn_tab(&mut self, at: i32, way: i32, stood: i32, fx: &mut Effects) {
        let was = self.state.tab;
        self.state.tab_by(way);
        self.blinks.turn(was, self.state.tab, at, stood);
        fx.cues.push(Cue::Page);
    }

    /// The leaving's look on the frame stepped last.
    pub fn look(&self) -> Look {
        self.leave.look(self.frame)
    }

    /// The moment the view draws: the age and the cursor's blink (it stands still while the dialog is up).
    pub fn moment(&self) -> Moment {
        let frame = self.frame;
        let stood = self.leave.stood(frame);
        // (B with nothing changed: the cursor stands still from the key's own frame, which is the last drawn: `field-menu` 2400)
        let still = i32::from(self.leave.left_at() == Some(frame));
        let age = self.blinks.age(frame, stood).unwrap_or(frame - stood) - still;
        Moment { age: frame, blink: options::blink_clock(age, options::blink_speed(self.state.tab)), from_menu: true }
    }

    /// The tab a key turns to is the state's; whether a change is unsaved.
    pub fn changed(&self) -> bool {
        self.state.changed
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
    fn a_change_asks_before_leaving_and_no_drops_it() {
        let mut r = run();
        let before = r.state.clone();
        r.step(&[]);
        r.step(&[Key::Right]);
        assert!(r.changed());
        r.step(&[Key::Back]);
        assert!(r.leave.asking());
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
        let tab = r.state.tab;
        let fx = r.step(&[Key::TabNext]);
        assert_eq!(fx.cues, vec![Cue::Cancel]);
        assert_eq!(r.state.tab, tab);
        r.step(&[Key::Confirm]);
        for _ in 0..crate::leave::SLIDE + 2 {
            r.step(&[]);
        }
        assert_ne!(r.state.tab, tab);
        assert!(!r.is_over());
    }

    #[test]
    fn the_cursor_wraps_on_a_press_and_stops_on_a_repeat() {
        let mut r = run();
        r.step(&[Key::Up]);
        let n = r.state.entries(r.state.tab).len();
        assert_eq!(r.state.cursor[r.state.tab], n - 1);
        r.step(&[Key::RepeatDown]);
        assert_eq!(r.state.cursor[r.state.tab], n - 1);
    }
}
