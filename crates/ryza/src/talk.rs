//! The talk of Ryza DX's events (the opening's lines, docs/ryza/open.md "The opening (P5b)"):
//! a line is white text on the letterbox's lower bar, the speaker's name right-aligned before
//! it, a turning arrow beside it, and after a moment a bar of button hints under the arrow.
//! Everything here is in the 1080-line screen's pixels (the opening key is 1920 x 1080); other
//! sizes scale by their height and keep the picture centred.
//!
//! Read from `opening-key2` (ryza-recomp, A pressed every 120 frames): the text's pen starts at
//! x 508.5 with its first line's top at y 930, 45 below the next, in the executable's
//! `MainFont01` at size 20 (5/6 of the atlas pixels at 1080 lines); a line wraps greedily at
//! 900 pixels (every line of the key breaks the same way at widths 900 to 909); the name is
//! "Name : " in the name's own table (`str_event_chara_name.xml`), its end where the text's first
//! glyph starts. A line fades in over 15 frames from the frame after its command and out over 15
//! from the button (docs/ryza/open.md).

use std::collections::HashMap;

use barrel_engine::stage::Line;
use barrel_render::text::{Glyph, Quad, Spin};

use crate::event::Taken;

/// The 1080-line screen's height, which the pixel numbers below are in.
const SCREEN: f32 = 1080.0;
/// The message font's size on that screen: the atlas pixels' scale.
const TEXT_SCALE: f32 = 5.0 / 6.0;
/// The button hints' font scale.
const HINT_SCALE: f32 = 2.0 / 3.0;
/// The text's pen start, its first line's top and the lines' pitch.
const TEXT_PEN: f32 = 508.5;
const TEXT_TOP: f32 = 930.0;
const TEXT_PITCH: f32 = 45.0;
/// Where a line wraps.
const WRAP: f32 = 900.0;
/// The frames a line takes to come in and to go out.
const FADE: f32 = 15.0;
/// The first frame, counted from the line's command, a line is drawn on (STAND-IN: between 3 and 6 by the key's frames 8395 and 8645; the real thing is its layout's own start).
const FIRST_DRAWN: f32 = 3.0;
/// The frames from a line's command to the hints' coming in.
const HINT_AFTER: f32 = 15.0;
/// The text colour (243 of 255) and the name's (231).
const TEXT_COLOR: f32 = 243.0 / 255.0;
const NAME_COLOR: f32 = 231.0 / 255.0;
/// The `String_No` of the first speaker name (`str_event_chara_name.xml`): a message's name field counts from it.
const FIRST_NAME: u32 = 262145;
/// The hints' text ends (the pen) at this x on every bar.
const HINT_END: f32 = 1902.0033;
const HINT_TOP: f32 = 1039.5;
/// The letterbox's bars (the key's numbers).
const BARS: [[f32; 4]; 2] = [[0.0, -2.9999757, 1923.0002, 124.50002], [0.0, 900.0, 1929.0001, 1079.9999]];
/// The arrow's centre and half size; its three pictures' texture coordinates (the cursor's, `a21_balloon_frame`).
const ARROW: [f32; 3] = [1474.0, 1000.0, 25.0];
const ARROW_UV: [[f32; 4]; 3] = [[0.50884616, 0.2075, 0.5473077, 0.23875], [0.4426923, 0.2075, 0.48115385, 0.23875], [0.37653846, 0.2075, 0.415, 0.23875]];
/// The hint plate (`a21_common`, texture 3): a left end, tiles of 184 pixels, the last one cut, a right end.
const PLATE_Y: [f32; 2] = [1026.0, 1092.0];
const PLATE_LEFT_UV: [f32; 4] = [0.004844961, 0.0, 0.61918604, 0.9705882];
const PLATE_TILE_UV: [f32; 4] = [0.61918604, 0.0, 0.9757752, 0.9705882];
const PLATE_END_UV: [f32; 4] = [0.9757752, 0.0, 0.992248, 0.9705882];
const PLATE_LEFT_WIDTH: f32 = 317.0;
const PLATE_TILE_WIDTH: f32 = 184.0;
const PLATE_END_AT: f32 = 1911.5;
const PLATE_END_TO: f32 = 1920.0;

/// The textures the talk draws from, as the layer holds them (`Talk::TEXTURES` after the layer's own).
pub struct Textures {
    pub font: usize,
    pub balloon: usize,
    pub common: usize,
    pub white: usize,
}

/// The button hints under a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hint {
    /// Event log, fast forward, skip, auto-play (`u6 1`).
    Full,
    /// Fast forward and skip (`u7 1`).
    Short,
}

