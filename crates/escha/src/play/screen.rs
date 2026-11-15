//! A 2D screen's frame loop (the title, the hero choice, the save list, the in-play menu):
//! a text layer of the screen's pictures, redrawn whenever its state changes, until a button
//! ends it. Offscreen it takes the given buttons, draws once and writes the picture.

use std::path::Path;

use ash::vk;

use crate::title::{button, Button};
use crate::{text, Gpu, Result};

/// Where a screen draws: the post pass and its framebuffers (the window's images, or the one
/// offscreen image), and the screen's size.
pub struct Target<'a> {
    pub pass: vk::RenderPass,
    pub framebuffers: &'a [vk::Framebuffer],
    pub width: u32,
    pub height: u32,
}

/// Runs screen `state` (its pictures in `images`, drawn by `quads`, moved by `press`) until
/// `press` returns an outcome or the window closes (`None`). Offscreen: presses `keys`, draws
/// once, writes the picture to `out` and returns the outcome the keys gave, if any.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn run<S: Clone + PartialEq, O>(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    images: &[&barrel_formats::g1t::Texture],
    state: &mut S,
    quads: impl Fn(&S) -> Vec<text::Quad>,
    press: impl FnMut(&mut S, Button) -> Option<O>,
    keys: &[Button],
    out: Option<&Path>,
) -> Result<Option<O>> {
    run_with(gpu, pool, target, window, offscreen, images, state, quads, press, keys, out, |_| None)
}

/// How many frames an offscreen clocked screen ([`run_clocked`]) plays (`--frames N`).
static OFFSCREEN_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Sets the frames an offscreen clocked screen plays; the last is written to the picture and each
/// one whose drawing changed reaches the draw trace, at the renderer's frame count.
pub fn set_offscreen_frames(n: u32) {
    OFFSCREEN_FRAMES.store(n.max(1), std::sync::atomic::Ordering::Relaxed);
}

