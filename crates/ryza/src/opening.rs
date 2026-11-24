//! The opening: the story's first event (`event_mm00_010`), which New Game on the title starts
//! (docs/ryza/open.md, "The opening (P5b)"; the answer key is ryza-recomp's `opening-key`).

use crate::new_game;

/// The event the title's New Game starts.
pub const EVENT: &str = "mm00_010";

/// The capture frame the event's first command runs on: the button's frame (1535) and
/// [`new_game::OPENING_AFTER`].
pub const START: i32 = 1535 + new_game::OPENING_AFTER;

/// The event's stage clock, in seconds, for the picture drawn on capture frame `frame`: the
/// picture shows the stage a tick on from the commands' own frame (as the title's event does,
/// `play::actors::EVENT_START`).
pub fn stage_seconds(frame: i32) -> f32 {
    (frame - START + 1) as f32 / 60.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_events_clock() {
        assert_eq!(START, 1600);
        assert_eq!(stage_seconds(START), 1.0 / 60.0);
        // a frame's step before its first telop's fade-in (1819 is two seconds in)
        assert_eq!(stage_seconds(1819), 120.0 / 60.0 + 1.0 / 60.0);
    }
}
