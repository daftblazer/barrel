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

/// How much of a field's plants a game draws, as its own grass presets name them (Ryza's
/// `grassqualitypreset.xml` / `grassrangepreset.xml`: Middle, Low, VeryLow).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foliage {
    Full,
    Reduced,
    Least,
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
    /// Anisotropic filtering of the scene's textures (most samples a lookup takes; 1 off). The
    /// game keeps the ground's cobbles sharp far along it, as anisotropic filtering does
    /// (docs/renderer.md, "Texture filtering"); the device's limit caps it.
    pub anisotropy: u32,
    /// How much of a field's plants to draw (a game's grass presets; config `foliage full |
    /// reduced | least`).
    pub foliage: Foliage,
    /// The scene drawn into a float target (`B10G11R11_UFLOAT`, the same size as 8-bit RGBA)
    /// instead of 8-bit RGBA, so what passes the half range's 2.0 keeps for the tone curve
    /// (`Frame::tone_white`). A game's choice, not a config entry (Ryza's trueHDR; Escha 8-bit).
    pub hdr_scene: bool,
    /// The screen-space ambient occlusion (`ao.rs`) for the games that have it (Ryza's: the
    /// capture's "PC High" has it on; Escha's frames never ask for it): a pass over the scene
    /// at half size before the main pass. Config `occlusion on | off`.
    pub occlusion: bool,
    /// The screen-space reflections (`reflect.rs`), Ryza's: the specular ambient mixed toward
    /// what the scene shows. A game's choice, not a config entry; it needs the normals and depth
    /// pre-pass (`ao.rs`, drawn when the occlusion, the reflections or the depth of field ask)
    /// and single-sampled drawing (it is left out with `msaa` above 1).
    pub reflections: bool,
    /// The depth of field's softening (`dof.rs`), Ryza's: the far view blurred by a weight of the
    /// depth. A game's choice, not a config entry; like the reflections it needs the normals and
    /// depth pre-pass (`ao.rs`) and single-sampled drawing.
    pub depth_of_field: bool,
    /// The reflections' march (`reflect::Rlr`: the game's quality level); unused without them.
    pub rlr: crate::reflect::Rlr,
}

impl RenderSettings {
    /// The desktop: full size, no anti-aliasing, the large shadow map (the renderer as it was).
    pub const DESKTOP: RenderSettings = RenderSettings { scale: 1.0, upscaler: Upscaler::Bilinear, msaa: 1, shadows: Shadows::DESKTOP, motion: false, jitter: false, taa: false, sharpen: false, gsr2: false, fsr3: false, anisotropy: 16, foliage: Foliage::Full, hdr_scene: false, occlusion: true, reflections: false, depth_of_field: false, rlr: crate::reflect::Rlr::HIGH };
    /// The desktop with its quality options on: 4× MSAA.
    pub const DESKTOP_HIGH: RenderSettings = RenderSettings { msaa: 4, ..Self::DESKTOP };
    /// The desktop with FSR 3.1 as anti-aliasing at full resolution (its "native AA").
    pub const DESKTOP_FSR3: RenderSettings = RenderSettings { fsr3: true, ..Self::DESKTOP };
    /// The handhelds with GSR 2 from two thirds instead of FSR 1 from three quarters.
    pub const HANDHELD_GSR2: RenderSettings = RenderSettings { scale: 2.0 / 3.0, upscaler: Upscaler::Bilinear, gsr2: true, ..Self::HANDHELD };
    /// The desktop with TAA in MSAA's place.
    pub const DESKTOP_TAA: RenderSettings = RenderSettings { taa: true, motion: true, jitter: true, sharpen: true, ..Self::DESKTOP };
    /// The handhelds: the small 16-bit shadow map, the scene at three quarters scaled up by
    /// FSR 1, and no MSAA (costly in their bandwidth; FSR 1 takes its place).
    pub const HANDHELD: RenderSettings = RenderSettings { scale: 0.75, upscaler: Upscaler::Fsr1, msaa: 1, shadows: Shadows::HANDHELD, motion: false, jitter: false, taa: false, sharpen: false, gsr2: false, fsr3: false, anisotropy: 4, foliage: Foliage::Reduced, hdr_scene: false, occlusion: false, reflections: false, depth_of_field: false, rlr: crate::reflect::Rlr::HIGH };

