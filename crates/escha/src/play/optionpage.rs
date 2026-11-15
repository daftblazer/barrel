//! The main menu's Option command as the game draws it (the `menu-cmd-8` capture): the Options
//! screen (layout 511) over the menu's overview, which the game has already drawn that frame. The
//! page is layout 408 at (584, 150): the panel (part 0x32, stretched, 930 by 872 from (512, 150)),
//! nine rules (part 0x28, kind 19, 784 wide, in the ink of the node's word 7), the chosen entry's
//! plate (part 0x3c for a volume row or a choice, 0x3d for "Default" and "Confirm"; the others
//! show no plate), the texts, seven volume sliders (layout 407 nested: a track, a frame and the
//! knob) and last the screen's title bar (part 0x30 with its heading and icon), the footer line
//! (372), the header line (371) and the cursor's glow (90 up and left of the plate).
//!
//! The seven volumes (BGM, SE, Env. SE, Sys. SE, Voice, B. Voice, Movie) are sliders, "Fast
//! Forward Event" (Read / All) and "Voice Language" (English / Japanese) two choices each, the
//! chosen one in dark ink and the other grey (the layout holds both inks as two texts).
//!
//! The values are Barrel's own settings (`options.conf` beside the graphics config, see
//! [`OptionPage::load`]): BGM, SE and Voice reach the mixer's volumes of music, effects and
//! voices ([`apply`]); Env. SE, Sys. SE, B. Voice, Movie, "Fast Forward Event" and "Voice
//! Language" are kept and drawn but nothing in Barrel reads them yet (no per-kind split of the
//! ambience, system and battle sounds, no movie volume, no read-event marks, and only the
//! English voices are shipped here).
//!
//! The page is the first of the Options screen's two (`options.rs` has the second, Barrel's
//! graphics): Down off "Confirm", or Up off the first row, turns to the other page.
//!
//! Not yet the game's: the knob's place below 100 (the
//! capture has every volume at its maximum: the knob at 399, the layout's 400 less one), the
//! plate of a two-way row (drawn under the chosen choice, a guess: the capture's cursor is on
//! the first row) and the outfit choice the same layout 511 holds (hidden here).

use barrel_escha::data::layout::{self, Placed};
use barrel_formats::pe::Pe;

use crate::play::hud::Element;
use crate::play::kit::{raised, tiled};
use crate::play::title::Button;

/// `SysInfo.xml`'s "Select an item to adjust." (the header line).
pub const HEADER_ROW: usize = 111;
/// `Menu_Line_Help.xml`'s "Select, Change, Return".
pub const FOOTER_ROW: usize = 141;
const SCREEN: u32 = 511;
const PAGE: u32 = 408;
const SLIDER: u32 = 407;
/// The hero's outfit choice, nested in the screen and not shown with the page.
const OUTFIT: u32 = 512;
const HEADER: u32 = 371;
const FOOTER: u32 = 372;
const DESK_CUT: i32 = 1;
const PANEL: i32 = 0x32;
const RULE: i32 = 0x28;
const PLATE: i32 = 0x3c;
const BUTTON: i32 = 0x3d;
const TEXT: i32 = 0x164;
const SLIDER_PART: i32 = 0x11;
const STRETCHED: i32 = 15;
const RULE_KIND: i32 = 19;
const KNOB_CUT: i32 = 0;
/// The glow's place from its plate (the capture: (518, 130) for the plate at (608, 220)).
const GLOW_FROM_PLATE: [i32; 2] = [-90, -90];
/// A slider's knob: its place in the slider's layout at the maximum (the layout's 400 less one,
/// as the capture) and at the minimum (the track's start, not seen).
const KNOB_MAX: i32 = 399;
const KNOB_MIN: i32 = 22;
pub const VOLUMES: usize = 7;
/// The entries the cursor goes over: the volumes, the two choices, "Default" and "Confirm".
pub const ENTRIES: usize = VOLUMES + 4;
const FAST_FORWARD: usize = VOLUMES;
const LANGUAGE: usize = VOLUMES + 1;
const DEFAULT: usize = VOLUMES + 2;
const CONFIRM: usize = VOLUMES + 3;
/// A volume's step with Left and Right, in percent.
const STEP: i32 = 10;
/// The volumes' names in the config, in the page's order.
const VOLUME_KEYS: [&str; VOLUMES] = ["bgm", "se", "env_se", "sys_se", "voice", "battle_voice", "movie"];
/// Which of the page's volumes the mixer's kinds take: music, effects, voices.
const MUSIC: usize = 0;
const EFFECT: usize = 1;
const VOICE: usize = 4;

