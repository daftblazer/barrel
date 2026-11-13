//! The live window: a winit event loop presenting frames at the display's rate through
//! [`crate::present::Presenter`]. For now it clears each frame to a colour that cycles with
//! the engine clock, a bounded smoke test of the window, swapchain and clock; the event's scene
//! comes next (`docs/renderer.md`, "Live window").

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::present::Presenter;
use crate::{Gpu, Result};

/// Opens a 1280×720 window, presents `frames` frames and exits.
pub fn run(frames: u32) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App { frames_left: frames.clamp(1, 100_000), live: None, error: None, clock: barrel_game::clock::Clock::default(), last: None, shown: 0 };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e.into()),
        None => {
            eprintln!("presented {} frames, {} logic ticks", app.shown, app.clock.ticks);
            Ok(())
        }
    }
}

struct Live {
    window: Window,
    gpu: Gpu,
    presenter: Presenter,
    pool: vk::CommandPool,
    ready: vk::Semaphore,
    done: vk::Semaphore,
    fence: vk::Fence,
}

struct App {
    frames_left: u32,
    live: Option<Live>,
    error: Option<String>,
    clock: barrel_game::clock::Clock,
    last: Option<std::time::Instant>,
    shown: u32,
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let attrs = Window::default_attributes().with_title("Barrel").with_inner_size(winit::dpi::PhysicalSize::new(1280, 720));
        let window = event_loop.create_window(attrs)?;
        let display = window.display_handle()?.as_raw();
        let handle = window.window_handle()?.as_raw();
        let gpu = Gpu::new(Some(display))?;
        let size = window.inner_size();
        let presenter = Presenter::new(&gpu, display, handle, size.width, size.height)?;
        let d = &gpu.device;
        let pool = unsafe {
            d.create_command_pool(
                &vk::CommandPoolCreateInfo::default().queue_family_index(gpu.queue_family).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };
        let ready = unsafe { d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
        let done = unsafe { d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
        let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)? };
        eprintln!("window {}x{}, swapchain {:?} {}x{}", size.width, size.height, presenter.format, presenter.extent.width, presenter.extent.height);
        window.request_redraw();
        self.live = Some(Live { window, gpu, presenter, pool, ready, done, fence });
        Ok(())
    }

    /// One frame: the clock takes the real time since the last; the image is cleared to a
    /// colour from the logic time, blended by `alpha` like any drawn state.
    fn frame(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        let real = self.last.map_or(0.0, |l| (now - l).as_secs_f64());
        self.last = Some(now);
        let f = self.clock.advance(real);
        let t = (self.clock.time() + f.alpha as f64 / self.clock.rate() as f64) as f32;
        let Some(live) = &self.live else { return Ok(()) };
        let d = &live.gpu.device;
        unsafe {
            d.wait_for_fences(&[live.fence], true, u64::MAX)?;
            d.reset_fences(&[live.fence])?;
        }
        // (The smoke test does not resize; a stale swapchain just skips the frame.)
        let Some(index) = live.presenter.acquire(live.ready)? else { return Ok(()) };
        let image = live.presenter.images[index as usize];
        let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(live.pool).command_buffer_count(1))?[0] };
        let range = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1);
        let barrier = |old, new, src, dst| {
            vk::ImageMemoryBarrier::default().old_layout(old).new_layout(new).src_access_mask(src).dst_access_mask(dst).image(image).subresource_range(range)
        };
        let colour = [0.5 + 0.4 * t.sin(), 0.5 + 0.4 * (t * 0.7).sin(), 0.5 + 0.4 * (t * 1.3).sin(), 1.0];
        unsafe {
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier(vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_WRITE)],
            );
            d.cmd_clear_color_image(cb, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &vk::ClearColorValue { float32: colour }, &[range]);
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier(vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::PRESENT_SRC_KHR, vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::empty())],
            );
            d.end_command_buffer(cb)?;
            let (waits, stages, cbs, signals) = ([live.ready], [vk::PipelineStageFlags::TRANSFER], [cb], [live.done]);
            d.queue_submit(
                live.gpu.queue,
                &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)],
                live.fence,
            )?;
        }
        let _ = live.presenter.present(&live.gpu, index, live.done)?;
        unsafe {
            d.wait_for_fences(&[live.fence], true, u64::MAX)?;
            d.free_command_buffers(live.pool, &[cb]);
        }
        self.shown += 1;
        Ok(())
    }

    fn close(&mut self) {
        if let Some(live) = self.live.take() {
            unsafe {
                let _ = live.gpu.device.device_wait_idle();
                live.gpu.device.destroy_fence(live.fence, None);
                live.gpu.device.destroy_semaphore(live.ready, None);
                live.gpu.device.destroy_semaphore(live.done, None);
                live.gpu.device.destroy_command_pool(live.pool, None);
            }
            live.presenter.destroy(&live.gpu);
            drop(live.window);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_none()
            && let Err(e) = self.open(event_loop) {
                self.error = Some(e.to_string());
                event_loop.exit();
            }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.close();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.frame() {
                    self.error = Some(e.to_string());
                }
                self.frames_left = self.frames_left.saturating_sub(1);
                if self.frames_left == 0 || self.error.is_some() {
                    self.close();
                    event_loop.exit();
                } else if let Some(live) = &self.live {
                    live.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// A window driven from the renderer's own frame loop: winit's events are pumped once a
/// frame (without blocking), so the scene code keeps its straight line and the FIFO
/// swapchain paces the loop.
pub struct Pump {
    event_loop: EventLoop<()>,
    state: PumpState,
    /// A game controller, if SDL's gamepad layer started (one may be plugged in later).
    pad: Option<crate::pad::Gamepad>,
    /// Which way the left stick leaned last frame (for menu steps on a lean).
    lean: (i8, i8),
}

/// The keys a pad button stands for when pressed: the menus' and the field's (A confirms and
/// gathers, B goes back and jumps, X strikes, Y turns running on or off, Start pauses an event,
/// Back saves, the right stick's click puts the camera back, the D-pad steps through menus).
/// Escape (quit) has no button.
fn pad_keys(b: crate::pad::Pad) -> &'static [winit::keyboard::KeyCode] {
    use crate::pad::Pad as P;
    use winit::keyboard::KeyCode as K;
    match b {
        P::South => &[K::Enter, K::KeyG],
        P::East => &[K::Backspace, K::KeyJ],
        P::West => &[K::KeyK],
        P::North => &[K::KeyT],
        P::Start => &[K::Space],
        P::Back => &[K::F5],
        P::RightStick => &[K::KeyR],
        P::Up => &[K::ArrowUp],
        P::Down => &[K::ArrowDown],
        P::Left => &[K::ArrowLeft],
        P::Right => &[K::ArrowRight],
        P::LeftShoulder | P::RightShoulder => &[],
    }
}

#[derive(Default)]
struct PumpState {
    window: Option<Window>,
    closed: bool,
    error: Option<String>,
    /// Set when the window changed size since it was last taken.
    resized: bool,
    /// Keys held down now, and keys pressed since the last `take_pressed`.
    held: std::collections::HashSet<winit::keyboard::KeyCode>,
    pressed: Vec<winit::keyboard::KeyCode>,
}

impl ApplicationHandler for PumpState {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("Barrel").with_inner_size(winit::dpi::PhysicalSize::new(1280, 720));
            match event_loop.create_window(attrs) {
                Ok(w) => self.window = Some(w),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.closed = true,
            WindowEvent::Resized(_) => self.resized = true,
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                    if event.state.is_pressed() {
                        if !event.repeat {
                            self.pressed.push(code);
                        }
                        self.held.insert(code);
                    } else {
                        self.held.remove(&code);
                    }
                }
            }
            WindowEvent::Focused(false) => self.held.clear(),
            _ => {}
        }
    }
}

