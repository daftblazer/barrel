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
use crate::options_view::{Kind, Row, Screen};
use crate::title_menu::Input;

/// The black cover rises as the load screen's (`load::COVER_RISE`, 20 frames to full) and the
/// screen fades in from the frame after it is full (capture: bg alpha 11/15 on the 30th frame
/// after the button).
const COVER_FULL: i32 = 20;
const COVER_RISE: f32 = 18.0;
const SCREEN_AGE_AT: i32 = COVER_FULL - 1;

/// The blink's clock against the frames: the capture's 120-frame blink ran 0.8% slow against its
/// frame count (alpha of the cursor on five frames, 150 to 650 frames in, fitted by hand); a
/// window's own clock is the frame count itself.
// STAND-IN: the rate and the start are fitted to the capture's five cursor alphas (all within 1.5 / 255); the real thing is the game's clock, which advances by its own frame times.
pub const BLINK_RATE: f32 = 0.9918;
pub const BLINK_START: f32 = 0.3;

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

/// The rows' directions held on the screen, as the title's list: `input.list_holds`.
pub fn cursor_on(input: &Input, count: usize, frame: i32) -> usize {
    crate::load::cursor(&input.list_holds, count, frame)
}

/// How a row's value is chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    /// One of these words (string numbers, or the words themselves).
    Words(Vec<Word>),
    /// A number from `min` to `max` on a gauge.
    Number(i32, i32),
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
const WORD_RANGES: [(u32, u32); 7] = [(4_521_985, 4_522_200), (720_897, 721_100), (786_433, 786_600), (655_360, 655_700), (6_619_137 + 780, 6_619_137 + 790), (6_620_020, 6_620_050), (4_194_304, 4_194_400)];

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
        other => Word::Text(other),
    }
}

/// The rows of tab `tab`.
pub fn entries(tab: usize) -> Vec<Entry> {
    let normal_inverted = [Word::Id(4_522_008), Word::Id(4_522_009)];
    match tab {
        0 => vec![
            words(4_522_130, 4_522_099, "camera_vertical", &normal_inverted, 0),
            words(4_522_131, 4_522_100, "camera_horizontal", &normal_inverted, 0),
            number(4_522_132, 4_522_102, "camera_vertical_speed", 1, 10, 5),
            number(4_522_133, 4_522_103, "camera_horizontal_speed", 1, 10, 5),
            words(4_522_134, 4_522_101, "zoom_controls", &normal_inverted, 0),
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
            number(4_522_147, 4_522_119, "volume_bgm", 0, 10, 10),
            number(4_522_148, 4_522_120, "volume_sfx", 0, 10, 10),
            number(4_522_149, 4_522_121, "volume_voice", 0, 10, 10),
            number(4_522_150, 4_522_122, "volume_movie", 0, 10, 10),
        ],
        3 => {
            let item = |name: u32, i: Item| Entry { name, text: name + 65_536, control: Control::Words(i.levels().iter().map(|&l| level_word(l)).collect()), default: 0, key: "", graphics: Some(Some(i)) };
            let quality = Entry { name: 720_911, text: 720_911 + 65_536, control: Control::Words(["Low", "Middle", "High", "Custom"].map(level_word).to_vec()), default: 2, key: "", graphics: Some(None) };
            vec![
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
            ]
        }
        _ => Vec::new(),
    }
}

/// The settings the screen shows and changes.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub tab: usize,
    pub cursor: [usize; TABS.len()],
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
        State { tab: FIRST_TAB, cursor: [0; TABS.len()], values, graphics, changed: false }
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

    /// Moves to another tab, by `way` tabs (stopping at the ends).
    pub fn tab_by(&mut self, way: i32) {
        self.tab = (self.tab as i32 + way).clamp(0, TABS.len() as i32 - 1) as usize;
    }

    /// Changes the row under the cursor by `way` (a choice wraps, a number stops at its ends).
    pub fn change(&mut self, way: i32) {
        let rows = entries(self.tab);
        let Some(e) = rows.get(self.cursor[self.tab]) else { return };
        let now = self.value(e);
        let next = match &e.control {
            Control::Words(w) => (now + way).rem_euclid(w.len() as i32),
            Control::Number(lo, hi) => (now + way).clamp(*lo, *hi),
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
                }
            })
            .collect();
        let on = self.cursor[self.tab].min(rows_in.len().saturating_sub(1));
        let (title, text) = rows_in.get(on).map_or((String::new(), String::new()), |e| (word(e.name), word(e.text)));
        Screen {
            caption: word(720_903),
            tabs: TABS.iter().map(|&t| word(t)).collect(),
            tab: self.tab,
            rows,
            cursor: on,
            title,
            text,
            help: vec![('\u{4e1}', word(4_521_991)), ('\u{4e0}', word(4_521_992)), ('\u{4dc}', word(655_441)), ('\u{4d8}', word(4_521_989))],
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
/// and `_MOVE`, `<IM00>`; the other tags are left out).
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
        s.cursor[3] = 7;
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
    fn a_description_reads_its_key_tags() {
        assert_eq!(keys("Use <KEY_ASSIGN_FIELD_DASH> to change."), "Use \u{4e7} to change.");
        assert_eq!(keys("a<CLGR>b<CLNR>"), "a<CLGR>b<CLNR>");
    }
}
