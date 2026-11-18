//! The Ryza games' sky clouds (docs/shading.md, "Ryza DX: the sky, the clouds"): two offscreen
//! maps the strip's program (`cloud_strip.*`, drawn in the sky pass) marches through. The
//! density map, a tileable noise, is made when the field's noise changes; the cloud map (the
//! density past a threshold, shaped to a soft edge) when its own constants do. Both are 1024
//! by 1024 and made by full-screen passes; nothing runs while the constants hold still.

use ash::vk;

use crate::{upload_texture, Gpu, Result};

/// The two maps' side.
pub const MAP_SIZE: u32 = 1024;
/// The grid's cells along a side (`cloud_strip.vert`).
pub const GRID_CELLS: u32 = 16;
/// The vertices of the grid's triangles.
pub const GRID_VERTICES: u32 = GRID_CELLS * GRID_CELLS * 6;

const DENSITY_FORMAT: vk::Format = vk::Format::R8_UNORM;
const MAP_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
pub const DENSITY_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cloud_density.frag.spv"));
pub const MAP_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cloud_map.frag.spv"));
pub const STRIP_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cloud_strip.vert.spv"));
pub const STRIP_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cloud_strip.frag.spv"));

/// The density map's noise (the game's `vSamplingRangeU/V`, `vPersistence`, `vNoiseScale`,
/// `nOctave`): `range` the noise's extent round each of the map's two axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Density {
    pub range: [f32; 2],
    pub persistence: f32,
    pub scale: f32,
    pub octaves: u32,
}

/// The cloud map's constants (`vThreshold`, `vSharpness`, `vThinness`, `vHeightScale`), a value
/// for each of its four channels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub threshold: [f32; 4],
    pub sharpness: [f32; 4],
    pub thinness: [f32; 4],
    pub height: [f32; 4],
}

/// What a frame asks of the clouds' maps (its sky draws the strip from the look block's cloud
/// slots, `CLOUD_SLOT`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudFrame {
    pub density: Density,
    pub shape: Shape,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DensityPush {
    shape: [f32; 4],
    offset: [f32; 4],
    octaves: u32,
}

/// The binding every set of the clouds reads: one texture, in the fragment stage.
pub fn set_layout(gpu: &Gpu) -> Result<vk::DescriptorSetLayout> {
    let binding = [vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)];
    Ok(unsafe { gpu.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)? })
}

/// A permutation of 0 to 255, the same every run (a Fisher-Yates shuffle by a fixed
/// xorshift): the noise's lattice hash. The game shuffles its own at random each run; any
/// permutation gives noise of the same kind.
pub fn permutation() -> [u8; 256] {
    let mut table: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut state = 0x9e37_79b9u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    for i in (1..256).rev() {
        table.swap(i, next() as usize % (i + 1));
    }
    table
}

struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
    set: vk::DescriptorSet,
}

/// The maps and the passes that make them; the strip reads `set`.
pub struct Maps {
    pool: vk::DescriptorPool,
    sampler: vk::Sampler,
    density_pass: vk::RenderPass,
    map_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    modules: [vk::ShaderModule; 3],
    density_pipeline: vk::Pipeline,
    map_pipeline: vk::Pipeline,
    permutation: (vk::Image, vk::DeviceMemory, vk::ImageView),
    permutation_set: vk::DescriptorSet,
    density: Target,
    map: Target,
    made_density: Option<Density>,
    made_shape: Option<(Density, Shape)>,
    /// The cloud map, for the strip's set 0.
    pub set: vk::DescriptorSet,
}

