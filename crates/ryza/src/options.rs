//! The Settings screen the title menu opens: what it lists on each tab, the value of each row,
//! the cursor and the tabs, how the values go to and come from Barrel's own config, and the
//! path from the menu's button to the screen.
//!
//! The pages are the game's: Camera, Game, Audio and Graphics (Keyboard and Controller, the
//! key assignments, are not built: `docs/ryza/open.md`, "Settings screen"). The words come from
//! the string tables; the order of the rows on the Game tab is the capture's, the others' the
//! string tables'.

use std::collections::HashMap;

use crate::graphics::{self, Item, Quality};
use crate::options_view::{Kind, Page, Row, Screen};
use crate::title_menu::{Hold, Input, Repeat};

/// The black cover rises as the load screen's (`load::COVER_RISE`, 20 frames to full) and the
/// screen fades in from the frame after it is full (capture: bg alpha 11/15 on the 30th frame
/// after the button).
const COVER_FULL: i32 = 20;
const COVER_RISE: f32 = 18.0;
const SCREEN_AGE_AT: i32 = COVER_FULL - 1;

/// The blink's loop in the capture's frames: the cursor's 120-frame blink restarts every 121
/// (alpha of the cursor on the settings-scroll frames 1600 to 1860, every 5: the clock is the
/// frame count to within the alpha's 1/255 steps, and wraps a frame late at 1680 and 1801).
// STAND-IN: 121 is read off those alphas; the real thing is the game's own animation clock, which counts a frame at the loop's end.
pub const BLINK_LOOP: i32 = 121;

/// How many times as fast the cursor of `tab` blinks.
pub fn blink_speed(tab: usize) -> i32 {
    if tab == KEYS_TAB { 2 } else { 1 }
}

/// The cursor's blink clock `age` frames after it restarted, for a cursor that blinks `speed`
/// times as fast (the key list's is twice the others'), in the animation's 120 frames.
// STAND-IN: on the frame the loop wraps the capture draws the cursor at 251/255, as at clock 119.06 (settings-scroll 1680 and 2000); the clock there is taken as that.
pub fn blink_clock(age: i32, speed: i32) -> f32 {
    match (age * speed).rem_euclid(BLINK_LOOP) {
        0 if age > 0 => 119.06,
        c => c as f32,
    }
}

/// Where the player is on the path from the menu to the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Title,
    /// The black over the menu: its opacity.
    Cover(f32),
    /// The screen, `age` frames after it started to fade in.
    Screen { age: i32 },
}

impl Stage {
    /// The stage on title frame `frame` for a button on the menu's Settings item at `choose`.
    pub fn at(choose: Option<i32>, frame: i32) -> Stage {
        let Some(c) = choose.filter(|&c| frame >= c) else { return Stage::Title };
        let f = frame - c;
        if f < COVER_FULL {
            Stage::Cover(((f + 1) as f32 / COVER_RISE).min(1.0))
        } else {
            Stage::Screen { age: f - SCREEN_AGE_AT }
        }
    }
}

/// A held direction on this screen: the step at once, the next after 30 frames, then every 8
/// (the capture's 22-row Graphics list with the direction held 220 frames: the cursor on rows 2 to
/// 21 at frames fitting 1650 + 8 a row to within the 5-frame sampling).
// STAND-IN: the repeat's period (8 frames) is fitted to that capture; the real thing is the game's own key repeat timer.
pub const REPEAT: Repeat = Repeat { first: 30, next: 8 };

/// The row the cursor is on at `frame` among `count`, from `base`, the rows' directions held on
/// the screen as `input.list_holds`: a press wraps at the ends, a held repeat stops there.
pub fn cursor_on(input: &Input, base: usize, count: usize, frame: i32) -> usize {
    let n = count.max(1) as i32;
    let mut steps: Vec<_> = input.list_holds.iter().flat_map(|h| h.steps_until(frame, REPEAT)).filter(|s| s.0 <= frame).collect();
    steps.sort_by_key(|s| s.0);
    let mut on = base as i32 % n;
    for (_, way, repeat) in steps {
        on = if repeat { (on + way).clamp(0, n - 1) } else { (on + way).rem_euclid(n) };
    }
    on as usize
}

