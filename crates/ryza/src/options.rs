//! The Settings screen the title menu opens: what it lists on each tab, the value of each row,
//! the cursor and the tabs, how the values go to and come from Barrel's own config, and the
//! path from the menu's button to the screen.
//!
//! The Graphics tab also carries Barrel's own engine rows (render scale, upscaler, sharpening) and
//! extends the game's anti-aliasing and texture filtering with the choices Barrel adds (MSAA, FSR
//! native AA, 2x to 8x filtering), woven in next to the game's rows; a choice that would conflict
//! with another is closed and the description panel names why (`barrel_render::graphics_options`,
//! as Escha's page). `--settings-as-game` leaves them out (the captures' tab).
//!
//! The pages are the game's: Camera, Game, Audio and Graphics (Keyboard and Controller, the
//! key assignments, are not built: `docs/ryza/open.md`, "Settings screen"). The words come from
//! the string tables; the order of the rows on the Game tab is the capture's, the others' the
//! string tables'.

use std::collections::HashMap;

use barrel_render::graphics_options::{self as g, Aa, Row as EngineRow, Up};
use barrel_render::settings::{Pacing, RenderSettings, Upscaler};

use crate::graphics::{self, Item, Quality};
use crate::options_view::{Ask, Kind, Page, Row, Screen};
use crate::title_menu::{Hold, Input, Repeat};

/// The black cover rises as the load screen's (`load::COVER_RISE`, 20 frames to full) and the
/// screen fades in from the frame after it is full (capture: bg alpha 11/15 on the 30th frame
/// after the button).
pub const COVER_FULL: i32 = 20;
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

/// A row Barrel adds to the game's page, or a game row it extends: how its choices are made of
/// the render settings (`State::engine_choice`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Scale,
    Upscaler,
    Sharpen,
    /// The game's Off / FXAA / TAA, then MSAA 2x, 4x, 8x and FSR native AA.
    AntiAliasing,
    /// The game's Linear, then anisotropic 2x, 4x, 8x and 16x (the game's Anisotropic).
    Filtering,
}

impl Engine {
    /// The choices that can be made (a row may show a value that is not one of them: "By the upscaler").
    fn choices(self) -> usize {
        match self {
            Engine::Scale => g::SCALES.len(),
            Engine::Upscaler => g::UP_CHOICES.len(),
            Engine::Sharpen => 2,
            Engine::AntiAliasing => 7,
            Engine::Filtering => g::FILTERING.len(),
        }
    }

    /// The row's description in Barrel's words (the rows the game lacks), by `graphics_options`.
    fn description(self) -> Option<&'static str> {
        match self {
            Engine::Scale => Some(g::description(EngineRow::Scale)),
            Engine::Upscaler => Some(g::description(EngineRow::Upscaling)),
            Engine::Sharpen => Some(g::description(EngineRow::Sharpening)),
            Engine::AntiAliasing | Engine::Filtering => None,
        }
    }
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
    /// A row of Barrel's own is named in Barrel's words, not the game's (`name` is 0).
    pub title: &'static str,
    pub engine: Option<Engine>,
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
    Entry { name, text, control: Control::Words(ON_OFF.to_vec()), default, key, graphics: None, title: "", engine: None }
}

fn words(name: u32, text: u32, key: &'static str, list: &[Word], default: i32) -> Entry {
    Entry { name, text, control: Control::Words(list.to_vec()), default, key, graphics: None, title: "", engine: None }
}