impl Pump {
    /// Opens the window (1280×720).
    pub fn open() -> Result<Pump> {
        use winit::platform::pump_events::EventLoopExtPumpEvents;
        let mut event_loop = EventLoop::new()?;
        let mut state = PumpState::default();
        for _ in 0..200 {
            let _ = event_loop.pump_app_events(Some(std::time::Duration::from_millis(10)), &mut state);
            if state.window.is_some() || state.error.is_some() {
                break;
            }
        }
        if let Some(e) = state.error.take() {
            return Err(e.into());
        }
        if state.window.is_none() {
            return Err("the window did not open".into());
        }
        Ok(Pump { event_loop, state, pad: crate::pad::Gamepad::open(), lean: (0, 0) })
    }

    pub fn window(&self) -> &Window {
        self.state.window.as_ref().expect("open made the window")
    }

    /// Keys pressed (not repeats) since the last call.
    pub fn take_pressed(&mut self) -> Vec<winit::keyboard::KeyCode> {
        std::mem::take(&mut self.state.pressed)
    }

    /// Whether `key` is held down now (on the keyboard, or by the pad: the D-pad holds W, A, S
    /// and D for walking, the shoulders - and = for the camera's distance).
    pub fn held(&self, key: winit::keyboard::KeyCode) -> bool {
        use crate::pad::Pad as P;
        use winit::keyboard::KeyCode as K;
        let pad = self.pad.as_ref().is_some_and(|g| match key {
            K::KeyW => g.held(P::Up),
            K::KeyS => g.held(P::Down),
            K::KeyA => g.held(P::Left),
            K::KeyD => g.held(P::Right),
            K::Minus => g.held(P::LeftShoulder),
            K::Equal => g.held(P::RightShoulder),
            _ => false,
        });
        pad || self.state.held.contains(&key)
    }

    /// The pad's sticks as read, (x right, y up) each: the left one walks, the right one turns
    /// the camera. Zero without a pad.
    pub fn sticks(&self) -> ([f32; 2], [f32; 2]) {
        self.pad.as_ref().map_or(([0.0; 2], [0.0; 2]), |g| (g.left, g.right))
    }

    /// Whether the window changed size since the last call.
    pub fn take_resized(&mut self) -> bool {
        std::mem::take(&mut self.state.resized)
    }

    /// Handles pending events; false once the window is closed.
    pub fn pump(&mut self) -> bool {
        use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
        if let PumpStatus::Exit(_) = self.event_loop.pump_app_events(Some(std::time::Duration::ZERO), &mut self.state) {
            return false;
        }
        if let Some(g) = self.pad.as_mut() {
            for b in g.poll() {
                self.state.pressed.extend_from_slice(pad_keys(b));
            }
            // A lean of the left stick past half way steps through menus like the arrows.
            let [x, y] = g.left;
            let lean = (if x > 0.5 { 1 } else if x < -0.5 { -1 } else { 0 }, if y > 0.5 { 1 } else if y < -0.5 { -1 } else { 0 });
            use winit::keyboard::KeyCode as K;
            if lean.0 != self.lean.0 && lean.0 != 0 {
                self.state.pressed.push(if lean.0 > 0 { K::ArrowRight } else { K::ArrowLeft });
            }
            if lean.1 != self.lean.1 && lean.1 != 0 {
                self.state.pressed.push(if lean.1 > 0 { K::ArrowUp } else { K::ArrowDown });
            }
            self.lean = lean;
        }
        !self.state.closed
    }
}
