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

/// A key's title button.
pub fn button(key: winit::keyboard::KeyCode) -> Option<Button> {
    use winit::keyboard::KeyCode as K;
    match key {
        K::ArrowUp | K::KeyW => Some(Button::Up),
        K::ArrowDown | K::KeyS => Some(Button::Down),
        K::Enter | K::Space | K::KeyZ => Some(Button::Confirm),
        K::Escape | K::Backspace | K::KeyX => Some(Button::Back),
        _ => None,
    }
}

/// Where the title draws: the post pass and its framebuffers (the window's images, or the one
/// offscreen image), and the screen's size.
pub struct Target<'a> {
    pub pass: vk::RenderPass,
    pub framebuffers: &'a [vk::Framebuffer],
    pub width: u32,
    pub height: u32,
}

/// Shows the title screen and returns the entry picked (`None`: the window was closed, or,
/// offscreen, no entry was picked by `keys`). In a window it runs until an entry it can go on
/// with (New Game, Exit Game) is picked; offscreen it takes `keys`, draws the screen once and
/// writes it to `out`.
pub fn run(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<Entry>> {
    let d = &gpu.device;
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let still = barrel_formats::layout::still(&pe, BACKGROUND).ok_or("title: no background picture")?;
    let x64 = root.join("Data/x64");
    let still_bytes = std::fs::read(x64.join(format!("{}_EN", still.folder)).join(&still.file)).or_else(|_| std::fs::read(x64.join(&still.folder).join(&still.file)))?;
    let still_g1t = barrel_formats::g1t::G1t::parse(&still_bytes)?;
    // The sprites' pictures, each pack read once.
    let mut keys_needed: Vec<(String, u32)> = Vec::new();
    for e in all_elements() {
        if let Element::Sprite { part, cut, at } = e
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
    let mut images = vec![still_g1t.textures.first().ok_or("title: an empty background")?];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
    for (name, index) in keys_needed {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    let mut layer = text::Layer::new(gpu, pool, target.pass, &images)?;
    let (w, h) = (target.width as f32, target.height as f32);
    let quads = |t: &Title| {
        // The background over the 1920 × 1080 screen, scaled to the height and centred.
        let k = h / 1080.0;
        let dx = (w - 1920.0 * k) / 2.0;
        let mut q = vec![text::Quad { rect: [dx, 0.0, dx + still.size[0] * k, still.size[1] * k], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], texture: 0 }];
        q.extend(hud::quads(&pe, None, &sheets, &t.elements(), w, h));
        q
    };
    let mut title = Title::new();
    let mut picked = None;
    for &b in keys {
        picked = title.press(b).or(picked);
    }
    layer.set(gpu, &quads(&title))?;
    let draw = |cb: vk::CommandBuffer, layer: &text::Layer, image: usize| unsafe {
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: target.width, height: target.height } };
        d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(target.pass).framebuffer(target.framebuffers[image]).render_area(area), vk::SubpassContents::INLINE);
        d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w, height: h, min_depth: 0.0, max_depth: 1.0 }]);
        d.cmd_set_scissor(cb, 0, &[area]);
        layer.record(d, cb, target.width, target.height);
        d.cmd_end_render_pass(cb);
    };
    let result = match (window, offscreen) {
        (Some((presenter, pump)), _) => {
            let (ready, done) = unsafe { (d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?, d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?) };
            let picked = loop {
                if !pump.pump() {
                    break None;
                }
                let mut chosen = None;
                for key in pump.take_pressed() {
                    if let Some(e) = button(key).and_then(|b| title.press(b)) {
                        match e {
                            Entry::NewGame | Entry::ExitGame => chosen = Some(e),
                            other => eprintln!("title: {other:?} is not there yet"),
                        }
                    }
                }
                if chosen.is_some() {
                    break chosen;
                }
                layer.set(gpu, &quads(&title))?;
                let Some(image) = presenter.acquire(ready)? else { continue };
                unsafe {
                    let cb = d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1))?[0];
                    d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
                    draw(cb, &layer, image as usize);
                    d.end_command_buffer(cb)?;
                    let fence = d.create_fence(&vk::FenceCreateInfo::default(), None)?;
                    let (waits, stages, cbs, signals) = ([ready], [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT], [cb], [done]);
                    d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)], fence)?;
                    presenter.present(gpu, image, done)?;
                    d.wait_for_fences(&[fence], true, u64::MAX)?;
                    d.destroy_fence(fence, None);
                    d.free_command_buffers(pool, &cbs);
                }
            };
            unsafe {
                d.device_wait_idle()?;
                d.destroy_semaphore(ready, None);
                d.destroy_semaphore(done, None);
            }
            picked
        }
        (None, Some(color)) => {
            let bytes = u64::from(target.width) * u64::from(target.height) * 4;
            let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
            gpu.submit(pool, |cb| unsafe {
                draw(cb, &layer, 0);
                let region = vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(vk::Extent3D { width: target.width, height: target.height, depth: 1 });
                d.cmd_copy_image_to_buffer(cb, color, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, readback, &[region]);
            })?;
            let mut pixels = vec![0u8; bytes as usize];
            unsafe {
                let p = d.map_memory(rmem, 0, bytes, vk::MemoryMapFlags::empty())? as *const u8;
                std::ptr::copy_nonoverlapping(p, pixels.as_mut_ptr(), pixels.len());
                d.unmap_memory(rmem);
                d.destroy_buffer(readback, None);
                d.free_memory(rmem, None);
            }
            if let Some(out) = out {
                std::fs::write(out, barrel_formats::png::encode_rgba(target.width, target.height, &pixels))?;
                eprintln!("wrote {}", out.display());
            }
            picked
        }
        _ => None,
    };
    unsafe { d.device_wait_idle()? };
    layer.destroy(gpu);
    Ok(result)
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
