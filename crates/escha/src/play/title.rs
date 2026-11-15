//! The title screen, from the game's own layout 1056 (`docs/formats.md`, "Interface layouts";
//! `WinXls_EN/a15_title64.g1t`): the background picture (still 9, `Ipu/title_bg01`), the logo
//! (part 0x15c cut 0), "Press any button" (0x15c cut 1) until a button is pressed, then the six
//! entries (parts 0x15d to 0x162, cut 0 normal and cut 1 lit) with the cursor (layout 1057: a
//! bar, 0x15c cut 2, and two ornaments, part 0x163) on the chosen one.
//!
//! Read from the layout: the pictures and their places (the entries 78 pixels apart from y 538).
//! Not yet the game's: which of the two title layouts the game uses (958, on
//! `a15_title.g1t`, has the same pieces with the logo 118 pixels further left and the entries
//! 78 lower, the last past the screen's foot: 1056 is taken); the cursor's place on an entry
//! (its bar centred on it); the menu wrapping round; no fades or animation; the later title
//! backgrounds (stills 10 and 11, chosen by progress) not used.

use std::path::Path;

use ash::vk;

use crate::hud::{self, Element};
use crate::{text, Gpu, Result};

/// Still picture of the title's background.
pub const BACKGROUND: u32 = 9;
const LOGO: i32 = 0x15c;
const LOGO_AT: [i32; 2] = [310, 30];
/// The logo's size: its node (layout 1056's first) is of kind 4, scaled, with word 5 = 80 (the
/// escha-boot recording at 17 s: the logo 0.8 of the picture's size, its corner at the node).
const LOGO_PERCENT: i32 = 80;
const CURSOR: i32 = 0x163;
/// The entries' parts, top to bottom, and their place.
const ENTRY_PARTS: [i32; 6] = [0x15d, 0x15e, 0x162, 0x15f, 0x160, 0x161];
const ENTRY_X: i32 = 630;
const ENTRY_Y: i32 = 538;
/// The cursor bar (90 high) centred on an entry (76 high).
const CURSOR_RISE: i32 = 7;
const ENTRY_STEP: i32 = 78;

/// The entries, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    NewGame,
    LoadGame,
    DressingRoom,
    Extras,
    Options,
    ExitGame,
}

pub const ENTRIES: [Entry; 6] = [Entry::NewGame, Entry::LoadGame, Entry::DressingRoom, Entry::Extras, Entry::Options, Entry::ExitGame];

/// A button on the title screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    PressAny,
    Menu(usize),
}

impl Title {
    pub fn new() -> Title {
        Title::PressAny
    }

    /// Takes a button: the menu opens on the first entry; up and down move round it; confirm
    /// picks the entry; back returns to "Press any button".
    pub fn press(&mut self, b: Button) -> Option<Entry> {
        match (*self, b) {
            (Title::PressAny, _) => *self = Title::Menu(0),
            (Title::Menu(k), Button::Up) => *self = Title::Menu((k + ENTRIES.len() - 1) % ENTRIES.len()),
            (Title::Menu(k), Button::Down) => *self = Title::Menu((k + 1) % ENTRIES.len()),
            (Title::Menu(k), Button::Confirm) => return Some(ENTRIES[k]),
            (Title::Menu(_), Button::Back) => *self = Title::PressAny,
            (Title::Menu(_), Button::Left | Button::Right) => {}
        }
        None
    }

    /// The sprites to draw over the background, on the 1920 × 1080 screen.
    pub fn elements(&self) -> Vec<Element> {
        let mut out = vec![Element::Scaled { part: LOGO, cut: 0, at: LOGO_AT, percent: LOGO_PERCENT, about: LOGO_AT }];
        match *self {
            Title::PressAny => out.push(Element::Sprite { part: LOGO, cut: PROMPT_CUT, at: [570, 660] }),
            Title::Menu(chosen) => {
                let y = ENTRY_Y + ENTRY_STEP * chosen as i32 - CURSOR_RISE;
                out.push(Element::Sprite { part: LOGO, cut: 2, at: [584, y] });
                out.push(Element::Sprite { part: CURSOR, cut: 0, at: [524, y - 16] });
                out.push(Element::Sprite { part: CURSOR, cut: 4, at: [1226, y - 16] });
                for (k, &part) in ENTRY_PARTS.iter().enumerate() {
                    out.push(Element::Sprite { part, cut: i32::from(k == chosen), at: [ENTRY_X, ENTRY_Y + ENTRY_STEP * k as i32] });
                }
            }
        }
        out
    }
}

impl Default for Title {
    fn default() -> Title {
        Title::new()
    }
}