fn number(name: u32, text: u32, key: &'static str, min: i32, max: i32, default: i32) -> Entry {
    Entry { name, text, control: Control::Number(min, max), default, key, graphics: None, title: "", engine: None }
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

/// Barrel's own words for the choices the game has none for.
const SCALE_WORDS: [&str; 8] = ["50%", "67%", "75%", "85%", "100%", "125%", "150%", "200%"];
const UPSCALER_WORDS: [&str; 5] = ["Off", "FSR 1", "GSR 1", "GSR 2", "FSR 3.1"];
const FILTER_WORDS: [&str; 4] = ["2x", "4x", "8x", "16x"];
/// The anti-aliasing the game does not have, and the value of a temporal upscaler's own.
const MSAA_WORDS: [&str; 3] = ["MSAA 2x", "MSAA 4x", "MSAA 8x"];
const NATIVE_WORD: &str = "FSR Native AA";
const BY_UPSCALER_WORD: &str = "By the upscaler";

/// A row of Barrel's own: named in its words, its choices the render settings'.
fn own(title: &'static str, which: Engine, words: &[&'static str], text: u32) -> Entry {
    Entry { name: 0, text, control: Control::Words(words.iter().map(|&w| Word::Text(w)).collect()), default: 0, key: "", graphics: None, title, engine: Some(which) }
}

/// The rows of tab `tab`; `extended` has the Graphics tab carry Barrel's rows and choices.
pub fn entries(tab: usize, extended: bool) -> Vec<Entry> {
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
            let item = |name: u32, i: Item| Entry { name, text: name + 65_536, control: Control::Words(item_words(i)), default: 0, key: "", graphics: Some(Some(i)), title: "", engine: None };
            let quality = Entry { name: 720_911, text: 720_911 + 65_536, control: Control::Words(["Low", "Middle", "High", "Custom"].map(level_word).to_vec()), default: 2, key: "", graphics: Some(None), title: "", engine: None };
            // The game's anti-aliasing and filtering with Barrel's choices after theirs.
            let anti_aliasing = match extended {
                true => Entry { control: Control::Words(item_words(Item::AntiAliasing).into_iter().chain(MSAA_WORDS.map(Word::Text)).chain([Word::Text(NATIVE_WORD), Word::Text(BY_UPSCALER_WORD)]).collect()), engine: Some(Engine::AntiAliasing), ..item(720_916, Item::AntiAliasing) },
                false => item(720_916, Item::AntiAliasing),
            };
            let filtering = match extended {
                true => Entry { control: Control::Words(item_words(Item::Filtering).into_iter().take(1).chain(FILTER_WORDS.map(Word::Text)).collect()), engine: Some(Engine::Filtering), ..item(721_318, Item::Filtering) },
                false => item(721_318, Item::Filtering),
            };
            let engine_rows = [
                own("Render Scale", Engine::Scale, &SCALE_WORDS, 0),
                own("Upscaler", Engine::Upscaler, &UPSCALER_WORDS, 0),
                own("Sharpening", Engine::Sharpen, &["Off", "On"], 0),
            ];
            // The four display rows are stored settings (the next start reads the ones it has).
            let resolutions = [655_380, 655_379, 655_378, 655_377, 655_376, 655_375, 655_382, 655_383].map(Word::Id);
            let [scale, upscaler, sharpen] = engine_rows;
            let mut rows = vec![
                words(720_909, 786_445, "resolution", &resolutions, RESOLUTION_DEFAULT),
                words(720_910, 786_446, "window_mode", &[Word::Id(655_385), Word::Id(655_386), Word::Id(655_387)], 0),
            ];
            if extended {
                rows.extend([scale, upscaler, sharpen]);
            }
            rows.extend([
                words(721_315, 786_851, "frame_rate", &[655_646, 655_645, 655_644, 655_643, 655_647].map(Word::Id), 1),
                onoff(721_314, 786_850, "vsync", 0),
                quality,
                item(720_912, Item::Texture),
                item(720_913, Item::Shadow),
                item(720_914, Item::Reflections),
                item(720_915, Item::Effects),
                anti_aliasing,
                item(720_917, Item::DepthOfField),
                item(720_918, Item::Occlusion),
                item(720_919, Item::Bloom),
                item(720_920, Item::LightShafts),
                item(720_921, Item::DrawRange),
                item(720_922, Item::PointLights),
                item(720_923, Item::GrassRange),
                item(720_924, Item::GrassDensity),
                filtering,
                item(721_316, Item::ShadowDistance),
                item(721_319, Item::Water),
                item(721_317, Item::LodDistance),
            ]);
            rows
        }
        // The key assignments' commands (Field, Combat, Synthesis); the pad's tab is a diagram without rows.
        4 => (655_914..=655_923).map(|name| Entry { name, text: 0, control: Control::Assign, default: 0, key: "", graphics: None, title: "", engine: None }).collect(),
        // The controller's sixteen buttons, the left side then the right (they have no words on the list).
        5 => (0..16).map(|_| Entry { name: 0, text: 0, control: Control::Assign, default: 0, key: "", graphics: None, title: "", engine: None }).collect(),
        _ => Vec::new(),
    }
}

