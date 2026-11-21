//! A game controller through SDL3's gamepad layer (its mapping database knows most pads, so
//! buttons are by place: south is A on an Xbox pad, cross on a PlayStation one). The first pad
//! found is used; it is looked for again about once a second while none is open, so one plugged
//! in later works too.

use std::collections::HashSet;

use sdl3_sys::events::SDL_PumpEvents;
use sdl3_sys::gamepad::*;
use sdl3_sys::init::{SDL_Init, SDL_INIT_GAMEPAD};
use sdl3_sys::stdinc::SDL_free;

/// A pad's buttons, by place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pad {
    South,
    East,
    West,
    North,
    Back,
    Start,
    LeftShoulder,
    RightShoulder,
    LeftStick,
    RightStick,
    /// The triggers, pressed once past [`TRIGGER`].
    LeftTrigger,
    RightTrigger,
    Up,
    Down,
    Left,
    Right,
}

const BUTTONS: [(Pad, SDL_GamepadButton); 14] = [
    (Pad::South, SDL_GAMEPAD_BUTTON_SOUTH),
    (Pad::East, SDL_GAMEPAD_BUTTON_EAST),
    (Pad::West, SDL_GAMEPAD_BUTTON_WEST),
    (Pad::North, SDL_GAMEPAD_BUTTON_NORTH),
    (Pad::Back, SDL_GAMEPAD_BUTTON_BACK),
    (Pad::Start, SDL_GAMEPAD_BUTTON_START),
    (Pad::LeftShoulder, SDL_GAMEPAD_BUTTON_LEFT_SHOULDER),
    (Pad::RightShoulder, SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER),
    (Pad::LeftStick, SDL_GAMEPAD_BUTTON_LEFT_STICK),
    (Pad::RightStick, SDL_GAMEPAD_BUTTON_RIGHT_STICK),
    (Pad::Up, SDL_GAMEPAD_BUTTON_DPAD_UP),
    (Pad::Down, SDL_GAMEPAD_BUTTON_DPAD_DOWN),
    (Pad::Left, SDL_GAMEPAD_BUTTON_DPAD_LEFT),
    (Pad::Right, SDL_GAMEPAD_BUTTON_DPAD_RIGHT),
];

/// How far a trigger must be pulled to count as pressed: the game's 30 of 255
/// (`0x1405d11e8`).
pub const TRIGGER: f32 = 30.0 / 255.0;

/// How far a stick must lean before it counts.
pub const DEAD_ZONE: f32 = 0.25;

pub struct Gamepad {
    pad: *mut SDL_Gamepad,
    held: HashSet<Pad>,
    next_look: std::time::Instant,
    /// The sticks as read, (x right, y up) each in -1..1 (the walk applies the game's own dead
    /// zone; the camera uses [`stick`]).
    pub left: [f32; 2],
    pub right: [f32; 2],
}

impl Gamepad {
    /// SDL's gamepad layer; `None` if it does not start.
    pub fn open() -> Option<Gamepad> {
        if !unsafe { SDL_Init(SDL_INIT_GAMEPAD) } {
            eprintln!("no gamepads: SDL's gamepad layer did not start");
            return None;
        }
        let mut g = Gamepad { pad: std::ptr::null_mut(), held: HashSet::new(), next_look: std::time::Instant::now(), left: [0.0; 2], right: [0.0; 2] };
        g.look();
        Some(g)
    }

    fn look(&mut self) {
        self.next_look = std::time::Instant::now() + std::time::Duration::from_secs(1);
        unsafe {
            let mut count = 0;
            let ids = SDL_GetGamepads(&mut count);
            if !ids.is_null() {
                if count > 0 {
                    let pad = SDL_OpenGamepad(*ids);
                    if !pad.is_null() {
                        let name = SDL_GetGamepadName(pad);
                        let name = if name.is_null() { "a gamepad".into() } else { std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned() };
                        eprintln!("gamepad: {name}");
                        self.pad = pad;
                    }
                }
                SDL_free(ids.cast());
            }
        }
    }

