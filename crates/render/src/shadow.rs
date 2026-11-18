//! The sun's shadow map: a depth image of `CASCADES` layers (the cascades, each a box fitted
//! to a slice of the view, and the static layer over the whole field) the field's casters
//! are drawn into from the sun, and a comparison
//! sampler the field's receivers read it through (`model.frag`, `terrain.frag`;
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

/// The map's layers: three cascades (layer 0 the near square every game has; Ryza adds the
/// next two, `Frame::shadow_cascades`), then the static layer (`Far`), then the characters'
/// own map (`Frame::shadow_sub`).
pub const CASCADES: usize = 5;
/// The static layer: the casters of the whole field, drawn once (the game's semi-static map).
pub const STATIC_LAYER: usize = 3;
/// The characters' map (the game's `sSubShadow`): drawn every frame at `SUB_SIZE` texels a
/// side in the layer's corner, read through its own sampling (`shadow_cascades.glsl`).
pub const SUB_LAYER: usize = 4;
/// The characters' map's side in texels (the game's `scSubSM`: 1024 over 1024), or the layer's
/// when that is smaller (the handhelds).
pub const SUB_SIZE: u32 = 1024;

/// How the game writes a cascade's depth (its shadow pass's `sbias`): the depth stored is the
/// depth plus `slope` times its change over a texel (`|dz/dx| + |dz/dy|`) plus `constant` (a
/// fraction of the cascade's depth range), and a texel under `alpha_ref` of alpha casts nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Bias {
    pub slope: f32,
    pub constant: f32,
    pub alpha_ref: f32,
}

/// The static layer: the view-projection it was drawn with, which the receivers read, and the
/// one being drawn. A game asking for another has it drawn whole in that frame.
#[derive(Default)]
pub struct Far {
    pub shown: Option<(glam::Mat4, usize)>,
    pub build: Option<FarBuild>,
    /// The layer cleared once (a game without one never reads it).
    pub cleared: bool,
}

pub struct FarBuild {
    pub vp: glam::Mat4,
    pub layer: usize,
}

impl Far {
    /// This frame's static layer: `want` is where the game has it. Gives what the receivers
    /// read (its view-projection and layer; None: none) and plans the drawing, before the scene.
    pub fn plan(&mut self, want: Option<glam::Mat4>) -> Option<(glam::Mat4, usize)> {
        let Some(vp) = want else {
            self.build = None;
            return None;
        };
        if self.shown.is_some_and(|(m, _)| m == vp) {
            self.build = None;
            return self.shown;
        }
        self.build = Some(FarBuild { vp, layer: STATIC_LAYER });
        Some((vp, STATIC_LAYER))
    }
}

pub struct ShadowTarget {
    pub image: vk::Image,
    memory: vk::DeviceMemory,
    /// The whole array, for the receivers.
    pub view: vk::ImageView,
    layer_views: [vk::ImageView; CASCADES],
    pub sampler: vk::Sampler,
    pub pass: vk::RenderPass,
    /// The same, keeping the layer's depth (a far square's later slices).
    pub pass_load: vk::RenderPass,
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
            let attachment_load = [attachment[0].load_op(vk::AttachmentLoadOp::LOAD).initial_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let pass_load = d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachment_load).subpasses(&subpass).dependencies(&deps), None)?;
            let mut framebuffers = [vk::Framebuffer::null(); CASCADES];
            for (f, v) in framebuffers.iter_mut().zip(&layer_views) {
                let views = [*v];
                *f = d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(size).height(size).layers(1), None)?;
            }
            Ok(ShadowTarget { image, memory, view, layer_views, sampler, pass, pass_load, framebuffers, size })
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
            d.destroy_render_pass(self.pass_load, None);
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

/// The sun's view and projection over the smallest box around `points` (render space) in the
/// sun's view (up the way of +Y), its depth starting at `plane` (a point's distance along the
/// sun's way, `p . dir`; the box starts there unless the points begin sooner) so casters up
/// the sun's rays are inside: one cascade over a slice of the view's frustum.
#[allow(deprecated)] // glam's older names, as above
pub fn fit_view_proj(points: &[glam::Vec3], dir: glam::Vec3, plane: f32) -> glam::Mat4 {
    let d = dir.normalize_or(glam::Vec3::NEG_Y);
    let up = if d.y.abs() > 0.95 { glam::Vec3::Z } else { glam::Vec3::Y };
    let view = glam::Mat4::look_to_rh(glam::Vec3::ZERO, d, up);
    let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
    for p in points {
        let q = view.transform_point3(*p);
        (lo, hi) = (lo.min(q), hi.max(q));
    }
    // The sun looks down -z of its view: the nearest point to the sun has the largest z.
    glam::Mat4::orthographic_rh(lo.x, hi.x, lo.y, hi.y, (-hi.z).min(plane), -lo.z) * view
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn a_fitted_box_holds_its_points_and_starts_at_the_plane() {
        let sun = Vec3::new(-0.35, -0.7, -0.6).normalize();
        let points = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1000.0, 200.0, -300.0), Vec3::new(-500.0, 80.0, 900.0), Vec3::new(300.0, 600.0, 100.0)];
        let plane = points.iter().map(|p| p.dot(sun)).fold(f32::MAX, f32::min) - 5000.0;
        let m = fit_view_proj(&points, sun, plane);
        for p in points {
            let c = m.project_point3(p);
            assert!(c.x >= -1.001 && c.x <= 1.001 && c.y >= -1.001 && c.y <= 1.001 && c.z >= 0.0 && c.z <= 1.001, "{c:?}");
        }
        // A point on the plane has depth 0.
        assert!(m.project_point3(sun * plane).z.abs() < 1e-4);
        // With no plane before the points, the box starts at the nearest of them.
        let tight = fit_view_proj(&points, sun, f32::MAX);
        let nearest = points.iter().map(|p| tight.project_point3(*p).z).fold(f32::MAX, f32::min);
        assert!(nearest.abs() < 1e-4);
    }
}
