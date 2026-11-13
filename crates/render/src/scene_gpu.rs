//! One scene's resources on the GPU: its geometry, its textures with their descriptor sets, and
//! its instances (model matrices, joint palette, morph offsets and weights) with their set.
//! The renderer (device, targets, pipelines, the post chain) outlives scenes; a scene is uploaded
//! when it is loaded and destroyed when it is dropped (`docs/renderer.md`, "Chained events").

use ash::vk;

use glam::Mat4;

use crate::{Gpu, Instance, Result};

fn bytes<T>(s: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

/// A scene as uploaded: the CPU side `main` builds.
pub struct SceneData<'a> {
    pub vertices: &'a [u8],
    pub indices: &'a [u8],
    pub textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pub instances: &'a [Instance],
    pub palette: &'a [Mat4],
    pub deltas: &'a [[f32; 4]],
    pub weights: &'a [f32],
    /// Each field's ground: its 8 textures uploaded (2 blend maps, 6 layers).
    pub grounds: Vec<GroundData>,
}

pub struct GroundData {
    pub field: u16,
    pub textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pub first_index: u32,
    pub index_count: u32,
    pub field_size: f32,
    pub scales: [f32; 6],
}

/// A field's ground as drawn: its texture set (`Shared::terrain_layout`) and its indices.
pub struct Ground {
    pub field: u16,
    pub set: vk::DescriptorSet,
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pub first_index: u32,
    pub index_count: u32,
    pub field_size: f32,
    pub scales: [f32; 6],
}

/// What a scene's sets point at that the renderer owns: the layouts, the texture sampler, the
/// character lighting buffer (binding 4) and the sun's shadow map (binding 5).
pub struct Shared {
    pub set_layout: vk::DescriptorSetLayout,
    pub sampler: vk::Sampler,
    pub instance_layout: vk::DescriptorSetLayout,
    pub look: vk::Buffer,
    pub shadow_sampler: vk::Sampler,
    pub shadow_view: vk::ImageView,
    pub terrain_layout: vk::DescriptorSetLayout,
}

pub struct SceneGpu {
    pub vbuf: vk::Buffer,
    vmem: vk::DeviceMemory,
    pub ibuf: vk::Buffer,
    imem: vk::DeviceMemory,
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pool: vk::DescriptorPool,
    instance_mem: vk::DeviceMemory,
    /// The joint palette's memory: this frame's joints, then last frame's at `half`.
    palette_mem: vk::DeviceMemory,
    half: u64,
    weight_mem: vk::DeviceMemory,
    buffers: [vk::Buffer; 4],
    delta_mem: vk::DeviceMemory,
    inst_pool: vk::DescriptorPool,
    /// Set 1: instances (0), palette (1), morph offsets (2) and weights (3), lighting (4), shadow (5).
    pub instance_set: vk::DescriptorSet,
    ground_pool: Option<vk::DescriptorPool>,
    pub grounds: Vec<Ground>,
    /// One set per texture (layout `set_layout`: one combined image sampler), in texture order.
    pub sets: Vec<vk::DescriptorSet>,
}

