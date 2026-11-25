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

/// The tutorial's pieces (the opening's room tutorial, `opening-key2` 13025 to 13779): the pictures' panel, the second
/// picture's fade, the box and its text (16 pixels' size: 2/3 of the atlas, rows 36 apart about a centre, wrapped at
/// 750: 714 fits and 768 does not), the arrow beside it with its shadow (a square of 38.4 turned 49.4 degrees), the dark
/// red text and its markup colours.
const PICTURE_PANEL: [f32; 4] = [249.6, 43.2, 1670.4, 842.4];
const PICTURE_FADE: f32 = 60.0;
const TUTORIAL_BOX: [f32; 4] = [426.0, 760.5, 1536.0, 1040.5];
const TUTORIAL_BOX_UV: [f32; 4] = [0.034615386, 0.00125, 0.88846153, 0.17625];
const TUTORIAL_SCALE: f32 = 2.0 / 3.0;
const TUTORIAL_WRAP: f32 = 750.0;
const TUTORIAL_PEN: f32 = 598.5;
const TUTORIAL_CENTRE: f32 = 892.5;
const TUTORIAL_PITCH: f32 = 36.0;
/// The arrow clock of a tutorial's lines (`engine::stage::Stage::tutorial_clock`): a closed line's fade out (30 frames) adds as many to it and a handed-over box costs 1 (`opening-key4` 13141 to 13442: run A and B at 0).
pub const TUTORIAL_CLOCK: (u32, u32) = (30, 1);
/// Where the letters' tops sit in the font's cell (the bracket's `y_offset`), in font units.
const LETTER_TOP: f32 = 10.0;
const TUTORIAL_TEXT: [f32; 3] = [26.0 / 255.0, 0.0, 0.0];
const TEXT_COLOR_DARK: [f32; 3] = TUTORIAL_TEXT;
const TUTORIAL_ARROW: [f32; 3] = [1390.0, 935.5, 25.0];
const TUTORIAL_SLIDE: f32 = 45.0;
const TUTORIAL_SHADOW_SIDE: f32 = 38.4;
const TUTORIAL_SHADOW_TURN: f32 = 49.4;
const TUTORIAL_SHADOW_ALPHA: f32 = 60.0 / 255.0;
const TUTORIAL_FADE: f32 = 30.0;
// STAND-IN: the hints come in between 27 and 37 frames into a tutorial line (capture frames 13165 and 13175); 30 is a guess.
const TUTORIAL_HINT_AFTER: f32 = 30.0;

