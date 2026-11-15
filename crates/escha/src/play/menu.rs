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
//! chosen button drawn as the yellow plate with the white name and icon and the frame (its cuts
//! turn with the clock), the others green with gold; all ten commands in the list, eight shown at a time,
//! scrolling with the chosen one; the Option rows' wording (Save, Load, Change BGM, Options,
//! Return to Title: the five lines `SysInfo.xml` describes, worded as the executable's strings
//! at 0x140794000) and their plates (the chosen white, cut 2, with the frame; the others cut 3);
//! the buttons at half strength while the Option list is open; "NEW" never shown; the figure always Escha's; no party plates (layout 365), help line, money
//! or date; no opening or closing motion.

use barrel_escha::data::layout::{Placed, Text};

use crate::hud::Element;
use crate::title::Button;

pub const BACKGROUND: u32 = 383;
pub const FIGURE: u32 = 361;
pub const LIST: u32 = 374;
const COMMAND: u32 = 373;
const OPTION_ROW: u32 = 378;
pub const PLATE: i32 = 0x1d;
const COMMAND_FRAME: i32 = 0x1e;
const OPTION_FRAME: i32 = 0x1f;
const LABEL: i32 = 0x2f;
const ICON: i32 = 0x2b;
const NEW: i32 = 0x4a;
/// The "NEW" badge's flipbook has five steps (node word 5) over four pictures (cuts 0 to 3, the
/// sheet's fifth is empty): the capture shows the last picture held for the fifth step.
const NEW_LAST_CUT: i32 = 3;
/// How large a button's icon is drawn (the game's capture: 54 of the sheet's 60 pixels).
const ICON_PERCENT: i32 = 90;
/// Cuts of a name or icon in white rather than gold.
const WHITE: i32 = 10;
/// The buttons the list shows at a time (layout 385's nine slots).
const VISIBLE: usize = 9;

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
    /// The page open over the menu, if any.
    pub page: Option<Page>,
    /// Opened straight on its page (the field's LB opens Assignment): closing the page closes
    /// the menu.
    pub direct: bool,
    /// The Container command in the list (the game leaves it out away from the workshop: its
    /// list on the area map starts at the Basket).
    pub container: bool,
    /// The clock's 60 Hz ticks since the menu opened (the glows' and badges' flipbooks).
    pub tick: u32,
    /// How many members the party has (the character pickers' and the formation's lists).
    pub party: usize,
}

/// A command's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Status,
    Basket(crate::basket::Basket),
    Assignment(crate::assignment::Assignment),
    Journal(crate::journal::Journal),
    Option(crate::optionpage::OptionPage),
    Picker(crate::picker::Kind, crate::picker::Picker),
    Formation(crate::formation::Formation),
}

/// Where a shortcut opens the menu: on a page, or on the Option list alone (the area map's RB,
/// "System").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Page(Page),
    System,
}

/// What a button did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Back to play.
    Close,
    Option(OptionRow),
    /// The Options screen's game page confirmed: its settings are to be kept.
    Sound(crate::optionpage::OptionPage),
    /// The Options screen turned to its graphics page (the second), carrying the game page's
    /// settings so far.
    Graphics(crate::optionpage::OptionPage),
    /// A command whose page Barrel does not have yet.
    Command(Command),
}

impl Menu {
    pub fn new() -> Menu {
        Menu { chosen: 0, first: 0, option: None, page: None, direct: false, container: true, tick: 0, party: 0 }
    }