/// How a row's value is chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    /// One of these words (string numbers, or the words themselves).
    Words(Vec<Word>),
    /// A number from `min` to `max` on a gauge.
    Number(i32, i32),
    /// A command whose keys are assigned on its own screen: no value on the list.
    Assign,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Word {
    Id(u32),
    Text(&'static str),
}

/// A row: its words, how it is set, the default and where Barrel keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: u32,
    pub text: u32,
    pub control: Control,
    pub default: i32,
    /// The config key (`option_` and this) for a game option; a graphics row names its item.
    pub key: &'static str,
    pub graphics: Option<Option<Item>>,
}

/// The string numbers the screen reads (the rest of the table is left out).
const WORD_RANGES: [(u32, u32); 12] = [(4_521_985, 4_522_200), (720_897, 721_100), (721_314, 721_320), (786_433, 786_600), (786_850, 786_856), (655_360, 655_700), (655_914, 655_924), (6_619_137 + 780, 6_619_137 + 790), (6_619_923, 6_619_925), (6_619_972, 6_619_974), (6_620_020, 6_620_050), (4_194_304, 4_194_400)];

/// The words a screen reads, by string number, from the string table's bytes.
pub fn read_strings(table: &[u8]) -> Result<HashMap<u32, String>, String> {
    let root = barrel_formats::table::parse(table).map_err(|e| format!("strcombineall: {e}"))?;
    Ok(root
        .children
        .iter()
        .filter_map(|s| Some((s.attr("String_No")?.parse::<u32>().ok()?, s.attr("Text")?.to_string())))
        .filter(|(n, _)| WORD_RANGES.iter().any(|&(a, b)| (a..b).contains(n)))
        .collect())
}

const ON_OFF: [Word; 2] = [Word::Id(4_522_011), Word::Id(4_522_012)];

fn onoff(name: u32, text: u32, key: &'static str, default: i32) -> Entry {
    Entry { name, text, control: Control::Words(ON_OFF.to_vec()), default, key, graphics: None }
}

fn words(name: u32, text: u32, key: &'static str, list: &[Word], default: i32) -> Entry {
    Entry { name, text, control: Control::Words(list.to_vec()), default, key, graphics: None }
}

fn number(name: u32, text: u32, key: &'static str, min: i32, max: i32, default: i32) -> Entry {
    Entry { name, text, control: Control::Number(min, max), default, key, graphics: None }
}

/// The tabs' words: Camera, Game, Audio, Graphics, Keyboard, Controller.
pub const TABS: [u32; 6] = [4_522_053, 4_522_051, 4_522_052, 720_898, 720_899, 655_407];
/// The tab the screen opens on (Game, as the capture).
pub const FIRST_TAB: usize = 1;
/// The Keyboard tab (a list of commands) and the Controller tab (a diagram).
const KEYS_TAB: usize = 4;
const PAD_TAB: usize = 5;

fn level_word(name: &'static str) -> Word {
    match name {
        "Low" => Word::Id(655_393),
        "Middle" => Word::Id(655_392),
        "High" => Word::Id(655_391),
        "Custom" => Word::Id(655_394),
        "Off" => Word::Id(655_398),
        "On" => Word::Id(655_397),
        "Fxaa" => Word::Id(655_638),
        "Taa" => Word::Id(655_637),
        "Linear" => Word::Id(655_641),
        "Anisotropic" => Word::Id(655_640),
        other => Word::Text(other),
    }
}

