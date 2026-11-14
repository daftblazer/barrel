//! The renderer a game draws its frames with: the scene pass (terrain, the field's models, the
//! distant view, outlines) with its shadow map, then glow, anti-aliasing and the upscalers,
//! then the post pass with the 2D layers over it, into a window's images or an offscreen one.
//! It is built for one set of graphics settings (`RenderSettings`); a change of settings
//! starts a new one.

use ash::vk;

use crate::present::Presenter;
use crate::settings::RenderSettings;
use glam::Mat4;

use crate::*;

pub struct Renderer {
    pub pool: vk::CommandPool,
    pub sampler: vk::Sampler,
    pub set_layout: vk::DescriptorSetLayout,
    pub look_ring: ring::Ring,
    pub instance_layout: vk::DescriptorSetLayout,
    pub terrain_layout: vk::DescriptorSetLayout,
    pub shadow_target: shadow::ShadowTarget,
    pub shared: scene_gpu::Shared,
    pub extent: vk::Extent3D,
    pub offscreen: Option<(vk::Image, vk::DeviceMemory)>,
    pub color_view: Option<vk::ImageView>,
    pub samples: vk::SampleCountFlags,
    pub render_pass: vk::RenderPass,
    pub post_pass: vk::RenderPass,
    pub post_set_layout: vk::DescriptorSetLayout,
    pub post_pool: vk::DescriptorPool,
    pub post_sets: [vk::DescriptorSet; 2],
    pub post_sampler: vk::Sampler,
    pub glow_pipelines: glow::Pipelines,
    pub upscale_pipelines: Option<upscale::Pipelines>,
    pub taa_pipelines: Option<taa::Pipelines>,
    pub gsr2_pipelines: Option<gsr2::Pipelines>,
    pub fsr3_pipelines: Option<fsr3::Pipelines>,
    pub timer: Option<timing::Timer>,
    pub targets: Targets,
    pub vs: vk::ShaderModule,
    pub fs: vk::ShaderModule,
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    pub sky_pipeline: vk::Pipeline,
    /// The sky plane (sky.vert/frag): drawn first among the far plane's fragments when a
    /// frame asks (`Frame::sky_plane`).
    pub sky_plane_pipeline: vk::Pipeline,
    pub sky_vs: vk::ShaderModule,
    pub sky_fs: vk::ShaderModule,
    pub outline_pipeline: vk::Pipeline,
    pub shadow_fs: vk::ShaderModule,
    pub shadow_pipeline: vk::Pipeline,
    pub post_vs: vk::ShaderModule,
    pub post_fs: vk::ShaderModule,
    pub post_layout: vk::PipelineLayout,
    pub post_pipeline: vk::Pipeline,
    pub terrain_gpu: TerrainGpu,
    pub settings: RenderSettings,
    pub show_motion: bool,
    /// The far shadow square's layers (`shadow::Far`): a game asks `far_shadow` each frame
    /// before writing its look block.
    pub far_shadow: std::cell::RefCell<shadow::Far>,
}

/// The images the post pass draws into: the window's swapchain images, or the offscreen one.
fn target_views(presenter: Option<&Presenter>, offscreen: Option<vk::ImageView>) -> Vec<vk::ImageView> {
    match (presenter, offscreen) {
        (Some(p), _) => p.views.clone(),
        (None, Some(v)) => vec![v],
        (None, None) => Vec::new(),
    }
}

/// What the scene camera sees for the upscalers that keep a history (FSR 3, GSR 2).
#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    /// This frame's sub-pixel jitter (pixels of the scene image).
    pub jitter: (f32, f32),
    /// The vertical field of view, radians.
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    /// Seconds since the last frame.
    pub dt: f32,
    /// The camera did not move since the last frame.
    pub same: bool,
}

/// One frame for [`Renderer::record`]: the scene and its draws, what the game shows of them
/// and how, the cameras, and the 2D layers over it all.
pub struct Frame<'a> {
    /// The target image: the window's acquired image, or 0 offscreen.
    pub image: usize,
    pub width: u32,
    pub height: u32,
    pub scene: &'a scene_gpu::SceneGpu,
    /// Set 1 for the field drawn (`SceneGpu::instance_set`).
    pub instance_set: vk::DescriptorSet,
    pub draws: &'a [Draw],
    pub instances: &'a [Instance],
    /// The texture sets, by texture number; `white` the plain white one, `outline_tex` the
    /// characters' outline texture.
    pub sets: &'a [vk::DescriptorSet],
    pub white: usize,
    pub outline_tex: Option<usize>,
    /// The field drawn: its terrain, and of the draws those of no field or of this one.
    pub current_field: u16,
    /// Instances not drawn this frame.
    pub hidden: &'a [bool],
    /// Whether the game shows a draw this frame (besides `hidden` and the field).
    pub shown: &'a dyn Fn(&Draw) -> bool,
    /// The game's material bits for a draw (`model.frag`'s flags past the renderer's own:
    /// 16 a solid piece of the distant view, 32 unlit, bits 8 to 11 the sway group, 128 a Ryza
    /// character's toon ramp with its row in bits 16 to 23) and its tint.
    pub material: &'a dyn Fn(&Draw) -> (u32, u32),
    pub draw_vp: Mat4,
    pub view_planes: [glam::Vec4; 6],
    pub shadow_vp: Mat4,
    pub alpha_cutoff: f32,
    /// The glow: its bright pass's threshold and its rate (0: none).
    pub glow: (f32, f32),
    /// The finished frame's saturation scale per channel (0: none).
    pub saturation: [f32; 3],
    /// The finished frame's tone curve white point (extended Reinhard; 0: none).
    pub tone_white: f32,
    pub camera: Camera,
    /// Copy this frame's scene into the frozen image (a cross-fade starts), and how much of
    /// the frozen image the post pass mixes in.
    pub freeze_now: bool,
    pub frozen_weight: f32,
    /// The screen's colour overlay (a fade).
    pub overlay: [f32; 4],
    pub layers: &'a [&'a text::Layer],
    /// Offscreen, the buffer this frame is copied into (a frame to keep).
    pub readback: Option<vk::Buffer>,
    /// Draw the sky plane (its look slots from `SKY_SLOT`) behind the distant view.
    pub sky_plane: bool,
}