    /// A preset by name (`desktop`, `desktop-high`, `desktop-taa`, `desktop-fsr3`, `handheld`, `handheld-gsr2`).
    pub fn preset(name: &str) -> Option<RenderSettings> {
        match name {
            "desktop" => Some(Self::DESKTOP),
            "desktop-high" => Some(Self::DESKTOP_HIGH),
            "desktop-taa" => Some(Self::DESKTOP_TAA),
            "desktop-fsr3" => Some(Self::DESKTOP_FSR3),
            "handheld-gsr2" => Some(Self::HANDHELD_GSR2),
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

/// Barrel's graphics config (its own file, not the game's: the Options screen's graphics page
/// writes it): one `key value` a line, `scale`, `upscaler` (bilinear, fsr1, gsr1), `msaa`,
/// `shadows` (high, low), `taa`, `sharpen`, `gsr2`, `fsr3` (on, off), `anisotropy`, `foliage`
/// (full, reduced, least), `occlusion`, `reflections`, `depth_of_field` (on, off; the last two
/// Ryza's only).
impl RenderSettings {
    pub fn to_config(&self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        let upscaler = match self.upscaler {
            Upscaler::Bilinear => "bilinear",
            Upscaler::Fsr1 => "fsr1",
            Upscaler::Gsr1 => "gsr1",
        };
        let shadows = if self.shadows.size >= Shadows::DESKTOP.size { "high" } else { "low" };
        format!(
            "scale {}\nupscaler {upscaler}\nmsaa {}\nshadows {shadows}\ntaa {}\nsharpen {}\ngsr2 {}\nfsr3 {}\nanisotropy {}\nfoliage {}\nocclusion {}\nreflections {}\ndepth_of_field {}\n",
            self.scale, self.msaa, on(self.taa), on(self.sharpen), on(self.gsr2), on(self.fsr3), self.anisotropy,
            match self.foliage {
                Foliage::Full => "full",
                Foliage::Reduced => "reduced",
                Foliage::Least => "least",
            },
            on(self.occlusion),
            on(self.reflections),
            on(self.depth_of_field)
        )
    }

    /// The settings a config gives, over the desktop preset, resolved; unknown lines are skipped.
    /// The scene image's format (`hdr_scene`).
    pub fn scene_format(&self) -> ash::vk::Format {
        if self.hdr_scene { ash::vk::Format::B10G11R11_UFLOAT_PACK32 } else { crate::SCENE_FORMAT }
    }

    pub fn from_config(text: &str) -> RenderSettings {
        Self::from_config_over(Self::DESKTOP, text)
    }

    /// The settings a config gives over `base` (a game's own defaults), resolved.
    pub fn from_config_over(base: RenderSettings, text: &str) -> RenderSettings {
        let mut s = base;
        for line in text.lines() {
            let Some((key, value)) = line.trim().split_once(' ') else { continue };
            let value = value.trim();
            let on = value == "on";
            match key {
                "scale" => s.scale = value.parse::<f32>().map_or(s.scale, |v| v.clamp(0.25, 2.0)),
                "upscaler" => {
                    s.upscaler = match value {
                        "fsr1" => Upscaler::Fsr1,
                        "gsr1" => Upscaler::Gsr1,
                        _ => Upscaler::Bilinear,
                    }
                }
                "msaa" => s.msaa = value.parse::<u32>().map_or(1, |v| v.clamp(1, 8)),
                "shadows" => s.shadows = if value == "low" { Shadows::HANDHELD } else { Shadows::DESKTOP },
                "taa" => s.taa = on,
                "sharpen" => s.sharpen = on,
                "gsr2" => s.gsr2 = on,
                "fsr3" => s.fsr3 = on,
                "anisotropy" => s.anisotropy = value.parse::<u32>().map_or(s.anisotropy, |v| v.clamp(1, 16)),
                "occlusion" => s.occlusion = on,
                "reflections" => s.reflections = on,
                "depth_of_field" => s.depth_of_field = on,
                "foliage" => {
                    s.foliage = match value {
                        "reduced" => Foliage::Reduced,
                        "least" => Foliage::Least,
                        _ => Foliage::Full,
                    }
                }
                _ => {}
            }
        }
        s.resolved()
    }

    /// Where the config lives: `BARREL_CONFIG`, else `graphics.conf` in the game's config folder
    /// (`~/.config/barrel/<game>/graphics.conf`, see `barrel_files::config`).
    pub fn config_path() -> Option<std::path::PathBuf> {
        barrel_files::config::graphics_path()
    }
}

/// The window: its size, or borderless fullscreen on its screen (then the screen's own size).
/// In the same config as the render settings: `window WxH`, `fullscreen on|off`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Display {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

/// The window sizes the graphics page offers.
pub const WINDOW_SIZES: [(u32, u32); 7] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1080), (2560, 1440), (3440, 1440), (3840, 2160)];

impl Display {
    pub const DEFAULT: Display = Display { width: 1280, height: 720, fullscreen: false };

