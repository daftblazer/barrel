//! `barrel-view`: draw one model offscreen with Vulkan and write the frame as PNG.
//!
//!     barrel-view MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png [--size N] [--yaw DEG]
//!
//! It renders a single frame and exits: no window, no loop. Set BARREL_VALIDATION=1 to
//! enable the Khronos validation layer.

use std::ffi::CStr;
use std::path::Path;

use ash::vk;
use barrel_formats::elixir::Elixir;
use barrel_formats::g1m::G1m;
use barrel_formats::g1t::{Format, G1t};
use glam::{Mat4, Vec3};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.vert.spv"));
const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.frag.spv"));
const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

fn entry_bytes(spec: &str) -> Result<Vec<u8>> {
    let (file, n) = spec.rsplit_once('#').ok_or("expected FILE#N")?;
    let bytes = std::fs::read(file)?;
    let e = Elixir::parse(&bytes)?;
    let entry = e.entries.get(n.parse::<usize>()?).ok_or("no such entry")?;
    Ok(e.data(entry).to_vec())
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

struct Draw {
    first_index: u32,
    index_count: u32,
    texture: usize,
}

struct Gpu {
    _entry: ash::Entry,
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    queue_family: u32,
    memory: vk::PhysicalDeviceMemoryProperties,
}

impl Gpu {
    fn new() -> Result<Gpu> {
        let entry = unsafe { ash::Entry::load()? };
        let validation = std::env::var("BARREL_VALIDATION").is_ok_and(|v| v == "1");
        let layers: Vec<*const i8> = if validation { vec![c"VK_LAYER_KHRONOS_validation".as_ptr()] } else { vec![] };
        let app = vk::ApplicationInfo::default().application_name(c"barrel-view").api_version(vk::API_VERSION_1_1);
        let instance = unsafe {
            entry.create_instance(&vk::InstanceCreateInfo::default().application_info(&app).enabled_layer_names(&layers), None)?
        };
        let devices = unsafe { instance.enumerate_physical_devices()? };
        let pick = |kind| {
            devices.iter().copied().find(|&d| unsafe { instance.get_physical_device_properties(d) }.device_type == kind)
        };
        let physical = pick(vk::PhysicalDeviceType::DISCRETE_GPU)
            .or_else(|| pick(vk::PhysicalDeviceType::INTEGRATED_GPU))
            .or_else(|| devices.first().copied())
            .ok_or("no Vulkan device")?;
        let props = unsafe { instance.get_physical_device_properties(physical) };
        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) };
        eprintln!("device: {}{}", name.to_string_lossy(), if validation { " (validation on)" } else { "" });
        let families = unsafe { instance.get_physical_device_queue_family_properties(physical) };
        let queue_family = families
            .iter()
            .position(|f| f.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or("no graphics queue")? as u32;
        let features = unsafe { instance.get_physical_device_features(physical) };
        if features.texture_compression_bc == 0 {
            return Err("device lacks BC texture compression".into());
        }
        let priorities = [1.0f32];
        let queues = [vk::DeviceQueueCreateInfo::default().queue_family_index(queue_family).queue_priorities(&priorities)];
        let enabled = vk::PhysicalDeviceFeatures::default().texture_compression_bc(true);
        let device = unsafe {
            instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default().queue_create_infos(&queues).enabled_features(&enabled),
                None,
            )?
        };
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let memory = unsafe { instance.get_physical_device_memory_properties(physical) };
        Ok(Gpu { _entry: entry, instance, physical, device, queue, queue_family, memory })
    }

    fn memory_type(&self, bits: u32, flags: vk::MemoryPropertyFlags) -> Result<u32> {
        (0..self.memory.memory_type_count)
            .find(|&i| bits & (1 << i) != 0 && self.memory.memory_types[i as usize].property_flags.contains(flags))
            .ok_or_else(|| "no suitable memory type".into())
    }

    fn buffer(&self, size: u64, usage: vk::BufferUsageFlags, host: bool) -> Result<(vk::Buffer, vk::DeviceMemory)> {
        let d = &self.device;
        let buffer = unsafe { d.create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage), None)? };
        let req = unsafe { d.get_buffer_memory_requirements(buffer) };
        let flags = if host {
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        } else {
            vk::MemoryPropertyFlags::DEVICE_LOCAL
        };
        let mem = unsafe {
            d.allocate_memory(
                &vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(self.memory_type(req.memory_type_bits, flags)?),
                None,
            )?
        };
        unsafe { d.bind_buffer_memory(buffer, mem, 0)? };
        Ok((buffer, mem))
    }

    fn upload(&self, mem: vk::DeviceMemory, bytes: &[u8]) -> Result<()> {
        unsafe {
            let p = self.device.map_memory(mem, 0, bytes.len() as u64, vk::MemoryMapFlags::empty())? as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            self.device.unmap_memory(mem);
        }
        Ok(())
    }

    fn image(&self, info: &vk::ImageCreateInfo) -> Result<(vk::Image, vk::DeviceMemory)> {
        let d = &self.device;
        let image = unsafe { d.create_image(info, None)? };
        let req = unsafe { d.get_image_memory_requirements(image) };
        let mem = unsafe {
            d.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(self.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?),
                None,
            )?
        };
        unsafe { d.bind_image_memory(image, mem, 0)? };
        Ok((image, mem))
    }

    /// Record with `f` into a one-off command buffer, submit it and wait (bounded).
    fn submit(&self, pool: vk::CommandPool, f: impl FnOnce(vk::CommandBuffer)) -> Result<()> {
        let d = &self.device;
        let cb = unsafe {
            d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1))?[0]
        };
        unsafe {
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?
        };
        f(cb);
        unsafe { d.end_command_buffer(cb)? };
        let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None)? };
        let cbs = [cb];
        unsafe {
            d.queue_submit(self.queue, &[vk::SubmitInfo::default().command_buffers(&cbs)], fence)?;
            d.wait_for_fences(&[fence], true, 10_000_000_000)?;
            d.destroy_fence(fence, None);
            d.free_command_buffers(pool, &cbs);
        }
        Ok(())
    }
}

