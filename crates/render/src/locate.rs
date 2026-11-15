//! The "Locate game files" screen, shown when no game install was found (`barrel_files::find`),
//! and the picker that asks which game to start when several were found.
//! The game's own art cannot be loaded before its files are known, so the screen is drawn with
//! Barrel's own 2D: flat rectangles and a small font atlas of its own (`locate_font`, Noto Sans,
//! made by `tools/gen-locate-font.py`), through the same text layer as the game's screens.
//!
//! A folder list ([`barrel_files::browse::Browser`]) is stepped with the arrows or the pad's
//! D-pad, Enter or A opens a folder (and chooses one that holds the game), Backspace or B goes
//! up a folder, Escape leaves.

use ash::vk;
use barrel_files::browse::{Browser, Input, Row};
use barrel_files::find::Title;
use barrel_formats::g1t::{Format, Texture};
use winit::keyboard::KeyCode;

use crate::locate_font as font;
use crate::text::{Layer, Quad, FONT, UI};
use crate::window::Pump;
use crate::{Gpu, Result};

const ALPHA: &[u8] = include_bytes!("locate_font.alpha");

const BACKGROUND: [f32; 4] = [0.055, 0.063, 0.082, 1.0];
const PANEL: [f32; 4] = [0.094, 0.106, 0.137, 1.0];
const SELECTED: [f32; 4] = [0.18, 0.27, 0.46, 1.0];
const TEXT: [f32; 4] = [0.93, 0.94, 0.96, 1.0];
const DIM: [f32; 4] = [0.58, 0.62, 0.70, 1.0];
const ACCENT: [f32; 4] = [0.40, 0.78, 0.55, 1.0];
const WARN: [f32; 4] = [0.95, 0.62, 0.40, 1.0];

/// The font atlas as an RGBA texture: white, with the coverage in alpha.
fn atlas_pixels() -> Vec<u8> {
    ALPHA.iter().flat_map(|&a| [255, 255, 255, a]).collect()
}

/// Draws text and rectangles into a list of quads.
struct Painter {
    quads: Vec<Quad>,
}

impl Painter {
    fn rect(&mut self, rect: [f32; 4], color: [f32; 4]) {
        self.quads.push(Quad { turn: false, spin: None, rect, uv: [-1.0, 0.0, -1.0, 0.0], color, texture: UI });
    }

    /// The width of `s` set at `size` pixels tall.
    fn width(s: &str, size: f32) -> f32 {
        let k = size / font::SIZE;
        s.chars().map(|c| glyph(c).map_or(0.0, |g| g.7 as f32 * k)).sum()
    }

    /// `s` with its top-left at (`x`, `y`), the em `size` pixels.
    fn text(&mut self, x: f32, y: f32, size: f32, color: [f32; 4], s: &str) {
        let k = size / font::SIZE;
        let mut pen = x;
        for c in s.chars() {
            let Some(&(_, gx, gy, w, h, xo, yo, adv)) = glyph(c) else { continue };
            if w > 0 && h > 0 {
                let left = pen + xo as f32 * k;
                let top = y + yo as f32 * k;
                let uv = [f32::from(gx) / font::WIDTH as f32, f32::from(gy) / font::HEIGHT as f32, f32::from(gx + w) / font::WIDTH as f32, f32::from(gy + h) / font::HEIGHT as f32];
                self.quads.push(Quad { turn: false, spin: None, rect: [left, top, left + f32::from(w) * k, top + f32::from(h) * k], uv, color, texture: FONT });
            }
            pen += adv as f32 * k;
        }
    }

