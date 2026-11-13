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
    RightStick,
    Up,
    Down,
    Left,
    Right,
}

const BUTTONS: [(Pad, SDL_GamepadButton); 13] = [
    (Pad::South, SDL_GAMEPAD_BUTTON_SOUTH),
    (Pad::East, SDL_GAMEPAD_BUTTON_EAST),
    (Pad::West, SDL_GAMEPAD_BUTTON_WEST),
    (Pad::North, SDL_GAMEPAD_BUTTON_NORTH),
    (Pad::Back, SDL_GAMEPAD_BUTTON_BACK),
    (Pad::Start, SDL_GAMEPAD_BUTTON_START),
    (Pad::LeftShoulder, SDL_GAMEPAD_BUTTON_LEFT_SHOULDER),
    (Pad::RightShoulder, SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER),
    (Pad::RightStick, SDL_GAMEPAD_BUTTON_RIGHT_STICK),
    (Pad::Up, SDL_GAMEPAD_BUTTON_DPAD_UP),
    (Pad::Down, SDL_GAMEPAD_BUTTON_DPAD_DOWN),
    (Pad::Left, SDL_GAMEPAD_BUTTON_DPAD_LEFT),
    (Pad::Right, SDL_GAMEPAD_BUTTON_DPAD_RIGHT),
];

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
        self.left = [axis(SDL_GAMEPAD_AXIS_LEFTX).clamp(-1.0, 1.0), (-axis(SDL_GAMEPAD_AXIS_LEFTY)).clamp(-1.0, 1.0)];
        self.right = [axis(SDL_GAMEPAD_AXIS_RIGHTX).clamp(-1.0, 1.0), (-axis(SDL_GAMEPAD_AXIS_RIGHTY)).clamp(-1.0, 1.0)];
        pressed
    }

    pub fn held(&self, b: Pad) -> bool {
        self.held.contains(&b)
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
    fn dead_zone() {
        assert_eq!(super::stick(0.2, 0.0), [0.0, 0.0]);
        let [x, y] = super::stick(1.0, 0.0);
        assert!((x - 1.0).abs() < 1e-6 && y == 0.0);
        let [x, _] = super::stick(0.625, 0.0);
        assert!((x - 0.5).abs() < 1e-6);
    }
}