impl Maps {
    pub fn new(gpu: &Gpu, command_pool: vk::CommandPool, set_layout: vk::DescriptorSetLayout) -> Result<Maps> {
        let d = &gpu.device;
        unsafe {
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 3 }];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(3).pool_sizes(&sizes), None)?;
            // Repeating and smooth: the maps tile.
            let sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::REPEAT)
                    .address_mode_w(vk::SamplerAddressMode::REPEAT),
                None,
            )?;
            let density_pass = crate::glow::pass(gpu, DENSITY_FORMAT, vk::AttachmentLoadOp::DONT_CARE)?;
            let map_pass = crate::glow::pass(gpu, MAP_FORMAT, vk::AttachmentLoadOp::DONT_CARE)?;
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: size_of::<Shape>().max(size_of::<DensityPush>()) as u32 }];
            let layouts = [set_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(POST_VERT)?, module(DENSITY_FRAG)?, module(MAP_FRAG)?];
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
            let make = |fragment: vk::ShaderModule, render_pass: vk::RenderPass| {
                let stages = [
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main"),
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
                    .render_pass(render_pass);
                d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map(|p| p[0]).map_err(|(_, e)| e)
            };
            let density_pipeline = make(modules[1], density_pass)?;
            let map_pipeline = make(modules[2], map_pass)?;

            let set_for = |view: vk::ImageView| -> Result<vk::DescriptorSet> {
                let layouts = [set_layout];
                let set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))?[0];
                let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]);
                Ok(set)
            };
            let target = |format: vk::Format, render_pass: vk::RenderPass| -> Result<Target> {
                let (image, memory) = gpu.image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D { width: MAP_SIZE, height: MAP_SIZE, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED),
                )?;
                let view = d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )?;
                let views = [view];
                let framebuffer = d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(render_pass).attachments(&views).width(MAP_SIZE).height(MAP_SIZE).layers(1), None)?;
                Ok(Target { image, memory, view, framebuffer, set: set_for(view)? })
            };
            let density = target(DENSITY_FORMAT, density_pass)?;
            let map = target(MAP_FORMAT, map_pass)?;

            // The permutation, a 256 by 1 texture (the value in each channel).
            let bytes: Vec<u8> = permutation().iter().flat_map(|&v| [v; 4]).collect();
            let texture = barrel_formats::g1t::Texture { width: 256, height: 1, format: barrel_formats::g1t::Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&bytes[..]] };
            let permutation = upload_texture(gpu, command_pool, &texture, false)?;
            let permutation_set = set_for(permutation.2)?;
            let set = map.set;
            Ok(Maps { pool, sampler, density_pass, map_pass, layout, modules, density_pipeline, map_pipeline, permutation, permutation_set, density, map, made_density: None, made_shape: None, set })
        }
    }

    /// Makes what `want` asks that is not made yet: the density map when its noise changed, the
    /// cloud map when its constants or the density map did.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass.
    pub unsafe fn prepare(&mut self, d: &ash::Device, cb: vk::CommandBuffer, want: &CloudFrame) {
        let run = |pass: vk::RenderPass, target: &Target, pipeline: vk::Pipeline, set: vk::DescriptorSet, push: &[u8]| unsafe {
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: MAP_SIZE, height: MAP_SIZE } };
            d.cmd_begin_render_pass(cb, &vk::RenderPassBeginInfo::default().render_pass(pass).framebuffer(target.framebuffer).render_area(area), vk::SubpassContents::INLINE);
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: MAP_SIZE as f32, height: MAP_SIZE as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[area]);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[set], &[]);
            d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::FRAGMENT, 0, push);
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        };
        if self.made_density != Some(want.density) {
            let n = &want.density;
            let push = DensityPush { shape: [n.range[0], n.range[1], n.persistence, n.scale], offset: [0.0; 4], octaves: n.octaves };
            run(self.density_pass, &self.density, self.density_pipeline, self.permutation_set, unsafe { bytes(&push) });
            self.made_density = Some(want.density);
        }
        if self.made_shape != Some((want.density, want.shape)) {
            run(self.map_pass, &self.map, self.map_pipeline, self.density.set, unsafe { bytes(&want.shape) });
            self.made_shape = Some((want.density, want.shape));
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for t in [self.density, self.map] {
                d.destroy_framebuffer(t.framebuffer, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            d.destroy_image_view(self.permutation.2, None);
            d.destroy_image(self.permutation.0, None);
            d.free_memory(self.permutation.1, None);
            d.destroy_pipeline(self.density_pipeline, None);
            d.destroy_pipeline(self.map_pipeline, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_render_pass(self.density_pass, None);
            d.destroy_render_pass(self.map_pass, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_descriptor_pool(self.pool, None);
        }
    }
}

/// A plain value's bytes, for push constants.
///
/// # Safety
/// `T` has no padding bytes that matter (all floats and integers).
unsafe fn bytes<T>(v: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((v as *const T) as *const u8, size_of::<T>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_permutation_is_a_shuffle_and_the_same_each_time() {
        let p = permutation();
        let mut seen = [false; 256];
        for v in p {
            assert!(!std::mem::replace(&mut seen[v as usize], true));
        }
        assert_eq!(p, permutation());
        assert_ne!(p[..8], [0, 1, 2, 3, 4, 5, 6, 7]);
    }
}