/// The words of a graphics item's levels, lowest first. The draw distances read Close, Normal
/// and Long and the grass density Low, Standard and High.
// STAND-IN: only the top words (Long, High) are the capture's (settings-tabs 1615, settings-scroll 1850); the lower ones are the string table's nearest words, unseen.
fn item_words(item: Item) -> Vec<Word> {
    match item {
        Item::DrawRange | Item::PointLights | Item::GrassRange => [655_400, 655_399, 655_404].map(Word::Id).to_vec(),
        Item::GrassDensity => [655_393, 655_392, 655_391].map(Word::Id).to_vec(),
        _ => item.levels().iter().map(|&l| level_word(l)).collect(),
    }
}

/// The rows of tab `tab`.
pub fn entries(tab: usize) -> Vec<Entry> {
    let normal_inverted = [Word::Id(4_522_008), Word::Id(4_522_009)];
    match tab {
        0 => vec![
            // The DX names ("Y-Axis Camera" ...) and order, as the capture (ryza-recomp settings-tabs, 1795).
            words(6_619_972, 4_522_099, "camera_vertical", &normal_inverted, 0),
            words(6_619_973, 4_522_100, "camera_horizontal", &normal_inverted, 0),
            words(4_522_134, 4_522_101, "zoom_controls", &normal_inverted, 0),
            number(6_619_923, 4_522_102, "camera_vertical_speed", 1, 10, 5),
            number(6_619_924, 4_522_103, "camera_horizontal_speed", 1, 10, 5),
            onoff(4_522_135, 4_522_104, "camera_autocorrect", 0),
        ],
        1 => vec![
            onoff(4_522_142, 4_522_112, "vibration", 0),
            words(4_522_143, 4_522_114, "difficulty", &[Word::Id(4_522_013), Word::Id(4_522_014), Word::Id(4_522_015), Word::Id(4_522_016), Word::Id(4_522_017)], 2),
            words(4_522_020, 4_522_107, "dash", &[Word::Id(4_522_023), Word::Id(4_522_024)], 0),
            onoff(4_522_021, 4_522_108, "map_rotation", 1),
            onoff(4_522_138, 4_522_109, "item_auto_select", 0),
            onoff(4_522_144, 4_522_115, "autosave", 0),
            onoff(4_522_145, 4_522_117, "save_order", 0),
            number(4_522_139, 4_522_110, "cursor_speed", 1, 10, 5),
            number(4_522_140, 4_522_111, "event_autoplay", 1, 10, 10),
            words(4_522_151, 4_522_126, "confirm_button", &[Word::Id(4_522_152), Word::Id(4_522_153)], 0),
            words(6_620_034, 6_620_035, "subtitle_size", &[Word::Id(6_620_036), Word::Id(6_620_037), Word::Id(6_620_038)], 0),
            onoff(6_620_041, 6_620_042, "toggle_icons", 0),
        ],
        2 => vec![
            // STAND-IN: the volumes' default 5 is the capture's (ryza-recomp settings-tabs: all four at 5 where every other value is at its default); the game's own defaults are not read.
            number(4_522_147, 4_522_119, "volume_bgm", 0, 10, 5),
            number(4_522_148, 4_522_120, "volume_sfx", 0, 10, 5),
            number(4_522_149, 4_522_121, "volume_voice", 0, 10, 5),
            number(4_522_150, 4_522_122, "volume_movie", 0, 10, 5),
        ],
        3 => {
            let item = |name: u32, i: Item| Entry { name, text: name + 65_536, control: Control::Words(item_words(i)), default: 0, key: "", graphics: Some(Some(i)) };
            let quality = Entry { name: 720_911, text: 720_911 + 65_536, control: Control::Words(["Low", "Middle", "High", "Custom"].map(level_word).to_vec()), default: 2, key: "", graphics: Some(None) };
            // The four display rows are stored settings (the next start reads the ones it has).
            let resolutions = [655_380, 655_379, 655_378, 655_377, 655_376, 655_375, 655_382, 655_383].map(Word::Id);
            vec![
                words(720_909, 786_445, "resolution", &resolutions, RESOLUTION_DEFAULT),
                words(720_910, 786_446, "window_mode", &[Word::Id(655_385), Word::Id(655_386), Word::Id(655_387)], 0),
                words(721_315, 786_851, "frame_rate", &[655_646, 655_645, 655_644, 655_643, 655_647].map(Word::Id), 1),
                onoff(721_314, 786_850, "vsync", 0),
                quality,
                item(720_912, Item::Texture),
                item(720_913, Item::Shadow),
                item(720_914, Item::Reflections),
                item(720_915, Item::Effects),
                item(720_916, Item::AntiAliasing),
                item(720_917, Item::DepthOfField),
                item(720_918, Item::Occlusion),
                item(720_919, Item::Bloom),
                item(720_920, Item::LightShafts),
                item(720_921, Item::DrawRange),
                item(720_922, Item::PointLights),
                item(720_923, Item::GrassRange),
                item(720_924, Item::GrassDensity),
                item(721_318, Item::Filtering),
                item(721_316, Item::ShadowDistance),
                item(721_319, Item::Water),
                item(721_317, Item::LodDistance),
            ]
        }
        // The key assignments' commands (Field, Combat, Synthesis); the pad's tab is a diagram without rows.
        4 => (655_914..=655_923).map(|name| Entry { name, text: 0, control: Control::Assign, default: 0, key: "", graphics: None }).collect(),
        // The controller's sixteen buttons, the left side then the right (they have no words on the list).
        5 => (0..16).map(|_| Entry { name: 0, text: 0, control: Control::Assign, default: 0, key: "", graphics: None }).collect(),
        _ => Vec::new(),
    }
}

