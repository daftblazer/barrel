//! A movie picture on screen: its I420 planes uploaded each frame into three one-channel
//! images (half the bytes of RGBA) and turned into RGB by the fragment shader.

use ash::vk;

use crate::{barrier, Gpu, Result};

const MOVIE_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/movie.vert.spv"));
const MOVIE_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/movie.frag.spv"));

#[repr(C)]
#[derive(Clone, Copy)]
struct Push {
    screen: [f32; 2],
    level: f32,
    pad: f32,
    rect: [f32; 4],
}

pub struct MovieLayer {
    planes: [(vk::Image, vk::DeviceMemory, vk::ImageView); 3],
    sizes: [(u32, u32); 3],
    staging: (vk::Buffer, vk::DeviceMemory, *mut u8),
    sampler: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    /// A picture was set and not uploaded yet.
    dirty: bool,
    /// A picture has been uploaded (before that nothing is drawn).
    ready: bool,
}

impl MovieLayer {
    /// For pictures of `width` × `height` (even), drawn in `render_pass`.
    pub fn new(gpu: &Gpu, pool: vk::CommandPool, render_pass: vk::RenderPass, width: u32, height: u32) -> Result<MovieLayer> {
        let d = &gpu.device;
        let sizes = [(width, height), (width / 2, height / 2), (width / 2, height / 2)];
        let mut planes = Vec::new();
        for &(w, h) in &sizes {
            let (image, mem) = gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8_UNORM)
                    .extent(vk::Extent3D { width: w, height: h, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED),
            )?;
            let view = unsafe {
                d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::R8_UNORM)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )?
            };
            planes.push((image, mem, view));
        }
        let planes: [(vk::Image, vk::DeviceMemory, vk::ImageView); 3] = [planes[0], planes[1], planes[2]];
        // Every upload goes SHADER_READ → TRANSFER_DST → SHADER_READ, so start there.
        gpu.submit(pool, |cb| {
            for p in &planes {
                barrier(d, cb, p.0, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::UNDEFINED, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            }
        })?;
        let bytes: u64 = sizes.iter().map(|&(w, h)| u64::from(w) * u64::from(h)).sum();
        let (buffer, smem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        unsafe {
            let mapped = d.map_memory(smem, 0, bytes, vk::MemoryMapFlags::empty())? as *mut u8;
            let sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?;
            let bindings: Vec<_> = (0..3)
                .map(|b| vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT))
                .collect();
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)?;
            let sizes_pool = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 3 }];
            let dpool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes_pool), None)?;
            let layouts = [set_layout];
            let set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(dpool).set_layouts(&layouts))?[0];
            for (b, p) in planes.iter().enumerate() {
                let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(p.2).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                d.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(b as u32).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)], &[]);
            }
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, offset: 0, size: size_of::<Push>() as u32 }];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(MOVIE_VERT)?, module(MOVIE_FRAG)?];
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(modules[1]).name(c"main"),
            ];
            let vinput = vk::PipelineVertexInputStateCreateInfo::default();
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let vstate = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
            let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
            let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth = vk::PipelineDepthStencilStateCreateInfo::default();
            let blend_att = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
            let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vinput)
                .input_assembly_state(&assembly)
                .viewport_state(&vstate)
                .rasterization_state(&raster)
                .multisample_state(&msaa)
                .depth_stencil_state(&depth)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic_state)
                .layout(layout)
                .render_pass(render_pass);
            let pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0];
            Ok(MovieLayer { planes, sizes, staging: (buffer, smem, mapped), sampler, set_layout, pool: dpool, set, layout, pipeline, modules, dirty: false, ready: false })
        }
    }

    /// Sets the picture to show (its Y, U and V planes, tightly packed). The caller has waited
    /// for the frame that last drew from the layer.
    pub fn set(&mut self, y: &[u8], u: &[u8], v: &[u8]) {
        let mut at = 0usize;
        for (k, plane) in [y, u, v].into_iter().enumerate() {
            let (w, h) = self.sizes[k];
            let n = (w * h) as usize;
            unsafe { std::ptr::copy_nonoverlapping(plane.as_ptr(), self.staging.2.add(at), n.min(plane.len())) };
            at += n;
        }
        self.dirty = true;
    }

    /// Records the upload of a picture set since the last frame (outside any render pass).
    pub fn upload(&mut self, d: &ash::Device, cb: vk::CommandBuffer) {
        if !self.dirty {
            return;
        }
        let mut offset = 0u64;
        for (k, p) in self.planes.iter().enumerate() {
            let (w, h) = self.sizes[k];
            barrier(d, cb, p.0, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
            let region = vk::BufferImageCopy::default()
                .buffer_offset(offset)
                .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                .image_extent(vk::Extent3D { width: w, height: h, depth: 1 });
            unsafe { d.cmd_copy_buffer_to_image(cb, self.staging.0, p.0, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]) };
            barrier(d, cb, p.0, vk::ImageAspectFlags::COLOR, 1, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            offset += u64::from(w) * u64::from(h);
        }
        self.dirty = false;
        self.ready = true;
    }

    /// Draws the picture over `rect` (pixels, y down) on a `screen`-sized target, times
    /// `level` (0 black to 1), inside the render pass.
    pub fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, screen: [f32; 2], rect: [f32; 4], level: f32) {
        if !self.ready {
            return;
        }
        let push = Push { screen, level, pad: 0.0, rect };
        unsafe {
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[self.set], &[]);
            let bytes = std::slice::from_raw_parts((&push as *const Push).cast::<u8>(), size_of::<Push>());
            d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytes);
            d.cmd_draw(cb, 6, 1, 0, 0);
        }
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.layout, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.sampler, None);
            for p in self.planes {
                d.destroy_image_view(p.2, None);
                d.destroy_image(p.0, None);
                d.free_memory(p.1, None);
            }
            d.unmap_memory(self.staging.1);
            d.destroy_buffer(self.staging.0, None);
            d.free_memory(self.staging.1, None);
        }
    }
}