    /// Reads the pad: the buttons pressed since the last call.
    pub fn poll(&mut self) -> Vec<Pad> {
        unsafe { SDL_PumpEvents() };
        if !self.pad.is_null() && !unsafe { SDL_GamepadConnected(self.pad) } {
            eprintln!("gamepad: gone");
            unsafe { SDL_CloseGamepad(self.pad) };
            self.pad = std::ptr::null_mut();
            self.held.clear();
            self.left = [0.0; 2];
            self.right = [0.0; 2];
        }
        if self.pad.is_null() {
            if std::time::Instant::now() >= self.next_look {
                self.look();
            }
            if self.pad.is_null() {
                return Vec::new();
            }
        }
        let mut pressed = Vec::new();
        for (b, sdl) in BUTTONS {
            let down = unsafe { SDL_GetGamepadButton(self.pad, sdl) };
            if down && self.held.insert(b) {
                pressed.push(b);
            } else if !down {
                self.held.remove(&b);
            }
        }
        let axis = |a| f32::from(unsafe { SDL_GetGamepadAxis(self.pad, a) }) / 32767.0;
        for (b, a) in [(Pad::LeftTrigger, SDL_GAMEPAD_AXIS_LEFT_TRIGGER), (Pad::RightTrigger, SDL_GAMEPAD_AXIS_RIGHT_TRIGGER)] {
            let down = axis(a) > TRIGGER;
            if down && self.held.insert(b) {
                pressed.push(b);
            } else if !down {
                self.held.remove(&b);
            }
        }
        self.left = [axis(SDL_GAMEPAD_AXIS_LEFTX).clamp(-1.0, 1.0), (-axis(SDL_GAMEPAD_AXIS_LEFTY)).clamp(-1.0, 1.0)];
        self.right = [axis(SDL_GAMEPAD_AXIS_RIGHTX).clamp(-1.0, 1.0), (-axis(SDL_GAMEPAD_AXIS_RIGHTY)).clamp(-1.0, 1.0)];
        pressed
    }

    pub fn held(&self, b: Pad) -> bool {
        self.held.contains(&b)
    }

    /// Every button down now.
    pub fn down(&self) -> &HashSet<Pad> {
        &self.held
    }
}

/// A stick's lean with the dead zone taken out (radial), rescaled so the edge of the zone is 0.
pub fn stick(x: f32, y: f32) -> [f32; 2] {
    let len = (x * x + y * y).sqrt();
    if len <= DEAD_ZONE {
        return [0.0; 2];
    }
    let k = ((len - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0) / len;
    [x * k, y * k]
}

/// A stick's lean as a menu direction, with hysteresis so a stick resting near the threshold does
/// not retrigger. Barrel's choice (the game's values are not read): a direction counts once the
/// stick passes [`MENU_PRESS`] and until it falls below [`MENU_RELEASE`], which is the field
/// walk's dead zone (0.2257, `barrel_ryza::walk`).
pub const MENU_PRESS: f32 = 0.4;
pub const MENU_RELEASE: f32 = 0.2257;

/// The directions a stick holds as menu input: x -1 left, 1 right; y -1 down, 1 up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lean {
    pub x: i8,
    pub y: i8,
}

impl Lean {
    /// Takes the stick as read, (x right, y up).
    pub fn update(&mut self, [x, y]: [f32; 2]) {
        fn axis(was: i8, v: f32) -> i8 {
            let side = if v > 0.0 { 1 } else { -1 };
            if was == side && v.abs() >= MENU_RELEASE {
                was
            } else if v.abs() >= MENU_PRESS {
                side
            } else {
                0
            }
        }
        // Only the stronger axis counts, so a diagonal does not step two ways at once.
        let (ax, ay) = (x.abs(), y.abs());
        self.x = if ax >= ay { axis(self.x, x) } else { 0 };
        self.y = if ay > ax { axis(self.y, y) } else { 0 };
    }
}

