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
    /// Ryza's sun flare: the sun's visibility and the wash's settings (`flare.rs`).
    pub sun: flare::Sun,
    pub shared: scene_gpu::Shared,
    black_cube: (vk::Image, vk::DeviceMemory, vk::ImageView),
    pub extent: vk::Extent3D,
    pub offscreen: Option<(vk::Image, vk::DeviceMemory)>,
    pub color_view: Option<vk::ImageView>,
    pub samples: vk::SampleCountFlags,
    pub render_pass: vk::RenderPass,
    /// The scene pass in two parts, for a frame with water (`ScenePart`).
    pub water_passes: [vk::RenderPass; 2],
    /// The water's view of the opaque scene (`Targets::refraction`): its set (the texture sets'
    /// layout), the pool of it and the sampler.
    pub refraction_set: vk::DescriptorSet,
    refraction_pool: vk::DescriptorPool,
    refraction_sampler: vk::Sampler,
    pub post_pass: vk::RenderPass,
    pub post_set_layout: vk::DescriptorSetLayout,
    pub post_pool: vk::DescriptorPool,
    pub post_sets: [vk::DescriptorSet; 2],
    pub post_sampler: vk::Sampler,
    /// The screen filter's texture (post binding 3; a clear 1 × 1 until a game sets one).
    pub filter: (vk::Image, vk::DeviceMemory, vk::ImageView),
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
    /// The scene pipeline with no face culling, for the draws a material shows from both
    /// sides (`Draw::two_sided`).
    pub pipeline_two_sided: vk::Pipeline,
    pub sky_pipeline: vk::Pipeline,
    /// The sky plane (sky.vert/frag): drawn first among the far plane's fragments when a
    /// frame asks (`Frame::sky_plane`).
    pub sky_plane_pipeline: vk::Pipeline,
    pub sky_vs: vk::ShaderModule,
    pub sky_fs: vk::ShaderModule,
    pub outline_pipeline: vk::Pipeline,
    /// The game's forward pass over a character (`model.frag`'s flag 1 << 25): the hull with
    /// front faces culled, blended by its alpha over the lit scene, depth tested, not written.
    pub forward_pipeline: vk::Pipeline,
    pub shadow_fs: vk::ShaderModule,
    pub shadow_pipeline: vk::Pipeline,
    /// The same writing a cascade's depth with the game's bias (`shadow::Bias`).
    pub shadow_cascade_pipeline: vk::Pipeline,
    pub shadow_cascade_fs: vk::ShaderModule,
    /// The ambient occlusion (`ao.rs`): its targets and passes, and the pipelines that draw the
    /// models' and the terrain's normals and depth into them.
    pub ao: ao::Occlusion,
    /// The screen-space reflections (`reflect.rs`), when the setting is on (and the scene is
    /// single-sampled): their passes, and the targets bound to the scene's.
    pub reflect: Option<reflect::Reflections>,
    /// The depth of field (`dof.rs`), when the setting is on (single-sampled, with the occlusion's
    /// depth): its passes, and the targets bound to the scene's and the post pass's.
    pub dof: Option<dof::DepthOfField>,
    ao_gbuf_pipeline: vk::Pipeline,
    ao_gbuf_two_sided_pipeline: vk::Pipeline,
    ao_gbuf_terrain_pipeline: vk::Pipeline,
    pub post_vs: vk::ShaderModule,
    pub post_fs: vk::ShaderModule,
    pub post_layout: vk::PipelineLayout,
    pub post_pipeline: vk::Pipeline,
    pub terrain_gpu: TerrainGpu,
    pub settings: RenderSettings,
    pub show_motion: bool,
    /// The static shadow layer (`shadow::Far`): a game asks `far_shadow` each frame before
    /// writing its look block.
    pub far_shadow: std::cell::RefCell<shadow::Far>,
    /// The sky clouds' strip (cloud_strip.*): its pipeline reads the cloud map as set 0 and the
    /// look block as set 1.
    pub cloud_set_layout: vk::DescriptorSetLayout,
    pub cloud_layout: vk::PipelineLayout,
    pub cloud_pipeline: vk::Pipeline,
    pub cloud_modules: [vk::ShaderModule; 2],
    /// The water: a ribbon grid from the look block's water slots, with the scene's layout.
    pub water_pipeline: vk::Pipeline,
    pub water_far_pipeline: vk::Pipeline,
    pub water_modules: [vk::ShaderModule; 2],
    /// The clouds' maps, made when a frame first asks for clouds.
    pub clouds: std::cell::RefCell<Option<clouds::Maps>>,
    /// A picture to fill the cloud map with (RGBA, `clouds::MAP_SIZE` square) in place of the
    /// passes: a parity tool (`BARREL_CLOUD_MAP`).
    pub fixed_cloud_map: Option<Vec<u8>>,
    /// The stage dumps (`BARREL_DUMP_STAGES`, `dump`), when asked for, and how far the next
    /// frame is recorded for them.
    pub dump: Option<dump::Dump>,
    pub upto: std::cell::Cell<dump::Stop>,
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
    /// Whether a shown draw is in sight by a test of the game's own, past the view's planes
    /// (something stands in front of it): asked for the scene, not for the shadows it casts.
    pub in_sight: &'a dyn Fn(&Draw) -> bool,
    /// The game's material bits for a draw (`model.frag`'s flags past the renderer's own:
    /// 16 a solid piece of the distant view, 32 unlit, bits 8 to 11 the sway group, 128 a Ryza
    /// character's toon ramp with its row in bits 16 to 23, 4096 a sky dome lit by the
    /// look's `DOME_LIGHT_SLOT`) and its tint.
    pub material: &'a dyn Fn(&Draw) -> (u32, u32),
    pub draw_vp: Mat4,
    pub view_planes: [glam::Vec4; 6],
    pub shadow_vp: Mat4,
    /// The next two cascades' view-projections (layers 1 and 2), drawn every frame; none for a
    /// game with the one near square.
    pub shadow_cascades: [Option<Mat4>; 2],
    /// How the three cascades' depth is written (`shadow::Bias`); none: the fixed bias of the
    /// near square (Escha).
    pub shadow_bias: Option<[shadow::Bias; 3]>,
    /// The characters' shadow map (`shadow::SUB_LAYER`): its view-projection and how the game
    /// writes its depth for each of the bias sets the draws name (`Draw::sub_bias`); none for a
    /// game without one. Its casters are left out of the cascades.
    pub shadow_sub: Option<(Mat4, [shadow::Bias; 2])>,
    /// Each layer's map in texels (width, height), from the layer's corner, where a game's maps
    /// are smaller than the layer or not square (Ryza's cascades); none: every layer whole.
    pub shadow_sides: Option<[[u32; 2]; shadow::CASCADES]>,
    pub alpha_cutoff: f32,
    /// The alpha test's reference for swaying draws (the plants), where a game holds another.
    pub sway_cutoff: f32,
    /// The glow: its bright pass's threshold and its rate (0: none).
    pub glow: (f32, f32),
    /// The exposure to meter the scene to (the Ryza games'; the scene was lit at `baked`, and the
    /// tone pass and the bloom take it to the metered one), none for the scene as lit.
    pub meter: Option<meter::Params>,
    /// The finished frame's saturation scale per channel (0: none).
    pub saturation: [f32; 3],
    /// The finished frame's tone curve white point (extended Reinhard; 0: none).
    pub tone_white: f32,
    /// The game's filmic curve instead, with `tone_white` as its white point, and this gamma
    /// after it (0: the Reinhard curve, if any).
    pub tone_gamma: f32,
    /// With the filmic curve: a scale per channel on the light before the curve (the Ryza
    /// games' `vColorScale`; 0: none), and after the gamma a saturation about the luma that
    /// grows as the colour darkens (`fSaturationScaleEx`; 1 or less: none).
    pub tone_scale: [f32; 3],
    pub tone_vivid: f32,
    /// The screen filter's strength (Escha's `overlayModifier`, `PostEffectTexFilter`: the
    /// texture set by [`Renderer::set_filter`] overlay-blended over the frame by its alpha
    /// times this; 0: none).
    pub filter: f32,
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
    /// Draw the sky's large clouds after the distant view (their look slots from `CLOUD_SLOT`),
    /// making the maps they read as asked.
    pub clouds: Option<clouds::CloudFrame>,
    /// The water's pieces (their look slots from `WATER_SLOT`), drawn after the field's opaque
    /// geometry; 0 for none.
    pub water_pieces: u32,
    /// Whether the game's frame has the screen-space ambient occlusion (`ao.rs`; Ryza's): it is
    /// made when the graphics setting `occlusion` is on too.
    pub occlusion: bool,
    /// Ryza's sun flare and paraffin wash with the point the sun's shadow is asked about; none
    /// for a game without it (Escha).
    pub sun_flare: Option<flare::Settings>,
}

