//! The blurred picture of what the layers before a layer drew (`docs/renderer.md`, "The blurred
//! screen"): the game's way of backing a sub-screen with its menu. The target so far is scaled to
//! half size and blurred along x, then along y, into images a layer then draws as a texture
//! ([`crate::text::Layer::set_screen_texture`]).

use ash::vk;

use crate::{barrier, Gpu, Result};

/// The two lines' tap spacing in pixels of the half-size target (read off the capture's blur
/// passes: 1.33 along x, 1.3 along y); the shader's offsets are in units of it.
pub const SPACING: [f32; 2] = [1.33, 1.3];

/// The x pass walks its taps the other way (its short side to the right): the capture's first target fits 1.97 bytes that way, 2.19 the other.
const DIRECTION: [f32; 2] = [-1.0, 1.0];

const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
const BLUR_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/screen_blur.frag.spv"));

/// The half-size target's size for an output of `width` x `height`.
pub fn half(width: u32, height: u32) -> (u32, u32) {
    (width.div_ceil(2).max(1), height.div_ceil(2).max(1))
}

/// A tap spacing's step in uv for the blur shader.
fn step(axis: usize, spacing: f32, size: (u32, u32)) -> [f32; 2] {
    match axis {
        0 => [spacing / size.0 as f32, 0.0],
        _ => [0.0, spacing / size.1 as f32],
    }
}

struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    set: vk::DescriptorSet,
    framebuffer: vk::Framebuffer,
}

/// The blur's images, pass and pipeline, for one output size.
pub struct Blur {
    size: (u32, u32),
    pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    sampler: vk::Sampler,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    /// The scaled copy of the screen, then the picture blurred along x, then along y (the result).
    targets: [Target; 3],
}

impl Blur {
    /// The blur for a screen of `output` pixels in `format`.
    pub fn new(gpu: &Gpu, format: vk::Format, output: (u32, u32)) -> Result<Blur> {
        let d = &gpu.device;
        let size = half(output.0, output.1);
        unsafe {
            let attachment = [vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let reference = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
            let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&reference)];
            let dependencies = [
                vk::SubpassDependency::default()
                    .src_subpass(vk::SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TRANSFER)
                    .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
                vk::SubpassDependency::default()
                    .src_subpass(0)
                    .dst_subpass(vk::SUBPASS_EXTERNAL)
                    .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                    .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ),
            ];
            let pass = d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&dependencies), None)?;
            let sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?;
            let binding = [vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)];
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)?;
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 3 }];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(3).pool_sizes(&sizes), None)?;
            let sets = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[set_layout; 3]))?;
            let mut made = Vec::new();
            for set in sets {
                let (image, memory) = gpu.image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D { width: size.0, height: size.1, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                )?;
                let view = d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )?;
                let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]);
                let views = [view];
                let framebuffer = d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(size.0).height(size.1).layers(1), None)?;
                made.push(Target { image, memory, view, set, framebuffer });
            }
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 8 }];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&[set_layout]).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(POST_VERT)?, module(BLUR_FRAG)?];
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(modules[1]).name(c"main"),
            ];
            let vinput = vk::PipelineVertexInputStateCreateInfo::default();
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let vstate = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
            let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
            let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let blend_att = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
            let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vinput)
                .input_assembly_state(&assembly)
                .viewport_state(&vstate)
                .rasterization_state(&raster)
                .multisample_state(&msaa)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic_state)
                .layout(layout)
                .render_pass(pass);
            let pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0];
            let targets: [Target; 3] = made.try_into().map_err(|_| "screen blur targets")?;
            Ok(Blur { size, pass, set_layout, pool, sampler, layout, pipeline, modules, targets })
        }
    }

    /// The output size this was made for.
    pub fn output_half(&self) -> (u32, u32) {
        self.size
    }

    /// The picture after the x blur and the final one, for the stage dumps (`dump`).
    pub fn stages(&self) -> [(&'static str, vk::Image); 2] {
        [("screen_blur_x", self.targets[1].image), ("screen_blur", self.targets[2].image)]
    }

    /// The blurred picture's descriptor set (valid once [`Blur::record`] has run; the layer
    /// samples it with a layout set of one combined sampler).
    pub fn picture(&self) -> vk::DescriptorSet {
        self.targets[2].set
    }

    /// Scales `source` (the output, `output` pixels, in TRANSFER_SRC_OPTIMAL) to half size and
    /// blurs it along x and along y. The picture is left in SHADER_READ_ONLY_OPTIMAL.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass.
    pub unsafe fn record(&self, gpu: &Gpu, cb: vk::CommandBuffer, source: vk::Image, output: (u32, u32)) {
        let d = &gpu.device;
        let color = vk::ImageAspectFlags::COLOR;
        let [scaled, across, down] = &self.targets;
        barrier(d, cb, scaled.image, color, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
        let layer = vk::ImageSubresourceLayers::default().aspect_mask(color).layer_count(1);
        let end = |(w, h): (u32, u32)| vk::Offset3D { x: w as i32, y: h as i32, z: 1 };
        let region = vk::ImageBlit::default().src_subresource(layer).src_offsets([vk::Offset3D::default(), end(output)]).dst_subresource(layer).dst_offsets([vk::Offset3D::default(), end(self.size)]);
        unsafe { d.cmd_blit_image(cb, source, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, scaled.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region], vk::Filter::LINEAR) };
        barrier(d, cb, scaled.image, color, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: self.size.0, height: self.size.1 } };
        for (axis, (from, to)) in [(scaled, across), (across, down)].into_iter().enumerate() {
            let push = step(axis, SPACING[axis] * DIRECTION[axis], self.size);
            unsafe {
                d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(self.pass).framebuffer(to.framebuffer).render_area(area), vk::SubpassContents::INLINE);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[from.set], &[]);
                d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, 8));
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.size.0 as f32, height: self.size.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[area]);
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_end_render_pass(cb);
            }
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            for t in self.targets {
                d.destroy_framebuffer(t.framebuffer, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.layout, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_render_pass(self.pass, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_half_the_output_rounded_up() {
        assert_eq!(half(1920, 1080), (960, 540));
        assert_eq!(half(1281, 721), (641, 361));
        assert_eq!(half(1, 1), (1, 1));
    }

    #[test]
    fn a_pass_steps_its_own_spacing_along_its_own_axis() {
        let s = step(0, 1.33, (960, 540));
        assert!((s[0] - 1.33 / 960.0).abs() < 1e-9 && s[1] == 0.0);
        let s = step(1, 1.3, (960, 540));
        assert!(s[0] == 0.0 && (s[1] - 1.3 / 540.0).abs() < 1e-9);
    }
}