impl Hint {
    // STAND-IN: the hints' words and button glyphs are the capture's (the game builds them from its button layout and the pad); the real thing is that layout.
    fn text(self) -> &'static str {
        match self {
            Hint::Full => "\u{4f0}Event Log \u{4e7}Fast Forward \u{4ec}Skip \u{4dc}Auto-Play",
            Hint::Short => "\u{4e7}Fast Forward \u{4ec}Skip",
        }
    }

    /// Where the plate's left end is (the key's; the game sizes it to the text).
    fn plate_left(self) -> f32 {
        match self {
            Hint::Full => 1177.8333,
            Hint::Short => 1511.1667,
        }
    }
}

/// What the event's commands switch on and off: the letterbox (`t/0x20` words 17 and 18) and the
/// hints (`u6` and `u7`), each at the time it was taken.
#[derive(Clone, Debug, Default)]
pub struct Cues {
    bars: Vec<(f32, bool)>,
    // (time, u6 on, u7 on) after the command
    hints: Vec<(f32, bool, bool)>,
}

impl Cues {
    /// The cues among an event's taken commands (raw words).
    pub fn from_taken(taken: &[Taken]) -> Cues {
        let mut cues = Cues::default();
        let (mut full, mut short) = (false, false);
        for (time, _, words) in taken {
            let on = words.get(1).is_some_and(|&v| v != 0);
            match words[0] {
                0x7420_0011 => cues.bars.push((*time, true)),
                0x7420_0012 => cues.bars.push((*time, false)),
                0x7500_0006 => {
                    full = on;
                    cues.hints.push((*time, full, short));
                }
                0x7500_0007 => {
                    short = on;
                    cues.hints.push((*time, full, short));
                }
                _ => {}
            }
        }
        cues
    }

    /// Whether the letterbox is up at stage time `t` (commands up to `t` taken).
    pub fn bars(&self, t: f32) -> bool {
        self.bars.iter().take_while(|b| b.0 <= t).last().is_some_and(|b| b.1)
    }

    /// The hints at stage time `t`.
    pub fn hint(&self, t: f32) -> Option<Hint> {
        match self.hints.iter().take_while(|h| h.0 <= t).last() {
            Some(&(_, _, true)) => Some(Hint::Short),
            Some(&(_, true, false)) => Some(Hint::Full),
            _ => None,
        }
    }
}

pub struct Talk {
    glyphs: HashMap<char, Glyph>,
    atlas: (f32, f32),
    names: HashMap<u32, String>,
    textures: Textures,
}

/// How far a line has come in and gone out at stage time `now` (the frame drawn is the stage's tick a frame on from the command's):
/// its opacity, or none once it has faded out.
fn opacity(line: &Line, now: f32) -> Option<f32> {
    let ticks = |since: f32| ((now - since) * 60.0).round() - 1.0;
    // (nothing is drawn for the first frames: capture frame 8395, two frames into a line, has none)
    if ticks(line.since) < FIRST_DRAWN {
        return None;
    }
    let mut a = (ticks(line.since) / FADE).clamp(0.0, 1.0);
    if let Some(c) = line.closing {
        a = a.min(1.0 - (ticks(c) / FADE).clamp(0.0, 1.0));
    }
    (a * 255.0 >= 1.0).then_some(a)
}

/// The `Name` strings of `str_event_chara_name.xml` by their `String_No`.
pub fn names(xml: &[u8]) -> HashMap<u32, String> {
    let text = String::from_utf8_lossy(xml);
    let mut out = HashMap::new();
    for part in text.split("<str ").skip(1) {
        let attr = |name: &str| part.split(&format!("{name}=\"")).nth(1).and_then(|r| r.split('"').next());
        if let (Some(n), Some(t)) = (attr("String_No").and_then(|n| n.parse().ok()), attr("Text")) {
            out.insert(n, t.replace("&amp;", "&"));
        }
    }
    out
}

impl Talk {
    pub fn new(glyphs: HashMap<char, Glyph>, atlas: (u32, u32), names: HashMap<u32, String>, textures: Textures) -> Talk {
        Talk { glyphs, atlas: (atlas.0 as f32, atlas.1 as f32), names, textures }
    }

    fn advance(&self, text: &str, scale: f32) -> f32 {
        text.chars().filter_map(|c| self.glyphs.get(&c)).map(|g| g.advance as f32 * scale).sum()
    }

