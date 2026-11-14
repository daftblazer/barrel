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

// ---------------------------------------------------------------------------------------------
// The GPU side: the passes, their resources and the frame's recording.

use ash::vk;

use crate::{Gpu, Result};

/// A binding's kind, as the passes declare them (reflected from their SPIR-V).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Sampled,
    Storage,
    Uniform,
    Sampler,
}

/// What a binding is bound to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Res {
    // Barrel's inputs.
    Color,
    Depth,
    Motion,
    // Internal, one image each.
    DilatedMotion,
    DilatedDepth,
    ReconstructedPrevDepth,
    NewLocks,
    SpdAtomic,
    FarthestMip1,
    ShadingChange,
    DilatedReactive,
    /// Shared by farthest depth and luma instability.
    Intermediate,
    FrameInfo,
    DefaultExposure,
    DefaultReactive,
    DefaultTransparency,
    Lut,
    Output,
    /// All the SPD mips (sampled), or one of them (storage).
    SpdMips,
    SpdMip(u32),
    // Pairs swapped each frame: this frame's and last frame's.
    LumaCurrent,
    LumaPrevious,
    LumaHistoryRead,
    LumaHistoryWrite,
    AccumulationRead,
    AccumulationWrite,
    UpscaledRead,
    UpscaledWrite,
    // Constant buffers and the linear sampler.
    MainConstants,
    SpdConstants,
    RcasConstants,
    LinearSampler,
}

use Kind::*;
use Res::*;

struct PassDesc {
    name: &'static str,
    spirv: &'static [u8],
    bindings: &'static [(u32, Kind, Res)],
}

macro_rules! spirv {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/fsr3_", $name, ".spv"))
    };
}

/// The passes in dispatch order, their bindings as reflected from the compiled shaders
/// (bindings the compiler dropped are left out).
const PASSES: [PassDesc; 8] = [
    PassDesc {
        name: "prepare_inputs",
        spirv: spirv!("prepare_inputs"),
        bindings: &[
            (0, Sampled, Motion),
            (1, Sampled, Depth),
            (2, Sampled, Color),
            (3, Storage, DilatedMotion),
            (4, Storage, DilatedDepth),
            (5, Storage, ReconstructedPrevDepth),
            (6, Storage, Intermediate),
            (7, Storage, LumaCurrent),
            (8, Uniform, MainConstants),
        ],
    },
    PassDesc {
        name: "luma_pyramid",
        spirv: spirv!("luma_pyramid"),
        bindings: &[
            (0, Sampled, LumaCurrent),
            (1, Sampled, Intermediate),
            (2, Storage, SpdAtomic),
            (3, Storage, FrameInfo),
            (9, Storage, SpdMip(5)),
            (10, Storage, FarthestMip1),
            (11, Uniform, MainConstants),
            (12, Uniform, SpdConstants),
        ],
    },
    PassDesc {
        name: "shading_change_pyramid",
        spirv: spirv!("shading_change_pyramid"),
        bindings: &[
            (0, Sampled, LumaCurrent),
            (1, Sampled, LumaPrevious),
            (2, Sampled, DilatedMotion),
            (3, Sampled, DefaultExposure),
            (4, Storage, SpdAtomic),
            (5, Storage, SpdMip(0)),
            (6, Storage, SpdMip(1)),
            (7, Storage, SpdMip(2)),
            (8, Storage, SpdMip(3)),
            (9, Storage, SpdMip(4)),
            (10, Storage, SpdMip(5)),
            (11, Uniform, MainConstants),
            (12, Uniform, SpdConstants),
        ],
    },
    PassDesc {
        name: "shading_change",
        spirv: spirv!("shading_change"),
        bindings: &[(0, Sampled, SpdMips), (1, Storage, ShadingChange), (2, Uniform, MainConstants), (1001, Sampler, LinearSampler)],
    },
    PassDesc {
        name: "prepare_reactivity",
        spirv: spirv!("prepare_reactivity"),
        bindings: &[
            (0, Sampled, ReconstructedPrevDepth),
            (1, Sampled, DilatedMotion),
            (2, Sampled, DilatedDepth),
            (3, Sampled, DefaultReactive),
            (4, Sampled, DefaultTransparency),
            (5, Sampled, AccumulationRead),
            (6, Sampled, ShadingChange),
            (7, Sampled, LumaCurrent),
            (8, Sampled, DefaultExposure),
            (9, Storage, DilatedReactive),
            (10, Storage, NewLocks),
            (11, Storage, AccumulationWrite),
            (12, Uniform, MainConstants),
            (1001, Sampler, LinearSampler),
        ],
    },
    PassDesc {
        name: "luma_instability",
        spirv: spirv!("luma_instability"),
        bindings: &[
            (0, Sampled, DefaultExposure),
            (1, Sampled, DilatedReactive),
            (2, Sampled, DilatedMotion),
            (4, Sampled, LumaHistoryRead),
            (6, Sampled, LumaCurrent),
            (7, Storage, LumaHistoryWrite),
            (8, Storage, Intermediate),
            (9, Uniform, MainConstants),
            (1001, Sampler, LinearSampler),
        ],
    },
    PassDesc {
        name: "accumulate",
        spirv: spirv!("accumulate"),
        bindings: &[
            (0, Sampled, DefaultExposure),
            (1, Sampled, DilatedReactive),
            // Render-size motion vectors: the dilated ones prepare_inputs made.
            (2, Sampled, DilatedMotion),
            (3, Sampled, UpscaledRead),
            (4, Sampled, Lut),
            (5, Sampled, FarthestMip1),
            (7, Sampled, Intermediate),
            (8, Sampled, Color),
            (9, Storage, UpscaledWrite),
            (10, Storage, Output),
            (11, Storage, NewLocks),
            (12, Uniform, MainConstants),
            (1001, Sampler, LinearSampler),
        ],
    },
    PassDesc {
        name: "rcas",
        spirv: spirv!("rcas"),
        bindings: &[(0, Sampled, DefaultExposure), (1, Sampled, UpscaledWrite), (2, Storage, Output), (4, Uniform, RcasConstants)],
    },
];

