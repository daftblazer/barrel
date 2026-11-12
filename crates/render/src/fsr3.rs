//! FSR 3.1's upscaler (`docs/engine.md`, "FSR 3.1"): the CPU side of AMD's passes (vendored in
//! `shaders/ffx/`), ported from the FidelityFX SDK's `ffx_fsr3upscaler.cpp` (MIT, Copyright (C)
//! 2024 Advanced Micro Devices, Inc.): the constant buffers, the Lanczos table, the jitter
//! sequence's length and the luma pyramid's (SPD) setup. The GPU side builds on these.

use std::f32::consts::PI;

/// The main constant buffer (`cbFSR3Upscaler` in `ffx_fsr3upscaler_callbacks_glsl.h`, std140).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Constants {
    pub render_size: [i32; 2],
    pub previous_render_size: [i32; 2],
    pub upscale_size: [i32; 2],
    pub previous_upscale_size: [i32; 2],
    pub max_render_size: [i32; 2],
    pub max_upscale_size: [i32; 2],
    pub device_to_view_depth: [f32; 4],
    pub jitter: [f32; 2],
    pub previous_jitter: [f32; 2],
    pub motion_vector_scale: [f32; 2],
    pub downscale_factor: [f32; 2],
    pub motion_vector_jitter_cancellation: [f32; 2],
    pub tan_half_fov: f32,
    pub jitter_sequence_length: f32,
    pub delta_time: f32,
    pub delta_pre_exposure: f32,
    pub view_space_to_meters: f32,
    pub frame_index: f32,
    pub velocity_factor: f32,
    pub reactiveness_scale: f32,
    pub shading_change_scale: f32,
    pub accumulation_added_per_frame: f32,
    pub min_disocclusion_accumulation: f32,
}

/// One frame's inputs (`FfxFsr3UpscalerDispatchDescription`'s, the ones Barrel sets).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub render: (u32, u32),
    pub upscale: (u32, u32),
    /// This frame's jitter in render pixels.
    pub jitter: (f32, f32),
    /// The vertical field of view, in radians.
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    /// Seconds since the last frame.
    pub dt: f32,
    pub reset: bool,
}

impl Constants {
    /// A new context's constants (the SDK's defaults for the tuning values).
    pub fn new() -> Constants {
        Constants {
            velocity_factor: 1.0,
            reactiveness_scale: 1.0,
            shading_change_scale: 1.0,
            accumulation_added_per_frame: 1.0 / 3.0,
            min_disocclusion_accumulation: -1.0 / 3.0,
            ..Constants::default()
        }
    }

    /// Advances to frame `f`, as `fsr3upscalerDispatch` does. Barrel's motion vectors are in
    /// texture units (current minus previous) without the jitter; FSR 3's are previous minus
    /// current, scaled by `motion_vector_scale` into texture units, hence the -1.
    pub fn advance(&mut self, f: &Frame) {
        let (rw, rh) = (f.render.0 as i32, f.render.1 as i32);
        let (uw, uh) = (f.upscale.0 as i32, f.upscale.1 as i32);
        self.previous_jitter = self.jitter;
        self.jitter = [f.jitter.0, f.jitter.1];
        self.previous_render_size = self.render_size;
        self.render_size = [rw, rh];
        self.max_render_size = [rw, rh];
        self.previous_upscale_size = self.upscale_size;
        self.upscale_size = [uw, uh];
        self.max_upscale_size = [uw, uh];
        let aspect = rw as f32 / rh as f32;
        let horizontal = ((f.fov / 2.0).tan() * aspect).atan() * 2.0;
        self.tan_half_fov = (horizontal * 0.5).tan();
        self.view_space_to_meters = 1.0;
        self.device_to_view_depth = device_to_view_depth(f.near, f.far, f.fov, aspect);
        self.downscale_factor = [rw as f32 / uw as f32, rh as f32 / uh as f32];
        self.delta_pre_exposure = 1.0;
        self.motion_vector_scale = [-1.0, -1.0];
        self.motion_vector_jitter_cancellation = [0.0, 0.0];
        let phases = jitter_phase_count(rw, uw) as f32;
        if f.reset || self.jitter_sequence_length == 0.0 {
            self.jitter_sequence_length = phases;
        } else if phases > self.jitter_sequence_length {
            self.jitter_sequence_length += 1.0;
        } else if phases < self.jitter_sequence_length {
            self.jitter_sequence_length -= 1.0;
        }
        self.delta_time = f.dt.clamp(0.0, 1.0);
        self.frame_index = if f.reset { 0.0 } else { self.frame_index + 1.0 };
    }

    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self as *const Constants as *const u8, std::mem::size_of::<Constants>()) }
    }
}