/// The sizes of the Screen Resolution row, in its order (the string numbers `655_380` ... `655_383` read the same).
const RESOLUTIONS: [&str; 8] = ["960x540", "1024x576", "1280x720", "1366x768", "1600x900", "1920x1080", "2560x1440", "3840x2160"];
const RESOLUTION_DEFAULT: i32 = 5;
/// The Frame Rate Limit row's choices in its order (the words 30, 60, 120, 144, Unlimited), in frames a second.
const FRAME_LIMITS: [Option<u32>; 5] = [Some(30), Some(60), Some(120), Some(144), None];

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
    /// The render settings the run starts from (the config's, before the game's presets draw
    /// their passes over them): what Barrel's own rows show and change.
    pub render: RenderSettings,
    /// The Graphics tab carries Barrel's rows and choices (not under `--settings-as-game`).
    pub extended: bool,
    /// One of Barrel's render choices was changed (only then are its keys written to the config).
    pub render_changed: bool,
    /// Something was changed since the screen opened.
    pub changed: bool,
}

impl State {
    pub fn new(graphics: graphics::Settings, render: RenderSettings) -> State {
        let mut values = HashMap::new();
        for tab in 0..TABS.len() {
            for e in entries(tab, false).iter().filter(|e| !e.key.is_empty()) {
                values.insert(e.key, e.default);
            }
        }
        State { tab: FIRST_TAB, cursor: [0; TABS.len()], first: [0; TABS.len()], values, graphics, render, extended: true, render_changed: false, changed: false }
    }

    /// The tab's rows as this screen has them.
    pub fn entries(&self, tab: usize) -> Vec<Entry> {
        entries(tab, self.extended)
    }