/// Where a button takes the player off the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    /// The Options screen closes, keeping the settings ("Confirm") or not (Back).
    Close(bool),
    /// The other page: its first entry, or its last (`last`, from going up).
    Graphics { last: bool },
}

/// The page's state: the entry the cursor is on and the values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionPage {
    pub chosen: usize,
    pub volumes: [i32; VOLUMES],
    /// "Fast Forward Event": All (else Read).
    pub fast_forward_all: bool,
    /// "Voice Language": English (else Japanese).
    pub english: bool,
}

impl Default for OptionPage {
    fn default() -> OptionPage {
        OptionPage { chosen: 0, volumes: [100; VOLUMES], fast_forward_all: true, english: true }
    }
}

impl OptionPage {
    /// A button; `Some` leaves the page.
    pub fn press(&mut self, b: Button) -> Option<Leave> {
        match b {
            Button::Up if self.chosen == 0 => return Some(Leave::Graphics { last: true }),
            Button::Down if self.chosen == CONFIRM => return Some(Leave::Graphics { last: false }),
            Button::Up => self.chosen -= 1,
            Button::Down => self.chosen += 1,
            Button::Left | Button::Right => {
                let sign = if b == Button::Right { 1 } else { -1 };
                match self.chosen {
                    k if k < VOLUMES => self.volumes[k] = (self.volumes[k] + sign * STEP).clamp(0, 100),
                    FAST_FORWARD => self.fast_forward_all = !self.fast_forward_all,
                    LANGUAGE => self.english = !self.english,
                    _ => {}
                }
            }
            Button::Confirm if self.chosen == DEFAULT => *self = OptionPage { chosen: self.chosen, ..OptionPage::default() },
            Button::Confirm if self.chosen == CONFIRM => return Some(Leave::Close(true)),
            Button::Confirm => {}
            Button::Back => return Some(Leave::Close(false)),
        }
        None
    }

    /// The page on its last entry or its first (coming back from the other page).
    pub fn at(self, last: bool) -> OptionPage {
        OptionPage { chosen: if last { CONFIRM } else { 0 }, ..self }
    }

    /// The settings as the config file holds them (the cursor is not one).
    pub fn to_config(&self) -> String {
        let mut out: String = VOLUME_KEYS.iter().zip(self.volumes).map(|(k, v)| format!("{k} {v}\n")).collect();
        out += &format!("fast_forward {}\nvoice_language {}\n", if self.fast_forward_all { "all" } else { "read" }, if self.english { "english" } else { "japanese" });
        out
    }

    pub fn from_config(text: &str) -> OptionPage {
        let mut o = OptionPage::default();
        for line in text.lines() {
            let Some((key, value)) = line.trim().split_once(' ') else { continue };
            let value = value.trim();
            match key {
                "fast_forward" => o.fast_forward_all = value != "read",
                "voice_language" => o.english = value != "japanese",
                k => {
                    if let Some(i) = VOLUME_KEYS.iter().position(|n| *n == k) {
                        o.volumes[i] = value.parse::<i32>().map_or(o.volumes[i], |v| v.clamp(0, 100));
                    }
                }
            }
        }
        o
    }

    /// Where the settings are kept: `options.conf` beside the graphics config.
    fn path() -> Option<std::path::PathBuf> {
        Some(crate::play::settings::RenderSettings::config_path()?.with_file_name("options.conf"))
    }

    /// The settings kept, or the defaults.
    pub fn load() -> OptionPage {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(OptionPage::default, |t| Self::from_config(&t))
    }

    pub fn save(&self) -> crate::play::Result<()> {
        let path = Self::path().ok_or("no place for the config (no HOME)")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_config())?;
        Ok(())
    }

    /// Sets the mixer's volumes of music, effects and voices to the page's BGM, SE and Voice.
    pub fn apply(&self, mixer: &mut barrel_audio::Mixer) {
        let level = |i: usize| self.volumes[i] as f32 / 100.0;
        mixer.kinds = [level(MUSIC), level(EFFECT), level(VOICE)];
    }
}

/// A mixer at the game's level with the saved volumes.
pub fn mixer() -> barrel_audio::Mixer {
    let mut m = barrel_audio::Mixer::game();
    OptionPage::load().apply(&mut m);
    m
}

