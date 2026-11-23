//! One scene's resources on the GPU: its geometry, its textures with their descriptor sets, and
//! its instances (model matrices, joint palette, morph offsets and weights) with their set.
//! The renderer (device, targets, pipelines, the post chain) outlives scenes; a scene is uploaded
//! when it is loaded and destroyed when it is dropped (`docs/renderer.md`, "Chained events").

use ash::vk;

use glam::Mat4;

use crate::ring::{Ring, SLOTS};
use crate::{Gpu, Instance, Result};

fn bytes<T>(s: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

/// A scene as uploaded: the CPU side `main` builds.
pub struct SceneData<'a> {
    pub vertices: &'a [u8],
    pub indices: &'a [u8],
    pub textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    /// Whether each texture is opaque (`upload_textures`; missing: not known, alpha-tested).
    pub opaque: Vec<bool>,
    pub instances: &'a [Instance],
    pub palette: &'a [Mat4],
    pub deltas: &'a [[f32; 4]],
    pub weights: &'a [f32],
    /// Each field's ground: its 8 textures uploaded (2 blend maps, 6 layers).
    pub grounds: Vec<GroundData>,
    /// Each field's light texture (none: `white`'s), one set 1 for each field.
    pub patterns: Vec<Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>>,
    /// Each field's specular environment cube map (`upload_cube`), indexed as `patterns`; none:
    /// the field has no specular ambient term.
    pub specular_cubes: Vec<Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>>,
    /// White's place in `textures`.
    pub white: usize,
}

pub struct GroundData {
    pub field: u16,
    pub textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pub patches: Vec<TerrainPatch>,
    pub cell: f32,
    pub extent: [f32; 2],
}

/// One terrain patch's lit draws (an index range) and its bake list (`bake_entry`).
#[derive(Clone, Debug)]
pub struct TerrainPatch {
    pub first_index: u32,
    pub index_count: u32,
    pub bake: Vec<u16>,
}

/// The most bake entries a patch's push constants hold (10: two a word over five words).
pub const BAKE_MAX: usize = 10;

/// A bake entry: layer (5 bits), map (3), channel (2: 0 red, 1 green, 2 blue) and the layer's
/// repeat in cells (4), as `terrain.frag` unpacks it.
pub fn bake_entry(layer: u8, map: u8, channel: u8, repeat: u8) -> u16 {
    u16::from(layer & 31) | u16::from(map & 7) << 5 | u16::from(channel & 3) << 8 | u16::from(repeat.min(15)) << 10
}

/// A bake list as six words: the count, then two entries a word (the first in the low half).
pub fn bake_words(bake: &[u16]) -> [u32; 6] {
    let mut w = [0u32; 6];
    w[0] = bake.len().min(BAKE_MAX) as u32;
    for (k, &e) in bake.iter().take(BAKE_MAX).enumerate() {
        w[1 + k / 2] |= u32::from(e) << (16 * (k % 2));
    }
    w
}

/// A field's ground as drawn: its texture set (`Shared::terrain_layout`) and its indices.
pub struct Ground {
    pub field: u16,
    pub set: vk::DescriptorSet,
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    pub patches: Vec<TerrainPatch>,
    pub cell: f32,
    pub extent: [f32; 2],
}

/// What a scene's sets point at that the renderer owns: the layouts, the texture sampler, the
/// character lighting buffer (binding 4: each slot's range of the renderer's ring) and the sun's
/// shadow map (binding 5).
pub struct Shared {
    pub set_layout: vk::DescriptorSetLayout,
    pub sampler: vk::Sampler,
    pub instance_layout: vk::DescriptorSetLayout,
    pub look: [vk::DescriptorBufferInfo; SLOTS],
    pub shadow_sampler: vk::Sampler,
    pub shadow_view: vk::ImageView,
    pub terrain_layout: vk::DescriptorSetLayout,
    /// A black cube map, bound where a field has no specular cube map.
    pub black_cube: vk::ImageView,
    /// The renderer's command pool, for the one-off copies of the scene's uploads.
    pub pool: vk::CommandPool,
}