    /// Choice `k` of the row `which`: the game's items and the render settings it leaves, or why it is closed.
    fn engine_choice(&self, which: Engine, k: usize) -> Result<(graphics::Settings, RenderSettings), &'static str> {
        let (mut game, mut r) = (self.graphics, self.render);
        match which {
            Engine::Scale => g::set_scale(&mut r, g::SCALES[k]),
            Engine::Upscaler => {
                g::up_open(&r, g::UP_CHOICES[k])?;
                g::set_upscaler(&mut r, g::UP_CHOICES[k]);
            }
            Engine::Sharpen => {
                g::sharpen_open(&r)?;
                g::set_sharpen(&mut r, k == 1);
            }
            Engine::AntiAliasing => {
                // The game's Off, FXAA and TAA are its item (its preset draws them); MSAA and native AA are Barrel's, with the item Off.
                let barrel = [Aa::Off, Aa::Off, Aa::Taa, Aa::Msaa(2), Aa::Msaa(4), Aa::Msaa(8), Aa::Native][k];
                g::aa_open(&r, barrel)?;
                game.set(Item::AntiAliasing, if k < 3 { k } else { 0 });
                game.quality = Quality::Custom;
                g::set_aa(&mut r, if k < 3 { Aa::Off } else { barrel });
            }
            Engine::Filtering => {
                // Anisotropic is the game's item; the count is Barrel's.
                let n = g::FILTERING[k];
                game.set(Item::Filtering, usize::from(n > 1));
                game.quality = Quality::Custom;
                r.anisotropy = n;
            }
        }
        Ok((game, r))
    }

    /// The choice of the row `which` the settings stand on (past the choices for a value that is not one).
    fn engine_value(&self, which: Engine) -> usize {
        let r = &self.render;
        match which {
            Engine::Scale => g::SCALES.iter().position(|&k| (k - r.scale).abs() < 0.005).unwrap_or_else(|| g::SCALES.iter().rposition(|&k| k <= r.scale).unwrap_or(0)),
            Engine::Upscaler => g::UP_CHOICES.iter().position(|&u| u == g::upscaler(r)).unwrap_or(0),
            Engine::Sharpen => usize::from(r.sharpen),
            Engine::AntiAliasing => match g::aa(r) {
                Aa::Native => 6,
                Aa::Msaa(n) => 2 + (n.max(2).ilog2() as usize),
                Aa::ByUpscaler => 7,
                // Barrel's own TAA (a config's, no game settings given) shows as the game's.
                Aa::Taa if !self.graphics.given => 2,
                _ => self.graphics.level(Item::AntiAliasing),
            },
            Engine::Filtering if self.graphics.level(Item::Filtering) == 0 => 0,
            Engine::Filtering => g::FILTERING.iter().position(|&n| n == r.anisotropy.max(2)).unwrap_or(g::FILTERING.len() - 1),
        }
    }

    /// The choices of `which` closed now with their reasons.
    fn closed(&self, which: Engine) -> Vec<(usize, &'static str)> {
        (0..which.choices()).filter_map(|k| self.engine_choice(which, k).err().map(|why| (k, why))).collect()
    }

    fn value(&self, e: &Entry) -> i32 {
        if let Some(which) = e.engine {
            return self.engine_value(which) as i32;
        }
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
        let n = self.entries(self.tab).len().max(1) as i32;
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
        let rows = self.entries(self.tab);
        let Some(e) = rows.get(self.cursor[self.tab]) else { return };
        let now = self.value(e);
        if let Some(which) = e.engine {
            // To the next choice open the way `way` goes (the closed ones are skipped, as Escha's page).
            let n = which.choices() as i32;
            let at = if (0..n).contains(&now) { now } else if way > 0 { -1 } else { n };
            let next = (1..=n).map(|k| (at + way * k).rem_euclid(n)).find(|&k| k != now && self.engine_choice(which, k as usize).is_ok());
            if let Some(k) = next {
                let (game, render) = self.engine_choice(which, k as usize).expect("an open choice");
                (self.graphics, self.render) = (game, render);
                self.render_changed = true;
                self.changed = true;
            }
            return;
        }
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
                // A quality preset sets the game's anti-aliasing: Barrel's goes.
                if self.graphics.quality != Quality::Custom && matches!(g::aa(&self.render), Aa::Msaa(_) | Aa::Native | Aa::Taa) {
                    g::set_aa(&mut self.render, Aa::Off);
                    self.render_changed = true;
                }
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
        let rows_in = self.entries(self.tab);
        let name = |e: &Entry| if e.title.is_empty() { word(e.name) } else { e.title.to_string() };
        let rows: Vec<Row> = rows_in
            .iter()
            .map(|e| {
                let v = self.value(e);
                match &e.control {
                    Control::Words(w) => Row { name: name(e), value: say(&w[v as usize % w.len()]), kind: Kind::Select },
                    Control::Number(lo, hi) => Row { name: name(e), value: PLACEHOLDER_VALUE.to_string(), kind: Kind::Gauge((v - lo) as f32 / (hi - lo).max(1) as f32, v.to_string()) },
                    Control::Assign => Row { name: name(e), value: String::new(), kind: Kind::Select },
                }
            })
            .collect();
        let on = self.cursor[self.tab].min(rows_in.len().saturating_sub(1));
        // The Keyboard tab's panel stays empty (its commands have no description).
        let (title, text) = rows_in.get(on).filter(|_| self.tab != KEYS_TAB).map_or((String::new(), String::new()), |e| (name(e), self.description(e, &word, &say)));
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
            ask: Ask {
                text: word(655_372),
                yes: word(4_194_375),
                no: word(4_194_376),
                // Select, Back and Confirm.
                help: [('\u{4e1}', word(4_521_991)), ('\u{4d8}', word(4_521_989)), ('\u{4da}', word(4_521_988))].to_vec(),
            },
            leave: Default::default(),
        }
    }

    /// The row's description: the game's words, or Barrel's for its own rows, and under it in red
    /// the choices closed now with the reasons (a row closed whole for one reason: the reason alone).
    fn description(&self, e: &Entry, word: &dyn Fn(u32) -> String, say: &dyn Fn(&Word) -> String) -> String {
        let Some(which) = e.engine else { return word(e.text) };
        let first = which.description().map_or_else(|| word(e.text), str::to_string);
        let closed = self.closed(which);
        if closed.is_empty() {
            return first;
        }
        let Control::Words(words) = &e.control else { return first };
        let mut groups: Vec<(Vec<String>, &str)> = Vec::new();
        for (k, why) in &closed {
            match groups.iter_mut().find(|(_, w)| w == why) {
                Some((labels, _)) => labels.push(say(&words[*k])),
                None => groups.push((vec![say(&words[*k])], why)),
            }
        }
        let lines: Vec<String> = match closed.len() == which.choices() && groups.len() == 1 {
            true => vec![groups[0].1.to_string()],
            false => groups.iter().map(|(labels, why)| format!("{}: {why}", labels.join(", "))).collect(),
        };
        format!("{first}^00<CLRE>{}<CLNR>", lines.join("^00"))
    }

    /// The Vertical Sync and Frame Rate Limit rows as the window paces its frames.
    pub fn pacing(&self) -> Pacing {
        Pacing { vsync: self.values["vsync"] == 0, limit: FRAME_LIMITS.get(self.values["frame_rate"] as usize).copied().flatten() }
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
        // Barrel's own render settings (the keys `RenderSettings::from_config_over` reads).
        let (r, on) = (&self.render, |b: bool| if b { "on" } else { "off" }.to_string());
        let upscaler = match r.upscaler {
            Upscaler::Bilinear => "bilinear",
            Upscaler::Fsr1 => "fsr1",
            Upscaler::Gsr1 => "gsr1",
        };
        // (only once one of them was changed: the config otherwise keeps what a preset or a launcher put there)
        out.extend(self.render_changed.then_some([
            ("scale".to_string(), r.scale.to_string()),
            ("upscaler".to_string(), upscaler.to_string()),
            ("msaa".to_string(), r.msaa.to_string()),
            ("taa".to_string(), on(r.taa)),
            ("fsr3".to_string(), on(r.fsr3)),
            ("gsr2".to_string(), on(r.gsr2)),
            ("sharpen".to_string(), on(r.sharpen)),
            ("anisotropy".to_string(), r.anisotropy.to_string()),
        ]).into_iter().flatten());
        // The display rows go to the keys the window reads at its next start, once either was moved off its default.
        let (resolution, mode) = (self.values["resolution"], self.values["window_mode"]);
        if (resolution, mode) != (RESOLUTION_DEFAULT, 0) {
            out.push(("window".into(), RESOLUTIONS[resolution as usize % RESOLUTIONS.len()].into()));
            out.push(("fullscreen".into(), if mode == 0 { "off" } else { "on" }.into()));
        }
        // The pacing rows go as Barrel's own keys, always: the game has them at 60 frames and
        // vsync on from its first start.
        out.extend(self.pacing().to_config().lines().filter_map(|l| l.split_once(' ')).map(|(k, v)| (k.to_string(), v.to_string())));
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
                // The display rows back at their defaults: a window or fullscreen key an earlier save
                // left goes back to Barrel's default window (the lines name them only off it).
                None if matches!(line.trim().split_once(' '), Some(("window" | "fullscreen", _))) => {
                    let default = barrel_render::settings::Display::DEFAULT.to_config();
                    let key = line.trim().split_once(' ').map_or("", |(k, _)| k);
                    out.extend(default.lines().filter(|l| l.split_once(' ').is_some_and(|(k, _)| k == key)).map(str::to_string));
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

    /// Takes the game options of `config` (`option_` keys; the graphics keys are `graphics::Settings`'s)
    /// and Barrel's render settings (its scale, upscaler, anti-aliasing, sharpening and filtering keys).
    pub fn load(&mut self, config: &str) {
        let r = RenderSettings::from_config_over(self.render, config);
        self.render = RenderSettings { scale: r.scale, upscaler: r.upscaler, msaa: r.msaa, taa: r.taa, fsr3: r.fsr3, gsr2: r.gsr2, sharpen: r.sharpen, anisotropy: r.anisotropy, ..self.render };
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
        let mut s = State::new(graphics::Settings::default(), RenderSettings::DESKTOP);
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
        let mut back = State::new(graphics::Settings::default(), RenderSettings::DESKTOP);
        back.load(&text);
        back.graphics.apply_config(&text);
        assert_eq!(back.option("difficulty"), Some(3));
        assert_eq!(back.option("cursor_speed"), Some(6));
        assert_eq!(back.graphics.quality, Quality::Custom);
        assert_eq!(back.graphics.level(Item::Occlusion), s.graphics.level(Item::Occlusion));
    }

    #[test]
    fn the_display_rows_go_to_the_windows_keys_once_moved() {
        let mut s = State::new(graphics::Settings::default(), RenderSettings::DESKTOP);
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
        let mut s = State::new(graphics::Settings::default(), RenderSettings::DESKTOP);
        s.tab = 3;
        for (on, first) in [(11, 0), (12, 1), (21, 10), (15, 10), (9, 9), (0, 0)] {
            s.cursor[3] = on;
            s.follow();
            assert_eq!(s.first[3], first, "cursor {on}");
        }
    }

    /// A screen on the Graphics tab with the cursor on the row of this engine kind (or the game's item).
    fn on_row(s: &mut State, f: impl Fn(&Entry) -> bool) {
        s.tab = 3;
        s.cursor[3] = s.entries(3).iter().position(f).expect("the row");
    }

    #[test]
    fn the_display_rows_go_to_the_config_and_back() {
        let mut s = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        // The game's start: 60 frames a second and vsync on, the window at its default.
        assert_eq!(s.pacing(), Pacing { vsync: true, limit: Some(60) });
        let text = s.store("");
        assert!(text.contains("vsync on\n") && text.contains("frame_limit 60\n") && !text.contains("window "));
        // Back at the defaults, the window a saved choice left is Barrel's default again.
        let back = s.store("window 1920x1080\nfullscreen on\n");
        assert_eq!(barrel_render::settings::Display::from_config(&back), barrel_render::settings::Display::DEFAULT);
        // Vsync off, the limit at 30 (one step down), Unlimited (three up from there), a 2560x1440 borderless window.
        s.values.insert("vsync", 1);
        s.values.insert("frame_rate", 0);
        assert_eq!(s.pacing(), Pacing { vsync: false, limit: Some(30) });
        s.values.insert("frame_rate", 4);
        assert_eq!(s.pacing().limit, None);
        s.values.insert("resolution", 6);
        s.values.insert("window_mode", 1);
        let text = s.store("window 1x1\n");
        assert_eq!(barrel_render::settings::Pacing::from_config(&text), Pacing { vsync: false, limit: None });
        assert_eq!(barrel_render::settings::Display::from_config(&text), barrel_render::settings::Display { width: 2560, height: 1440, fullscreen: true });
        let mut back = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        back.load(&text);
        assert_eq!(back.pacing(), s.pacing());
    }

    #[test]
    fn the_graphics_tab_weaves_barrels_rows_in_unless_it_is_the_games() {
        let mut s = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        let rows = s.entries(3);
        let at = |which| rows.iter().position(|e| e.engine == Some(which)).unwrap();
        // Screen Resolution, Window Settings, then the three rows, then Frame Rate Limit.
        assert_eq!((at(Engine::Scale), at(Engine::Upscaler), at(Engine::Sharpen)), (2, 3, 4));
        assert_eq!(rows.len(), 25);
        assert!(rows[at(Engine::AntiAliasing)].graphics == Some(Some(Item::AntiAliasing)) && rows[at(Engine::Filtering)].graphics == Some(Some(Item::Filtering)));
        s.extended = false;
        let game = s.entries(3);
        assert_eq!(game.len(), 22);
        assert!(game.iter().all(|e| e.engine.is_none()));
        // The game's own rows with the game's own choices (Off, FXAA, TAA; Linear, Anisotropic).
        let words = |item| game.iter().find(|e| e.graphics == Some(Some(item))).map(|e| match &e.control { Control::Words(w) => w.len(), _ => 0 }).unwrap();
        assert_eq!((words(Item::AntiAliasing), words(Item::Filtering)), (3, 2));
    }

    #[test]
    fn stepping_skips_the_closed_choices() {
        // Scale 75%, bilinear: native AA needs 100%, so from MSAA 8x on a step goes round to Off.
        let mut render = RenderSettings::DESKTOP.resolved();
        g::set_scale(&mut render, 0.75);
        g::set_aa(&mut render, Aa::Msaa(8));
        let mut s = State::new(graphics::Settings::default(), render);
        on_row(&mut s, |e| e.engine == Some(Engine::AntiAliasing));
        assert_eq!(s.engine_value(Engine::AntiAliasing), 5);
        assert_eq!(s.closed(Engine::AntiAliasing), vec![(6, "Native AA needs a render scale of 100% or more.")]);
        s.change(1);
        assert_eq!((s.engine_value(Engine::AntiAliasing), s.render.msaa, s.graphics.level(Item::AntiAliasing)), (0, 1, 0));
        // ... and back the other way from Off lands on MSAA 8x, past the closed one.
        s.change(-1);
        assert_eq!((s.engine_value(Engine::AntiAliasing), s.render.msaa), (5, 8));
        // A row with nothing else open stays: under GSR 2 every anti-aliasing is closed and shows "By the upscaler".
        g::set_upscaler(&mut s.render, Up::Gsr2);
        assert_eq!(s.engine_value(Engine::AntiAliasing), 7);
        let before = s.clone();
        s.change(1);
        assert_eq!(s, before);
        // The upscalers need a scale under 100%: at 100% only Off is open and Right does nothing.
        let mut full = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        on_row(&mut full, |e| e.engine == Some(Engine::Upscaler));
        let before = full.clone();
        full.change(1);
        assert_eq!(full, before);
        assert!(!full.changed);
    }

    #[test]
    fn the_descriptions_name_the_closed_choices_in_red() {
        let strings: HashMap<u32, String> = HashMap::from([(721_318 + 65_536, "Keeps textures sharp.".to_string())]);
        let mut render = RenderSettings::DESKTOP.resolved();
        g::set_scale(&mut render, 0.75);
        let mut s = State::new(graphics::Settings::default(), render);
        on_row(&mut s, |e| e.engine == Some(Engine::AntiAliasing));
        let text = s.screen(&strings).text;
        assert!(text.contains("<CLRE>FSR Native AA: Native AA needs a render scale of 100% or more.<CLNR>"), "{text}");
        // Barrel's own row: its own words, and a whole row closed for one reason names that reason alone.
        let mut full = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        on_row(&mut full, |e| e.engine == Some(Engine::Upscaler));
        let screen = full.screen(&strings);
        assert_eq!(screen.title, "Upscaler");
        assert!(screen.text.starts_with("How a scene drawn below full size is scaled up.^00<CLRE>FSR 1, GSR 1, GSR 2, FSR 3.1: Upscalers need a render scale under 100%.<CLNR>"), "{}", screen.text);
        // The game's filtering row is the game's words with nothing closed.
        on_row(&mut full, |e| e.engine == Some(Engine::Filtering));
        assert_eq!(full.screen(&strings).text, "Keeps textures sharp.");
    }

    #[test]
    fn anti_aliasing_and_filtering_go_to_the_config_and_back() {
        let mut s = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        on_row(&mut s, |e| e.engine == Some(Engine::AntiAliasing));
        // The game's own FXAA (the default), then TAA, MSAA 2x and 4x.
        assert_eq!(s.engine_value(Engine::AntiAliasing), 1);
        for _ in 0..3 {
            s.change(1);
        }
        assert_eq!((s.engine_value(Engine::AntiAliasing), s.render.msaa), (4, 4));
        assert_eq!((s.graphics.level(Item::AntiAliasing), s.graphics.quality), (0, Quality::Custom));
        let text = s.store("");
        let mut back = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        back.load(&text);
        back.graphics.apply_config(&text);
        assert_eq!((back.engine_value(Engine::AntiAliasing), back.render.msaa), (4, 4));
        // Choosing a game level clears Barrel's choice: TAA is the game's item, the render keys have none.
        s.change(-1);
        assert_eq!((s.engine_value(Engine::AntiAliasing), s.render.msaa), (3, 2));
        s.change(-1);
        assert_eq!((s.engine_value(Engine::AntiAliasing), s.render.msaa, s.graphics.level(Item::AntiAliasing)), (2, 1, 2));
        s.change(-1);
        assert_eq!((s.graphics.level(Item::AntiAliasing), s.render.msaa), (1, 1));
        // Native AA at full size is FSR 3 without the upscaler row's choice.
        for _ in 0..5 {
            s.change(1);
        }
        assert_eq!((s.engine_value(Engine::AntiAliasing), s.render.fsr3, s.graphics.level(Item::AntiAliasing)), (6, true, 0));
        assert!(s.store("").contains("fsr3 on"));
        // A quality preset takes the game's anti-aliasing and Barrel's goes (Custom after the items, then round to Low).
        on_row(&mut s, |e| e.graphics == Some(None));
        s.change(1);
        assert_eq!(s.graphics.quality, Quality::Low);
        assert!(!s.render.fsr3 && s.render.msaa == 1);
        // Texture filtering: Linear, then 2x to 16x with the game's item Anisotropic from 2x.
        on_row(&mut s, |e| e.engine == Some(Engine::Filtering));
        assert_eq!(s.engine_value(Engine::Filtering), 4);
        s.change(1);
        assert_eq!((s.engine_value(Engine::Filtering), s.graphics.level(Item::Filtering), s.render.anisotropy), (0, 0, 1));
        s.change(1);
        s.change(1);
        assert_eq!((s.engine_value(Engine::Filtering), s.graphics.level(Item::Filtering), s.render.anisotropy), (2, 1, 4));
        let text = s.store("");
        assert!(text.contains("anisotropy 4"));
        let mut back = State::new(graphics::Settings::default(), RenderSettings::DESKTOP.resolved());
        back.load(&text);
        back.graphics.apply_config(&text);
        assert_eq!((back.engine_value(Engine::Filtering), back.render.anisotropy), (2, 4));
    }

    #[test]
    fn a_description_reads_its_key_tags() {
        assert_eq!(keys("Use <KEY_ASSIGN_FIELD_DASH> to change."), "Use \u{4e7} to change.");
        assert_eq!(keys("a<CLGR>b<CLNR>"), "a<CLGR>b<CLNR>");
    }
}