/// The layouts the page is made of, placed once.
pub struct Layouts {
    screen: Vec<Placed>,
    header: Vec<Placed>,
    footer: Vec<Placed>,
    cursor: Option<layout::Flip>,
}

impl Layouts {
    pub fn load(pe: &Pe) -> Layouts {
        let get = |id| layout::placed(pe, id);
        Layouts { screen: get(SCREEN), header: get(HEADER), footer: get(FOOTER), cursor: crate::play::kit::cursor_glow(pe) }
    }

    /// The page's own nodes (layout 408 nested in the screen, without the outfit choice).
    pub(crate) fn page(&self) -> Vec<&Placed> {
        self.screen.iter().filter(|p| !p.path.contains(&OUTFIT) && p.path.contains(&PAGE)).collect()
    }

    /// The desk under the screen.
    pub(crate) fn desk(&self) -> Vec<Element> {
        let nodes = self.screen.iter().filter(|p| !p.path.contains(&OUTFIT));
        nodes.filter(|p| p.path.len() == 1 && p.node.part == 0).map(|p| Element::Sprite { part: 0, cut: DESK_CUT, at: p.at }).collect()
    }

    /// The paper panel and the first `rules` of the page's rules.
    pub(crate) fn panel(&self, pe: &Pe, rules: usize) -> Vec<Element> {
        let mut out = Vec::new();
        let mut left = rules;
        for p in self.page() {
            let n = p.node;
            if n.part == PANEL && n.kind == STRETCHED {
                out.extend(tiled(pe, PANEL, p.at, [layout::word(pe, p, 5).unwrap_or(0), layout::word(pe, p, 6).unwrap_or(0)]));
            } else if n.part == RULE && n.kind == RULE_KIND && left > 0 {
                left -= 1;
                out.extend(rule(pe, p));
            }
        }
        out
    }

    /// The screen's title bar (its heading and icon).
    pub(crate) fn title(&self) -> Vec<Element> {
        let nodes = self.screen.iter().filter(|p| !p.path.contains(&OUTFIT));
        nodes.filter(|p| p.path.len() == 1 && matches!(p.node.part, 0x30 | 0x2e | 0x2c)).map(Element::stored).collect()
    }

    /// The footer line and the header line, with their words.
    pub(crate) fn lines(&self, pe: &Pe, words: &Words) -> Vec<Element> {
        let line = |nodes: &[Placed], text: &str| -> Vec<Element> {
            nodes.iter().flat_map(|p| match layout::text(pe, p) {
                Some(t) => vec![raised(t, text)],
                None => vec![Element::stored(p)],
            }).collect()
        };
        let mut out = line(&self.footer, &words.footer);
        out.extend(line(&self.header, &words.header));
        out
    }

    /// The cursor's glow at the chosen plate.
    pub(crate) fn glow(&self, plate: [i32; 2], tick: u32) -> Element {
        let at = [plate[0] + GLOW_FROM_PLATE[0], plate[1] + GLOW_FROM_PLATE[1]];
        Element::Sprite { part: crate::play::kit::CURSOR, cut: self.cursor.map_or(0, |f| f.cut(tick)), at }
    }

    /// Every cut the page's flipbooks turn through.
    pub fn flips(&self) -> Vec<Element> {
        self.cursor.into_iter().flat_map(|f| crate::play::kit::cuts_of(crate::play::kit::CURSOR, f)).collect()
    }
}

/// The page's two lines of words.
pub struct Words {
    pub header: String,
    pub footer: String,
}

/// A rule: its left end, the middle repeated (the last repeat cut short) and its right end, in
/// the node's ink. The cut-short piece is an inked slice.
fn rule(pe: &Pe, p: &Placed) -> Vec<Element> {
    let width = layout::word(pe, p, 5).unwrap_or(0);
    let rgb = crate::play::kit::ink(pe, p);
    let wide = |cut| layout::sprite(pe, p.node.part, cut, [0, 0]).map_or(0, |s| i32::from(s.src[2] - s.src[0]));
    let (left, middle, right) = (wide(0), wide(1), wide(2));
    let sprite = |cut, x| Element::Tinted { part: p.node.part, cut, at: [p.at[0] + x, p.at[1]], rgb };
    let mut out = vec![sprite(0, 0)];
    let mut x = left;
    while x < width - right {
        if x + middle <= width - right {
            out.push(sprite(1, x));
        } else {
            let cut_short = crate::play::kit::horizontal_bar(pe, p.node.part, p.at, width).into_iter().find_map(|e| match e {
                Element::Slice { pack, index, src, rect } if rect[0] == p.at[0] + x => Some(Element::InkedSlice { pack, index, src, rect, rgb }),
                _ => None,
            });
            out.extend(cut_short);
        }
        x += middle;
    }
    out.push(sprite(2, width - right));
    out
}