    /// `s` cut to `max` pixels wide, with an ellipsis where it was; `from_start` keeps its end.
    fn fit(s: &str, size: f32, max: f32, from_start: bool) -> String {
        if Self::width(s, size) <= max {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        for keep in (1..chars.len()).rev() {
            let t: String = if from_start { std::iter::once('\u{2026}').chain(chars[chars.len() - keep..].iter().copied()).collect() } else { chars[..keep].iter().copied().chain(std::iter::once('\u{2026}')).collect() };
            if Self::width(&t, size) <= max {
                return t;
            }
        }
        "\u{2026}".into()
    }
}

fn glyph(c: char) -> Option<&'static (char, u16, u16, u16, u16, i32, i32, u32)> {
    font::GLYPHS.iter().find(|g| g.0 == c).or_else(|| font::GLYPHS.iter().find(|g| g.0 == '?'))
}

/// The screen's quads for `browser` in a window `width` × `height`.
fn layout(browser: &Browser, width: f32, height: f32) -> Vec<Quad> {
    let s = (height / 1080.0).max(0.55);
    let mut p = Painter { quads: Vec::new() };
    p.rect([0.0, 0.0, width, height], BACKGROUND);
    let margin = (width * 0.08).max(24.0 * s);
    let content = width - 2.0 * margin;
    let mut y = height * 0.07;
    p.text(margin, y, 54.0 * s, TEXT, "Locate game files");
    y += 78.0 * s;
    let names: Vec<&str> = browser.titles.iter().map(|t| t.name()).collect();
    let markers: Vec<&str> = browser.titles.iter().map(|t| t.marker()).collect();
    let intro = if names.len() == 1 { format!("Barrel did not find {} in Steam's libraries.", names[0]) } else { "Barrel did not find an Atelier game it plays in Steam's libraries.".to_string() };
    p.text(margin, y, 26.0 * s, DIM, &Painter::fit(&intro, 26.0 * s, content, false));
    y += 36.0 * s;
    let ask = format!("Open the folder that holds {} and press Enter on it.", markers.join(" or "));
    p.text(margin, y, 26.0 * s, DIM, &Painter::fit(&ask, 26.0 * s, content, false));
    y += 64.0 * s;
    // The folder being listed.
    p.text(margin, y, 28.0 * s, TEXT, &Painter::fit(&browser.dir.to_string_lossy(), 28.0 * s, content, true));
    y += 50.0 * s;
    // The list.
    let footer = 70.0 * s;
    let row_h = 46.0 * s;
    let panel = [margin, y, margin + content, height - footer - 16.0 * s];
    p.rect(panel, PANEL);
    let visible = (((panel[3] - panel[1]) - 16.0 * s) / row_h).floor().max(1.0) as usize;
    let first = browser.selected.saturating_sub(visible / 2).min(browser.rows.len().saturating_sub(visible));
    for (i, row) in browser.rows.iter().enumerate().skip(first).take(visible) {
        let top = panel[1] + 8.0 * s + (i - first) as f32 * row_h;
        if i == browser.selected {
            p.rect([panel[0] + 8.0 * s, top, panel[2] - 8.0 * s, top + row_h - 4.0 * s], SELECTED);
        }
        let text_y = top + 5.0 * s;
        match row {
            Row::Up => p.text(panel[0] + 24.0 * s, text_y, 28.0 * s, DIM, "..   Up one folder"),
            Row::Folder { name, game } => {
                let tag = "Game folder";
                let tag_w = if *game { Painter::width(tag, 24.0 * s) + 24.0 * s } else { 0.0 };
                let name = Painter::fit(name, 28.0 * s, content - 64.0 * s - tag_w, false);
                p.text(panel[0] + 24.0 * s, text_y, 28.0 * s, if *game { ACCENT } else { TEXT }, &name);
                if *game {
                    p.text(panel[2] - 24.0 * s - Painter::width(tag, 24.0 * s), text_y + 2.0 * s, 24.0 * s, ACCENT, tag);
                }
            }
        }
    }
    // A scroll bar when the list is longer than the panel.
    if browser.rows.len() > visible {
        let track = [panel[2] - 6.0 * s, panel[1] + 8.0 * s, panel[2] - 2.0 * s, panel[3] - 8.0 * s];
        let (h, n) = (track[3] - track[1], browser.rows.len() as f32);
        p.rect(track, [1.0, 1.0, 1.0, 0.06]);
        p.rect([track[0], track[1] + h * first as f32 / n, track[2], track[1] + h * (first + visible) as f32 / n], [1.0, 1.0, 1.0, 0.25]);
    }
    // The line below the list: a problem, else the keys.
    let fy = height - footer + 4.0 * s;
    if browser.message.is_empty() {
        p.text(margin, fy, 24.0 * s, DIM, &Painter::fit("Up / Down  move      Enter or A  open      Backspace or B  up a folder      Esc  quit", 24.0 * s, content, false));
    } else {
        p.text(margin, fy, 24.0 * s, WARN, &Painter::fit(&browser.message, 24.0 * s, content, false));
    }
    p.quads
}