/// The sizes of the Screen Resolution row, in its order (the string numbers `655_380` ... `655_383` read the same).
const RESOLUTIONS: [&str; 8] = ["960x540", "1024x576", "1280x720", "1366x768", "1600x900", "1920x1080", "2560x1440", "3840x2160"];
const RESOLUTION_DEFAULT: i32 = 5;

/// The settings the screen shows and changes.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub tab: usize,
    pub cursor: [usize; TABS.len()],
    /// The first row shown on each tab (the list scrolls when the cursor leaves its window).
    pub first: [usize; TABS.len()],
    /// The game options' values by their keys.
    values: HashMap<&'static str, i32>,
    pub graphics: graphics::Settings,
    /// Something was changed since the screen opened.
    pub changed: bool,
}

impl State {
    pub fn new(graphics: graphics::Settings) -> State {
        let mut values = HashMap::new();
        for tab in 0..TABS.len() {
            for e in entries(tab).iter().filter(|e| !e.key.is_empty()) {
                values.insert(e.key, e.default);
            }
        }
        State { tab: FIRST_TAB, cursor: [0; TABS.len()], first: [0; TABS.len()], values, graphics, changed: false }
    }

    fn value(&self, e: &Entry) -> i32 {
        match e.graphics {
            Some(None) => self.graphics.quality as i32,
            Some(Some(i)) => self.graphics.level(i) as i32,
            None => self.values.get(e.key).copied().unwrap_or(e.default),
        }
    }

    /// The value of the game option `key`.
    pub fn option(&self, key: &str) -> Option<i32> {
        self.values.get(key).copied()
    }

    /// Moves the cursor by `way` rows (wrapping).
    pub fn step(&mut self, way: i32) {
        let n = entries(self.tab).len().max(1) as i32;
        self.cursor[self.tab] = (self.cursor[self.tab] as i32 + way).rem_euclid(n) as usize;
    }

    /// Scrolls the tab's list so the cursor is in its window of twelve rows.
    pub fn follow(&mut self) {
        let (tab, on) = (self.tab, self.cursor[self.tab]);
        self.first[tab] = self.first[tab].clamp(on.saturating_sub(11), on);
    }

