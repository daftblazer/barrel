//! The renderer's settings (`docs/engine.md`, "Targets"): how big the scene is drawn and how it
//! reaches the output, anti-aliasing and the shadow map, with a preset for each tier. The
//! desktop preset draws exactly what the renderer drew before the settings existed.

use crate::shadow::Shadows;

/// How the scene, drawn at `scale` times the output, is brought to the output's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upscaler {
    /// The post pass's bilinear read.
    Bilinear,
    /// FSR 1: its edge-adaptive scale and sharpening (`upscale.rs`), when the scene is smaller.
    Fsr1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderSettings {
    /// The scene's size as a fraction (or multiple) of the output's, 0.25 to 2.
    pub scale: f32,
    pub upscaler: Upscaler,
    /// Samples a pixel the scene asks for (1, 2, 4 or 8; the device may allow fewer).
    pub msaa: u32,
    pub shadows: Shadows,
}

impl RenderSettings {
    /// The desktop: full size, no anti-aliasing, the large shadow map (the renderer as it was).
    pub const DESKTOP: RenderSettings = RenderSettings { scale: 1.0, upscaler: Upscaler::Bilinear, msaa: 1, shadows: Shadows::DESKTOP };
    /// The desktop with its quality options on: 4× MSAA.
    pub const DESKTOP_HIGH: RenderSettings = RenderSettings { msaa: 4, ..Self::DESKTOP };
    /// The handhelds: the small 16-bit shadow map, the scene at three quarters scaled up by
    /// FSR 1, and no MSAA (costly in their bandwidth; FSR 1 takes its place).
    pub const HANDHELD: RenderSettings = RenderSettings { scale: 0.75, upscaler: Upscaler::Fsr1, msaa: 1, shadows: Shadows::HANDHELD };

    /// A preset by name (`desktop`, `desktop-high`, `handheld`).
    pub fn preset(name: &str) -> Option<RenderSettings> {
        match name {
            "desktop" => Some(Self::DESKTOP),
            "desktop-high" => Some(Self::DESKTOP_HIGH),
            "handheld" => Some(Self::HANDHELD),
            _ => None,
        }
    }

    /// Whether FSR 1 runs: chosen and the scene smaller than the output.
    pub fn fsr1(&self) -> bool {
        self.upscaler == Upscaler::Fsr1 && self.scale < 1.0
    }

    /// The scene's size for an output of `width` × `height` (at least 1 × 1).
    pub fn scene_size(&self, width: u32, height: u32) -> (u32, u32) {
        (((width as f32 * self.scale).round() as u32).max(1), ((height as f32 * self.scale).round() as u32).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets() {
        assert_eq!(RenderSettings::preset("handheld"), Some(RenderSettings::HANDHELD));
        assert_eq!(RenderSettings::HANDHELD.scene_size(1920, 1080), (1440, 810));
        assert!(RenderSettings::HANDHELD.fsr1() && !RenderSettings::DESKTOP.fsr1());
        assert_eq!(RenderSettings::DESKTOP.scene_size(1280, 720), (1280, 720));
    }
}
