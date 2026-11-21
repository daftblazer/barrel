//! The screen-space ambient occlusion (`docs/renderer.md`, "Ambient occlusion"; the game's
//! own pass: `docs/shading.md`, "The ambient occlusion"). The scene is drawn first at half size
//! into a target of world normal and linear depth (the game reads the same from its
//! G-buffer), a full-screen pass turns those into the occlusion (`ao.frag`), and the main pass's
//! fragments read it, stretched over the scene, in the light they take.

use ash::vk;
use glam::{Mat4, Vec3};

use crate::{Gpu, Result};

/// The normal and linear depth target: a world normal in rgb, the depth along the view in a.
pub const GBUF_FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
/// The scene pass's world normal, octahedral in two channels (`normal_octahedral.glsl`): a half
/// float each keeps the direction to a milli-radian at four bytes a pixel.
/// The view depth alone, which the derive pass writes beside the normal and depth target for the
/// reflections' march: each of its steps reads 4 bytes, not the target's 16.
pub const MARCH_DEPTH_FORMAT: vk::Format = vk::Format::R32_SFLOAT;
pub const NORMAL_FORMAT: vk::Format = vk::Format::R16G16_SFLOAT;
/// The occlusion (red; the others repeat it, for the stage dumps).
pub const MAP_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

// The constants of the game's occlusion program as captured (Rasenboden at noon, its 960 by 540
// target): `ao.frag` holds the same numbers, and a test checks they agree.
/// `vDepthParam.z`: nothing past it is shaded.
pub const DEPTH_FAR: f32 = 299910.0;
/// `vRadiusParam.w`: the march's longest reach, in pixels of the half-size target.
pub const MAX_PIXELS: f32 = 54.0;
/// `vRadiusParam2`: the radius near and far, and its slope with the depth.
pub const RADIUS: [f32; 4] = [50.0, 200.0, -0.00066666666, -0.33333334];
/// `vRayParam`: the directions, the steps, the turn a direction as a fraction, the bias (the
/// tangent of 10 degrees).
pub const RAY: [f32; 4] = [8.0, 6.0, 0.125, 0.17632698];
/// `vOccParam.xy`: the occlusion's scale and ceiling.
pub const OCC: [f32; 2] = [2.5, 0.8];

/// The depth the target is cleared to: past the pass's far limit, so nothing there is shaded.
pub const CLEAR_DEPTH: f32 = 1.0e9;

/// The size of the occlusion target for a scene of `size`: half, rounded up.
pub fn half(size: (u32, u32)) -> (u32, u32) {
    (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1))
}

/// What `ao.frag` takes (its push constants): the rotation from the world to the game's view
/// axes (x right, y up, z toward the viewer: the rows), the position's scales and the target's
/// size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub rows: [[f32; 4]; 3],
    pub scale: [f32; 4],
    /// A temporal resolve (TAA, FSR 3, GSR 2) follows: the noise's 4 by 4 pattern moves every
    /// frame so the resolve averages it away (`ao.frag`, `noise`).
    pub temporal: bool,
}

impl View {
    /// From the frame's view-projection (the projection's x and y scales sit on its diagonal and
    /// its w row is the view's depth, so the view's rotation falls out of the rows). The render
    /// space is mirrored in x (the projection's x scale is negative); the game's axes are not.
    pub fn of(vp: &Mat4) -> View {
        let row = |i: usize| Vec3::new(vp.x_axis[i], vp.y_axis[i], vp.z_axis[i]);
        let (x, y, w) = (row(0), row(1), row(3));
        // The projection's scales: the rows' lengths over the unit view rotation they hold.
        let (sx, sy) = (x.length().max(1e-9), y.length().max(1e-9));
        let rx = x / sx;
        let ry = -y / sy;
        // z toward the viewer is the view's z (the depth is minus it).
        let rz = -w;
        View {
            rows: [[rx.x, rx.y, rx.z, 0.0], [ry.x, ry.y, ry.z, 0.0], [rz.x, rz.y, rz.z, 0.0]],
            // vViewParam: x and y of the position per unit depth over the uv: 2 tan to the right and
            // minus 2 tan down, from minus tan and tan.
            scale: [2.0 / sx, -2.0 / sy, -1.0 / sx, 1.0 / sy],
            temporal: false,
        }
    }
}