/// The button a key stands for.
fn input(key: KeyCode) -> Option<Input> {
    Some(match key {
        KeyCode::ArrowUp => Input::Up,
        KeyCode::ArrowDown => Input::Down,
        KeyCode::PageUp => Input::PageUp,
        KeyCode::PageDown => Input::PageDown,
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::ArrowRight => Input::Confirm,
        KeyCode::Backspace | KeyCode::ArrowLeft => Input::Back,
        _ => return None,
    })
}

/// Runs the folder screen until a folder that holds one of `titles` is chosen: the folder, and the
/// window to carry on with. `None` when the window is closed or Escape pressed. `display` is
/// the window as the config has it; `pump` a window already open (there is one event loop to a
/// process).
pub fn run(titles: &[Title], pump: Option<Pump>, display: crate::settings::Display) -> Result<Option<(std::path::PathBuf, Pump)>> {
    let start = std::env::current_dir().ok().filter(|d| d.parent().is_some()).unwrap_or_else(barrel_files::browse::home);
    show(pump, display, Browser::for_games(titles.to_vec(), start), |b, w, h| layout(b, w, h), |browser, keys| {
        for key in keys {
            if key == KeyCode::Escape {
                return Some(None);
            }
            if let Some(b) = input(key)
                && let Some(dir) = browser.press(b)
            {
                return Some(Some(dir));
            }
        }
        None
    })
}

/// What the picker shows: the games found and the one the cursor is on.
#[derive(Clone, PartialEq)]
struct Pick {
    games: Vec<(Title, std::path::PathBuf)>,
    selected: usize,
}

/// The picker: which of the `games` found to start. `None` when the window is closed or Escape
/// pressed. Remembers nothing.
pub fn pick(games: &[(Title, std::path::PathBuf)], pump: Option<Pump>, display: crate::settings::Display) -> Result<Option<(usize, Pump)>> {
    show(pump, display, Pick { games: games.to_vec(), selected: 0 }, pick_layout, |p, keys| {
        let last = p.games.len() - 1;
        for key in keys {
            match key {
                KeyCode::Escape => return Some(None),
                KeyCode::ArrowUp => p.selected = p.selected.checked_sub(1).unwrap_or(last),
                KeyCode::ArrowDown => p.selected = if p.selected >= last { 0 } else { p.selected + 1 },
                KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::ArrowRight => return Some(Some(p.selected)),
                _ => {}
            }
        }
        None
    })
}