/// Every sprite any state of the title shows (to load their pictures once).
fn all_elements() -> Vec<Element> {
    let mut out = Title::PressAny.elements();
    for k in 0..ENTRIES.len() {
        out.extend(Title::Menu(k).elements());
    }
    out
}

/// The screens before play: the title, then the hero choice or the Load Game list.
#[derive(Clone, Debug, PartialEq)]
enum Front {
    Title(Title),
    Hero(crate::hero::Hero),
    Load(crate::loadlist::LoadList),
}

/// How the screens before play ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// A new game with this lead (Logy when true).
    NewGame { logy: bool },
    /// A saved game from this file.
    Load(std::path::PathBuf),
    /// The Options screen (the title shows again after it).
    Options,
    Exit,
}

impl Front {
    fn press(&mut self, b: Button, saves: Option<&Path>) -> Option<Start> {
        match self {
            Front::Title(t) => match t.press(b)? {
                Entry::NewGame => *self = Front::Hero(crate::hero::Hero::new()),
                Entry::LoadGame => *self = Front::Load(crate::loadlist::LoadList::new(saves.map(barrel_escha::saves::list).unwrap_or_default())),
                Entry::ExitGame => return Some(Start::Exit),
                Entry::Options => return Some(Start::Options),
                other => eprintln!("title: {other:?} is not there yet"),
            },
            Front::Load(l) => match l.press(b)? {
                crate::loadlist::Outcome::Load(path) => return Some(Start::Load(path)),
                crate::loadlist::Outcome::Back => *self = Front::Title(Title::Menu(1)),
            },
            Front::Hero(h) => match h.press(b)? {
                crate::hero::Outcome::Chosen(logy) => return Some(Start::NewGame { logy }),
                crate::hero::Outcome::Back => *self = Front::Title(Title::Menu(0)),
            },
        }
        None
    }
}

/// Buttons from a comma-separated list (up, down, left, right, confirm, back), for offscreen
/// runs.
pub fn keys(list: &str) -> Vec<Button> {
    list.split(',')
        .filter_map(|k| match k.trim() {
            "up" => Some(Button::Up),
            "down" => Some(Button::Down),
            "left" => Some(Button::Left),
            "right" => Some(Button::Right),
            "confirm" => Some(Button::Confirm),
            "back" => Some(Button::Back),
            _ => None,
        })
        .collect()
}

/// A key's title button.
pub fn button(key: winit::keyboard::KeyCode) -> Option<Button> {
    use winit::keyboard::KeyCode as K;
    match key {
        K::ArrowUp | K::KeyW => Some(Button::Up),
        K::ArrowDown | K::KeyS => Some(Button::Down),
        K::ArrowLeft | K::KeyA => Some(Button::Left),
        K::ArrowRight | K::KeyD => Some(Button::Right),
        K::Enter | K::Space | K::KeyZ => Some(Button::Confirm),
        K::Escape | K::Backspace | K::KeyX => Some(Button::Back),
        _ => None,
    }
}

pub use crate::screen::Target;

/// Shows the title screen and the hero choice, and returns how they ended (`None`: the window
/// was closed, or, offscreen, `keys` did not end them). In a window it runs until a new game is
/// chosen or Exit Game is picked; offscreen it takes `keys`, draws the screen once and writes it
/// to `out`.
/// The title screen's music: track 1, `sy_title01_piano.kvs` ("タイトル画面 ピアノ 8ch"), the
/// piece the recordings play at the title (0.92 correlation against 0.07 to 0.13 for the
/// orchestral `sy_title01_oke` and the loop `sy_op_loop`). Track 168, the same piece as
/// `sy_title01_piano_2ch.kvs`, matches as well (0.91): which of the two the PC plays is open.
const TITLE_MUSIC: u32 = 1;

/// The title's fade in: a white veil over it, from opaque to clear in a straight line over
/// [`WHITE_FADE`] seconds (the escha-boot recording at 16.25 s, after the logos and the pre-title
/// movie: the veil's opacity, fitted frame by frame, falls by 0.055 a 60 Hz frame).
const WHITE_FADE: f32 = 0.3;
static WHITE: [u8; 64] = [255; 64];

/// "Press any button" pulses: its opacity falls in a straight line from 1 to [`PROMPT_LOW`]
/// over half of [`PROMPT_PERIOD`] seconds and rises back the same way, from 1 as the title
/// starts (the escha-boot recording from 16.25 s: the prompt's opacity fitted frame by frame
/// against ours drawn at full and half strength, a triangle wave within 0.003).
const PROMPT_PERIOD: f32 = 2.4;
const PROMPT_LOW: f32 = 0.2;
const PROMPT_CUT: i32 = 1;