impl SceneGpu {
    /// Uploads the scene: vertices and indices, instances, the palette (twice: last frame's
    /// is this one's to begin with), morphs (all host-visible, rewritten between frames), and
    /// takes the uploaded textures, giving each a descriptor set through `shared.sampler`.
    pub fn new(gpu: &Gpu, data: SceneData, shared: &Shared) -> Result<SceneGpu> {
        let d = &gpu.device;
        let host = |b: &[u8], usage| -> Result<(vk::Buffer, vk::DeviceMemory)> {
            let (buf, mem) = gpu.buffer(b.len() as u64, usage, true)?;
            gpu.upload(mem, b)?;
            Ok((buf, mem))
        };
        let (vbuf, vmem) = host(data.vertices, vk::BufferUsageFlags::VERTEX_BUFFER)?;
        let (ibuf, imem) = host(data.indices, vk::BufferUsageFlags::INDEX_BUFFER)?;
        let textures = data.textures;
        let n = textures.len() as u32;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: n }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)? };
        let layouts = vec![shared.set_layout; textures.len()];
        let sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))? };
        for (set, (_, _, view)) in sets.iter().zip(&textures) {
            let info = [vk::DescriptorImageInfo::default().sampler(shared.sampler).image_view(*view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let write = vk::WriteDescriptorSet::default()
                .dst_set(*set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&info);
            unsafe { d.update_descriptor_sets(&[write], &[]) };
        }

        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let (instance_buf, instance_mem) = host(bytes(data.instances), storage)?;
        let half = bytes(data.palette).len() as u64;
        let (palette_buf, palette_mem) = gpu.buffer((2 * half).max(64), storage, true)?;
        gpu.upload(palette_mem, bytes(data.palette))?;
        gpu.upload_at(palette_mem, half, bytes(data.palette))?;
        let (delta_buf, delta_mem) = host(bytes(data.deltas), storage)?;
        let (weight_buf, weight_mem) = host(bytes(data.weights), storage)?;
        let inst_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 4 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 1 },
        ];
        let inst_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&inst_sizes), None)? };
        let inst_layouts = [shared.instance_layout];
        let instance_set =
            unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(inst_pool).set_layouts(&inst_layouts))?[0] };
        let whole = |b| [vk::DescriptorBufferInfo::default().buffer(b).offset(0).range(vk::WHOLE_SIZE)];
        let infos = [whole(instance_buf), whole(palette_buf), whole(delta_buf), whole(weight_buf)];
        let look_info = whole(shared.look);
        let shadow_info = [vk::DescriptorImageInfo::default()
            .sampler(shared.shadow_sampler)
            .image_view(shared.shadow_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let mut writes: Vec<_> = infos
            .iter()
            .enumerate()
            .map(|(b, info)| vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(b as u32).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(info))
            .collect();
        writes.push(vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&look_info));
        writes.push(
            vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(5).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&shadow_info),
        );
        unsafe { d.update_descriptor_sets(&writes, &[]) };

        let mut ground_pool = None;
        let mut grounds = Vec::new();
        if !data.grounds.is_empty() {
            let n = data.grounds.len() as u32;
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 8 * n }];
            let tpool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)? };
            ground_pool = Some(tpool);
            for g in data.grounds {
                let tl = [shared.terrain_layout];
                let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(tpool).set_layouts(&tl))?[0] };
                let infos: Vec<vk::DescriptorImageInfo> = g
                    .textures
                    .iter()
                    .map(|(_, _, view)| vk::DescriptorImageInfo::default().sampler(shared.sampler).image_view(*view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL))
                    .collect();
                let writes = [
                    vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos[..2]),
                    vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos[2..]),
                ];
                unsafe { d.update_descriptor_sets(&writes, &[]) };
                grounds.push(Ground {
                    field: g.field,
                    set,
                    textures: g.textures,
                    first_index: g.first_index,
                    index_count: g.index_count,
                    field_size: g.field_size,
                    scales: g.scales,
                });
            }
        }
        Ok(SceneGpu {
            ground_pool,
            grounds,
            vbuf,
            vmem,
            ibuf,
            imem,
            textures,
            pool,
            instance_mem,
            palette_mem,
            half,
            weight_mem,
            buffers: [instance_buf, palette_buf, delta_buf, weight_buf],
            delta_mem,
            inst_pool,
            instance_set,
            sets,
        })
    }

    /// This frame's instances (their model matrices and last frame's).
    pub fn write_instances(&self, gpu: &Gpu, instances: &[Instance]) -> Result<()> {
        gpu.upload(self.instance_mem, bytes(instances))
    }

    /// This frame's joints and last frame's.
    pub fn write_palette(&self, gpu: &Gpu, palette: &[Mat4], prev: &[Mat4]) -> Result<()> {
        gpu.upload(self.palette_mem, bytes(palette))?;
        gpu.upload_at(self.palette_mem, self.half, bytes(prev))
    }

    /// The morph targets' weights.
    pub fn write_weights(&self, gpu: &Gpu, weights: &[f32]) -> Result<()> {
        gpu.upload(self.weight_mem, bytes(weights))
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_pool(self.inst_pool, None);
            if let Some(p) = self.ground_pool {
                d.destroy_descriptor_pool(p, None);
            }
            for (image, mem, view) in self.textures.into_iter().chain(self.grounds.into_iter().flat_map(|g| g.textures)) {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
            for (b, m) in self.buffers.into_iter().zip([self.instance_mem, self.palette_mem, self.delta_mem, self.weight_mem]) {
                d.destroy_buffer(b, None);
                d.free_memory(m, None);
            }
            d.destroy_buffer(self.vbuf, None);
            d.free_memory(self.vmem, None);
            d.destroy_buffer(self.ibuf, None);
            d.free_memory(self.imem, None);
        }
    }
}