/// The picker's quads in a window `width` x `height`: one row per game, in the folder screen's style.
fn pick_layout(pick: &Pick, width: f32, height: f32) -> Vec<Quad> {
    let s = (height / 1080.0).max(0.55);
    let mut p = Painter { quads: Vec::new() };
    p.rect([0.0, 0.0, width, height], BACKGROUND);
    let margin = (width * 0.08).max(24.0 * s);
    let content = width - 2.0 * margin;
    let mut y = height * 0.07;
    p.text(margin, y, 54.0 * s, TEXT, "Choose a game");
    y += 78.0 * s;
    p.text(margin, y, 26.0 * s, DIM, &Painter::fit("Barrel found more than one game. Pick the one to play.", 26.0 * s, content, false));
    y += 80.0 * s;
    let row_h = 96.0 * s;
    let panel = [margin, y, margin + content, y + 16.0 * s + row_h * pick.games.len() as f32];
    p.rect(panel, PANEL);
    for (i, (title, dir)) in pick.games.iter().enumerate() {
        let top = panel[1] + 8.0 * s + i as f32 * row_h;
        if i == pick.selected {
            p.rect([panel[0] + 8.0 * s, top, panel[2] - 8.0 * s, top + row_h - 8.0 * s], SELECTED);
        }
        p.text(panel[0] + 24.0 * s, top + 8.0 * s, 34.0 * s, TEXT, &Painter::fit(title.name(), 34.0 * s, content - 48.0 * s, false));
        p.text(panel[0] + 24.0 * s, top + 52.0 * s, 24.0 * s, DIM, &Painter::fit(&dir.to_string_lossy(), 24.0 * s, content - 48.0 * s, true));
    }
    p.text(margin, height - 70.0 * s + 4.0 * s, 24.0 * s, DIM, &Painter::fit("Up / Down  move      Enter or A  start      Esc  quit", 24.0 * s, content, false));
    p.quads
}

/// Draws a screen of Barrel's own in a window until `step` ends it: `state` is what the screen
/// shows (laid out again whenever it or the window changes), `step` takes the keys pressed since
/// the last frame and gives `Some(answer)` to finish (`Some(None)` when the user left).
fn show<S: Clone + PartialEq, T>(
    pump: Option<Pump>,
    display: crate::settings::Display,
    mut state: S,
    layout: impl Fn(&S, f32, f32) -> Vec<Quad>,
    mut step: impl FnMut(&mut S, Vec<KeyCode>) -> Option<Option<T>>,
) -> Result<Option<(T, Pump)>> {
    let mut pump = match pump {
        Some(p) => p,
        None => Pump::open(display)?,
    };
    let handle = raw_window_handle::HasDisplayHandle::display_handle(pump.window())?.as_raw();
    let gpu = Gpu::new(Some(handle))?;
    let window = raw_window_handle::HasWindowHandle::window_handle(pump.window())?.as_raw();
    let size = pump.window().inner_size();
    let mut presenter = crate::present::Presenter::new(&gpu, handle, window, size.width, size.height)?;
    let d = &gpu.device;
    let pool = unsafe { d.create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(gpu.queue_family), None)? };
    let attachment = [vk::AttachmentDescription::default()
        .format(presenter.format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
    let color_ref = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
    let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&color_ref)];
    // As the renderer's post pass: the acquire semaphore holds the colour output stage.
    let dependency = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
    let pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&dependency), None)? };
    let framebuffers = |p: &crate::present::Presenter| -> Result<Vec<vk::Framebuffer>> {
        p.views
            .iter()
            .map(|&v| {
                let views = [v];
                unsafe { d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(p.extent.width).height(p.extent.height).layers(1), None) }.map_err(Into::into)
            })
            .collect()
    };
    let mut buffers = framebuffers(&presenter)?;
    let atlas = atlas_pixels();
    let font_texture = Texture { width: font::WIDTH, height: font::HEIGHT, format: Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&atlas[..]] };
    let white = [255u8; 4];
    let ui_texture = Texture { width: 1, height: 1, format: Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&white[..]] };
    let mut layer = Layer::new(&gpu, pool, pass, &[&font_texture, &ui_texture])?;
    let (ready, done) = unsafe { (d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?, d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?) };

    let mut shown: Option<(S, (u32, u32))> = None;
    let chosen = loop {
        if !pump.pump() {
            break None;
        }
        if let Some(f) = step(&mut state, pump.take_pressed()) {
            break f;
        }
        if pump.take_resized() {
            let s = pump.window().inner_size();
            if s.width > 0 && s.height > 0 {
                presenter.rebuild(&gpu, s.width, s.height)?;
                unsafe { d.device_wait_idle()? };
                for f in buffers.drain(..) {
                    unsafe { d.destroy_framebuffer(f, None) };
                }
                buffers = framebuffers(&presenter)?;
            }
        }
        let extent = (presenter.extent.width, presenter.extent.height);
        if shown.as_ref().map(|(b, e)| (b, *e)) != Some((&state, extent)) {
            layer.set(&gpu, &layout(&state, extent.0 as f32, extent.1 as f32))?;
            shown = Some((state.clone(), extent));
        }
        let Some(image) = presenter.acquire(ready)? else {
            let s = pump.window().inner_size();
            if s.width > 0 && s.height > 0 {
                presenter.rebuild(&gpu, s.width, s.height)?;
                unsafe { d.device_wait_idle()? };
                for f in buffers.drain(..) {
                    unsafe { d.destroy_framebuffer(f, None) };
                }
                buffers = framebuffers(&presenter)?;
            }
            continue;
        };
        unsafe {
            let cb = d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1))?[0];
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: presenter.extent };
            let clear = [vk::ClearValue { color: vk::ClearColorValue { float32: BACKGROUND } }];
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(buffers[image as usize]).render_area(area).clear_values(&clear), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: extent.0 as f32, height: extent.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            layer.record(d, cb, extent.0, extent.1);
            d.cmd_end_render_pass(cb);
            d.end_command_buffer(cb)?;
            let fence = d.create_fence(&vk::FenceCreateInfo::default(), None)?;
            let (waits, stages, cbs, signals) = ([ready], [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT], [cb], [done]);
            d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)], fence)?;
            let stale = presenter.present(&gpu, image, done)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
            d.destroy_fence(fence, None);
            d.free_command_buffers(pool, &cbs);
            gpu.frame.set(gpu.frame.get() + 1);
            if stale {
                shown = None;
            }
        }
    };
    unsafe {
        d.device_wait_idle()?;
        d.destroy_semaphore(ready, None);
        d.destroy_semaphore(done, None);
        for f in buffers {
            d.destroy_framebuffer(f, None);
        }
    }
    layer.destroy(&gpu);
    unsafe {
        d.destroy_render_pass(pass, None);
        d.destroy_command_pool(pool, None);
    }
    presenter.destroy(&gpu);
    Ok(chosen.map(|answer| (answer, pump)))
}

