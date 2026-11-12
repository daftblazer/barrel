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
    /// Snapdragon GSR 1: one pass that scales and sharpens (`upscale.rs`, `gsr1.frag`).
    Gsr1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderSettings {
    /// The scene's size as a fraction (or multiple) of the output's, 0.25 to 2.
    pub scale: f32,
    pub upscaler: Upscaler,
    /// Samples a pixel the scene asks for (1, 2, 4 or 8; the device may allow fewer).
    pub msaa: u32,
    pub shadows: Shadows,
    /// Whether the scene pass also writes motion vectors (a second colour target, `R16G16`:
    /// each pixel's movement since the last frame in texture coordinates), which the temporal
    /// methods read; it takes single-sampled drawing.
    pub motion: bool,
    /// Whether the camera is shifted by a different sub-pixel offset each frame ([`jitter`]),
    /// which the temporal methods gather into more detail than one frame has.
    pub jitter: bool,
    /// Temporal anti-aliasing (`taa.rs`): takes the motion vectors and the jitter, and
    /// single-sampled drawing ([`RenderSettings::resolved`] sees to all three).
    pub taa: bool,
    /// RCAS on the scene at the output's size too (FSR 1 always sharpens when it scales):
    /// offsets TAA's softness.
    pub sharpen: bool,
    /// Snapdragon GSR 2, the temporal upscaler (`gsr2.rs`): takes the motion vectors, the jitter
    /// and the depth, and replaces TAA, MSAA and the spatial upscalers.
    pub gsr2: bool,
    /// FSR 3.1's upscaler (`fsr3.rs`): the same inputs and replacements as GSR 2.
    pub fsr3: bool,
}

impl RenderSettings {
    /// The desktop: full size, no anti-aliasing, the large shadow map (the renderer as it was).
    pub const DESKTOP: RenderSettings = RenderSettings { scale: 1.0, upscaler: Upscaler::Bilinear, msaa: 1, shadows: Shadows::DESKTOP, motion: false, jitter: false, taa: false, sharpen: false, gsr2: false, fsr3: false };
    /// The desktop with its quality options on: 4× MSAA.
    pub const DESKTOP_HIGH: RenderSettings = RenderSettings { msaa: 4, ..Self::DESKTOP };
    /// The desktop with TAA in MSAA's place.
    pub const DESKTOP_TAA: RenderSettings = RenderSettings { taa: true, motion: true, jitter: true, sharpen: true, ..Self::DESKTOP };
    /// The handhelds: the small 16-bit shadow map, the scene at three quarters scaled up by
    /// FSR 1, and no MSAA (costly in their bandwidth; FSR 1 takes its place).
    pub const HANDHELD: RenderSettings = RenderSettings { scale: 0.75, upscaler: Upscaler::Fsr1, msaa: 1, shadows: Shadows::HANDHELD, motion: false, jitter: false, taa: false, sharpen: false, gsr2: false, fsr3: false };

    /// A preset by name (`desktop`, `desktop-high`, `desktop-taa`, `handheld`).
    pub fn preset(name: &str) -> Option<RenderSettings> {
        match name {
            "desktop" => Some(Self::DESKTOP),
            "desktop-high" => Some(Self::DESKTOP_HIGH),
            "desktop-taa" => Some(Self::DESKTOP_TAA),
            "handheld" => Some(Self::HANDHELD),
            _ => None,
        }
    }

    /// These settings with what they need of each other: TAA turns on the motion vectors and
    /// the jitter and takes single-sampled drawing.
    pub fn resolved(mut self) -> RenderSettings {
        if self.fsr3 {
            self.gsr2 = false;
        }
        if self.gsr2 || self.fsr3 {
            self.taa = true;
        }
        if self.taa {
            self.motion = true;
            self.jitter = true;
            self.msaa = 1;
        }
        if self.gsr2 || self.fsr3 {
            self.taa = false;
            self.sharpen = false;
            self.upscaler = Upscaler::Bilinear;
        }
        self
    }

    /// Whether RCAS runs on its own: sharpening asked for, the scene at the output's size.
    pub fn sharpen_only(&self) -> bool {
        self.sharpen && !self.upscales()
    }

    /// Whether a spatial upscaler (FSR 1 or GSR 1) runs: chosen and the scene smaller than the
    /// output.
    pub fn upscales(&self) -> bool {
        self.upscaler != Upscaler::Bilinear && self.scale < 1.0
    }

    /// The scene's size for an output of `width` × `height` (at least 1 × 1).
    pub fn scene_size(&self, width: u32, height: u32) -> (u32, u32) {
        (((width as f32 * self.scale).round() as u32).max(1), ((height as f32 * self.scale).round() as u32).max(1))
    }
}

/// Whether the scene pass keeps its depth for a temporal upscaler to read.
impl RenderSettings {
    pub fn keeps_depth(&self) -> bool {
        self.gsr2 || self.fsr3
    }
}

/// Frame `n`'s offset in a Halton cycle of `length` frames (FSR 3.1 sets its own length).
pub fn jitter_cycle(n: u32, length: u32) -> (f32, f32) {
    let i = n % length.max(1) + 1;
    (halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

fn halton(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// Frame `n`'s sub-pixel offset, in pixels from -0.5 to 0.5 each way: the Halton sequence in
/// bases 2 and 3 over a cycle of 16 (as FSR 2 and most TAA use).
pub fn jitter(n: u32) -> (f32, f32) {
    let halton = |mut i: u32, base: u32| {
        let (mut f, mut r) = (1.0f32, 0.0f32);
        while i > 0 {
            f /= base as f32;
            r += f * (i % base) as f32;
            i /= base;
        }
        r
    };
    let i = n % 16 + 1;
    (halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets() {
        assert_eq!(RenderSettings::preset("handheld"), Some(RenderSettings::HANDHELD));
        assert_eq!(RenderSettings::HANDHELD.scene_size(1920, 1080), (1440, 810));
        assert!(RenderSettings::HANDHELD.upscales() && !RenderSettings::DESKTOP.upscales());
        assert_eq!(RenderSettings::DESKTOP.scene_size(1280, 720), (1280, 720));
        // Halton 2, 3 from 1: (1/2, 1/3), (1/4, 2/3), (3/4, 1/9), ...
        assert_eq!(jitter(0), (0.0, 1.0 / 3.0 - 0.5));
        assert_eq!(jitter(1), (-0.25, 2.0 / 3.0 - 0.5));
        assert_eq!(jitter(16), jitter(0));
    }
}