pub struct SceneGpu {
    pub vbuf: vk::Buffer,
    vmem: vk::DeviceMemory,
    pub ibuf: vk::Buffer,
    imem: vk::DeviceMemory,
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    /// Whether each texture is opaque (`SceneData::opaque`): its draws need no alpha test.
    pub opaque: Vec<bool>,
    pool: vk::DescriptorPool,
    /// The per-frame data, one slot per frame in flight: the instances, the joint palette (this
    /// frame's joints, then last frame's at `half`) and the morph weights.
    instances: Ring,
    palette: Ring,
    half: u64,
    /// The palette as last written, kept only while the draw trace is on (`trace_palette`).
    traced: std::sync::Mutex<Vec<Mat4>>,
    weights: Ring,
    delta_buf: vk::Buffer,
    delta_mem: vk::DeviceMemory,
    inst_pool: vk::DescriptorPool,
    /// Set 1, for each slot and each field (slot-major): instances (0), palette (1), morph
    /// offsets (2) and weights (3), lighting (4), shadow (5), the field's light texture (6), its specular cube map (7).
    instance_sets: Vec<vk::DescriptorSet>,
    fields: usize,
    patterns: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    /// Each field's light texture and specular cube map as its sets name them (the white
    /// texture, the black cube map for a field without), kept to write the sets again.
    pattern_views: Vec<vk::ImageView>,
    /// The fields without a light texture of their own: their sets name the white texture.
    pattern_white: Vec<bool>,
    /// White's place in the textures.
    white: usize,
    cube_views: Vec<vk::ImageView>,
    cubes: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
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
        // The geometry never changes: it sits in the device's own memory (through a staging
        // buffer), not in host memory the vertex fetch would read across the bus every pass.
        let local = |b: &[u8], usage| -> Result<(vk::Buffer, vk::DeviceMemory)> {
            let (staging, staging_mem) = gpu.buffer(b.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
            gpu.upload(staging_mem, b)?;
            let (buf, mem) = gpu.buffer(b.len() as u64, usage | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC, false)?;
            gpu.submit(shared.pool, |cb| unsafe { d.cmd_copy_buffer(cb, staging, buf, &[vk::BufferCopy::default().size(b.len() as u64)]) })?;
            unsafe {
                d.destroy_buffer(staging, None);
                d.free_memory(staging_mem, None);
            }
            Ok((buf, mem))
        };
        let (vbuf, vmem) = local(data.vertices, vk::BufferUsageFlags::VERTEX_BUFFER)?;
        let (ibuf, imem) = local(data.indices, vk::BufferUsageFlags::INDEX_BUFFER)?;
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
        let instances = Ring::new(gpu, bytes(data.instances).len() as u64, storage)?;
        instances.write_all(0, bytes(data.instances));
        let half = bytes(data.palette).len() as u64;
        let palette = Ring::new(gpu, (2 * half).max(64), storage)?;
        palette.write_all(0, bytes(data.palette));
        palette.write_all(half, bytes(data.palette));
        let (delta_buf, delta_mem) = host(bytes(data.deltas), storage)?;
        let weights = Ring::new(gpu, bytes(data.weights).len() as u64, storage)?;
        weights.write_all(0, bytes(data.weights));
        let fields = data.patterns.len().max(1);
        let n = (fields * SLOTS) as u32;
        let inst_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 4 * n },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: n },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 3 * n },
        ];
        let inst_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&inst_sizes), None)? };
        let inst_layouts = vec![shared.instance_layout; fields * SLOTS];
        let instance_sets = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(inst_pool).set_layouts(&inst_layouts))? };
        let delta_info = [vk::DescriptorBufferInfo::default().buffer(delta_buf).offset(0).range(vk::WHOLE_SIZE)];
        let image = |sampler, view| [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let shadow_info = image(shared.shadow_sampler, shared.shadow_view);
        let white_view = textures[data.white].2;
        let pattern_views: Vec<vk::ImageView> = (0..fields).map(|k| data.patterns.get(k).and_then(|p| p.as_ref()).map_or(white_view, |p| p.2)).collect();
        let pattern_white: Vec<bool> = (0..fields).map(|k| data.patterns.get(k).is_none_or(|p| p.is_none())).collect();
        let cube_views: Vec<vk::ImageView> = (0..fields).map(|k| data.specular_cubes.get(k).and_then(|p| p.as_ref()).map_or(shared.black_cube, |p| p.2)).collect();
        let pattern_infos: Vec<_> = pattern_views.iter().map(|&v| image(shared.sampler, v)).collect();
        let cube_infos: Vec<_> = cube_views.iter().map(|&v| image(shared.sampler, v)).collect();
        for (k, set) in instance_sets.iter().enumerate() {
            let (slot, pattern_info, cube_info) = (k / fields, &pattern_infos[k % fields], &cube_infos[k % fields]);
            let infos = [[instances.info(slot)], [palette.info(slot)], delta_info, [weights.info(slot)]];
            let look_info = [shared.look[slot]];
            let mut writes: Vec<_> = infos
                .iter()
                .enumerate()
                .map(|(b, info)| vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(b as u32).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(info))
                .collect();
            writes.push(vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&look_info));
            writes.push(vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(5).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&shadow_info));
            writes.push(vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(6).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(pattern_info));
            writes.push(vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(7).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(cube_info));
            unsafe { d.update_descriptor_sets(&writes, &[]) };
        }
        let patterns = data.patterns.into_iter().flatten().collect();
        let cubes = data.specular_cubes.into_iter().flatten().collect();

        let mut ground_pool = None;
        let mut grounds = Vec::new();
        if !data.grounds.is_empty() {
            let n = data.grounds.len() as u32;
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: (crate::TERRAIN_MAPS + 2 * crate::TERRAIN_LAYERS) as u32 * n }];
            let tpool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)? };
            ground_pool = Some(tpool);
            for g in data.grounds {
                let tl = [shared.terrain_layout];
                let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(tpool).set_layouts(&tl))?[0] };
                write_ground_set(d, set, shared.sampler, &g.textures);
                grounds.push(Ground {
                    field: g.field,
                    set,
                    textures: g.textures,
                    patches: g.patches,
                    cell: g.cell,
                    extent: g.extent,
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
            opaque: data.opaque,
            pool,
            instances,
            palette,
            half,
            traced: std::sync::Mutex::new(if crate::trace::enabled() { data.palette.to_vec() } else { Vec::new() }),
            weights,
            delta_buf,
            delta_mem,
            inst_pool,
            instance_sets,
            fields,
            patterns,
            pattern_views,
            pattern_white,
            white: data.white,
            cube_views,
            cubes,
            sets,
        })
    }

    /// The renderer's sampler and shadow map are new (`Renderer::apply_settings`): every set
    /// that names them is written again, with the same textures. Nothing may be in flight.
    pub fn rebind(&self, gpu: &Gpu, shared: &Shared) {
        let d = &gpu.device;
        let image = |sampler, view| [vk::DescriptorImageInfo::default().sampler(sampler).image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        for (set, (_, _, view)) in self.sets.iter().zip(&self.textures) {
            let info = image(shared.sampler, *view);
            let write = vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info);
            unsafe { d.update_descriptor_sets(&[write], &[]) };
        }
        let shadow = image(shared.shadow_sampler, shared.shadow_view);
        for (k, set) in self.instance_sets.iter().enumerate() {
            let field = k % self.fields;
            let (pattern, cube) = (image(shared.sampler, self.pattern_views[field]), image(shared.sampler, self.cube_views[field]));
            let writes = [(5, &shadow), (6, &pattern), (7, &cube)].map(|(b, info)| vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(b).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(info));
            unsafe { d.update_descriptor_sets(&writes, &[]) };
        }
        for g in &self.grounds {
            write_ground_set(d, g.set, shared.sampler, &g.textures);
        }
    }

    /// The geometry from `keep_vertices` bytes of vertices and `keep_indices` bytes of indices
    /// on is replaced by the tails given (what the scene's last part, the field's plants, comes
    /// to for a new setting); what comes before is kept. The device is idle first.
    pub fn replace_geometry_tail(&mut self, gpu: &Gpu, pool: vk::CommandPool, keep_vertices: u64, vertex_tail: &[u8], keep_indices: u64, index_tail: &[u8]) -> Result<()> {
        unsafe { gpu.device.device_wait_idle()? };
        (self.vbuf, self.vmem) = resize_tail(gpu, pool, (self.vbuf, self.vmem), keep_vertices, vertex_tail, vk::BufferUsageFlags::VERTEX_BUFFER)?;
        (self.ibuf, self.imem) = resize_tail(gpu, pool, (self.ibuf, self.imem), keep_indices, index_tail, vk::BufferUsageFlags::INDEX_BUFFER)?;
        Ok(())
    }

    /// The textures uploaded again (same count and order: a new mip bias), their sets written to
    /// name them. The device is idle first.
    pub fn replace_textures(&mut self, gpu: &Gpu, shared: &Shared, textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>, opaque: Vec<bool>) -> Result<()> {
        assert_eq!(textures.len(), self.textures.len());
        let d = &gpu.device;
        unsafe { d.device_wait_idle()? };
        for (image, mem, view) in std::mem::replace(&mut self.textures, textures) {
            unsafe {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
        }
        self.opaque = opaque;
        let white = self.textures[self.white].2;
        for (view, _) in self.pattern_views.iter_mut().zip(&self.pattern_white).filter(|(_, w)| **w) {
            *view = white;
        }
        self.rebind(gpu, shared);
        Ok(())
    }

    /// Set 1 for field `field` (the first's for a scene without fields) in the slot of the
    /// frame being prepared.
    pub fn instance_set(&self, gpu: &Gpu, field: usize) -> vk::DescriptorSet {
        self.instance_sets[gpu.slot() * self.fields + field.min(self.fields - 1)]
    }

    /// This frame's instances (their model matrices and last frame's).
    pub fn write_instances(&self, gpu: &Gpu, instances: &[Instance]) -> Result<()> {
        self.instances.write(gpu, 0, bytes(instances));
        Ok(())
    }

    /// The joint palette the frame draws with, for the draw trace (empty with the trace off).
    pub fn traced_palette(&self) -> std::sync::MutexGuard<'_, Vec<Mat4>> {
        self.traced.lock().unwrap()
    }

    /// This frame's joints and last frame's.
    pub fn write_palette(&self, gpu: &Gpu, palette: &[Mat4], prev: &[Mat4]) -> Result<()> {
        if crate::trace::enabled() {
            *self.traced.lock().unwrap() = palette.to_vec();
        }
        self.palette.write(gpu, 0, bytes(palette));
        self.palette.write(gpu, self.half, bytes(prev));
        Ok(())
    }

    /// The morph targets' weights.
    pub fn write_weights(&self, gpu: &Gpu, weights: &[f32]) -> Result<()> {
        self.weights.write(gpu, 0, bytes(weights));
        Ok(())
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_pool(self.inst_pool, None);
            if let Some(p) = self.ground_pool {
                d.destroy_descriptor_pool(p, None);
            }
            for (image, mem, view) in self.textures.into_iter().chain(self.grounds.into_iter().flat_map(|g| g.textures)).chain(self.patterns).chain(self.cubes) {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
            d.destroy_buffer(self.delta_buf, None);
            d.free_memory(self.delta_mem, None);
            d.destroy_buffer(self.vbuf, None);
            d.free_memory(self.vmem, None);
            d.destroy_buffer(self.ibuf, None);
            d.free_memory(self.imem, None);
        }
        self.instances.destroy(gpu);
        self.palette.destroy(gpu);
        self.weights.destroy(gpu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bake_lists_pack_two_entries_a_word() {
        let e = bake_entry(3, 1, 2, 4);
        assert_eq!(e, 3 | 1 << 5 | 2 << 8 | 4 << 10);
        let w = bake_words(&[e, 7, 9]);
        assert_eq!(w, [3, u32::from(e) | 7 << 16, 9, 0, 0, 0]);
    }
}

/// A ground's set: its blend maps, colour tiles and normal maps, each through `sampler`.
fn write_ground_set(d: &ash::Device, set: vk::DescriptorSet, sampler: vk::Sampler, textures: &[(vk::Image, vk::DeviceMemory, vk::ImageView)]) {
    let info = |k: usize| vk::DescriptorImageInfo::default().sampler(sampler).image_view(textures[k].2).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    let first_normal = crate::TERRAIN_MAPS + crate::TERRAIN_LAYERS;
    let infos: Vec<vk::DescriptorImageInfo> = (0..first_normal).map(info).collect();
    // The normal maps (as many as the layers have, the last repeated for the slots past them).
    let normal_infos: Vec<vk::DescriptorImageInfo> = (0..crate::TERRAIN_LAYERS).map(|i| info((first_normal + i).min(textures.len() - 1))).collect();
    let writes = [
        vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos[..crate::TERRAIN_MAPS]),
        vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos[crate::TERRAIN_MAPS..]),
        vk::WriteDescriptorSet::default().dst_set(set).dst_binding(2).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&normal_infos),
    ];
    unsafe { d.update_descriptor_sets(&writes, &[]) };
}

