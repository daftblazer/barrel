//! The game's glow (`docs/shading.md`, "After the scene"): the scene's bright parts
//! (`saturate(scene - threshold)`) into a target a quarter of the screen's size, blurred across
//! and down over five levels each half the last, blurred back up adding into each level, and
//! level 0 added to the scene at `glow_add_rate` by the post pass.

use ash::vk;

use crate::{Gpu, Result};

pub const LEVELS: usize = 5;
/// The glow targets' format (the game's are 8-bit, like the scene).
const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
const GLOW_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/glow.frag.spv"));

/// The pipelines, made once: one that overwrites its target and one that adds into it.
pub struct Pipelines {
    overwrite_pass: vk::RenderPass,
    add_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    modules: [vk::ShaderModule; 2],
    overwrite: vk::Pipeline,
    add: vk::Pipeline,
}

/// One target: its image, view and framebuffer, and a set to read it from.
struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
    add_framebuffer: vk::Framebuffer,
    set: vk::DescriptorSet,
    size: (u32, u32),
}

/// The targets for one screen size: x and y of each level, and a set reading the scene.
pub struct Targets {
    pool: vk::DescriptorPool,
    scene_set: vk::DescriptorSet,
    x: Vec<Target>,
    y: Vec<Target>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Push {
    step: [f32; 2],
    threshold: f32,
    mode: u32,
}

fn pass(gpu: &Gpu, load: vk::AttachmentLoadOp) -> Result<vk::RenderPass> {
    let attachment = [vk::AttachmentDescription::default()
        .format(FORMAT)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(load)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(if load == vk::AttachmentLoadOp::LOAD { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL } else { vk::ImageLayout::UNDEFINED })
        .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let colour = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
    let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour)];
    // Reads of the target before it is written, and the next pass's reads after.
    let deps = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];
    Ok(unsafe { gpu.device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&deps), None)? })
}

impl Pipelines {
    pub fn new(gpu: &Gpu) -> Result<Pipelines> {
        let d = &gpu.device;
        let overwrite_pass = pass(gpu, vk::AttachmentLoadOp::DONT_CARE)?;
        let add_pass = pass(gpu, vk::AttachmentLoadOp::LOAD)?;
        let binding = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        unsafe {
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)?;
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: size_of::<Push>() as u32 }];
            let layouts = [set_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(POST_VERT)?, module(GLOW_FRAG)?];
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
            let make = |additive: bool, render_pass: vk::RenderPass| {
                let blend_att = [vk::PipelineColorBlendAttachmentState::default()
                    .blend_enable(additive)
                    .src_color_blend_factor(vk::BlendFactor::ONE)
                    .dst_color_blend_factor(vk::BlendFactor::ONE)
                    .color_blend_op(vk::BlendOp::ADD)
                    .src_alpha_blend_factor(vk::BlendFactor::ONE)
                    .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                    .alpha_blend_op(vk::BlendOp::ADD)
                    .color_write_mask(vk::ColorComponentFlags::RGBA)];
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
                    .render_pass(render_pass);
                d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map(|p| p[0]).map_err(|(_, e)| e)
            };
            let overwrite = make(false, overwrite_pass)?;
            let add = make(true, add_pass)?;
            Ok(Pipelines { overwrite_pass, add_pass, set_layout, layout, modules, overwrite, add })
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.overwrite, None);
            d.destroy_pipeline(self.add, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_render_pass(self.overwrite_pass, None);
            d.destroy_render_pass(self.add_pass, None);
        }
    }
}

impl Targets {
    /// The targets for a `width` × `height` screen, reading the scene from `scene`.
    pub fn new(gpu: &Gpu, p: &Pipelines, scene: vk::ImageView, sampler: vk::Sampler, width: u32, height: u32) -> Result<Targets> {
        let d = &gpu.device;
        let n = (2 * LEVELS + 1) as u32;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: n }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)? };
        let set_for = |view: vk::ImageView| -> Result<vk::DescriptorSet> {
            let layouts = [p.set_layout];
            let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))?[0] };
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            unsafe {
                d.update_descriptor_sets(
                    &[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)],
                    &[],
                )
            };
            Ok(set)
        };
        let target = |w: u32, h: u32| -> Result<Target> {
            let (image, memory) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D { width: w, height: h, depth: 1 })
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
            let fb = |pass| unsafe {
                let views = [view];
                d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(w).height(h).layers(1), None)
            };
            Ok(Target { image, memory, view, framebuffer: fb(p.overwrite_pass)?, add_framebuffer: fb(p.add_pass)?, set: set_for(view)?, size: (w, h) })
        };
        let (mut x, mut y) = (Vec::new(), Vec::new());
        for level in 0..LEVELS {
            let (w, h) = ((width / 4 >> level).max(1), (height / 4 >> level).max(1));
            x.push(target(w, h)?);
            y.push(target(w, h)?);
        }
        Ok(Targets { pool, scene_set: set_for(scene)?, x, y })
    }

    /// The finished glow, for the post pass to read.
    pub fn result(&self) -> vk::ImageView {
        self.y[0].view
    }

    /// Records the glow: bright pass, blurs down, blurs back up adding.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene's pass.
    pub unsafe fn record(&self, d: &ash::Device, p: &Pipelines, cb: vk::CommandBuffer, threshold: f32) {
        let run = |to: &Target, from: vk::DescriptorSet, add: bool, push: Push| unsafe {
            let (w, h) = to.size;
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
            let (pass, fb, pipe) = if add { (p.add_pass, to.add_framebuffer, p.add) } else { (p.overwrite_pass, to.framebuffer, p.overwrite) };
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(fb).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipe);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, p.layout, 0, &[from], &[]);
            d.cmd_push_constants(cb, p.layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts((&push as *const Push) as *const u8, size_of::<Push>()));
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        };
        let across = |t: &Target| Push { step: [1.0 / t.size.0 as f32, 0.0], threshold, mode: 1 };
        let down = |t: &Target| Push { step: [0.0, 1.0 / t.size.1 as f32], threshold, mode: 1 };
        // The bright parts into y0; then each level: across from the level above (or y0
        // itself) into x, down from x into y.
        run(&self.y[0], self.scene_set, false, Push { step: [0.0; 2], threshold, mode: 0 });
        for l in 0..LEVELS {
            let from = self.y[l.saturating_sub(1)].set;
            run(&self.x[l], from, false, across(&self.x[l]));
            run(&self.y[l], self.x[l].set, false, down(&self.y[l]));
        }
        // Back up: each level's y blurred into the one above, added.
        for l in (1..LEVELS).rev() {
            run(&self.y[l - 1], self.y[l].set, true, across(&self.y[l - 1]));
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for t in self.x.into_iter().chain(self.y) {
                d.destroy_framebuffer(t.framebuffer, None);
                d.destroy_framebuffer(t.add_framebuffer, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
        }
    }
}
