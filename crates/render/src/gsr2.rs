//! Snapdragon GSR 2, the temporal upscaler (`settings::RenderSettings::gsr2`; `docs/engine.md`,
//! "GSR 2"): after the scene pass, `gsr2_convert.frag` turns the motion vectors and depth into
//! `motion_depth_clip` at the render's size, then `gsr2_upscale.frag` upsamples the jittered
//! scene and blends it with the reprojected history into `result` at the output's size, which
//! the post pass reads; `result` is then copied into `history` for the next frame. Both are
//! 16-bit float so the accumulation does not band.

use ash::vk;

use crate::{Gpu, Result};

const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
const CONVERT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gsr2_convert.frag.spv"));
const UPSCALE_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gsr2_upscale.frag.spv"));
const PUSH_SIZE: u32 = 64;

/// One frame's inputs beyond the images.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// This frame's jitter in render pixels.
    pub jitter: (f32, f32),
    /// The vertical field of view, in radians.
    pub fov: f32,
    /// No history to use (the first frame, a cut).
    pub reset: bool,
    /// The camera has not moved since the last frame.
    pub same_camera: bool,
}

/// The push constants both passes read (`gsr2_common.glsl`).
fn push(render: (u32, u32), output: (u32, u32), f: Frame) -> [f32; 16] {
    let (rw, rh) = (render.0 as f32, render.1 as f32);
    let (ow, oh) = (output.0 as f32, output.1 as f32);
    // As the compute variant derives them: the kernel's widest bias and the history box's scale.
    let bias_max = (ow / rw).min(1.99);
    let box_scale = ((ow / rw) * (oh / rh)).powi(3).min(20.0);
    let fov_hor = (f.fov * 0.5).tan() * rw / rh;
    // How much unclamped history a still camera keeps where the samples' box rejects it (the
    // samples leave it to the caller; 0.3 holds detail on a still view without visible trails).
    let min_lerp = if f.same_camera { 0.3 } else { 0.0 };
    [rw, rh, ow, oh, 1.0 / rw, 1.0 / rh, 1.0 / ow, 1.0 / oh, f.jitter.0, f.jitter.1, bias_max, box_scale, fov_hor, min_lerp, if f.reset { 1.0 } else { 0.0 }, 0.0]
}

pub struct Pipelines {
    convert_pass: vk::RenderPass,
    upscale_pass: vk::RenderPass,
    convert_layout: vk::DescriptorSetLayout,
    upscale_layout: vk::DescriptorSetLayout,
    layouts: [vk::PipelineLayout; 2],
    modules: [vk::ShaderModule; 3],
    convert: vk::Pipeline,
    upscale: vk::Pipeline,
    /// Depth and motion are read texel by texel: depth formats need not filter.
    nearest: vk::Sampler,
}

struct Image {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

pub struct Targets {
    pool: vk::DescriptorPool,
    convert_set: vk::DescriptorSet,
    upscale_set: vk::DescriptorSet,
    motion_depth_clip: Image,
    result: Image,
    history: Image,
    convert_fb: vk::Framebuffer,
    upscale_fb: vk::Framebuffer,
    render: (u32, u32),
    output: (u32, u32),
}

fn render_pass(d: &ash::Device, final_layout: vk::ImageLayout, dst_stage: vk::PipelineStageFlags, dst_access: vk::AccessFlags) -> Result<vk::RenderPass> {
    let attachment = [vk::AttachmentDescription::default()
        .format(FORMAT)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(final_layout)];
    let colour = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
    let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour)];
    let deps = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::SHADER_READ),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(dst_stage)
            .dst_access_mask(dst_access),
    ];
    Ok(unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&deps), None)? })
}

fn set_layout(d: &ash::Device, count: u32) -> Result<vk::DescriptorSetLayout> {
    let bindings: Vec<_> = (0..count)
        .map(|b| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        })
        .collect();
    Ok(unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)? })
}

impl Pipelines {
    pub fn new(gpu: &Gpu) -> Result<Pipelines> {
        let d = &gpu.device;
        let convert_pass = render_pass(d, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::AccessFlags::SHADER_READ)?;
        let upscale_pass = render_pass(d, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::PipelineStageFlags::TRANSFER, vk::AccessFlags::TRANSFER_READ)?;
        let convert_layout = set_layout(d, 2)?;
        let upscale_layout = set_layout(d, 3)?;
        let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: PUSH_SIZE }];
        let pipeline_layout = |set: vk::DescriptorSetLayout| {
            let sets = [set];
            unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sets).push_constant_ranges(&push), None) }
        };
        let layouts = [pipeline_layout(convert_layout)?, pipeline_layout(upscale_layout)?];
        let module = |code: &[u8]| {
            let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
            unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None) }
        };
        let modules = [module(POST_VERT)?, module(CONVERT_FRAG)?, module(UPSCALE_FRAG)?];
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
        let make = |frag: vk::ShaderModule, layout: vk::PipelineLayout, pass: vk::RenderPass| {
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(frag).name(c"main"),
            ];
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
            unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map(|p| p[0]).map_err(|(_, e)| e) }
        };
        let convert = make(modules[1], layouts[0], convert_pass)?;
        let upscale = make(modules[2], layouts[1], upscale_pass)?;
        let nearest = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?
        };
        Ok(Pipelines { convert_pass, upscale_pass, convert_layout, upscale_layout, layouts, modules, convert, upscale, nearest })
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_sampler(self.nearest, None);
            d.destroy_pipeline(self.convert, None);
            d.destroy_pipeline(self.upscale, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            for l in self.layouts {
                d.destroy_pipeline_layout(l, None);
            }
            d.destroy_descriptor_set_layout(self.convert_layout, None);
            d.destroy_descriptor_set_layout(self.upscale_layout, None);
            d.destroy_render_pass(self.convert_pass, None);
            d.destroy_render_pass(self.upscale_pass, None);
        }
    }
}

