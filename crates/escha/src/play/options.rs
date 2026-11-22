//! The Options screen's graphics page (`docs/renderer.md`, "Graphics settings"): the game's
//! Options screen (layout 511: the desk, its title bar) with Barrel's graphics rows drawn from
//! the sound page's pieces (layout 408 at (584, 150): a row's name on its bar 0x3c, the rules
//! 0x28, the cursor 0x32, the "Default" / "Confirm" buttons 0x3d), each row's value in a 0x3d
//! box. Left / right step a row to its next open choice; closed ones are named, with the
//! reason, in the help bar; a row with no other open choice shows its value grey. Confirm on
//! "Confirm" keeps the settings, "Default" puts the preset back, back leaves them as they were.
//!
//! The Options screen has two pages: the game's own (`optionpage.rs`: volumes, Fast Forward
//! Event, Voice Language) and this one. Down off "Confirm" or Up off the first row turns from one
//! to the other (the way the Journal's tabs are a list the cursor walks; the game's screens have
//! no shoulder-button paging). Either page's "Confirm" keeps both pages' settings, Back leaves
//! both as they were.
//!
//! Not yet the game's (none of this is the game's: the PC release has no such page): the
//! page's title "Graphics" (a plain label beside the screen's own heading), the help bar (a
//! plain line at the screen's foot), the sound page itself.

use std::path::Path;

use barrel_escha::data::layout::{self, Placed, Text};
use barrel_formats::pe::Pe;

use crate::play::graphics_options::{self as g, Row};
use crate::play::hud::{self, Element};
use crate::play::screen;
use crate::play::settings::{Display, RenderSettings, WINDOW_SIZES};
use crate::play::kit::raised;
use crate::play::optionpage::{self, Layouts, Leave, OptionPage, Words};
use crate::play::title::Button;
use crate::play::{Result, text};

const SCREEN: u32 = 511;
/// The game's row bar and button plates and its text part (layout 408).
const PLATE: i32 = 0x3c;
const BUTTON: i32 = 0x3d;
const TEXT: i32 = 0x164;
/// The page's name in the header bar.
const PAGE_NAME: &str = "Graphics";

/// What the page sets: the render settings and the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Graphics {
    pub render: RenderSettings,
    pub display: Display,
}

impl Graphics {
    pub fn to_config(&self) -> String {
        self.display.to_config() + &self.render.to_config()
    }
}

/// Where a button took the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// The screen is left, keeping the settings or not.
    Done(bool),
    /// To the game's page: its first entry, or its last (from going up).
    Game { last: bool },
}

/// The page: the row the cursor is on (the rows, then "Default", "Confirm") and the settings
/// as chosen so far.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub cursor: usize,
    pub settings: Graphics,
    pub default: Graphics,
}

fn name(row: Row) -> &'static str {
    match row {
        Row::Mode => "Display",
        Row::WindowSize => "Window Size",
        Row::Scale => "Render Scale",
        Row::AntiAliasing => "Anti-Aliasing",
        Row::Upscaling => "Upscaler",
        Row::Sharpening => "Sharpening",
        Row::Shadows => "Shadows",
        Row::Filtering => "Texture Filtering",
        Row::Occlusion => "Ambient Occlusion",
    }
}