/// A tutorial text without its markup, and where each colour starts (`<#RRGGBB>`, `<CLNR>` back to the plain colour), by character.
fn markup(text: &str) -> (String, Vec<(usize, [f32; 3])>) {
    let (mut plain, mut runs) = (String::new(), Vec::new());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(tag) = rest.strip_prefix("<#").and_then(|r| r.get(..7)).filter(|t| t.ends_with('>')) {
            let v = u32::from_str_radix(&tag[..6], 16).unwrap_or(0);
            runs.push((plain.chars().count(), [(v >> 16) as f32 / 255.0, (v >> 8 & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0]));
            rest = &rest[9..];
        } else if let Some(r) = rest.strip_prefix("<KEY_ASSIGN_FIELD_PHOTO_MODE>") {
            // the button's icon in the font (STAND-IN: the pad's layout is not read; the key shows the one icon, `opening-key3` 14005)
            plain.push('\u{4f4}');
            rest = r;
        } else if let Some(r) = rest.strip_prefix("<CLNR>") {
            runs.push((plain.chars().count(), TUTORIAL_TEXT));
            rest = r;
        } else {
            let c = rest.chars().next().unwrap();
            plain.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    (plain, runs)
}

/// The textures the talk draws from, as the layer holds them (`Talk::TEXTURES` after the layer's own).
pub struct Textures {
    pub font: usize,
    pub balloon: usize,
    pub common: usize,
    pub white: usize,
    /// The tutorial pictures' set: a tiled texture first, the two backgrounds, the arrow's shadow and each picture (`u76`'s number, its texture).
    pub tiled: usize,
    pub background: usize,
    pub frame: usize,
    pub circle: usize,
    pub pictures: Vec<(u32, usize)>,
}

/// The button hints under a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hint {
    /// Event log, fast forward, skip, auto-play (`u6 1`).
    Full,
    /// Fast forward and skip (`u7 1`).
    Short,
    /// A help box's own: the full set without skip (the help event's, `opening-key3` 14005).
    NoSkip,
}

impl Hint {
    // STAND-IN: the hints' words and button glyphs are the capture's (the game builds them from its button layout and the pad); the real thing is that layout.
    fn text(self) -> &'static str {
        match self {
            Hint::Full => "\u{4f0}Event Log \u{4e7}Fast Forward \u{4ec}Skip \u{4dc}Auto-Play",
            Hint::Short => "\u{4e7}Fast Forward \u{4ec}Skip",
            Hint::NoSkip => "\u{4f0}Event Log \u{4e7}Fast Forward \u{4dc}Auto-Play",
        }
    }

    /// Where the plate's left end is (the key's; the game sizes it to the text).
    fn plate_left(self) -> f32 {
        match self {
            Hint::Full => 1177.8333,
            Hint::Short => 1511.1667,
            Hint::NoSkip => 1270.5,
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
    // `u76` (a tutorial picture): (time, shown, picture number)
    pictures: Vec<(f32, bool, u32)>,
    /// The stage time the help event starts at: its boxes have no event log among their hints.
    help_from: Option<f32>,
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
                0x7500_004c => cues.pictures.push((*time, on, words.get(3).copied().unwrap_or(0))),
                _ => {}
            }
        }
        cues
    }

    /// Marks the event that starts at `t` as the help event ([`Hint::NoSkip`]).
    pub fn help_from(mut self, t: f32) -> Cues {
        self.help_from = Some(t);
        self
    }

    /// The hints of a tutorial box at stage time `t`: the full set, without skip in the help event.
    pub fn box_hint(&self, t: f32) -> Hint {
        if self.help_from.is_some_and(|h| t >= h) { Hint::NoSkip } else { Hint::Full }
    }

    /// Whether the letterbox is up at stage time `t` (commands up to `t` taken).
    pub fn bars(&self, t: f32) -> bool {
        self.bars.iter().take_while(|b| b.0 <= t).last().is_some_and(|b| b.1)
    }

    /// The tutorial pictures up at stage time `t`: the first, and the one that came over it with the time it did.
    pub fn pictures(&self, t: f32) -> Option<(u32, Option<(u32, f32)>)> {
        let mut state = None;
        for &(time, shown, id) in self.pictures.iter().take_while(|p| p.0 <= t) {
            state = match (shown, state) {
                (false, _) => None,
                (true, None) => Some((id, None)),
                (true, Some((first, None))) if first != id => Some((first, Some((id, time)))),
                (true, s) => s,
            };
        }
        state
    }

    /// The numbers of the pictures the event shows (to load).
    pub fn picture_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.pictures.iter().filter(|p| p.1).map(|p| p.2).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
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