    pub fn to_config(&self) -> String {
        format!("window {}x{}\nfullscreen {}\n", self.width, self.height, if self.fullscreen { "on" } else { "off" })
    }

    pub fn from_config(text: &str) -> Display {
        let mut d = Self::DEFAULT;
        for line in text.lines() {
            match line.trim().split_once(' ') {
                Some(("window", v)) => {
                    if let Some((w, h)) = v.trim().split_once('x')
                        && let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            (d.width, d.height) = (w.clamp(320, 16384), h.clamp(200, 16384));
                        }
                }
                Some(("fullscreen", v)) => d.fullscreen = v.trim() == "on",
                _ => {}
            }
        }
        d
    }
}

/// How frames are paced to the window: vertical sync on (the swapchain waits for the display)
/// or off (frames go as fast as the loop draws them), and an optional frame rate limit. In the
/// same config as the render settings: `vsync on|off`, `frame_limit N|off`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pacing {
    pub vsync: bool,
    /// The most frames a second the loop presents (none: as the display and vsync allow).
    pub limit: Option<u32>,
}

impl Pacing {
    pub const DEFAULT: Pacing = Pacing { vsync: true, limit: None };

    pub fn to_config(&self) -> String {
        let limit = self.limit.map_or("off".to_string(), |n| n.to_string());
        format!("vsync {}\nframe_limit {limit}\n", if self.vsync { "on" } else { "off" })
    }

    pub fn from_config(text: &str) -> Pacing {
        Self::DEFAULT.over(text)
    }

    /// This pacing with what `text` (a config) sets laid over it: a game's own default where the
    /// config says nothing.
    pub fn over(self, text: &str) -> Pacing {
        let mut p = self;
        for line in text.lines() {
            match line.trim().split_once(' ') {
                Some(("vsync", v)) => p.vsync = v.trim() != "off",
                Some(("frame_limit", v)) => p.limit = v.trim().parse::<u32>().ok().filter(|n| *n > 0),
                _ => {}
            }
        }
        p
    }

    /// The time one frame takes at the limit.
    pub fn interval(&self) -> Option<std::time::Duration> {
        self.limit.map(|n| std::time::Duration::from_secs_f64(1.0 / f64::from(n)))
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

    #[test]
    fn configs_round_trip() {
        for s in [RenderSettings::DESKTOP, RenderSettings::HANDHELD, RenderSettings::DESKTOP_TAA, RenderSettings::HANDHELD_GSR2, RenderSettings::DESKTOP_FSR3].map(RenderSettings::resolved) {
            assert_eq!(RenderSettings::from_config(&s.to_config()), s);
        }
        assert_eq!(RenderSettings::from_config("msaa 4\nnonsense\n").msaa, 4);
        let d = Display { width: 2560, height: 1440, fullscreen: true };
        let text = d.to_config() + &RenderSettings::HANDHELD.resolved().to_config();
        assert_eq!((Display::from_config(&text), RenderSettings::from_config(&text)), (d, RenderSettings::HANDHELD.resolved()));
    }

    #[test]
    fn pacing_keys_and_limit_interval() {
        assert_eq!(Pacing::from_config(""), Pacing::DEFAULT);
        let p = Pacing { vsync: false, limit: Some(30) };
        assert_eq!(Pacing::from_config(&p.to_config()), p);
        assert_eq!(Pacing::from_config("frame_limit off\nvsync on\n"), Pacing::DEFAULT);
        assert_eq!(Pacing::from_config("frame_limit 0\n").limit, None);
        assert_eq!(p.interval(), Some(std::time::Duration::from_secs_f64(1.0 / 30.0)));
        assert_eq!(Pacing::DEFAULT.interval(), None);
    }
}