/// A choice: its label and what it makes, or why it is closed.
pub type Choice = (String, std::result::Result<Graphics, &'static str>);

/// A row's choices in order.
pub fn choices(row: Row, now: &Graphics) -> Vec<Choice> {
    let s = &now.render;
    let render = |f: &dyn Fn(&mut RenderSettings)| {
        let mut t = *now;
        f(&mut t.render);
        t
    };
    let display = |d: Display| Graphics { display: d, ..*now };
    match row {
        Row::Mode => [false, true].iter().map(|&full| (if full { "Fullscreen" } else { "Window" }.to_string(), Ok(display(Display { fullscreen: full, ..now.display })))).collect(),
        Row::WindowSize => WINDOW_SIZES
            .iter()
            .map(|&(w, h)| {
                let open = if now.display.fullscreen { Err("Fullscreen takes the screen's own size.") } else { Ok(display(Display { width: w, height: h, ..now.display })) };
                (format!("{w} x {h}"), open)
            })
            .collect(),
        Row::Scale => g::SCALES.iter().map(|&k| (format!("{}%", (k * 100.0).round()), Ok(render(&|t| g::set_scale(t, k))))).collect(),
        Row::AntiAliasing => g::AA_CHOICES.iter().map(|&a| (a.label(), g::aa_open(s, a).map(|_| render(&|t| g::set_aa(t, a))))).collect(),
        Row::Upscaling => g::UP_CHOICES.iter().map(|&u| (u.label().to_string(), g::up_open(s, u).map(|_| render(&|t| g::set_upscaler(t, u))))).collect(),
        Row::Sharpening => [false, true].iter().map(|&on| (if on { "On" } else { "Off" }.to_string(), g::sharpen_open(s).map(|_| render(&|t| g::set_sharpen(t, on))))).collect(),
        Row::Shadows => [false, true].iter().map(|&high| (if high { "High" } else { "Low" }.to_string(), Ok(render(&|t| g::set_shadows(t, high))))).collect(),
        Row::Filtering => g::FILTERING.iter().map(|&n| (if n == 1 { "Off".to_string() } else { format!("{n}x") }, Ok(render(&|t| t.anisotropy = n)))).collect(),
        Row::Occlusion => [false, true].iter().map(|&on| (if on { "On" } else { "Off" }.to_string(), Ok(render(&|t| g::set_occlusion(t, on))))).collect(),
    }
}

/// The row's value as shown.
pub fn value(row: Row, now: &Graphics) -> String {
    let s = &now.render;
    match row {
        Row::Mode => if now.display.fullscreen { "Fullscreen" } else { "Window" }.to_string(),
        Row::WindowSize => format!("{} x {}", now.display.width, now.display.height),
        Row::Scale => format!("{}%", (s.scale * 100.0).round()),
        Row::AntiAliasing => g::aa(s).label(),
        Row::Upscaling => g::upscaler(s).label().to_string(),
        Row::Sharpening => if s.sharpen { "On" } else { "Off" }.to_string(),
        Row::Shadows => if s.shadows.size >= 2048 { "High" } else { "Low" }.to_string(),
        Row::Filtering => if s.anisotropy <= 1 { "Off".to_string() } else { format!("{}x", s.anisotropy) },
        Row::Occlusion => if s.occlusion { "On" } else { "Off" }.to_string(),
    }
}

/// The next open choice of `row` from its value the way `step` (+1 or -1) goes, wrapping round.
fn stepped(row: Row, now: &Graphics, step: i32) -> Option<Graphics> {
    let all = choices(row, now);
    let shown = value(row, now);
    let n = all.len() as i32;
    let at = all.iter().position(|(l, _)| *l == shown).map_or(if step > 0 { -1 } else { n }, |k| k as i32);
    (1..=n).map(|k| (at + step * k).rem_euclid(n)).find_map(|k| all[k as usize].1.ok().filter(|t| value(row, t) != shown))
}

/// What a row sets, for the header bar when nothing on it is closed (Barrel's own words, shared with Ryza's page).
fn description(row: Row) -> &'static str {
    g::description(row)
}

/// Whether the row has a choice open other than its value.
fn row_open(row: Row, now: &Graphics) -> bool {
    let shown = value(row, now);
    choices(row, now).iter().any(|(l, r)| r.is_ok() && *l != shown)
}

