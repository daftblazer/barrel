//! The depth of field's softening of Ryza's field (`docs/renderer.md`, "Depth of field"; the
//! game's own passes: `docs/shading.md`, "The depth of field (type 0)"). After the scene is
//! drawn, a weight from the occlusion's depth (`dof_weight.frag`), two quarter-size layers of the
//! scene split by that weight (`dof_down.frag`) and their 16-tap blur (`dof_blur.frag`) are
//! made; the post pass mixes the blurs over the scene by the weight (`post.frag`).

use ash::vk;

use crate::{Gpu, Result};

/// The weight's and the layers' format (the game's targets are half floats too).
pub const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

// The constants of the game's passes as captured (Rasenboden at noon; docs/shading.md, "The depth
// of field (type 0)"): the shaders hold the same numbers, and a test checks they agree.
/// The raw depth's near and far planes (the capture's camera, `docs/renderer.md`).
pub const NEAR: f32 = 50.0;
pub const FAR: f32 = 300000.0;
/// STAND-IN: the weight's curve (a 512 x 1 texture the game's CPU fills out of sight, indexed by
/// the raw depth). Its texels in 255ths, read off the capture's draw 2038 against the raw depth
/// (`docs/ryza/open.md`): the base for texels 0 to `CURVE_TAIL_START` - 1, then the tail. What
/// builds it (the field's or camera's focus settings) is not read.
pub const CURVE_BASE: f32 = 127.0;
pub const CURVE_TAIL: [f32; 16] = [129.0, 132.0, 134.0, 136.0, 139.0, 141.0, 143.0, 145.0, 147.0, 148.0, 150.0, 152.0, 154.0, 155.0, 157.0, 158.0];
pub const CURVE_TAIL_START: u32 = 496;
pub const CURVE_SIZE: u32 = 512;
/// The layers' tap constants (pass 30: the ramp's bias and the weight off the tap's layer).
pub const RAMP_BIAS: f32 = 0.99218;
pub const OFF_LAYER: f32 = 0.00001;
/// The blur's taps: the offsets in texels, and a row of the program's table for each four taps
/// (the term taken from the distance, the tap's weight, the term added, the distance's scale).
pub const BLUR_OFFSETS: [[f32; 2]; 16] = [
    [0.6666667, 0.6666667],
    [-0.6666667, 0.6666667],
    [0.6666667, -0.6666667],
    [-0.6666667, -0.6666667],
    [2.4, 0.0],
    [0.0, 2.4],
    [-2.4, 0.0],
    [0.0, -2.4],
    [1.3, 2.4],
    [2.4, 1.3],
    [-1.3, 2.4],
    [-2.4, 1.3],
    [1.3, -2.4],
    [2.4, -1.3],
    [-1.3, -2.4],
    [-2.4, -1.3],
];
pub const BLUR_TABLE: [[f32; 4]; 4] = [[3.7712362, 3.0, 4.0, 0.25], [10.0, 2.0, 4.5, 0.22222222], [10.917875, 2.5, 5.5, 0.18181819], [10.917875, 2.5, 5.5, 0.18181819]];

const WEIGHT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dof_weight.frag.spv"));
const DOWN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dof_down.frag.spv"));
const BLUR_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dof_blur.frag.spv"));

/// The size of the layers for a scene of `size`: a quarter, rounded up.
pub fn quarter(size: (u32, u32)) -> (u32, u32) {
    (size.0.div_ceil(4).max(1), size.1.div_ceil(4).max(1))
}

/// A colour target with its view.
struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

/// The targets of one scene size.
struct Maps {
    size: (u32, u32),
    layer_size: (u32, u32),
    weight: Target,
    /// The scene split in two (the near view's, the far view's), then each blurred.
    split: [Target; 2],
    blurred: [Target; 2],
    weight_fb: vk::Framebuffer,
    split_fb: vk::Framebuffer,
    blurred_fb: vk::Framebuffer,
    weight_set: vk::DescriptorSet,
    down_set: vk::DescriptorSet,
    blur_set: vk::DescriptorSet,
}