/// The game found and the window it may have opened to ask.
pub struct Resolved {
    pub root: std::path::PathBuf,
    pub pump: Option<Pump>,
}

/// Removes `flag VALUE` from `args` and gives the value.
pub fn take_option(args: &mut Vec<String>, flag: &str) -> Option<String> {
    barrel_files::detect::take_option(args, flag)
}

/// Steam's launch option `barrel-game %command%` gives the game's own program (`...\\Atelier_X.exe`)
/// as the first argument: removes it and gives its folder, the install.
pub fn take_game_program(args: &mut Vec<String>) -> Option<std::path::PathBuf> {
    let first = args.first().filter(|a| a.to_ascii_lowercase().ends_with(".exe"))?;
    // A Windows path on any host: both slashes end a folder name.
    let folder = first.rsplit_once(['/', '\\']).map(|(dir, _)| std::path::PathBuf::from(dir)).filter(|d| !d.as_os_str().is_empty());
    args.remove(0);
    folder
}

/// The window as the graphics config has it (else the launcher's default, else 1280x720).
pub fn config_display() -> crate::settings::Display {
    let text = barrel_files::config::graphics_path().and_then(|p| std::fs::read_to_string(p).ok());
    text.as_deref().map_or_else(
        || crate::settings::Display { fullscreen: std::env::var_os("BARREL_DEFAULT_FULLSCREEN").is_some_and(|v| v == "1"), ..crate::settings::Display::DEFAULT },
        crate::settings::Display::from_config,
    )
}