/// Why the cursor's row has choices closed (the help bar's line).
pub fn help(row: Row, now: &Graphics) -> String {
    let mut closed: Vec<(String, &str)> = Vec::new();
    for (label, why) in choices(row, now).into_iter().filter_map(|(l, r)| r.err().map(|w| (l, w))) {
        match closed.iter_mut().find(|(_, w)| *w == why) {
            Some((ls, _)) => ls.push_str(&format!(", {label}")),
            None => closed.push((label, why)),
        }
    }
    if closed.is_empty() {
        return description(row).into();
    }
    // A whole row closed for one reason: the reason alone.
    if closed.len() == 1 && choices(row, now).iter().all(|(_, r)| r.is_err()) {
        return closed[0].1.to_string();
    }
    closed.iter().map(|(l, w)| format!("{l}: {w}")).collect::<Vec<_>>().join("  ")
}

impl Page {
    /// A button; `Some` ends the page.
    pub fn press(&mut self, b: Button) -> Option<Turn> {
        let rows = g::ROWS.len();
        match b {
            Button::Up if self.cursor == 0 => return Some(Turn::Game { last: true }),
            Button::Down if self.cursor == rows + 1 => return Some(Turn::Game { last: false }),
            Button::Up => self.cursor -= 1,
            Button::Down => self.cursor += 1,
            Button::Left | Button::Right if self.cursor < rows => {
                if let Some(t) = stepped(g::ROWS[self.cursor], &self.settings, if b == Button::Right { 1 } else { -1 }) {
                    self.settings = t;
                }
            }
            Button::Confirm if self.cursor == rows => self.settings = self.default,
            Button::Confirm if self.cursor == rows + 1 => return Some(Turn::Done(true)),
            Button::Back => return Some(Turn::Done(false)),
            _ => {}
        }
        None
    }

    /// The page's elements: the game's own Options frame (desk, panel, rules, plate, quill
    /// glow, title, footer and header bars) with this page's rows in the game's row slots.
    pub fn elements(&self, pe: &Pe, l: &Layouts, words: &Words, tick: u32) -> Vec<Element> {
        let rows = g::ROWS.len();
        let nodes = l.page();
        let of = |part: i32| nodes.iter().copied().filter(move |p| p.node.part == part);
        let bars: Vec<&Placed> = of(PLATE).take(optionpage::VOLUMES).collect();
        let boxes: Vec<&Placed> = of(PLATE).skip(optionpage::VOLUMES).collect();
        let buttons: Vec<&Placed> = of(BUTTON).collect();
        let texts: Vec<Text> = of(TEXT).filter_map(|p| layout::text(pe, p)).collect();
        let is_grey = |t: &&Text| t.colour[1] == 0x81;
        let (Some(dark), Some(grey)) = (texts.iter().find(|t| !is_grey(t) && t.kind == 21), texts.iter().find(is_grey)) else { return Vec::new() };
        // The game's two choice columns (its "Read" / "All" texts).
        let columns: Vec<i32> = texts.iter().filter(is_grey).map(|t| t.at[0]).take(2).collect();
        let say = |t: &Text, text: &str, at: [i32; 2]| raised(Text { at, ..t.clone() }, text);
        // The game's rows are 74 apart; its volume bars are seven, this page has eight rows.
        let row_y = |k: usize| bars[0].at[1] + k as i32 * (bars[1].at[1] - bars[0].at[1]);

        let mut out = l.desk();
        out.extend(l.panel(pe, rows));
        let current = |row: Row| value(row, &self.settings);
        let open = |row: Row| row_open(row, &self.settings);
        let two_way = |row: Row| open(row) && choices(row, &self.settings).len() == 2;
        // The chosen entry's plate: the row's bar (a two-way row's box under its chosen word)
        // or the button.
        let plate = if self.cursor < rows {
            let row = g::ROWS[self.cursor];
            let second = choices(row, &self.settings).get(1).is_some_and(|(label, _)| *label == current(row));
            match boxes.get(usize::from(second)) {
                Some(b) if two_way(row) => [b.at[0], row_y(self.cursor)],
                _ => [bars[0].at[0], row_y(self.cursor)],
            }
        } else {
            buttons[self.cursor - rows].at
        };
        let plate_part = if self.cursor < rows { PLATE } else { BUTTON };
        out.push(Element::Sprite { part: plate_part, cut: 0, at: plate });
        for (k, &row) in g::ROWS.iter().enumerate() {
            out.push(say(dark, name(row), [dark.at[0], row_y(k)]));
            if two_way(row) {
                for (i, (label, _)) in choices(row, &self.settings).iter().enumerate() {
                    out.push(say(if *label == current(row) { dark } else { grey }, label, [columns[i], row_y(k)]));
                }
            } else {
                out.push(say(if open(row) { dark } else { grey }, &current(row), [columns[0], row_y(k)]));
            }
        }
        // The buttons' words, where the game's are.
        out.extend(texts.iter().filter(|t| t.kind == 22).map(|t| raised(t.clone(), &t.text)));
        out.extend(l.title());
        let help_line = match self.cursor {
            k if k < rows => help(g::ROWS[k], &self.settings),
            k if k == rows => "Put the graphics back to the defaults.".into(),
            _ => "Keep these settings.".into(),
        };
        out.extend(l.lines(pe, &Words { header: format!("{PAGE_NAME}: {help_line}"), footer: words.footer.clone() }));
        // The quill stays at the row's bar, whichever box carries the plate.
        out.push(l.glow([bars[0].at[0].min(plate[0]), plate[1]], tick));
        out
    }
}