/// The view depth of a device depth `d` as `1 / (x + y * d)`, returned as `[x, y]`, for a
/// perspective projection in `vp`: the depth row is `alpha * w + beta`, so `d = alpha + beta / w`.
fn device_depth_to_view(vp: &Mat4) -> [f32; 2] {
    let row = |i: usize| glam::Vec4::new(vp.x_axis[i], vp.y_axis[i], vp.z_axis[i], vp.w_axis[i]);
    let (z, w) = (row(2), row(3));
    let alpha = z.truncate().dot(w.truncate()) / w.truncate().length_squared();
    let beta = z.w - alpha * w.w;
    [-alpha / beta, 1.0 / beta]
}

impl Derive {
    #[allow(clippy::too_many_arguments)]
    fn new(
        gpu: &Gpu,
        input: &vk::PipelineVertexInputStateCreateInfo,
        assembly: &vk::PipelineInputAssemblyStateCreateInfo,
        viewport: &vk::PipelineViewportStateCreateInfo,
        raster: &vk::PipelineRasterizationStateCreateInfo,
        msaa: &vk::PipelineMultisampleStateCreateInfo,
        dynamic_state: &vk::PipelineDynamicStateCreateInfo,
        shader_read: vk::ImageLayout,
    ) -> Result<Derive> {
        let d = &gpu.device;
        // Every texel is written: nothing to load. The normal and depth target, then the view
        // depth alone (`MARCH_DEPTH_FORMAT`).
        let target = |format| {
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(shader_read)
        };
        let attachment = [target(GBUF_FORMAT), target(MARCH_DEPTH_FORMAT)];
        let colour = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }, vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colour)];
        let to_shader = [vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)];
        let pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&to_shader), None)? };
        let binding = |i| vk::DescriptorSetLayoutBinding::default().binding(i).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let bindings = [binding(0), binding(1)];
        let set_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)? };
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 2 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None)? };
        let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[set_layout]))?[0] };
        let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 16 }];
        let set_layouts = [set_layout];
        let pipeline_layout = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push), None)? };
        let words = |code: &[u8]| -> Vec<u32> { code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() };
        let module = unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words(crate::GBUF_DERIVE_FRAG)), None)? };
        let post = unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words(crate::POST_VERT)), None)? };
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(post).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(module).name(c"main"),
        ];
        let writes = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA); 2];
        let blend = &vk::PipelineColorBlendStateCreateInfo::default().attachments(&writes);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(input)
            .input_assembly_state(assembly)
            .viewport_state(viewport)
            .rasterization_state(raster)
            .multisample_state(msaa)
            .color_blend_state(blend)
            .dynamic_state(dynamic_state)
            .layout(pipeline_layout)
            .render_pass(pass);
        let pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0] };
        // The vertex program is only needed to build the pipeline.
        unsafe { d.destroy_shader_module(post, None) };
        Ok(Derive { pass, pipeline_layout, pipeline, module, set_layout, pool, set })
    }
}

/// The passes, pipeline and targets.
pub struct Occlusion {
    pub gbuf_pass: vk::RenderPass,
    map_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    point: vk::Sampler,
    linear: vk::Sampler,
    pool: vk::DescriptorPool,
    set_layout: vk::DescriptorSetLayout,
    /// Whether the targets are the scene's half size (the setting is on) or one texel.
    on: bool,
    /// The pass that builds the half-size target from the scene pass's depth and normal, where
    /// there is no geometry pre-pass.
    derive: Option<Derive>,
    maps: Option<Maps>,
    /// Frames shaded with a temporal resolve after them (the noise's offset, `View::temporal`).
    noise_frame: std::cell::Cell<u32>,
}

/// The pass, pipeline and inputs of [`Occlusion::derive`].
struct Derive {
    pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    module: vk::ShaderModule,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    /// The scene's depth and normal, bound by [`Occlusion::bind_scene`].
    set: vk::DescriptorSet,
}

/// The targets of one size.
struct Maps {
    size: (u32, u32),
    gbuf: (vk::Image, vk::DeviceMemory, vk::ImageView),
    depth: (vk::Image, vk::DeviceMemory, vk::ImageView),
    map: (vk::Image, vk::DeviceMemory, vk::ImageView),
    gbuf_fb: vk::Framebuffer,
    map_fb: vk::Framebuffer,
    /// The normal and depth target and the march's depth, for the derive pass.
    derive_fb: Option<vk::Framebuffer>,
    /// The view depth for the reflections' march: the derive pass's own target, or without the
    /// derive pass the normal and depth target with its alpha read as red.
    march_depth: Option<(vk::Image, vk::DeviceMemory)>,
    march_view: vk::ImageView,
    /// The pass reads the normal target through this one; the main pass the occlusion through
    /// `map_set`.
    gbuf_set: vk::DescriptorSet,
    map_set: vk::DescriptorSet,
}