    /// Moves to another tab, by `way` tabs (stopping at the ends).
    pub fn tab_by(&mut self, way: i32) {
        // The game wraps (RB on Controller opens Camera: ryza-recomp scenarios/settings-tabs.sh).
        self.tab = (self.tab as i32 + way).rem_euclid(TABS.len() as i32) as usize;
    }

    /// Changes the row under the cursor by `way` (a choice wraps, a number stops at its ends).
    pub fn change(&mut self, way: i32) {
        let rows = entries(self.tab);
        let Some(e) = rows.get(self.cursor[self.tab]) else { return };
        let now = self.value(e);
        let next = match &e.control {
            Control::Words(w) => (now + way).rem_euclid(w.len() as i32),
            Control::Number(lo, hi) => (now + way).clamp(*lo, *hi),
            Control::Assign => return,
        };
        if next == now {
            return;
        }
        self.changed = true;
        match e.graphics {
            Some(None) => {
                self.graphics.quality = [Quality::Low, Quality::Middle, Quality::High, Quality::Custom][next as usize];
                self.graphics.given = true;
            }
            Some(Some(i)) => {
                // Changing an item sets the quality to Custom, as the game's.
                self.graphics.quality = Quality::Custom;
                self.graphics.set(i, next as usize);
                self.graphics.given = true;
            }
            None => {
                self.values.insert(e.key, next);
            }
        }
    }

    /// The row under the cursor on `tab` and the screen's words for it.
    pub fn screen(&self, strings: &HashMap<u32, String>) -> Screen {
        let word = |n: u32| strings.get(&n).map_or(String::new(), |s| keys(s));
        let say = |w: &Word| match w {
            Word::Id(n) => word(*n),
            Word::Text(t) => (*t).to_string(),
        };
        let rows_in = entries(self.tab);
        let rows: Vec<Row> = rows_in
            .iter()
            .map(|e| {
                let v = self.value(e);
                match &e.control {
                    Control::Words(w) => Row { name: word(e.name), value: say(&w[v as usize % w.len()]), kind: Kind::Select },
                    Control::Number(lo, hi) => Row { name: word(e.name), value: PLACEHOLDER_VALUE.to_string(), kind: Kind::Gauge((v - lo) as f32 / (hi - lo).max(1) as f32, v.to_string()) },
                    Control::Assign => Row { name: word(e.name), value: String::new(), kind: Kind::Select },
                }
            })
            .collect();
        let on = self.cursor[self.tab].min(rows_in.len().saturating_sub(1));
        // The Keyboard tab's panel stays empty (its commands have no description).
        let (title, text) = rows_in.get(on).filter(|_| self.tab != KEYS_TAB).map_or((String::new(), String::new()), |e| (word(e.name), word(e.text)));
        Screen {
            caption: word(720_903),
            tabs: TABS.iter().map(|&t| word(t)).collect(),
            tab: self.tab,
            rows,
            cursor: on,
            first: self.first[self.tab],
            title,
            text,
            // "Change" only on a tab with a choice row (the Audio tab's gauges have none: ryza-recomp
            // settings-tabs 1555; on the Game tab it stays whatever row the cursor is on).
            help: [('\u{4e1}', word(4_521_991))]
                .into_iter()
                .filter(|_| self.tab != PAD_TAB)
                .chain(rows_in.iter().any(|e| matches!(e.control, Control::Words(_))).then(|| ('\u{4e0}', word(4_521_992))))
                .chain([('\u{4dc}', word(655_441)), ('\u{4d8}', word(4_521_989))])
                .chain((self.tab >= KEYS_TAB).then(|| ('\u{4da}', word(655_436))))
                .collect(),
            page: match self.tab {
                KEYS_TAB => Page::Keys,
                PAD_TAB => Page::Pad,
                _ => Page::List,
            },
            shoulders: ['\u{4e3}', '\u{4e7}'],
        }
    }