const PREPARE_INPUTS: usize = 0;
const LUMA_PYRAMID: usize = 1;
const SHADING_CHANGE_PYRAMID: usize = 2;
const SHADING_CHANGE: usize = 3;
const PREPARE_REACTIVITY: usize = 4;
const LUMA_INSTABILITY: usize = 5;
const ACCUMULATE: usize = 6;
const RCAS: usize = 7;

/// RCAS's sharpness (0 off to 1 most): the SDK samples' default.
pub const SHARPNESS: f32 = 0.8;

pub struct Pipelines {
    set_layouts: Vec<vk::DescriptorSetLayout>,
    layouts: Vec<vk::PipelineLayout>,
    pipelines: Vec<vk::Pipeline>,
    linear: vk::Sampler,
}

impl Pipelines {
    pub fn new(gpu: &Gpu) -> Result<Pipelines> {
        let d = &gpu.device;
        let (mut set_layouts, mut layouts, mut pipelines) = (vec![], vec![], vec![]);
        let linear = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .max_lod(vk::LOD_CLAMP_NONE)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?
        };
        for pass in &PASSES {
            let bindings: Vec<_> = pass
                .bindings
                .iter()
                .map(|&(b, kind, _)| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(b)
                        .descriptor_type(descriptor_type(kind))
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::COMPUTE)
                })
                .collect();
            let set_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)? };
            let sets = [set_layout];
            let layout = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sets), None)? };
            let words: Vec<u32> = pass.spirv.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
            let module = unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)? };
            let stage = vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::COMPUTE).module(module).name(c"main");
            let pipeline = unsafe {
                d.create_compute_pipelines(vk::PipelineCache::null(), &[vk::ComputePipelineCreateInfo::default().stage(stage).layout(layout)], None)
                    .map_err(|(_, e)| format!("FSR 3 {}: {e}", pass.name))?[0]
            };
            unsafe { d.destroy_shader_module(module, None) };
            set_layouts.push(set_layout);
            layouts.push(layout);
            pipelines.push(pipeline);
        }
        Ok(Pipelines { set_layouts, layouts, pipelines, linear })
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            for p in self.pipelines {
                d.destroy_pipeline(p, None);
            }
            for l in self.layouts {
                d.destroy_pipeline_layout(l, None);
            }
            for l in self.set_layouts {
                d.destroy_descriptor_set_layout(l, None);
            }
            d.destroy_sampler(self.linear, None);
        }
    }
}