/// A slider's knob place for `volume` percent.
fn knob_x(volume: i32) -> i32 {
    KNOB_MIN + (KNOB_MAX - KNOB_MIN) * volume.clamp(0, 100) / 100
}

/// All the page's elements; `tick` the clock's 60 Hz ticks (the glow).
pub fn elements(pe: &Pe, l: &Layouts, words: &Words, view: &OptionPage, tick: u32) -> Vec<Element> {
    let mut out = l.desk();
        let page = || l.page().into_iter();
    // The chosen entry's plate: the row's bar, the choice's box or the button.
    let plates: Vec<&Placed> = page().filter(|p| matches!(p.node.part, PLATE | BUTTON)).collect();
    let plate = match view.chosen {
        k if k < VOLUMES => plates.get(k),
        FAST_FORWARD => plates.get(VOLUMES + usize::from(view.fast_forward_all)),
        LANGUAGE => plates.get(VOLUMES + 2 + usize::from(!view.english)),
        k => plates.get(VOLUMES + 4 + k - DEFAULT),
    };
    out.extend(l.panel(pe, usize::MAX));
    if let Some(p) = plate {
        out.push(Element::Sprite { part: p.node.part, cut: p.node.cut, at: p.at });
    }
    // The texts: a choice's two inks, the chosen one's dark.
    let chosen_words = [if view.fast_forward_all { "All" } else { "Read" }, if view.english { "English" } else { "Japanese" }];
    for p in page().filter(|p| p.node.part == TEXT) {
        let Some(t) = layout::text(pe, p) else { continue };
        let grey = t.colour[1] == 0x81;
        let choice = matches!(t.text.as_str(), "Read" | "All" | "English" | "Japanese");
        if !choice || grey != chosen_words.contains(&t.text.as_str()) {
            out.push(raised(t.clone(), &t.text));
        }
    }
    // The sliders: track, frame and knob, three nodes each.
    for (k, group) in page().filter(|p| p.path.contains(&SLIDER)).collect::<Vec<_>>().chunks(3).enumerate() {
        for p in group {
            let at = if p.node.part == SLIDER_PART && p.node.cut == KNOB_CUT {
                // The knob's place in the layout is the slider's maximum.
                [p.at[0] - (400 - knob_x(view.volumes.get(k).copied().unwrap_or(100))), p.at[1]]
            } else {
                p.at
            };
            out.push(Element::Sprite { part: p.node.part, cut: p.node.cut, at });
        }
    }
    out.extend(l.title());
    out.extend(l.lines(pe, words));
    out.extend(plate.map(|p| l.glow(p.at, tick)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_the_entries_and_changes_values() {
        let mut o = OptionPage::default();
        assert_eq!(o.press(Button::Up), Some(Leave::Graphics { last: true }));
        o.chosen = CONFIRM;
        assert_eq!(o.press(Button::Down), Some(Leave::Graphics { last: false }));
        o.chosen = 0;
        o.press(Button::Left);
        assert_eq!(o.volumes[0], 100 - STEP);
        o.chosen = LANGUAGE;
        o.press(Button::Right);
        assert!(!o.english);
        o.chosen = DEFAULT;
        o.press(Button::Confirm);
        assert_eq!(o, OptionPage { chosen: DEFAULT, ..OptionPage::default() });
        o.chosen = CONFIRM;
        assert_eq!(o.press(Button::Confirm), Some(Leave::Close(true)));
        assert_eq!(o.press(Button::Back), Some(Leave::Close(false)));
    }

    #[test]
    fn the_config_round_trips() {
        let o = OptionPage { chosen: 0, volumes: [10, 20, 30, 40, 50, 60, 70], fast_forward_all: false, english: false };
        assert_eq!(OptionPage::from_config(&o.to_config()), o);
        assert_eq!(OptionPage::from_config("bgm 300\nnonsense\n").volumes[0], 100);
    }

    #[test]
    fn the_knob_runs_with_the_volume() {
        assert_eq!(knob_x(100), KNOB_MAX);
        assert_eq!(knob_x(0), KNOB_MIN);
    }
}
