//! What the player does on the screens before the field, as menu actions: the keyboard and the
//! pad both feed one [`MenuInput`], so a screen asks "was Back pressed" without knowing which.

/// A menu action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    TabPrev,
    TabNext,
}

/// A set of actions (one bit each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actions(u8);

impl Actions {
    pub fn with(self, a: Action, on: bool) -> Actions {
        let bit = 1 << a as u8;
        Actions(if on { self.0 | bit } else { self.0 & !bit })
    }

    pub fn has(self, a: Action) -> bool {
        self.0 >> a as u8 & 1 != 0
    }
}

impl std::ops::BitOr for Actions {
    type Output = Actions;
    fn bitor(self, o: Actions) -> Actions {
        Actions(self.0 | o.0)
    }
}

/// The actions down and pressed on this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuInput {
    held: Actions,
    pressed: Actions,
    /// What the pad held last time (its presses are the new bits; the keyboard brings its own).
    pad_before: Actions,
    /// Any pad button went down this frame (it ends "Press any button").
    pub any_pad: bool,
}

impl MenuInput {
    /// Takes one frame: the keyboard's presses since the last frame and the keys down now, and
    /// what the pad holds now (its buttons, and its stick past the dead zone).
    pub fn frame(&mut self, key_presses: Actions, keys_down: Actions, pad_down: Actions, any_pad: bool) {
        let new_on_pad = Actions(pad_down.0 & !self.pad_before.0);
        self.pressed = key_presses | new_on_pad;
        self.held = keys_down | pad_down;
        self.pad_before = pad_down;
        self.any_pad = any_pad;
    }

    pub fn pressed(&self, a: Action) -> bool {
        self.pressed.has(a)
    }

    pub fn held(&self, a: Action) -> bool {
        self.held.has(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_presses_are_edges_and_keys_come_as_given() {
        let mut m = MenuInput::default();
        let down = Actions::default().with(Action::Down, true);
        m.frame(Actions::default(), Actions::default(), down, false);
        assert!(m.pressed(Action::Down) && m.held(Action::Down));
        m.frame(Actions::default(), Actions::default(), down, false);
        assert!(!m.pressed(Action::Down) && m.held(Action::Down));
        m.frame(down, Actions::default(), Actions::default(), false);
        assert!(m.pressed(Action::Down) && !m.held(Action::Down), "a key tapped between two frames");
    }
}