/// A scripted pad for runs without a person (`BARREL_PAD_SCRIPT=file`): one line each,
/// `<frame> <button> <frames held>`, frames at 60 Hz on the run's own clock. Buttons: A B X Y LB RB
/// LT RT UP DOWN LEFT RIGHT START BACK; `LX=v` / `LY=v` / `RX=v` / `RY=v` hold a stick axis at
/// `v`. `#` starts a comment.
#[derive(Clone, Debug, Default)]
pub struct Script {
    lines: Vec<(i64, Input, i64)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Input {
    Button(Pad),
    /// Axis 0..4: left x, left y, right x, right y.
    Axis(usize, f32),
}

impl Script {
    pub fn parse(text: &str) -> Script {
        let mut lines = Vec::new();
        for line in text.lines().map(|l| l.split('#').next().unwrap_or("")) {
            let mut w = line.split_whitespace();
            let (Some(f), Some(b), Some(n)) = (w.next(), w.next(), w.next()) else { continue };
            let (Ok(f), Ok(n)) = (f.parse::<i64>(), n.parse::<i64>()) else { continue };
            let input = match b.to_ascii_uppercase().as_str() {
                "A" => Input::Button(Pad::South),
                "B" => Input::Button(Pad::East),
                "X" => Input::Button(Pad::West),
                "Y" => Input::Button(Pad::North),
                "LB" => Input::Button(Pad::LeftShoulder),
                "RB" => Input::Button(Pad::RightShoulder),
                "LT" => Input::Button(Pad::LeftTrigger),
                "RT" => Input::Button(Pad::RightTrigger),
                "UP" => Input::Button(Pad::Up),
                "DOWN" => Input::Button(Pad::Down),
                "LEFT" => Input::Button(Pad::Left),
                "RIGHT" => Input::Button(Pad::Right),
                "START" => Input::Button(Pad::Start),
                "BACK" => Input::Button(Pad::Back),
                axis => {
                    let Some((name, v)) = axis.split_once('=') else { continue };
                    let (Some(i), Ok(v)) = (["LX", "LY", "RX", "RY"].iter().position(|a| *a == name), v.parse::<f32>()) else { continue };
                    Input::Axis(i, v)
                }
            };
            lines.push((f, input, n));
        }
        Script { lines }
    }

    pub fn load_env() -> Option<Script> {
        let path = std::env::var_os("BARREL_PAD_SCRIPT")?;
        match std::fs::read_to_string(&path) {
            Ok(t) => Some(Script::parse(&t)),
            Err(e) => {
                eprintln!("pad script: {}: {e}", std::path::Path::new(&path).display());
                None
            }
        }
    }

    /// The buttons down and the sticks (left, right) on `frame`.
    pub fn at(&self, frame: i64) -> (HashSet<Pad>, [f32; 2], [f32; 2]) {
        let (mut down, mut axes) = (HashSet::new(), [0.0f32; 4]);
        for &(f, input, n) in &self.lines {
            if frame >= f && frame < f + n {
                match input {
                    Input::Button(b) => {
                        down.insert(b);
                    }
                    Input::Axis(i, v) => axes[i] = v,
                }
            }
        }
        (down, [axes[0], axes[1]], [axes[2], axes[3]])
    }
}

impl Drop for Gamepad {
    fn drop(&mut self) {
        if !self.pad.is_null() {
            unsafe { SDL_CloseGamepad(self.pad) };
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lean_hysteresis() {
        let mut l = super::Lean::default();
        l.update([0.3, 0.0]);
        assert_eq!(l, super::Lean::default());
        l.update([0.5, 0.0]);
        assert_eq!(l.x, 1);
        l.update([0.3, 0.0]);
        assert_eq!(l.x, 1, "held between the two thresholds");
        l.update([0.2, 0.0]);
        assert_eq!(l.x, 0);
        l.update([0.3, 0.0]);
        assert_eq!(l.x, 0, "not again until past the press threshold");
        l.update([0.0, -1.0]);
        assert_eq!((l.x, l.y), (0, -1));
        l.update([0.6, 0.7]);
        assert_eq!((l.x, l.y), (0, 1), "the stronger axis only");
    }

    #[test]
    fn script_lines() {
        let s = super::Script::parse("10 A 6\n20 LY=-1.0 6 # tap\n");
        assert!(s.at(9).0.is_empty() && s.at(10).0.contains(&super::Pad::South) && s.at(16).0.is_empty());
        assert_eq!(s.at(22).1, [0.0, -1.0]);
        assert_eq!(s.at(26).1, [0.0, 0.0]);
    }

    #[test]
    fn dead_zone() {
        assert_eq!(super::stick(0.2, 0.0), [0.0, 0.0]);
        let [x, y] = super::stick(1.0, 0.0);
        assert!((x - 1.0).abs() < 1e-6 && y == 0.0);
        let [x, _] = super::stick(0.625, 0.0);
        assert!((x - 0.5).abs() < 1e-6);
    }
}
