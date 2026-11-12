//! FSR 1 in the frame (`fsr.rs` has the constants, `easu.frag` and `rcas.frag` the passes):
//! when the scene is drawn below the output's size, EASU scales it to the output into
//! `upscaled` and RCAS sharpens that into `sharp`, which the post pass then reads in the
//! scene's place. Both targets are 8-bit like the scene.

use ash::vk;

use crate::{fsr, Gpu, Result};

const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
const EASU_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/easu.frag.spv"));
const RCAS_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rcas.frag.spv"));
/// RCAS's sharpness in stops below the sharpest (FSR's samples use 0.2).
pub const SHARPNESS_STOPS: f32 = 0.2;

/// The pass and the two pipelines, made once.
pub struct Pipelines {
    pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    modules: [vk::ShaderModule; 3],
    easu: vk::Pipeline,
    rcas: vk::Pipeline,
}

struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
}

/// The two output-size targets and the sets reading the scene and the upscaled image.
pub struct Targets {
    pool: vk::DescriptorPool,
    scene_set: vk::DescriptorSet,
    upscaled_set: vk::DescriptorSet,
    upscaled: Target,
    sharp: Target,
    input: (u32, u32),
    output: (u32, u32),
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
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let colour = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour)];
        // The last frame's reads of the target before it is written, and the next pass's after.
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
                .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                .dst_access_mask(vk::AccessFlags::SHADER_READ),
        ];
        let binding = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        unsafe {
            let pass = d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&deps), None)?;
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)?;
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 64 }];
            let layouts = [set_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(POST_VERT)?, module(EASU_FRAG)?, module(RCAS_FRAG)?];
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
            let make = |frag: vk::ShaderModule| {
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
                d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map(|p| p[0]).map_err(|(_, e)| e)
            };
            let easu = make(modules[1])?;
            let rcas = make(modules[2])?;
            Ok(Pipelines { pass, set_layout, layout, modules, easu, rcas })
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.easu, None);
            d.destroy_pipeline(self.rcas, None);
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
    /// Targets for a scene of `input` pixels scaled to an output of `output` pixels.
    pub fn new(gpu: &Gpu, p: &Pipelines, scene: vk::ImageView, sampler: vk::Sampler, input: (u32, u32), output: (u32, u32)) -> Result<Targets> {
        let d = &gpu.device;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 2 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(2).pool_sizes(&sizes), None)? };
        let set_for = |view: vk::ImageView| -> Result<vk::DescriptorSet> {
            let layouts = [p.set_layout];
            let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))?[0] };
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            unsafe {
                d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[])
            };
            Ok(set)
        };
        let target = || -> Result<Target> {
            let (image, memory) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D { width: output.0, height: output.1, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
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
            let views = [view];
            let framebuffer = unsafe {
                d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(p.pass).attachments(&views).width(output.0).height(output.1).layers(1), None)?
            };
            Ok(Target { image, memory, view, framebuffer })
        };
        let upscaled = target()?;
        let sharp = target()?;
        Ok(Targets { pool, scene_set: set_for(scene)?, upscaled_set: set_for(upscaled.view)?, upscaled, sharp, input, output })
    }

    /// The upscaled, sharpened scene, for the post pass to read.
    pub fn result(&self) -> vk::ImageView {
        self.sharp.view
    }

    /// Records EASU then RCAS.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene's pass.
    pub unsafe fn record(&self, d: &ash::Device, p: &Pipelines, cb: vk::CommandBuffer) {
        let (w, h) = self.output;
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
        let run = |to: &Target, pipeline: vk::Pipeline, set: vk::DescriptorSet, push: &[f32]| unsafe {
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(p.pass).framebuffer(to.framebuffer).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, p.layout, 0, &[set], &[]);
            let bytes: Vec<u8> = push.iter().flat_map(|v| v.to_le_bytes()).collect();
            d.cmd_push_constants(cb, p.layout, vk::ShaderStageFlags::FRAGMENT, 0, &bytes);
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        };
        let con: Vec<f32> = fsr::easu_constants(self.input, self.output).concat();
        run(&self.upscaled, p.easu, self.scene_set, &con);
        run(&self.sharp, p.rcas, self.upscaled_set, &[fsr::rcas_sharpness(SHARPNESS_STOPS)]);
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for t in [self.upscaled, self.sharp] {
                d.destroy_framebuffer(t.framebuffer, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
        }
    }
}