    /// The menu opened straight on `page`, as the field's shortcuts do; backing out of the page
    /// goes back to play.
    pub fn at(start: Start) -> Menu {
        let chosen = COMMANDS.iter().position(|&c| match start {
            Start::Page(Page::Status) => c == Command::Status,
            Start::Page(Page::Basket(_)) => c == Command::Basket,
            Start::Page(Page::Assignment(_)) => c == Command::Assignment,
            Start::Page(Page::Journal(_)) => c == Command::Journal,
            Start::Page(Page::Option(_)) => c == Command::Option,
            Start::Page(Page::Picker(k, _)) => c == k.command(),
            Start::Page(Page::Formation(_)) => c == Command::Formation,
            Start::System => c == Command::Option,
        });
        let chosen = chosen.unwrap_or(0);
        let (page, option) = match start {
            Start::Page(p) => (Some(p), None),
            Start::System => (None, Some(0)),
        };
        Menu { chosen, first: (chosen + 1).saturating_sub(VISIBLE), page, option, direct: true, container: true, tick: 0, party: 0 }
    }

    /// The menu without its Container command (the chosen one stays the same command).
    pub fn without_container(mut self) -> Menu {
        if self.container {
            self.container = false;
            self.chosen = self.chosen.saturating_sub(1);
            self.first = self.first.saturating_sub(1);
        }
        self
    }

