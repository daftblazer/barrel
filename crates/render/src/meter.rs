//! The exposure's metering, as the Ryza games do it (`docs/shading.md`, "The exposure"): the
//! scene's luminance averaged in the log domain down to one pixel (64, 16, 4 and 1 pixel
//! targets), eased toward over a few seconds, and the exposure taken as the middle gray over it,
//! kept inside the lighting set's range.
//!
//! The scene is lit at a fixed exposure (`Params::baked`) and the tone pass and the bloom take
//! it to the metered one by the ratio this leaves in a one-pixel target, so a frame never needs
//! the exposure before it is drawn: its own metering applies to it, and the first frame starts
//! at the exposure its scene calls for.

use ash::vk;

use crate::glow::pass;
use crate::{Gpu, Result};

/// The adaptation's base rate and its time: the game's `fAdaptBaseRate` and `fAdaptTime`
/// (0.95 and 3 s in the capture of Rasenboden's noon; no lighting set carries them).
pub const ADAPT_BASE_RATE: f32 = 0.95;
pub const ADAPT_TIME: f32 = 3.0;
/// The scene's stored brightness to its true one: it is drawn at `HDR_RANGE_INV` (a half).
const SCENE_GAIN: f32 = 1.0 / crate::HDR_RANGE_INV;
/// The metering's targets' format: four floats a pixel.
const FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/meter.frag.spv"));
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));

/// What a lighting set says of the metering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    /// The luminance the exposure aims the scene's average at (`fMiddleGray`).
    pub key: f32,
    /// The exposure's range (`vExposureRange`).
    pub range: (f32, f32),
    /// The adaptation's scale toward a darker and toward a brighter scene (`fLightAdaptedScale`
    /// is the first, taken when the scene is darker than the eye has adapted to, and
    /// `fDarkAdaptedScale` the second).
    pub toward_darker: f32,
    pub toward_brighter: f32,
}

impl Rule {
    /// How far a frame of `dt` seconds goes toward the scene's luminance: the base rate over
    /// `ADAPT_TIME`, `1 - (1 - base)^(dt / time)`.
    pub fn rate(dt: f32) -> f32 {
        (1.0 - (1.0 - ADAPT_BASE_RATE).powf(dt / ADAPT_TIME)).max(0.0)
    }

    /// The exposure for an average luminance `lum`: the key over it, inside the range.
    pub fn exposure(&self, lum: f32) -> f32 {
        (self.key / (lum + 1e-5)).clamp(self.range.0, self.range.1)
    }

    /// The exposure after a frame of `dt` seconds, from `last` (none at the start: the scene's
    /// own) for a scene of average luminance `scene` (as it is without any exposure). The
    /// metering shader (`meter.frag`, mode 3) does the same on the GPU.
    pub fn adapt(&self, last: Option<f32>, scene: f32, dt: f32) -> f32 {
        let from = last.map_or(scene, |e| self.key / (e + 1e-5));
        let scale = if scene < from { self.toward_darker } else { self.toward_brighter };
        self.exposure(from + (scene - from) * Self::rate(dt) * scale)
    }
}

/// One frame's metering: the rule, the exposure the scene's lights were scaled by, and the
/// frame's time.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub rule: Rule,
    pub baked: f32,
    pub dt: f32,
}

/// The pipeline and render pass the metering's draws share.
pub struct Pipelines {
    pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    modules: [vk::ShaderModule; 2],
    pipeline: vk::Pipeline,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Push {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
}

struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
    size: u32,
}

/// The targets and sets of one meter: 64, 16, 4 and 1 pixels, the exposure drawn this frame
/// and the one kept for the next.
pub struct Targets {
    pool: vk::DescriptorPool,
    /// Passes in order, each as the set reading its source (and the one before it) and the
    /// target index it draws: the scene to 64, 64 to 16, 16 to 4, 4 to 1, the adaptation, the
    /// exposure kept.
    sets: [vk::DescriptorSet; 6],
    targets: [Target; 6],
    fresh: std::cell::Cell<bool>,
    readback: (vk::Buffer, vk::DeviceMemory),
}

const LEVEL_64: usize = 0;
const LEVEL_1: usize = 3;
const EXPOSURE: usize = 4;
const KEPT: usize = 5;

impl Pipelines {
    pub fn new(gpu: &Gpu) -> Result<Pipelines> {
        let d = &gpu.device;
        let pass = pass(gpu, FORMAT, vk::AttachmentLoadOp::DONT_CARE)?;
        let bindings = [0, 1].map(|b| {
            vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)
        });
        unsafe {
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)?;
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: size_of::<Push>() as u32 }];
            let layouts = [set_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let modules = [shader_module(gpu, POST_VERT)?, shader_module(gpu, FRAG)?];
            let pipeline = fullscreen(gpu, layout, pass, &modules)?;
            Ok(Pipelines { pass, set_layout, layout, modules, pipeline })
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.pipeline, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_render_pass(self.pass, None);
        }
    }
}

