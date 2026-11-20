//! From the title menu's choice to the field: the chosen item leaves, the screen goes black, the
//! load screen (`docs/ryza/open.md`, "The load screen") lists the saves, a button on one starts
//! loading it and the run goes on into the field from that file.
//!
//! The times are title frames (60 a second) counted from the frame of the button on the menu
//! (`c`) and, on the list, from the button on a save (`k`). The game's own are the times its
//! loading takes, so these are measured on the capture `load-path` (inputs at 1400 and 1600).

use barrel_render::text::Quad;

use crate::saves::Slot;
use crate::title::Title;
use crate::title_menu::{Hold, Input};

/// The black cover over the menu rises by 1/18 a frame from the button and is full 20 frames
/// after it (the capture's 1/18, 6/18 ... 16/18 at 0, 5 ... 15, full at 20).
const COVER_RISE: f32 = 18.0;
const COVER_FULL: i32 = 20;
// STAND-IN: The wait before the list (160 frames after the button) is the time the game spent reading the saves in the capture; the real thing is the reading itself.
const LIST_AT: i32 = 160;
// STAND-IN: The list leaves 15 frames after the button on a save and the field follows 130 frames after it (the loading's own time in the capture); the real thing is the load job.
const LEAVE: i32 = 15;
const FIELD_AT: i32 = 130;

/// Where the run is on the path.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    /// Not on it yet (no item chosen, or one that does not load).
    Title,
    /// Under the rising black cover, with this opacity.
    Cover(f32),
    /// Black, waiting for the saves to be read.
    Black,
    /// The list, the cursor on this slot.
    List(usize),
    /// A save was chosen and is loading; `Some(slot)` once the run is to go on into the field.
    Loading(usize, bool),
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
        if let Some(k) = input.confirm.filter(|&k| k >= c + LIST_AT && frame >= k) {
            let on = cursor(&input.list_holds, count, k);
            return Stage::Loading(on, frame - k >= FIELD_AT);
        }
        match from_c {
            f if f < COVER_FULL => Stage::Cover(((f + 1) as f32 / COVER_RISE).min(1.0)),
            f if f < LIST_AT => Stage::Black,
            _ => Stage::List(cursor(&input.list_holds, count, frame)),
        }
    }
}

/// The cover: the whole screen black at `alpha` (drawn from `texture`, which the plain fill does not read).
pub fn cover(alpha: f32, screen: (u32, u32), texture: usize) -> Quad {
    Quad { turn: false, spin: None, rect: [0.0, 0.0, screen.0 as f32, screen.1 as f32], uv: [-1.0; 4], color: [0.0, 0.0, 0.0, alpha], texture }
}

/// The list: the caption, each save's name and time of saving, the cursor's row lit and the
/// chosen save's figures at the right.
// STAND-IN: The list is drawn from the title's font alone (no frames or pictures yet), in the layout's places (`uil_a21_saveload.xml`: the list at 115, 80 with rows 111 apart, the info at 727, 129); the real thing is the screen's own styles, which are not read yet.
pub fn list_quads(title: &Title, slots: &[Slot], on: usize, leaving: bool, screen: (u32, u32)) -> Vec<Quad> {
    let ink = [1.0, 1.0, 1.0, if leaving { 0.0 } else { 1.0 }];
    let (k, left) = (screen.1 as f32 / 720.0, (screen.0 as f32 - 1280.0 * screen.1 as f32 / 720.0) * 0.5);
    let mut out = vec![Quad { turn: false, spin: None, rect: [0.0, 0.0, screen.0 as f32, screen.1 as f32], uv: [-1.0; 4], color: [0.1, 0.07, 0.05, ink[3]], texture: 0 }];
    out.extend(title.text_quads("Load", (48.0, 17.0), 28.0, ink, screen));
    for (i, s) in slots.iter().enumerate().take(5) {
        let y = 80.0 + 111.0 * i as f32;
        if i == on {
            out.push(Quad { turn: false, spin: None, rect: [left + 115.0 * k, y * k, left + 625.0 * k, (y + 100.0) * k], uv: [-1.0; 4], color: [1.0, 1.0, 1.0, 0.25 * ink[3]], texture: 0 });
        }
        out.extend(title.text_quads(&s.name, (115.0 + 234.0, y + 40.0), 17.0, ink, screen));
        out.extend(title.text_quads(&s.header.saved_at(), (115.0 + 234.0, y + 68.0), 17.0, ink, screen));
    }
    if let Some(s) = slots.get(on) {
        let h = &s.header;
        let rows = [("Playtime", h.play_time()), ("Alchemy Level", h.alchemy_level.to_string()), ("Money", h.money.to_string()), ("Gold Coin", h.gold_coins.to_string())];
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = 129.0 + 48.0 + 27.0 * i as f32;
            out.extend(title.text_quads(label, (727.0 + 38.0, y), 17.0, ink, screen));
            out.extend(title.text_quads(value, (727.0 + 38.0 + 200.0, y), 17.0, ink, screen));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Input {
        Input { choose: Some(857), confirm: Some(1057), ..Input::default() }
    }

    #[test]
    fn the_cover_rises_then_the_list_comes_and_a_button_loads() {
        let i = input();
        assert_eq!(Stage::at(&i, 2, 856), Stage::Title);
        assert_eq!(Stage::at(&i, 2, 857), Stage::Cover(1.0 / 18.0));
        assert_eq!(Stage::at(&i, 2, 872), Stage::Cover(16.0 / 18.0));
        assert_eq!(Stage::at(&i, 2, 877), Stage::Black);
        assert_eq!(Stage::at(&i, 2, 1017), Stage::List(0));
        assert_eq!(Stage::at(&i, 2, 1057), Stage::Loading(0, false));
        assert_eq!(Stage::at(&i, 2, 1187), Stage::Loading(0, true));
    }

    #[test]
    fn the_cursor_steps_and_wraps() {
        let down = |from| Hold { from, until: Some(from + 1), way: 1 };
        assert_eq!(cursor(&[down(10)], 2, 9), 0);
        assert_eq!(cursor(&[down(10)], 2, 10), 1);
        assert_eq!(cursor(&[down(10), down(20)], 2, 25), 0);
    }
}