impl Targets {
    /// The targets for a render of `render` pixels upscaled to `output`, reading the scene, its
    /// depth and its motion vectors.
    #[allow(clippy::too_many_arguments)]
    pub fn new(gpu: &Gpu, p: &Pipelines, scene: vk::ImageView, depth: vk::ImageView, motion: vk::ImageView, sampler: vk::Sampler, render: (u32, u32), output: (u32, u32)) -> Result<Targets> {
        let d = &gpu.device;
        let image = |size: (u32, u32), usage| -> Result<Image> {
            let (image, memory) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D { width: size.0, height: size.1, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(usage),
            )?;
            let view = unsafe {
                d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(FORMAT)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )?
            };
            Ok(Image { image, memory, view })
        };
        let motion_depth_clip = image(render, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)?;
        let result = image(output, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)?;
        let history = image(output, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)?;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 5 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(2).pool_sizes(&sizes), None)? };
        let alloc = |layout| -> Result<vk::DescriptorSet> {
            let layouts = [layout];
            Ok(unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))?[0] })
        };
        let convert_set = alloc(p.convert_layout)?;
        let upscale_set = alloc(p.upscale_layout)?;
        let info = |sampler, view| [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let infos = [
            (convert_set, 0, info(p.nearest, depth)),
            (convert_set, 1, info(p.nearest, motion)),
            (upscale_set, 0, info(sampler, history.view)),
            (upscale_set, 1, info(sampler, motion_depth_clip.view)),
            (upscale_set, 2, info(sampler, scene)),
        ];
        let writes: Vec<_> = infos
            .iter()
            .map(|(set, binding, i)| {
                vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(*binding).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(i)
            })
            .collect();
        unsafe { d.update_descriptor_sets(&writes, &[]) };
        let framebuffer = |pass, view, size: (u32, u32)| {
            let views = [view];
            unsafe { d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(size.0).height(size.1).layers(1), None) }
        };
        let convert_fb = framebuffer(p.convert_pass, motion_depth_clip.view, render)?;
        let upscale_fb = framebuffer(p.upscale_pass, result.view, output)?;
        Ok(Targets { pool, convert_set, upscale_set, motion_depth_clip, result, history, convert_fb, upscale_fb, render, output })
    }

    /// The upscaled scene, for the post pass to read.
    pub fn result(&self) -> vk::ImageView {
        self.result.view
    }

    /// Records both passes and the copy into the history. `first`: the history has no layout
    /// yet.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene's pass (depth and motion ready
    /// to sample).
    pub unsafe fn record(&self, d: &ash::Device, p: &Pipelines, cb: vk::CommandBuffer, frame: Frame, first: bool) {
        let range = vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1);
        let barrier = |image, old, new, src_access, dst_access| {
            vk::ImageMemoryBarrier::default()
                .image(image)
                .old_layout(old)
                .new_layout(new)
                .src_access_mask(src_access)
                .dst_access_mask(dst_access)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(range)
        };
        let push: Vec<u8> = push(self.render, self.output, frame).iter().flat_map(|v| v.to_le_bytes()).collect();
        let run = |pass, fb, (w, h): (u32, u32), pipeline, layout, set| unsafe {
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(fb).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set], &[]);
            d.cmd_push_constants(cb, layout, vk::ShaderStageFlags::FRAGMENT, 0, &push);
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        };
        unsafe {
            if first {
                d.cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier(self.history.image, vk::ImageLayout::UNDEFINED, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::empty(), vk::AccessFlags::SHADER_READ)],
                );
            }
            run(p.convert_pass, self.convert_fb, self.render, p.convert, p.layouts[0], self.convert_set);
            run(p.upscale_pass, self.upscale_fb, self.output, p.upscale, p.layouts[1], self.upscale_set);
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier(self.history.image, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::AccessFlags::SHADER_READ, vk::AccessFlags::TRANSFER_WRITE)],
            );
            let layers = vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1);
            let (w, h) = self.output;
            let region = vk::ImageCopy::default().src_subresource(layers).dst_subresource(layers).extent(vk::Extent3D { width: w, height: h, depth: 1 });
            d.cmd_copy_image(cb, self.result.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, self.history.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[
                    barrier(self.result.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::SHADER_READ),
                    barrier(self.history.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ),
                ],
            );
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            d.destroy_framebuffer(self.convert_fb, None);
            d.destroy_framebuffer(self.upscale_fb, None);
            for i in [self.motion_depth_clip, self.result, self.history] {
                d.destroy_image_view(i.view, None);
                d.destroy_image(i.image, None);
                d.free_memory(i.memory, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_constants() {
        // 1280×720 into 1920×1080: 1.5× each way.
        let p = push((1280, 720), (1920, 1080), Frame { jitter: (0.25, -0.1), fov: 22.5f32.to_radians(), reset: false, same_camera: true });
        assert_eq!(&p[..4], &[1280.0, 720.0, 1920.0, 1080.0]);
        assert_eq!(p[10], 1.5);
        assert!((p[11] - 1.5f32.powi(6)).abs() < 1e-4);
        assert_eq!((p[13], p[14]), (0.3, 0.0));
    }
}