/// Points the water's refraction set at the refraction image.
fn write_refraction_set(d: &ash::Device, set: vk::DescriptorSet, sampler: vk::Sampler, view: vk::ImageView) {
    let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    unsafe { d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]) };
}

/// Which part of the scene pass a render pass is: the whole of it, or the first part of a frame
/// with water (the opaque scene: its colour and depth kept, the colour left for a copy) and the
/// second (them loaded again, the water and the rest drawn over). The three are compatible: the
/// pipelines are made with the whole.
#[derive(Clone, Copy, PartialEq)]
enum ScenePart {
    Whole,
    First,
    Second,
}

/// The scene pass (see `ScenePart`): colour (multisampled with a resolve into the scene image, or
/// the scene image itself with a motion target after depth) and depth. With `reflect`
/// (single-sampled), the reflections' two per-pixel targets follow (their colour locations 2 and
/// 3; location 1 is the motion target, or unused): they are kept for the pass that reads them
/// after the scene, so the second part of a frame with water leaves them alone, and the first
/// part leaves the scene image readable for the reflections' march instead of for the copy.
fn scene_render_pass(d: &ash::Device, settings: &RenderSettings, samples: vk::SampleCountFlags, part: ScenePart, reflect: bool, normals: bool) -> Result<vk::RenderPass> {
    use vk::{AttachmentLoadOp as Load, AttachmentStoreOp as Store, ImageLayout as Layout};
    let multisampled = samples != vk::SampleCountFlags::TYPE_1;
    let single = vk::SampleCountFlags::TYPE_1;
    let (first, second) = (part == ScenePart::First, part == ScenePart::Second);
    let attachment = |format, samples, load, store, initial, last| {
        vk::AttachmentDescription::default().format(format).samples(samples).load_op(load).store_op(store).initial_layout(initial).final_layout(last)
    };
    // What each attachment starts from and ends in: the first part leaves the scene image for the
    // copy; the second finds it, and the kept images, ready to draw into.
    let load = if second { Load::LOAD } else { Load::CLEAR };
    let initial = |layout| if second { layout } else { Layout::UNDEFINED };
    let depth_read = settings.keeps_depth() || normals;
    let depth_last = if depth_read { Layout::SHADER_READ_ONLY_OPTIMAL } else { Layout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
    // Kept for GSR 2 or the half-size normal and depth target to read, else only across the two parts.
    let depth_store = if depth_read || first { Store::STORE } else { Store::DONT_CARE };
    let depth = attachment(DEPTH_FORMAT, samples, load, depth_store, initial(Layout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL), if first { Layout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL } else { depth_last });
    let scene_last = if first && !reflect { Layout::TRANSFER_SRC_OPTIMAL } else { Layout::SHADER_READ_ONLY_OPTIMAL };
    let mut attachments = vec![];
    if multisampled {
        // The multisampled colour, kept across the parts only.
        attachments.push(attachment(settings.scene_format(), samples, load, if first { Store::STORE } else { Store::DONT_CARE }, initial(Layout::COLOR_ATTACHMENT_OPTIMAL), Layout::COLOR_ATTACHMENT_OPTIMAL));
        attachments.push(depth);
        // The resolve target: the scene image, copied after the first part.
        attachments.push(attachment(settings.scene_format(), single, Load::DONT_CARE, Store::STORE, initial(Layout::COLOR_ATTACHMENT_OPTIMAL), scene_last));
    } else {
        attachments.push(attachment(settings.scene_format(), single, load, Store::STORE, initial(Layout::COLOR_ATTACHMENT_OPTIMAL), scene_last));
        attachments.push(depth);
        if settings.motion {
            // The motion vectors: a second colour target after depth.
            attachments.push(attachment(MOTION_FORMAT, single, load, Store::STORE, initial(Layout::COLOR_ATTACHMENT_OPTIMAL), if first { Layout::COLOR_ATTACHMENT_OPTIMAL } else { Layout::SHADER_READ_ONLY_OPTIMAL }));
        }
    }
    // Colour locations: the scene image, the motion vectors (or nothing), the reflections' two.
    let unused = vk::AttachmentReference { attachment: vk::ATTACHMENT_UNUSED, layout: Layout::UNDEFINED };
    let mut colour = vec![vk::AttachmentReference { attachment: 0, layout: Layout::COLOR_ATTACHMENT_OPTIMAL }];
    if settings.motion && !multisampled {
        colour.push(vk::AttachmentReference { attachment: 2, layout: Layout::COLOR_ATTACHMENT_OPTIMAL });
    }
    if normals && !multisampled {
        // Location 1, where the motion vectors would be (there are none with the normals).
        // Read after the scene pass (by the half-size target's pass), not carried into the water's part.
        let (normal_load, normal_store, normal_last) = if second { (Load::DONT_CARE, Store::DONT_CARE, Layout::COLOR_ATTACHMENT_OPTIMAL) } else { (Load::CLEAR, Store::STORE, Layout::SHADER_READ_ONLY_OPTIMAL) };
        colour.push(vk::AttachmentReference { attachment: attachments.len() as u32, layout: Layout::COLOR_ATTACHMENT_OPTIMAL });
        attachments.push(attachment(crate::ao::NORMAL_FORMAT, single, normal_load, normal_store, Layout::UNDEFINED, normal_last));
    }
    if reflect && !multisampled {
        if colour.len() == 1 {
            colour.push(unused);
        }
        // Read after the scene pass (by the application), not carried into the water's part.
        let (spec_load, spec_store, spec_initial, spec_last) = if second {
            (Load::DONT_CARE, Store::DONT_CARE, Layout::UNDEFINED, Layout::COLOR_ATTACHMENT_OPTIMAL)
        } else {
            (Load::CLEAR, Store::STORE, Layout::UNDEFINED, Layout::SHADER_READ_ONLY_OPTIMAL)
        };
        for _ in 0..2 {
            colour.push(vk::AttachmentReference { attachment: attachments.len() as u32, layout: Layout::COLOR_ATTACHMENT_OPTIMAL });
            attachments.push(attachment(crate::reflect::FORMAT, single, spec_load, spec_store, spec_initial, spec_last));
        }
    }
    let colour = &colour[..];
    let resolve = [vk::AttachmentReference { attachment: 2, layout: Layout::COLOR_ATTACHMENT_OPTIMAL }];
    let depth_ref = vk::AttachmentReference { attachment: 1, layout: Layout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
    let mut subpass = vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(colour).depth_stencil_attachment(&depth_ref);
    if multisampled {
        subpass = subpass.resolve_attachments(&resolve);
    }
    // What reads the scene after the pass: the post pass, or the copy of the first part.
    let done = [vk::SubpassDependency::default()
        .src_subpass(0)
        .dst_subpass(vk::SUBPASS_EXTERNAL)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
        .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TRANSFER)
        .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::TRANSFER_READ)];
    let subpasses = [subpass];
    Ok(unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses).dependencies(&done), None)? })
}

/// The draw trace's pass names of the shadow layers (the three cascades, the static map, the
/// characters' map), in layer order.
const SHADOW_TRACE_PASSES: [&str; shadow::CASCADES] = ["shadow0", "shadow1", "shadow2", "shadow-static", "shadow-sub"];

