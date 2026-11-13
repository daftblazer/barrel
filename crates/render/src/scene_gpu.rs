//! One scene's resources on the GPU: its geometry and its textures with their descriptor sets.
//! The renderer (device, targets, pipelines, the post chain) outlives scenes; a scene is uploaded
//! when it is loaded and destroyed when it is dropped (`docs/renderer.md`, "Chained events").

use ash::vk;

use crate::{Gpu, Result};

pub struct SceneGpu {
    pub vbuf: vk::Buffer,
    vmem: vk::DeviceMemory,
    pub ibuf: vk::Buffer,
    imem: vk::DeviceMemory,
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pool: vk::DescriptorPool,
    /// One set per texture (layout `set_layout`: one combined image sampler), in texture order.
    pub sets: Vec<vk::DescriptorSet>,
}

impl SceneGpu {
    /// Uploads the vertices and indices (host-visible: one frame does not need a staging copy)
    /// and takes the uploaded textures, giving each a descriptor set through `sampler`.
    pub fn new(
        gpu: &Gpu,
        vertices: &[u8],
        indices: &[u8],
        textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
        set_layout: vk::DescriptorSetLayout,
        sampler: vk::Sampler,
    ) -> Result<SceneGpu> {
        let d = &gpu.device;
        let (vbuf, vmem) = gpu.buffer(vertices.len() as u64, vk::BufferUsageFlags::VERTEX_BUFFER, true)?;
        gpu.upload(vmem, vertices)?;
        let (ibuf, imem) = gpu.buffer(indices.len() as u64, vk::BufferUsageFlags::INDEX_BUFFER, true)?;
        gpu.upload(imem, indices)?;
        let n = textures.len() as u32;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: n }];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)? };
        let layouts = vec![set_layout; textures.len()];
        let sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))? };
        for (set, (_, _, view)) in sets.iter().zip(&textures) {
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(*view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let write = vk::WriteDescriptorSet::default()
                .dst_set(*set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&info);
            unsafe { d.update_descriptor_sets(&[write], &[]) };
        }
        Ok(SceneGpu { vbuf, vmem, ibuf, imem, textures, pool, sets })
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_descriptor_pool(self.pool, None);
            for (image, mem, view) in self.textures {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
            d.destroy_buffer(self.vbuf, None);
            d.free_memory(self.vmem, None);
            d.destroy_buffer(self.ibuf, None);
            d.free_memory(self.imem, None);
        }
    }
}