    /// The game options and graphics items as `key value` lines for Barrel's config.
    pub fn config_lines(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        // The graphics keys only once the player has set any (they switch Barrel's own choices for the game's presets).
        if self.graphics.given {
            out.push(("quality".into(), ["Low", "Middle", "High", "Custom"][self.graphics.quality as usize].to_string()));
            for i in Item::ALL {
                out.push((i.config_key().to_string(), i.levels()[self.graphics.level(i)].to_string()));
            }
        }
        // The display rows go to the keys the window reads at its next start, once either was moved off its default.
        let (resolution, mode) = (self.values["resolution"], self.values["window_mode"]);
        if (resolution, mode) != (RESOLUTION_DEFAULT, 0) {
            out.push(("window".into(), RESOLUTIONS[resolution as usize % RESOLUTIONS.len()].into()));
            out.push(("fullscreen".into(), if mode == 0 { "off" } else { "on" }.into()));
        }
        let mut keys: Vec<_> = self.values.iter().collect();
        keys.sort();
        for (k, v) in keys {
            out.push((format!("option_{k}"), v.to_string()));
        }
        out
    }

    /// `config` with this state's lines in it (the others kept as they are).
    pub fn store(&self, config: &str) -> String {
        let lines = self.config_lines();
        let mut out: Vec<String> = Vec::new();
        let mut done = vec![false; lines.len()];
        for line in config.lines() {
            match line.trim().split_once(' ').and_then(|(k, _)| lines.iter().position(|(key, _)| key == k)) {
                Some(n) => {
                    out.push(format!("{} {}", lines[n].0, lines[n].1));
                    done[n] = true;
                }
                None => out.push(line.to_string()),
            }
        }
        for (n, (k, v)) in lines.iter().enumerate() {
            if !done[n] {
                out.push(format!("{k} {v}"));
            }
        }
        out.join("\n") + "\n"
    }

    /// Takes the game options of `config` (`option_` keys; the graphics keys are `graphics::Settings`'s).
    pub fn load(&mut self, config: &str) {
        for line in config.lines() {
            let Some((key, value)) = line.trim().split_once(' ') else { continue };
            let Some(name) = key.strip_prefix("option_") else { continue };
            if let (Some(slot), Ok(v)) = (self.values.iter_mut().find(|(k, _)| **k == name), value.trim().parse::<i32>()) {
                *slot.1 = v;
            }
        }
    }
}

/// A gauge row keeps the row template's value word, which the game does not clear.
const PLACEHOLDER_VALUE: &str = "1920x1080";

