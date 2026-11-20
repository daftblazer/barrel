//! The screen-space reflections of Ryza's field (`docs/renderer.md`, "Reflections"; the game's
//! own passes: `docs/shading.md`, "The screen-space reflections"). After the opaque scene is
//! drawn, a half-size march (`reflect.frag`) over the occlusion's normals and view depth
//! (`ao.rs`) finds what each pixel's reflection ray meets in the lit scene, a six-level mip chain
//! blurs it, and a full-screen pass (`reflect_apply.frag`) moves the specular ambient the scene
//! pass already added toward it, by what the scene pass left per pixel (its `spec_light` and
//! `spec_env` attachments, `model.frag`).

use ash::vk;

use crate::ao::{half, View};
use crate::{Gpu, Result};

/// The march target's format (a reflection's colour and its alpha).
pub const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
/// The scene pass's two per-pixel attachments, as narrow as they hold (a tiler keeps every
/// attachment in its tile memory, so their bytes decide its bins): `spec_light`, the factor (an
/// occlusion times a Fresnel term: 0 to 1, sRGB-encoded so the small ones keep their precision)
/// and the gloss; `spec_env`, the cube's light (positive, unbounded).
pub const TARGET_FORMATS: [vk::Format; 2] = [vk::Format::R8G8B8A8_SRGB, vk::Format::B10G11R11_UFLOAT_PACK32];
/// The mip chain's levels: 960 by 540 down to 30 by 16 (the game's five downsample draws).
pub const LEVELS: u32 = 6;

// The constants of the game's passes as captured (Rasenboden at noon, its 960 by 540 target;
// docs/shading.md, "The screen-space reflections"): the shaders hold the same numbers, and a test
// checks they agree.
/// `nMaxRaySmpl` at the game's High level: the march's steps.
pub const MAX_STEPS: u32 = 256;
/// `vReflParam` at High (one pixel a step): a half-size pixel's step in normalised device units (2 / 540), the aspect
/// (16 / 9), the ray direction's scale and its bias (a ray with `z / |R| * scale + bias` of 1 or
/// more points at the viewer and shows nothing).
pub const REFL: [f32; 4] = [0.0037037037, 1.7777778, 1.0, 0.5];
/// `vDepthParam.xy`: the depth target's linear depth is `1 / (d * x + y)`.
pub const DEPTH_PARAM: [f32; 2] = [-0.02, 0.02];
/// `vDepthFadeParam`: the thickest surface a ray hits (units), and the scale of its fade.
pub const DEPTH_FADE: [f32; 3] = [100.0, 0.01, 0.0];
/// `vSampleEdgeFadeParam` at High: the steps' fade (`i * x + y`: from step 128) and the screen edge's
/// (`d * z + w`, `d` the hit's distance from the middle in the larger axis).
pub const EDGE_FADE: [f32; 4] = [0.0078125, -1.0, 20.0, -9.0];
/// The pass reads nothing as a surface past the occlusion's far limit (`ao::DEPTH_FAR`).
pub const SKY_DEPTH: f32 = crate::ao::DEPTH_FAR;
/// `vMerge.x`: the reflection's weight on the specular ambient (times its alpha).
pub const MERGE_X: f32 = 1.0;
/// `vAmbParam.z`: the cube map's last mip, which the reflections' chain matches.
pub const MIP_LAST: f32 = (LEVELS - 1) as f32;

/// One level of the game's reflection quality (`rlrpreset.xml`: `MaxRaySamples` 256 / 128 / 64,
/// `PixelsPerStep` 1 / 2 / 4 for High, Middle, Low). The game's constants follow from them as
/// captured at each level (the answer key's `gs-q2`, `gs-q1`, `gs-q0` Rasenboden frames):
/// `vReflParam.x` is `PixelsPerStep` times High's step, and `vSampleEdgeFadeParam.x` (the steps'
/// fade, which ends at the last step) is two over `MaxRaySamples`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rlr {
    pub max_ray_samples: u32,
    pub pixels_per_step: u32,
}

impl Rlr {
    pub const HIGH: Rlr = Rlr { max_ray_samples: MAX_STEPS, pixels_per_step: 1 };