    /// `text` with its pen at `pen` and the line's top at `top` (1080-line pixels) at `scale`.
    fn glyph_quads(&self, text: &str, (pen, top): (f32, f32), scale: f32, color: [f32; 4], to: &impl Fn([f32; 4]) -> [f32; 4], out: &mut Vec<Quad>) {
        let mut pen = pen;
        for g in text.chars().filter_map(|c| self.glyphs.get(&c)) {
            let (x, y, w, h) = (g.x as f32, g.y as f32, g.width as f32, g.height as f32);
            let l = pen + g.x_offset as f32 * scale;
            let t = top + g.y_offset as f32 * scale;
            out.push(Quad {
                turn: false,
                spin: None,
                rect: to([l, t, l + w * scale, t + h * scale]),
                uv: [x / self.atlas.0, y / self.atlas.1, (x + w) / self.atlas.0, (y + h) / self.atlas.1],
                color,
                texture: self.textures.font,
            });
            pen += g.advance as f32 * scale;
        }
    }

    /// A line's rows: the text wrapped greedily at [`WRAP`], a word being what is between spaces and the spaces after it.
    fn rows(&self, text: &str) -> Vec<String> {
        let mut rows = Vec::new();
        for paragraph in text.split("<CR>") {
            let (mut row, mut x) = (String::new(), 0.0f32);
            let mut rest = paragraph;
            while !rest.is_empty() {
                // a word: what is up to a space, and the spaces after it
                let bare = rest.find(' ').unwrap_or(rest.len());
                let end = bare + rest[bare..].len() - rest[bare..].trim_start_matches(' ').len();
                let (word, tail) = rest.split_at(end);
                let fit = self.advance(word.trim_end_matches(' '), TEXT_SCALE);
                if x > 0.0 && x + fit > WRAP {
                    rows.push(std::mem::take(&mut row));
                    x = 0.0;
                }
                x += self.advance(word, TEXT_SCALE);
                row.push_str(word);
                rest = tail;
            }
            rows.push(row);
        }
        rows
    }

    /// The letterbox's two bars, black.
    pub fn bars(&self, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / SCREEN;
        BARS.iter().map(|b| Quad { turn: false, spin: None, rect: b.map(|v| v * k), uv: [0.0, 0.0, 0.125, 0.125], color: [0.0, 0.0, 0.0, 1.0], texture: self.textures.white }).collect()
    }