/// What the Options screen sets: both pages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub graphics: Graphics,
    pub game: OptionPage,
}

/// The screen's state: the page up, both pages' values and the clock (the game page's glow).
#[derive(Clone, Debug, PartialEq)]
struct Screen {
    on_game: bool,
    game: OptionPage,
    graphics: Page,
    tick: u32,
}

impl Screen {
    /// A button; `Some(keep)` leaves the screen.
    fn press(&mut self, b: Button) -> Option<bool> {
        if self.on_game {
            match self.game.press(b)? {
                Leave::Close(keep) => return Some(keep),
                Leave::Graphics { last } => {
                    self.on_game = false;
                    self.graphics.cursor = if last { g::ROWS.len() + 1 } else { 0 };
                }
            }
        } else {
            match self.graphics.press(b)? {
                Turn::Done(keep) => return Some(keep),
                Turn::Game { last } => {
                    self.on_game = true;
                    self.game = self.game.at(last);
                }
            }
        }
        None
    }
}

/// Runs the Options screen (opened on the graphics page, or on the game's) until it is left;
/// the settings chosen, if kept.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::play::Gpu,
    pool: ash::vk::CommandPool,
    target: &screen::Target,
    window: Option<(&crate::play::present::Presenter, &mut crate::play::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &Path,
    now: Settings,
    default: Graphics,
    on_game: bool,
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<Settings>> {
    let exe = barrel_files::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("options: no glyph table")?;
    let atlas_bytes = barrel_files::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("options: an empty font atlas")?.width as f32 };
    // The game's page: its layouts and its two lines of words.
    let table = |rel: &str| -> Result<barrel_formats::table::Element> { Ok(barrel_formats::table::parse(&barrel_files::read(root.join(rel))?)?) };
    let (sys_info, line_help) = (table("Saves_EN/systemMessage/SysInfo.xml")?, table("Saves_EN/systemMessage/Menu_Line_Help.xml")?);
    let layouts = optionpage::Layouts::load(&pe);
    let words = optionpage::Words {
        header: barrel_escha::data::sys_text::lines(&sys_info, "Text").get(optionpage::HEADER_ROW).cloned().unwrap_or_default(),
        footer: barrel_escha::data::sys_text::lines(&line_help, "LineHelp").get(optionpage::FOOTER_ROW).cloned().unwrap_or_default(),
    };
    let resolved = |g: Graphics| Graphics { render: g.render.resolved(), ..g };
    let mut screen = Screen { on_game, game: now.game, graphics: Page { cursor: 0, settings: resolved(now.graphics), default: resolved(default) }, tick: 0 };
    let all = |s: &Screen| {
        if s.on_game {
            optionpage::elements(&pe, &layouts, &words, &s.game, s.tick)
        } else {
            s.graphics.elements(&pe, &layouts, &words, s.tick)
        }
    };
    // Every picture either page turns through, for loading once.
    let mut every = layouts.flips();
    for chosen in 0..optionpage::ENTRIES {
        for flags in 0..4 {
            every.extend(optionpage::elements(&pe, &layouts, &words, &OptionPage { chosen, fast_forward_all: flags & 1 == 0, english: flags & 2 == 0, ..screen.game }, 0));
        }
    }
    for cursor in 0..g::ROWS.len() + 2 {
        let graphics = Page { cursor, ..screen.graphics.clone() };
        every.extend(all(&Screen { on_game: false, graphics, ..screen.clone() }));
    }
    let keys_needed = screen::sheet_keys(&pe, every);
    let packs = screen::read_packs(root, &keys_needed)?;
    let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), barrel_formats::g1t::G1t::parse(b)?))).collect::<Result<Vec<_>>>()?;
    let mut images = vec![&atlas.textures[0]];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
    for (name, index) in keys_needed {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |s: &Screen| hud::quads(&pe, Some(&font), &sheets, &all(s), w, h);
    let kept = screen::run_clocked(gpu, pool, target, window, offscreen, &images, &mut screen, quads, |s, b| s.press(b), keys, out, |s, tick| {
        s.tick = tick;
        None
    })?;
    Ok(match kept {
        Some(true) => Some(Settings { graphics: screen.graphics.settings, game: screen.game }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_occlusion_row_toggles_the_setting_and_is_not_on_this_page() {
        assert!(!g::ROWS.contains(&Row::Occlusion));
        let start = Graphics { render: RenderSettings::DESKTOP.resolved(), display: Display::DEFAULT };
        assert_eq!(value(Row::Occlusion, &start), "On");
        let off = stepped(Row::Occlusion, &start, 1).unwrap();
        assert!(!off.render.occlusion);
        assert_eq!(value(Row::Occlusion, &off), "Off");
        assert_eq!(name(Row::Occlusion), "Ambient Occlusion");
    }

    #[test]
    fn right_skips_closed_choices() {
        let start = Graphics { render: RenderSettings::DESKTOP.resolved(), display: Display::DEFAULT };
        let row = |r: Row| g::ROWS.iter().position(|&x| x == r).unwrap();
        // Desktop at 100%: the upscaler row has only bilinear open; anti-aliasing goes Off →
        // MSAA 2x → ... → FSR 3 Native AA.
        let mut p = Page { cursor: row(Row::Upscaling), settings: start, default: start };
        p.press(Button::Right);
        assert_eq!(g::upscaler(&p.settings.render), g::Up::Bilinear);
        assert!(help(Row::Upscaling, &p.settings).contains("FSR 1, GSR 1, GSR 2, FSR 3: Upscalers need a render scale under 100%."));
        p.cursor = row(Row::AntiAliasing);
        p.press(Button::Left);
        assert_eq!(g::aa(&p.settings.render), g::Aa::Native);
        // Native AA closes sharpening.
        assert!(choices(Row::Sharpening, &p.settings).iter().all(|(_, r)| r.is_err()));
        // Fullscreen closes the window size.
        p.cursor = row(Row::Mode);
        p.press(Button::Right);
        assert!(p.settings.display.fullscreen);
        assert_eq!(help(Row::WindowSize, &p.settings), "Fullscreen takes the screen's own size.");
        // Default puts it back; back leaves.
        p.cursor = g::ROWS.len();
        p.press(Button::Confirm);
        assert_eq!(p.settings, start);
        assert_eq!(p.press(Button::Back), Some(Turn::Done(false)));
        p.cursor = 0;
        assert_eq!(p.press(Button::Up), Some(Turn::Game { last: true }));
    }
}