    /// `vReflParam` at this level.
    pub fn refl(&self) -> [f32; 4] {
        [REFL[0] * self.pixels_per_step as f32, REFL[1], REFL[2], REFL[3]]
    }

    /// `vSampleEdgeFadeParam` at this level.
    pub fn edge_fade(&self) -> [f32; 4] {
        [EDGE_FADE[0] * MAX_STEPS as f32 / self.max_ray_samples as f32, EDGE_FADE[1], EDGE_FADE[2], EDGE_FADE[3]]
    }
}

const MARCH_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/reflect.frag.spv"));
const MIP_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/reflect_mip.frag.spv"));
const APPLY_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/reflect_apply.frag.spv"));

/// The size of the level `k` of a chain whose first level is `size`.
fn level_size(size: (u32, u32), k: u32) -> (u32, u32) {
    ((size.0 >> k).max(1), (size.1 >> k).max(1))
}

/// The passes, pipelines and targets.
pub struct Reflections {
    /// One colour target of `FORMAT`, left readable: the march and each mip level.
    pass: vk::RenderPass,
    /// The scene's colour, loaded and added to; it ends readable (the post pass reads it) or,
    /// in a frame with water, as the copy's source.
    apply_passes: [vk::RenderPass; 2],
    layouts: [vk::PipelineLayout; 3],
    set_layouts: [vk::DescriptorSetLayout; 3],
    pipelines: [vk::Pipeline; 3],
    modules: [vk::ShaderModule; 4],
    point: vk::Sampler,
    linear: vk::Sampler,
    chain: vk::Sampler,
    pool: vk::DescriptorPool,
    /// The reflection quality level the march runs at.
    rlr: Rlr,
    maps: Option<Maps>,
}

/// The targets of one scene size.
struct Maps {
    size: (u32, u32),
    scene_size: (u32, u32),
    image: (vk::Image, vk::DeviceMemory),
    whole_view: vk::ImageView,
    level_views: Vec<vk::ImageView>,
    level_fbs: Vec<vk::Framebuffer>,
    apply_fb: vk::Framebuffer,
    march_set: vk::DescriptorSet,
    mip_sets: Vec<vk::DescriptorSet>,
    apply_set: vk::DescriptorSet,
}