impl Renderer {
    /// This frame's static shadow layer (`shadow::Far::plan`): `want` where the game has it
    /// (drawn whole when it changes); gives the view-projection and layer for the look
    /// block's static slots, None while there is none.
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
            // The field's specular environment cube map (binding 7).
            vk::DescriptorSetLayoutBinding::default()
                .binding(7)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let instance_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&inst_binding), None)? };
        // Each field's ground textures: its blend maps, its layers' colour tiles and their normal maps.
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
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(TERRAIN_LAYERS as u32)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let terrain_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&terrain_bindings), None)? };
        let shadow_target = shadow::ShadowTarget::new(&gpu, settings.shadows)?;
        // A black cube map for the fields that have no specular one.
        let black = [0u8; 48];
        let black_cube = upload_cube(&gpu, pool, &barrel_formats::g1t::Texture { width: 1, height: 1, format: barrel_formats::g1t::Format::Rgba16f, format_code: 0, layers: 6, volume: false, levels: vec![&black[..]] })?;
        let shared = scene_gpu::Shared {
            set_layout,
            sampler,
            instance_layout,
            look: std::array::from_fn(|slot| look_ring.info(slot)),
            shadow_sampler: shadow_target.sampler,
            shadow_view: shadow_target.view,
            terrain_layout,
            black_cube: black_cube.2,
            pool,
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
        // The scene pass: its colour ends ready to be sampled by the post pass. Multisampled, it
        // draws into a multisampled colour and depth and resolves into the scene image at the end
        // of the pass. A frame with water draws it in two parts, the opaque scene first and the
        // rest after a copy of it (the water's refraction), which the same attachments carry on
        // through (`ScenePart`).
        // The reflections need the normals and depth pre-pass and single-sampled drawing.
        let reflect_on = settings.reflections && samples == vk::SampleCountFlags::TYPE_1;
        let dof_on = settings.depth_of_field && samples == vk::SampleCountFlags::TYPE_1;
        // Without the occlusion's own half-size pre-pass the reflections and the depth of field get
        // their normals and depth from the scene pass: it writes the world normal, keeps its depth,
        // and one full-screen pass builds the half-size target (`ao::Occlusion::derive`).
        let from_scene = !settings.occlusion && !settings.motion && (reflect_on || dof_on);
        let render_pass = scene_render_pass(d, &settings, samples, ScenePart::Whole, reflect_on, from_scene)?;
        let water_passes = [scene_render_pass(d, &settings, samples, ScenePart::First, reflect_on, from_scene)?, scene_render_pass(d, &settings, samples, ScenePart::Second, reflect_on, from_scene)?];
        // Colour locations: the scene, the motion vectors (or the world normal), the reflections' two.
        let colour_targets = if reflect_on { 4 } else if (settings.motion && samples == vk::SampleCountFlags::TYPE_1) || from_scene { 2 } else { 1 };
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
        let post_binding = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|b| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(if b == 5 { vk::DescriptorType::UNIFORM_BUFFER } else { vk::DescriptorType::COMBINED_IMAGE_SAMPLER })
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        });
        let post_set_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&post_binding), None)? };
        // Two sets: GSR 2 swaps its output each frame, and the post pass reads the one just written.
        let post_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 18 }, vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 2 }];
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
        let clear = [0u8; 4];
        let filter = upload_texture(&gpu, pool, &barrel_formats::g1t::Texture { width: 1, height: 1, format: barrel_formats::g1t::Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&clear[..]] }, false)?;
        write_filter(&gpu, &post_sets, post_sampler, filter.2);
        // The depth of field's three textures (bound for real by `dof.rs`): a placeholder until then.
        write_dof_placeholder(&gpu, &post_sets, post_sampler, filter.2);
        let sun = flare::Sun::new(&gpu, (shadow_target.view, shadow_target.sampler), post_sampler)?;
        write_wash(&gpu, &post_sets, post_sampler, &sun);
        let glow_pipelines = glow::Pipelines::new(&gpu, settings.hdr_scene)?;
        let upscale_mode = if settings.upscaler == settings::Upscaler::Gsr1 && settings.upscales() { upscale::Mode::Gsr1 } else { upscale::Mode::Fsr1 };
        let upscale_pipelines = if settings.upscales() || settings.sharpen_only() { Some(upscale::Pipelines::new(&gpu, upscale_mode, settings.scene_format())?) } else { None };
        let taa_pipelines = if settings.taa { Some(taa::Pipelines::new(&gpu, settings.scene_format())?) } else { None };
        let gsr2_pipelines = if settings.gsr2 { Some(gsr2::Pipelines::new(&gpu)?) } else { None };
        let fsr3_pipelines = if settings.fsr3 { Some(fsr3::Pipelines::new(&gpu)?) } else { None };
        let timer = if timings { timing::Timer::new(&gpu)? } else { None };
        // The water's refraction image's set: one texel sampler, clamped.
        let refraction_sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?
        };
        let refraction_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 1 }];
        let refraction_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&refraction_sizes), None)? };
        let refraction_set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(refraction_pool).set_layouts(&[set_layout]))?[0] };
        let targets = Targets::new(&gpu, render_pass, post_pass, &target_views(presenter, color_view), width, height, settings.scene_size(width, height), post_sets, post_sampler, &glow_pipelines, upscale_pipelines.as_ref(), taa_pipelines.as_ref(), gsr2_pipelines.as_ref(), fsr3_pipelines.as_ref(), samples, settings.motion, show_motion, settings.scene_format(), reflect_on, from_scene)?;

        // Pipeline.
        let module = |code: &[u8]| -> Result<vk::ShaderModule> {
            let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
            Ok(unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)? })
        };
        let (vs, fs) = (module(VERT)?, module(FRAG)?);
        // The models' and the terrain's programs write the world normal in the motion vectors'
        // attachment where the scene pass feeds the half-size target (`NORMAL_AS_MOTION`).
        let normal_entry = [vk::SpecializationMapEntry { constant_id: 1, offset: 0, size: 4 }];
        let normal_data = u32::from(from_scene).to_le_bytes();
        let normal_special = vk::SpecializationInfo::default().map_entries(&normal_entry).data(&normal_data);
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main").specialization_info(&normal_special),
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
            vk::VertexInputAttributeDescription { location: 8, binding: 0, format: vk::Format::A2B10G10R10_SNORM_PACK32, offset: 68 },
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
        if colour_targets > 1 && unsafe { gpu.instance.get_physical_device_features(gpu.physical) }.independent_blend == 0 {
            return Err("motion vectors need the device's independent blending".into());
        }
        // The reflections' two targets take what the model program writes (zero where it has
        // nothing); a program without those outputs leaves them as they are.
        let spec_written = vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA);
        let spec_kept = vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::empty());
        let opaque = vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA);
        // The second location: the motion vectors, or the world normal (two channels) that only
        // the models and the terrain write, the other programs leave as it is.
        let (model_slot, other_slot) = if from_scene { (motion_blend, spec_kept) } else { (motion_blend, motion_blend) };
        let blend_att = [opaque, model_slot, spec_written, spec_written];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att[..colour_targets]);
        // The terrain, the sky's plane and the water (their programs have no reflections' outputs;
        // they are drawn before the models or where nothing the reflections see is).
        let flat_att = [opaque, other_slot, spec_kept, spec_kept];
        let ground_att = [opaque, model_slot, spec_kept, spec_kept];
        let ground_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ground_att[..colour_targets]);
        // The outline hulls rewrite the specular targets but not the normal.
        let outline_att = [opaque, other_slot, spec_written, spec_written];
        let outline_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&outline_att[..colour_targets]);
        let flat_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&flat_att[..colour_targets]);
        let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, offset: 0, size: PUSH_SIZE }];
        // Set 0 the colour, 1 the instances (and the look), 2 the mask.
        // Sets 0, 2 and 3: a draw's texture, mask and emission map.
        // Set 4: its specular map. Set 5: the ambient occlusion's map.
        let set_layouts = [set_layout, instance_layout, set_layout, set_layout, set_layout, set_layout];
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

        let two_sided_raster = raster.cull_mode(vk::CullModeFlags::NONE);
        let pipeline_two_sided = unsafe {
            d.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info.rasterization_state(&two_sided_raster)], None).map_err(|(_, e)| e)?[0]
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
            .color_write_mask(vk::ColorComponentFlags::RGBA), other_slot, spec_kept, spec_kept];
        // The forward pass over the characters blends over the lit scene: the specular ambient
        // below it (the reflections' factor) shows by one minus the hull's alpha, its gloss stays.
        let forward_spec = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ZERO)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let forward_att = [sky_blend_att[0], other_slot, forward_spec, spec_kept];
        let forward_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&forward_att[..colour_targets]);
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
        let sky_plane_info = pipeline_info.stages(&sky_stages).vertex_input_state(&sky_input).rasterization_state(&sky_raster).depth_stencil_state(&no_depth).color_blend_state(&flat_blend);
        let sky_plane_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[sky_plane_info], None).map_err(|(_, e)| e)?[0] };
        // The sky's large clouds: a grid with no vertex buffer, blended over the sky like the
        // dome, reading the cloud map (set 0) and the look block (set 1).
        let cloud_set_layout = clouds::set_layout(&gpu)?;
        let cloud_set_layouts = [cloud_set_layout, instance_layout];
        let cloud_layout = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&cloud_set_layouts), None)? };
        let cloud_modules = [module(clouds::STRIP_VERT)?, module(clouds::STRIP_FRAG)?];
        let cloud_stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(cloud_modules[0]).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(cloud_modules[1]).name(c"main"),
        ];
        let cloud_info = pipeline_info.stages(&cloud_stages).vertex_input_state(&sky_input).rasterization_state(&sky_raster).depth_stencil_state(&no_depth).color_blend_state(&sky_blend).layout(cloud_layout);
        let cloud_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[cloud_info], None).map_err(|(_, e)| e)?[0] };
        // The water: ribbons from no vertex buffer, opaque, writing depth, both faces.
        let water_modules = [module(WATER_VERT)?, module(WATER_FRAG)?];
        let water_stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(water_modules[0]).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(water_modules[1]).name(c"main"),
        ];
        let water_info = pipeline_info.stages(&water_stages).vertex_input_state(&sky_input).rasterization_state(&sky_raster).color_blend_state(&flat_blend);
        let water_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[water_info], None).map_err(|(_, e)| e)?[0] };
        // Its far part, drawn again in the distant view's pass (on the far plane, where nothing
        // nearer is): past the far plane its vertices are pulled onto it, and the sky plane,
        // drawn there first, does not cover it; the backdrop's mountains cover it after.
        let water_far_info = water_info.depth_stencil_state(&no_depth);
        let water_far_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[water_far_info], None).map_err(|(_, e)| e)?[0] };

        // The characters' outline pass: the same shaders in outline mode, front faces culled (an
        // inverted hull: only the pushed-out back faces show, as a rim).
        let outline_raster = raster.cull_mode(vk::CullModeFlags::FRONT);
        let outline_info = pipeline_info.rasterization_state(&outline_raster).color_blend_state(&outline_blend);
        let outline_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[outline_info], None).map_err(|(_, e)| e)?[0] };
        let forward_info = outline_info.depth_stencil_state(&no_depth).color_blend_state(&forward_blend);
        let forward_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[forward_info], None).map_err(|(_, e)| e)?[0] };

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
        // The cascades': the game's own bias written in the shader, none in the raster state.
        let shadow_cascade_fs = module(SHADOW_CASCADE_FRAG)?;
        let cascade_stages = [shadow_stages[0], vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(shadow_cascade_fs).name(c"main")];
        let cascade_raster = raster.cull_mode(vk::CullModeFlags::NONE);
        let cascade_info = shadow_info.stages(&cascade_stages).rasterization_state(&cascade_raster);
        let shadow_cascade_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[cascade_info], None).map_err(|(_, e)| e)?[0] };

        // The post pass's pipeline: one triangle over the screen, no vertex input or depth.
        let (post_vs, post_fs) = (module(POST_VERT)?, module(POST_FRAG)?);
        let post_stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(post_vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(post_fs).name(c"main"),
        ];
        let post_push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 80 }];
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
            let tpl_layouts = [terrain_layout, instance_layout, set_layout];
            let tpl = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&tpl_layouts).push_constant_ranges(&push), None)? };
            let tfs = module(TERRAIN_FRAG)?;
            let tstages = [stages[0], vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(tfs).name(c"main").specialization_info(&normal_special)];
            // The heightmap is wound the other way from the models and seen from above only: no
            // culling.
            let traster = raster.cull_mode(vk::CullModeFlags::NONE);
            let info = pipeline_info.stages(&tstages).layout(tpl).rasterization_state(&traster).color_blend_state(&ground_blend);
            let tpipe = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0] };
            TerrainGpu { pipeline_layout: tpl, shader: tfs, pipeline: tpipe }
        };

        // The ambient occlusion's normal and depth pass: the models' and the terrain's own
        // fragment programs, specialised to write the pixel's normal and depth in place of its
        // light (`AO_PASS`), into a half-size target of one sample, back faces culled as the
        // scene's (the terrain's heightmap both ways).
        // The pre-pass runs when the occlusion, the reflections or the depth of field read its
        // normals and depth; the occlusion program itself only with the occlusion.
        let ao = ao::Occlusion::new(&gpu, set_layout, settings.occlusion || reflect_on || dof_on, from_scene, targets.scene_size)?;
        if let Some(normal) = &targets.normal {
            ao.bind_scene(&gpu, targets.depth_view, normal.2);
        }
        let dof = if dof_on {
            let mut dof = dof::DepthOfField::new(&gpu)?;
            dof.rebind(&gpu, &ao, &targets, &post_sets, post_sampler)?;
            Some(dof)
        } else {
            None
        };
        let reflect = if reflect_on {
            let mut r = reflect::Reflections::new(&gpu, settings.scene_format(), settings.rlr)?;
            r.rebind(&gpu, &ao, &targets)?;
            Some(r)
        } else {
            None
        };
        let ao_pass_on = 1u32.to_le_bytes();
        let ao_entry = [vk::SpecializationMapEntry { constant_id: 0, offset: 0, size: 4 }];
        let ao_special = vk::SpecializationInfo::default().map_entries(&ao_entry).data(&ao_pass_on);
        let ao_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att[..1]);
        let ao_stages = |fs| [stages[0], vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main").specialization_info(&ao_special)];
        let ao_models = ao_stages(fs);
        let ao_ground = ao_stages(terrain_gpu.shader);
        let ao_info = pipeline_info.multisample_state(&msaa).color_blend_state(&ao_blend).render_pass(ao.gbuf_pass);
        let ao_ground_raster = raster.cull_mode(vk::CullModeFlags::NONE);
        let ao_gbuf_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[ao_info.stages(&ao_models)], None).map_err(|(_, e)| e)?[0] };
        let ao_gbuf_two_sided_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[ao_info.stages(&ao_models).rasterization_state(&two_sided_raster)], None).map_err(|(_, e)| e)?[0] };
        let ao_gbuf_terrain_pipeline = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[ao_info.stages(&ao_ground).rasterization_state(&ao_ground_raster).layout(terrain_gpu.pipeline_layout)], None).map_err(|(_, e)| e)?[0] };


        write_refraction_set(d, refraction_set, refraction_sampler, targets.refraction_view);
        Ok(Renderer { reflect, dof, water_passes, refraction_set, refraction_pool, refraction_sampler, forward_pipeline, pool, sampler, set_layout, look_ring, instance_layout, terrain_layout, shadow_target, sun, shared, extent, offscreen, color_view, samples, render_pass, post_pass, post_set_layout, post_pool, post_sets, post_sampler, filter, glow_pipelines, upscale_pipelines, taa_pipelines, gsr2_pipelines, fsr3_pipelines, timer, targets, vs, fs, pipeline_layout, pipeline, pipeline_two_sided, sky_pipeline, sky_plane_pipeline, sky_vs, sky_fs, outline_pipeline, shadow_fs, shadow_pipeline, shadow_cascade_fs, shadow_cascade_pipeline, ao, ao_gbuf_pipeline, ao_gbuf_two_sided_pipeline, ao_gbuf_terrain_pipeline, post_vs, post_fs, post_layout, post_pipeline, black_cube, terrain_gpu, settings, show_motion, far_shadow: Default::default(), cloud_set_layout, cloud_layout, cloud_pipeline, cloud_modules, water_pipeline, water_far_pipeline, water_modules, clouds: Default::default(), fixed_cloud_map: None, dump: dump::Dump::from_env(), upto: std::cell::Cell::new(dump::Stop::Full) })
    }

    /// Records a frame into `cb`: the shadow map, the scene, glow and the anti-aliasing or
    /// upscaling the settings ask for, then the post pass and `frame.layers` over it (and
    /// offscreen, the copy into `frame.readback`). Returns how many draws went into the scene
    /// and into the shadow map.
    pub fn record(&mut self, gpu: &Gpu, cb: vk::CommandBuffer, frame: &Frame) -> [u32; 2] {
        let frozen_ready = self.targets.frozen_ready;
        self.targets.frozen_ready = true;
        let d = &gpu.device;
        let Frame { image, width, height, scene, instance_set, draws, instances, sets, white, outline_tex, current_field, hidden, shown, in_sight, material, draw_vp, view_planes: view_planes_now, shadow_vp, shadow_cascades, shadow_bias, shadow_sub, shadow_sides, alpha_cutoff, sway_cutoff, glow, meter, saturation, tone_white, tone_gamma, tone_scale, tone_vivid, filter: filter_strength, ref camera, freeze_now, frozen_weight, overlay, layers, readback, sky_plane, clouds: cloud_frame, water_pieces, occlusion, sun_flare } = *frame;
        let upto = self.upto.get();
        let Renderer { forward_pipeline, ref timer, ref targets, ref shadow_target, shadow_pipeline, shadow_cascade_pipeline, pipeline_layout, render_pass, pipeline, sky_pipeline, sky_plane_pipeline, ref terrain_gpu, outline_pipeline, ref glow_pipelines, ref taa_pipelines, ref fsr3_pipelines, ref gsr2_pipelines, ref upscale_pipelines, post_pass, post_pipeline, post_layout, show_motion, offscreen, extent, .. } = *self;
        let (vbuf, ibuf) = (scene.vbuf, scene.ibuf);
        let ao_set = self.ao.map_set();
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
            let mut trace3d = trace::Draws3d::new();
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
            // cascades' layers each frame; the unused layers cleared once, and the casters of
            // a moved static layer (`shadow::Far`) drawn whole.
            let mut far = self.far_shadow.borrow_mut();
            let mut jobs: Vec<(usize, Option<Mat4>, bool)> = vec![(0, Some(shadow_vp), false)];
            for (k, cascade) in shadow_cascades.iter().enumerate() {
                if cascade.is_some() {
                    jobs.push((k + 1, *cascade, false));
                }
            }
            if !far.cleared {
                far.cleared = true;
                jobs.extend((1..shadow::CASCADES).filter(|l| *l == shadow::STATIC_LAYER || *l == shadow::SUB_LAYER || shadow_cascades[l - 1].is_none()).map(|l| (l, None, false)));
            }
            if let Some(b) = &far.build {
                jobs.push((b.layer, Some(b.vp), false));
            }
            // The characters' map, drawn every frame; cleared once when the game has none.
            if let Some((vp, _)) = shadow_sub {
                jobs.push((shadow::SUB_LAYER, Some(vp), false));
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
                // The characters' map is written `SUB_SIZE` texels a side in the layer's corner; a
                // game's own map sizes (`Frame::shadow_sides`) likewise.
                let [width, height] = match shadow_sides {
                    Some(sides) => sides[layer].map(|v| v.min(shadow_target.size)),
                    None if layer == shadow::SUB_LAYER => [shadow::SUB_SIZE.min(shadow_target.size); 2],
                    None => [shadow_target.size; 2],
                };
                let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width, height } };
                d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: width as f32, height: height as f32, min_depth: 0.0, max_depth: 1.0 }]);
                d.cmd_set_scissor(cb, 0, &[area]);
                d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
                d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
                // A cascade is written with the game's bias where the game gave one.
                let bias = if layer == shadow::SUB_LAYER { shadow_sub.map(|(_, b)| b[0]) } else { shadow_bias.filter(|_| layer <= shadow::STATIC_LAYER).map(|b| b[layer.min(2)]) };
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, if bias.is_some() { shadow_cascade_pipeline } else { shadow_pipeline });
                let mut push = [0f32; 32];
                // Word 31: the lighting material (Draw::light); full for what pushes no draw's own (the terrain).
                push[31] = f32::from_bits(FULL_LIGHT);
                push[..16].copy_from_slice(&shadow_vp.to_cols_array());
                push[16] = bias.map_or(alpha_cutoff, |b| b.alpha_ref);
                if let Some(b) = bias {
                    (push[17], push[18]) = (b.slope, b.constant);
                }
                let stages_pc = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize));
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                let shadow_planes = view_planes(&shadow_vp);
                // In a far layer, casters whose box is under four of its texels across (the
                // map's side over its size) cast nothing seen: left out.
                let least = if layer > 0 && layer != shadow::SUB_LAYER { 4.0 * 2.0 / (shadow_vp.x_axis.x.hypot(shadow_vp.y_axis.x).hypot(shadow_vp.z_axis.x) * width as f32) } else { 0.0 };
                let big = |d: &Draw| least == 0.0 || d.bounds.is_none_or(|b| b.extent(&instances[d.instance as usize].model) >= least);
                // The characters' map takes its own casters and the cascades leave them out.
                let own = |d: &Draw| (layer == shadow::SUB_LAYER) == (d.sub_bias.is_some() && shadow_sub.is_some());
                let casts = |d: &Draw| own(d) && big(d) && d.bounds.is_none_or(|b| b.seen(&instances[d.instance as usize].model, &shadow_planes)) && d.casts && !d.sky && !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && shown(d);
                for draw in draws.iter() {
                    if !casts(draw) {
                        continue;
                    }
                    // xEF_ field models (window light: TOWN_03's obj-xEF_windows_tw03) are not lit, flag
                    // 32: their texture on their vertex colour, as the distant view's clouds.
                    let flags = u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1;
                    let mut per_draw = [0u8; 8];
                    per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                    per_draw[4..].copy_from_slice(&flags.to_le_bytes());
                    // One bias set a draw in the characters' map (the shaders it casts with differ).
                    if let (Some(k), Some((_, sets))) = (draw.sub_bias, shadow_sub) {
                        let b = sets[usize::from(k)];
                        let params = [b.alpha_ref, b.slope, b.constant, 0.0];
                        d.cmd_push_constants(cb, pipeline_layout, stages_pc, 64, std::slice::from_raw_parts(params.as_ptr() as *const u8, 16));
                    }
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                    d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                    if trace3d.on() {
                        trace3d.add(SHADOW_TRACE_PASSES[layer], draw.index_count, &[Some(draw.texture)], &instances[draw.instance as usize].model);
                    }
                    drawn[1] += 1;
                }
                d.cmd_end_render_pass(cb);
                if layer == shadow::STATIC_LAYER && let Some(b) = far.build.take() {
                    far.shown = Some((b.vp, b.layer));
                }
            }
            drop(far);
            if let Some(t) = &timer {
                t.mark(d, cb, 1);
            }
            // The clouds' maps, when the frame has clouds and their constants changed.
            if let Some(want) = &cloud_frame {
                let mut maps = self.clouds.borrow_mut();
                if maps.is_none() {
                    *maps = clouds::Maps::new(gpu, self.pool, self.cloud_set_layout).ok();
                    if let (Some(m), Some(rgba)) = (maps.as_mut(), &self.fixed_cloud_map) {
                        let _ = m.load_map(gpu, self.pool, rgba);
                    }
                }
                if let Some(m) = maps.as_mut() {
                    m.prepare(d, cb, want);
                    if let Some(dump) = &self.dump {
                        dump.grab(gpu, cb, "cloud_map", m.map_image(), (clouds::MAP_SIZE, clouds::MAP_SIZE), dump::Texels::Rgba8, 1.0);
                    }
                }
            }
            // The scene's push constants: view_proj, params, the terrain's scales; the instance
            // number goes in per draw. Word 31: the lighting material (Draw::light); full for
            // what pushes no draw's own (the terrain).
            let scene_push = {
                let mut push = [0f32; 32];
                push[31] = f32::from_bits(FULL_LIGHT);
                push[..16].copy_from_slice(&draw_vp.to_cols_array());
                push[16] = alpha_cutoff;
                push[17] = sway_cutoff;
                push[18] = width as f32 / height.max(1) as f32;
                push[19] = HDR_RANGE_INV;
                push
            };
            let stages_pc = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            // The ambient occlusion: the field's models and terrain (and the distant view's solid
            // pieces) as world normal and depth at half size, the occlusion from them (ao.frag);
            // a map of 1 where the setting or the game has none.
            if self.ao.on() && !self.ao.from_scene() && occlusion {
                self.ao.begin_gbuf(gpu, cb);
                d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
                d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
                let bytes = |p: &[f32; 32]| std::slice::from_raw_parts(p.as_ptr() as *const u8, PUSH_SIZE as usize);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.ao_gbuf_terrain_pipeline);
                let mut ground_push = scene_push;
                // Flags (word 29): 64, the terrain (model.vert: no point lights).
                ground_push[29] = f32::from_bits(64);
                let tg = &terrain_gpu;
                for g in scene.grounds.iter().filter(|g| g.field == current_field) {
                    ground_push[17] = g.cell;
                    ground_push[20] = g.extent[0];
                    ground_push[21] = g.extent[1];
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, tg.pipeline_layout, 0, &[g.set, instance_set, sets[white]], &[]);
                    for patch in g.patches.iter().filter(|p| p.index_count > 0) {
                        // The bake list as the main pass pushes it.
                        ground_push[22..28].copy_from_slice(&scene_gpu::bake_words(&patch.bake).map(f32::from_bits));
                        d.cmd_push_constants(cb, tg.pipeline_layout, stages_pc, 0, bytes(&ground_push));
                        d.cmd_draw_indexed(cb, patch.index_count, 1, patch.first_index, 0, 0);
                    }
                }
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.ao_gbuf_pipeline);
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, bytes(&scene_push));
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                // The models' normal map (flag 8192) is the mask set; the rest hold a white one.
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[white], sets[white], sets[white], sets[white]], &[]);
                // The draws by texture (the order does not matter to a depth test), so a texture is
                // bound once for its run.
                let mut batch: Vec<(&Draw, u32, u32)> = draws
                    .iter()
                    .filter(|d| d.bounds.is_none_or(|b| b.seen(&instances[d.instance as usize].model, &view_planes_now)))
                    .filter(|d| !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && shown(d) && in_sight(d))
                    .filter_map(|draw| {
                        let (bits, tint) = material(draw);
                        // The distant view's solid pieces (flag 16) have the game's own occlusion; its
                        // sky and clouds none.
                        (!draw.sky || bits & 16 != 0).then(|| (draw, u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1 | u32::from(draw.sky) << 3 | bits, tint))
                    })
                    .collect();
                batch.sort_by_key(|(draw, flags, _)| (draw.two_sided, flags & 8192 != 0, draw.texture));
                let (mut texture, mut mask_bound, mut two_sided) = (usize::MAX, false, false);
                for (draw, flags, tint) in batch {
                    if draw.two_sided != two_sided {
                        two_sided = draw.two_sided;
                        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, if two_sided { self.ao_gbuf_two_sided_pipeline } else { self.ao_gbuf_pipeline });
                    }
                    let mut per_draw = [0u8; 16];
                    per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                    per_draw[4..8].copy_from_slice(&flags.to_le_bytes());
                    per_draw[8..12].copy_from_slice(&tint.to_le_bytes());
                    per_draw[12..].copy_from_slice(&draw.light.to_le_bytes());
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                    if draw.texture != texture {
                        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                        texture = draw.texture;
                    }
                    // A model's normal map (flag 8192) is the mask set; the rest hold a white one.
                    let mask = flags & 8192 != 0;
                    if mask || mask_bound {
                        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[draw.mask.filter(|_| mask).unwrap_or(white)]], &[]);
                        mask_bound = mask;
                    }
                    d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                }
                d.cmd_end_render_pass(cb);
                self.ao.shade(gpu, cb, self.settings.occlusion.then(|| ao::View::of(&draw_vp)).as_ref());
            } else {
                self.ao.shade(gpu, cb, None);
            }
            if let (Some(dump), Some((image, size))) = (&self.dump, self.ao.map_image()) {
                dump.grab(gpu, cb, "ao", image, size, dump::Texels::Rgba8, 1.0);
            }
            // The normals and view depth the occlusion reads (at half size; the clear depth where
            // nothing is drawn): the coverage the board compares against the game's depth.
            if !self.ao.from_scene() {
                self.dump_gbuf(gpu, cb);
            }
            if let Some(t) = &timer {
                t.mark(d, cb, 2);
            }
            let clear = [
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.12, 0.12, 0.14, 1.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
                // The motion target's (no movement), when there is one, and the reflections'
                // two (nothing to add): the attachments after depth, whichever are there.
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
            ];
            // The reflections run on a frame whose normals and depth were drawn, at the stage
            // that has the whole scene.
            let reflect_frame = self.reflect.is_some() && self.ao.on() && occlusion && upto == dump::Stop::Full;
            // A frame with water draws the scene in two parts, the water refracting a copy of the
            // first (it needs the occlusion's depth of the opaque scene).
            // The depth of field also needs the occlusion's depth, at the stage with the whole scene.
            let dof_frame = self.dof.is_some() && self.ao.on() && occlusion && upto == dump::Stop::Full;
            let refract = water_pieces > 0 && upto != dump::Stop::Chara && self.ao.on() && occlusion;
            d.cmd_begin_render_pass(
                cb,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(if refract { self.water_passes[0] } else { render_pass })
                    .framebuffer(targets.scene_framebuffer)
                    .render_area(scene_area)
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: scene_width as f32, height: scene_height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[scene_area]);
            d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
            d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
            let mut push = scene_push;
            // The terrain's push constants: the shared ones with its field size and layer scales.
            let mut terrain_push = push;
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            let pc = std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize);
            d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 5, &[ao_set], &[]);
            // The terrain and the field's models first, then the current field's distant view where
            // they leave the far plane bare (as if drawn first and covered, without shading the
            // covered part).
            for (pass_sky, pipe) in [(false, pipeline), (true, sky_pipeline)] {
            if pass_sky && !upto.sky() {
                break;
            }
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
                        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, tg.pipeline_layout, 0, &[g.set, instance_set, ao_set], &[]);
                        // A patch at a time, its bake list in the push constants' floats 22 to 27
                        // (their bits: the count, then two entries a word).
                        for patch in g.patches.iter().filter(|p| p.index_count > 0) {
                            terrain_push[22..28].copy_from_slice(&scene_gpu::bake_words(&patch.bake).map(f32::from_bits));
                            d.cmd_push_constants(cb, tg.pipeline_layout, stages_pc, 0, std::slice::from_raw_parts(terrain_push.as_ptr() as *const u8, PUSH_SIZE as usize));
                            d.cmd_draw_indexed(cb, patch.index_count, 1, patch.first_index, 0, 0);
                            if trace3d.on() {
                                trace3d.add("terrain", patch.index_count, &[], &Mat4::IDENTITY);
                            }
                        }
                    }
                }
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 5, &[ao_set], &[]);
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
            if pass_sky && sky_plane && water_pieces > 0 {
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.water_far_pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[if refract { self.refraction_set } else { sets[white] }], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[if refract { self.ao.gbuf_set() } else { sets[white] }], &[]);
                d.cmd_draw(cb, water_pieces * WATER_PIECE_VERTICES, 1, 0, 0);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipe);
            }
            let mut two_sided = false;
            let mut dome_seen = 0usize;
            for draw in draws.iter().filter(|d| d.bounds.is_none_or(|b| b.seen(&instances[d.instance as usize].model, &view_planes_now))).filter(|d| d.sky == pass_sky && !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && shown(d) && in_sight(d)).filter(|d| {
                // The stage dumps' stops inside the dome: its first layers only.
                if pass_sky && d.name == "sky dome" {
                    dome_seen += 1;
                    return dome_seen <= upto.dome_layers();
                }
                true
            }) {
                // The instance, and whether it is shaded as a character (not part of a field), as
                // an eye and as the distant view; the game's own material bits and tint.
                let (bits, tint) = material(draw);
                let flags = u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1 | u32::from(draw.sky) << 3 | bits;
                if draw.two_sided != two_sided {
                    two_sided = draw.two_sided;
                    d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, if two_sided { self.pipeline_two_sided } else { pipe });
                }
                let mut per_draw = [0u8; 16];
                per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                per_draw[4..8].copy_from_slice(&flags.to_le_bytes());
                per_draw[8..12].copy_from_slice(&tint.to_le_bytes());
                per_draw[12..].copy_from_slice(&draw.light.to_le_bytes());
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[draw.mask.unwrap_or(white)], sets[draw.emission.unwrap_or(white)], sets[draw.specular.unwrap_or(white)]], &[]);
                d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                if trace3d.on() {
                    let inst = &instances[draw.instance as usize];
                    // A character's mask slot holds the toon ramp (flag 128), a texture of Barrel's own.
                    let mask = draw.mask.filter(|_| flags & 128 == 0);
                    trace3d.add(if pass_sky { "sky" } else { "scene" }, draw.index_count, &[Some(draw.texture), mask, draw.normal_map, draw.emission], &inst.model);
                    if let Some((lo, hi)) = draw.part.or(draw.bounds.map(|b| (b.lo, b.hi))) {
                        trace3d.bounds(lo, hi);
                    }
                    trace3d.what(&draw.name, draw.class, draw.lod, draw.object);
                    if !draw.bones.is_empty() {
                        trace3d.bones(&frame.scene.traced_palette(), inst.palette as usize, &draw.bones, &inst.model);
                    }
                }
                drawn[0] += 1;
            }
            // The water, over the opaque scene (it writes depth: the sky plane shows past it).
            if !pass_sky && water_pieces > 0 && upto != dump::Stop::Chara {
                if refract {
                    // The second part of the scene pass, after a copy of the first.
                    d.cmd_end_render_pass(cb);
                    // The half-size normal and depth target from the opaque scene, which the water
                    // and the reflections read (the depth read in the shader layout meanwhile).
                    let derive = self.ao.from_scene() && occlusion;
                    if derive {
                        barrier(d, cb, targets.depth, vk::ImageAspectFlags::DEPTH, 1, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                        self.ao.derive(gpu, cb, &draw_vp);
                        self.dump_gbuf(gpu, cb);
                    }
                    // The reflections go into what the water refracts; the scene is left as the copy reads it.
                    if let Some(r) = self.reflect.as_ref().filter(|_| reflect_frame) {
                        r.record(gpu, cb, &ao::View::of(&draw_vp), true);
                        self.dump_reflections(gpu, cb, 1.0 / (HDR_RANGE_INV * meter.as_ref().map_or(1.0, |m| m.baked)));
                    } else if self.reflect.is_some() {
                        barrier(d, cb, targets.scene, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
                    }
                    let layers = vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1);
                    let region = vk::ImageCopy::default().src_subresource(layers).dst_subresource(layers).extent(vk::Extent3D { width: scene_width, height: scene_height, depth: 1 });
                    barrier(d, cb, targets.refraction, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
                    d.cmd_copy_image(cb, targets.scene, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, targets.refraction, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
                    barrier(d, cb, targets.refraction, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    barrier(d, cb, targets.scene, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                    // The kept colour and depth, written by the first part, are read again by the second.
                    let (kept, aspect) = (targets.multisample.map_or(targets.scene, |m| m.0), vk::ImageAspectFlags::COLOR);
                    barrier(d, cb, kept, aspect, 1, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                    let depth_was = if derive { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL } else { vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
                    barrier(d, cb, targets.depth, vk::ImageAspectFlags::DEPTH, 1, depth_was, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
                    if let Some((motion, ..)) = targets.motion {
                        barrier(d, cb, motion, aspect, 1, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                    }
                    d.cmd_begin_render_pass(
                        cb,
                        &vk::RenderPassBeginInfo::default().render_pass(self.water_passes[1]).framebuffer(targets.scene_framebuffer).render_area(scene_area).clear_values(&clear),
                        vk::SubpassContents::INLINE,
                    );
                    d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: scene_width as f32, height: scene_height as f32, min_depth: 0.0, max_depth: 1.0 }]);
                    d.cmd_set_scissor(cb, 0, &[scene_area]);
                    d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
                    d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 5, &[ao_set], &[]);
                }
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.water_pipeline);
                // Set 0 the refraction (white: none), set 2 the opaque scene's depth.
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[if refract { self.refraction_set } else { sets[white] }], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[if refract { self.ao.gbuf_set() } else { sets[white] }], &[]);
                d.cmd_draw(cb, water_pieces * WATER_PIECE_VERTICES, 1, 0, 0);
                // One trace entry a piece (the game draws each on its own).
                if trace3d.on() {
                    for _ in 0..water_pieces {
                        trace3d.add("water", WATER_PIECE_VERTICES, &[], &Mat4::IDENTITY);
                    }
                }
            }
            // The sky's large clouds, over the dome (on the far plane too, so where nothing covers it).
            if pass_sky && upto.clouds() && cloud_frame.is_some() && let Some(m) = self.clouds.borrow().as_ref() {
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.cloud_pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.cloud_layout, 0, &[m.set, instance_set], &[]);
                d.cmd_draw(cb, clouds::GRID_VERTICES, 1, 0, 0);
            }
            }
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: scene_width as f32, height: scene_height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            // The outlines: the outlined materials again, pushed out, with the outline texture.
            if let Some(outline_tex) = outline_tex.filter(|_| upto.outlines()) {
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, outline_pipeline);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[outline_tex], sets[white], sets[white]], &[]);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 5, &[ao_set], &[]);
                for draw in draws.iter().filter(|d| !hidden[d.instance as usize] && d.field.is_none() && !d.eye) {
                    let Some(colour) = draw.outline else { continue };
                    let mut per_draw = [0u8; 12];
                    per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                    per_draw[4..8].copy_from_slice(&(1u32 | 4).to_le_bytes());
                    per_draw[8..].copy_from_slice(&colour.to_le_bytes());
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                    d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                    if trace3d.on() {
                        trace3d.add("outline", draw.index_count, &[Some(draw.texture)], &instances[draw.instance as usize].model);
                    }
                }
            }
            // The forward pass over the characters (a Ryza character's toon-ramp draws): the
            // game's second look at them, the hull blended over the lit scene.
            if upto.outlines() {
                let mut bound = false;
                for draw in draws.iter().filter(|d| !hidden[d.instance as usize] && d.field.is_some() && !d.sky) {
                    let (bits, _) = material(draw);
                    if bits & 128 == 0 {
                        continue;
                    }
                    if !bound {
                        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, forward_pipeline);
                        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
                        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 5, &[ao_set], &[]);
                        bound = true;
                    }
                    let flags = (bits & 128) | u32::from(draw.specular.is_some()) << 24 | 1 << 25;
                    let mut per_draw = [0u8; 16];
                    per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                    per_draw[4..8].copy_from_slice(&flags.to_le_bytes());
                    per_draw[12..].copy_from_slice(&draw.light.to_le_bytes());
                    d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                    d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[white], sets[white], sets[draw.specular.unwrap_or(white)]], &[]);
                    d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
                }
            }
            d.cmd_end_render_pass(cb);
            // The half-size normal and depth target from the scene pass (a frame with water built
            // it from the opaque part).
            if self.ao.from_scene() && occlusion && !refract {
                self.ao.derive(gpu, cb, &draw_vp);
                self.dump_gbuf(gpu, cb);
            }
            // The reflections over the opaque scene (a frame with water did them before its copy).
            if let Some(r) = self.reflect.as_ref().filter(|_| reflect_frame && !refract) {
                r.record(gpu, cb, &ao::View::of(&draw_vp), false);
                self.dump_reflections(gpu, cb, 1.0 / (HDR_RANGE_INV * meter.as_ref().map_or(1.0, |m| m.baked)));
            }
            // The stage dumps: the scene in the game's units (it is stored at half and lit at
            // the exposure it was baked with).
            let baked = meter.as_ref().map_or(1.0, |m| m.baked);
            if let Some(dump) = &self.dump {
                // The sun's share is written as it is, times the storage's half.
                let scene_scale = if matches!(upto, dump::Stop::Shadow | dump::Stop::Chara) { 1.0 / HDR_RANGE_INV } else { 1.0 / (HDR_RANGE_INV * baked) };
                dump.grab(gpu, cb, upto.name(), targets.scene, targets.scene_size, dump::Texels::of(self.settings.scene_format()), scene_scale);
            }
            trace3d.finish(gpu.frame.get(), &draw_vp, &shadow_vp);
            if let Some(t) = &timer {
                t.mark(d, cb, 3);
            }
            // The depth of field's weight, layers and blur (the post pass mixes them in).
            if let Some(dof) = self.dof.as_ref().filter(|_| dof_frame) {
                dof.record(gpu, cb);
                if let (Some(dump), Some((weight, size))) = (&self.dump, dof.weight_image()) {
                    dump.grab(gpu, cb, "dof_weight", weight, size, dump::Texels::Rgba16F, 1.0);
                }
                if let (Some(dump), Some(([front, back], size))) = (&self.dump, dof.layer_images()) {
                    let k = 1.0 / (HDR_RANGE_INV * baked);
                    dump.grab(gpu, cb, "dof_layers", front, size, dump::Texels::Rgba16F, k);
                    dump.grab(gpu, cb, "dof_layers_back", back, size, dump::Texels::Rgba16F, k);
                }
            }
            // The glow from the scene's bright parts: its bright pass's threshold, and its rate in
            // the post pass.
            let (threshold, rate) = glow;
            if let Some(g) = &targets.glow {
                g.record(d, &glow_pipelines, cb, threshold, rate, tone_white, meter.as_ref());
                if let (Some(dump), Some((bright, bloom))) = (self.dump.as_ref().filter(|_| upto == dump::Stop::Full), g.bloom_images()) {
                    // The glow's targets are stored at half too (the post pass doubles them).
                    let k = 1.0 / HDR_RANGE_INV;
                    dump.grab(gpu, cb, "bright", bright.0, bright.1, dump::Texels::Rgba16F, k);
                    dump.grab(gpu, cb, "bloom", bloom.0, bloom.1, dump::Texels::Rgba16F, k);
                }
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
                t.mark(d, cb, 4);
            }
            // The sun flare's visibility, and the wash's settings for the post pass.
            // The reference point is in the first cascade's map: within the layer where the map is smaller.
            let sun_flare = sun_flare.clone().map(|mut s| {
                if let Some([[w, h], ..]) = shadow_sides {
                    s.reference[0] *= w as f32 / shadow_target.size as f32;
                    s.reference[1] *= h as f32 / shadow_target.size as f32;
                }
                s
            });
            self.sun.record(d, cb, sun_flare.as_ref());
            // The post pass: the scene back to its range with the glow, then the text over it.
            d.cmd_begin_render_pass(
                cb,
                &vk::RenderPassBeginInfo::default().render_pass(post_pass).framebuffer(targets.framebuffers[image]).render_area(area),
                vk::SubpassContents::INLINE,
            );
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, post_pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, post_layout, 0, &[targets.post_set()], &[]);
            let mut post_push = [0u8; 80];
            post_push[48..52].copy_from_slice(&filter_strength.to_le_bytes());
            let metered = meter.is_some() && targets.glow.as_ref().is_some_and(|g| g.has_meter());
            post_push[52..56].copy_from_slice(&(if metered { 1.0f32 } else { 0.0 }).to_le_bytes());
            post_push[56..60].copy_from_slice(&(if dof_frame { 1.0f32 } else { 0.0 }).to_le_bytes());
            post_push[..4].copy_from_slice(&rate.to_le_bytes());
            post_push[4..8].copy_from_slice(&frozen_weight.to_le_bytes());
            post_push[8..12].copy_from_slice(&(if show_motion { 1.0f32 } else { 0.0 }).to_le_bytes());
            for (k, v) in saturation.iter().enumerate() {
                post_push[32 + 4 * k..36 + 4 * k].copy_from_slice(&v.to_le_bytes());
            }
            post_push[44..48].copy_from_slice(&tone_white.to_le_bytes());
            post_push[12..16].copy_from_slice(&tone_gamma.to_le_bytes());
            for (k, v) in tone_scale.iter().enumerate() {
                post_push[64 + 4 * k..68 + 4 * k].copy_from_slice(&v.to_le_bytes());
            }
            post_push[76..80].copy_from_slice(&tone_vivid.to_le_bytes());
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
                t.mark(d, cb, 5);
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

    /// The stage dump of the half-size normals and view depth the occlusion, the reflections and
    /// the depth of field read (the clear depth where nothing is drawn): the coverage the board
    /// compares against the game's depth.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the target was drawn.
    unsafe fn dump_gbuf(&self, gpu: &Gpu, cb: vk::CommandBuffer) {
        if let (Some(dump), Some((image, size))) = (&self.dump, self.ao.gbuf_image()) {
            unsafe { dump.grab(gpu, cb, "depth", image, size, dump::Texels::Rgba32F, 1.0) };
        }
    }

    /// The stage dump of the reflections' march (`reflections`: the colour and the alpha, in the
    /// scene's units as stored).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the reflections were recorded.
    unsafe fn dump_reflections(&self, gpu: &Gpu, cb: vk::CommandBuffer, scale: f32) {
        if let (Some(dump), Some((image, size))) = (&self.dump, self.reflect.as_ref().and_then(|r| r.image())) {
            unsafe { dump.grab(gpu, cb, "reflections", image, size, dump::Texels::Rgba16F, scale) };
        }
    }

    /// The targets again at a new window size (after the presenter's swapchain was rebuilt).
    pub fn rebuild_targets(&mut self, gpu: &Gpu, presenter: Option<&Presenter>, width: u32, height: u32) -> Result<()> {
        self.targets.destroy(gpu);
        let s = &self.settings;
        self.ao.resize(gpu, s.scene_size(width, height))?;
        let targets = Targets::new(gpu, self.render_pass, self.post_pass, &target_views(presenter, self.color_view), width, height, s.scene_size(width, height), self.post_sets, self.post_sampler, &self.glow_pipelines, self.upscale_pipelines.as_ref(), self.taa_pipelines.as_ref(), self.gsr2_pipelines.as_ref(), self.fsr3_pipelines.as_ref(), self.samples, s.motion, self.show_motion, s.scene_format(), self.reflect.is_some(), self.ao.from_scene())?;
        write_refraction_set(&gpu.device, self.refraction_set, self.refraction_sampler, targets.refraction_view);
        if let Some(normal) = &targets.normal {
            self.ao.bind_scene(gpu, targets.depth_view, normal.2);
        }
        if let Some(r) = self.reflect.as_mut() {
            r.rebind(gpu, &self.ao, &targets)?;
        }
        if let Some(dof) = self.dof.as_mut() {
            dof.rebind(gpu, &self.ao, &targets, &self.post_sets, self.post_sampler)?;
        }
        self.targets = targets;
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
            d.destroy_image_view(self.black_cube.2, None);
            d.destroy_image(self.black_cube.0, None);
            d.free_memory(self.black_cube.1, None);
            d.destroy_image_view(self.filter.2, None);
            d.destroy_image(self.filter.0, None);
            d.free_memory(self.filter.1, None);
            d.destroy_render_pass(self.post_pass, None);
            for pass in self.water_passes {
                d.destroy_render_pass(pass, None);
            }
            d.destroy_descriptor_pool(self.refraction_pool, None);
            d.destroy_sampler(self.refraction_sampler, None);
            d.destroy_pipeline(self.outline_pipeline, None);
            d.destroy_pipeline(self.forward_pipeline, None);
            d.destroy_pipeline(self.ao_gbuf_pipeline, None);
            d.destroy_pipeline(self.ao_gbuf_two_sided_pipeline, None);
            d.destroy_pipeline(self.pipeline_two_sided, None);
            d.destroy_pipeline(self.ao_gbuf_terrain_pipeline, None);
            d.destroy_pipeline(self.shadow_pipeline, None);
            d.destroy_pipeline(self.shadow_cascade_pipeline, None);
            d.destroy_shader_module(self.shadow_cascade_fs, None);
            d.destroy_shader_module(self.shadow_fs, None);
            d.destroy_pipeline(self.sky_pipeline, None);
            d.destroy_pipeline(self.sky_plane_pipeline, None);
            d.destroy_shader_module(self.sky_vs, None);
            d.destroy_shader_module(self.sky_fs, None);
            d.destroy_pipeline(self.cloud_pipeline, None);
            d.destroy_pipeline(self.water_pipeline, None);
            d.destroy_pipeline(self.water_far_pipeline, None);
            for m in self.water_modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_pipeline_layout(self.cloud_layout, None);
            d.destroy_descriptor_set_layout(self.cloud_set_layout, None);
            for m in self.cloud_modules {
                d.destroy_shader_module(m, None);
            }
            if let Some(m) = self.clouds.take() {
                m.destroy(gpu);
            }
            self.sun.destroy(gpu);
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
        if let Some(r) = self.reflect.take() {
            r.destroy(gpu);
        }
        if let Some(dof) = self.dof.take() {
            dof.destroy(gpu);
        }
        self.ao.destroy(gpu);
        unsafe {
            d.destroy_descriptor_set_layout(self.instance_layout, None);
            d.destroy_command_pool(self.pool, None);
        }
        Ok(())
    }
}

