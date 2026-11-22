//! The graphics page's choices (Barrel's own: the PC release sets these in a launcher, which
//! Barrel does not have; `docs/renderer.md`, "Graphics settings"): render scale, anti-aliasing,
//! upscaler, sharpening, shadows and texture filtering over [`RenderSettings`], each choice
//! either open or greyed out with the reason the help bar gives. Choosing keeps the settings
//! as [`RenderSettings::resolved`] would have them, so the page never shows a choice the
//! renderer would quietly undo (the page starts from the settings resolved).

use crate::settings::{RenderSettings, Upscaler};
use crate::shadow::Shadows;

/// The page's rows, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// Window or fullscreen, and the window's size (`settings::Display`: `options.rs`).
    Mode,
    WindowSize,
    Scale,
    AntiAliasing,
    Upscaling,
    Sharpening,
    Shadows,
    Filtering,
    /// The screen-space ambient occlusion (`ao.rs`), for the games that have the pass.
    Occlusion,
}

/// The rows of a game without the occlusion pass (Escha's page).
pub const ROWS: [Row; 8] = [Row::Mode, Row::WindowSize, Row::Scale, Row::AntiAliasing, Row::Upscaling, Row::Sharpening, Row::Shadows, Row::Filtering];
/// The rows of a game with it (Ryza's): the same, and the occlusion's.
pub const ROWS_WITH_OCCLUSION: [Row; 9] = [Row::Mode, Row::WindowSize, Row::Scale, Row::AntiAliasing, Row::Upscaling, Row::Sharpening, Row::Shadows, Row::Filtering, Row::Occlusion];

/// The render scales offered.
pub const SCALES: [f32; 8] = [0.5, 2.0 / 3.0, 0.75, 0.85, 1.0, 1.25, 1.5, 2.0];
/// The anisotropic filtering levels offered (1: off).
pub const FILTERING: [u32; 5] = [1, 2, 4, 8, 16];

/// Anti-aliasing as the page names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aa {
    Off,
    Msaa(u32),
    Taa,
    /// FSR 3 at full resolution (its "native AA").
    Native,
    /// A temporal upscaler (GSR 2, FSR 3 below full size) does it.
    ByUpscaler,
}

pub const AA_CHOICES: [Aa; 6] = [Aa::Off, Aa::Msaa(2), Aa::Msaa(4), Aa::Msaa(8), Aa::Taa, Aa::Native];

/// Upscalers as the page names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Up {
    Bilinear,
    Fsr1,
    Gsr1,
    Gsr2,
    Fsr3,
}

pub const UP_CHOICES: [Up; 5] = [Up::Bilinear, Up::Fsr1, Up::Gsr1, Up::Gsr2, Up::Fsr3];

/// What a row sets, for the help line when nothing on it is closed (Barrel's own words).
pub fn description(row: Row) -> &'static str {
    match row {
        Row::Mode => "Play in a window or borderless fullscreen.",
        Row::WindowSize => "The window's size.",
        Row::Scale => "The scene is drawn at this share of the screen's size, then scaled to it.",
        Row::AntiAliasing => "Smooths jagged edges.",
        Row::Upscaling => "How a scene drawn below full size is scaled up.",
        Row::Sharpening => "Sharpens the picture.",
        Row::Shadows => "The shadow maps' detail.",
        Row::Filtering => "Keeps textures sharp at a slant.",
        Row::Occlusion => "Darkens creases and corners the light does not reach.",
    }
}

impl Aa {
    pub fn label(self) -> String {
        match self {
            Aa::Off => "Off".into(),
            Aa::Msaa(n) => format!("MSAA {n}x"),
            Aa::Taa => "TAA".into(),
            Aa::Native => "FSR 3 Native AA".into(),
            Aa::ByUpscaler => "By the upscaler".into(),
        }
    }
}

impl Up {
    pub fn label(self) -> &'static str {
        match self {
            Up::Bilinear => "Bilinear",
            Up::Fsr1 => "FSR 1",
            Up::Gsr1 => "GSR 1",
            Up::Gsr2 => "GSR 2",
            Up::Fsr3 => "FSR 3",
        }
    }

    fn temporal(self) -> bool {
        matches!(self, Up::Gsr2 | Up::Fsr3)
    }
}