/// A buffer in the device's memory with its first `keep` bytes kept and `tail` after them.
fn resize_tail(gpu: &Gpu, pool: vk::CommandPool, old: (vk::Buffer, vk::DeviceMemory), keep: u64, tail: &[u8], usage: vk::BufferUsageFlags) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    let d = &gpu.device;
    let (buf, mem) = gpu.buffer((keep + tail.len() as u64).max(16), usage | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC, false)?;
    let staging = if tail.is_empty() {
        None
    } else {
        let (s, sm) = gpu.buffer(tail.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        gpu.upload(sm, tail)?;
        Some((s, sm))
    };
    gpu.submit(pool, |cb| unsafe {
        if keep > 0 {
            d.cmd_copy_buffer(cb, old.0, buf, &[vk::BufferCopy::default().size(keep)]);
        }
        if let Some((s, _)) = staging {
            d.cmd_copy_buffer(cb, s, buf, &[vk::BufferCopy::default().dst_offset(keep).size(tail.len() as u64)]);
        }
    })?;
    unsafe {
        if let Some((s, sm)) = staging {
            d.destroy_buffer(s, None);
            d.free_memory(sm, None);
        }
        d.destroy_buffer(old.0, None);
        d.free_memory(old.1, None);
    }
    Ok((buf, mem))
}
