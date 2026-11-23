//! Ryza's sun flare and paraffin wash (`docs/renderer.md`, "The sun flare"): after the scene is
//! done the game washes the frame with a warm veil along the top of the screen (the flare, along
//! a curve tilted toward the sun) and tints it where the sun is not blocked (the paraffin), both
//! by a one-pixel "sun visibility" that the sun's shadow nudges every frame.
//!
//! This module owns the visibility (two one-pixel targets and the pass that updates them from the
//! first shadow cascade) and the settings buffer the post pass (`post.frag`) reads the wash's
//! constants from. A frame that has no [`Settings`] (Escha's) switches the wash off.

use ash::vk;

use crate::glow::pass;
use crate::meter::{fullscreen, shader_module};
use crate::{Gpu, Result};

const FORMAT: vk::Format = vk::Format::R16_SFLOAT;
const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sunvis.frag.spv"));
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));

/// One of the wash's two layers: a curve across the screen, `amount - (coefficient * (center -
/// x)^2 + y)` in the tilted screen space, and a colour added (flare) or multiplied in
/// (paraffin) by it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// The colour in 0..1 and its weight in `w`.
    pub color: [f32; 4],
    pub amount: f32,
    pub curve_center: f32,
    pub curve_coefficient: f32,
    /// How much the sun's visibility takes the layer away (flare) or brings it in (paraffin).
    pub shadow_influence: f32,
}

/// What a frame feeds: the reference point and speeds of the visibility, and the wash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// The point the sun's shadow is asked about, in the first cascade's map: its uv, its depth,
    /// and 1.
    pub reference: [f32; 4],
    /// The visibility's step per frame toward shadowed and toward lit.
    pub up_speed: f32,
    pub down_speed: f32,
    /// The screen's tilt about its centre: row 0 then row 1 of the 2x2.
    pub rotation: [f32; 4],
    pub flare: Layer,
    pub paraffin: Layer,
    pub intensity: f32,
    /// The wash's strength over the sky (where the depth is the far plane's). Not applied yet:
    /// the post pass has no depth, so the sky takes `intensity` (docs/ryza/open.md).
    pub sky_intensity: f32,
}

/// The settings buffer's layout as `post.frag` declares it (std140: six vectors).
#[repr(C)]
#[derive(Clone, Copy)]
struct Block {
    rotation: [f32; 4],
    flare_color: [f32; 4],
    paraffin_color: [f32; 4],
    flare: [f32; 4],
    paraffin: [f32; 4],
    /// The shadow influence of each layer, whether the wash is on, and the intensity.
    extra: [f32; 4],
}

