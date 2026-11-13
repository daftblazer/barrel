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
        let mut out = vec![Element::Sprite { part: LOGO, cut: 0, at: LOGO_AT }];
        match *self {
            Title::PressAny => out.push(Element::Sprite { part: LOGO, cut: 1, at: [570, 660] }),
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
    Exit,
}

impl Front {
    fn press(&mut self, b: Button, saves: Option<&Path>) -> Option<Start> {
        match self {
            Front::Title(t) => match t.press(b)? {
                Entry::NewGame => *self = Front::Hero(crate::hero::Hero::new()),
                Entry::LoadGame => *self = Front::Load(crate::loadlist::LoadList::new(saves.map(barrel_game::saves::list).unwrap_or_default())),
                Entry::ExitGame => return Some(Start::Exit),
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
pub fn run(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    keys: &[Button],
    out: Option<&Path>,
    saves: Option<&Path>,
) -> Result<Option<Start>> {
    use crate::hero::Hero;
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let still = barrel_formats::layout::still(&pe, BACKGROUND).ok_or("title: no background picture")?;
    let x64 = root.join("Data/x64");
    let still_bytes = std::fs::read(x64.join(format!("{}_EN", still.folder)).join(&still.file)).or_else(|_| std::fs::read(x64.join(&still.folder).join(&still.file)))?;
    let still_g1t = barrel_formats::g1t::G1t::parse(&still_bytes)?;
    let hero_nodes = barrel_formats::layout::placed(&pe, crate::hero::LAYOUT);
    // The hero choice's confirming line, from the system messages.
    let sys_mess = std::fs::read(root.join("Saves_EN/systemMessage/SysMess.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok()).map(|t| barrel_formats::sys_text::sys_mess(&t)).unwrap_or_default();
    let question = sys_mess.get(crate::hero::QUESTION_ROW).map_or_else(|| "Do you want to use this character?".to_string(), |m| m.text.clone());
    let load_nodes = barrel_formats::layout::placed(&pe, crate::loadlist::LAYOUT);
    let texts = |p: &barrel_formats::layout::Placed| barrel_formats::layout::text(&pe, p);
    let glyphs = barrel_formats::font::glyphs(&pe, barrel_formats::font::FONT_EN).ok_or("title: no glyph table")?;
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
            && let Some(p) = barrel_formats::layout::sprite(&pe, part, cut, at) {
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
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |f: &Front| match f {
        Front::Title(t) => {
            // The background over the 1920 × 1080 screen, scaled to the height and centred.
            let k = h / 1080.0;
            let dx = (w - 1920.0 * k) / 2.0;
            let mut q = vec![text::Quad { rect: [dx, 0.0, dx + still.size[0] * k, still.size[1] * k], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], texture: 1 }];
            q.extend(hud::quads(&pe, None, &sheets, &t.elements(), w, h));
            q
        }
        Front::Hero(hero) => hud::quads(&pe, Some(&font), &sheets, &hero.elements(&hero_nodes, texts, &question), w, h),
        Front::Load(list) => hud::quads(&pe, Some(&font), &sheets, &list.elements(&load_nodes, texts), w, h),
    };
    let mut front = Front::Title(Title::new());
    crate::screen::run(gpu, pool, target, window, offscreen, &images, &mut front, quads, |f, b| f.press(b, saves), keys, out)
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
        let e = Title::Menu(1).elements();
        assert!(e.contains(&Element::Sprite { part: 0x15e, cut: 1, at: [630, 616] }));
        assert!(e.contains(&Element::Sprite { part: 0x15d, cut: 0, at: [630, 538] }));
        assert!(e.contains(&Element::Sprite { part: LOGO, cut: 2, at: [584, 609] }));
        assert!(!Title::PressAny.elements().iter().any(|x| matches!(x, Element::Sprite { part: 0x15d, .. })));
    }
}