fn descriptor_type(kind: Kind) -> vk::DescriptorType {
    match kind {
        Sampled => vk::DescriptorType::SAMPLED_IMAGE,
        Storage => vk::DescriptorType::STORAGE_IMAGE,
        Uniform => vk::DescriptorType::UNIFORM_BUFFER,
        Sampler => vk::DescriptorType::SAMPLER,
    }
}

struct Image {
    image: vk::Image,
    memory: vk::DeviceMemory,
    /// The whole image, then (for the SPD mips) one view per level.
    views: Vec<vk::ImageView>,
}

/// The SPD mips the passes bind (levels 0 to 5).
const SPD_LEVELS: u32 = 6;
const CB_STRIDE: u64 = 256;

pub struct Targets {
    pool: vk::DescriptorPool,
    /// Each pass's set for even and odd frames.
    sets: Vec<[vk::DescriptorSet; 2]>,
    images: Vec<Image>,
    /// Indices into `images`.
    index: std::collections::HashMap<&'static str, usize>,
    constants: (vk::Buffer, vk::DeviceMemory),
    lut_staging: (vk::Buffer, vk::DeviceMemory),
    render: (u32, u32),
    output: (u32, u32),
    pub state: Constants,
    frame: u32,
    first: bool,
}

impl Targets {
    /// The targets for a render of `render` pixels upscaled to `output`, reading the scene, its
    /// depth and motion vectors (all in `SHADER_READ_ONLY_OPTIMAL`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(gpu: &Gpu, p: &Pipelines, scene: vk::ImageView, depth: vk::ImageView, motion: vk::ImageView, render: (u32, u32), output: (u32, u32)) -> Result<Targets> {
        let d = &gpu.device;
        let half = ((render.0 / 2).max(1), (render.1 / 2).max(1));
        let spd_levels = ((half.0.max(half.1) as f32).log2().floor() as u32 + 1).max(SPD_LEVELS);
        // name, format, size, mip levels.
        let specs: [(&str, vk::Format, (u32, u32), u32); 23] = [
            ("dilated_motion", vk::Format::R16G16_SFLOAT, render, 1),
            ("dilated_depth", vk::Format::R32_SFLOAT, render, 1),
            ("reconstructed_prev_depth", vk::Format::R32_UINT, render, 1),
            ("new_locks", vk::Format::R8_UNORM, output, 1),
            ("spd_atomic", vk::Format::R32_UINT, (1, 1), 1),
            ("farthest_mip1", vk::Format::R16_SFLOAT, half, 1),
            ("shading_change", vk::Format::R8_UNORM, half, 1),
            ("dilated_reactive", vk::Format::R8G8B8A8_UNORM, render, 1),
            ("intermediate", vk::Format::R16_SFLOAT, render, 1),
            ("frame_info", vk::Format::R32G32B32A32_SFLOAT, (1, 1), 1),
            ("default_exposure", vk::Format::R32G32_SFLOAT, (1, 1), 1),
            ("default_reactive", vk::Format::R8_UNORM, (1, 1), 1),
            ("default_transparency", vk::Format::R8_UNORM, (1, 1), 1),
            ("lut", vk::Format::R16_SNORM, (128, 1), 1),
            ("output", vk::Format::R16G16B16A16_SFLOAT, output, 1),
            ("spd_mips", vk::Format::R16G16_SFLOAT, half, spd_levels),
            ("luma_0", vk::Format::R16_SFLOAT, render, 1),
            ("luma_1", vk::Format::R16_SFLOAT, render, 1),
            // RGBA8, as the shaders declare it (the SDK makes it RGBA16F).
            ("luma_history_0", vk::Format::R8G8B8A8_UNORM, render, 1),
            ("luma_history_1", vk::Format::R8G8B8A8_UNORM, render, 1),
            ("accumulation_0", vk::Format::R8_UNORM, render, 1),
            ("accumulation_1", vk::Format::R8_UNORM, render, 1),
            ("upscaled_0", vk::Format::R16G16B16A16_SFLOAT, output, 1),
        ];
        let mut specs = specs.to_vec();
        specs.push(("upscaled_1", vk::Format::R16G16B16A16_SFLOAT, output, 1));
        let mut images = vec![];
        let mut index = std::collections::HashMap::new();
        for (name, format, size, levels) in specs {
            let (image, memory) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(vk::Extent3D { width: size.0, height: size.1, depth: 1 })
                    .mip_levels(levels)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST),
            )?;
            let view = |base: u32, count: u32| unsafe {
                d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).base_mip_level(base).level_count(count).layer_count(1)),
                    None,
                )
            };
            let mut views = vec![view(0, levels)?];
            if levels > 1 {
                for l in 0..SPD_LEVELS {
                    views.push(view(l, 1)?);
                }
            }
            index.insert(name, images.len());
            images.push(Image { image, memory, views });
        }
        // Three constant buffers (main, SPD, RCAS) for each parity: a frame writes its own
        // parity's while the GPU may still read the frame before's (frames in flight).
        let (cb, cb_mem) = gpu.buffer(6 * CB_STRIDE, vk::BufferUsageFlags::UNIFORM_BUFFER, true)?;
        let lut: Vec<u8> = lanczos_lut().iter().flat_map(|v| v.to_le_bytes()).collect();
        let (staging, staging_mem) = gpu.buffer(lut.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        gpu.upload(staging_mem, &lut)?;

        let total: u32 = PASSES.iter().map(|p| p.bindings.len() as u32).sum::<u32>() * 2;
        let sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: total },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_IMAGE, descriptor_count: total },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: total },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: total },
        ];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(PASSES.len() as u32 * 2).pool_sizes(&sizes), None)? };
        let mut t = Targets {
            pool,
            sets: vec![],
            images,
            index,
            constants: (cb, cb_mem),
            lut_staging: (staging, staging_mem),
            render,
            output,
            state: Constants::new(),
            frame: 0,
            first: true,
        };
        for (k, pass) in PASSES.iter().enumerate() {
            let layouts = [p.set_layouts[k], p.set_layouts[k]];
            let s = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))? };
            for (parity, &set) in s.iter().enumerate() {
                let mut images = vec![];
                let mut buffers = vec![];
                let mut samplers = vec![];
                for &(_, kind, res) in pass.bindings {
                    match kind {
                        Sampled | Storage => {
                            let (view, layout) = match res {
                                Color => (scene, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                                Depth => (depth, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                                Motion => (motion, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                                _ => (t.view(res, parity), vk::ImageLayout::GENERAL),
                            };
                            images.push([vk::DescriptorImageInfo::default().image_view(view).image_layout(layout)]);
                        }
                        Uniform => {
                            let slot = match res {
                                MainConstants => 0,
                                SpdConstants => 1,
                                _ => 2,
                            };
                            buffers.push([vk::DescriptorBufferInfo::default().buffer(cb).offset((3 * parity as u64 + slot) * CB_STRIDE).range(CB_STRIDE)]);
                        }
                        Sampler => samplers.push([vk::DescriptorImageInfo::default().sampler(p.linear)]),
                    }
                }
                let (mut ii, mut bi, mut si) = (0, 0, 0);
                let writes: Vec<_> = pass
                    .bindings
                    .iter()
                    .map(|&(binding, kind, _)| {
                        let w = vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(descriptor_type(kind));
                        match kind {
                            Sampled | Storage => {
                                ii += 1;
                                w.image_info(&images[ii - 1])
                            }
                            Uniform => {
                                bi += 1;
                                w.buffer_info(&buffers[bi - 1])
                            }
                            Sampler => {
                                si += 1;
                                w.image_info(&samplers[si - 1])
                            }
                        }
                    })
                    .collect();
                unsafe { d.update_descriptor_sets(&writes, &[]) };
            }
            t.sets.push([s[0], s[1]]);
        }
        Ok(t)
    }

    fn image(&self, name: &str) -> &Image {
        &self.images[self.index[name]]
    }

    /// The view bound for `res` on frames of `parity` (0 even, 1 odd), following the SDK's
    /// swaps: on even frames luma 0 is this frame's and accumulation 0, history 0 and upscaled 0
    /// are read while their 1 is written; odd frames the other way round.
    fn view(&self, res: Res, parity: usize) -> vk::ImageView {
        let pair = |name: &str, k: usize| self.images[self.index[format!("{name}_{k}").as_str()]].views[0];
        let (this, other) = (parity, 1 - parity);
        match res {
            DilatedMotion => self.image("dilated_motion").views[0],
            DilatedDepth => self.image("dilated_depth").views[0],
            ReconstructedPrevDepth => self.image("reconstructed_prev_depth").views[0],
            NewLocks => self.image("new_locks").views[0],
            SpdAtomic => self.image("spd_atomic").views[0],
            FarthestMip1 => self.image("farthest_mip1").views[0],
            ShadingChange => self.image("shading_change").views[0],
            DilatedReactive => self.image("dilated_reactive").views[0],
            Intermediate => self.image("intermediate").views[0],
            FrameInfo => self.image("frame_info").views[0],
            DefaultExposure => self.image("default_exposure").views[0],
            DefaultReactive => self.image("default_reactive").views[0],
            DefaultTransparency => self.image("default_transparency").views[0],
            Lut => self.image("lut").views[0],
            Output => self.image("output").views[0],
            SpdMips => self.image("spd_mips").views[0],
            SpdMip(l) => self.image("spd_mips").views[1 + l as usize],
            LumaCurrent => pair("luma", this),
            LumaPrevious => pair("luma", other),
            LumaHistoryRead => pair("luma_history", this),
            LumaHistoryWrite => pair("luma_history", other),
            AccumulationRead => pair("accumulation", this),
            AccumulationWrite => pair("accumulation", other),
            UpscaledRead => pair("upscaled", this),
            UpscaledWrite => pair("upscaled", other),
            _ => unreachable!("not an internal image: {res:?}"),
        }
    }

    /// The upscaled scene, for the post pass to read (`SHADER_READ_ONLY_OPTIMAL` after
    /// `record`).
    pub fn result(&self) -> vk::ImageView {
        self.image("output").views[0]
    }

    /// Records the frame's passes. `frame`'s sizes are this targets' own.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene's pass (colour, depth and
    /// motion ready to sample).
    pub unsafe fn record(&mut self, gpu: &Gpu, p: &Pipelines, cb: vk::CommandBuffer, frame: &Frame) -> Result<()> {
        let d = &gpu.device;
        let reset = frame.reset || self.first;
        self.state.advance(&Frame { reset, ..*frame });
        let parity = (self.frame & 1) as usize;
        // The constant buffers: main, SPD, RCAS.
        let (groups, spd) = spd_setup(self.render.0, self.render.1);
        let base = 3 * parity as u64 * CB_STRIDE;
        gpu.upload_at(self.constants.1, base, self.state.bytes())?;
        gpu.upload_at(self.constants.1, base + CB_STRIDE, &spd.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())?;
        gpu.upload_at(self.constants.1, base + 2 * CB_STRIDE, &rcas_config(SHARPNESS).iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())?;

        let all = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(vk::REMAINING_MIP_LEVELS).layer_count(1);
        let to_general = |image: vk::Image, old: vk::ImageLayout| {
            vk::ImageMemoryBarrier::default()
                .image(image)
                .old_layout(old)
                .new_layout(vk::ImageLayout::GENERAL)
                .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE | vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(all)
        };
        let memory = |src: vk::PipelineStageFlags, src_access: vk::AccessFlags, dst: vk::PipelineStageFlags, dst_access: vk::AccessFlags| unsafe {
            d.cmd_pipeline_barrier(cb, src, dst, vk::DependencyFlags::empty(), &[vk::MemoryBarrier::default().src_access_mask(src_access).dst_access_mask(dst_access)], &[], &[]);
        };
        let clear_f = |name: &str, v: [f32; 4]| unsafe {
            d.cmd_clear_color_image(cb, self.image(name).image, vk::ImageLayout::GENERAL, &vk::ClearColorValue { float32: v }, &[all]);
        };
        let clear_u = |name: &str, v: u32| unsafe {
            d.cmd_clear_color_image(cb, self.image(name).image, vk::ImageLayout::GENERAL, &vk::ClearColorValue { uint32: [v; 4] }, &[all]);
        };
        unsafe {
            if self.first {
                // Everything into GENERAL for good; the output goes back to it each frame below.
                let barriers: Vec<_> = self.images.iter().map(|i| to_general(i.image, vk::ImageLayout::UNDEFINED).src_access_mask(vk::AccessFlags::empty())).collect();
                d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::COMPUTE_SHADER, vk::DependencyFlags::empty(), &[], &[], &barriers);
                for name in ["luma_0", "luma_1", "accumulation_0", "accumulation_1", "luma_history_0", "luma_history_1", "upscaled_0", "upscaled_1", "default_reactive", "default_transparency"] {
                    clear_f(name, [0.0; 4]);
                }
                clear_f("default_exposure", [1.0, 1.0, 0.0, 0.0]);
                let layers = vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1);
                let region = vk::BufferImageCopy::default().image_subresource(layers).image_extent(vk::Extent3D { width: 128, height: 1, depth: 1 });
                d.cmd_copy_buffer_to_image(cb, self.lut_staging.0, self.image("lut").image, vk::ImageLayout::GENERAL, &[region]);
            } else {
                d.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[to_general(self.image("output").image, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL).src_access_mask(vk::AccessFlags::SHADER_READ)],
                );
                memory(vk::PipelineStageFlags::COMPUTE_SHADER, vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE, vk::PipelineStageFlags::TRANSFER, vk::AccessFlags::TRANSFER_WRITE);
            }
            if reset {
                clear_f(&format!("accumulation_{parity}"), [0.0; 4]);
                clear_f("spd_mips", [0.0; 4]);
                clear_f("frame_info", [-1.0, 1.0, 0.0, 0.0]);
            }
            // Each frame: the reconstructed depth to 1.0 (its bits: atomic minimum), the counter to 0.
            clear_u("reconstructed_prev_depth", 1.0f32.to_bits());
            clear_u("spd_atomic", 0);
            memory(
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
            );
            let src = (self.render.0.div_ceil(8), self.render.1.div_ceil(8));
            let dst = (self.output.0.div_ceil(8), self.output.1.div_ceil(8));
            let dispatch = |k: usize, (x, y): (u32, u32)| {
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, p.pipelines[k]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::COMPUTE, p.layouts[k], 0, &[self.sets[k][parity]], &[]);
                d.cmd_dispatch(cb, x, y, 1);
                memory(
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::AccessFlags::SHADER_WRITE,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                );
            };
            dispatch(PREPARE_INPUTS, src);
            dispatch(LUMA_PYRAMID, (groups[0], groups[1]));
            dispatch(SHADING_CHANGE_PYRAMID, (groups[0], groups[1]));
            dispatch(SHADING_CHANGE, ((self.render.0 / 2).div_ceil(8), (self.render.1 / 2).div_ceil(8)));
            dispatch(PREPARE_REACTIVITY, src);
            dispatch(LUMA_INSTABILITY, src);
            dispatch(ACCUMULATE, dst);
            dispatch(RCAS, (self.output.0.div_ceil(16), self.output.1.div_ceil(16)));
            let range = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1);
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .image(self.image("output").image)
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .subresource_range(range)],
            );
        }
        self.first = false;
        self.frame += 1;
        Ok(())
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for i in self.images {
                for v in i.views {
                    d.destroy_image_view(v, None);
                }
                d.destroy_image(i.image, None);
                d.free_memory(i.memory, None);
            }
            for (b, m) in [self.constants, self.lut_staging] {
                d.destroy_buffer(b, None);
                d.free_memory(m, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
        }
    }
}