/// A shader module from SPIR-V bytes.
pub(crate) fn shader_module(gpu: &Gpu, code: &[u8]) -> Result<vk::ShaderModule> {
    let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    Ok(unsafe { gpu.device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)? })
}

/// A pipeline drawing one screen-covering triangle (vertex module first) with no blending.
pub(crate) fn fullscreen(gpu: &Gpu, layout: vk::PipelineLayout, pass: vk::RenderPass, modules: &[vk::ShaderModule; 2]) -> Result<vk::Pipeline> {
    let stages = [
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(modules[1]).name(c"main"),
    ];
    let input = vk::PipelineVertexInputStateCreateInfo::default();
    let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
    let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
    let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
    let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth = vk::PipelineDepthStencilStateCreateInfo::default();
    let blend_att = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
    let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att);
    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&input)
        .input_assembly_state(&assembly)
        .viewport_state(&viewport)
        .rasterization_state(&raster)
        .multisample_state(&msaa)
        .depth_stencil_state(&depth)
        .color_blend_state(&blend)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(pass);
    unsafe { gpu.device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map(|p| p[0]).map_err(|(_, e)| e.into()) }
}

impl Targets {
    /// The targets for a scene read through `scene` (sampled with `sampler`).
    pub fn new(gpu: &Gpu, p: &Pipelines, scene: vk::ImageView, sampler: vk::Sampler) -> Result<Targets> {
        let d = &gpu.device;
        let target = |size: u32| -> Result<Target> {
            let (image, memory) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D { width: size, height: size, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC),
            )?;
            unsafe {
                let view = d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(FORMAT)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )?;
                let views = [view];
                let framebuffer = d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(p.pass).attachments(&views).width(size).height(size).layers(1), None)?;
                Ok(Target { image, memory, view, framebuffer, size })
            }
        };
        let targets = [target(64)?, target(16)?, target(4)?, target(1)?, target(1)?, target(1)?];
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 12 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(6).pool_sizes(&sizes), None)? };
        let layouts = [p.set_layout; 6];
        let sets: [vk::DescriptorSet; 6] = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))? }.try_into().unwrap();
        // Set k reads what pass k reads: the scene, then each level, the adaptation reads the
        // last level and the exposure kept, the keeping reads the exposure drawn.
        let reads = [(scene, scene), (targets[0].view, targets[0].view), (targets[1].view, targets[1].view), (targets[2].view, targets[2].view), (targets[LEVEL_1].view, targets[KEPT].view), (targets[EXPOSURE].view, targets[EXPOSURE].view)];
        for (set, (first, second)) in sets.iter().zip(reads) {
            let info = |view| [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let (a, b) = (info(first), info(second));
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&a),
                vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&b),
            ];
            unsafe { d.update_descriptor_sets(&writes, &[]) };
        }
        let readback = gpu.buffer(16, vk::BufferUsageFlags::TRANSFER_DST, true)?;
        Ok(Targets { pool, sets, targets, fresh: std::cell::Cell::new(true), readback })
    }

    /// The exposure drawn this frame (`x` the exposure, `y` its ratio to the baked one), for
    /// the passes that apply it.
    pub fn exposure(&self) -> vk::ImageView {
        self.targets[EXPOSURE].view
    }

    /// Records the metering of the scene (as drawn at `params.baked`), after its pass.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene's pass.
    pub unsafe fn record(&self, d: &ash::Device, p: &Pipelines, cb: vk::CommandBuffer, params: &Params) {
        let fresh = self.fresh.replace(false);
        let range = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1);
        unsafe {
            if fresh {
                // The kept exposure has nothing yet: its layout is made right and the shader does not read it.
                let barrier = [vk::ImageMemoryBarrier::default()
                    .image(self.targets[KEPT].image)
                    .subresource_range(range)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)];
                d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &barrier);
            }
            let Rule { key, range: (lo, hi), toward_darker, toward_brighter } = params.rule;
            let rate = Rule::rate(params.dt);
            let run = |pass_no: usize, to: usize, mode: f32| {
                let t = &self.targets[to];
                let push = Push {
                    a: [mode, if fresh { 1.0 } else { 0.0 }, rate * toward_darker, rate * toward_brighter],
                    b: [key, lo, hi, params.baked],
                    c: [SCENE_GAIN, 0.0, 0.0, 0.0],
                };
                let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: t.size, height: t.size } };
                d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(p.pass).framebuffer(t.framebuffer).render_area(area), vk::SubpassContents::INLINE);
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: t.size as f32, height: t.size as f32, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[area]);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, p.pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, p.layout, 0, &[self.sets[pass_no]], &[]);
                d.cmd_push_constants(cb, p.layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts((&push as *const Push) as *const u8, size_of::<Push>()));
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_end_render_pass(cb);
            };
            // Down the levels (the last takes the average back out of the log), eased toward,
            // then kept for the next frame.
            run(0, LEVEL_64, 0.0);
            run(1, 1, 1.0);
            run(2, 2, 1.0);
            run(3, LEVEL_1, 2.0);
            run(4, EXPOSURE, 3.0);
            run(5, KEPT, 4.0);
            // The exposure out to the host as well (`Targets::read`).
            let e = &self.targets[EXPOSURE];
            let to_copy = [vk::ImageMemoryBarrier::default()
                .image(e.image)
                .subresource_range(range)
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)];
            d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &to_copy);
            let region = vk::BufferImageCopy::default()
                .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                .image_extent(vk::Extent3D { width: 1, height: 1, depth: 1 });
            d.cmd_copy_image_to_buffer(cb, e.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, self.readback.0, &[region]);
            let back = [vk::ImageMemoryBarrier::default()
                .image(e.image)
                .subresource_range(range)
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &back);
        }
    }

    /// What the last finished frame was metered to: the exposure, and the scene's average
    /// luminance (as it is without any exposure) it came from.
    pub fn read(&self, gpu: &Gpu) -> Result<(f32, f32)> {
        let mut out = [0u8; 16];
        unsafe {
            let p = gpu.device.map_memory(self.readback.1, 0, 16, vk::MemoryMapFlags::empty())? as *const u8;
            std::ptr::copy_nonoverlapping(p, out.as_mut_ptr(), 16);
            gpu.device.unmap_memory(self.readback.1);
        }
        let f = |i: usize| f32::from_le_bytes(out[4 * i..4 * i + 4].try_into().unwrap());
        Ok((f(0), f(2)))
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for t in self.targets {
                d.destroy_framebuffer(t.framebuffer, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_buffer(self.readback.0, None);
            d.free_memory(self.readback.1, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE: Rule = Rule { key: 0.18, range: (0.9, 2.4), toward_darker: 0.5, toward_brighter: 0.5 };

    #[test]
    fn a_frame_goes_a_part_of_the_way() {
        // At 60 Hz: 1 - 0.05^(1/180), a sixtieth of the 95 % in three seconds
        let r = Rule::rate(1.0 / 60.0);
        assert!((r - 0.016_50).abs() < 1e-4, "{r}");
        // in the full time it covers the base rate, and with no time none
        assert!((Rule::rate(ADAPT_TIME) - ADAPT_BASE_RATE).abs() < 1e-6);
        assert_eq!(Rule::rate(0.0), 0.0);
    }

    #[test]
    fn the_exposure_is_the_key_over_the_luminance_inside_the_range() {
        assert!((RULE.exposure(0.1) - 1.8).abs() < 1e-3);
        // brighter scenes stop at the range's bottom, dim ones at its top
        assert_eq!(RULE.exposure(0.5), 0.9);
        assert_eq!(RULE.exposure(0.01), 2.4);
        assert_eq!(RULE.exposure(0.0), 2.4);
    }

    #[test]
    fn the_first_frame_meters_to_the_scene_and_later_ones_ease() {
        assert!((RULE.adapt(None, 0.1, 1.0 / 60.0) - 1.8).abs() < 1e-3);
        // the eye adapted to a luminance of 0.1 (exposure 1.8) meets one of 0.15: the luminance
        // it takes is 0.1 plus half of the frame's rate of the 0.05 between
        let e = RULE.adapt(Some(1.8), 0.15, 1.0 / 60.0);
        let lum = 0.1 + 0.05 * Rule::rate(1.0 / 60.0) * 0.5;
        assert!((e - 0.18 / lum).abs() < 1e-3, "{e}");
        // and a long run of the same scene settles at its exposure (a hair under: the 1e-5 the
        // game adds on the way in and out of the exposure is lost from the luminance each frame)
        let mut e = Some(2.4);
        for _ in 0..3000 {
            e = Some(RULE.adapt(e, 0.1, 1.0 / 60.0));
        }
        assert!((e.unwrap() - 1.8).abs() < 3e-2, "{e:?}");
    }

    #[test]
    fn a_scene_too_bright_for_the_range_holds_its_bottom() {
        let mut e = Some(1.8);
        for _ in 0..3000 {
            e = Some(RULE.adapt(e, 0.4, 1.0 / 60.0));
        }
        assert_eq!(e, Some(0.9));
    }

    #[test]
    fn the_rates_differ_by_direction() {
        // Adapted to a luminance of 0.18 (exposure 1), in a range wide enough to show it.
        let rule = Rule { range: (0.1, 10.0), toward_darker: 0.3, toward_brighter: 0.5, ..RULE };
        let r = Rule::rate(0.1);
        let brighter = rule.adapt(Some(1.0), 0.20, 0.1);
        assert!((brighter - 0.18 / (0.18 + 0.02 * r * 0.5)).abs() < 1e-4, "{brighter}");
        let darker = rule.adapt(Some(1.0), 0.16, 0.1);
        assert!((darker - 0.18 / (0.18 - 0.02 * r * 0.3)).abs() < 1e-4, "{darker}");
    }
}