    /// The line on screen at stage time `now`: the text, the name, the arrow, the hints' plate and the hints' words, in the
    /// game's draw order; `hint` is what the event has switched on.
    pub fn line_quads(&self, line: &Line, now: f32, hint: Option<Hint>, screen: (u32, u32)) -> Vec<Quad> {
        let Some(alpha) = opacity(line, now) else { return Vec::new() };
        let k = screen.1 as f32 / SCREEN;
        let left = (screen.0 as f32 - 1920.0 * k) * 0.5;
        let to = |r: [f32; 4]| [left + r[0] * k, r[1] * k, left + r[2] * k, r[3] * k];
        let mut out = Vec::new();
        // The text, then the name before it.
        let color = [TEXT_COLOR, TEXT_COLOR, TEXT_COLOR, alpha];
        let rows = self.rows(&line.text);
        for (i, row) in rows.iter().enumerate() {
            self.glyph_quads(row, (TEXT_PEN, TEXT_TOP + TEXT_PITCH * i as f32), TEXT_SCALE, color, &to, &mut out);
        }
        if let Some(name) = line.name.and_then(|n| self.names.get(&(FIRST_NAME + n as u32))) {
            let name = format!("{name} : ");
            let first = line.text.chars().find_map(|c| self.glyphs.get(&c)).map_or(0.0, |g| g.x_offset as f32 * TEXT_SCALE);
            let pen = TEXT_PEN + first - self.advance(&name, TEXT_SCALE);
            self.glyph_quads(&name, (pen, TEXT_TOP), TEXT_SCALE, [NAME_COLOR, NAME_COLOR, NAME_COLOR, alpha], &to, &mut out);
        }
        // The arrow: its clock runs over the lines that stood before and this one's ticks.
        let ticks = line.shown_before + (((now - line.since) * 60.0).round() as u32).saturating_sub(1);
        let turn = crate::title_menu::loop_shown(ticks as i32, 4.0) * 90.0;
        let [cx, cy, half] = ARROW;
        let still = to([cx - half, cy - half, cx + half, cy + half]);
        let pivot = [(still[0] + still[2]) * 0.5, (still[1] + still[3]) * 0.5];
        // The two turning pictures go one way and the other (the trace writes them as the capture does: a picture
        // turned past a quarter is read as flipped and turned back).
        for (uv, theta) in ARROW_UV.iter().zip([turn, -turn]) {
            out.push(Quad { turn: false, spin: Some(Spin::About { pivot, angle: theta.to_radians() }), rect: still, uv: *uv, color: [1.0, 1.0, 1.0, alpha], texture: self.textures.balloon });
        }
        out.push(Quad { turn: false, spin: None, rect: still, uv: ARROW_UV[2], color: [1.0, 1.0, 1.0, alpha], texture: self.textures.balloon });
        // The hints come in after a moment and go when the button goes down.
        let since = ((now - line.since) * 60.0).round() - 1.0;
        // (the hints fade in once, with the first line; after it they stand at once, at the same moment: capture frames 8645/8655 and 8285/8295 hold it between 12 and 16)
        let hint_alpha = if line.shown_before == 0 { ((since - HINT_AFTER) / FADE).clamp(0.0, 1.0) } else { f32::from(u8::from(since >= HINT_AFTER)) };
        let hint = hint.filter(|_| line.closing.is_none() && hint_alpha * 255.0 >= 1.0);
        if let Some(h) = hint {
            let mut at = h.plate_left();
            let mut plate = |from: f32, to_x: f32, uv: [f32; 4]| out.push(Quad { turn: false, spin: None, rect: to([from, PLATE_Y[0], to_x, PLATE_Y[1]]), uv, color: [1.0, 1.0, 1.0, hint_alpha], texture: self.textures.common });
            plate(at, at + PLATE_LEFT_WIDTH, PLATE_LEFT_UV);
            at += PLATE_LEFT_WIDTH;
            while at < PLATE_END_AT - 0.01 {
                let end = (at + PLATE_TILE_WIDTH).min(PLATE_END_AT);
                let mut uv = PLATE_TILE_UV;
                uv[2] = uv[0] + (uv[2] - uv[0]) * (end - at) / PLATE_TILE_WIDTH;
                plate(at, end, uv);
                at = end;
            }
            plate(PLATE_END_AT, PLATE_END_TO, PLATE_END_UV);
        }
        if let Some(h) = hint {
            let text = h.text();
            let pen = HINT_END - self.advance(text, HINT_SCALE);
            self.glyph_quads(text, (pen, HINT_TOP), HINT_SCALE, [1.0, 1.0, 1.0, hint_alpha], &to, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(advance: u32) -> Glyph {
        Glyph { x: 0, y: 0, width: 10, height: 10, x_offset: 0, y_offset: 0, advance }
    }

    fn talk() -> Talk {
        let glyphs = [('a', glyph(24)), (' ', glyph(12)), ('b', glyph(24))].into_iter().collect();
        Talk::new(glyphs, (4096, 2048), HashMap::new(), Textures { font: 0, balloon: 1, common: 2, white: 3 })
    }

    #[test]
    fn a_line_wraps_at_nine_hundred_pixels() {
        let t = talk();
        // 'a ' is 36 atlas pixels, 30 on screen: thirty fill the row (900) and the next word starts the next
        let text = "a ".repeat(30) + "aaa";
        let rows = t.rows(&text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], "a ".repeat(30));
        assert_eq!(rows[1], "aaa");
        // a word that ends at the edge stays: its trailing space may pass it
        assert_eq!(t.rows(&("a ".repeat(29) + "a")).len(), 1);
    }

    #[test]
    fn a_line_fades_in_and_out_over_fifteen_frames() {
        let line = Line { speaker: 0, name: None, text: String::new(), until: None, since: 1.0, closing: None, shown_before: 0 };
        // the frame drawn a tick after the command's: 7 frames on it is 8 ticks, 7/15
        assert_eq!(opacity(&line, 1.0 + 8.0 / 60.0), Some(7.0 / 15.0));
        assert_eq!(opacity(&line, 1.0 + 40.0 / 60.0), Some(1.0));
        let closing = Line { closing: Some(2.0), ..line };
        assert_eq!(opacity(&closing, 2.0 + 2.0 / 60.0), Some(14.0 / 15.0));
        assert_eq!(opacity(&closing, 2.0 + 17.0 / 60.0), None);
    }

    #[test]
    fn the_names_come_from_the_table_file() {
        let xml = br#"<Root><str String_No="262148" Text="Ryza"/><str String_No="262210" Text="Ryza &amp; Lent"/></Root>"#;
        let n = names(xml);
        assert_eq!(n[&262148], "Ryza");
        assert_eq!(n[&262210], "Ryza & Lent");
    }

    #[test]
    fn the_hints_follow_the_commands() {
        let taken: Vec<Taken> = vec![(1.0, 0, vec![0x7500_0006, 1]), (5.0, 0, vec![0x7500_0007, 1]), (5.0, 0, vec![0x7500_0006, 0]), (9.0, 0, vec![0x7420_0011, 0])];
        let cues = Cues::from_taken(&taken);
        assert_eq!(cues.hint(0.5), None);
        assert_eq!(cues.hint(2.0), Some(Hint::Full));
        assert_eq!(cues.hint(6.0), Some(Hint::Short));
        assert!(!cues.bars(8.0) && cues.bars(9.0));
    }
}
