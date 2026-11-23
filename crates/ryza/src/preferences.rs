//! What the Settings screen's Audio, Camera and Game values do once they are saved: the
//! category volumes, the right stick's turn and pitch, and which pad button confirms.
//!
//! The values are the rows' `option_` keys (`options::State`): read here from the saved config
//! so a start and a live apply (Yes on the screen) take the same path. The defaults are the
//! rows' defaults, and at those everything is exactly as it was before the rows did anything.

use barrel_render::pad::Pad;

use crate::options::State;
use crate::walk::Feel;

/// The volume rows' middle, which is the level the game plays at with the defaults.
const VOLUME_MIDDLE: i32 = 5;
// STAND-IN: the game's volume curve is not read. Barrel steps the rows in decibels about the
// default (row 5 is 1.0, the level measured against the opening's recording): 6 dB a step down,
// to -24 dB at 1 and silence at 0, and 1.2 dB a step up, to +6 dB (twice the level) at 10.
const DB_STEP_DOWN: f32 = 6.0;
const DB_STEP_UP: f32 = 1.2;

/// The gain of a volume row's value (0 to 10).
pub fn volume_gain(value: i32) -> f32 {
    let value = value.clamp(0, 10);
    if value == 0 {
        return 0.0;
    }
    let step = value - VOLUME_MIDDLE;
    let db = step as f32 * if step < 0 { DB_STEP_DOWN } else { DB_STEP_UP };
    10f32.powf(db / 20.0)
}

/// The gains of the four sound categories.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volumes {
    pub music: f32,
    pub effects: f32,
    pub voice: f32,
    pub movie: f32,
}

impl Volumes {
    pub fn log_line(&self) -> String {
        format!("sound: volumes bgm {:.3} sfx {:.3} voice {:.3} movie {:.3}", self.music, self.effects, self.voice, self.movie)
    }
}

/// The saved values that change how the game plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub volumes: Volumes,
    pub feel: Feel,
    /// The Confirm Button row's second choice: B confirms and A goes back.
    pub swap_confirm: bool,
    /// The Map Rotation row on (value 0; Off, 1, is the default): the minimap turns with the camera.
    pub map_rotation: bool,
}

impl Play {
    /// The values of `state` (the screen's, or the config's loaded into a fresh one).
    pub fn of(state: &State) -> Play {
        let value = |key: &str, default: i32| state.option(key).unwrap_or(default);
        Play {
            volumes: Volumes {
                music: volume_gain(value("volume_bgm", 5)),
                effects: volume_gain(value("volume_sfx", 5)),
                voice: volume_gain(value("volume_voice", 5)),
                movie: volume_gain(value("volume_movie", 5)),
            },
            feel: Feel::of(value("camera_horizontal", 0) != 0, value("camera_vertical", 0) != 0, value("camera_horizontal_speed", 5), value("camera_vertical_speed", 5)),
            swap_confirm: value("confirm_button", 0) != 0,
            map_rotation: value("map_rotation", 1) == 0,
        }
    }

    /// The values `config` holds (the rows' defaults for those it lacks).
    pub fn from_config(config: Option<&str>) -> Play {
        let mut state = State::new(Default::default(), barrel_render::settings::RenderSettings::DESKTOP);
        if let Some(c) = config {
            state.load(c);
        }
        Play::of(&state)
    }

    /// The pad buttons that confirm and go back.
    pub fn buttons(&self) -> (Pad, Pad) {
        match self.swap_confirm {
            false => (Pad::South, Pad::East),
            true => (Pad::East, Pad::South),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_rows_scale_about_the_default() {
        assert_eq!(volume_gain(0), 0.0);
        assert_eq!(volume_gain(5), 1.0);
        assert!((volume_gain(10) - 2.0).abs() < 0.02, "{}", volume_gain(10));
        assert!((volume_gain(1) - 0.0631).abs() < 1e-3, "{}", volume_gain(1));
        assert!((1..10).all(|v| volume_gain(v) < volume_gain(v + 1)));
    }

    #[test]
    fn the_defaults_change_nothing() {
        let p = Play::from_config(None);
        assert_eq!(p.volumes, Volumes { music: 1.0, effects: 1.0, voice: 1.0, movie: 1.0 });
        assert_eq!(p.feel, Feel::default());
        assert_eq!(p.buttons(), (Pad::South, Pad::East));
        assert_eq!(Play::from_config(Some("window_width 1280\n")), p);
    }

    #[test]
    fn the_saved_values_reach_what_they_scale() {
        let p = Play::from_config(Some("option_volume_bgm 0\noption_volume_voice 10\noption_camera_horizontal 1\noption_camera_vertical_speed 10\noption_confirm_button 1\n"));
        assert_eq!(p.volumes.music, 0.0);
        assert!(p.volumes.voice > 1.9);
        assert_eq!(p.volumes.effects, 1.0);
        assert_eq!(p.feel, Feel { yaw: -1.0, pitch: 2.0 });
        assert_eq!(p.buttons(), (Pad::East, Pad::South));
    }

    #[test]
    fn map_rotation_is_on_at_value_zero_and_off_by_default() {
        assert!(!Play::from_config(None).map_rotation);
        assert!(Play::from_config(Some("option_map_rotation 0\n")).map_rotation);
        assert!(!Play::from_config(Some("option_map_rotation 1\n")).map_rotation);
    }
}