/// The key tags of the words as the glyphs of the keyboard's and pad's buttons (`<KEY_ASSIGN_FIELD_DASH>`
/// `_MOVE`, `_CAMERA_MOVE`, `<IM00>`; the other tags are left out).
// STAND-IN: the buttons' glyphs are the capture's (the game picks them for the pad's assignment).
fn keys(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(a) = rest.find('<') {
        out.push_str(&rest[..a]);
        let Some(b) = rest[a..].find('>') else { break };
        let tag = &rest[a + 1..a + b];
        match tag {
            "KEY_ASSIGN_FIELD_DASH" => out.push('\u{4e7}'),
            "KEY_ASSIGN_FIELD_MOVE" => out.push('\u{4f2}'),
            "KEY_ASSIGN_FIELD_CAMERA_MOVE" => out.push('\u{4f3}'),
            "IM00" => out.push('\u{4da}'),
            t if t.starts_with("CL") => out.push_str(&rest[a..=a + b]),
            _ => {}
        }
        rest = &rest[a + b + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_wraps_and_a_held_repeat_stops_at_the_end() {
        let hold = |from, until, way| Hold { from, until: Some(until), way };
        let input = |holds| Input { list_holds: holds, ..Input::default() };
        // Five rows from the first: one press up wraps to the last.
        assert_eq!(cursor_on(&input(vec![hold(10, 11, -1)]), 0, 5, 20), 4);
        // Held down from the second row of five: steps at 100, 130, 140, 150 reach the last at 140 and stay.
        let held = input(vec![hold(100, 400, 1)]);
        let at = |f| cursor_on(&held, 1, 5, f);
        assert_eq!([at(99), at(100), at(129), at(130), at(140), at(150), at(300)], [1, 2, 2, 3, 4, 4, 4]);
        // A fresh press at the end wraps.
        let again = input(vec![hold(100, 400, 1), hold(500, 501, 1)]);
        assert_eq!(cursor_on(&again, 1, 5, 500), 0);
    }

    #[test]
    fn the_screen_comes_up_after_the_cover() {
        assert_eq!(Stage::at(None, 100), Stage::Title);
        assert_eq!(Stage::at(Some(100), 99), Stage::Title);
        assert_eq!(Stage::at(Some(100), 105), Stage::Cover(6.0 / 18.0));
        assert_eq!(Stage::at(Some(100), 119), Stage::Cover(1.0));
        assert_eq!(Stage::at(Some(100), 130), Stage::Screen { age: 11 });
    }

    #[test]
    fn a_change_goes_to_the_config_and_back() {
        let mut s = State::new(graphics::Settings::default());
        // Game tab: Vibration On -> Off, Difficulty Normal -> Hard, Cursor Speed 5 -> 6.
        s.change(1);
        s.step(1);
        s.change(1);
        for _ in 0..6 {
            s.step(1);
        }
        s.change(1);
        assert_eq!((s.option("vibration"), s.option("difficulty"), s.option("cursor_speed")), (Some(1), Some(3), Some(6)));
        // Nothing of the graphics is written until one of its rows is changed.
        assert!(!s.store("").contains("quality"));
        // Graphics tab: an item turns the quality to Custom.
        s.tab = 3;
        s.cursor[3] = 11;
        s.change(-1);
        assert_eq!(s.graphics.quality, Quality::Custom);
        let text = s.store("window_width 1280\nvibration x\nao Off\n");
        assert!(text.contains("window_width 1280"));
        assert!(text.contains("option_vibration 1"));
        assert!(text.contains("quality Custom"));
        // The line that was there is replaced, not repeated.
        assert_eq!(text.matches("\nao ").count() + usize::from(text.starts_with("ao ")), 1);
        let mut back = State::new(graphics::Settings::default());
        back.load(&text);
        back.graphics.apply_config(&text);
        assert_eq!(back.option("difficulty"), Some(3));
        assert_eq!(back.option("cursor_speed"), Some(6));
        assert_eq!(back.graphics.quality, Quality::Custom);
        assert_eq!(back.graphics.level(Item::Occlusion), s.graphics.level(Item::Occlusion));
    }

    #[test]
    fn the_display_rows_go_to_the_windows_keys_once_moved() {
        let mut s = State::new(graphics::Settings::default());
        assert!(!s.store("").contains("\nwindow ") && !s.store("").starts_with("window "));
        // Graphics tab: Screen Resolution one step down (1600x900), Window Settings one on (Borderless).
        s.tab = 3;
        s.change(-1);
        s.cursor[3] = 1;
        s.change(1);
        let text = s.store("fullscreen off\nwindow 800x600\n");
        assert!(text.contains("window 1600x900") && text.contains("fullscreen on"));
        assert!(!text.contains("800x600"));
    }

    #[test]
    fn the_list_scrolls_when_the_cursor_leaves_its_window() {
        let mut s = State::new(graphics::Settings::default());
        s.tab = 3;
        for (on, first) in [(11, 0), (12, 1), (21, 10), (15, 10), (9, 9), (0, 0)] {
            s.cursor[3] = on;
            s.follow();
            assert_eq!(s.first[3], first, "cursor {on}");
        }
    }

    #[test]
    fn a_description_reads_its_key_tags() {
        assert_eq!(keys("Use <KEY_ASSIGN_FIELD_DASH> to change."), "Use \u{4e7} to change.");
        assert_eq!(keys("a<CLGR>b<CLNR>"), "a<CLGR>b<CLNR>");
    }
}