impl Renderer {
    /// This frame's far shadow square (`shadow::Far::plan`): `want` where the game would have
    /// it (in steps: a moved square is drawn over `FAR_FRAMES` frames); gives the
    /// view-projection and layer for the look block's far slots, None while there is none.
    pub fn far_shadow(&self, want: Option<Mat4>) -> Option<(Mat4, usize)> {
        self.far_shadow.borrow_mut().plan(want)
    }

    /// The far square is drawn again from scratch (the scene's casters changed).
    pub fn forget_far_shadow(&self) {
        *self.far_shadow.borrow_mut() = shadow::Far { cleared: true, ..Default::default() };
    }

    /// A renderer for `settings` (resolved: `RenderSettings::resolved`), drawing into the
    /// window `presenter` shows, or offscreen at `width` × `height`; `timings` times the GPU's
    /// stages, `show_motion` shows the motion vectors in place of the frame.
    pub fn new(gpu: &Gpu, settings: RenderSettings, presenter: Option<&Presenter>, width: u32, height: u32, timings: bool, show_motion: bool) -> Result<Renderer> {
        let d = &gpu.device;
        let color_format = presenter.map_or(COLOR_FORMAT, |p| p.format);
        let pool = unsafe {
            d.create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(gpu.queue_family), None)?
        };