/// Names `title` as the game of this process (its config and data folders) and sets up the
/// layered files (`barrel_files::install`) over `root` and the game's enabled mods.
pub fn start(title: Title, root: std::path::PathBuf, pump: Option<Pump>) -> Resolved {
    barrel_files::config::use_game(title);
    eprintln!("game: {}", root.display());
    barrel_files::install(barrel_files::mods::game_fs(&root));
    Resolved { root, pump }
}

/// Saves the folder the user located for `title`.
pub fn remember(title: Title, dir: &std::path::Path) {
    match barrel_files::config::save_game(title.key(), dir) {
        Ok(file) => eprintln!("game folder saved to {}", file.display()),
        Err(e) => eprintln!("game folder not saved: {e}"),
    }
}

/// Finds `title`'s install and starts it ([`start`]). In order: `given` (`--game`, which must
/// hold the game); `from_args`, the install the command line's own paths are in;
/// `BARREL_GAME_DIR`; the folder saved in the config; Steam's libraries; else the "Locate game
/// files" screen, whose answer is saved. `None` when that screen was closed.
pub fn resolve(title: Title, given: Option<std::path::PathBuf>, from_args: Option<std::path::PathBuf>) -> Result<Option<Resolved>> {
    let mut pump = None;
    let root = if let Some(g) = given {
        barrel_files::find::find(title, Some(g))?.map(|f| f.path().to_path_buf())
    } else if let Some(r) = from_args {
        Some(r)
    } else {
        match barrel_files::find::find(title, None)? {
            Some(found) => Some(found.path().to_path_buf()),
            None => match run(&[title], None, config_display())? {
                Some((dir, p)) => {
                    remember(title, &dir);
                    pump = Some(p);
                    Some(dir)
                }
                None => None,
            },
        }
    };
    Ok(root.map(|root| start(title, root, pump)))
}

