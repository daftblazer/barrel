//! From the title menu's choice to the field: the chosen item leaves, the screen goes black, the
//! load screen (`docs/ryza/open.md`, "The load screen") lists the saves, a button on one starts
//! loading it and the run goes on into the field from that file.
//!
//! The times are title frames (60 a second) counted from the frame of the button on the menu
//! (`c`) and, on the list, from the button on a save (`k`). The game's own are the times its
//! loading takes, so these are measured on the capture `load-path` (inputs at 1400 and 1600).

use barrel_render::text::Quad;

use crate::title_menu::{Hold, Input};

/// The black cover over the menu rises by 1/18 a frame from the button and is full 20 frames
/// after it (the capture's 1/18, 6/18 ... 16/18 at 0, 5 ... 15, full at 20).
const COVER_RISE: f32 = 18.0;
const COVER_FULL: i32 = 20;
// STAND-IN: The notice, the list and the loading come at the frames the capture has them (counted from the button on Load Game and from the button on a save); the real thing is the reading of the saves and the load job, which these waits stand for.
/// The notice (the saving icon over the black) fades in from this many frames after the button
/// on the menu (capture 1448 from 1400).
const NOTICE_AT: i32 = 48;
/// The list fades in from this many frames after the button, as the notice fades out (1556).
const LIST_AT: i32 = 156;
/// After the button on a save: the list fades out over the common fade, the notice fades in a
/// frame later, holds, and fades out again from `NOTICE_OUT` (capture 1600, 1601, 1707) to
/// `LOADING_AT` (1725), where the loading screen takes over; the field follows at `FIELD_AT`.
const NOTICE_IN: i32 = 1;
const NOTICE_OUT: i32 = 107;
pub const LOADING_AT: i32 = 125;
const FIELD_AT: i32 = 130;
/// Frames of the common fade (`anm_common_fade_in`).
const FADE: i32 = 15;

/// Where the run is on the path.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    /// Not on it yet (no item chosen, or one that does not load).
    Title,
    /// Under the rising black cover, with this opacity.
    Cover(f32),
    /// Black.
    Black,
    /// The load screen's own drawing: the list (with its age and, once a save was chosen, the
    /// frames since) and the notice over it (its animation's frames and opacity), the cursor on
    /// slot `on`.
    Screen { on: usize, list: Option<(i32, Option<i32>)>, notice: Option<(i32, f32)> },
    /// The loading screen after a save was chosen, in frames since the choice; `true` once the
    /// run is to go on into the field.
    Loading { on: usize, since: i32, done: bool },
}

/// The slot the cursor is on at `frame`: one step a press, a held direction repeating as the
/// title menu's does, wrapping at the ends on a press.
pub fn cursor(holds: &[Hold], count: usize, frame: i32) -> usize {
    let mut on = 0i32;
    let mut steps: Vec<(i32, i32)> = holds
        .iter()
        .flat_map(|h| {
            let end = h.until.unwrap_or(i32::MAX).min(frame + 1);
            (h.from..end).step_by(1).take(1).map(move |at| (at, h.way)).chain((h.from + 37..end).step_by(15).map(move |at| (at, h.way)))
        })
        .collect();
    steps.sort();
    for (at, way) in steps.into_iter().filter(|s| s.0 <= frame) {
        on = (on + way).rem_euclid(count.max(1) as i32);
    }
    on as usize
}

impl Stage {
    /// The stage on title frame `frame`, `count` saves being listed.
    pub fn at(input: &Input, count: usize, frame: i32) -> Stage {
        let Some(c) = input.choose.filter(|&c| frame >= c) else { return Stage::Title };
        let from_c = frame - c;
        let fade = |frames: i32| (frames as f32 / FADE as f32).clamp(0.0, 1.0);
        if let Some(k) = input.confirm.filter(|&k| k >= c + LIST_AT && frame >= k) {
            let on = cursor(&input.list_holds, count, k);
            let since = frame - k;
            if since >= LOADING_AT {
                return Stage::Loading { on, since, done: since >= FIELD_AT };
            }
            let notice = (since >= NOTICE_IN).then(|| (since - NOTICE_IN, if since >= NOTICE_OUT { 1.0 - fade(since - NOTICE_OUT) } else { fade(since - NOTICE_IN) }));
            return Stage::Screen { on, list: (since < FADE).then_some((from_c - LIST_AT, Some(since))), notice };
        }
        match from_c {
            f if f < COVER_FULL => Stage::Cover(((f + 1) as f32 / COVER_RISE).min(1.0)),
            f if f < NOTICE_AT => Stage::Black,
            f if f < LIST_AT + FADE => {
                let notice = Some((f - NOTICE_AT, if f < LIST_AT { fade(f - NOTICE_AT) } else { 1.0 - fade(f - LIST_AT) }));
                Stage::Screen { on: cursor(&input.list_holds, count, frame), list: (f >= LIST_AT).then_some((f - LIST_AT, None)), notice }
            }
            f => Stage::Screen { on: cursor(&input.list_holds, count, frame), list: Some((f - LIST_AT, None)), notice: None },
        }
    }
}

/// The cover: the whole screen black at `alpha` (drawn from `texture`, which the plain fill does not read).
pub fn cover(alpha: f32, screen: (u32, u32), texture: usize) -> Quad {
    Quad { turn: false, spin: None, rect: [0.0, 0.0, screen.0 as f32, screen.1 as f32], uv: [-1.0; 4], color: [0.0, 0.0, 0.0, alpha], texture }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Input {
        Input { choose: Some(857), confirm: Some(1057), ..Input::default() }
    }

    #[test]
    fn the_cover_rises_then_the_notice_the_list_and_a_button_loads() {
        let i = input();
        assert_eq!(Stage::at(&i, 2, 856), Stage::Title);
        assert_eq!(Stage::at(&i, 2, 857), Stage::Cover(1.0 / 18.0));
        assert_eq!(Stage::at(&i, 2, 872), Stage::Cover(16.0 / 18.0));
        assert_eq!(Stage::at(&i, 2, 877), Stage::Black);
        // the notice from 1448 (capture) with 2/15 at 1450
        assert_eq!(Stage::at(&i, 2, 857 + 50), Stage::Screen { on: 0, list: None, notice: Some((2, 2.0 / 15.0)) });
        // the list from 1556, the notice fading out as it fades in
        assert_eq!(Stage::at(&i, 2, 857 + 160), Stage::Screen { on: 0, list: Some((4, None)), notice: Some((112, 1.0 - 4.0 / 15.0)) });
        assert_eq!(Stage::at(&i, 2, 857 + 176), Stage::Screen { on: 0, list: Some((20, None)), notice: None });
        // a button on a save: the list fades out, the notice in, then the loading screen and the field
        assert_eq!(Stage::at(&i, 2, 1057 + 5), Stage::Screen { on: 0, list: Some((49, Some(5))), notice: Some((4, 4.0 / 15.0)) });
        assert_eq!(Stage::at(&i, 2, 1057 + 125), Stage::Loading { on: 0, since: 125, done: false });
        assert_eq!(Stage::at(&i, 2, 1057 + 130), Stage::Loading { on: 0, since: 130, done: true });
    }

    #[test]
    fn the_cursor_steps_and_wraps() {
        let down = |from| Hold { from, until: Some(from + 1), way: 1 };
        assert_eq!(cursor(&[down(10)], 2, 9), 0);
        assert_eq!(cursor(&[down(10)], 2, 10), 1);
        assert_eq!(cursor(&[down(10), down(20)], 2, 25), 0);
    }
}
