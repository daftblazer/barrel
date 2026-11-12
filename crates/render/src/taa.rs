//! Temporal anti-aliasing (`settings::RenderSettings::taa`; `docs/engine.md`, "TAA"): after the
//! scene pass, `taa.frag` blends the jittered scene into the history of the frames before it,
//! reprojected by the motion vectors and clamped to each pixel's neighbourhood, into
//! `resolved`, which the post pass (or FSR 1) reads in the scene's place; `resolved` is then
//! copied into `history` for the next frame. Both are the scene's size and format.

use ash::vk;

use crate::{Gpu, Result};

const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
const TAA_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/taa.frag.spv"));
/// How much of each new frame goes into the history.
pub const BLEND: f32 = 0.1;

pub struct Pipelines {
    pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    modules: [vk::ShaderModule; 2],
    pipeline: vk::Pipeline,
}

struct Image {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

pub struct Targets {
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    resolved: Image,
    history: Image,
    framebuffer: vk::Framebuffer,
    size: (u32, u32),
}

impl Pipelines {
    pub fn new(gpu: &Gpu) -> Result<Pipelines> {
        let d = &gpu.device;
        let attachment = [vk::AttachmentDescription::default()
            .format(FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)];
        let colour = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour)];
        let deps = [
            vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
            vk::SubpassDependency::default()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_stage_mask(vk::PipelineStageFlags::TRANSFER)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ),
        ];
        let bindings = [0, 1, 2].map(|b| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        });
        unsafe {
            let pass = d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&deps), None)?;
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)?;
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 16 }];
            let layouts = [set_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(POST_VERT)?, module(TAA_FRAG)?];
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
            let pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0];
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

impl Targets {
    /// The targets for a scene of `size`, reading the scene and its motion vectors.
    pub fn new(gpu: &Gpu, p: &Pipelines, scene: vk::ImageView, motion: vk::ImageView, sampler: vk::Sampler, size: (u32, u32)) -> Result<Targets> {
        let d = &gpu.device;
        let image = |usage| -> Result<Image> {
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
        let resolved = image(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)?;
        let history = image(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)?;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 3 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None)? };
        let layouts = [p.set_layout];
        let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))?[0] };
        let info = |view| [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let (a, b, c) = (info(scene), info(history.view), info(motion));
        fn write(set: vk::DescriptorSet, binding: u32, i: &[vk::DescriptorImageInfo]) -> vk::WriteDescriptorSet<'_> {
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(i)
        }
        unsafe { d.update_descriptor_sets(&[write(set, 0, &a), write(set, 1, &b), write(set, 2, &c)], &[]) };
        let views = [resolved.view];
        let framebuffer = unsafe { d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(p.pass).attachments(&views).width(size.0).height(size.1).layers(1), None)? };
        Ok(Targets { pool, set, resolved, history, framebuffer, size })
    }

    /// The anti-aliased scene, for the post pass (or FSR 1) to read.
    pub fn result(&self) -> vk::ImageView {
        self.resolved.view
    }

    /// Records the resolve and the copy into the history. `reset`: no history to use (the
    /// first frame, a cut); `first`: the history has no layout yet.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene's pass.
    pub unsafe fn record(&self, d: &ash::Device, p: &Pipelines, cb: vk::CommandBuffer, reset: bool, first: bool) {
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
            let (w, h) = self.size;
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(p.pass).framebuffer(self.framebuffer).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, p.pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, p.layout, 0, &[self.set], &[]);
            let push: Vec<u8> = [1.0 / w as f32, 1.0 / h as f32, if reset { 1.0 } else { 0.0 }, BLEND].iter().flat_map(|v| v.to_le_bytes()).collect();
            d.cmd_push_constants(cb, p.layout, vk::ShaderStageFlags::FRAGMENT, 0, &push);
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
            // The history takes this frame's result for the next.
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
            let region = vk::ImageCopy::default().src_subresource(layers).dst_subresource(layers).extent(vk::Extent3D { width: w, height: h, depth: 1 });
            d.cmd_copy_image(cb, self.resolved.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, self.history.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[
                    barrier(self.resolved.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::SHADER_READ),
                    barrier(self.history.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ),
                ],
            );
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            d.destroy_framebuffer(self.framebuffer, None);
            for i in [self.resolved, self.history] {
                d.destroy_image_view(i.view, None);
                d.destroy_image(i.image, None);
                d.free_memory(i.memory, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
        }
    }
}