/// The screen's quads as a picture, the way the GPU draws them: rectangles filled with their
/// colour, glyphs as the atlas's coverage in alpha (bilinear), blended over what is below.
/// RGBA, `width` x `height`. For the draw check (`BARREL_LOCATE_SHOT`), which needs no GPU.
fn rasterize(quads: &[Quad], width: u32, height: u32) -> Vec<u8> {
    let mut img = vec![[0.0f32; 3]; (width * height) as usize];
    let coverage = |u: f32, v: f32| -> f32 {
        let (x, y) = (u * font::WIDTH as f32 - 0.5, v * font::HEIGHT as f32 - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |px: f32, py: f32| -> f32 {
            let (px, py) = (px.clamp(0.0, font::WIDTH as f32 - 1.0) as usize, py.clamp(0.0, font::HEIGHT as f32 - 1.0) as usize);
            f32::from(ALPHA[py * font::WIDTH as usize + px]) / 255.0
        };
        (at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx) * (1.0 - fy) + (at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx) * fy
    };
    for q in quads {
        let [l, t, r, b] = q.rect;
        for y in (t.floor().max(0.0) as u32)..(b.ceil().min(height as f32) as u32) {
            for x in (l.floor().max(0.0) as u32)..(r.ceil().min(width as f32) as u32) {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                if cx < l || cx > r || cy < t || cy > b {
                    continue;
                }
                let mut alpha = q.color[3];
                if q.uv[0] >= 0.0 && q.texture == FONT {
                    let u = q.uv[0] + (q.uv[2] - q.uv[0]) * (cx - l) / (r - l);
                    let v = q.uv[1] + (q.uv[3] - q.uv[1]) * (cy - t) / (b - t);
                    alpha *= coverage(u, v);
                }
                let px = &mut img[(y * width + x) as usize];
                for c in 0..3 {
                    px[c] += (q.color[c] - px[c]) * alpha;
                }
            }
        }
    }
    img.iter().flat_map(|p| [(p[0] * 255.0 + 0.5) as u8, (p[1] * 255.0 + 0.5) as u8, (p[2] * 255.0 + 0.5) as u8, 255]).collect()
}

/// The draw check: writes the folder screen to `out` and the picker to `out` with `-pick` added
/// to its name, 1920x1080, from the folders of `start` and two sample games.
pub fn shot(out: &std::path::Path, start: std::path::PathBuf) -> std::io::Result<()> {
    let (w, h) = (1920, 1080);
    let browser = Browser::for_games(Title::ALL.to_vec(), start);
    let games: Vec<_> = Title::ALL.iter().map(|t| (*t, std::path::Path::new("/home/player/.local/share/Steam/steamapps/common").join(t.folder()))).collect();
    let pick = Pick { games, selected: 1 };
    let name = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let pick_out = out.with_file_name(format!("{name}-pick.png"));
    std::fs::write(out, barrel_formats::png::encode_rgba(w, h, &rasterize(&layout(&browser, w as f32, h as f32), w, h)))?;
    std::fs::write(pick_out, barrel_formats::png::encode_rgba(w, h, &rasterize(&pick_layout(&pick, w as f32, h as f32), w, h)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_programs_folder_is_the_install() {
        let mut a: Vec<String> = ["/g/Atelier Escha and Logy DX/Atelier_Escha_and_Logy.exe", "-x"].map(String::from).to_vec();
        assert_eq!(take_game_program(&mut a), Some("/g/Atelier Escha and Logy DX".into()));
        assert_eq!(a, ["-x"]);
        let mut b: Vec<String> = ["--field".to_string()].to_vec();
        assert_eq!(take_game_program(&mut b), None);
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn an_option_is_taken_out_with_its_value() {
        let mut a: Vec<String> = ["x", "--game", "/g", "--window", "0"].map(String::from).to_vec();
        assert_eq!(take_option(&mut a, "--game").as_deref(), Some("/g"));
        assert_eq!(a, ["x", "--window", "0"]);
        assert_eq!(take_option(&mut a, "--game"), None);
    }

    #[test]
    fn every_ascii_character_has_a_glyph() {
        assert!(font::GLYPHS.len() > 95);
        for c in ' '..='~' {
            assert!(font::GLYPHS.iter().any(|g| g.0 == c), "{c:?}");
        }
        assert_eq!(ALPHA.len(), (font::WIDTH * font::HEIGHT) as usize);
        // Every glyph box lies inside the atlas.
        assert!(font::GLYPHS.iter().all(|g| u32::from(g.1 + g.3) <= font::WIDTH && u32::from(g.2 + g.4) <= font::HEIGHT));
    }

    #[test]
    fn long_texts_are_cut_to_fit_with_an_ellipsis() {
        let s = "/a/very/long/folder/name/that/does/not/fit";
        let t = Painter::fit(s, 28.0, 200.0, true);
        assert!(t.starts_with('\u{2026}') && t.ends_with("fit") && Painter::width(&t, 28.0) <= 200.0);
        let t = Painter::fit(s, 28.0, 200.0, false);
        assert!(t.ends_with('\u{2026}') && Painter::width(&t, 28.0) <= 200.0);
        assert_eq!(Painter::fit("short", 28.0, 200.0, false), "short");
    }

    #[test]
    fn the_layout_shows_the_folders_and_fits_the_window() {
        let dir = std::env::temp_dir();
        let b = Browser::new(Title::Escha, dir);
        for (w, h) in [(1920.0, 1080.0), (1280.0, 720.0), (800.0, 480.0)] {
            let quads = layout(&b, w, h);
            assert!(quads.len() > 20);
            assert!(quads.iter().all(|q| q.rect[0] >= -1.0 && q.rect[2] <= w + 1.0 && q.rect[3] <= h + 1.0), "{w}x{h}");
        }
    }
}
