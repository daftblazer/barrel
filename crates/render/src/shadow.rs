//! The sun's shadow map: a depth image of `CASCADES` layers (the near square over the
//! camera's target; a wider one when the game gives it, else cleared) the field's casters are
//! drawn into from the sun, and a comparison sampler the field's receivers read it through (`model.frag`, `terrain.frag`;
//! `docs/renderer.md`, "Shadows").

use ash::vk;

use crate::{Gpu, Result};

/// The map's size and depth format: 2048² of 32-bit float depth on the desktop; on the
/// handhelds (`Shadows::HANDHELD`) a quarter the texels at 16 bits, for their bandwidth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shadows {
    pub size: u32,
    pub format: vk::Format,
}

impl Shadows {
    pub const DESKTOP: Shadows = Shadows { size: 2048, format: vk::Format::D32_SFLOAT };
    pub const HANDHELD: Shadows = Shadows { size: 1024, format: vk::Format::D16_UNORM };
}

/// The map's layers: the near square, then the far one.
pub const CASCADES: usize = 2;

pub struct ShadowTarget {
    pub image: vk::Image,
    memory: vk::DeviceMemory,
    /// The whole array, for the receivers.
    pub view: vk::ImageView,
    layer_views: [vk::ImageView; CASCADES],
    pub sampler: vk::Sampler,
    pub pass: vk::RenderPass,
    /// One per layer, to draw into.
    pub framebuffers: [vk::Framebuffer; CASCADES],
    pub size: u32,
}

impl ShadowTarget {
    pub fn new(gpu: &Gpu, shadows: Shadows) -> Result<ShadowTarget> {
        let Shadows { size, format } = shadows;
        let d = &gpu.device;
        let (image, memory) = gpu.image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width: size, height: size, depth: 1 })
                .mip_levels(1)
                .array_layers(CASCADES as u32)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
        )?;
        unsafe {
            let make_view = |kind: vk::ImageViewType, first: u32, count: u32| {
                d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(kind)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::DEPTH).level_count(1).base_array_layer(first).layer_count(count)),
                    None,
                )
            };
            let view = make_view(vk::ImageViewType::TYPE_2D_ARRAY, 0, CASCADES as u32)?;
            let mut layer_views = [vk::ImageView::null(); CASCADES];
            for (k, v) in layer_views.iter_mut().enumerate() {
                *v = make_view(vk::ImageViewType::TYPE_2D, k as u32, 1)?;
            }
            // Outside the map counts as lit (the border is the far plane).
            let sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
                    .compare_enable(true)
                    .compare_op(vk::CompareOp::LESS_OR_EQUAL),
                None,
            )?;
            let attachment = [vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let depth_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
            let subpass = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).depth_stencil_attachment(&depth_ref)];
            // The scene's fragment shaders read the map after it is written.
            let deps = [vk::SubpassDependency {
                src_subpass: 0,
                dst_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                src_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                dst_access_mask: vk::AccessFlags::SHADER_READ,
                dependency_flags: vk::DependencyFlags::empty(),
            }];
            let pass = d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment).subpasses(&subpass).dependencies(&deps), None)?;
            let mut framebuffers = [vk::Framebuffer::null(); CASCADES];
            for (f, v) in framebuffers.iter_mut().zip(&layer_views) {
                let views = [*v];
                *f = d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(size).height(size).layers(1), None)?;
            }
            Ok(ShadowTarget { image, memory, view, layer_views, sampler, pass, framebuffers, size })
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            for (f, v) in self.framebuffers.iter().zip(&self.layer_views) {
                d.destroy_framebuffer(*f, None);
                d.destroy_image_view(*v, None);
            }
            d.destroy_render_pass(self.pass, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_image_view(self.view, None);
            d.destroy_image(self.image, None);
            d.free_memory(self.memory, None);
        }
    }
}

/// The sun's view and projection over a square `2 × half` field units across, centred on
/// `centre`, looking along `dir` (render space), depth over `depth` units each way.
#[allow(deprecated)] // glam's older names, as main.rs uses them
pub fn sun_view_proj(centre: glam::Vec3, dir: glam::Vec3, half: f32, depth: f32) -> glam::Mat4 {
    let d = dir.normalize_or(glam::Vec3::NEG_Y);
    let up = if d.y.abs() > 0.95 { glam::Vec3::Z } else { glam::Vec3::Y };
    let view = glam::Mat4::look_at_rh(centre - d * depth, centre, up);
    let proj = glam::Mat4::orthographic_rh(-half, half, -half, half, 0.0, 2.0 * depth);
    proj * view
}