/// Whether `s` draws the scene smaller than the output.
fn below_full(s: &RenderSettings) -> bool {
    s.scale < 1.0
}

pub fn aa(s: &RenderSettings) -> Aa {
    if s.fsr3 && !below_full(s) {
        Aa::Native
    } else if s.gsr2 || s.fsr3 {
        Aa::ByUpscaler
    } else if s.taa {
        Aa::Taa
    } else if s.msaa > 1 {
        Aa::Msaa(s.msaa)
    } else {
        Aa::Off
    }
}

pub fn upscaler(s: &RenderSettings) -> Up {
    if s.gsr2 {
        Up::Gsr2
    } else if s.fsr3 && below_full(s) {
        Up::Fsr3
    } else {
        match s.upscaler {
            Upscaler::Bilinear => Up::Bilinear,
            Upscaler::Fsr1 => Up::Fsr1,
            Upscaler::Gsr1 => Up::Gsr1,
        }
    }
}

/// Whether anti-aliasing `a` can be chosen, else why not.
pub fn aa_open(s: &RenderSettings, a: Aa) -> Result<(), &'static str> {
    if upscaler(s).temporal() {
        return Err("The temporal upscaler does its own anti-aliasing.");
    }
    match a {
        Aa::Native if below_full(s) => Err("Native AA needs a render scale of 100% or more."),
        Aa::Native if upscaler(s) != Up::Bilinear => Err("Native AA takes the place of the upscaler."),
        Aa::ByUpscaler => Err("Chosen with the upscaler."),
        _ => Ok(()),
    }
}

/// Whether upscaler `u` can be chosen, else why not.
pub fn up_open(s: &RenderSettings, u: Up) -> Result<(), &'static str> {
    match u {
        Up::Bilinear => Ok(()),
        _ if !below_full(s) => Err("Upscalers need a render scale under 100%."),
        Up::Fsr1 | Up::Gsr1 if aa(s) == Aa::Native => Err("Native AA takes the place of the upscaler."),
        _ => Ok(()),
    }
}

/// Whether sharpening can be changed, else why not.
pub fn sharpen_open(s: &RenderSettings) -> Result<(), &'static str> {
    if s.gsr2 || s.fsr3 { Err("FSR 3 and GSR 2 sharpen on their own.") } else { Ok(()) }
}

/// The temporal settings one of GSR 2 or FSR 3 runs with (`resolved`'s outcome).
fn temporal(s: &mut RenderSettings, fsr3: bool) {
    s.fsr3 = fsr3;
    s.gsr2 = !fsr3;
    s.taa = false;
    s.msaa = 1;
    s.motion = true;
    s.jitter = true;
    s.sharpen = false;
    s.upscaler = Upscaler::Bilinear;
}

/// Neither TAA nor a temporal upscaler.
fn no_temporal(s: &mut RenderSettings) {
    s.fsr3 = false;
    s.gsr2 = false;
    s.taa = false;
    s.motion = false;
    s.jitter = false;
}

pub fn set_aa(s: &mut RenderSettings, a: Aa) {
    match a {
        Aa::Off | Aa::ByUpscaler => {
            no_temporal(s);
            s.msaa = 1;
        }
        Aa::Msaa(n) => {
            no_temporal(s);
            s.msaa = n;
        }
        Aa::Taa => {
            no_temporal(s);
            s.taa = true;
            s.motion = true;
            s.jitter = true;
            s.msaa = 1;
        }
        Aa::Native => temporal(s, true),
    }
}

pub fn set_upscaler(s: &mut RenderSettings, u: Up) {
    match u {
        Up::Gsr2 => temporal(s, false),
        Up::Fsr3 => temporal(s, true),
        spatial => {
            if upscaler(s).temporal() {
                no_temporal(s);
            }
            s.upscaler = match spatial {
                Up::Fsr1 => Upscaler::Fsr1,
                Up::Gsr1 => Upscaler::Gsr1,
                _ => Upscaler::Bilinear,
            };
        }
    }
}

/// A new render scale: at full size or more the upscalers have nothing to do (GSR 2 goes, and
/// with it its anti-aliasing; FSR 3 stays as native AA).
pub fn set_scale(s: &mut RenderSettings, scale: f32) {
    s.scale = scale;
    if !below_full(s) {
        s.upscaler = Upscaler::Bilinear;
        if s.gsr2 {
            no_temporal(s);
        }
    }
}