    /// The commands the list holds.
    pub fn commands(&self) -> &'static [Command] {
        if self.container { &COMMANDS } else { &COMMANDS[1..] }
    }

    /// A button press, with `count` items in the basket (for its page).
    pub fn press(&mut self, b: Button, count: usize) -> Option<Outcome> {
        // The Status picker's confirm opens the Status page (of the lead for now).
        if matches!(self.page, Some(Page::Picker(crate::picker::Kind::Status, _))) && b == Button::Confirm {
            self.page = Some(Page::Status);
            return None;
        }
        let close = match self.page.as_mut() {
            Some(Page::Status) => b == Button::Back,
            Some(Page::Basket(basket)) => basket.press(b, count),
            Some(Page::Assignment(a)) => a.press(b),
            Some(Page::Journal(j)) => j.press(b),
            Some(Page::Option(o)) => match o.press(b) {
                None => false,
                Some(crate::optionpage::Leave::Close(true)) => return Some(Outcome::Sound(*o)),
                Some(crate::optionpage::Leave::Close(false)) => true,
                Some(crate::optionpage::Leave::Graphics { .. }) => return Some(Outcome::Graphics(*o)),
            },
            Some(Page::Picker(_, p)) => p.press(b),
            Some(Page::Formation(f)) => f.press(b),
            None => return self.press_list(b),
        };
        if close {
            self.page = None;
            if self.direct {
                return Some(Outcome::Close);
            }
        }
        None
    }

    fn press_list(&mut self, b: Button) -> Option<Outcome> {
        if let Some(row) = self.option.as_mut() {
            let n = OPTION_ROWS.len();
            match b {
                Button::Up => *row = (*row + n - 1) % n,
                Button::Down => *row = (*row + 1) % n,
                Button::Left | Button::Right => {}
                Button::Back if self.direct => return Some(Outcome::Close),
                Button::Back => self.option = None,
                Button::Confirm => return Some(Outcome::Option(OPTION_ROWS[*row].0)),
            }
            return None;
        }
        let n = self.commands().len();
        match b {
            Button::Up => self.chosen = (self.chosen + n - 1) % n,
            Button::Down => self.chosen = (self.chosen + 1) % n,
            Button::Left | Button::Right => {}
            Button::Back => return Some(Outcome::Close),
            Button::Confirm => match self.commands()[self.chosen] {
                Command::Option => self.page = Some(Page::Option(crate::optionpage::OptionPage::load())),
                Command::Status => self.page = Some(Page::Picker(crate::picker::Kind::Status, crate::picker::Picker { chosen: 0, count: self.party })),
                Command::Basket => self.page = Some(Page::Basket(crate::basket::Basket::default())),
                Command::Assignment => self.page = Some(Page::Assignment(crate::assignment::Assignment::default())),
                Command::Journal => self.page = Some(Page::Journal(crate::journal::Journal::default())),
                c @ (Command::AdventureEquip | Command::BattleEquip) => {
                    let kind = if c == Command::BattleEquip { crate::picker::Kind::BattleEquip } else { crate::picker::Kind::AdventureEquip };
                    self.page = Some(Page::Picker(kind, crate::picker::Picker { chosen: 0, count: kind.count(self.party) }));
                }
                Command::Formation => self.page = Some(Page::Formation(crate::formation::Formation { chosen: 0, count: self.party })),
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
        self.elements_with(nodes, texts, None)
    }

    /// [`Menu::elements`] with the "NEW" badge on the button of `new`.
    pub fn elements_with(&self, nodes: &[Placed], texts: impl Fn(&Placed) -> Option<Text>, new: Option<Command>) -> Vec<Element> {
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
                    let Some(&command) = self.commands().get(k) else { continue };
                    let which = COMMANDS.iter().position(|&c| c == command).unwrap_or(0) as i32;
                    let lit = k == self.chosen && self.option.is_none();
                    let white = if lit { WHITE } else { 0 };
                    let cut = match n.part {
                        PLATE => i32::from(lit),
                        LABEL | ICON => which + white,
                        COMMAND_FRAME if lit => n.flip().map_or(0, |f| f.cut(self.tick)),
                        NEW if new == Some(command) => n.flip().map_or(0, |f| f.cut(self.tick).min(NEW_LAST_CUT)),
                        _ => continue, // the frame off, "NEW" unless due
                    };
                    // Under the open Option list the buttons are drawn at half strength; the icon's
                    // node is a kind 4 picture drawn at 90% about its corner.
                    out.push(match n.part {
                        _ if self.option.is_some() => Element::Dim { part: n.part, cut, at: p.at },
                        ICON => Element::Scaled { part: n.part, cut, at: p.at, percent: ICON_PERCENT, about: p.at },
                        _ => Element::Sprite { part: n.part, cut, at: p.at },
                    });
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
                _ => out.push(Element::stored(p)),
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
    source: Option<&crate::overview::Source>,
    hud: &[Element],
    stock: &crate::basket::Stock,
    card: &[Option<crate::assignment::Task>],
    keys: &[Button],
    start: Option<Start>,
    out: Option<&std::path::Path>,
) -> crate::Result<Option<Outcome>> {
    use crate::{hud, screen, text};
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let logy = source.is_some_and(|s| s.lead == 1);
    let lead = source.and_then(|s| s.party.get(s.lead));
    let nodes: Vec<Placed> = [BACKGROUND, FIGURE, LIST].iter().flat_map(|&id| barrel_escha::data::layout::placed(&pe, id)).collect();
    // The Status page: its nodes over the desk, and the lead's values.
    let status_nodes: Vec<Placed> = [BACKGROUND, crate::status::LAYOUT].iter().flat_map(|&id| barrel_escha::data::layout::placed(&pe, id)).collect();
    let item_rows = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/item/itemData.xml"))?)?;
    let values = crate::status::Values::lead(&pe, logy, lead, &item_rows.children).ok_or("menu: no party tables")?;
    // The Basket page: the items the player carries, with what the game's tables say of each.
    let table = |rel: &str| -> crate::Result<barrel_formats::table::Element> { Ok(barrel_formats::table::parse(&std::fs::read(root.join(rel))?)?) };
    let (status, potentials) = (table("Saves/item/item_status.xml")?, table("Saves_EN/item/item_potential.xml")?);
    let items = crate::basket::held(stock, &crate::basket::Catalog { items: &item_rows.children, status: &status.children, potentials: &potentials.children });
    let page_layouts = crate::basketpage::Layouts::load(&pe);
    let empty = (stock.limit - stock.items.len() as i32).max(0);
    let status_elements = |texts: &dyn Fn(&Placed) -> Option<Text>| {
        let mut e: Vec<crate::hud::Element> = status_nodes.iter().filter(|p| p.path.first() == Some(&BACKGROUND)).map(crate::hud::Element::stored).collect();
        e.extend(crate::status::elements(&status_nodes, texts, &values));
        e
    };
    // The party overview: the page the list shows when nothing is open over it.
    let ranks = barrel_escha::rank::ranks(&table("Saves/rank/RankData.xml")?);
    // The Assignment page: the period's card, the rank box's numbers and its two lines.
    let assignment_layouts = crate::assignment::Layouts::load(&pe);
    let (sys_info, line_help) = (table("Saves_EN/systemMessage/SysInfo.xml")?, table("Saves_EN/systemMessage/Menu_Line_Help.xml")?);
    let assignment_words = crate::assignment::Words {
        header: barrel_escha::data::sys_text::lines(&sys_info, "Text").get(crate::assignment::HEADER_ROW).cloned().unwrap_or_default(),
        footer: barrel_escha::data::sys_text::lines(&line_help, "LineHelp").get(crate::assignment::FOOTER_ROW).cloned().unwrap_or_default(),
    };
    let rank_panel = source.map_or(crate::requests::Panel { icon: 0, to_next: 0 }, |s| crate::requests::Panel::of(&ranks, s.rank.0, s.rank.1));
    let assignment_elements = |a: &crate::assignment::Assignment, tick: u32| a.elements(&pe, &assignment_layouts, &assignment_words, card, rank_panel, tick);
    let overview = source.and_then(|s| crate::overview::Overview::of(&pe, s, &item_rows.children, &ranks));
    let words = crate::overview::Words::read(&table("Saves_EN/systemMessage/SysInfo.xml")?, &table("Saves_EN/systemMessage/Menu_Line_Help.xml")?);
    let screen_layouts = crate::overview::Layouts::load(&pe);
    let cursor = screen_layouts.cursor;
    let page_words = crate::basketpage::Words { header: barrel_escha::data::sys_text::lines(&table("Saves_EN/systemMessage/SysInfo.xml")?, "Text").get(crate::basketpage::HEADER_ROW).cloned().unwrap_or_default(), footer: barrel_escha::data::sys_text::lines(&table("Saves_EN/systemMessage/Menu_Line_Help.xml")?, "LineHelp").get(crate::basketpage::FOOTER_ROW).cloned().unwrap_or_default() };
    let basket_elements = |b: &crate::basket::Basket, tick: u32| crate::basketpage::elements(&pe, &page_layouts, &page_words, b, &items, empty, tick);
    // The Journal page: its tabs, with the badges when the save has library marks clear.
    let journal_layouts = crate::journal::Layouts::load(&pe);
    let journal_words = crate::journal::Words {
        header: barrel_escha::data::sys_text::lines(&sys_info, "Text").get(crate::journal::HEADER_ROW).cloned().unwrap_or_default(),
        footer: barrel_escha::data::sys_text::lines(&line_help, "LineHelp").get(crate::journal::FOOTER_ROW).cloned().unwrap_or_default(),
    };
    let library_new = source.is_some_and(|s| s.library_new);
    let journal_elements = |j: &crate::journal::Journal, tick: u32| crate::journal::elements(&pe, &journal_layouts, &journal_words, j, library_new, tick);
    // The Option command's page: the Options screen, drawn over the overview.
    let option_layouts = crate::optionpage::Layouts::load(&pe);
    let option_words = crate::optionpage::Words {
        header: barrel_escha::data::sys_text::lines(&sys_info, "Text").get(crate::optionpage::HEADER_ROW).cloned().unwrap_or_default(),
        footer: barrel_escha::data::sys_text::lines(&line_help, "LineHelp").get(crate::optionpage::FOOTER_ROW).cloned().unwrap_or_default(),
    };
    let option_elements = |o: &crate::optionpage::OptionPage, tick: u32| crate::optionpage::elements(&pe, &option_layouts, &option_words, o, tick);
    // The character picker and the Formation page: the party in the squad's order (the lead and
    // the other alchemist when the save has none), each with its face and values.
    let sys_lines = barrel_escha::data::sys_text::lines(&sys_info, "Text");
    let help_lines = barrel_escha::data::sys_text::lines(&line_help, "LineHelp");
    let words_at = |header: usize, footer: usize| crate::picker::Words { header: sys_lines.get(header).cloned().unwrap_or_default(), footer: help_lines.get(footer).cloned().unwrap_or_default() };
    let (picker_words, formation_words) = (words_at(crate::picker::HEADER_ROW, crate::picker::FOOTER_ROW), words_at(crate::formation::HEADER_ROW, crate::formation::FOOTER_ROW));
    let lead_character = source.map_or(0, |s| s.lead);
    let party: Vec<crate::formation::Member> = source.map_or_else(Vec::new, |s| {
        let order: Vec<usize> = if s.squad.is_empty() { (0..2).collect() } else { s.squad.to_vec() };
        order.into_iter().filter_map(|c| Some(crate::formation::Member { face: crate::overview::Face::of(&pe, s, &item_rows.children, c)?, values: crate::status::Values::of(&pe, c, s.party.get(c), &item_rows.children)? })).collect()
    });
    let picker_layouts = crate::picker::Layouts::load(&pe);
    let formation_layouts = crate::formation::Layouts::load(&pe);
    let picker_elements = |k: crate::picker::Kind, v: &crate::picker::Picker, tick: u32| {
        let faces: Vec<crate::overview::Face> = party.iter().map(|m| &m.face).filter(|f| k.lists(&[f.character as usize]).len() == 1).cloned().collect();
        crate::picker::elements(&pe, &picker_layouts, &picker_words, k, v, &faces, lead_character, tick)
    };
    let formation_elements = |f: &crate::formation::Formation, tick: u32| crate::formation::elements(&pe, &formation_layouts, &formation_words, f, &party, lead_character, tick);
    let texts = |p: &Placed| barrel_escha::data::layout::text(&pe, p);
    let overview_elements = |m: &Menu, v: &crate::overview::Overview| {
        let mut e = crate::overview::elements(&pe, &screen_layouts, m, v, &words, texts);
        e.extend(hud.iter().cloned());
        // The cursor's glow over the chosen button, drawn last.
        e.extend(crate::overview::cursor(&screen_layouts, m));
        e
    };
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("menu: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("menu: an empty font atlas")?.width as f32 };
    // Every picture any state shows: each command lit and not, and the Option rows.
    let mut every = Vec::new();
    for chosen in 0..COMMANDS.len() {
        let first = chosen.saturating_sub(VISIBLE - 1);
        every.extend(Menu { chosen, first, ..Menu::new() }.elements(&nodes, texts));
        every.extend(Menu { chosen, first: 0, ..Menu::new() }.elements(&nodes, texts));
        every.extend(Menu { chosen, first: COMMANDS.len() - VISIBLE, ..Menu::new() }.elements(&nodes, texts));
    }
    for row in 0..OPTION_ROWS.len() {
        every.extend(Menu { chosen: COMMANDS.len() - 1, first: COMMANDS.len() - VISIBLE, option: Some(row), ..Menu::new() }.elements(&nodes, texts));
    }
    every.extend(status_elements(&texts));
    // Every cut the flipbooks turn through: the buttons' frames and badges, the pages' glows.
    every.extend(crate::kit::flip_elements(&nodes));
    every.extend(page_layouts.flips());
    every.extend(assignment_layouts.flips());
    every.extend(journal_layouts.flips());
    every.extend(option_layouts.flips());
    every.extend(picker_layouts.flips());
    every.extend(formation_layouts.flips());
    for chosen in 0..party.len().max(1) {
        for k in [crate::picker::Kind::AdventureEquip, crate::picker::Kind::BattleEquip, crate::picker::Kind::Status] {
            every.extend(picker_elements(k, &crate::picker::Picker { chosen, count: party.len() }, 0));
        }
        every.extend(formation_elements(&crate::formation::Formation { chosen, count: party.len() }, 0));
    }
    every.extend(cursor.into_iter().flat_map(|f| crate::kit::cuts_of(crate::kit::CURSOR, f)));
    if let Some(v) = overview.as_ref().map(|v| crate::overview::Overview { new_journal: true, ..v.clone() }) {
        for chosen in 0..COMMANDS.len() {
            every.extend(overview_elements(&Menu { chosen, first: chosen.saturating_sub(VISIBLE - 1), ..Menu::new() }, &v));
        }
    }
    // Every item's card.
    for chosen in 0..items.len().max(1) {
        for first in [0, chosen / crate::basket::COLUMNS] {
            every.extend(basket_elements(&crate::basket::Basket { chosen, first }, 0));
        }
    }
    for chosen in 0..25 {
        every.extend(assignment_elements(&crate::assignment::Assignment { chosen }, 0));
    }
    for chosen in 0..crate::journal::ROWS {
        every.extend(journal_elements(&crate::journal::Journal { chosen }, 0));
    }
    for chosen in 0..11 {
        for flags in 0..4 {
            every.extend(option_elements(&crate::optionpage::OptionPage { chosen, fast_forward_all: flags & 1 == 0, english: flags & 2 == 0, ..Default::default() }, 0));
        }
    }
    // Every task icon, plain and lit.
    every.extend((5..=15).map(|cut| crate::hud::Element::Sprite { part: 0xe, cut, at: [0, 0] }));
    // Every digit the numbers may show.
    every.extend((0..13).map(|cut| crate::hud::Element::Sprite { part: 0x4e, cut, at: [0, 0] }));
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
    let quads = |m: &Menu| {
        let e = match m.page {
            Some(Page::Status) => status_elements(&texts),
            Some(Page::Basket(b)) => basket_elements(&b, m.tick),
            Some(Page::Assignment(a)) => assignment_elements(&a, m.tick),
            Some(Page::Journal(j)) => journal_elements(&j, m.tick),
            // The game draws the menu's overview first and the Options screen over it.
            Some(Page::Option(o)) => {
                let mut e = match overview.as_ref() {
                    Some(v) => crate::overview::body(&pe, &screen_layouts, &Menu { page: None, ..*m }, v, &texts),
                    None => m.elements(&nodes, texts),
                };
                e.extend(option_elements(&o, m.tick));
                e
            }
            Some(Page::Picker(k, p)) => picker_elements(k, &p, m.tick),
            Some(Page::Formation(f)) => formation_elements(&f, m.tick),
            None if m.option.is_none() && overview.is_some() => overview_elements(m, overview.as_ref().unwrap()),
            None => m.elements(&nodes, texts),
        };
        hud::quads(&pe, Some(&font), &sheets, &e, w, h)
    };
    let mut menu = start.map_or_else(Menu::new, Menu::at);
    menu.party = party.len();
    if !source.is_none_or(|s| s.container) {
        menu = menu.without_container();
    }
    // Barrel acts on Close and on the Option rows it has; the rest only report.
    let count = items.len();
    let press = |m: &mut Menu, b: Button| match m.press(b, count)? {
        Outcome::Command(c) => {
            eprintln!("menu: {c:?} is not there yet");
            None
        }
        Outcome::Option(o @ OptionRow::ChangeBgm) => {
            eprintln!("menu: {o:?} is not there yet");
            None
        }
        o => Some(o),
    };
    screen::run_clocked(gpu, pool, target, window, offscreen, &images, &mut menu, quads, press, keys, out, |m, tick| {
        m.tick = tick;
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_escha::data::layout::Node;

    #[test]
    fn a_shortcut_page_closes_back_to_play() {
        let mut m = Menu::at(Start::System);
        assert_eq!((COMMANDS[m.chosen], m.option), (Command::Option, Some(0)));
        assert_eq!(m.press(Button::Back, 0), Some(Outcome::Close));
        let mut m = Menu::at(Start::Page(Page::Assignment(crate::assignment::Assignment::default())));
        assert_eq!(COMMANDS[m.chosen], Command::Assignment);
        assert_eq!(m.press(Button::Back, 0), Some(Outcome::Close));
        let mut m = Menu::new();
        m.page = Some(Page::Status);
        assert_eq!(m.press(Button::Back, 0), None);
        assert_eq!(m.page, None);
    }

    #[test]
    fn moves_scrolls_and_opens_options() {
        let mut m = Menu::new();
        assert_eq!(m.press(Button::Up, 0), None);
        assert_eq!((m.chosen, m.first), (9, 1));
        // The Option command opens the Options screen (the game's), the System list is the
        // shortcut's own.
        assert_eq!(m.press(Button::Confirm, 0), None);
        assert!(matches!(m.page, Some(Page::Option(_))));
        assert_eq!(m.press(Button::Back, 0), None);
        assert_eq!(m.page, None);
        let mut system = Menu::at(Start::System);
        system.press(Button::Up, 0);
        assert_eq!(system.press(Button::Confirm, 0), Some(Outcome::Option(OptionRow::ReturnToTitle)));
        m.press(Button::Down, 0);
        assert_eq!((m.chosen, m.first), (0, 0));
        m.press(Button::Down, 0);
        // Basket opens its page; Back closes it, then the menu.
        assert_eq!(m.press(Button::Confirm, 0), None);
        assert!(matches!(m.page, Some(Page::Basket(_))));
        assert_eq!(m.press(Button::Back, 0), None);
        assert_eq!(m.page, None);
        m.press(Button::Down, 0);
        assert_eq!(m.press(Button::Confirm, 0), None);
        assert!(matches!(m.page, Some(Page::Picker(crate::picker::Kind::AdventureEquip, _))));
        assert_eq!(m.press(Button::Back, 0), None);
        assert_eq!(m.page, None);
        assert_eq!(m.press(Button::Back, 0), Some(Outcome::Close));
    }

    #[test]
    fn the_list_may_leave_out_the_container() {
        let mut m = Menu::new().without_container();
        assert_eq!((m.commands().len(), m.commands()[m.chosen]), (9, Command::Basket));
        assert_eq!(m.press(Button::Up, 0), None);
        assert_eq!(m.commands()[m.chosen], Command::Option);
        assert_eq!(Menu::at(Start::System).without_container().commands()[m.chosen], Command::Option);
    }

    #[test]
    fn chosen_button_is_lit() {
        let node = |path: &[u32], part, at: [i32; 2], local: [i32; 2]| Placed { path: path.to_vec(), node: Node { part, cut: 0, at: local, kind: 0, mirror: 0, pace: 0 }, at, va: 0, tilt: None };
        let nodes = vec![
            node(&[LIST, COMMAND], PLATE, [36, 184], [0, 0]),
            node(&[LIST, COMMAND], LABEL, [124, 194], [88, 10]),
            node(&[LIST, COMMAND], NEW, [28, 176], [-8, -8]),
            node(&[LIST, COMMAND], PLATE, [48, 264], [0, 0]),
            node(&[LIST, COMMAND], LABEL, [136, 274], [88, 10]),
        ];
        let mut m = Menu::new();
        m.press(Button::Down, 0);
        let e = m.elements(&nodes, |_| None);
        assert_eq!(e, vec![
            Element::Sprite { part: PLATE, cut: 0, at: [36, 184] },
            Element::Sprite { part: LABEL, cut: 0, at: [124, 194] },
            Element::Sprite { part: PLATE, cut: 1, at: [48, 264] },
            Element::Sprite { part: LABEL, cut: 11, at: [136, 274] },
        ]);
    }
}
