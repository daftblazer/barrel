//! The live window: winit's events pumped once a frame ([`Pump`]), frames presented at the
//! display's rate through [`crate::present::Presenter`] (`docs/renderer.md`, "Live window").

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::Result;

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
    /// A new window size or fullscreen to open again with at the next scene's start (the
    /// compositor keeps a live window's size: `reopen`).
    pub pending: Option<crate::settings::Display>,
}

/// How many OS windows this process has made (printed as each is, `window: created N`, so a
/// scripted run can check that a play keeps its one window).
static CREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The keys a pad button stands for on every screen: the game's menus confirm with A and go
/// back with B and step with the D-pad (`docs/engine.md`, "Controller"). The rest of the pad
/// means something else on each screen; the field and events read it as pad buttons
/// ([`Pump::take_input`]). Escape (quit) has no button.
fn pad_keys(b: crate::pad::Pad) -> &'static [winit::keyboard::KeyCode] {
    use crate::pad::Pad as P;
    use winit::keyboard::KeyCode as K;
    match b {
        P::South => &[K::Enter],
        P::East => &[K::Backspace],
        P::Up => &[K::ArrowUp],
        P::Down => &[K::ArrowDown],
        P::Left => &[K::ArrowLeft],
        P::Right => &[K::ArrowRight],
        _ => &[],
    }
}

#[derive(Default)]
struct PumpState {
    /// The window as it opens (`settings::Display`).
    display: Option<crate::settings::Display>,
    window: Option<Window>,
    closed: bool,
    error: Option<String>,
    /// Set when the window changed size since it was last taken.
    resized: bool,
    /// Keys held down now, and keys pressed since the last `take_pressed`.
    held: std::collections::HashSet<winit::keyboard::KeyCode>,
    pressed: Vec<winit::keyboard::KeyCode>,
    /// Pad buttons pressed since the last `take_input` (or `take_pressed`, which drops them).
    pad_pressed: Vec<crate::pad::Pad>,
}

impl PumpState {
    fn make_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let d = self.display.unwrap_or(crate::settings::Display::DEFAULT);
            let attrs = Window::default_attributes()
                .with_title("Barrel")
                .with_inner_size(winit::dpi::PhysicalSize::new(d.width, d.height))
                .with_fullscreen(d.fullscreen.then_some(winit::window::Fullscreen::Borderless(None)));
            match event_loop.create_window(attrs) {
                Ok(w) => {
                    let n = CREATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    eprintln!("window: created {n}");
                    self.window = Some(w);
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
}

impl ApplicationHandler for PumpState {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.make_window(event_loop);
    }

    // A window closed to open again (`Pump::reopen`) is made on the loop's next pass.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.make_window(event_loop);
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
    /// Opens the window as `display` has it (its size, or borderless fullscreen).
    pub fn open(display: crate::settings::Display) -> Result<Pump> {
        use winit::platform::pump_events::EventLoopExtPumpEvents;
        let mut event_loop = EventLoop::new()?;
        let mut state = PumpState { display: Some(display), ..PumpState::default() };
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
        // SIGTERM (how Steam and Kintsugi stop a game) and SIGINT end the run as closing the
        // window does.
        unsafe {
            libc::signal(libc::SIGTERM, stop_signal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGINT, stop_signal as *const () as libc::sighandler_t);
        }
        Ok(Pump { event_loop, state, pad: crate::pad::Gamepad::open(), lean: (0, 0), pending: None })
    }

    /// Closes the window and opens it again as `display` has it (between scenes: nothing may
    /// draw to the old one).
    pub fn reopen(&mut self, display: crate::settings::Display) -> Result<()> {
        use winit::platform::pump_events::EventLoopExtPumpEvents;
        self.state.window = None;
        self.state.display = Some(display);
        self.state.held.clear();
        for _ in 0..200 {
            let _ = self.event_loop.pump_app_events(Some(std::time::Duration::from_millis(10)), &mut self.state);
            if self.state.window.is_some() || self.state.error.is_some() {
                break;
            }
        }
        if let Some(e) = self.state.error.take() {
            return Err(e.into());
        }
        if self.state.window.is_none() {
            return Err("the window did not open again".into());
        }
        Ok(())
    }

    pub fn window(&self) -> &Window {
        self.state.window.as_ref().expect("open made the window")
    }

    /// Keys pressed (not repeats) since the last call; the pad's own presses are dropped (a
    /// screen reads the pad through the keys it stands for).
    pub fn take_pressed(&mut self) -> Vec<winit::keyboard::KeyCode> {
        self.take_input().0
    }

    /// Keys pressed since the last call, and the pad's buttons pressed (each also among the
    /// keys if it stands for one on every screen).
    pub fn take_input(&mut self) -> (Vec<winit::keyboard::KeyCode>, Vec<crate::pad::Pad>) {
        (std::mem::take(&mut self.state.pressed), std::mem::take(&mut self.state.pad_pressed))
    }

    /// Whether `key` is held down now, on the keyboard (the pad holds no keys: the game's field
    /// gives the triggers and shoulders nothing to hold).
    pub fn held(&self, key: winit::keyboard::KeyCode) -> bool {
        self.state.held.contains(&key)
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
                self.state.pad_pressed.push(b);
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
        !self.state.closed && !STOPPED.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Set by SIGTERM or SIGINT.
static STOPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn stop_signal(_: libc::c_int) {
    STOPPED.store(true, std::sync::atomic::Ordering::Relaxed);
}