impl Block {
    fn of(s: Option<&Settings>) -> Block {
        let Some(s) = s else { return Block { rotation: [0.0; 4], flare_color: [0.0; 4], paraffin_color: [0.0; 4], flare: [0.0; 4], paraffin: [0.0; 4], extra: [0.0; 4] } };
        let layer = |l: &Layer| [l.amount, l.curve_center, l.curve_coefficient, 0.0];
        Block { rotation: s.rotation, flare_color: s.flare.color, paraffin_color: s.paraffin.color, flare: layer(&s.flare), paraffin: layer(&s.paraffin), extra: [s.flare.shadow_influence, s.paraffin.shadow_influence, 1.0, s.intensity] }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Push {
    reference: [f32; 4],
    rates: [f32; 4],
}

struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
}

/// The visibility's pipeline, its two one-pixel targets (the value drawn this frame and the one
/// kept for the next) and the settings buffer.
pub struct Sun {
    pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    modules: [vk::ShaderModule; 2],
    pipeline: vk::Pipeline,
    pool: vk::DescriptorPool,
    /// Draw the value from the kept one; keep the value drawn.
    sets: [vk::DescriptorSet; 2],
    /// The value of this frame, and the one kept.
    targets: [Target; 2],
    settings: (vk::Buffer, vk::DeviceMemory),
    /// Whether the targets still have to be put in a readable layout, and whether no value
    /// has been drawn yet.
    unprepared: std::cell::Cell<bool>,
    fresh: std::cell::Cell<bool>,
}

const VALUE: usize = 0;
const KEPT: usize = 1;

impl Sun {
    /// `shadow`: the shadow map's array view and its comparison sampler.
    pub fn new(gpu: &Gpu, shadow: (vk::ImageView, vk::Sampler), sampler: vk::Sampler) -> Result<Sun> {
        let d = &gpu.device;
        let pass = pass(gpu, FORMAT, vk::AttachmentLoadOp::DONT_CARE)?;
        let bindings = [0, 1].map(|b| vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT));
        unsafe {
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)?;
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: size_of::<Push>() as u32 }];
            let layouts = [set_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let modules = [shader_module(gpu, POST_VERT)?, shader_module(gpu, FRAG)?];
            let pipeline = fullscreen(gpu, layout, pass, &modules)?;
            let target = || -> Result<Target> {
                let (image, memory) = gpu.image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(FORMAT)
                        .extent(vk::Extent3D { width: 1, height: 1, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
                )?;
                let view = d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(FORMAT)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )?;
                let views = [view];
                let framebuffer = d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(1).height(1).layers(1), None)?;
                Ok(Target { image, memory, view, framebuffer })
            };
            let targets = [target()?, target()?];
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 4 }];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(2).pool_sizes(&sizes), None)?;
            let set_layouts = [set_layout; 2];
            let sets: [vk::DescriptorSet; 2] = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&set_layouts))?.try_into().unwrap();
            let settings = gpu.buffer(size_of::<Block>() as u64, vk::BufferUsageFlags::UNIFORM_BUFFER | vk::BufferUsageFlags::TRANSFER_DST, false)?;
            let sun = Sun { pass, set_layout, layout, modules, pipeline, pool, sets, targets, settings, unprepared: std::cell::Cell::new(true), fresh: std::cell::Cell::new(true) };
            sun.rebind(gpu, shadow, sampler);
            Ok(sun)
        }
    }

    /// Points the sets at `shadow` (the shadow map's array view and its comparison sampler) and
    /// reads the values through `sampler` (a renderer made again keeps its sun's share).
    pub fn rebind(&self, gpu: &Gpu, shadow: (vk::ImageView, vk::Sampler), sampler: vk::Sampler) {
        // Set 0 reads the kept value, set 1 the value just drawn.
        for (set, before) in self.sets.iter().zip([self.targets[KEPT].view, self.targets[VALUE].view]) {
            let map = [vk::DescriptorImageInfo::default().sampler(shadow.1).image_view(shadow.0).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let last = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(before).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&map),
                vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&last),
            ];
            unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
        }
    }

    /// The settings buffer the post pass reads.
    pub fn settings(&self) -> vk::Buffer {
        self.settings.0
    }

    /// The kept value, which the post pass reads as the sun's visibility.
    pub fn visibility(&self) -> vk::ImageView {
        self.targets[KEPT].view
    }

    /// Records the frame's update of the visibility (when it has settings) and the settings'
    /// upload, before the post pass.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the shadow map is drawn.
    pub unsafe fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, settings: Option<&Settings>) {
        let range = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1);
        unsafe {
            if self.unprepared.replace(false) {
                let barriers = self.targets.each_ref().map(|t| {
                    vk::ImageMemoryBarrier::default()
                        .image(t.image)
                        .subresource_range(range)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)
                });
                d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &barriers);
            }
            // The last frame's post pass has read the buffer before this frame writes it.
            let idle = [vk::BufferMemoryBarrier::default().buffer(self.settings.0).size(vk::WHOLE_SIZE).src_access_mask(vk::AccessFlags::UNIFORM_READ).dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)];
            d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &idle, &[]);
            let block = Block::of(settings);
            d.cmd_update_buffer(cb, self.settings.0, 0, std::slice::from_raw_parts((&block as *const Block) as *const u8, size_of::<Block>()));
            let ready = [vk::BufferMemoryBarrier::default()
                .buffer(self.settings.0)
                .size(vk::WHOLE_SIZE)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::UNIFORM_READ)];
            d.cmd_pipeline_barrier(cb, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &ready, &[]);
            let Some(s) = settings else { return };
            let fresh = self.fresh.replace(false);
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: 1, height: 1 } };
            // The value from the kept one, then kept for the next frame.
            for (k, copy) in [(0, 0.0), (1, 1.0)] {
                let push = Push { reference: s.reference, rates: [s.up_speed, s.down_speed, if fresh && k == 0 { 1.0 } else { 0.0 }, copy] };
                let to = &self.targets[if k == 0 { VALUE } else { KEPT }];
                d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(self.pass).framebuffer(to.framebuffer).render_area(area), vk::SubpassContents::INLINE);
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: 1.0, height: 1.0, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[area]);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[self.sets[k]], &[]);
                d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts((&push as *const Push) as *const u8, size_of::<Push>()));
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_end_render_pass(cb);
            }
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            d.destroy_pipeline(self.pipeline, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_render_pass(self.pass, None);
            for t in self.targets {
                d.destroy_framebuffer(t.framebuffer, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            d.destroy_buffer(self.settings.0, None);
            d.free_memory(self.settings.1, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_is_six_vectors() {
        assert_eq!(size_of::<Block>(), 96);
        assert_eq!(size_of::<Push>(), 32);
    }

    #[test]
    fn no_settings_switch_the_wash_off() {
        assert_eq!(Block::of(None).extra[2], 0.0);
        let layer = Layer { color: [0.1; 4], amount: 0.5, curve_center: 0.5, curve_coefficient: -0.2, shadow_influence: 1.0 };
        let s = Settings { reference: [0.5, 0.5, 0.5, 1.0], up_speed: 0.1, down_speed: 0.1, rotation: [1.0, 0.0, 0.0, 1.0], flare: layer, paraffin: layer, intensity: 15.0, sky_intensity: 15.0 };
        let b = Block::of(Some(&s));
        assert_eq!((b.extra[2], b.extra[3], b.flare[0]), (1.0, 15.0, 0.5));
    }
}