/// `setupDeviceDepthToViewSpaceDepthParams` for depth neither inverted nor infinite.
pub fn device_to_view_depth(near: f32, far: f32, fov: f32, aspect: f32) -> [f32; 4] {
    let (lo, hi) = (near.min(far), near.max(far));
    let q = hi / (lo - hi);
    let cot = (0.5 * fov).cos() / (0.5 * fov).sin();
    [-q, q * lo, aspect / cot, 1.0 / cot]
}

/// How many jitter phases the accumulation expects: 8 × the upscale ratio squared.
pub fn jitter_phase_count(render_width: i32, display_width: i32) -> i32 {
    (8.0 * (display_width as f32 / render_width as f32).powi(2)) as i32
}

fn lanczos2(x: f32) -> f32 {
    if x.abs() < f32::EPSILON {
        1.0
    } else {
        ((PI * x).sin() / (PI * x)) * ((0.5 * PI * x).sin() / (0.5 * PI * x))
    }
}

/// The Lanczos-2 table the reprojection reads (R16_SNORM, 128 wide, over 0..2).
pub fn lanczos_lut() -> [i16; 128] {
    std::array::from_fn(|i| (lanczos2(2.0 * i as f32 / 127.0) * 32767.0).round() as i16)
}

/// The luma pyramid's setup (`ffxSpdSetup` for the whole render): thread groups x and y, and
/// the constant buffer (`cbSPD_t`: mips, groups, offset, render size).
pub fn spd_setup(width: u32, height: u32) -> ([u32; 2], [u32; 6]) {
    let groups = [(width - 1) / 64 + 1, (height - 1) / 64 + 1];
    let mips = (width.max(height) as f32).log2().floor().min(12.0) as u32;
    (groups, [mips, groups[0] * groups[1], 0, 0, width, height])
}

fn f16_bits(v: f32) -> u32 {
    // Positive normal values only (RCAS's scale is in (0, 1]).
    let b = v.to_bits();
    let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let mant = (b >> 13) & 0x3ff;
    if exp <= 0 {
        0
    } else {
        ((exp as u32) << 10) | mant
    }
}

/// RCAS's constants (`FsrRcasCon`) for a sharpness from 0 (off) to 1 (most), remapped as the
/// SDK does to 2 → 0 stops.
pub fn rcas_config(sharpness: f32) -> [u32; 4] {
    let scale = (-(2.0 - 2.0 * sharpness)).exp2();
    let h = f16_bits(scale);
    [scale.to_bits(), h | (h << 16), 0, 0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        // std140: six ivec2 (48), a vec4 (16), five vec2 (40), eleven floats (44).
        assert_eq!(std::mem::size_of::<Constants>(), 148);
    }

    #[test]
    fn tables() {
        let lut = lanczos_lut();
        assert_eq!(lut[0], 32767);
        assert_eq!(lut[127], 0);
        // Lanczos-2 dips below zero between 1 and 2.
        assert!(lut[90] < 0);
        // 1080p from 720p: 1.5² × 8 = 18 phases; from half: 32.
        assert_eq!(jitter_phase_count(1280, 1920), 18);
        assert_eq!(jitter_phase_count(960, 1920), 32);
        let (groups, cb) = spd_setup(1280, 720);
        assert_eq!(groups, [20, 12]);
        assert_eq!(cb[..2], [10, 240]);
        // Full sharpness: scale 1, its half 0x3c00.
        assert_eq!(rcas_config(1.0)[..2], [1.0f32.to_bits(), 0x3c00_3c00]);
    }

    #[test]
    fn frames() {
        let mut c = Constants::new();
        let f = Frame { render: (960, 540), upscale: (1920, 1080), jitter: (0.25, -0.25), fov: 22.5f32.to_radians(), near: 10.0, far: 10000.0, dt: 1.0 / 60.0, reset: true };
        c.advance(&f);
        assert_eq!((c.frame_index, c.jitter_sequence_length), (0.0, 32.0));
        assert_eq!(c.downscale_factor, [0.5, 0.5]);
        c.advance(&Frame { reset: false, jitter: (-0.1, 0.1), ..f });
        assert_eq!((c.frame_index, c.previous_jitter), (1.0, [0.25, -0.25]));
    }
}