/// The prompt's opacity (0 to 255) at `t` seconds into the title.
fn prompt_at(t: f32) -> u8 {
    let x = (t.max(0.0) / PROMPT_PERIOD).fract();
    ((PROMPT_LOW + (1.0 - PROMPT_LOW) * (1.0 - 2.0 * x).abs()) * 255.0).round() as u8
}

/// The veil's opacity (0 to 255) at `t` seconds.
fn white_at(t: f32) -> u8 {
    ((1.0 - t / WHITE_FADE).clamp(0.0, 1.0) * 255.0).round() as u8
}

pub fn run(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    load_list: bool,
    keys: &[Button],
    out: Option<&Path>,
    saves: Option<&Path>,
) -> Result<Option<Start>> {
    use crate::hero::Hero;
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let still = barrel_escha::data::layout::still(&pe, BACKGROUND).ok_or("title: no background picture")?;
    let x64 = root.join("Data/x64");
    let still_bytes = std::fs::read(x64.join(format!("{}_EN", still.folder)).join(&still.file)).or_else(|_| std::fs::read(x64.join(&still.folder).join(&still.file)))?;
    let still_g1t = barrel_formats::g1t::G1t::parse(&still_bytes)?;
    let hero_nodes = barrel_escha::data::layout::placed(&pe, crate::hero::LAYOUT);
    // The hero choice's confirming line, from the system messages.
    let sys_mess = std::fs::read(root.join("Saves_EN/systemMessage/SysMess.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok()).map(|t| barrel_escha::data::sys_text::sys_mess(&t)).unwrap_or_default();
    let question = sys_mess.get(crate::hero::QUESTION_ROW).map_or_else(|| "Do you want to use this character?".to_string(), |m| m.text.clone());
    let load_nodes = barrel_escha::data::layout::placed(&pe, crate::loadlist::LAYOUT);
    let texts = |p: &barrel_escha::data::layout::Placed| barrel_escha::data::layout::text(&pe, p);
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("title: no glyph table")?;
    let atlas_bytes = std::fs::read(x64.join("Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("title: an empty font atlas")?.width as f32 };
    // The sprites' pictures (every state of both screens), each pack read once.
    let mut every = all_elements();
    for logy in [false, true] {
        every.extend(Hero::Choose { logy }.elements(&hero_nodes, texts, &question));
        for yes in [false, true] {
            every.extend(Hero::Confirm { logy, yes }.elements(&hero_nodes, texts, &question));
        }
    }
    // The list's sprites as stored, and the cuts it swaps in (the stamps, the "Load" heading).
    for p in &load_nodes {
        for cut in [p.node.cut.max(0), 1, 2] {
            every.push(Element::Sprite { part: p.node.part, cut, at: p.at });
        }
    }
    let mut keys_needed: Vec<(String, u32)> = Vec::new();
    for e in every {
        if let Some((part, cut, at)) = e.sprite()
            && let Some(p) = barrel_escha::data::layout::sprite(&pe, part, cut, at) {
                let key = (p.texture, p.index);
                if !keys_needed.contains(&key) {
                    keys_needed.push(key);
                }
            }
    }
    let mut packs: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, _) in &keys_needed {
        if !packs.iter().any(|(n, _)| n == name) {
            let bytes = std::fs::read(x64.join("WinXls_EN").join(name)).or_else(|_| std::fs::read(x64.join("WinXls").join(name)))?;
            packs.push((name.clone(), bytes));
        }
    }
    let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), barrel_formats::g1t::G1t::parse(b)?))).collect::<Result<Vec<_>>>()?;
    // Slots: the font atlas, the title's background, then the sheets.
    let mut images = vec![&atlas.textures[0], still_g1t.textures.first().ok_or("title: an empty background")?];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 2 };
    for (name, index) in keys_needed {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    // A white picture for the title's fade in.
    let white = barrel_formats::g1t::Texture { width: 4, height: 4, format: barrel_formats::g1t::Format::Rgba8, format_code: 0, layers: 1, volume: false, levels: vec![&WHITE] };
    let white_slot = images.len();
    images.push(&white);
    let (w, h) = (target.width as f32, target.height as f32);
    let front_quads = |f: &Front, pulse: u8| match f {
        Front::Title(t) => {
            // The background over the 1920 × 1080 screen, scaled to the height and centred.
            let k = h / 1080.0;
            let dx = (w - 1920.0 * k) / 2.0;
            let mut q = vec![text::Quad { turn: false, spin: None, rect: [dx, 0.0, dx + still.size[0] * k, still.size[1] * k], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], texture: 1 }];
            // The prompt pulsing.
            let e = t.elements().into_iter().map(|e| match e {
                Element::Sprite { part: LOGO, cut: PROMPT_CUT, at } => Element::Faded { part: LOGO, cut: PROMPT_CUT, at, alpha: pulse },
                e => e,
            });
            q.extend(hud::quads(&pe, None, &sheets, &e.collect::<Vec<_>>(), w, h));
            q
        }
        Front::Hero(hero) => hud::quads(&pe, Some(&font), &sheets, &hero.elements(&hero_nodes, texts, &question), w, h),
        Front::Load(list) => hud::quads(&pe, Some(&font), &sheets, &list.elements(&load_nodes, texts), w, h),
    };
    // On the Load Game list (the in-play menu's Load), else the title.
    let quads = |(f, white, pulse): &(Front, u8, u8)| {
        let mut q = front_quads(f, *pulse);
        if *white > 0 {
            q.push(text::Quad { turn: false, spin: None, rect: [0.0, 0.0, w, h], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, f32::from(*white) / 255.0], texture: white_slot });
        }
        q
    };
    let mut front = if load_list { Front::Load(crate::loadlist::LoadList::new(saves.map(barrel_escha::saves::list).unwrap_or_default())) } else { Front::Title(Title::new()) };
    // In a window, the title's music from its start, looping as the track does, under the
    // title, the hero choice and the Load list (not the in-play menu's Load).
    let mut music = (window.is_some() && !load_list)
        .then(|| {
            let out = barrel_audio::Output::open(0.1).map_err(|e| eprintln!("title: no sound: {e}")).ok()?;
            let tracks = barrel_escha::data::bgm::tracks(&pe, barrel_escha::data::bgm::TRACKS_EN).unwrap_or_default();
            let m = barrel_audio::Library::new(root, "Voice_EN", tracks).music(TITLE_MUSIC)?;
            let mut mixer = crate::optionpage::mixer();
            let playing = mixer.play_stream(m.stream, m.volume, m.loop_from);
            eprintln!("title music: track {TITLE_MUSIC}, volume {}, loop from {:?}", m.volume, m.loop_from);
            Some((out, mixer, playing))
        })
        .flatten();
    // The title fades in from white (in a window; offscreen at `BARREL_TITLE_FADE_AT` seconds in).
    let fade_at = std::env::var("BARREL_TITLE_FADE_AT").ok().and_then(|v| v.parse::<f32>().ok());
    let fading = !load_list && (window.is_some() || fade_at.is_some());
    let shift = fade_at.unwrap_or(0.0);
    let mut state = (front, if fading { white_at(shift) } else { 0 }, prompt_at(shift));
    // The veil and the prompt's pulse follow the screen's clock (60 ticks a second).
    crate::screen::run_clocked(gpu, pool, target, window, offscreen, &images, &mut state, quads, |(f, ..), b| f.press(b, saves), keys, out, |(_, white, pulse), tick| {
        let t = shift + tick as f32 / crate::screen::TICKS_PER_SECOND as f32;
        if *white > 0 {
            *white = white_at(t);
        }
        *pulse = prompt_at(t);
        if let Some((out, mixer, _)) = music.as_mut() {
            out.pump(mixer);
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_moves_and_picks() {
        let mut t = Title::new();
        assert_eq!(t.press(Button::Confirm), None);
        assert_eq!(t, Title::Menu(0));
        assert_eq!(t.press(Button::Up), None);
        assert_eq!(t, Title::Menu(5));
        t.press(Button::Down);
        t.press(Button::Down);
        assert_eq!(t.press(Button::Confirm), Some(Entry::LoadGame));
        t.press(Button::Back);
        assert_eq!(t, Title::PressAny);
    }

    #[test]
    fn lit_entry_and_cursor() {
        assert_eq!(prompt_at(0.0), 255);
        assert_eq!(prompt_at(1.2), 51);
        assert_eq!(prompt_at(2.4), 255);
        assert_eq!(white_at(0.0), 255);
        assert_eq!(white_at(0.15), 128);
        assert_eq!(white_at(0.31), 0);
        let e = Title::Menu(1).elements();
        assert!(e.contains(&Element::Sprite { part: 0x15e, cut: 1, at: [630, 616] }));
        assert!(e.contains(&Element::Sprite { part: 0x15d, cut: 0, at: [630, 538] }));
        assert!(e.contains(&Element::Sprite { part: LOGO, cut: 2, at: [584, 609] }));
        assert!(!Title::PressAny.elements().iter().any(|x| matches!(x, Element::Sprite { part: 0x15d, .. })));
    }
}