/// The tick an offscreen clocked screen starts at (`BARREL_CLOCK_AT`, default 0): the game's
/// flipbooks all run from one frame counter that was 204 ahead of the capture's frame number
/// (`docs/escha/parity.md`, "Time alignment"), so a check against game frame F starts Barrel's
/// clock at F + 204.
fn clock_start() -> u32 {
    std::env::var("BARREL_CLOCK_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// The 60 Hz tick the game's clocks count (every animation's rates are in these ticks).
pub const TICKS_PER_SECOND: f64 = 60.0;

/// [`run`], with `tick` called once a frame in a window (the title's music, the splashes'
/// clock): it may move the state, or end the screen with an outcome.
#[allow(clippy::too_many_arguments)]
pub fn run_with<S: Clone + PartialEq, O>(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    images: &[&barrel_formats::g1t::Texture],
    state: &mut S,
    quads: impl Fn(&S) -> Vec<text::Quad>,
    press: impl FnMut(&mut S, Button) -> Option<O>,
    keys: &[Button],
    out: Option<&Path>,
    mut tick: impl FnMut(&mut S) -> Option<O>,
) -> Result<Option<O>> {
    run_inner(gpu, pool, target, window, offscreen, images, state, quads, press, keys, out, |s, _| tick(s), false)
}

/// [`run_with`] for a screen with a clock: `tick` gets the 60 Hz tick count since the screen
/// opened (in a window the elapsed time in ticks, offscreen the frame number, the run's
/// `--frames`: each frame the picture changes in reaches the draw trace) and may move the state.
/// The picture is drawn again whenever `quads` gives something new.
#[allow(clippy::too_many_arguments)]
pub fn run_clocked<S: Clone + PartialEq, O>(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    images: &[&barrel_formats::g1t::Texture],
    state: &mut S,
    quads: impl Fn(&S) -> Vec<text::Quad>,
    press: impl FnMut(&mut S, Button) -> Option<O>,
    keys: &[Button],
    out: Option<&Path>,
    tick: impl FnMut(&mut S, u32) -> Option<O>,
) -> Result<Option<O>> {
    run_inner(gpu, pool, target, window, offscreen, images, state, quads, press, keys, out, tick, true)
}

#[allow(clippy::too_many_arguments)]
fn run_inner<S: Clone + PartialEq, O>(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &Target,
    window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>,
    offscreen: Option<vk::Image>,
    images: &[&barrel_formats::g1t::Texture],
    state: &mut S,
    quads: impl Fn(&S) -> Vec<text::Quad>,
    mut press: impl FnMut(&mut S, Button) -> Option<O>,
    keys: &[Button],
    out: Option<&Path>,
    mut tick: impl FnMut(&mut S, u32) -> Option<O>,
    clocked: bool,
) -> Result<Option<O>> {
    let d = &gpu.device;
    let mut layer = text::Layer::new(gpu, pool, target.pass, images)?;
    let (w, h) = (target.width as f32, target.height as f32);
    let mut picked = None;
    for &b in keys {
        if picked.is_none() {
            picked = press(state, b);
        }
    }
    // An offscreen clock sets the picture at each of its frames, the first included.
    let frames = if clocked && window.is_none() { OFFSCREEN_FRAMES.load(std::sync::atomic::Ordering::Relaxed) } else { 1 };
    let clock_offscreen = clocked && window.is_none();
    if !clock_offscreen {
        layer.set(gpu, &quads(state))?;
    }
    let draw = |cb: vk::CommandBuffer, layer: &text::Layer, image: usize| unsafe {
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: target.width, height: target.height } };
        d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(target.pass).framebuffer(target.framebuffers[image]).render_area(area), vk::SubpassContents::INLINE);
        d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w, height: h, min_depth: 0.0, max_depth: 1.0 }]);
        d.cmd_set_scissor(cb, 0, &[area]);
        // The pass leaves the image's old contents undefined (the scene's post pass covers
        // every pixel); a screen whose pieces leave gaps (the Load Game list) shows black there,
        // not whatever the memory held.
        d.cmd_clear_attachments(
            cb,
            &[vk::ClearAttachment { aspect_mask: vk::ImageAspectFlags::COLOR, color_attachment: 0, clear_value: vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] } } }],
            &[vk::ClearRect { rect: area, base_array_layer: 0, layer_count: 1 }],
        );
        layer.record(d, cb, target.width, target.height);
        d.cmd_end_render_pass(cb);
    };
    let result = match (window, offscreen) {
        (Some((presenter, pump)), _) => {
            let (ready, done) = unsafe { (d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?, d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?) };
            let mut shown: Option<S> = None;
            let mut shown_quads: Option<Vec<text::Quad>> = None;
            let started = std::time::Instant::now();
            // Keys given beforehand may already have picked (a scripted windowed run).
            let picked = if picked.is_some() { picked } else { loop {
                if !pump.pump() {
                    break None;
                }
                let mut chosen = tick(state, (started.elapsed().as_secs_f64() * TICKS_PER_SECOND) as u32);
                for key in pump.take_pressed() {
                    if let Some(b) = button(key)
                        && chosen.is_none() {
                            chosen = press(state, b);
                        }
                }
                if chosen.is_some() {
                    break chosen;
                }
                if clocked {
                    let q = quads(state);
                    if shown_quads.as_ref() != Some(&q) {
                        layer.set(gpu, &q)?;
                        shown_quads = Some(q);
                    }
                } else if shown.as_ref() != Some(&*state) {
                    layer.set(gpu, &quads(state))?;
                    shown = Some(state.clone());
                }
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
            } };
            unsafe {
                d.device_wait_idle()?;
                d.destroy_semaphore(ready, None);
                d.destroy_semaphore(done, None);
            }
            picked
        }
        (None, Some(color)) => {
            // The clock's frames before the last: the picture set (and traced) where it changes.
            if clock_offscreen {
                let mut shown: Option<Vec<text::Quad>> = None;
                let start = clock_start();
                for f in 0..frames - 1 {
                    if picked.is_none() {
                        picked = tick(state, start + f);
                    }
                    let q = quads(state);
                    if shown.as_ref() != Some(&q) {
                        layer.set(gpu, &q)?;
                        gpu.submit(pool, |cb| unsafe { draw(cb, &layer, 0) })?;
                        shown = Some(q);
                    }
                    gpu.frame.set(gpu.frame.get() + 1);
                }
                if picked.is_none() {
                    picked = tick(state, start + frames - 1);
                }
                let q = quads(state);
                if shown.as_ref() != Some(&q) {
                    layer.set(gpu, &q)?;
                }
            }
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

/// The (pack, picture) keys of the sprites among `elements`, each once.
pub fn sheet_keys(pe: &barrel_formats::pe::Pe, elements: impl IntoIterator<Item = crate::hud::Element>) -> Vec<(String, u32)> {
    let mut keys: Vec<(String, u32)> = Vec::new();
    for e in elements {
        if let crate::hud::Element::Picture { pack, index, .. } | crate::hud::Element::Slice { pack, index, .. } | crate::hud::Element::InkedSlice { pack, index, .. } = &e {
            let key = (pack.clone(), *index);
            if !keys.contains(&key) {
                keys.push(key);
            }
            continue;
        }
        if let Some((part, cut, at)) = e.sprite()
            && let Some(p) = barrel_escha::data::layout::sprite(pe, part, cut, at) {
                let key = (p.texture, p.index);
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
    }
    keys
}

/// The bytes of each interface pack `keys` name, once each (English first).
pub fn read_packs(root: &Path, keys: &[(String, u32)]) -> Result<Vec<(String, Vec<u8>)>> {
    let x64 = root.join("Data/x64");
    let mut packs: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, _) in keys {
        if !packs.iter().any(|(n, _)| n == name) {
            let bytes = std::fs::read(x64.join("WinXls_EN").join(name)).or_else(|_| std::fs::read(x64.join("WinXls").join(name)))?;
            packs.push((name.clone(), bytes));
        }
    }
    Ok(packs)
}
