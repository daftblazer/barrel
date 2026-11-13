//! The main menu opened in play, from the game's own layouts (`WinXls_EN/a15_menu.g1t`): the
//! desk background (layout 383), the lead's figure at the right (361, nesting 957 on
//! `a15_menu_chara01.g1t`), and the command list (374): eight command buttons (layout 373, one
//! under another and stepping right) and the five rows of the Option list (layout 378).
//!
//! A command button is a plate (part 0x1d: cut 0 green, cut 1 yellow), the command's name
//! (part 0x2f) and icon (part 0x2b), whose cuts 0 to 9 are the ten commands in gold and cuts 10
//! to 19 the same in white: Container, Basket, Adventure Equip., Battle Equip., Formation,
//! Status, Assignment, Request, Journal, Option. A glowing frame (part 0x1e, cuts 0 to 4 a pulse)
//! and a "NEW" badge (part 0x4a) complete it. An Option row is a plate (part 0x1d cut 2 or 3),
//! a frame (part 0x1f) and a text filled in at run time.
//!
//! Read from the layouts: the pictures and their places, the ten names. Not yet the game's: the
//! chosen button drawn as the yellow plate with the white name and icon and the frame (still,
//! not pulsing), the others green with gold; all ten commands in the list, eight shown at a time,
//! scrolling with the chosen one; the Option rows' wording (Save, Load, Change BGM, Options,
//! Return to Title: the five lines `SysInfo.xml` describes, worded as the executable's strings
//! at 0x140794000) and their plates (the chosen white, cut 2, with the frame; the others cut 3);
//! the buttons at half strength while the Option list is open; "NEW" never shown; the figure always Escha's; no party plates (layout 365), help line, money
//! or date; no opening or closing motion.

use barrel_formats::layout::{Placed, Text};

use crate::hud::Element;
use crate::title::Button;

pub const BACKGROUND: u32 = 383;
pub const FIGURE: u32 = 361;
pub const LIST: u32 = 374;
const COMMAND: u32 = 373;
const OPTION_ROW: u32 = 378;
const PLATE: i32 = 0x1d;
const COMMAND_FRAME: i32 = 0x1e;
const OPTION_FRAME: i32 = 0x1f;
const LABEL: i32 = 0x2f;
const ICON: i32 = 0x2b;
const NEW: i32 = 0x4a;
/// Cuts of a name or icon in white rather than gold.
const WHITE: i32 = 10;
const VISIBLE: usize = 8;

/// The commands, in the order of their names' cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Container,
    Basket,
    AdventureEquip,
    BattleEquip,
    Formation,
    Status,
    Assignment,
    Request,
    Journal,
    Option,
}

pub const COMMANDS: [Command; 10] = [
    Command::Container,
    Command::Basket,
    Command::AdventureEquip,
    Command::BattleEquip,
    Command::Formation,
    Command::Status,
    Command::Assignment,
    Command::Request,
    Command::Journal,
    Command::Option,
];

/// The Option list's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionRow {
    Save,
    Load,
    ChangeBgm,
    Options,
    ReturnToTitle,
}

pub const OPTION_ROWS: [(OptionRow, &str); 5] = [
    (OptionRow::Save, "Save"),
    (OptionRow::Load, "Load"),
    (OptionRow::ChangeBgm, "Change BGM"),
    (OptionRow::Options, "Options"),
    (OptionRow::ReturnToTitle, "Return to Title"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    pub chosen: usize,
    pub first: usize,
    /// The Option list open, on this row.
    pub option: Option<usize>,
}

/// What a button did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Back to play.
    Close,
    Option(OptionRow),
    /// A command whose page Barrel does not have yet.
    Command(Command),
}

impl Menu {
    pub fn new() -> Menu {
        Menu { chosen: 0, first: 0, option: None }
    }