        let anisotropy = (settings.anisotropy as f32).min(gpu.max_anisotropy).max(1.0);
        let sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::REPEAT)
                    .max_lod(16.0)
                    .anisotropy_enable(anisotropy > 1.0)
                    .max_anisotropy(anisotropy),
                None,
            )?
        };

        // Descriptors: one combined image sampler per texture.
        let binding = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let set_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)? };
        let storage = |b: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
        };
        // And the field's character lighting (binding 4, for the fragment shader).
        // The lighting block, one slot per frame in flight.
        let look_ring = ring::Ring::new(&gpu, LOOK_SIZE, vk::BufferUsageFlags::UNIFORM_BUFFER)?;
        let inst_binding = [
            storage(0),
            storage(1),
            storage(2),
            storage(3),
            vk::DescriptorSetLayoutBinding::default()
                .binding(4)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            // The sun's shadow map (binding 5).
            vk::DescriptorSetLayoutBinding::default()
                .binding(5)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // The field's light texture, the drifting cloud shadows (binding 6).
            vk::DescriptorSetLayoutBinding::default()
                .binding(6)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let instance_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&inst_binding), None)? };
        // Each field's ground textures: a set of 2 blend maps + 6 layers.
        let terrain_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(TERRAIN_MAPS as u32)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(TERRAIN_LAYERS as u32)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let terrain_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&terrain_bindings), None)? };
        let shadow_target = shadow::ShadowTarget::new(&gpu, settings.shadows)?;
        let shared = scene_gpu::Shared {
            set_layout,
            sampler,
            instance_layout,
            look: std::array::from_fn(|slot| look_ring.info(slot)),
            shadow_sampler: shadow_target.sampler,
            shadow_view: shadow_target.view,
            terrain_layout,
        };

        // Offscreen target.
        let extent = vk::Extent3D { width, height, depth: 1 };
        let target_info = |format, usage| {
            vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(extent)
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(usage)
        };
        // The offscreen colour image (none with a window: the swapchain's images are the targets).
        let offscreen = if presenter.is_none() {
            Some(gpu.image(&target_info(COLOR_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC))?)
        } else {
            None
        };
        let view_of = |image, format, aspect| unsafe {
            d.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(1).layer_count(1)),
                None,
            )
        };
        let color_view = offscreen.map(|(color, _)| view_of(color, COLOR_FORMAT, vk::ImageAspectFlags::COLOR)).transpose()?;

        // The scene's samples: the most the device allows for colour and depth, up to `--msaa`.
        let samples = {
            let limits = unsafe { gpu.instance.get_physical_device_properties(gpu.physical) }.limits;
            let allowed = limits.framebuffer_color_sample_counts & limits.framebuffer_depth_sample_counts;
            [8u32, 4, 2, 1]
                .into_iter()
                .map(|n| (n, vk::SampleCountFlags::from_raw(n)))
                .find(|&(n, f)| n <= if settings.motion { 1 } else { settings.msaa.max(1) } && allowed.contains(f))
                .map_or(vk::SampleCountFlags::TYPE_1, |(_, f)| f)
        };
        let multisampled = samples != vk::SampleCountFlags::TYPE_1;
        // The scene pass: its colour ends ready to be sampled by the post pass. Multisampled, it
        // draws into a transient multisampled colour and depth and resolves into the scene image
        // at the end of the pass (in tile memory on the handhelds).
        let single_attachments = [
            vk::AttachmentDescription::default()
                .format(settings.scene_format())
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            vk::AttachmentDescription::default()
                .format(DEPTH_FORMAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                // Kept for GSR 2 to read, else never stored.
                .store_op(if settings.keeps_depth() { vk::AttachmentStoreOp::STORE } else { vk::AttachmentStoreOp::DONT_CARE })
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(if settings.keeps_depth() { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL } else { vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL }),
        ];
        let multi_attachments = [
            vk::AttachmentDescription::default()
                .format(settings.scene_format())
                .samples(samples)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
            single_attachments[1].samples(samples),
            vk::AttachmentDescription::default()
                .format(settings.scene_format())
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        ];
        // With motion vectors (single-sampled only): a second colour target after depth.
        let motion_attachments = [
            single_attachments[0],
            single_attachments[1],
            vk::AttachmentDescription::default()
                .format(MOTION_FORMAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        ];
        let attachments: &[vk::AttachmentDescription] = if multisampled {
            &multi_attachments
        } else if settings.motion {
            &motion_attachments
        } else {
            &single_attachments
        };
        let color_refs = [
            vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL },
            vk::AttachmentReference { attachment: 2, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL },
        ];
        let color_ref = &color_refs[..if settings.motion && !multisampled { 2 } else { 1 }];
        let resolve_ref = [vk::AttachmentReference { attachment: 2, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let depth_ref = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
        let mut subpass_desc = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(color_ref)
            .depth_stencil_attachment(&depth_ref);
        if multisampled {
            subpass_desc = subpass_desc.resolve_attachments(&resolve_ref);
        }
        let subpass = [subpass_desc];
        // The post pass reads what the scene pass wrote.
        let scene_done = [vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)];
        let render_pass = unsafe {
            d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(attachments).subpasses(&subpass).dependencies(&scene_done), None)?
        };
        // The post pass: into the window's image or the offscreen one, no depth.
        let post_attachment = [vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(if presenter.is_some() { vk::ImageLayout::PRESENT_SRC_KHR } else { vk::ImageLayout::TRANSFER_SRC_OPTIMAL })];
        let post_ref = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
        let post_subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&post_ref)];
        // In: the image's move to COLOR_ATTACHMENT waits at the colour output stage, the one the
        // swapchain image's acquire semaphore holds. Out: offscreen, the move to TRANSFER_SRC is
        // done before the readback copies (in a window, the present waits on its own semaphore).
        let post_dependencies = [
            vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
            vk::SubpassDependency::default()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(if presenter.is_some() { vk::PipelineStageFlags::BOTTOM_OF_PIPE } else { vk::PipelineStageFlags::TRANSFER })
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(if presenter.is_some() { vk::AccessFlags::empty() } else { vk::AccessFlags::TRANSFER_READ }),
        ];
        let post_pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&post_attachment).subpasses(&post_subpass).dependencies(&post_dependencies), None)? };
        // The post pass's one texture: the scene.
        let post_binding = [0, 1, 2].map(|b| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        });
        let post_set_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&post_binding), None)? };
        // Two sets: GSR 2 swaps its output each frame, and the post pass reads the one just written.
        let post_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 6 }];
        let post_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(2).pool_sizes(&post_sizes), None)? };
        let post_layouts = [post_set_layout, post_set_layout];
        let post_sets: [vk::DescriptorSet; 2] = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(post_pool).set_layouts(&post_layouts))? }
            .try_into()
            .unwrap();
        let post_sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?
        };
        let glow_pipelines = glow::Pipelines::new(&gpu)?;
        let upscale_mode = if settings.upscaler == settings::Upscaler::Gsr1 && settings.upscales() { upscale::Mode::Gsr1 } else { upscale::Mode::Fsr1 };
        let upscale_pipelines = if settings.upscales() || settings.sharpen_only() { Some(upscale::Pipelines::new(&gpu, upscale_mode, settings.scene_format())?) } else { None };
        let taa_pipelines = if settings.taa { Some(taa::Pipelines::new(&gpu, settings.scene_format())?) } else { None };
        let gsr2_pipelines = if settings.gsr2 { Some(gsr2::Pipelines::new(&gpu)?) } else { None };
        let fsr3_pipelines = if settings.fsr3 { Some(fsr3::Pipelines::new(&gpu)?) } else { None };
        let timer = if timings { timing::Timer::new(&gpu)? } else { None };
        let targets = Targets::new(&gpu, render_pass, post_pass, &target_views(presenter, color_view), width, height, settings.scene_size(width, height), post_sets, post_sampler, &glow_pipelines, upscale_pipelines.as_ref(), taa_pipelines.as_ref(), gsr2_pipelines.as_ref(), fsr3_pipelines.as_ref(), samples, settings.motion, show_motion, settings.scene_format())?;

        // Pipeline.
        let module = |code: &[u8]| -> Result<vk::ShaderModule> {
            let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
            Ok(unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)? })
        };
        let (vs, fs) = (module(VERT)?, module(FRAG)?);
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
        ];
        let vbind = [vk::VertexInputBindingDescription { binding: 0, stride: size_of::<Vertex>() as u32, input_rate: vk::VertexInputRate::VERTEX }];
        let vattr = [
            vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 0 },
            vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 12 },
            vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: 24 },
            vk::VertexInputAttributeDescription { location: 3, binding: 0, format: vk::Format::R16G16B16A16_UINT, offset: 32 },
            vk::VertexInputAttributeDescription { location: 4, binding: 0, format: vk::Format::R8G8B8A8_UNORM, offset: 40 },
            vk::VertexInputAttributeDescription { location: 5, binding: 0, format: vk::Format::R32G32_UINT, offset: 44 },
            vk::VertexInputAttributeDescription { location: 6, binding: 0, format: vk::Format::R8G8B8A8_UNORM, offset: 52 },
            vk::VertexInputAttributeDescription { location: 7, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 56 },
        ];
        let vinput = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbind).vertex_attribute_descriptions(&vattr);
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        // Viewport and scissor are set per frame (the window can change size).
        let vstate = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            // Back faces are culled, as the game does: rooms are one-sided boxes the event camera
            // often looks into from outside, through walls and a ceiling that face away from it.
            // The models' front faces wind clockwise once x is mirrored into render space, and
            // counter-clockwise again once the projection mirrors it back on screen.
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let scene_msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);
        let depth_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS);
        // The motion target, when there is one, takes the vectors unblended.
        let motion_blend = vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G);
        let colour_targets = color_ref.len();
        if colour_targets > 1 && unsafe { gpu.instance.get_physical_device_features(gpu.physical) }.independent_blend == 0 {
            return Err("motion vectors need the device's independent blending".into());
        }
        let blend_att = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA), motion_blend];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att[..colour_targets]);
        let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, offset: 0, size: PUSH_SIZE }];
        // Set 0 the colour, 1 the instances (and the look), 2 the mask.
        let set_layouts = [set_layout, instance_layout, set_layout];
        let pipeline_layout = unsafe {
            d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push), None)?
        };
        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vinput)
            .input_assembly_state(&assembly)
            .viewport_state(&vstate)
            .rasterization_state(&raster)
            .multisample_state(&scene_msaa)
            .depth_stencil_state(&depth_state)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass);
        let pipeline = unsafe {
            d.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).map_err(|(_, e)| e)?[0]
        };

        // The distant view: drawn after the field's opaque geometry with a viewport whose depth range
        // is 1 to 1 (its clipping unchanged, every fragment on the far plane), where the depth is
        // still the cleared 1.0: only the pixels nothing covers are shaded. No depth write, so its
        // layers all pass and blend in their order.
        let no_depth = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
        // Alpha-blended: its clouds are soft-edged layers over the dome.
        let sky_blend_att = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA), motion_blend];
        let sky_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&sky_blend_att[..colour_targets]);
        let sky_info = pipeline_info.depth_stencil_state(&no_depth).color_blend_state(&sky_blend);
        let sky_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[sky_info], None).map_err(|(_, e)| e)?[0] };
        // The sky plane: a screen-covering triangle (no vertices) on the far plane where nothing
        // covers it, like the distant view, unblended (the clouds blend over it after).
        let (sky_vs, sky_fs) = (module(SKY_VERT)?, module(SKY_FRAG)?);
        let sky_stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(sky_vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(sky_fs).name(c"main"),
        ];
        let sky_input = vk::PipelineVertexInputStateCreateInfo::default();
        let sky_raster = raster.cull_mode(vk::CullModeFlags::NONE);
        let sky_plane_info = pipeline_info.stages(&sky_stages).vertex_input_state(&sky_input).rasterization_state(&sky_raster).depth_stencil_state(&no_depth).color_blend_state(&blend);
        let sky_plane_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[sky_plane_info], None).map_err(|(_, e)| e)?[0] };

        // The characters' outline pass: the same shaders in outline mode, front faces culled (an
        // inverted hull: only the pushed-out back faces show, as a rim).
        let outline_raster = raster.cull_mode(vk::CullModeFlags::FRONT);
        let outline_info = pipeline_info.rasterization_state(&outline_raster);
        let outline_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[outline_info], None).map_err(|(_, e)| e)?[0] };

        // The sun's shadow map pass: the same vertex shader from the sun, depth only (the alpha
        // test kept), both faces, a little depth bias against acne.
        let shadow_fs = module(SHADOW_FRAG)?;
        let shadow_stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(shadow_fs).name(c"main"),
        ];
        let shadow_raster = raster.cull_mode(vk::CullModeFlags::NONE).depth_bias_enable(true).depth_bias_constant_factor(1.5).depth_bias_slope_factor(2.0);
        let shadow_blend = vk::PipelineColorBlendStateCreateInfo::default();
        let shadow_info = pipeline_info
            .stages(&shadow_stages)
            .rasterization_state(&shadow_raster)
            .color_blend_state(&shadow_blend)
            .multisample_state(&msaa)
            .render_pass(shadow_target.pass);
        let shadow_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[shadow_info], None).map_err(|(_, e)| e)?[0] };

        // The post pass's pipeline: one triangle over the screen, no vertex input or depth.
        let (post_vs, post_fs) = (module(POST_VERT)?, module(POST_FRAG)?);
        let post_stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(post_vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(post_fs).name(c"main"),
        ];
        let post_push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 48 }];
        let post_layout = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&post_layouts).push_constant_ranges(&post_push), None)? };
        let no_input = vk::PipelineVertexInputStateCreateInfo::default();
        let post_depth = vk::PipelineDepthStencilStateCreateInfo::default();
        let post_raster = raster.cull_mode(vk::CullModeFlags::NONE);
        // The post pass has the one colour target.
        let post_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att[..1]);
        let post_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&post_stages)
            .vertex_input_state(&no_input)
            .input_assembly_state(&assembly)
            .viewport_state(&vstate)
            .rasterization_state(&post_raster)
            .multisample_state(&msaa)
            .depth_stencil_state(&post_depth)
            .color_blend_state(&post_blend)
            .dynamic_state(&dynamic_state)
            .layout(post_layout)
            .render_pass(post_pass);
        let post_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[post_info], None).map_err(|(_, e)| e)?[0] };

        // Terrain: one pipeline with the terrain fragment shader, over each field's ground set
        // (2 blend maps + 6 layers, `SceneGpu`).
        let terrain_gpu = {
            let tpl_layouts = [terrain_layout, instance_layout];
            let tpl = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&tpl_layouts).push_constant_ranges(&push), None)? };
            let tfs = module(TERRAIN_FRAG)?;
            let tstages = [stages[0], vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(tfs).name(c"main")];
            // The heightmap is wound the other way from the models and seen from above only: no
            // culling.
            let traster = raster.cull_mode(vk::CullModeFlags::NONE);
            let info = pipeline_info.stages(&tstages).layout(tpl).rasterization_state(&traster);
            let tpipe = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0] };
            TerrainGpu { pipeline_layout: tpl, shader: tfs, pipeline: tpipe }
        };


        Ok(Renderer { pool, sampler, set_layout, look_ring, instance_layout, terrain_layout, shadow_target, shared, extent, offscreen, color_view, samples, render_pass, post_pass, post_set_layout, post_pool, post_sets, post_sampler, glow_pipelines, upscale_pipelines, taa_pipelines, gsr2_pipelines, fsr3_pipelines, timer, targets, vs, fs, pipeline_layout, pipeline, sky_pipeline, sky_plane_pipeline, sky_vs, sky_fs, outline_pipeline, shadow_fs, shadow_pipeline, post_vs, post_fs, post_layout, post_pipeline, terrain_gpu, settings, show_motion, far_shadow: Default::default() })
    }

    /// Records a frame into `cb`: the shadow map, the scene, glow and the anti-aliasing or
    /// upscaling the settings ask for, then the post pass and `frame.layers` over it (and
    /// offscreen, the copy into `frame.readback`). Returns how many draws went into the scene
    /// and into the shadow map.
    pub fn record(&mut self, gpu: &Gpu, cb: vk::CommandBuffer, frame: &Frame) -> [u32; 2] {
        let frozen_ready = self.targets.frozen_ready;
        self.targets.frozen_ready = true;
        let d = &gpu.device;
        let Frame { image, width, height, scene, instance_set, draws, instances, sets, white, outline_tex, current_field, hidden, shown, material, draw_vp, view_planes: view_planes_now, shadow_vp, alpha_cutoff, glow, saturation, tone_white, ref camera, freeze_now, frozen_weight, overlay, layers, readback, sky_plane } = *frame;
        let Renderer { ref timer, ref targets, ref shadow_target, shadow_pipeline, pipeline_layout, render_pass, pipeline, sky_pipeline, sky_plane_pipeline, ref terrain_gpu, outline_pipeline, ref glow_pipelines, ref taa_pipelines, ref fsr3_pipelines, ref gsr2_pipelines, ref upscale_pipelines, post_pass, post_pipeline, post_layout, show_motion, offscreen, extent, .. } = *self;
        let (vbuf, ibuf) = (scene.vbuf, scene.ibuf);
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width, height } };
        let (scene_width, scene_height) = targets.scene_size;
        let scene_area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: scene_width, height: scene_height } };
        let mut drawn = [0u32; 2];
        unsafe {
            // The frame before may still be drawing (frames in flight): its passes finish, their
            // writes made visible, before this frame's begin on the same targets.
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::default().src_access_mask(vk::AccessFlags::MEMORY_WRITE).dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)],
                &[],
                &[],
            );
            if let Some(t) = &timer {
                t.begin(d, cb);
            }
            // The frozen frame: copied from the last frame's scene when a cross-fade starts; given
            // its layout once otherwise.
            if freeze_now {
                barrier(d, cb, targets.scene, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
                barrier(d, cb, targets.frozen, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
                let layers = vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1);
                let region = vk::ImageCopy::default().src_subresource(layers).dst_subresource(layers).extent(vk::Extent3D { width: scene_width, height: scene_height, depth: 1 });
                d.cmd_copy_image(cb, targets.scene, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, targets.frozen, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
                barrier(d, cb, targets.frozen, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            } else if !frozen_ready {
                barrier(d, cb, targets.frozen, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            }
            // The sun's shadow map: every caster but the distant view, from the sun, into the
            // near layer; the far layers cleared once, and a slice of a moved far square's
            // casters into its layer (`shadow::Far`).
            let mut far = self.far_shadow.borrow_mut();
            let mut jobs: Vec<(usize, Option<Mat4>, bool)> = vec![(0, Some(shadow_vp), false)];
            if !far.cleared {
                far.cleared = true;
                jobs.extend((1..shadow::CASCADES).map(|l| (l, None, false)));
            }
            if let Some(b) = &far.build {
                jobs.push((b.layer, Some(b.vp), b.next > 0));
            }
            for (layer, cascade, load) in jobs {
                let clear = [vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }];
                let full = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: shadow_target.size, height: shadow_target.size } };
                let pass = if load { shadow_target.pass_load } else { shadow_target.pass };
                d.cmd_begin_render_pass(
                    cb,
                    &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(shadow_target.framebuffers[layer]).render_area(full).clear_values(&clear),
                    vk::SubpassContents::INLINE,
                );
                let Some(shadow_vp) = cascade else {
                    d.cmd_end_render_pass(cb);
                    continue;
                };
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: shadow_target.size as f32, height: shadow_target.size as f32, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[full]);
                d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
                d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, shadow_pipeline);
                let mut push = [0f32; 32];
                // Word 31: the lighting material (Draw::light); full for what pushes no draw's own (the terrain).
                push[31] = f32::from_bits(FULL_LIGHT);
                push[..16].copy_from_slice(&shadow_vp.to_cols_array());
                push[16] = alpha_cutoff;
                let stages_pc = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize));
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                let shadow_planes = view_planes(&shadow_vp);
                // In a far layer, casters whose box is under four of its texels across (the
                // map's side over its size) cast nothing seen: left out.
                let least = if layer > 0 { 4.0 * 2.0 / (shadow_vp.x_axis.x.hypot(shadow_vp.y_axis.x).hypot(shadow_vp.z_axis.x) * shadow_target.size as f32) } else { 0.0 };
                let big = |d: &Draw| least == 0.0 || d.bounds.is_none_or(|b| b.extent(&instances[d.instance as usize].model) >= least);
                let casts = |d: &Draw| big(d) && d.bounds.is_none_or(|b| b.seen(&instances[d.instance as usize].model, &shadow_planes)) && d.casts && !d.sky && !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && shown(d);
                // A far slice: from the build's next draw, about `budget` indices (a
                // FAR_FRAMES-th of its casters' when spread, all of them otherwise).
                let (start, budget) = match (layer, &mut far.build) {
                    (0, _) | (_, None) => (0, u64::MAX),
                    (_, Some(b)) => {
                        if b.budget.is_none() {
                            let total: u64 = draws.iter().filter(|d| casts(d)).map(|d| u64::from(d.index_count)).sum();
                            b.budget = Some(if b.spread { total.div_ceil(shadow::FAR_FRAMES).max(1) } else { u64::MAX });
                        }
                        (b.next, b.budget.unwrap_or(u64::MAX))
                    }
                };
                let (mut spent, mut stop) = (0u64, draws.len());
                for (k, draw) in draws.iter().enumerate().skip(start) {
                    if spent >= budget {
                        stop = k;
                        break;
                    }
                    if !casts(draw) {
                        continue;
                    }
                    spent += u64::from(draw.index_count);
                    // xEF_ field models (window light: TOWN_03's obj-xEF_windows_tw03) are not lit, flag
                    // 32: their texture on their vertex colour, as the distant view's clouds.
                    let flags = u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1;
                    let mut per_draw = [0u8; 8];
                    per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                    per_draw[4..].copy_from_slice(&flags.to_le_bytes());
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                    d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                    drawn[1] += 1;
                }
                d.cmd_end_render_pass(cb);
                if layer > 0 && let Some(b) = far.build.take() {
                    if stop >= draws.len() {
                        far.shown = Some((b.vp, b.layer));
                    } else {
                        far.build = Some(shadow::FarBuild { next: stop, ..b });
                    }
                }
            }
            drop(far);
            if let Some(t) = &timer {
                t.mark(d, cb, 1);
            }
            let clear = [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.12, 0.12, 0.14, 1.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
                // The motion target's (no movement), when there is one.
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
            ];
            d.cmd_begin_render_pass(
                cb,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(render_pass)
                    .framebuffer(targets.scene_framebuffer)
                    .render_area(scene_area)
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: scene_width as f32, height: scene_height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[scene_area]);
            d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
            d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
            let stages_pc = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            // view_proj, params, the terrain's scales; the instance number goes in per draw.
            let mut push = [0f32; 32];
                // Word 31: the lighting material (Draw::light); full for what pushes no draw's own (the terrain).
                push[31] = f32::from_bits(FULL_LIGHT);
            push[..16].copy_from_slice(&draw_vp.to_cols_array());
            push[16] = alpha_cutoff;
            push[18] = width as f32 / height.max(1) as f32;
            push[19] = HDR_RANGE_INV;
            // The terrain's push constants: the shared ones with its field size and layer scales.
            let mut terrain_push = push;
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            let pc = std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize);
            d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
            // The terrain and the field's models first, then the current field's distant view where
            // they leave the far plane bare (as if drawn first and covered, without shading the
            // covered part).
            for (pass_sky, pipe) in [(false, pipeline), (true, sky_pipeline)] {
            if !pass_sky {
                {
                    let tg = &terrain_gpu;
                    for g in scene.grounds.iter().filter(|g| g.field == current_field) {
                        terrain_push[17] = g.cell;
                        terrain_push[20] = g.extent[0];
                        terrain_push[21] = g.extent[1];
                        // Flags (word 29): 64, the terrain (model.vert: no point lights).
                        terrain_push[29] = f32::from_bits(64);
                        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, tg.pipeline);
                        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, tg.pipeline_layout, 0, &[g.set, instance_set], &[]);
                        // A patch at a time, its bake list in the push constants' floats 22 to 27
                        // (their bits: the count, then two entries a word).
                        for patch in g.patches.iter().filter(|p| p.index_count > 0) {
                            terrain_push[22..28].copy_from_slice(&scene_gpu::bake_words(&patch.bake).map(f32::from_bits));
                            d.cmd_push_constants(cb, tg.pipeline_layout, stages_pc, 0, std::slice::from_raw_parts(terrain_push.as_ptr() as *const u8, PUSH_SIZE as usize));
                            d.cmd_draw_indexed(cb, patch.index_count, 1, patch.first_index, 0, 0);
                        }
                    }
                }
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
            }
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipe);
            // The distant view's fragments on the far plane (see `sky_pipeline`).
            let depth_range = if pass_sky { (1.0, 1.0) } else { (0.0, 1.0) };
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: scene_width as f32, height: scene_height as f32, min_depth: depth_range.0, max_depth: depth_range.1 }]);
            if pass_sky && sky_plane {
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, sky_plane_pipeline);
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipe);
            }
            for draw in draws.iter().filter(|d| d.bounds.is_none_or(|b| b.seen(&instances[d.instance as usize].model, &view_planes_now))).filter(|d| d.sky == pass_sky && !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && shown(d)) {
                // The instance, and whether it is shaded as a character (not part of a field), as
                // an eye and as the distant view; the game's own material bits and tint.
                let (bits, tint) = material(draw);
                let flags = u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1 | u32::from(draw.sky) << 3 | bits;
                let mut per_draw = [0u8; 16];
                per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                per_draw[4..8].copy_from_slice(&flags.to_le_bytes());
                per_draw[8..12].copy_from_slice(&tint.to_le_bytes());
                per_draw[12..].copy_from_slice(&draw.light.to_le_bytes());
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[draw.mask.unwrap_or(white)]], &[]);
                d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                drawn[0] += 1;
            }
            }
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: scene_width as f32, height: scene_height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            // The outlines: the outlined materials again, pushed out, with the outline texture.
            if let Some(outline_tex) = outline_tex {
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, outline_pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[outline_tex]], &[]);
                for draw in draws.iter().filter(|d| !hidden[d.instance as usize] && d.field.is_none() && !d.eye) {
                    let Some(colour) = draw.outline else { continue };
                    let mut per_draw = [0u8; 12];
                    per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                    per_draw[4..8].copy_from_slice(&(1u32 | 4).to_le_bytes());
                    per_draw[8..].copy_from_slice(&colour.to_le_bytes());
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                    d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                }
            }
            d.cmd_end_render_pass(cb);
            if let Some(t) = &timer {
                t.mark(d, cb, 2);
            }
            // The glow from the scene's bright parts: its bright pass's threshold, and its rate in
            // the post pass.
            let (threshold, rate) = glow;
            if let Some(g) = &targets.glow {
                g.record(d, &glow_pipelines, cb, threshold);
            }
            if let (Some(t), Some(p)) = (&targets.taa, &taa_pipelines) {
                let fresh = targets.history_fresh.replace(false);
                t.record(d, p, cb, fresh, fresh);
            }
            if let (Some(f), Some(p)) = (targets.fsr3.borrow_mut().as_mut(), &fsr3_pipelines) {
                let fresh = targets.history_fresh.replace(false);
                let frame = fsr3::Frame {
                    render: targets.scene_size,
                    upscale: (width, height),
                    jitter: camera.jitter,
                    fov: camera.fov,
                    near: camera.near,
                    far: camera.far,
                    dt: camera.dt,
                    reset: fresh,
                };
                if let Err(e) = f.record(&gpu, p, cb, &frame) {
                    eprintln!("FSR 3: {e}");
                }
            }
            if let (Some(g), Some(p)) = (&targets.gsr2, &gsr2_pipelines) {
                let fresh = targets.history_fresh.replace(false);
                let frame = gsr2::Frame { jitter: camera.jitter, fov: camera.fov, reset: fresh, same_camera: camera.same };
                g.record(d, p, cb, frame, fresh);
            }
            if let (Some(u), Some(p)) = (&targets.upscale, &upscale_pipelines) {
                u.record(d, p, cb);
            }
            if let Some(t) = &timer {
                t.mark(d, cb, 3);
            }
            // The post pass: the scene back to its range with the glow, then the text over it.
            d.cmd_begin_render_pass(
                cb,
                &vk::RenderPassBeginInfo::default().render_pass(post_pass).framebuffer(targets.framebuffers[image]).render_area(area),
                vk::SubpassContents::INLINE,
            );
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, post_pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, post_layout, 0, &[targets.post_set()], &[]);
            let mut post_push = [0u8; 48];
            post_push[..4].copy_from_slice(&rate.to_le_bytes());
            post_push[4..8].copy_from_slice(&frozen_weight.to_le_bytes());
            post_push[8..12].copy_from_slice(&(if show_motion { 1.0f32 } else { 0.0 }).to_le_bytes());
            for (k, v) in saturation.iter().enumerate() {
                post_push[32 + 4 * k..36 + 4 * k].copy_from_slice(&v.to_le_bytes());
            }
            post_push[44..48].copy_from_slice(&tone_white.to_le_bytes());
            for (k, v) in overlay.iter().enumerate() {
                post_push[16 + 4 * k..20 + 4 * k].copy_from_slice(&v.to_le_bytes());
            }
            d.cmd_push_constants(cb, post_layout, vk::ShaderStageFlags::FRAGMENT, 0, &post_push);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: width as f32, height: height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_draw(cb, 3, 1, 0, 0);
            for layer in layers {
                layer.record(d, cb, width, height);
            }
            d.cmd_end_render_pass(cb);
            if let Some(t) = &timer {
                t.mark(d, cb, 4);
            }
            if let (Some((color, _)), Some(readback)) = (offscreen, readback) {
                let region = vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(extent);
                d.cmd_copy_image_to_buffer(cb, color, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, readback, &[region]);
            }
        }
        drawn
    }

    /// The targets again at a new window size (after the presenter's swapchain was rebuilt).
    pub fn rebuild_targets(&mut self, gpu: &Gpu, presenter: Option<&Presenter>, width: u32, height: u32) -> Result<()> {
        self.targets.destroy(gpu);
        let s = &self.settings;
        self.targets = Targets::new(gpu, self.render_pass, self.post_pass, &target_views(presenter, self.color_view), width, height, s.scene_size(width, height), self.post_sets, self.post_sampler, &self.glow_pipelines, self.upscale_pipelines.as_ref(), self.taa_pipelines.as_ref(), self.gsr2_pipelines.as_ref(), self.fsr3_pipelines.as_ref(), self.samples, s.motion, self.show_motion, s.scene_format())?;
        Ok(())
    }

    /// Everything it made, gone (the device idle first); the GPU timings reported.
    pub fn destroy(mut self, gpu: &Gpu) -> Result<()> {
        let d = &gpu.device;
        unsafe {
            d.device_wait_idle()?;
            d.destroy_pipeline(self.terrain_gpu.pipeline, None);
            d.destroy_pipeline_layout(self.terrain_gpu.pipeline_layout, None);
            d.destroy_shader_module(self.terrain_gpu.shader, None);
            d.destroy_descriptor_set_layout(self.terrain_layout, None);
            d.destroy_pipeline(self.post_pipeline, None);
            d.destroy_pipeline_layout(self.post_layout, None);
            d.destroy_shader_module(self.post_vs, None);
            d.destroy_shader_module(self.post_fs, None);
            d.destroy_descriptor_pool(self.post_pool, None);
            d.destroy_descriptor_set_layout(self.post_set_layout, None);
            d.destroy_sampler(self.post_sampler, None);
            d.destroy_render_pass(self.post_pass, None);
            d.destroy_pipeline(self.outline_pipeline, None);
            d.destroy_pipeline(self.shadow_pipeline, None);
            d.destroy_shader_module(self.shadow_fs, None);
            d.destroy_pipeline(self.sky_pipeline, None);
            d.destroy_pipeline(self.sky_plane_pipeline, None);
            d.destroy_shader_module(self.sky_vs, None);
            d.destroy_shader_module(self.sky_fs, None);
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            d.destroy_shader_module(self.vs, None);
            d.destroy_shader_module(self.fs, None);
            self.targets.destroy(gpu);
            self.glow_pipelines.destroy(gpu);
            if let Some(p) = self.upscale_pipelines {
                p.destroy(gpu);
            }
            if let Some(p) = self.taa_pipelines {
                p.destroy(gpu);
            }
            if let Some(p) = self.gsr2_pipelines {
                p.destroy(gpu);
            }
            if let Some(p) = self.fsr3_pipelines {
                p.destroy(gpu);
            }
            if let Some(t) = self.timer.take() {
                t.report();
                t.destroy(gpu);
            }
            d.destroy_render_pass(self.render_pass, None);
            if let Some(v) = self.color_view {
                d.destroy_image_view(v, None);
            }
            if let Some((color, cmem)) = self.offscreen {
                d.destroy_image(color, None);
                d.free_memory(cmem, None);
            }
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.sampler, None);
        }
        self.look_ring.destroy(gpu);
        self.shadow_target.destroy(gpu);
        unsafe {
            d.destroy_descriptor_set_layout(self.instance_layout, None);
            d.destroy_command_pool(self.pool, None);
        }
        Ok(())
    }
}