impl Occlusion {
    /// The passes and the pipeline; `on`: the targets are the scene's half size (for a scene of
    /// `scene_size`), else a white texel every frame. `set_layout` is the one-sampler layout of
    /// the texture sets. `from_scene`: the target is built from the scene pass's depth and normal
    /// ([`Occlusion::derive`]) in place of a geometry pre-pass of its own.
    pub fn new(gpu: &Gpu, set_layout: vk::DescriptorSetLayout, on: bool, from_scene: bool, scene_size: (u32, u32)) -> Result<Occlusion> {
        let d = &gpu.device;
        let colour = |format, to: vk::ImageLayout| {
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(to)
        };
        let to_shader = [vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)];
        let shader_read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let gbuf_attachments = [
            colour(GBUF_FORMAT, shader_read),
            vk::AttachmentDescription::default()
                .format(crate::DEPTH_FORMAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        ];
        let gbuf_colour = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let gbuf_depth = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
        let gbuf_subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&gbuf_colour).depth_stencil_attachment(&gbuf_depth)];
        let gbuf_pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&gbuf_attachments).subpasses(&gbuf_subpass).dependencies(&to_shader), None)? };
        let map_attachments = [colour(MAP_FORMAT, shader_read)];
        let map_subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&gbuf_colour)];
        let map_pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&map_attachments).subpasses(&map_subpass).dependencies(&to_shader), None)? };

        let words = |code: &[u8]| -> Vec<u32> { code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() };
        let module = |code: &[u8]| unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words(code)), None) };
        let modules = [module(crate::POST_VERT)?, module(crate::AO_FRAG)?];
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(modules[1]).name(c"main"),
        ];
        let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 80 }];
        let set_layouts = [set_layout];
        let pipeline_layout = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push), None)? };
        let input = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
        let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend_attachment = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachment);
        let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&input)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&msaa)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(map_pass);
        let pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0] };

        let sampler = |filter, anisotropy: bool| unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(filter)
                    .min_filter(filter)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .anisotropy_enable(anisotropy),
                None,
            )
        };
        // The normal target is fetched by texel; the occlusion is stretched over the scene
        // bilinearly, as the game does.
        let point = sampler(vk::Filter::NEAREST, false)?;
        let linear = sampler(vk::Filter::LINEAR, false)?;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 2 }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(2).pool_sizes(&sizes), None)? };
        let derive = if on && from_scene { Some(Derive::new(gpu, &input, &assembly, &viewport, &raster, &msaa, &dynamic_state, shader_read)?) } else { None };
        let mut o = Occlusion { gbuf_pass, map_pass, pipeline_layout, pipeline, modules, point, linear, pool, set_layout, on, derive, maps: None, noise_frame: std::cell::Cell::new(0) };
        o.make_maps(gpu, scene_size)?;
        Ok(o)
    }

    fn make_maps(&mut self, gpu: &Gpu, scene_size: (u32, u32)) -> Result<()> {
        let d = &gpu.device;
        let size = if self.on { half(scene_size) } else { (1, 1) };
        let image = |format, usage| {
            gpu.image(&vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(format).extent(vk::Extent3D { width: size.0, height: size.1, depth: 1 }).mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1).usage(usage))
        };
        let view = |image, format, aspect| unsafe {
            d.create_image_view(
                &vk::ImageViewCreateInfo::default().image(image).view_type(vk::ImageViewType::TYPE_2D).format(format).subresource_range(vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(1).layer_count(1)),
                None,
            )
        };
        let colour = vk::ImageAspectFlags::COLOR;
        let (gi, gm) = image(GBUF_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)?;
        let gbuf = (gi, gm, view(gi, GBUF_FORMAT, colour)?);
        let (di, dm) = image(crate::DEPTH_FORMAT, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT)?;
        let depth = (di, dm, view(di, crate::DEPTH_FORMAT, vk::ImageAspectFlags::DEPTH)?);
        let (mi, mm) = image(MAP_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)?;
        let map = (mi, mm, view(mi, MAP_FORMAT, colour)?);
        let framebuffer = |pass, views: &[vk::ImageView]| unsafe { d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(views).width(size.0).height(size.1).layers(1), None) };
        let gbuf_fb = framebuffer(self.gbuf_pass, &[gbuf.2, depth.2])?;
        let map_fb = framebuffer(self.map_pass, &[map.2])?;
        let (march_depth, march_view) = if self.derive.is_some() {
            let (i, m) = image(MARCH_DEPTH_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)?;
            (Some((i, m)), view(i, MARCH_DEPTH_FORMAT, colour)?)
        } else {
            let alpha = vk::ComponentMapping { r: vk::ComponentSwizzle::A, g: vk::ComponentSwizzle::A, b: vk::ComponentSwizzle::A, a: vk::ComponentSwizzle::A };
            let info = vk::ImageViewCreateInfo::default().image(gi).view_type(vk::ImageViewType::TYPE_2D).format(GBUF_FORMAT).components(alpha).subresource_range(vk::ImageSubresourceRange::default().aspect_mask(colour).level_count(1).layer_count(1));
            (None, unsafe { d.create_image_view(&info, None)? })
        };
        let derive_fb = self.derive.as_ref().map(|dv| framebuffer(dv.pass, &[gbuf.2, march_view])).transpose()?;
        let layouts = [self.set_layout, self.set_layout];
        let sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(self.pool).set_layouts(&layouts))? };
        let write = |set, sampler, view| {
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            unsafe { d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]) };
        };
        write(sets[0], self.point, gbuf.2);
        write(sets[1], self.linear, map.2);
        self.maps = Some(Maps { size, gbuf, depth, map, gbuf_fb, map_fb, derive_fb, march_depth, march_view, gbuf_set: sets[0], map_set: sets[1] });
        Ok(())
    }

    fn free_maps(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        if let Some(m) = self.maps.take() {
            unsafe {
                // The sets go back to the pool with the targets they point at.
                let _ = d.free_descriptor_sets(self.pool, &[m.gbuf_set, m.map_set]);
                d.destroy_framebuffer(m.gbuf_fb, None);
                d.destroy_framebuffer(m.map_fb, None);
                if let Some(fb) = m.derive_fb {
                    d.destroy_framebuffer(fb, None);
                }
                d.destroy_image_view(m.march_view, None);
                if let Some((image, memory)) = m.march_depth {
                    d.destroy_image(image, None);
                    d.free_memory(memory, None);
                }
                for (image, memory, view) in [m.gbuf, m.depth, m.map] {
                    d.destroy_image_view(view, None);
                    d.destroy_image(image, None);
                    d.free_memory(memory, None);
                }
            }
        }
    }

    /// The targets again for a scene of `scene_size` (the device idle first).
    pub fn resize(&mut self, gpu: &Gpu, scene_size: (u32, u32)) -> Result<()> {
        unsafe { gpu.device.device_wait_idle()? };
        self.free_maps(gpu);
        self.make_maps(gpu, scene_size)
    }

    /// Whether the occlusion is drawn (the setting is on): else the map is one white texel.
    pub fn on(&self) -> bool {
        self.on
    }

    /// The set the main pass reads the occlusion through (set 5 of the model layout, set 2 of
    /// the terrain's).
    pub fn map_set(&self) -> vk::DescriptorSet {
        self.maps.as_ref().map_or(vk::DescriptorSet::null(), |m| m.map_set)
    }

    /// The set the water reads the opaque scene's normals and view depth (alpha) through, at half
    /// the scene's size: set 2 of the model layout.
    pub fn gbuf_set(&self) -> vk::DescriptorSet {
        self.maps.as_ref().map_or(vk::DescriptorSet::null(), |m| m.gbuf_set)
    }

    /// The normal and depth target's view, for the reflections' march.
    pub fn gbuf_view(&self) -> Option<vk::ImageView> {
        self.maps.as_ref().map(|m| m.gbuf.2)
    }

    /// The view depth alone (in red), for the reflections' march's steps.
    pub fn march_depth_view(&self) -> Option<vk::ImageView> {
        self.maps.as_ref().map(|m| m.march_view)
    }

    /// The normal and depth target's image and size, for the stage dump (`depth`).
    pub fn gbuf_image(&self) -> Option<(vk::Image, (u32, u32))> {
        self.maps.as_ref().map(|m| (m.gbuf.0, m.size))
    }

    /// The occlusion map's image, its size and the normal target's, for the stage dump.
    pub fn map_image(&self) -> Option<(vk::Image, (u32, u32))> {
        self.maps.as_ref().map(|m| (m.map.0, m.size))
    }

    /// Whether the normal and depth target is built from the scene pass ([`Occlusion::derive`]).
    pub fn from_scene(&self) -> bool {
        self.derive.is_some()
    }

    /// Points the derive pass at the scene's depth (in its shader-read layout) and normal.
    pub fn bind_scene(&self, gpu: &Gpu, depth: vk::ImageView, normal: vk::ImageView) {
        let Some(dv) = &self.derive else { return };
        let info = |view| [vk::DescriptorImageInfo::default().sampler(self.point).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let (depth, normal) = (info(depth), info(normal));
        let writes = [
            vk::WriteDescriptorSet::default().dst_set(dv.set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&depth),
            vk::WriteDescriptorSet::default().dst_set(dv.set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&normal),
        ];
        unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
    }

    /// The half-size normal and depth target from the scene pass's depth and normal, for the
    /// frame's view-projection `vp`: one full-screen pass (the geometry is not drawn again).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass; the scene's depth is in its shader-read layout
    /// and its normal target was drawn.
    pub unsafe fn derive(&self, gpu: &Gpu, cb: vk::CommandBuffer, vp: &Mat4) {
        let (Some(dv), Some(m)) = (&self.derive, &self.maps) else { return };
        let Some(fb) = m.derive_fb else { return };
        let d = &gpu.device;
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: m.size.0, height: m.size.1 } };
        let [x, y] = device_depth_to_view(vp);
        let push = [x, y, CLEAR_DEPTH, 0.0];
        unsafe {
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(dv.pass).framebuffer(fb).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: m.size.0 as f32, height: m.size.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, dv.pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, dv.pipeline_layout, 0, &[dv.set], &[]);
            d.cmd_push_constants(cb, dv.pipeline_layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, 16));
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        }
    }

    /// Begins the pass that draws the normals and depth.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass.
    pub unsafe fn begin_gbuf(&self, gpu: &Gpu, cb: vk::CommandBuffer) {
        let m = self.maps.as_ref().expect("targets");
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: m.size.0, height: m.size.1 } };
        let clear = [
            vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 1.0, CLEAR_DEPTH] } },
            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
        ];
        let d = &gpu.device;
        unsafe {
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(self.gbuf_pass).framebuffer(m.gbuf_fb).render_area(area).clear_values(&clear), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: m.size.0 as f32, height: m.size.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
        }
    }

    /// The occlusion from the target just drawn, for the view `view`; `None`: the map cleared to
    /// 1 (no occlusion).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass; the normal target was drawn if `view` is given.
    pub unsafe fn shade(&self, gpu: &Gpu, cb: vk::CommandBuffer, view: Option<&View>) {
        let m = self.maps.as_ref().expect("targets");
        let d = &gpu.device;
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: m.size.0, height: m.size.1 } };
        let clear = [vk::ClearValue { color: vk::ClearColorValue { float32: [1.0; 4] } }];
        unsafe {
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(self.map_pass).framebuffer(m.map_fb).render_area(area).clear_values(&clear), vk::SubpassContents::INLINE);
            if let Some(v) = view {
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: m.size.0 as f32, height: m.size.1 as f32, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[area]);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline_layout, 0, &[m.gbuf_set], &[]);
                let res = [m.size.0 as f32, m.size.1 as f32, 1.0 / m.size.0 as f32, 1.0 / m.size.1 as f32];
                let mut push = [0f32; 20];
                for (k, row) in v.rows.iter().enumerate() {
                    push[4 * k..4 * k + 4].copy_from_slice(row);
                }
                push[12..16].copy_from_slice(&v.scale);
                // The noise's offset this frame (in the rows' unused w): 0 without a temporal
                // resolve, else each of the pattern's 16 places in turn.
                if v.temporal {
                    let n = self.noise_frame.get();
                    self.noise_frame.set(n.wrapping_add(1));
                    (push[3], push[7]) = ((n % 4) as f32, ((n / 4) % 4) as f32);
                }
                push[16..20].copy_from_slice(&res);
                d.cmd_push_constants(cb, self.pipeline_layout, vk::ShaderStageFlags::FRAGMENT, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, 80));
                d.cmd_draw(cb, 3, 1, 0, 0);
            }
            d.cmd_end_render_pass(cb);
        }
    }

    pub fn destroy(mut self, gpu: &Gpu) {
        self.free_maps(gpu);
        let d = &gpu.device;
        unsafe {
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_sampler(self.point, None);
            d.destroy_sampler(self.linear, None);
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_render_pass(self.gbuf_pass, None);
            d.destroy_render_pass(self.map_pass, None);
            if let Some(dv) = self.derive.take() {
                d.destroy_descriptor_pool(dv.pool, None);
                d.destroy_descriptor_set_layout(dv.set_layout, None);
                d.destroy_pipeline(dv.pipeline, None);
                d.destroy_pipeline_layout(dv.pipeline_layout, None);
                d.destroy_shader_module(dv.module, None);
                d.destroy_render_pass(dv.pass, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shader_holds_the_same_constants() {
        let source = include_str!("../shaders/ao.frag");
        // The numbers of `const <type> NAME = [vecN](a, b, ...);`.
        let numbers = |name: &str| -> Vec<f32> {
            let line = source.lines().find(|l| l.contains(&format!(" {name} = "))).unwrap_or_else(|| panic!("ao.frag has no {name}"));
            let rhs = line.split(" = ").nth(1).unwrap().split(';').next().unwrap();
            rhs.split_once('(').map_or(rhs, |(_, inner)| inner.trim_end_matches(')')).split(',').map(|v| v.trim().parse().unwrap_or_else(|_| panic!("{name}: {v}"))).collect()
        };
        assert_eq!(numbers("DEPTH_FAR"), [DEPTH_FAR]);
        assert_eq!(numbers("MAX_PIXELS"), [MAX_PIXELS]);
        assert_eq!(numbers("RADIUS"), RADIUS);
        assert_eq!(numbers("RAY"), RAY);
        assert_eq!(numbers("OCC"), OCC);
    }

    #[test]
    fn device_depth_gives_the_view_depth() {
        // Depth 0..1 as the scene pass writes it: a perspective projection (mirrored in x and flipped
        // in y as the renderer's), a view looking from a point at a target.
        let mut proj = Mat4::perspective_rh(45f32.to_radians(), 16.0 / 9.0, 50.0, 300_000.0);
        proj.y_axis.y *= -1.0;
        proj.x_axis.x *= -1.0;
        let eye = Vec3::new(10.0, 20.0, 30.0);
        let view = Mat4::look_at_rh(eye, Vec3::new(-100.0, 5.0, -2000.0), Vec3::Y);
        let vp = proj * view;
        let [x, y] = device_depth_to_view(&vp);
        for p in [Vec3::new(-100.0, 5.0, -2000.0), Vec3::new(300.0, -40.0, -700.0), Vec3::new(10.0, 20.0, -90000.0)] {
            let clip = vp * p.extend(1.0);
            let want = clip.w;
            let got = 1.0 / (x + y * clip.z / clip.w);
            assert!((got - want).abs() < want * 1e-3, "{got} {want}");
        }
    }

    #[test]
    fn half_rounds_up() {
        assert_eq!(half((1920, 1080)), (960, 540));
        assert_eq!(half((1281, 721)), (641, 361));
        assert_eq!(half((1, 1)), (1, 1));
    }

    #[test]
    fn view_scales_come_from_the_projection() {
        // The game's own: fov 45 at 16:9 gives vViewParam (1.4727594, -0.82842714, -0.7363797, 0.41421357).
        let mut proj = Mat4::perspective_rh(45f32.to_radians(), 16.0 / 9.0, 50.0, 300_000.0);
        proj.y_axis.y *= -1.0;
        proj.x_axis.x *= -1.0;
        let view = Mat4::look_at_rh(Vec3::new(10.0, 20.0, 30.0), Vec3::ZERO, Vec3::Y);
        let v = View::of(&(proj * view));
        for (got, want) in v.scale.iter().zip([1.4727594, -0.82842714, -0.7363797, 0.41421357]) {
            assert!((got - want).abs() < 1e-4, "{got} {want}");
        }
        // The rows turn a world direction into the game's axes: the view's forward is -z, its
        // right +x (mirrored from the render space's).
        let fwd = (Vec3::ZERO - Vec3::new(10.0, 20.0, 30.0)).normalize();
        let row = |k: usize| Vec3::new(v.rows[k][0], v.rows[k][1], v.rows[k][2]);
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        assert!((row(2).dot(fwd) + 1.0).abs() < 1e-4);
        assert!((row(0).dot(right) + 1.0).abs() < 1e-4);
        assert!((row(1).dot(up) - 1.0).abs() < 1e-4);
    }
}