/// A tutorial picture's file (`u76`'s number): the room tutorial's are `ipu\t<84 + number>`; the help pages' (the opening's
/// following event, number 163 for `help_h_075`) are `ipu\help_h_<number - 88>`. STAND-IN: the executable's table that names
/// each picture is not read; the second rule is fitted to the one page the key shows (`opening-key3`, 14255 to 14755).
pub fn picture_file(install: &crate::install::Install, id: u32) -> Result<Vec<u8>, String> {
    install
        .read(&format!("\\data\\x64\\res_cmn\\ipu\\t{}.g1t", 84 + id))
        .or_else(|_| install.read(&format!("\\data\\x64\\res_cmn\\ipu\\help_h_{:03}.g1t", id.saturating_sub(88))))
        .map_err(|e| format!("tutorial picture {id}: {e}"))
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
    if ticks(line.since) < FIRST_DRAWN && !line.joined {
        return None;
    }
    let mut a = if line.joined { 1.0 } else { (ticks(line.since) / FADE).clamp(0.0, 1.0) };
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
        self.rows_at(text, TEXT_SCALE, WRAP)
    }

    /// [`Talk::rows`] at `scale` and a width of `wrap` pixels.
    fn rows_at(&self, text: &str, scale: f32, wrap: f32) -> Vec<String> {
        let mut rows = Vec::new();
        for paragraph in text.split("<CR>") {
            let (mut row, mut x) = (String::new(), 0.0f32);
            let mut rest = paragraph;
            while !rest.is_empty() {
                // a word: what is up to a space, and the spaces after it
                let bare = rest.find(' ').unwrap_or(rest.len());
                let end = bare + rest[bare..].len() - rest[bare..].trim_start_matches(' ').len();
                let (word, tail) = rest.split_at(end);
                let fit = self.advance(word.trim_end_matches(' '), scale);
                if x > 0.0 && x + fit > wrap {
                    rows.push(std::mem::take(&mut row));
                    x = 0.0;
                }
                x += self.advance(word, scale);
                row.push_str(word);
                rest = tail;
            }
            rows.push(row);
        }
        rows
    }

    /// The tutorial's pictures at stage time `now`, in the game's draw order: the tiled texture, each picture whole, the white
    /// fill, the first background, each picture in its panel, the second background. The second picture comes in over a second.
    pub fn picture_quads(&self, set: (u32, Option<(u32, f32)>), now: f32, screen: (u32, u32)) -> Vec<Quad> {
        let k = screen.1 as f32 / SCREEN;
        let left = (screen.0 as f32 - 1920.0 * k) * 0.5;
        let to = |r: [f32; 4]| [left + r[0] * k, r[1] * k, left + r[2] * k, r[3] * k];
        let quad = |rect: [f32; 4], uv: [f32; 4], alpha: f32, texture: usize| Quad { turn: false, spin: None, rect: to(rect), uv, color: [1.0, 1.0, 1.0, alpha], texture };
        let texture = |id: u32| self.textures.pictures.iter().find(|p| p.0 == id).map_or(self.textures.white, |p| p.1);
        let shown: Vec<(u32, f32)> = std::iter::once((set.0, 1.0)).chain(set.1.map(|(id, at)| (id, (((now - at) * 60.0).round() / PICTURE_FADE).clamp(0.0, 1.0)))).collect();
        let mut out = vec![quad([0.0, 0.0, 9600.0, 3240.0], [0.0, 0.0, 7.5, 4.5], 1.0, self.textures.tiled)];
        for &(id, a) in &shown {
            out.push(quad([0.0, 0.0, 1920.0, 1080.0], [0.0, 0.0, 1.0, 1.0], a, texture(id)));
        }
        out.push(quad([0.0, 0.0, 1920.0, 1080.0], [0.0, 0.0, 0.125, 0.125], 1.0, self.textures.white));
        out.push(quad([0.0, 0.0, 3840.0, 2160.0], [0.0, 0.0, 2.0, 2.0], 1.0, self.textures.background));
        for &(id, a) in &shown {
            out.push(quad(PICTURE_PANEL, [0.0, 0.0, 1.0, 1.0], a, texture(id)));
        }
        out.push(quad([0.0, 0.0, 3840.0, 2160.0], [0.0, 0.0, 2.0, 2.0], 1.0, self.textures.frame));
        out
    }

    /// A tutorial line at stage time `now`: its box, its text in dark red (`<#RRGGBB>` to `<CLNR>` in that colour), the
    /// arrow's shadow and arrow, and the hints. It comes in over 30 frames along an ease-out cubic (the arrow sliding up
    /// 45 pixels with it) and goes out over 30 along an ease-in cubic.
    pub fn tutorial_quads(&self, line: &Line, now: f32, hint: Hint, screen: (u32, u32)) -> Vec<Quad> {
        let ticks = |since: f32| ((now - since) * 60.0).round() - 1.0;
        let k_in = ticks(line.since);
        if k_in < FIRST_DRAWN && !line.joined {
            return Vec::new();
        }
        let t_in = if line.joined { 1.0 } else { (k_in / TUTORIAL_FADE).clamp(0.0, 1.0) };
        let mut alpha = 1.0 - (1.0 - t_in).powi(3);
        if let Some(c) = line.closing {
            alpha = alpha.min(1.0 - (ticks(c) / TUTORIAL_FADE).clamp(0.0, 1.0).powi(3));
        }
        if alpha * 255.0 < 1.0 {
            return Vec::new();
        }
        let k = screen.1 as f32 / SCREEN;
        let left = (screen.0 as f32 - 1920.0 * k) * 0.5;
        let to = |r: [f32; 4]| [left + r[0] * k, r[1] * k, left + r[2] * k, r[3] * k];
        // the box, its text and the arrow all slide up with the fade: 45 pixels where it is clear
        let slide = TUTORIAL_SLIDE * (1.0 - alpha);
        let fixed = to;
        let to = |r: [f32; 4]| fixed([r[0], r[1] + slide, r[2], r[3] + slide]);
        let mut out = vec![Quad { turn: false, spin: None, rect: to(TUTORIAL_BOX), uv: TUTORIAL_BOX_UV, color: [1.0, 1.0, 1.0, alpha], texture: self.textures.balloon }];
        // the text: rows wrapped at TUTORIAL_WRAP, a block centred on TUTORIAL_CENTRE; colours by the markup
        let (plain, runs) = markup(&line.text);
        let rows = self.rows_at(&plain, TUTORIAL_SCALE, TUTORIAL_WRAP);
        // STAND-IN: a lone row is centred by the box its glyphs fill, so one that holds a taller glyph (a button's icon rises 7
        // font units above the letters, the bracket's top at 10) stands half that rise higher (`opening-key3` 14005: 7/3
        // pixels with an icon; `opening-key4` 13141 to 13323: none for plain letters); the rule for several rows is not read.
        let rise = if let [row] = rows.as_slice() { row.chars().filter_map(|c| self.glyphs.get(&c)).map(|g| g.y_offset).min().map_or(0.0, |y| (LETTER_TOP - y as f32).max(0.0)) } else { 0.0 };
        let top = TUTORIAL_CENTRE - TUTORIAL_PITCH * rows.len() as f32 * 0.5 - rise * TUTORIAL_SCALE * 0.5;
        let mut at = 0;
        for (i, row) in rows.iter().enumerate() {
            let mut pen = TUTORIAL_PEN;
            for ch in row.chars() {
                // a button's icon keeps its own colours
                let color = if ('\u{4d0}'..='\u{4ff}').contains(&ch) { [1.0; 3] } else { runs.iter().rev().find(|r| r.0 <= at).map_or(TUTORIAL_TEXT, |r| r.1) };
                self.glyph_quads(&ch.to_string(), (pen, top + TUTORIAL_PITCH * i as f32), TUTORIAL_SCALE, [color[0], color[1], color[2], alpha], &to, &mut out);
                pen += self.advance(&ch.to_string(), TUTORIAL_SCALE);
                at += 1;
            }
        }
        // the arrow and its shadow slide up with the box coming in
        // (the frame the button hands the box on costs the clock one tick: `TUTORIAL_CLOCK`)
        let stalled = line.handed_at.is_some_and(|h| (now - h) * 60.0 >= 1.5) as u32 * TUTORIAL_CLOCK.1;
        let ticks_run = (line.shown_before + k_in as u32).saturating_sub(stalled);
        let turn = crate::title_menu::loop_shown(ticks_run as i32, 4.0) * 90.0;
        if std::env::var_os("TALK_DEBUG").is_some() {
            eprintln!("tutorial: since {} before {} ticks {ticks_run} turn {turn}", line.since, line.shown_before);
        }
        let [cx, cy, half] = TUTORIAL_ARROW;
        let pivot = to([cx, cy, cx, cy]);
        let pivot = [pivot[0], pivot[1]];
        let shadow_half = TUTORIAL_SHADOW_SIDE * 0.5;
        out.push(Quad { turn: false, spin: Some(Spin::About { pivot, angle: TUTORIAL_SHADOW_TURN.to_radians() }), rect: to([cx - shadow_half, cy - shadow_half, cx + shadow_half, cy + shadow_half]), uv: [1.0, 1.0, 0.0, 0.0], color: [TEXT_COLOR_DARK[0], TEXT_COLOR_DARK[1], TEXT_COLOR_DARK[2], TUTORIAL_SHADOW_ALPHA * alpha], texture: self.textures.circle });
        let still = to([cx - half, cy - half, cx + half, cy + half]);
        for (uv, theta) in ARROW_UV.iter().zip([turn, -turn]) {
            out.push(Quad { turn: false, spin: Some(Spin::About { pivot, angle: theta.to_radians() }), rect: still, uv: *uv, color: [1.0, 1.0, 1.0, alpha], texture: self.textures.balloon });
        }
        out.push(Quad { turn: false, spin: None, rect: still, uv: ARROW_UV[2], color: [1.0, 1.0, 1.0, alpha], texture: self.textures.balloon });
        // the hints come in once the line has
        if (k_in >= TUTORIAL_HINT_AFTER || line.joined) && line.closing.is_none() {
            out.extend(self.hint_quads(hint, 1.0, &fixed));
        }
        out
    }

    /// The hints' plate and words at `alpha`.
    fn hint_quads(&self, hint: Hint, alpha: f32, to: &impl Fn([f32; 4]) -> [f32; 4]) -> Vec<Quad> {
        let mut out = Vec::new();
        let mut at = hint.plate_left();
        let mut plate = |from: f32, to_x: f32, uv: [f32; 4]| out.push(Quad { turn: false, spin: None, rect: to([from, PLATE_Y[0], to_x, PLATE_Y[1]]), uv, color: [1.0, 1.0, 1.0, alpha], texture: self.textures.common });
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
        let text = hint.text();
        let pen = HINT_END - self.advance(text, HINT_SCALE);
        self.glyph_quads(text, (pen, HINT_TOP), HINT_SCALE, [1.0, 1.0, 1.0, alpha], to, &mut out);
        out
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
        if std::env::var_os("TALK_DEBUG").is_some() {
            eprintln!("talk: since {} before {} ticks {ticks} turn {turn}", line.since, line.shown_before);
        }
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
        let hint_alpha = if line.joined { 1.0 } else if line.shown_before == 0 { ((since - HINT_AFTER) / FADE).clamp(0.0, 1.0) } else { f32::from(u8::from(since >= HINT_AFTER)) };
        let hint = hint.filter(|_| line.closing.is_none() && hint_alpha * 255.0 >= 1.0);
        if let Some(h) = hint {
            out.extend(self.hint_quads(h, hint_alpha, &to));
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
        Talk::new(glyphs, (4096, 2048), HashMap::new(), Textures { font: 0, balloon: 1, common: 2, white: 3, tiled: 4, background: 5, frame: 6, circle: 7, pictures: Vec::new() })
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
        let line = Line { speaker: 0, name: None, text: String::new(), until: None, since: 1.0, closing: None, shown_before: 0, joined: false, handed_over: false, more_follows: false, tutorial: false, handed_at: None };
        // the frame drawn a tick after the command's: 7 frames on it is 8 ticks, 7/15
        assert_eq!(opacity(&line, 1.0 + 8.0 / 60.0), Some(7.0 / 15.0));
        assert_eq!(opacity(&line, 1.0 + 40.0 / 60.0), Some(1.0));
        let closing = Line { closing: Some(2.0), ..line };
        assert_eq!(opacity(&closing, 2.0 + 2.0 / 60.0), Some(14.0 / 15.0));
        assert_eq!(opacity(&closing, 2.0 + 17.0 / 60.0), None);
    }

    #[test]
    fn the_markup_colours_a_run() {
        let (plain, runs) = markup("go <#006E5A>save<CLNR> now");
        assert_eq!(plain, "go save now");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].0, 3);
        assert!((runs[0].1[1] - 110.0 / 255.0).abs() < 1e-6);
        assert_eq!(runs[1], (7, TUTORIAL_TEXT));
    }

    #[test]
    fn the_pictures_follow_the_commands() {
        let taken: Vec<Taken> = vec![(1.0, 0, vec![0x7500_004c, 1, 0, 27]), (5.0, 0, vec![0x7500_004c, 1, 0, 26]), (9.0, 0, vec![0x7500_004c, 0, 0, 4])];
        let cues = Cues::from_taken(&taken);
        assert_eq!(cues.picture_ids(), [26, 27]);
        assert_eq!(cues.pictures(0.5), None);
        assert_eq!(cues.pictures(2.0), Some((27, None)));
        assert_eq!(cues.pictures(6.0), Some((27, Some((26, 5.0)))));
        assert_eq!(cues.pictures(9.0), None);
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
