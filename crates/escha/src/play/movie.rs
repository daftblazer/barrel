//! A movie screen: a `.wmv` file played full screen with its sound (`barrel_movie::player`),
//! the picture following the sound's clock, until it ends or a button skips it.

use std::path::Path;
use std::sync::Arc;

use ash::vk;
use barrel_render::movie::MovieLayer;

use crate::screen::Target;
use crate::{Gpu, Result};

/// The picture's place on the target: scaled to fit and centred (black bars elsewhere).
fn fit(target: &Target, width: usize, height: usize) -> [f32; 4] {
    let (tw, th) = (target.width as f32, target.height as f32);
    let k = (tw / width as f32).min(th / height as f32);
    let (w, h) = (width as f32 * k, height as f32 * k);
    [(tw - w) / 2.0, (th - h) / 2.0, (tw + w) / 2.0, (th + h) / 2.0]
}

/// Plays `file`. In a window: in real time, sound and all, until the end or a button. Offscreen:
/// the picture at `shown_at` seconds, written to `out`.
pub fn play(gpu: &Gpu, pool: vk::CommandPool, target: &Target, window: Option<(&crate::present::Presenter, &mut crate::window::Pump)>, offscreen: Option<vk::Image>, file: &Path, shown_at: f64, out: Option<&Path>) -> Result<()> {
    let t0 = std::time::Instant::now();
    let mut movie = barrel_movie::player::Movie::open(std::fs::read(file)?)?;
    eprintln!("movie: {} {}x{}, {:.1} s, opened in {:.2} s", file.display(), movie.width, movie.height, movie.duration, t0.elapsed().as_secs_f64());
    let d = &gpu.device;
    let mut layer = MovieLayer::new(gpu, pool, target.pass, movie.width as u32, movie.height as u32)?;
    let rect = fit(target, movie.width, movie.height);
    let screen = [target.width as f32, target.height as f32];
    let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: target.width, height: target.height } };
    let draw = |cb: vk::CommandBuffer, layer: &mut MovieLayer, image: usize| unsafe {
        layer.upload(d, cb);
        d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(target.pass).framebuffer(target.framebuffers[image]).render_area(area), vk::SubpassContents::INLINE);
        d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: screen[0], height: screen[1], min_depth: 0.0, max_depth: 1.0 }]);
        d.cmd_set_scissor(cb, 0, &[area]);
        d.cmd_clear_attachments(
            cb,
            &[vk::ClearAttachment { aspect_mask: vk::ImageAspectFlags::COLOR, color_attachment: 0, clear_value: vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] } } }],
            &[vk::ClearRect { rect: area, base_array_layer: 0, layer_count: 1 }],
        );
        layer.record(d, cb, screen, rect, 1.0);
        d.cmd_end_render_pass(cb);
    };
    match (window, offscreen) {
        (Some((presenter, pump)), _) => {
            // The sound through its own output; the clock is what the device has played.
            let mut sound = movie.audio.take().and_then(|a| {
                let out = barrel_audio::Output::open(0.1).map_err(|e| eprintln!("movie: no sound: {e}")).ok()?;
                let mut mixer = barrel_audio::Mixer::new();
                mixer.play(Arc::new(barrel_audio::Sound { rate: a.rate, channels: a.channels }), 1.0, None);
                Some((out, mixer, 0usize))
            });
            let start = std::time::Instant::now();
            let (ready, done) = unsafe { (d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?, d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?) };
            let mut shown = -1.0f64;
            loop {
                if !pump.pump() {
                    break;
                }
                if pump.take_pressed().into_iter().any(|k| crate::title::button(k).is_some()) {
                    eprintln!("movie: skipped at {:.2} s", start.elapsed().as_secs_f64());
                    break;
                }
                let clock = match sound.as_mut() {
                    Some((out, mixer, fed)) => {
                        *fed += out.pump(mixer);
                        fed.saturating_sub(out.queued()) as f64 / f64::from(barrel_audio::RATE)
                    }
                    None => start.elapsed().as_secs_f64(),
                };
                if movie.ended(clock) {
                    break;
                }
                if let Some(f) = movie.frame_at(clock, false)
                    && f.time != shown {
                        layer.set(&f.y, &f.u, &f.v);
                        shown = f.time;
                    }
                let Some(image) = presenter.acquire(ready)? else { continue };
                unsafe {
                    let cb = d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1))?[0];
                    d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
                    draw(cb, &mut layer, image as usize);
                    d.end_command_buffer(cb)?;
                    let fence = d.create_fence(&vk::FenceCreateInfo::default(), None)?;
                    let (waits, stages, cbs, signals) = ([ready], [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT], [cb], [done]);
                    d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)], fence)?;
                    presenter.present(gpu, image, done)?;
                    d.wait_for_fences(&[fence], true, u64::MAX)?;
                    d.destroy_fence(fence, None);
                    d.free_command_buffers(pool, &cbs);
                }
            }
            eprintln!("movie: ended at {:.2} s, {} pictures dropped{}", start.elapsed().as_secs_f64(), movie.dropped, movie.error.as_ref().map(|e| format!(", error {e}")).unwrap_or_default());
            unsafe {
                d.device_wait_idle()?;
                d.destroy_semaphore(ready, None);
                d.destroy_semaphore(done, None);
            }
        }
        (None, Some(color)) => {
            if let Some(f) = movie.frame_at(shown_at, true) {
                layer.set(&f.y, &f.u, &f.v);
            }
            let bytes = u64::from(target.width) * u64::from(target.height) * 4;
            let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
            gpu.submit(pool, |cb| unsafe {
                draw(cb, &mut layer, 0);
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
        }
        _ => {}
    }
    unsafe { d.device_wait_idle()? };
    layer.destroy(gpu);
    Ok(())
}