/// The passes, pipelines and targets.
pub struct DepthOfField {
    /// One target, and two (the layers'), each left readable.
    passes: [vk::RenderPass; 2],
    layouts: [vk::PipelineLayout; 3],
    set_layouts: [vk::DescriptorSetLayout; 3],
    pipelines: [vk::Pipeline; 3],
    modules: [vk::ShaderModule; 4],
    point: vk::Sampler,
    linear: vk::Sampler,
    pool: vk::DescriptorPool,
    maps: Option<Maps>,
}

impl DepthOfField {
    pub fn new(gpu: &Gpu) -> Result<DepthOfField> {
        let d = &gpu.device;
        let shader_read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let colour_refs = [
            vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL },
            vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL },
        ];
        // What reads a target after its pass (a fragment shader, or the stage dump's copy), and
        // what read it the frame before.
        let done = vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TRANSFER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_READ);
        let after_reads = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TRANSFER)
            .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_READ)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
        let deps = [after_reads, done];
        let attachment = || {
            vk::AttachmentDescription::default()
                .format(FORMAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(shader_read)
        };
        let pass = |count: usize| -> Result<vk::RenderPass> {
            let attachments: Vec<_> = (0..count).map(|_| attachment()).collect();
            let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour_refs[..count])];
            Ok(unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpass).dependencies(&deps), None)? })
        };
        let passes = [pass(1)?, pass(2)?];

        let words = |code: &[u8]| -> Vec<u32> { code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() };
        let module = |code: &[u8]| unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words(code)), None) };
        let modules = [module(crate::POST_VERT)?, module(WEIGHT_FRAG)?, module(DOWN_FRAG)?, module(BLUR_FRAG)?];
        // Sets: the weight (the depth), the layers (the scene, the weight), the blur (the layers).
        let set_layout = |count: u32| unsafe {
            let bindings: Vec<_> = (0..count)
                .map(|b| vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT))
                .collect();
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)
        };
        let set_layouts = [set_layout(1)?, set_layout(2)?, set_layout(2)?];
        let layout = |set: vk::DescriptorSetLayout, push: u32| unsafe {
            let sets = [set];
            let ranges = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: push }];
            let info = vk::PipelineLayoutCreateInfo::default().set_layouts(&sets);
            d.create_pipeline_layout(&if push > 0 { info.push_constant_ranges(&ranges) } else { info }, None)
        };
        let layouts = [layout(set_layouts[0], 0)?, layout(set_layouts[1], 8)?, layout(set_layouts[2], 8)?];
        let input = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
        let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let plain = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA); 2];
        let one_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&plain[..1]);
        let two_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&plain);
        let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
        let stages = |fragment| {
            [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main"),
            ]
        };
        let (weight_stages, down_stages, blur_stages) = (stages(modules[1]), stages(modules[2]), stages(modules[3]));
        let base = vk::GraphicsPipelineCreateInfo::default()
            .vertex_input_state(&input)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&msaa)
            .dynamic_state(&dynamic_state);
        let infos = [
            base.stages(&weight_stages).color_blend_state(&one_blend).layout(layouts[0]).render_pass(passes[0]),
            base.stages(&down_stages).color_blend_state(&two_blend).layout(layouts[1]).render_pass(passes[1]),
            base.stages(&blur_stages).color_blend_state(&two_blend).layout(layouts[2]).render_pass(passes[1]),
        ];
        let made = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &infos, None).map_err(|(_, e)| e)? };
        let pipelines = [made[0], made[1], made[2]];

        let sampler = |filter| unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default().mag_filter(filter).min_filter(filter).address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE).address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        };
        let point = sampler(vk::Filter::NEAREST)?;
        let linear = sampler(vk::Filter::LINEAR)?;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 5 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(3).pool_sizes(&sizes), None)? };
        Ok(DepthOfField { passes, layouts, set_layouts, pipelines, modules, point, linear, pool, maps: None })
    }

    /// The targets for a scene of `scene_size` drawn into `scene`, reading the occlusion's depth
    /// `gbuf`; the old ones are gone first. The post pass's sets get the weight and the blurred
    /// layers (bindings 7 to 9).
    pub fn bind(&mut self, gpu: &Gpu, scene_size: (u32, u32), gbuf: vk::ImageView, scene: vk::ImageView, post_sets: &[vk::DescriptorSet; 2], post_sampler: vk::Sampler) -> Result<()> {
        self.free_maps(gpu);
        let d = &gpu.device;
        let layer_size = quarter(scene_size);
        let target = |(w, h): (u32, u32)| -> Result<Target> {
            let (image, memory) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D { width: w, height: h, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC),
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
            Ok(Target { image, memory, view })
        };
        let weight = target(scene_size)?;
        let split = [target(layer_size)?, target(layer_size)?];
        let blurred = [target(layer_size)?, target(layer_size)?];
        let framebuffer = |pass, views: &[vk::ImageView], (w, h): (u32, u32)| unsafe { d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(views).width(w).height(h).layers(1), None) };
        let weight_fb = framebuffer(self.passes[0], &[weight.view], scene_size)?;
        let split_fb = framebuffer(self.passes[1], &[split[0].view, split[1].view], layer_size)?;
        let blurred_fb = framebuffer(self.passes[1], &[blurred[0].view, blurred[1].view], layer_size)?;
        let layouts = [self.set_layouts[0], self.set_layouts[1], self.set_layouts[2]];
        let sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(self.pool).set_layouts(&layouts))? };
        let write = |set, binding, sampler, view| {
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            unsafe { d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]) };
        };
        write(sets[0], 0, self.point, gbuf);
        write(sets[1], 0, self.linear, scene);
        write(sets[1], 1, self.linear, weight.view);
        write(sets[2], 0, self.linear, split[0].view);
        write(sets[2], 1, self.linear, split[1].view);
        for &set in post_sets {
            write(set, 7, post_sampler, weight.view);
            write(set, 8, post_sampler, blurred[0].view);
            write(set, 9, post_sampler, blurred[1].view);
        }
        self.maps = Some(Maps { size: scene_size, layer_size, weight, split, blurred, weight_fb, split_fb, blurred_fb, weight_set: sets[0], down_set: sets[1], blur_set: sets[2] });
        Ok(())
    }

    /// The targets again, bound to the occlusion's depth and to `targets`' scene.
    pub fn rebind(&mut self, gpu: &Gpu, ao: &crate::ao::Occlusion, targets: &crate::Targets, post_sets: &[vk::DescriptorSet; 2], post_sampler: vk::Sampler) -> Result<()> {
        let Some(gbuf) = ao.gbuf_view() else { return Ok(()) };
        self.bind(gpu, targets.scene_size, gbuf, targets.scene_view, post_sets, post_sampler)
    }

    fn free_maps(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        if let Some(m) = self.maps.take() {
            unsafe {
                let _ = d.device_wait_idle();
                let _ = d.free_descriptor_sets(self.pool, &[m.weight_set, m.down_set, m.blur_set]);
                for fb in [m.weight_fb, m.split_fb, m.blurred_fb] {
                    d.destroy_framebuffer(fb, None);
                }
                for t in std::iter::once(m.weight).chain(m.split).chain(m.blurred) {
                    d.destroy_image_view(t.view, None);
                    d.destroy_image(t.image, None);
                    d.free_memory(t.memory, None);
                }
            }
        }
    }

    /// The weight's image and size, for the stage dump (`dof_weight`).
    pub fn weight_image(&self) -> Option<(vk::Image, (u32, u32))> {
        self.maps.as_ref().map(|m| (m.weight.image, m.size))
    }

    /// The blurred layers' images (the near view's, the far view's) and their size, for the stage
    /// dump (`dof_layers`, `dof_layers_back`).
    pub fn layer_images(&self) -> Option<([vk::Image; 2], (u32, u32))> {
        self.maps.as_ref().map(|m| ([m.blurred[0].image, m.blurred[1].image], m.layer_size))
    }

    /// The weight, the two layers and their blur, from the scene the pass before left readable
    /// (it stays readable).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene pass and the occlusion's depth
    /// were drawn.
    pub unsafe fn record(&self, gpu: &Gpu, cb: vk::CommandBuffer) {
        let Some(m) = self.maps.as_ref() else { return };
        let d = &gpu.device;
        let draw = |pass, fb, size: (u32, u32), pipeline, layout, set, push: &[f32]| unsafe {
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: size.0, height: size.1 } };
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(fb).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: size.0 as f32, height: size.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set], &[]);
            if !push.is_empty() {
                d.cmd_push_constants(cb, layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, push.len() * 4));
            }
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        };
        let inverse = |size: (u32, u32)| [1.0 / size.0 as f32, 1.0 / size.1 as f32];
        draw(self.passes[0], m.weight_fb, m.size, self.pipelines[0], self.layouts[0], m.weight_set, &[]);
        draw(self.passes[1], m.split_fb, m.layer_size, self.pipelines[1], self.layouts[1], m.down_set, &inverse(m.size));
        draw(self.passes[1], m.blurred_fb, m.layer_size, self.pipelines[2], self.layouts[2], m.blur_set, &inverse(m.layer_size));
    }

    pub fn destroy(mut self, gpu: &Gpu) {
        self.free_maps(gpu);
        let d = &gpu.device;
        unsafe {
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_sampler(self.point, None);
            d.destroy_sampler(self.linear, None);
            for p in self.pipelines {
                d.destroy_pipeline(p, None);
            }
            for l in self.layouts {
                d.destroy_pipeline_layout(l, None);
            }
            for l in self.set_layouts {
                d.destroy_descriptor_set_layout(l, None);
            }
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            for p in self.passes {
                d.destroy_render_pass(p, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers in the initialiser of `const <type> NAME... = ...;` in a shader, in order (an
    /// array's size inside its brackets is the first, `vec2(...)`s are flattened; identifiers with
    /// digits in them, `vec2`, are skipped).
    fn numbers(source: &str, name: &str) -> Vec<f32> {
        let start = source.find(&format!(" {name}")).unwrap_or_else(|| panic!("no {name}"));
        let rhs = source[start..].split_once(" = ").unwrap().1.split(';').next().unwrap();
        let mut out = Vec::new();
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut Vec<f32>| {
            if word.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') {
                out.push(word.parse().unwrap_or_else(|_| panic!("{name}: {word}")));
            }
            word.clear();
        };
        for c in rhs.chars() {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                word.push(c);
            } else {
                flush(&mut word, &mut out);
            }
        }
        flush(&mut word, &mut out);
        out
    }

    #[test]
    fn the_weight_holds_the_same_constants() {
        let source = include_str!("../shaders/dof_weight.frag");
        assert_eq!(numbers(source, "NEAR"), [NEAR]);
        assert_eq!(numbers(source, "FAR"), [FAR]);
        assert_eq!(numbers(source, "CURVE_BASE"), [CURVE_BASE]);
        assert_eq!(numbers(source, "CURVE_TAIL_START"), [CURVE_TAIL_START as f32]);
        assert_eq!(numbers(source, "CURVE_SIZE"), [CURVE_SIZE as f32]);
        // The array's declaration repeats its size first.
        assert_eq!(numbers(source, "CURVE_TAIL")[1..], CURVE_TAIL);
    }

    #[test]
    fn the_layers_hold_the_same_constants() {
        let source = include_str!("../shaders/dof_down.frag");
        assert_eq!(numbers(source, "RAMP_BIAS"), [RAMP_BIAS]);
        assert_eq!(numbers(source, "OFF_LAYER"), [OFF_LAYER]);
    }

    #[test]
    fn the_blur_holds_the_same_constants() {
        let source = include_str!("../shaders/dof_blur.frag");
        let offsets: Vec<f32> = BLUR_OFFSETS.iter().flatten().copied().collect();
        assert_eq!(numbers(source, "OFFSETS")[1..], offsets);
        let table: Vec<f32> = BLUR_TABLE.iter().flatten().copied().collect();
        assert_eq!(numbers(source, "TABLE")[1..], table);
    }

    #[test]
    fn the_curve_is_the_captured_one() {
        // The texel 495 is the base (127/255); the last is the far plane's 158/255.
        assert_eq!(CURVE_TAIL.len() as u32, CURVE_SIZE - CURVE_TAIL_START);
        assert_eq!(CURVE_TAIL[15], 158.0);
    }

    #[test]
    fn the_layers_are_a_quarter_of_the_scene() {
        assert_eq!(quarter((1920, 1080)), (480, 270));
    }

    #[test]
    fn the_raw_depth_of_the_far_curve_is_where_the_capture_reads_it() {
        // Depth 1,550 units: texel 495 to 496 of 512 (0.969).
        let raw = (1.0 / 1550.0 - 1.0 / NEAR) / (1.0 / FAR - 1.0 / NEAR);
        assert!((raw * CURVE_SIZE as f32 - 495.5).abs() < 1.0);
    }
}