    pub fn press(&mut self, b: Button) -> Option<Outcome> {
        if let Some(row) = self.option.as_mut() {
            let n = OPTION_ROWS.len();
            match b {
                Button::Up => *row = (*row + n - 1) % n,
                Button::Down => *row = (*row + 1) % n,
                Button::Left | Button::Right => {}
                Button::Back => self.option = None,
                Button::Confirm => return Some(Outcome::Option(OPTION_ROWS[*row].0)),
            }
            return None;
        }
        let n = COMMANDS.len();
        match b {
            Button::Up => self.chosen = (self.chosen + n - 1) % n,
            Button::Down => self.chosen = (self.chosen + 1) % n,
            Button::Left | Button::Right => {}
            Button::Back => return Some(Outcome::Close),
            Button::Confirm => match COMMANDS[self.chosen] {
                Command::Option => self.option = Some(0),
                c => return Some(Outcome::Command(c)),
            },
        }
        if self.chosen < self.first {
            self.first = self.chosen;
        } else if self.chosen >= self.first + VISIBLE {
            self.first = self.chosen + 1 - VISIBLE;
        }
        None
    }

    /// The elements to draw from the placed nodes of the background, the figure and the list
    /// (in that order; `texts` gives a node's text).
    pub fn elements(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>) -> Vec<Element> {
        let mut out = Vec::new();
        // Each button's or row's place, in the order they come, gives its slot.
        let (mut buttons, mut rows): (Vec<[i32; 2]>, Vec<[i32; 2]>) = (Vec::new(), Vec::new());
        for p in nodes {
            let n = p.node;
            let origin = [p.at[0] - n.at[0], p.at[1] - n.at[1]];
            let slot = |seen: &mut Vec<[i32; 2]>| match seen.iter().position(|&o| o == origin) {
                Some(k) => k,
                None => {
                    seen.push(origin);
                    seen.len() - 1
                }
            };
            match p.path.get(1) {
                Some(&COMMAND) => {
                    let r = slot(&mut buttons);
                    let k = self.first + r;
                    if k >= COMMANDS.len() {
                        continue;
                    }
                    let lit = k == self.chosen && self.option.is_none();
                    let white = if lit { WHITE } else { 0 };
                    let cut = match n.part {
                        PLATE => i32::from(lit),
                        LABEL | ICON => k as i32 + white,
                        COMMAND_FRAME if lit => 0,
                        _ => continue, // the frame off, "NEW"
                    };
                    // Under the open Option list the buttons are drawn at half strength.
                    out.push(if self.option.is_some() { Element::Dim { part: n.part, cut, at: p.at } } else { Element::Sprite { part: n.part, cut, at: p.at } });
                }
                Some(&OPTION_ROW) => {
                    let r = slot(&mut rows);
                    let Some(chosen) = self.option else { continue };
                    let Some((_, word)) = OPTION_ROWS.get(r) else { continue };
                    match n.part {
                        0x164 => {
                            if let Some(t) = texts(p) {
                                out.push(Element::Label(Text { text: (*word).into(), ..t }));
                            }
                        }
                        PLATE => out.push(Element::Sprite { part: PLATE, cut: if r == chosen { 2 } else { 3 }, at: p.at }),
                        OPTION_FRAME if r == chosen => out.push(Element::Sprite { part: n.part, cut: 0, at: p.at }),
                        _ => {}
                    }
                }
                _ if n.part == NEW => {}
                _ => out.push(Element::Sprite { part: n.part, cut: n.cut.max(0), at: p.at }),
            }
        }
        out
    }
}

impl Default for Menu {
    fn default() -> Menu {
        Menu::new()
    }
}