fn barrier(
    d: &ash::Device,
    cb: vk::CommandBuffer,
    image: vk::Image,
    aspect: vk::ImageAspectFlags,
    levels: u32,
    from: vk::ImageLayout,
    to: vk::ImageLayout,
) {
    let b = vk::ImageMemoryBarrier::default()
        .image(image)
        .old_layout(from)
        .new_layout(to)
        .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
        .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(levels).layer_count(1));
    unsafe {
        d.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[b],
        )
    };
}

#[allow(deprecated)] // glam 0.33 renamed the camera helpers; the old ones are kept
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: barrel-view MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png [--size N] [--yaw DEG]".into());
    }
    let mut size = 768u32;
    let mut yaw = 0f32;
    let mut it = args[3..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--size" => size = it.next().ok_or("--size N")?.parse::<u32>()?.clamp(64, 1280),
            "--yaw" => yaw = it.next().ok_or("--yaw DEG")?.parse()?,
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    // CPU side: the model's vertices and draws, and the texture pack.
    let model_bytes = entry_bytes(&args[0])?;
    let model = G1m::parse(&model_bytes)?;
    let geometry = model.geometry.as_ref().ok_or("model has no geometry")?;
    let tex_bytes = entry_bytes(&args[1])?;
    let pack = G1t::parse(&tex_bytes)?;
    let mut vertices = Vec::<Vertex>::new();
    let mut indices = Vec::<u32>::new();
    let mut draws = Vec::new();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for s in &geometry.submeshes {
        let mesh = geometry.simple_mesh(s)?;
        if mesh.uvs.is_empty() || mesh.normals.is_empty() {
            continue;
        }
        let base = vertices.len() as u32;
        for i in 0..mesh.positions.len() {
            let p = Vec3::from(mesh.positions[i]);
            lo = lo.min(p);
            hi = hi.max(p);
            vertices.push(Vertex { pos: mesh.positions[i], normal: mesh.normals[i], uv: mesh.uvs[i] });
        }
        let first_index = indices.len() as u32;
        for t in &mesh.triangles {
            indices.extend(t.iter().map(|&v| v + base));
        }
        let texture = geometry
            .materials
            .get(mesh.material)
            .and_then(|m| m.textures.iter().find(|t| t.kind == 1).or(m.textures.first()))
            .map_or(0, |t| t.index as usize)
            .min(pack.textures.len().saturating_sub(1));
        draws.push(Draw { first_index, index_count: indices.len() as u32 - first_index, texture });
    }
    if draws.is_empty() || pack.textures.is_empty() {
        return Err("nothing to draw".into());
    }
    eprintln!("{} draws, {} vertices, {} triangles, {} textures", draws.len(), vertices.len(), indices.len() / 3, pack.textures.len());

    let gpu = Gpu::new()?;
    let d = &gpu.device;
    let pool = unsafe {
        d.create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(gpu.queue_family), None)?
    };

    // Geometry buffers (host-visible: one frame does not need a staging copy).
    let vbytes = unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * size_of::<Vertex>()) };
    let ibytes = unsafe { std::slice::from_raw_parts(indices.as_ptr() as *const u8, indices.len() * 4) };
    let (vbuf, vmem) = gpu.buffer(vbytes.len() as u64, vk::BufferUsageFlags::VERTEX_BUFFER, true)?;
    gpu.upload(vmem, vbytes)?;
    let (ibuf, imem) = gpu.buffer(ibytes.len() as u64, vk::BufferUsageFlags::INDEX_BUFFER, true)?;
    gpu.upload(imem, ibytes)?;

    // Textures, uploaded as their BC blocks with all mip levels.
    let mut textures = Vec::new();
    for t in &pack.textures {
        let format = match t.format {
            Format::Bc1 => vk::Format::BC1_RGBA_UNORM_BLOCK,
            Format::Bc3 => vk::Format::BC3_UNORM_BLOCK,
            Format::Rgba8 => vk::Format::R8G8B8A8_UNORM,
        };
        let levels = t.levels.len() as u32;
        let (image, mem) = gpu.image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width: t.width, height: t.height, depth: 1 })
                .mip_levels(levels)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED),
        )?;
        let data: Vec<u8> = t.levels.concat();
        let (staging, smem) = gpu.buffer(data.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        gpu.upload(smem, &data)?;
        let mut regions = Vec::new();
        let mut offset = 0u64;
        for (level, bytes) in t.levels.iter().enumerate() {
            regions.push(
                vk::BufferImageCopy::default()
                    .buffer_offset(offset)
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).mip_level(level as u32).layer_count(1))
                    .image_extent(vk::Extent3D { width: (t.width >> level).max(1), height: (t.height >> level).max(1), depth: 1 }),
            );
            offset += bytes.len() as u64;
        }
        gpu.submit(pool, |cb| unsafe {
            barrier(d, cb, image, vk::ImageAspectFlags::COLOR, levels, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
            d.cmd_copy_buffer_to_image(cb, staging, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &regions);
            barrier(d, cb, image, vk::ImageAspectFlags::COLOR, levels, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        })?;
        unsafe {
            d.destroy_buffer(staging, None);
            d.free_memory(smem, None);
        }
        let view = unsafe {
            d.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(levels).layer_count(1)),
                None,
            )?
        };
        textures.push((image, mem, view));
    }
    let sampler = unsafe {
        d.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .max_lod(16.0),
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
    let pool_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: textures.len() as u32 }];
    let dpool = unsafe {
        d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(textures.len() as u32).pool_sizes(&pool_sizes), None)?
    };
    let layouts = vec![set_layout; textures.len()];
    let sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(dpool).set_layouts(&layouts))? };
    for (set, (_, _, view)) in sets.iter().zip(&textures) {
        let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(*view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(*set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&info);
        unsafe { d.update_descriptor_sets(&[write], &[]) };
    }

    // Offscreen target.
    let extent = vk::Extent3D { width: size, height: size, depth: 1 };
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
    let (color, cmem) = gpu.image(&target_info(COLOR_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC))?;
    let (depth, dmem) = gpu.image(&target_info(DEPTH_FORMAT, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT))?;
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
    let color_view = view_of(color, COLOR_FORMAT, vk::ImageAspectFlags::COLOR)?;
    let depth_view = view_of(depth, DEPTH_FORMAT, vk::ImageAspectFlags::DEPTH)?;

    let attachments = [
        vk::AttachmentDescription::default()
            .format(COLOR_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL),
        vk::AttachmentDescription::default()
            .format(DEPTH_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    ];
    let color_ref = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
    let depth_ref = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
    let subpass = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)
        .depth_stencil_attachment(&depth_ref)];
    let render_pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpass), None)? };
    let fb_views = [color_view, depth_view];
    let framebuffer = unsafe {
        d.create_framebuffer(
            &vk::FramebufferCreateInfo::default().render_pass(render_pass).attachments(&fb_views).width(size).height(size).layers(1),
            None,
        )?
    };

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
    ];
    let vinput = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbind).vertex_attribute_descriptions(&vattr);
    let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = [vk::Viewport { x: 0.0, y: 0.0, width: size as f32, height: size as f32, min_depth: 0.0, max_depth: 1.0 }];
    let scissor = [vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: size, height: size } }];
    let vstate = vk::PipelineViewportStateCreateInfo::default().viewports(&viewport).scissors(&scissor);
    let raster = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .line_width(1.0);
    let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth_state = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS);
    let blend_att = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
    let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_att);
    let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX, offset: 0, size: 64 }];
    let set_layouts = [set_layout];
    let pipeline_layout = unsafe {
        d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push), None)?
    };
    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vinput)
        .input_assembly_state(&assembly)
        .viewport_state(&vstate)
        .rasterization_state(&raster)
        .multisample_state(&msaa)
        .depth_stencil_state(&depth_state)
        .color_blend_state(&blend)
        .layout(pipeline_layout)
        .render_pass(render_pass);
    let pipeline = unsafe {
        d.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).map_err(|(_, e)| e)?[0]
    };

    // Camera: look at the model's bounds from its front, turned by --yaw. Models face +z.
    let center = (lo + hi) * 0.5;
    let radius = (hi - lo).length() * 0.5;
    let fov = 30f32.to_radians();
    let dist = radius / (fov * 0.5).sin() * 1.05;
    let dir = Vec3::new(yaw.to_radians().sin(), 0.0, yaw.to_radians().cos());
    let view = Mat4::look_at_rh(center + dir * dist, center, Vec3::Y);
    let mut proj = Mat4::perspective_rh(fov, 1.0, dist * 0.01, dist * 4.0);
    proj.y_axis.y *= -1.0; // Vulkan's y points down.
    let mvp = proj * view;
    // The game's data is left-handed (DirectX); mirror x so the model is not flipped.
    let mvp = mvp * Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));

    // Render one frame and read it back.
    let bytes = (size * size * 4) as u64;
    let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
    gpu.submit(pool, |cb| unsafe {
        let clear = [
            vk::ClearValue { color: vk::ClearColorValue { float32: [0.12, 0.12, 0.14, 1.0] } },
            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
        ];
        d.cmd_begin_render_pass(
            cb,
            &vk::RenderPassBeginInfo::default()
                .render_pass(render_pass)
                .framebuffer(framebuffer)
                .render_area(scissor[0])
                .clear_values(&clear),
            vk::SubpassContents::INLINE,
        );
        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
        d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
        d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
        let m = mvp.to_cols_array();
        let pc = std::slice::from_raw_parts(m.as_ptr() as *const u8, 64);
        d.cmd_push_constants(cb, pipeline_layout, vk::ShaderStageFlags::VERTEX, 0, pc);
        for draw in &draws {
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
            d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
        }
        d.cmd_end_render_pass(cb);
        let region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
            .image_extent(extent);
        d.cmd_copy_image_to_buffer(cb, color, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, readback, &[region]);
    })?;
    let mut pixels = vec![0u8; bytes as usize];
    unsafe {
        let p = d.map_memory(rmem, 0, bytes, vk::MemoryMapFlags::empty())? as *const u8;
        std::ptr::copy_nonoverlapping(p, pixels.as_mut_ptr(), pixels.len());
        d.unmap_memory(rmem);
    }
    std::fs::write(Path::new(&args[2]), barrel_formats::png::encode_rgba(size, size, &pixels))?;
    eprintln!("wrote {}", args[2]);

    // Tear down in reverse order.
    unsafe {
        d.device_wait_idle()?;
        d.destroy_buffer(readback, None);
        d.free_memory(rmem, None);
        d.destroy_pipeline(pipeline, None);
        d.destroy_pipeline_layout(pipeline_layout, None);
        d.destroy_shader_module(vs, None);
        d.destroy_shader_module(fs, None);
        d.destroy_framebuffer(framebuffer, None);
        d.destroy_render_pass(render_pass, None);
        d.destroy_image_view(color_view, None);
        d.destroy_image_view(depth_view, None);
        d.destroy_image(color, None);
        d.free_memory(cmem, None);
        d.destroy_image(depth, None);
        d.free_memory(dmem, None);
        d.destroy_descriptor_pool(dpool, None);
        d.destroy_descriptor_set_layout(set_layout, None);
        d.destroy_sampler(sampler, None);
        for (image, mem, view) in textures {
            d.destroy_image_view(view, None);
            d.destroy_image(image, None);
            d.free_memory(mem, None);
        }
        d.destroy_buffer(vbuf, None);
        d.free_memory(vmem, None);
        d.destroy_buffer(ibuf, None);
        d.free_memory(imem, None);
        d.destroy_command_pool(pool, None);
        d.destroy_device(None);
        gpu.instance.destroy_instance(None);
    }
    let _ = gpu.physical;
    Ok(())
}
