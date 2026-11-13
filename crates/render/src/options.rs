//! The Options screen's graphics page (`docs/renderer.md`, "Graphics settings"): the game's
//! Options screen (layout 511: the desk, its title bar) with Barrel's graphics rows drawn from
//! the sound page's pieces (layout 408 at (584, 150): a row's name on its bar 0x3c, the rules
//! 0x28, the cursor 0x32, the "Default" / "Confirm" buttons 0x3d), each row's value in a 0x3d
//! box. Left / right step a row to its next open choice; closed ones are named, with the
//! reason, in the help bar; a row with no other open choice shows its value grey. Confirm on
//! "Confirm" keeps the settings, "Default" puts the preset back, back leaves them as they were.
//!
//! Not yet the game's (none of this is the game's: the PC release has no such page): the
//! page's title "Graphics" (a plain label beside the screen's own heading), the help bar (a
//! plain line at the screen's foot), the sound page itself.

use std::path::Path;

use barrel_formats::layout::Text;

use crate::graphics_options::{self as g, Row};
use crate::hud::{self, Element};
use crate::screen;
use crate::settings::{Display, RenderSettings, WINDOW_SIZES};
use crate::title::Button;
use crate::{Result, text};

const SCREEN: u32 = 511;
const PAGE_AT: [i32; 2] = [584, 150];
const ROW_STEP: i32 = 74;
const FIRST_ROW: i32 = 70;
const VALUE_BOX: [i32; 2] = [300, -0];
const VALUE_CENTRE: i32 = 492;
const DARK: [u8; 4] = [0xff, 0x1a, 0x28, 0x5a];
const GREY: [u8; 4] = [0xff, 0x78, 0x78, 0x81];
const HELP_AT: [i32; 2] = [150, 1012];

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
        return "Left / Right: change".into();
    }
    // A whole row closed for one reason: the reason alone.
    if closed.len() == 1 && choices(row, now).iter().all(|(_, r)| r.is_err()) {
        return closed[0].1.to_string();
    }
    closed.iter().map(|(l, w)| format!("{l}: {w}")).collect::<Vec<_>>().join("  ")
}

impl Page {
    /// A button; `Some(true)` keeps the settings, `Some(false)` leaves them.
    pub fn press(&mut self, b: Button) -> Option<bool> {
        let rows = g::ROWS.len();
        match b {
            Button::Up => self.cursor = (self.cursor + rows + 1) % (rows + 2),
            Button::Down => self.cursor = (self.cursor + 1) % (rows + 2),
            Button::Left | Button::Right if self.cursor < rows => {
                if let Some(t) = stepped(g::ROWS[self.cursor], &self.settings, if b == Button::Right { 1 } else { -1 }) {
                    self.settings = t;
                }
            }
            Button::Confirm if self.cursor == rows => self.settings = self.default,
            Button::Confirm if self.cursor == rows + 1 => return Some(true),
            Button::Back => return Some(false),
            _ => {}
        }
        None
    }

    /// The page's elements past the screen's own (desk, title bar).
    pub fn elements(&self) -> Vec<Element> {
        let at = |x: i32, y: i32| [PAGE_AT[0] + x, PAGE_AT[1] + y];
        let label = |text: String, at: [i32; 2], colour: [u8; 4], kind: i32| Element::Label(Text { at, size: 33.0, colour, text, kind });
        let mut out = Vec::new();
        for (k, &row) in g::ROWS.iter().enumerate() {
            let y = FIRST_ROW + ROW_STEP * k as i32;
            out.push(Element::Sprite { part: 0x3c, cut: 0, at: at(24, y) });
            out.push(label(name(row).into(), at(50, y), DARK, 21));
            out.push(Element::Sprite { part: 0x3d, cut: 0, at: at(VALUE_BOX[0], y) });
            let open = choices(row, &self.settings).iter().any(|(l, r)| r.is_ok() && *l != value(row, &self.settings));
            let shown = value(row, &self.settings);
            let shown = if open { format!("<  {shown}  >") } else { shown };
            out.push(label(shown, at(VALUE_CENTRE, y), if open { DARK } else { GREY }, 22));
            out.push(Element::Sprite { part: 0x28, cut: 0, at: at(0, y + 40) });
        }
        let buttons = FIRST_ROW + ROW_STEP * g::ROWS.len() as i32 + 30;
        for (k, text) in ["Default", "Confirm"].into_iter().enumerate() {
            let y = buttons + 54 * k as i32;
            out.push(Element::Sprite { part: 0x3d, cut: 0, at: at(224, y) });
            out.push(label(text.into(), at(398 + 18, y), DARK, 22));
        }
        let cursor_y = if self.cursor < g::ROWS.len() { FIRST_ROW + ROW_STEP * self.cursor as i32 - 10 } else { buttons + 54 * (self.cursor - g::ROWS.len()) as i32 - 10 };
        let cursor_x = if self.cursor < g::ROWS.len() { -72 } else { 152 };
        out.push(Element::Sprite { part: 0x32, cut: 0, at: at(cursor_x, cursor_y) });
        let help_line = if self.cursor < g::ROWS.len() { help(g::ROWS[self.cursor], &self.settings) } else if self.cursor == g::ROWS.len() { "Put the graphics back to the defaults".into() } else { "Keep these settings".into() };
        out.push(label(help_line, HELP_AT, [0, 0, 0, 0], 21));
        out.push(Element::Label(Text { at: [1300, 62], size: 39.0, colour: [0, 0, 0, 0], text: "Graphics".into(), kind: 21 }));
        out
    }
}

/// Runs the page over the Options screen until it is left; the settings chosen, if kept.
#[allow(clippy::too_many_arguments)]
pub fn run(
    gpu: &crate::Gpu,
    pool: ash::vk::CommandPool,
    target: &screen::Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<ash::vk::Image>,
    root: &Path,
    settings: Graphics,
    default: Graphics,
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<Graphics>> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_formats::font::glyphs(&pe, barrel_formats::font::FONT_EN).ok_or("options: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes)?;
    let font = text::Font { glyphs, size: atlas.textures.first().ok_or("options: an empty font atlas")?.width as f32 };
    // The screen's own: the desk and the title bar (its heading and icon).
    let frame: Vec<Element> = barrel_formats::layout::placed(&pe, SCREEN)
        .iter()
        .filter(|p| matches!(p.node.part, 0x0 | 0x30 | 0x2e | 0x2c))
        .map(Element::stored)
        .collect();
    let resolved = |g: Graphics| Graphics { render: g.render.resolved(), ..g };
    let mut page = Page { cursor: 0, settings: resolved(settings), default: resolved(default) };
    let all = |p: &Page| {
        let mut e = frame.clone();
        e.extend(p.elements());
        e
    };
    let keys_needed = screen::sheet_keys(&pe, all(&page));
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
    let quads = |p: &Page| hud::quads(&pe, Some(&font), &sheets, &all(p), w, h);
    let kept = screen::run(gpu, pool, target, window, offscreen, &images, &mut page, quads, |p, b| p.press(b), keys, out)?;
    Ok(match kept {
        Some(true) => Some(page.settings),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(p.press(Button::Back), Some(false));
    }
}
