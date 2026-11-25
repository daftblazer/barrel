//! The opening: the story's first event (`event_mm00_010`), which New Game on the title starts
//! (docs/ryza/open.md, "The opening (P5b)"; the answer key is ryza-recomp's `opening-key`).

use crate::event::{self, Taken, Talk};
use crate::new_game;
use barrel_engine::event::{Buttons, Timed};
use barrel_formats::ebd::Ebd;

/// The event the title's New Game starts.
pub const EVENT: &str = "mm00_010";

/// The capture frame the event's first command runs on: the button's frame (1535) and
/// [`new_game::OPENING_AFTER`].
pub const START: i32 = 1535 + new_game::OPENING_AFTER;

/// The event that follows the opening's last fade (the game's help-page event: the first line over
/// the stage, then two pictures' worth of explanation; `opening-key3`, 13846 on).
pub const FOLLOWING: &str = "xx01_150";

/// The help event's half-second fades hold it 33 frames (14047 to 14080, 14110 to 14143, 14829 to 14862 in the key) where the opening's hold
/// 32: one frame more. STAND-IN: the rule behind the difference (the game's own test of a fade's end) is not read.
pub const HELP_FADE_LATE: f32 = 1.0;

/// The frames the following event starts after the opening's last command's wait: its first commands run
/// a frame after the queue ends (the last fade, preset 3 half a second, is on 13813 and holds
/// [`barrel_engine::event::fade_hold`]; the next event's first commands are on 13846 in the key).
pub const HAND_OVER_FRAMES: f32 = 1.0;

/// Both events as one run: the opening's steps, then the following event's from where the opening ends, each
/// with the commands taken. `buttons` hold the presses in seconds from the opening's first command; the
/// following event gets the same presses from its own start. A following event behind a line still
/// waiting for its press is laid out at infinity.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn schedule<'a>(
    install: &'a crate::install::Install,
    opening: &'a Ebd,
    following: &'a Ebd,
    messages: [&'a [barrel_formats::ebm::Message]; 2],
    buttons: Option<Buttons>,
    given: &[f32],
    clock: u32,
    until: f32,
) -> Result<(Vec<Timed<'a>>, Vec<Taken>, f32), String> {
    let talk = |m, b, late| Talk { messages: m, buttons: b, fade_late: late };
    let (mut steps, mut taken) = event::schedule_drawn_taken_with(opening, given, clock, until, Some(event::motion_lengths(install)), talk(messages[0], buttons.clone(), 0.0))?;
    // the opening is over when its last command's wait is (it ends on a fade of half a second)
    let last = taken.iter().map(|t| t.0).fold(0.0f32, f32::max);
    let end = last + barrel_engine::event::fade_hold(0.5) + HAND_OVER_FRAMES / 60.0;
    let then = buttons.map(|b| Buttons { presses: b.presses.iter().map(|p| p - end).collect(), ..b });
    let (next_steps, next_taken) = event::schedule_drawn_taken_with(following, &[], clock, until, Some(event::motion_lengths(install)), talk(messages[1], then, HELP_FADE_LATE))?;
    let shift = opening.actions.len();
    steps.extend(next_steps.into_iter().map(|t| Timed { time: t.time + end, action: t.action + shift, step: t.step }));
    taken.extend(next_taken.into_iter().map(|(time, action, words)| (time + end, action + shift, words)));
    Ok((steps, taken, end))
}

/// The capture frame the field's HUD arrives on after the help event (its last command's wait ends on 14861; the help bar
/// grows from the next frame, `opening-key3`).
pub const FREE_PLAY: i32 = 14862;

/// The field the opening and the free play after it are in (`FIELDMAP_S06_T_RASEN`, the event's `q2` 5).
pub const FIELD_MAP: u32 = 5;

/// The basket's count of a new game (`opening-key3` 15255 on; a save's own is in its data).
pub const BASKET: u32 = 200;

/// Where she stands when the free play begins and which way she faces (degrees): the opening's last `q5` for her slot (the key's 13779).
pub fn player_start(install: &crate::install::Install) -> Result<([f32; 3], f32), String> {
    let path = event::path(EVENT).ok_or("opening event")?;
    let ebd = Ebd::parse(&install.read(&path).map_err(|e| e.to_string())?).map_err(|e| format!("{path}: {e}"))?;
    let added = event::added(&ebd)?;
    let her = added.get(&0).ok_or("the opening adds no player")?;
    Ok((her.position, her.degrees[1]))
}

/// The story at the free play's start: the opening has played (the minimap's marker points on from it).
pub fn free_play(_install: &crate::install::Install, story: &mut crate::story::Story) -> Result<(), String> {
    story.finish("MM00_010");
    Ok(())
}

/// When the help event is over and the field's HUD arrives, in seconds from the opening's first command: its last command (a fade of half
/// a second) has held its event (infinity while a line still waits for its button).
pub fn over_at(taken: &[Taken]) -> f32 {
    taken.iter().map(|t| t.0).fold(0.0f32, f32::max) + barrel_engine::event::fade_hold(0.5) + HELP_FADE_LATE / 60.0
}

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
    fn the_help_event_ends_the_opening() {
        // the last command's wait is half a second's fade (32 frames), and the HUD arrives a frame on
        let taken: Vec<Taken> = vec![(100.0, 0, vec![0x7500_0031]), (221.0 - 33.0 / 60.0, 1, vec![0x7500_0031])];
        let over = over_at(&taken);
        assert!((over - 221.0).abs() < 1e-4, "{over}");
        // the free play's first frame (14862 = 1600 + 13262)
        assert_eq!(FREE_PLAY, START + 13262);
        assert!(over_at(&[(f32::INFINITY, 0, vec![])]).is_infinite());
    }

    #[test]
    fn the_events_clock() {
        assert_eq!(START, 1600);
        assert_eq!(stage_seconds(START), 1.0 / 60.0);
        // the first telop's command is on 1819 (a fade of 67 frames to the load's end, 2 s of wait): 219 frames
        // in, drawn with the step after it
        assert!((stage_seconds(1819) - 220.0 / 60.0).abs() < 1e-6);
    }
}