/// Points the post pass's binding 3 (the screen filter) at `view`, in both sets.
/// Points the post pass's sets at the sun flare's settings buffer and sun visibility.
fn write_wash(gpu: &Gpu, sets: &[vk::DescriptorSet; 2], sampler: vk::Sampler, sun: &flare::Sun) {
    let buffer = [vk::DescriptorBufferInfo::default().buffer(sun.settings()).range(vk::WHOLE_SIZE)];
    let image = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(sun.visibility()).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = sets.iter().flat_map(|s| {
        [
            vk::WriteDescriptorSet::default().dst_set(*s).dst_binding(5).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&buffer),
            vk::WriteDescriptorSet::default().dst_set(*s).dst_binding(6).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&image),
        ]
    });
    let writes: Vec<_> = writes.collect();
    unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
}

/// The post pass's depth-of-field bindings (the weight, the two layers) set to `view`, which the
/// shader never reads while the frame has no depth of field.
fn write_dof_placeholder(gpu: &Gpu, sets: &[vk::DescriptorSet; 2], sampler: vk::Sampler, view: vk::ImageView) {
    let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes: Vec<_> = sets.iter().flat_map(|s| (7..=9).map(move |b| (*s, b))).map(|(s, b)| vk::WriteDescriptorSet::default().dst_set(s).dst_binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)).collect();
    unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
}

fn write_filter(gpu: &Gpu, sets: &[vk::DescriptorSet; 2], sampler: vk::Sampler, view: vk::ImageView) {
    let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = sets.map(|s| vk::WriteDescriptorSet::default().dst_set(s).dst_binding(3).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info));
    unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
}

impl Renderer {
    /// The screen filter's texture (Escha's `PostEffect/texture/overlay.g1t`), replacing the
    /// one before; [`Frame::filter`] says how strongly it is laid over.
    pub fn set_filter(&mut self, gpu: &Gpu, texture: &barrel_formats::g1t::Texture) -> Result<()> {
        let new = upload_texture(gpu, self.pool, texture, false)?;
        unsafe { gpu.device.device_wait_idle()? };
        write_filter(gpu, &self.post_sets, self.post_sampler, new.2);
        let old = std::mem::replace(&mut self.filter, new);
        unsafe {
            gpu.device.destroy_image_view(old.2, None);
            gpu.device.destroy_image(old.0, None);
            gpu.device.free_memory(old.1, None);
        }
        Ok(())
    }
}