pub fn set_sharpen(s: &mut RenderSettings, on: bool) {
    if sharpen_open(s).is_ok() {
        s.sharpen = on;
    }
}

pub fn set_shadows(s: &mut RenderSettings, high: bool) {
    s.shadows = if high { Shadows::DESKTOP } else { Shadows::HANDHELD };
}

pub fn set_occlusion(s: &mut RenderSettings, on: bool) {
    s.occlusion = on;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_game_with_the_pass_has_the_occlusion_row() {
        assert!(!ROWS.contains(&Row::Occlusion));
        assert_eq!(ROWS_WITH_OCCLUSION[..ROWS.len()], ROWS);
        assert_eq!(ROWS_WITH_OCCLUSION[ROWS.len()..], [Row::Occlusion]);
    }

    #[test]
    fn the_occlusion_choice_sets_the_setting_and_is_kept_by_the_config() {
        let mut s = RenderSettings::DESKTOP.resolved();
        set_occlusion(&mut s, false);
        assert!(!s.occlusion);
        set_occlusion(&mut s, true);
        assert!(s.occlusion);
    }

    /// Every open choice from `s`, applied.
    fn moves(s: RenderSettings) -> Vec<RenderSettings> {
        let mut out = Vec::new();
        for &scale in &SCALES {
            let mut t = s;
            set_scale(&mut t, scale);
            out.push(t);
        }
        for a in AA_CHOICES.into_iter().filter(|&a| aa_open(&s, a).is_ok()) {
            let mut t = s;
            set_aa(&mut t, a);
            out.push(t);
        }
        for u in UP_CHOICES.into_iter().filter(|&u| up_open(&s, u).is_ok()) {
            let mut t = s;
            set_upscaler(&mut t, u);
            out.push(t);
        }
        for on in [false, true] {
            let mut t = s;
            set_sharpen(&mut t, on);
            out.push(t);
        }
        out
    }

    #[test]
    fn every_reachable_choice_is_one_resolved_keeps() {
        // From each preset as the renderer takes it (resolved), three choices deep.
        let mut seen = [RenderSettings::DESKTOP, RenderSettings::HANDHELD, RenderSettings::DESKTOP_TAA, RenderSettings::HANDHELD_GSR2].map(RenderSettings::resolved).to_vec();
        let mut edge = seen.clone();
        for _ in 0..3 {
            let mut next = Vec::new();
            for s in edge {
                for t in moves(s) {
                    assert_eq!(t.resolved(), t, "from {s:?}");
                    if !seen.contains(&t) {
                        seen.push(t);
                        next.push(t);
                    }
                }
            }
            edge = next;
        }
        assert!(seen.len() > 50, "{}", seen.len());
    }

    #[test]
    fn conflicts_grey_out() {
        let mut s = RenderSettings::DESKTOP;
        // At full size: no upscalers, native AA open.
        assert!(up_open(&s, Up::Fsr1).is_err() && aa_open(&s, Aa::Native).is_ok());
        set_aa(&mut s, Aa::Msaa(4));
        assert_eq!(aa(&s), Aa::Msaa(4));
        // Below full size with GSR 2: MSAA, TAA and sharpening greyed out.
        set_scale(&mut s, 0.75);
        set_upscaler(&mut s, Up::Gsr2);
        assert_eq!((aa(&s), s.msaa), (Aa::ByUpscaler, 1));
        assert!(aa_open(&s, Aa::Msaa(4)).is_err() && aa_open(&s, Aa::Taa).is_err() && sharpen_open(&s).is_err());
        // Back to full size: GSR 2 goes, anti-aliasing is off.
        set_scale(&mut s, 1.0);
        assert_eq!((upscaler(&s), aa(&s)), (Up::Bilinear, Aa::Off));
        // Native AA, then a smaller scale: FSR 3 as the upscaler.
        set_aa(&mut s, Aa::Native);
        set_scale(&mut s, 2.0 / 3.0);
        assert_eq!((upscaler(&s), aa(&s)), (Up::Fsr3, Aa::ByUpscaler));
    }
}