/// Shows the main menu until it is closed or ends in an outcome Barrel acts on (`None`: the
/// window was closed). Offscreen it takes `keys`, draws once and writes the picture to `out`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &crate::screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &std::path::Path,
    keys: &[Button],
    out: Option<&std::path::Path>,
) -> crate::Result<Option<Outcome>> {
    use crate::{hud, screen, text};
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let nodes: Vec<Placed> = [BACKGROUND, FIGURE, LIST].iter().flat_map(|&id| barrel_formats::layout::placed(&pe, id)).collect();
    let texts = |p: &Placed| barrel_formats::layout::text(&pe, p);
    let glyphs = barrel_formats::font::glyphs(&pe, barrel_formats::font::FONT_EN).ok_or("menu: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("menu: an empty font atlas")?.width as f32 };
    // Every picture any state shows: each command lit and not, and the Option rows.
    let mut every = Vec::new();
    for chosen in 0..COMMANDS.len() {
        let first = chosen.saturating_sub(VISIBLE - 1);
        every.extend(Menu { chosen, first, option: None }.elements(&nodes, texts));
        every.extend(Menu { chosen, first: 0, option: None }.elements(&nodes, texts));
        every.extend(Menu { chosen, first: COMMANDS.len() - VISIBLE, option: None }.elements(&nodes, texts));
    }
    for row in 0..OPTION_ROWS.len() {
        every.extend(Menu { chosen: COMMANDS.len() - 1, first: COMMANDS.len() - VISIBLE, option: Some(row) }.elements(&nodes, texts));
    }
    let keys_needed = screen::sheet_keys(&pe, every);
    let packs = screen::read_packs(root, &keys_needed)?;
    let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), barrel_formats::g1t::G1t::parse(b)?))).collect::<crate::Result<Vec<_>>>()?;
    let mut images = vec![&atlas.textures[0]];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
    for (name, index) in keys_needed {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |m: &Menu| hud::quads(&pe, Some(&font), &sheets, &m.elements(&nodes, texts), w, h);
    let mut menu = Menu::new();
    // Barrel acts on Close and on the Option rows it has; the rest only report.
    let press = |m: &mut Menu, b: Button| match m.press(b)? {
        Outcome::Command(c) => {
            eprintln!("menu: {c:?} is not there yet");
            None
        }
        Outcome::Option(o @ (OptionRow::ChangeBgm | OptionRow::Options)) => {
            eprintln!("menu: {o:?} is not there yet");
            None
        }
        o => Some(o),
    };
    screen::run(gpu, pool, target, window, offscreen, &images, &mut menu, quads, press, keys, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::layout::Node;

    #[test]
    fn moves_scrolls_and_opens_options() {
        let mut m = Menu::new();
        assert_eq!(m.press(Button::Up), None);
        assert_eq!((m.chosen, m.first), (9, 2));
        assert_eq!(m.press(Button::Confirm), None);
        assert_eq!(m.option, Some(0));
        m.press(Button::Up);
        assert_eq!(m.press(Button::Confirm), Some(Outcome::Option(OptionRow::ReturnToTitle)));
        m.press(Button::Back);
        assert_eq!(m.option, None);
        m.press(Button::Down);
        assert_eq!((m.chosen, m.first), (0, 0));
        m.press(Button::Down);
        assert_eq!(m.press(Button::Confirm), Some(Outcome::Command(Command::Basket)));
        assert_eq!(m.press(Button::Back), Some(Outcome::Close));
    }

    #[test]
    fn chosen_button_is_lit() {
        let node = |path: &[u32], part, at: [i32; 2], local: [i32; 2]| Placed { path: path.to_vec(), node: Node { part, cut: 0, at: local, kind: 0 }, at, va: 0 };
        let nodes = vec![
            node(&[LIST, COMMAND], PLATE, [36, 184], [0, 0]),
            node(&[LIST, COMMAND], LABEL, [124, 194], [88, 10]),
            node(&[LIST, COMMAND], NEW, [28, 176], [-8, -8]),
            node(&[LIST, COMMAND], PLATE, [48, 264], [0, 0]),
            node(&[LIST, COMMAND], LABEL, [136, 274], [88, 10]),
        ];
        let mut m = Menu::new();
        m.press(Button::Down);
        let e = m.elements(&nodes, |_| None);
        assert_eq!(e, vec![
            Element::Sprite { part: PLATE, cut: 0, at: [36, 184] },
            Element::Sprite { part: LABEL, cut: 0, at: [124, 194] },
            Element::Sprite { part: PLATE, cut: 1, at: [48, 264] },
            Element::Sprite { part: LABEL, cut: 11, at: [136, 274] },
        ]);
    }
}