impl Reflections {
    /// The passes and pipelines, for a scene in `scene_format`.
    pub fn new(gpu: &Gpu, scene_format: vk::Format, rlr: Rlr) -> Result<Reflections> {
        let d = &gpu.device;
        let shader_read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let colour_ref = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour_ref)];
        // What reads a target after the pass: a fragment shader, or the copy of the scene.
        let done = vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TRANSFER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_READ);
        let attachment = |format, load, initial, last| {
            vk::AttachmentDescription::default().format(format).samples(vk::SampleCountFlags::TYPE_1).load_op(load).store_op(vk::AttachmentStoreOp::STORE).initial_layout(initial).final_layout(last)
        };
        let target = [attachment(FORMAT, vk::AttachmentLoadOp::DONT_CARE, vk::ImageLayout::UNDEFINED, shader_read)];
        let target_deps = [done];
        let pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&target).subpasses(&subpass).dependencies(&target_deps), None)? };
        // The scene's colour, read by the march just before: the write waits for those reads.
        let after_reads = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
        let apply_deps = [after_reads, done];
        let apply_pass = |last| -> Result<vk::RenderPass> {
            let scene = [attachment(scene_format, vk::AttachmentLoadOp::LOAD, shader_read, last)];
            Ok(unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&scene).subpasses(&subpass).dependencies(&apply_deps), None)? })
        };
        let apply_passes = [apply_pass(shader_read)?, apply_pass(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)?];

        let words = |code: &[u8]| -> Vec<u32> { code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() };
        let module = |code: &[u8]| unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words(code)), None) };
        let modules = [module(crate::POST_VERT)?, module(MARCH_FRAG)?, module(MIP_FRAG)?, module(APPLY_FRAG)?];
        // Sets: the march (normals and depth, the scene), a mip level (the level above), the
        // application (the two per-pixel targets, the chain).
        let set_layout = |count: u32| unsafe {
            let bindings: Vec<_> = (0..count)
                .map(|b| vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT))
                .collect();
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)
        };
        let set_layouts = [set_layout(2)?, set_layout(1)?, set_layout(3)?];
        let layout = |set: vk::DescriptorSetLayout, push: u32| unsafe {
            let sets = [set];
            let ranges = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: push }];
            d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sets).push_constant_ranges(&ranges), None)
        };
        let layouts = [layout(set_layouts[0], 96)?, layout(set_layouts[1], 8)?, layout(set_layouts[2], 8)?];
        let input = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
        let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let plain = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
        // The application adds to the scene's colour (not its alpha).
        let adding = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B)];
        let plain_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&plain);
        let adding_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&adding);
        let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
        let stages = |fragment| {
            [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main"),
            ]
        };
        let (march_stages, mip_stages, apply_stages) = (stages(modules[1]), stages(modules[2]), stages(modules[3]));
        let base = vk::GraphicsPipelineCreateInfo::default()
            .vertex_input_state(&input)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&msaa)
            .dynamic_state(&dynamic_state);
        let infos = [
            base.stages(&march_stages).color_blend_state(&plain_blend).layout(layouts[0]).render_pass(pass),
            base.stages(&mip_stages).color_blend_state(&plain_blend).layout(layouts[1]).render_pass(pass),
            base.stages(&apply_stages).color_blend_state(&adding_blend).layout(layouts[2]).render_pass(apply_passes[0]),
        ];
        let made = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &infos, None).map_err(|(_, e)| e)? };
        let pipelines = [made[0], made[1], made[2]];

        let sampler = |filter, mipmap, max_lod| unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(filter)
                    .min_filter(filter)
                    .mipmap_mode(mipmap)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(max_lod),
                None,
            )
        };
        // The normals and depth are fetched by texel; the scene and the chain are read
        // bilinearly (the chain across its levels by the lod).
        let point = sampler(vk::Filter::NEAREST, vk::SamplerMipmapMode::NEAREST, 0.0)?;
        let linear = sampler(vk::Filter::LINEAR, vk::SamplerMipmapMode::NEAREST, 0.0)?;
        let chain = sampler(vk::Filter::LINEAR, vk::SamplerMipmapMode::LINEAR, MIP_LAST)?;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 2 + (LEVELS - 1) + 3 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(LEVELS + 1).pool_sizes(&sizes), None)? };
        Ok(Reflections { pass, apply_passes, layouts, set_layouts, pipelines, modules, point, linear, chain, pool, rlr, maps: None })
    }

    /// The targets for a scene of `scene_size` drawn into `scene` (a view of its colour image),
    /// reading the occlusion's normals and depth `gbuf` and the scene pass's two per-pixel
    /// targets `spec`; the old ones are gone first.
    pub fn bind(&mut self, gpu: &Gpu, scene_size: (u32, u32), gbuf: vk::ImageView, scene: vk::ImageView, spec: [vk::ImageView; 2]) -> Result<()> {
        self.free_maps(gpu);
        let d = &gpu.device;
        let size = half(scene_size);
        let (image, memory) = gpu.image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(FORMAT)
                .extent(vk::Extent3D { width: size.0, height: size.1, depth: 1 })
                .mip_levels(LEVELS)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC),
        )?;
        let view = |base: u32, count: u32| unsafe {
            d.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(FORMAT)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).base_mip_level(base).level_count(count).layer_count(1)),
                None,
            )
        };
        let whole_view = view(0, LEVELS)?;
        let level_views = (0..LEVELS).map(|k| view(k, 1)).collect::<std::result::Result<Vec<_>, _>>()?;
        let framebuffer = |pass, v: vk::ImageView, (w, h): (u32, u32)| unsafe {
            let views = [v];
            d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(w).height(h).layers(1), None)
        };
        let level_fbs = (0..LEVELS).map(|k| framebuffer(self.pass, level_views[k as usize], level_size(size, k))).collect::<std::result::Result<Vec<_>, _>>()?;
        let apply_fb = framebuffer(self.apply_passes[0], scene, scene_size)?;
        let layouts: Vec<_> = std::iter::once(self.set_layouts[0]).chain((1..LEVELS).map(|_| self.set_layouts[1])).chain([self.set_layouts[2]]).collect();
        let sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(self.pool).set_layouts(&layouts))? };
        let write = |set, binding, sampler, view| {
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            unsafe { d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]) };
        };
        let march_set = sets[0];
        write(march_set, 0, self.point, gbuf);
        write(march_set, 1, self.linear, scene);
        // Level k + 1 reads level k.
        let mip_sets = sets[1..LEVELS as usize].to_vec();
        for (k, &set) in mip_sets.iter().enumerate() {
            write(set, 0, self.linear, level_views[k]);
        }
        let apply_set = sets[LEVELS as usize];
        write(apply_set, 0, self.point, spec[0]);
        write(apply_set, 1, self.point, spec[1]);
        write(apply_set, 2, self.chain, whole_view);
        self.maps = Some(Maps { size, scene_size, image: (image, memory), whole_view, level_views, level_fbs, apply_fb, march_set, mip_sets, apply_set });
        Ok(())
    }

    fn free_maps(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        if let Some(m) = self.maps.take() {
            unsafe {
                let _ = d.device_wait_idle();
                let mut sets = vec![m.march_set, m.apply_set];
                sets.extend(&m.mip_sets);
                let _ = d.free_descriptor_sets(self.pool, &sets);
                for fb in m.level_fbs.into_iter().chain([m.apply_fb]) {
                    d.destroy_framebuffer(fb, None);
                }
                for v in m.level_views.into_iter().chain([m.whole_view]) {
                    d.destroy_image_view(v, None);
                }
                d.destroy_image(m.image.0, None);
                d.free_memory(m.image.1, None);
            }
        }
    }

    /// The targets again, bound to the occlusion's normals and depth and to `targets` (the scene
    /// and its per-pixel targets).
    pub fn rebind(&mut self, gpu: &Gpu, ao: &crate::ao::Occlusion, targets: &crate::Targets) -> Result<()> {
        let (Some(gbuf), Some(spec)) = (ao.gbuf_view(), targets.spec.as_ref()) else { return Ok(()) };
        self.bind(gpu, targets.scene_size, gbuf, targets.scene_view, [spec[0].2, spec[1].2])
    }

    /// The march target's image and size, for the stage dump (`reflections`).
    pub fn image(&self) -> Option<(vk::Image, (u32, u32))> {
        self.maps.as_ref().map(|m| (m.image.0, m.size))
    }

    /// The march, the chain and the application on the scene, which the pass before left
    /// readable (and leaves readable, or with `to_copy` ready to be copied from).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the scene pass and the occlusion's
    /// normals and depth were drawn, for the view `view`.
    pub unsafe fn record(&self, gpu: &Gpu, cb: vk::CommandBuffer, view: &View, to_copy: bool) {
        let Some(m) = self.maps.as_ref() else { return };
        let d = &gpu.device;
        let draw = |pass, fb, size: (u32, u32), pipeline, layout, set, push: &[f32]| unsafe {
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: size.0, height: size.1 } };
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(fb).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: size.0 as f32, height: size.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set], &[]);
            d.cmd_push_constants(cb, layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, push.len() * 4));
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        };
        let inverse = |size: (u32, u32)| [1.0 / size.0 as f32, 1.0 / size.1 as f32];
        let mut push = [0f32; 24];
        for (k, row) in view.rows.iter().enumerate() {
            push[4 * k..4 * k + 4].copy_from_slice(row);
        }
        push[12..16].copy_from_slice(&view.scale);
        let [iw, ih] = inverse(m.size);
        push[16..20].copy_from_slice(&[m.size.0 as f32, m.size.1 as f32, iw, ih]);
        push[20..24].copy_from_slice(&[self.rlr.max_ray_samples as f32, self.rlr.pixels_per_step as f32, 0.0, 0.0]);
        draw(self.pass, m.level_fbs[0], m.size, self.pipelines[0], self.layouts[0], m.march_set, &push);
        for k in 1..LEVELS {
            let size = level_size(m.size, k);
            draw(self.pass, m.level_fbs[k as usize], size, self.pipelines[1], self.layouts[1], m.mip_sets[k as usize - 1], &inverse(size));
        }
        draw(self.apply_passes[usize::from(to_copy)], m.apply_fb, m.scene_size, self.pipelines[2], self.layouts[2], m.apply_set, &inverse(m.scene_size));
    }

    pub fn destroy(mut self, gpu: &Gpu) {
        self.free_maps(gpu);
        let d = &gpu.device;
        unsafe {
            d.destroy_descriptor_pool(self.pool, None);
            for s in [self.point, self.linear, self.chain] {
                d.destroy_sampler(s, None);
            }
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
            d.destroy_render_pass(self.pass, None);
            for p in self.apply_passes {
                d.destroy_render_pass(p, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers of `const <type> NAME = [vecN](a, b, ...);` in a shader.
    fn numbers(source: &str, name: &str) -> Vec<f32> {
        let line = source.lines().find(|l| l.contains(&format!(" {name} = "))).unwrap_or_else(|| panic!("no {name}"));
        let rhs = line.split(" = ").nth(1).unwrap().split(';').next().unwrap();
        rhs.split_once('(').map_or(rhs, |(_, inner)| inner.trim_end_matches(')')).split(',').map(|v| v.trim().parse().unwrap_or_else(|_| panic!("{name}: {v}"))).collect()
    }

    #[test]
    fn the_march_holds_the_same_constants() {
        let source = include_str!("../shaders/reflect.frag");
        assert_eq!(numbers(source, "MAX_STEPS"), [MAX_STEPS as f32]);
        assert_eq!(numbers(source, "REFL"), REFL);
        assert_eq!(numbers(source, "DEPTH_PARAM"), DEPTH_PARAM);
        assert_eq!(numbers(source, "DEPTH_FADE"), DEPTH_FADE);
        assert_eq!(numbers(source, "EDGE_FADE"), EDGE_FADE);
        assert_eq!(numbers(source, "SKY_DEPTH"), [SKY_DEPTH]);
    }

    #[test]
    fn the_levels_give_the_captured_constants() {
        // The game's `vReflParam.x` and `vSampleEdgeFadeParam.x` at Middle and Low (gs-q1, gs-q0).
        let middle = Rlr { max_ray_samples: 128, pixels_per_step: 2 };
        let low = Rlr { max_ray_samples: 64, pixels_per_step: 4 };
        assert_eq!((Rlr::HIGH.refl(), Rlr::HIGH.edge_fade()), (REFL, EDGE_FADE));
        assert!((middle.refl()[0] - 0.0074074).abs() < 1e-6 && (middle.edge_fade()[0] - 0.015625).abs() < 1e-9);
        assert!((low.refl()[0] - 0.0148148).abs() < 1e-6 && (low.edge_fade()[0] - 0.03125).abs() < 1e-9);
    }

    #[test]
    fn the_application_holds_the_same_constants() {
        let source = include_str!("../shaders/reflect_apply.frag");
        assert_eq!(numbers(source, "MERGE_X"), [MERGE_X]);
        assert_eq!(numbers(source, "MIP_LAST"), [MIP_LAST]);
    }

    #[test]
    fn the_chain_runs_from_960_by_540_to_30_by_16() {
        let size = half((1920, 1080));
        assert_eq!(level_size(size, 0), (960, 540));
        assert_eq!(level_size(size, LEVELS - 1), (30, 16));
    }

    #[test]
    fn the_depth_terms_are_the_games_linear_depth() {
        // 1 / (d * -0.02 + 0.02) = 50 / (1 - d): the near plane at 0, 1000 units at 0.95.
        let lin = |d: f32| 1.0 / (d * DEPTH_PARAM[0] + DEPTH_PARAM[1]);
        assert!((lin(0.0) - 50.0).abs() < 1e-3);
        assert!((lin(0.95) - 1000.0).abs() < 0.1);
    }
}
