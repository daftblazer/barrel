//! `barrel-view`: draw one model offscreen with Vulkan and write the frame as PNG.
//!
//!     barrel-view MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png [--size N] [--yaw DEG]
//!     barrel-view --field FIELD_DIR OUT.png [--size N] [--yaw DEG]
//!
//! The second form draws every object a field's placement list puts down, each with its
//! model from the field's `_Obj` container and the field's shared textures.
//!
//! It renders a single frame and exits: no window, no loop. Set BARREL_VALIDATION=1 to
//! enable the Khronos validation layer.

use std::ffi::CStr;
use std::path::{Path, PathBuf};

use ash::vk;
use barrel_formats::elixir::Elixir;
use barrel_formats::g1m::G1m;
use barrel_formats::g1t::{Format, G1t};
use glam::{Mat4, Vec3};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod fsr;
mod glow;
mod fsr3;
mod gsr2;
mod present;
mod settings;
mod shadow;
mod taa;
mod timing;
mod text;
mod upscale;
mod window;

const VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.vert.spv"));
const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.frag.spv"));
const TERRAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/terrain.frag.spv"));
const SHADOW_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.frag.spv"));
/// Push constants: mvp, params (alpha cutoff, field size), two vec4 of layer tile scales.
/// view_proj (64), params, scale_a, scale_b (48), then the draw's instance number (4).
const PUSH_SIZE: u32 = 124;
const PUSH_INSTANCE: u32 = 112;
/// The field look's uniform block (see `model.frag`): 6 vec4s, 4 point lights × 4 vec4s, then
/// the field's sun, fill and ambient (5 vec4s).
const LOOK_SIZE: u64 = 48 * 16;
/// Half the side of the sun's shadow map square, in field units (calibrated against oracle
/// frames; docs/renderer.md, "Shadows").
const SHADOW_HALF: f32 = 1500.0;
const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
/// The scene's colour target: 8 bits a channel, as the game's (it draws at the HDR range's
/// inverse and doubles the result after: `docs/shading.md`, "After the scene").
const SCENE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
/// The motion vectors' format (`--motion`): x and y in texture coordinates.
const MOTION_FORMAT: vk::Format = vk::Format::R16G16_SFLOAT;
/// HdrRangeInv: the scene is drawn at half its brightness and doubled by the post pass.
const HDR_RANGE_INV: f32 = 0.5;
const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
const POST_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.frag.spv"));

/// `FILE#N` is entry N of a container; a plain path is a loose file (Sophie 2 keeps its
/// texture packs beside the model containers).
fn entry_bytes(spec: &str) -> Result<Vec<u8>> {
    let Some((file, n)) = spec.rsplit_once('#') else {
        return Ok(std::fs::read(spec)?);
    };
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
    /// Up to four joints of the model's skeleton (indices into its instance's palette) and
    /// their weights out of 255; all weights 0 for geometry that is not skinned.
    joints: [u16; 4],
    weights: [u8; 4],
    /// Facial morph targets: where this vertex's offsets begin in the delta buffer, then
    /// (target count | weight offset << 16) into its instance's weights; 0 targets: none.
    morph: [u32; 2],
    /// The vertex colour (field models bake their light into it); white when it has none.
    color: [u8; 4],
    /// A billboard corner (`MapSTD-B*`, PSIZE): the vertex is its quad's centre and this
    /// offset, already scaled with the model, turns to face the camera; zero otherwise.
    corner: [f32; 3],
}

/// A model's placement on the GPU (std430: 80 bytes): its matrix and where its joints'
/// matrices start in the palette.
#[repr(C)]
#[derive(Clone, Copy)]
struct Instance {
    model: Mat4,
    palette: u32,
    /// Where its morph weights begin in the weight buffer.
    weights: u32,
    /// Where its last frame's joints begin in the palette buffer (the palette's second half),
    /// for the motion vectors.
    prev_palette: u32,
    _pad: u32,
    /// Its last frame's model matrix, for the motion vectors.
    prev_model: Mat4,
}

struct Draw {
    first_index: u32,
    index_count: u32,
    texture: usize,
    /// A texture to lay over `texture` by its alpha (the iris over the eye white).
    overlay: Option<usize>,
    /// Which of [`Scene::instances`] places it.
    instance: u32,
    /// The material's mask (`docs/shading.md`, "Which texture is which"), if it has one; for
    /// an eye, its highlights.
    mask: Option<usize>,
    /// Shaded as an eye (`CharaEYE`: `docs/shading.md`, "Eyes").
    eye: bool,
    /// Part of a field's distant view (unlit: its texture on its vertex colour).
    sky: bool,
    /// The outline pass's colour (the material's `atOutCol`, grey), when the material has an
    /// outline (an `atEdge` parameter).
    outline: Option<f32>,
    /// The field (in the order the scene loaded them) it belongs to; None for what is drawn
    /// in every field (the event's characters).
    field: Option<u16>,
    /// Drawn into the sun's shadow map: the characters, and the field objects marked `xDS_`
    /// (in their name or their placement's tags; docs/renderer.md, "Shadows").
    casts: bool,
    /// What it is part of, for `BARREL_HIDE` (a field object's placement name, `sky:<model>`
    /// for the distant view, empty for characters).
    name: String,
    /// The version a field object belongs to (its placement tag `xAM_…_ver<N>`): drawn only
    /// while the event shows that version (`q15`, `Stage::versions_shown`).
    version: Option<u32>,
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
    /// A device for offscreen work (`display` None) or for presenting to a window on
    /// `display` (adds the surface and swapchain extensions).
    fn new(display: Option<raw_window_handle::RawDisplayHandle>) -> Result<Gpu> {
        let entry = unsafe { ash::Entry::load()? };
        let validation = std::env::var("BARREL_VALIDATION").is_ok_and(|v| v == "1");
        let layers: Vec<*const i8> = if validation { vec![c"VK_LAYER_KHRONOS_validation".as_ptr()] } else { vec![] };
        let app = vk::ApplicationInfo::default().application_name(c"barrel-view").api_version(vk::API_VERSION_1_1);
        let instance = unsafe {
            let extensions = match display {
                Some(d) => present::instance_extensions(d)?,
                None => Vec::new(),
            };
            entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(&app).enabled_layer_names(&layers).enabled_extension_names(&extensions),
                None,
            )?
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
        // Independent blending where there is one (desktop, Turnip and Mali all have it): the
        // motion target is written unblended beside a blended colour.
        // FSR 3.1 writes its output through format-less storage images.
        let enabled = vk::PhysicalDeviceFeatures::default()
            .texture_compression_bc(true)
            .independent_blend(features.independent_blend != 0)
            .shader_storage_image_write_without_format(features.shader_storage_image_write_without_format != 0)
            .shader_storage_image_read_without_format(features.shader_storage_image_read_without_format != 0);
        let swapchain_ext = [ash::khr::swapchain::NAME.as_ptr()];
        let device = unsafe {
            instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_features(&enabled)
                    .enabled_extension_names(if display.is_some() { &swapchain_ext[..] } else { &[] }),
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
        self.upload_at(mem, 0, bytes)
    }

    fn upload_at(&self, mem: vk::DeviceMemory, offset: u64, bytes: &[u8]) -> Result<()> {
        unsafe {
            let p = self.device.map_memory(mem, offset, bytes.len() as u64, vk::MemoryMapFlags::empty())? as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            self.device.unmap_memory(mem);
        }
        Ok(())
    }

    fn image(&self, info: &vk::ImageCreateInfo) -> Result<(vk::Image, vk::DeviceMemory)> {
        let d = &self.device;
        let image = unsafe { d.create_image(info, None)? };
        let req = unsafe { d.get_image_memory_requirements(image) };
        // A transient attachment (one whose contents never leave the render pass) takes lazily
        // allocated memory where the driver has it: on tile-based GPUs (Turnip, Mali) it then
        // lives in tile memory only and costs no real memory. Elsewhere, ordinary device memory.
        let lazy = vk::MemoryPropertyFlags::DEVICE_LOCAL | vk::MemoryPropertyFlags::LAZILY_ALLOCATED;
        let index = if info.usage.contains(vk::ImageUsageFlags::TRANSIENT_ATTACHMENT) {
            self.memory_type(req.memory_type_bits, lazy).or_else(|_| self.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL))?
        } else {
            self.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?
        };
        let mem = unsafe { d.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(index), None)? };
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

/// The terrain on the GPU: one pipeline, and each field's ground with its set of textures.
struct TerrainGpu {
    layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    pipeline_layout: vk::PipelineLayout,
    shader: vk::ShaderModule,
    pipeline: vk::Pipeline,
    grounds: Vec<Ground>,
}

struct Ground {
    field: u16,
    set: vk::DescriptorSet,
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    first_index: u32,
    index_count: u32,
    field_size: f32,
    scales: [f32; 6],
}

/// The field's ground: a heightmap grid drawn with the terrain shader.
struct TerrainDraw {
    first_index: u32,
    index_count: u32,
    /// The field's width in world units (the blend maps span it).
    field_size: f32,
    /// Tile scale (1/world units) of each of the six layers the blend maps weight.
    scales: [f32; 6],
    /// The terrain bundle, for its layer and blend textures.
    ktf: Vec<u8>,
}

struct Scene {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    draws: Vec<Draw>,
    /// Each field's ground, by field.
    terrains: Vec<(u16, TerrainDraw)>,
    /// The field geometry added now belongs to (see [`Draw::field`]).
    field: Option<u16>,
    lo: Vec3,
    hi: Vec3,
    /// The next model's facial morph sets by vertex layout: each target's offset per vertex,
    /// and the set's starting weights (from its facial animations).
    morph: std::collections::HashMap<usize, (Vec<Vec<[f32; 3]>>, Vec<f32>)>,
    /// Every morphed vertex's target offsets (vertex by vertex, target by target), for the
    /// GPU; never empty.
    deltas: Vec<[f32; 4]>,
    /// Every morphed model's target weights, one block per instance; never empty.
    morph_weights: Vec<f32>,
    /// Model matrices in render space (x mirrored), one per movable model; 0 is the identity,
    /// for geometry baked in place. The renderer uploads them to a storage buffer the vertex
    /// shader reads, so a model moves by changing its matrix, not its vertices.
    instances: Vec<Instance>,
    /// Every skinned model's joint matrices (bind pose to posed, render space), one after
    /// another; an instance's `palette` is where its own begin. Never empty (the GPU buffer
    /// needs a size), so it starts with one identity.
    palette: Vec<Mat4>,
}

impl Scene {
    fn new() -> Scene {
        Scene {
            morph: Default::default(),
            deltas: vec![[0.0; 4]],
            morph_weights: vec![0.0],
            vertices: Vec::new(),
            indices: Vec::new(),
            draws: Vec::new(),
            terrains: Vec::new(),
            field: None,
            instances: vec![Instance { model: Mat4::IDENTITY, palette: 0, weights: 0, prev_palette: 0, _pad: 0, prev_model: Mat4::IDENTITY }],
            palette: vec![Mat4::IDENTITY],
            lo: Vec3::splat(f32::MAX),
            hi: Vec3::splat(f32::MIN),
        }
    }

    /// Add a model's textured submeshes, placed by `transform`.
    fn add(&mut self, geometry: &barrel_formats::g1m::Geometry, transform: Mat4, textures: usize) -> Result<()> {
        self.add_posed(geometry, transform, textures, None, 0)
    }

    /// A new instance placed by `transform` (game space); returns its number.
    fn instance(&mut self, transform: Mat4) -> u32 {
        self.instances.push(Instance { model: render_space(transform), palette: 0, weights: 0, prev_palette: 0, _pad: 0, prev_model: render_space(transform) });
        (self.instances.len() - 1) as u32
    }

    /// Add a movable model (its own instance, placed by `transform`) whose texture pack starts
    /// at `base` in the scene's texture list, skinned by `joints` if given. Returns its instance.
    fn add_with_pack(&mut self, geometry: &barrel_formats::g1m::Geometry, transform: Mat4, base: usize, count: usize, joints: Option<&[Mat4]>) -> Result<u32> {
        let first = self.draws.len();
        let instance = self.instance(transform);
        self.add_posed(geometry, Mat4::IDENTITY, count, joints, instance)?;
        for d in &mut self.draws[first..] {
            d.texture += base;
            d.overlay = d.overlay.map(|o| o + base);
            d.mask = d.mask.map(|m| m + base);
        }
        Ok(instance)
    }

    /// Add a model, skinned by `joints` (one matrix per joint, bind pose to posed) if given.
    /// `transform` is baked into the vertices; `instance` places them at draw time.
    fn add_posed(&mut self, geometry: &barrel_formats::g1m::Geometry, transform: Mat4, textures: usize, joints: Option<&[Mat4]>, instance: u32) -> Result<()> {
        let placed = self.instances[instance as usize].model;
        // A posed model's joints go to the palette, in render space (S·M·S like placements),
        // and the GPU skins its vertices; baked geometry (instance 0) must not be posed.
        if let Some(joints) = joints {
            if instance == 0 {
                return Err("a posed model needs an instance of its own".into());
            }
            self.instances[instance as usize].palette = self.palette.len() as u32;
            self.palette.extend(joints.iter().map(|&m| render_space(m)));
        }
        // The model's morph sets: their weights go to the instance's block, each set at an
        // offset in it (sorted by layout so the order is fixed).
        let mut sets: Vec<(usize, u32)> = Vec::new();
        if !self.morph.is_empty() {
            if instance == 0 {
                return Err("a morphed model needs an instance of its own".into());
            }
            self.instances[instance as usize].weights = self.morph_weights.len() as u32;
            let mut layouts: Vec<usize> = self.morph.keys().copied().collect();
            layouts.sort();
            let mut offset = 0u32;
            for l in layouts {
                sets.push((l, offset));
                let w = &self.morph[&l].1;
                self.morph_weights.extend(w);
                offset += w.len() as u32;
            }
        }
        let normals = Mat4::from_mat3(glam::Mat3::from_mat4(transform).inverse().transpose());
        for s in &geometry.submeshes {
            let mesh = geometry.simple_mesh(s)?;
            if mesh.uvs.is_empty() || mesh.normals.is_empty() {
                continue;
            }
            let base = self.vertices.len() as u32;
            let skin = match joints {
                Some(_) => geometry.skin(s)?,
                None => None,
            };
            // The games are left-handed (DirectX): mirror x to draw them right-handed.
            let mirror = Vec3::new(-1.0, 1.0, 1.0);
            let morph = self.morph.get(&s.layout).map(|(t, _)| t);
            let weight_offset = sets.iter().find(|(l, _)| *l == s.layout).map_or(0, |&(_, o)| o);
            for i in 0..mesh.positions.len() {
                let (pos, nrm) = (Vec3::from(mesh.positions[i]), Vec3::from(mesh.normals[i]));
                // This vertex's offset in each target, for the GPU to weight.
                let mut morph_ref = [0u32; 2];
                if let Some(targets) = morph {
                    let v = mesh.first_vertex + i;
                    if targets.iter().all(|t| v < t.len()) {
                        morph_ref = [self.deltas.len() as u32, targets.len() as u32 | weight_offset << 16];
                        self.deltas.extend(targets.iter().map(|t| [t[v][0], t[v][1], t[v][2], 0.0]));
                    }
                }
                // The joints and weights, for the GPU (joints past the pose's count or weights of
                // 0 are dropped); the bounds use the CPU's own skinning of the point.
                let (mut joint_ix, mut weight_q, mut skinned) = ([0u16; 4], [0u8; 4], pos);
                if let (Some(skin), Some(joints)) = (&skin, joints) {
                    let (j, w) = skin[i];
                    let total: f32 = (0..4).filter(|&k| w[k] > 0.0 && (j[k] as usize) < joints.len()).map(|k| w[k]).sum();
                    if total > 0.0 {
                        skinned = Vec3::ZERO;
                        for k in 0..4 {
                            if w[k] > 0.0 && (j[k] as usize) < joints.len() {
                                joint_ix[k] = j[k] as u16;
                                weight_q[k] = (w[k] / total * 255.0).round() as u8;
                                skinned += joints[j[k] as usize].transform_point3(pos) * (w[k] / total);
                            }
                        }
                    }
                }
                let p = transform.transform_point3(pos) * mirror;
                let n = normals.transform_vector3(nrm).normalize_or_zero() * mirror;
                let w = placed.transform_point3(transform.transform_point3(skinned) * mirror);
                self.lo = self.lo.min(w);
                self.hi = self.hi.max(w);
                let color = mesh.colors.get(i).map_or([255; 4], |c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
                // The game's mBbL2W: the camera's axes times the model's scale.
                let corner = mesh.corners.get(i).map_or([0.0; 3], |c| (Vec3::from(*c) * transform.x_axis.truncate().length()).into());
                // A billboard's normal stays in its own frame, for the camera's axes to turn.
                let n = if corner == [0.0; 3] { n } else { nrm };
                self.vertices.push(Vertex { pos: p.into(), normal: n.into(), uv: mesh.uvs[i], joints: joint_ix, weights: weight_q, morph: morph_ref, color, corner });
            }
            let first_index = self.indices.len() as u32;
            for t in &mesh.triangles {
                self.indices.extend(t.iter().map(|&v| v + base));
            }
            let material = geometry.materials.get(mesh.material);
            let texture = material
                .and_then(|m| m.textures.iter().find(|t| t.kind == 1).or(m.textures.first()))
                .map_or(0, |t| t.index as usize)
                .min(textures.saturating_sub(1));
            // Eye materials: four or more textures, none of subtype 1. The first is the eye
            // white, the second the iris with alpha, which the game's shader lays on top (the
            // rest, a glow and highlights, are left out here).
            let overlay = material
                .filter(|m| m.textures.len() >= 4 && m.textures.iter().all(|t| t.subtype != 1))
                .map(|m| (m.textures[1].index as usize).min(textures.saturating_sub(1)));
            // The mask: the first subtype-0 texture after the colour.
            // An eye's highlights are its fourth texture.
            let eye = overlay.is_some();
            let mask = if eye {
                material.and_then(|m| m.textures.get(3)).map(|t| (t.index as usize).min(textures.saturating_sub(1)))
            } else {
                material.and_then(|m| {
                    let at = m.textures.iter().position(|t| t.index as usize == texture)?;
                    m.textures[at + 1..].iter().find(|t| t.subtype == 0).map(|t| (t.index as usize).min(textures.saturating_sub(1)))
                })
            };
            let outline = geometry.param(s, "atEdge").and(Some(geometry.param(s, "atOutCol").and_then(|c| c.first().copied()).unwrap_or(1.0)));
            self.draws.push(Draw { first_index, index_count: self.indices.len() as u32 - first_index, texture, overlay, instance, mask, eye, sky: false, outline, field: self.field, casts: true, name: String::new(), version: None });
        }
        Ok(())
    }
}

impl Scene {
    /// A heightmap as a grid of quads; UVs run 0..1 across the field. Returns the index range.
    fn add_heightmap(&mut self, h: &barrel_formats::khm::Heightmap) -> (u32, u32) {
        let mirror = Vec3::new(-1.0, 1.0, 1.0);
        let base = self.vertices.len() as u32;
        let w = h.cells_x + 1;
        for z in 0..=h.cells_z {
            for x in 0..=h.cells_x {
                let p = Vec3::new(x as f32 * h.cell_size, h.height(x, z), z as f32 * h.cell_size) * mirror;
                let hx = |dx: isize| h.height((x as isize + dx).clamp(0, h.cells_x as isize) as usize, z);
                let hz = |dz: isize| h.height(x, (z as isize + dz).clamp(0, h.cells_z as isize) as usize);
                let n = Vec3::new(hx(-1) - hx(1), 2.0 * h.cell_size, hz(-1) - hz(1)).normalize() * mirror;
                self.lo = self.lo.min(p);
                self.hi = self.hi.max(p);
                let uv = [x as f32 / h.cells_x as f32, z as f32 / h.cells_z as f32];
                self.vertices.push(Vertex { pos: p.into(), normal: n.into(), uv, joints: [0; 4], weights: [0; 4], morph: [0; 2], color: [255; 4], corner: [0.0; 3] });
            }
        }
        let first_index = self.indices.len() as u32;
        for z in 0..h.cells_z {
            for x in 0..h.cells_x {
                let i = base + (z * w + x) as u32;
                let w = w as u32;
                self.indices.extend([i, i + 1, i + w, i + 1, i + w + 1, i + w]);
            }
        }
        (first_index, self.indices.len() as u32 - first_index)
    }
}

/// What a frame draws into besides the colour image: a depth image at the target's size and
/// a framebuffer per colour image (one offscreen, or one per swapchain image).
/// What the frames draw into, remade at a new size: the scene's colour (sampled by the post
/// pass) and depth with their framebuffer, and one post framebuffer per output image (the
/// offscreen image, or each swapchain image).
struct Targets {
    scene: vk::Image,
    scene_memory: vk::DeviceMemory,
    scene_view: vk::ImageView,
    depth: vk::Image,
    depth_memory: vk::DeviceMemory,
    depth_view: vk::ImageView,
    scene_framebuffer: vk::Framebuffer,
    framebuffers: Vec<vk::Framebuffer>,
    glow: Option<glow::Targets>,
    /// FSR 1, when it is on and the scene is drawn below the output's size.
    upscale: Option<upscale::Targets>,
    /// TAA (`--taa`), and whether its (or GSR 2's) history is still empty (new targets: the
    /// first frame; a cut).
    taa: Option<taa::Targets>,
    /// GSR 2 (`--gsr2`): the temporal upscaler.
    gsr2: Option<gsr2::Targets>,
    /// FSR 3.1 (`--fsr3`): the desktop's temporal upscaler (its state changes as it records).
    fsr3: std::cell::RefCell<Option<fsr3::Targets>>,
    history_fresh: std::cell::Cell<bool>,
    /// The scene as it was when a cross-fade began (`u13` kind 3), faded out over the live one;
    /// whether it has been given a layout yet.
    frozen: vk::Image,
    frozen_memory: vk::DeviceMemory,
    frozen_view: vk::ImageView,
    frozen_ready: bool,
    /// The scene's own size: the output's times the render scale (`--scale`); the post pass
    /// scales it to the output, which keeps the text sharp.
    scene_size: (u32, u32),
    /// The multisampled colour (`--msaa`), resolved into `scene` by the scene pass.
    multisample: Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    /// The motion vectors (`--motion`).
    motion: Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
}

impl Targets {
    #[allow(clippy::too_many_arguments)]
    fn new(
        gpu: &Gpu,
        scene_pass: vk::RenderPass,
        post_pass: vk::RenderPass,
        output: &[vk::ImageView],
        width: u32,
        height: u32,
        scene_size: (u32, u32),
        post_set: vk::DescriptorSet,
        sampler: vk::Sampler,
        glow_pipelines: &glow::Pipelines,
        upscale_pipelines: Option<&upscale::Pipelines>,
        taa_pipelines: Option<&taa::Pipelines>,
        gsr2_pipelines: Option<&gsr2::Pipelines>,
        fsr3_pipelines: Option<&fsr3::Pipelines>,
        samples: vk::SampleCountFlags,
        motion: bool,
        show_motion: bool,
    ) -> Result<Targets> {
        let d = &gpu.device;
        let (scene_width, scene_height) = scene_size;
        let image_with = |format, usage, samples| {
            gpu.image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(vk::Extent3D { width: scene_width, height: scene_height, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(samples)
                    .usage(usage),
            )
        };
        let image = |format, usage| image_with(format, usage, vk::SampleCountFlags::TYPE_1);
        let view = |image, format, aspect| unsafe {
            d.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(1).layer_count(1)),
                None,
            )
        };
        let (scene, scene_memory) =
            image(SCENE_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)?;
        let (frozen, frozen_memory) = image(SCENE_FORMAT, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)?;
        let frozen_view = view(frozen, SCENE_FORMAT, vk::ImageAspectFlags::COLOR)?;
        let scene_view = view(scene, SCENE_FORMAT, vk::ImageAspectFlags::COLOR)?;
        // Depth is cleared and never stored (the scene pass): transient.
        // Depth is transient unless GSR 2 reads it (then the scene pass stores it).
        let depth_usage = if gsr2_pipelines.is_some() || fsr3_pipelines.is_some() { vk::ImageUsageFlags::SAMPLED } else { vk::ImageUsageFlags::TRANSIENT_ATTACHMENT };
        let (depth, depth_memory) = image_with(DEPTH_FORMAT, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | depth_usage, samples)?;
        // With samples, the multisampled colour the pass resolves into the scene: transient too.
        let multisample = if samples != vk::SampleCountFlags::TYPE_1 {
            let (image, memory) = image_with(SCENE_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT, samples)?;
            Some((image, memory, view(image, SCENE_FORMAT, vk::ImageAspectFlags::COLOR)?))
        } else {
            None
        };
        let depth_view = view(depth, DEPTH_FORMAT, vk::ImageAspectFlags::DEPTH)?;
        let framebuffer = |pass, views: &[vk::ImageView], (w, h): (u32, u32)| unsafe {
            d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(pass).attachments(views).width(w).height(h).layers(1), None)
        };
        // The motion vectors (`--motion`), read by the temporal methods (and `--show-motion`).
        let motion_target = if motion && multisample.is_none() {
            let (image, memory) = image(MOTION_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)?;
            Some((image, memory, view(image, MOTION_FORMAT, vk::ImageAspectFlags::COLOR)?))
        } else {
            None
        };
        let scene_framebuffer = match (multisample, motion_target) {
            (Some((_, _, msaa_view)), _) => framebuffer(scene_pass, &[msaa_view, depth_view, scene_view], scene_size)?,
            (None, Some((_, _, motion_view))) => framebuffer(scene_pass, &[scene_view, depth_view, motion_view], scene_size)?,
            (None, None) => framebuffer(scene_pass, &[scene_view, depth_view], scene_size)?,
        };
        let framebuffers = output.iter().map(|&v| framebuffer(post_pass, &[v], (width, height))).collect::<std::result::Result<Vec<_>, _>>()?;
        if framebuffers.is_empty() {
            return Err("no target to draw into".into());
        }
        let glow = glow::Targets::new(gpu, glow_pipelines, scene_view, sampler, scene_width, scene_height)?;
        // TAA resolves the jittered scene (with its motion vectors) before anything scales it.
        let taa = match (taa_pipelines, motion_target) {
            (Some(p), Some((_, _, motion_view))) => Some(taa::Targets::new(gpu, p, scene_view, motion_view, sampler, scene_size)?),
            _ => None,
        };
        let resolved = taa.as_ref().map_or(scene_view, taa::Targets::result);
        // GSR 2 upscales the jittered scene with its depth and motion vectors.
        let gsr2 = match (gsr2_pipelines, motion_target) {
            (Some(p), Some((_, _, motion_view))) => Some(gsr2::Targets::new(gpu, p, scene_view, depth_view, motion_view, sampler, scene_size, (width, height))?),
            _ => None,
        };
        let resolved = gsr2.as_ref().map_or(resolved, gsr2::Targets::result);
        // FSR 3.1 likewise.
        let fsr3 = match (fsr3_pipelines, motion_target) {
            (Some(p), Some((_, _, motion_view))) => Some(fsr3::Targets::new(gpu, p, scene_view, depth_view, motion_view, scene_size, (width, height))?),
            _ => None,
        };
        let resolved = fsr3.as_ref().map_or(resolved, fsr3::Targets::result);
        let upscale = match upscale_pipelines {
            // FSR 1 when the scene is smaller; RCAS alone (`--sharpen`) when it is not.
            Some(p) => Some(upscale::Targets::new(gpu, p, resolved, sampler, scene_size, (width, height))?),
            _ => None,
        };
        // The post pass samples the new scene image (TAA's, then FSR's when they run) and the glow.
        let shown = match (show_motion, motion_target) {
            (true, Some((_, _, motion_view))) => motion_view,
            _ => upscale.as_ref().map_or(resolved, upscale::Targets::result),
        };
        let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(shown).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let glow_info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(glow.result()).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let frozen_info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(frozen_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        unsafe {
            d.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::default().dst_set(post_set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info),
                    vk::WriteDescriptorSet::default().dst_set(post_set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&glow_info),
                    vk::WriteDescriptorSet::default().dst_set(post_set).dst_binding(2).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&frozen_info),
                ],
                &[],
            )
        };
        Ok(Targets {
            motion: motion_target,
            multisample,
            scene_size,
            scene,
            scene_memory,
            scene_view,
            depth,
            depth_memory,
            depth_view,
            scene_framebuffer,
            framebuffers,
            glow: Some(glow),
            upscale,
            taa,
            gsr2,
            fsr3: std::cell::RefCell::new(fsr3),
            history_fresh: std::cell::Cell::new(true),
            frozen,
            frozen_memory,
            frozen_view,
            frozen_ready: false,
        })
    }

    fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for fb in self.framebuffers.drain(..) {
                d.destroy_framebuffer(fb, None);
            }
            d.destroy_framebuffer(self.scene_framebuffer, None);
            d.destroy_image_view(self.frozen_view, None);
            d.destroy_image(self.frozen, None);
            d.free_memory(self.frozen_memory, None);
            if let Some(g) = self.glow.take() {
                g.destroy(gpu);
            }
            if let Some(t) = self.taa.take() {
                t.destroy(gpu);
            }
            if let Some(g) = self.gsr2.take() {
                g.destroy(gpu);
            }
            if let Some(f) = self.fsr3.get_mut().take() {
                f.destroy(gpu);
            }
            if let Some(u) = self.upscale.take() {
                u.destroy(gpu);
            }
            d.destroy_image_view(self.scene_view, None);
            d.destroy_image(self.scene, None);
            d.free_memory(self.scene_memory, None);
            if let Some((image, memory, view)) = self.motion.take() {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(memory, None);
            }
            if let Some((image, memory, view)) = self.multisample.take() {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(memory, None);
            }
            d.destroy_image_view(self.depth_view, None);
            d.destroy_image(self.depth, None);
            d.free_memory(self.depth_memory, None);
        }
    }
}

/// A game-space placement as a render-space matrix: the vertices are mirrored in x as they are
/// built, so a placement T becomes S·T·S with S the x mirror (a rotation stays a rotation).
fn render_space(transform: Mat4) -> Mat4 {
    let s = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    s * transform * s
}

/// Each joint's skinning matrix for an animation at `time`: its posed world transform times
/// the inverse of its bind world transform. Joints without a track keep their bind pose.
fn pose(skeleton: &barrel_formats::g1m::Skeleton, anim: &barrel_formats::g1a::Animation, time: f32) -> Vec<Mat4> {
    skin(skeleton, &local_pose(skeleton, anim, time))
}

/// A joint's local transform: scale, rotation, translation.
type Trs = (Vec3, glam::Quat, Vec3);

/// Each joint's local transform for an animation at `time`. Joints without a track keep their
/// bind pose; rotation-only tracks keep the joint's bind translation and scale.
fn local_pose(skeleton: &barrel_formats::g1m::Skeleton, anim: &barrel_formats::g1a::Animation, time: f32) -> Vec<Trs> {
    let bind = |j: &barrel_formats::g1m::Joint| (Vec3::from(j.scale), glam::Quat::from_array(j.rotation).normalize(), Vec3::from(j.position));
    let mut out: Vec<Trs> = skeleton.joints.iter().map(bind).collect();
    for t in &anim.tracks {
        let Some(slot) = out.get_mut(t.bone as usize) else { continue };
        if let Some((r, p, s)) = t.pose(time) {
            *slot = (Vec3::from(s), glam::Quat::from_array(r).normalize(), Vec3::from(p));
        } else if let Some(r) = t.rotation(time) {
            slot.1 = glam::Quat::from_array(r).normalize();
        }
    }
    out
}

/// Two local poses mixed: `w` of the way from `a` to `b` (rotations by slerp).
fn mix_pose(a: &[Trs], b: &[Trs], w: f32) -> Vec<Trs> {
    a.iter().zip(b).map(|(x, y)| (x.0.lerp(y.0, w), x.1.slerp(y.1, w), x.2.lerp(y.2, w))).collect()
}

/// Skinning matrices for local joint transforms: each joint's posed world transform times the
/// inverse of its bind world transform.
fn skin(skeleton: &barrel_formats::g1m::Skeleton, locals: &[Trs]) -> Vec<Mat4> {
    let local = |j: &barrel_formats::g1m::Joint| {
        Mat4::from_scale_rotation_translation(Vec3::from(j.scale), glam::Quat::from_array(j.rotation).normalize(), Vec3::from(j.position))
    };
    let posed_local: Vec<Mat4> = locals.iter().map(|&(s, r, t)| Mat4::from_scale_rotation_translation(s, r, t)).collect();
    let world = |locals: &[Mat4]| {
        let mut out = vec![Mat4::IDENTITY; locals.len()];
        for i in skeleton.order() {
            out[i] = match skeleton.joints[i].parent {
                Some(p) => out[p] * locals[i],
                None => locals[i],
            };
        }
        out
    };
    let bind_local: Vec<Mat4> = skeleton.joints.iter().map(local).collect();
    let (bind, posed) = (world(&bind_local), world(&posed_local));
    bind.iter().zip(&posed).map(|(b, p)| *p * b.inverse()).collect()
}

/// Place the characters an event adds, at their first position, in the scene. Returns the
/// texture packs it used, appended after the `base` textures already in the scene.
/// An event's camera: where it looks, from how far, at which angles (degrees) and field of
/// view. From `t20/0x10` (0x74100014), all in hundredths: {yaw°, angle°, pitch°, angle°,
/// distance, target x, y, z, offset x, y, z}; the camera looks at target + offset.
#[derive(Clone, Copy, Debug)]
struct EventCamera {
    /// Where the eye is, when known outright (the stage's camera, `--at`); otherwise it is
    /// worked out from distance, yaw and pitch.
    eye: Option<Vec3>,
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    fov: f32,
}

/// A character the frame loop moves: its instance, its skeleton, where its motions come
/// from, and the motion it plays (its `CharaBase` column) and how far into it it is at the
/// start (`--at`).
struct Actor {
    who: u32,
    instance: u32,
    skeleton: barrel_formats::g1m::Skeleton,
    motions: Motions,
    column: String,
    anim: barrel_formats::g1a::Animation,
    start: f32,
    looping: bool,
    /// The motion being blended out: it, its time at the switch, whether it loops.
    previous: Option<(barrel_formats::g1a::Animation, f32, bool)>,
    face: Option<FaceRig>,
    /// The time shown at the last frame, in the current motion.
    last_t: f32,
    /// The head gestures' clip and the joints it turns (`s116`).
    head: Option<(barrel_formats::g1a::Animation, Vec<usize>)>,
    /// The division of the motion it plays (`s5`, `s11`), when one is set and found.
    division: Option<barrel_formats::chara_div::Division>,
    /// The motion whose division was last logged.
    last_division_column: String,
}

/// What a character's face needs to change during playback: its rows, its facial
/// animations (loaded by name, kept), and its morph sets' places in the weight buffer.
struct FaceRig {
    row: u32,
    head: String,
    facials: Vec<u8>,
    /// Per morph set (sorted by layout, as the weights are laid out): its target count and
    /// where its weights begin in the instance's block.
    sets: Vec<(usize, u32)>,
    loaded: std::collections::HashMap<String, Option<barrel_formats::g1a::Animation>>,
}

impl FaceRig {
    fn anim(&mut self, name: &str) -> Option<&barrel_formats::g1a::Animation> {
        if !self.loaded.contains_key(name) {
            let anim = Elixir::parse(&self.facials)
                .ok()
                .and_then(|c| named(&c, &format!("{name}.g1a")).map(|b| b.to_vec()))
                .and_then(|b| barrel_formats::g1a::parse(&b).ok());
            self.loaded.insert(name.to_string(), anim);
        }
        self.loaded.get(name).and_then(Option::as_ref)
    }

    /// The weights for a face (brow and eyes held at 0.5 s; the mouth at its end, or moving
    /// over `time` while the character talks), into `block` (the instance's weights).
    fn weights(&mut self, face: &barrel_game::face::Face, talking: bool, time: f32, block: &mut [f32]) {
        block.fill(0.0);
        for (name, t) in [(&face.brow, Some(0.5)), (&face.eye, Some(0.5)), (&face.mouth, None)] {
            let Some(name) = name.clone() else { continue };
            let sets = self.sets.clone();
            let Some(anim) = self.anim(&name) else { continue };
            let Some(&(n, offset)) = sets.iter().find(|(n, _)| n + 1 == anim.tracks.len()) else { continue };
            let at = t.unwrap_or(if talking && anim.duration > 0.0 { time.rem_euclid(anim.duration) } else { 1.0 });
            let w = face_weights(anim, n, at);
            let o = offset as usize;
            if let Some(slot) = block.get_mut(o..o + n) {
                slot.copy_from_slice(&w);
            }
        }
    }
}

/// A character's motions: `<FileHead>_<value>.g1a` for a `CharaBase` column's value, in the
/// `<Motion1>` container or in a container of its own beside it. Loaded on demand, kept once
/// loaded.
struct Motions {
    char_dir: PathBuf,
    head: String,
    container: String,
    /// The `CharaBase` row's columns (`F_WAIT` → `ACT_DEF`, ...).
    columns: std::collections::HashMap<String, String>,
    loaded: std::collections::HashMap<String, Option<barrel_formats::g1a::Animation>>,
    /// The `Neck` container (`<FileHead>_NECK`), with the head gestures' clip.
    neck: Option<String>,
}

impl Motions {
    /// The head gestures' clip (`<FileHead>_M_HEAD.g1a` in the `Neck` container) and the joints
    /// it turns (more than half a degree anywhere in it): `s116` lays only those over the body.
    fn head_gestures(&self) -> Option<(barrel_formats::g1a::Animation, Vec<usize>)> {
        let bytes = std::fs::read(self.char_dir.join(format!("{}.elixir.gz", self.neck.as_ref()?))).ok()?;
        let c = Elixir::parse(&bytes).ok()?;
        let anim = barrel_formats::g1a::parse(named(&c, &format!("{}_M_HEAD.g1a", self.head))?).ok()?;
        let frames = (anim.duration * 30.0).round() as usize;
        let moving = anim
            .tracks
            .iter()
            .filter(|t| {
                let Some((first, ..)) = t.pose(0.0) else { return false };
                let first = glam::Quat::from_array(first).normalize();
                (0..=frames).filter_map(|f| t.pose(f as f32 / 30.0)).any(|(q, ..)| glam::Quat::from_array(q).normalize().angle_between(first) > 0.5f32.to_radians())
            })
            .map(|t| t.bone as usize)
            .collect();
        Some((anim, moving))
    }

    fn new(root: &Path, model: &str, row: &barrel_formats::table::Element) -> Option<Motions> {
        let dir = model.rsplit_once('/').map_or("", |(d, _)| d);
        Some(Motions {
            char_dir: root.join("Data/x64/Character").join(dir),
            head: row.attr("FileHead")?.to_string(),
            container: row.attr("Motion1")?.to_string(),
            columns: row.attributes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            loaded: Default::default(),
            neck: row.attr("Neck").map(str::to_string),
        })
    }

    /// The motion in `column`, if the row has one and its file is there.
    fn get(&mut self, column: &str) -> Option<&barrel_formats::g1a::Animation> {
        if !self.loaded.contains_key(column) {
            let anim = self.columns.get(column).and_then(|value| {
                let name = format!("{}_{value}.g1a", self.head);
                let from = |file: String| -> Option<Vec<u8>> {
                    let bytes = std::fs::read(self.char_dir.join(file)).ok()?;
                    let c = Elixir::parse(&bytes).ok()?;
                    named(&c, &name).map(|b| b.to_vec())
                };
                let bytes = from(format!("{}.elixir.gz", self.container)).or_else(|| from(format!("{}_{value}.elixir.gz", self.head)))?;
                barrel_formats::g1a::parse(&bytes).ok()
            });
            self.loaded.insert(column.to_string(), anim);
        }
        self.loaded.get(column).and_then(Option::as_ref)
    }
}

/// An event character's placement: number, position and facing angle in degrees.
type Placement = (u32, [f32; 3], f32);

fn event_characters(
    event_path: &Path,
    scene: &mut Scene,
    base: usize,
    line: Option<usize>,
    at: Option<f32>,
    actors: &mut Vec<Actor>,
    absent_out: &mut std::collections::HashSet<u32>,
) -> Result<(Vec<Vec<u8>>, Option<Vec3>, Option<EventCamera>, Vec<Placement>)> {
    use barrel_formats::ebd::{self, Ebd, OperandTable};
    let event = Ebd::parse(&std::fs::read(event_path)?)?;
    let root = event_path.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
    let table = |name: &str| -> Result<barrel_formats::table::Element> {
        Ok(barrel_formats::table::parse(&std::fs::read(root.join("Saves/chara").join(name))?)?)
    };
    let (charas, bases) = (table("Chara.xml")?, table("CharaBase.xml")?);
    // Faces at message line `line`: each speaker's latest expression up to it (the line's
    // fourth field), with the line's own speaker talking.
    let mut faces: std::collections::HashMap<i32, (i32, bool)> = Default::default();
    // And each speaker's latest gesture (the line's third field).
    let mut gestures: std::collections::HashMap<i32, i32> = Default::default();
    let mut face_tables = None;
    let mut gesture_table: Option<(Vec<[Option<barrel_game::gesture::Form>; 2]>, Vec<String>)> = None;
    if let Some(n) = line {
        let stem = event_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ebm = event_path.with_file_name(format!("{}.ebm", stem.replacen("EVENT_", "EVENT_MESSAGE_", 1)));
        let messages = barrel_formats::ebm::parse(&std::fs::read(&ebm)?)?;
        for (i, m) in messages.iter().enumerate().take(n + 1) {
            if m.fields[3] >= 0 {
                faces.insert(m.speaker, (m.fields[3], false));
            }
            if m.fields[2] >= 0 {
                gestures.insert(m.speaker, m.fields[2]);
            }
            if i == n {
                if let Some(text) = Some(m.repaired_text()) {
                    eprintln!("line {n}, speaker {}: {}", m.speaker, text.replace("<CR>", " "));
                }
                faces.entry(m.speaker).or_insert((0, false)).1 = true;
            }
        }
    }
    // The face and gesture tables (always: a played event changes faces as it goes).
    for entry in std::fs::read_dir(root)?.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|x| x == "exe") {
            let bytes = std::fs::read(&p)?;
            if let Ok(pe) = barrel_formats::pe::Pe::parse(&bytes) {
                let t = barrel_game::face::Tables::read(&pe);
                if !t.brow.is_empty() {
                    face_tables = Some(t);
                    if let Some(g) = barrel_game::gesture::read(&bytes) {
                        gesture_table = Some((g, barrel_game::gesture::motions(&pe)));
                    }
                    break;
                }
            }
        }
    }
    // First placement of each character: q4 (ADDITION_EX: number, ..., x, y, z, ?, angle) or
    // s4 (POS_SET: number, 0, 0, 0, x, y, z).
    let mut first: Vec<(u32, [f32; 3], f32)> = Vec::new();
    let mut camera = None;
    let ops = OperandTable::escha();
    for a in &event.actions {
        for c in ebd::decode(&a.words, &ops)? {
            let o = c.operands;
            if c.word == 0x7410_0014 && o.len() >= 11 && camera.is_none() {
                let i = |k: usize| o[k] as i32 as f32;
                camera = Some(EventCamera {
                    eye: None,
                    target: Vec3::new(i(5) + i(8), i(6) + i(9), i(7) + i(10)),
                    distance: i(4),
                    yaw: i(0) / 100.0,
                    pitch: i(3) / 100.0,
                    fov: i(2) / 100.0,
                });
            }
            let place = match c.word {
                0x7100_0004 if o.len() >= 9 => Some((o[0], [o[4], o[5], o[6]], o[8] as i32 as f32)),
                0x7300_0004 if o.len() >= 7 => Some((o[0], [o[4], o[5], o[6]], 0.0)),
                _ => None,
            };
            if let Some((who, p, angle)) = place {
                if !first.iter().any(|f| f.0 == who) {
                    first.push((who, p.map(|v| v as i32 as f32), angle));
                }
            }
        }
    }
    let mut stage_motions: std::collections::HashMap<u32, (String, f32, bool)> = Default::default();
    let absent = absent_out;
    // --at SECONDS: where the stage has everyone then (barrel_game::stage), and its camera
    // when it has set both the eye and the target.
    if let Some(at) = at {
        use barrel_game::stage::{Measures, Stage, FRAME};
        let curves = std::fs::read(root.join("Saves/curve/curve_data.xml"))
            .ok()
            .and_then(|b| barrel_formats::table::parse(&b).ok())
            .map(|t| barrel_game::curve::Curve::table(&t))
            .unwrap_or_default();
        // The same schedule playback uses: the event's lines, each held for a reading time.
        let stem = event_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ebm = event_path.with_file_name(format!("{}.ebm", stem.replacen("EVENT_", "EVENT_MESSAGE_", 1)));
        // The English lines when there are (as playback reads them: the reading times follow the
        // text, so other lines would put the stage out of step with what is shown).
        let english = event_path.strip_prefix(root.join("Event")).ok().map(|rel| root.join("Event_EN").join(rel).with_file_name(ebm.file_name().unwrap_or_default()));
        let read_lines = |p: &Path| std::fs::read(p).ok().and_then(|b| barrel_formats::ebm::parse(&b).ok()).filter(|m| !m.is_empty());
        let lines = english.as_deref().and_then(read_lines).or_else(|| read_lines(&ebm)).unwrap_or_default();
        let steps = barrel_game::event::Player::escha(&event, &lines, &ops)?.reading_time(barrel_game::event::reading_time).schedule();
        let upto: Vec<_> = steps.into_iter().filter(|t| t.time <= at).collect();
        let mut stage = Stage { measures: Measures::table(&charas), curves, ..Stage::default() }.play(&upto, |_, _| {});
        stage.run_until(at + FRAME * 0.5);
        // Everyone the event ever places is loaded (so playing on can show characters who come
        // later); those not on the stage at `at` start hidden.
        let later: Vec<Placement> = first.iter().filter(|f| !stage.characters.contains_key(&f.0)).copied().collect();
        first = stage.characters.iter().map(|(&n, b)| (n, b.position, b.facing.to_degrees())).collect();
        absent.extend(later.iter().map(|f| f.0));
        first.extend(later);
        // Each character's motion then, by its MOTION_* name (a CharaBase column), and how far in.
        let names = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_game::gesture::motions(&pe)).filter(|m| !m.is_empty()))
            .unwrap_or_default();
        for (&n, b) in &stage.characters {
            if let Some(m) = b.motion {
                if let Some(name) = names.get(m.motion as usize) {
                    stage_motions.insert(n, (name.clone(), at - m.since, m.looping));
                }
            }
        }
        if let (Some(e), Some(t)) = (stage.eye.position, stage.target.position) {
            let (e, t) = (Vec3::from(e), Vec3::from(t));
            camera = Some(EventCamera { eye: Some(e), target: t, distance: (e - t).length().max(1.0), yaw: 0.0, pitch: 0.0, fov: 40.0 });
        }
        eprintln!("at {at} s: {} characters on the stage, {} positions not placed", first.len(), stage.unresolved);
    }
    let placements: Vec<Placement> = first.clone();
    let mut packs = Vec::new();
    let mut tex_base = base;
    let mut centre = Vec3::ZERO;
    let mut placed = 0.0f32;
    for (who, pos, angle) in first {
        let Some(tag) = charas.children.get(who as usize).and_then(|c| c.attr("BaseTag")) else { continue };
        let Some(row_index) = bases.children.iter().position(|b| b.attr("BaseTag") == Some(tag)) else { continue };
        let row = &bases.children[row_index];
        let Some(model) = row.attr("Model") else { continue };
        let path = root.join("Data/x64/Character").join(format!("{model}.elixir.gz"));
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let container = Elixir::parse(&bytes)?;
        let stem = model.rsplit('/').next().unwrap_or(model);
        let (Some(mesh), Some(tex)) = (named(&container, &format!("{stem}_default.g1m")), named(&container, &format!("{stem}.g1t"))) else {
            continue;
        };
        let count = G1t::parse(tex)?.textures.len();
        let g = G1m::parse(mesh)?;
        let Some(geometry) = g.geometry.as_ref() else { continue };
        let transform = Mat4::from_rotation_translation(glam::Quat::from_rotation_y(angle.to_radians()), Vec3::from(pos));
        // The body motion: the speaker's gesture at this line if any (its main motion's
        // CharaBase column), otherwise the field wait (F_WAIT). The animation is
        // <FileHead>_<value>.g1a, in the <Motion1> container or in a container of its own
        // beside it, on the skeleton in the base model (<stem>.g1m).
        let staged = stage_motions.get(&who).filter(|(c, ..)| row.attr(c).is_some());
        let column = staged.map(|(c, ..)| c.clone()).or_else(|| gestures
            .get(&(who as i32))
            .and_then(|&g| {
                let (table, names) = gesture_table.as_ref()?;
                let form = table.get(g as usize)?[0].as_ref()?;
                names.get(form.main_motion() as usize).cloned()
            })
            .filter(|c| row.attr(c).is_some()))
            .unwrap_or_else(|| "F_WAIT".to_string());
        // How far into it: the stage's time in the motion (looping ones wrap), else the middle.
        let motion_time = staged.map(|&(_, t, looping)| (t, looping));
        let looping = motion_time.is_none_or(|(_, l)| l);
        let rig = (|| -> Option<(Vec<Mat4>, barrel_formats::g1m::Skeleton, Motions, barrel_formats::g1a::Animation, f32)> {
            let mut motions = Motions::new(root, model, row)?;
            let anim = motions.get(&column)?.clone();
            let skeleton = G1m::parse(named(&container, &format!("{stem}.g1m"))?).ok()?.skeleton?;
            eprintln!("character {who}: motion {column} = {}_{}", motions.head, motions.columns[&column]);
            let t0 = motion_time.map_or(0.5, |(t, _)| t);
            let joints = pose(&skeleton, &anim, motion_clock(&anim, t0, looping));
            Some((joints, skeleton, motions, anim, t0))
        })();
        let (joints, rig) = match rig {
            Some((j, sk, m, an, t0)) => (Some(j), Some((sk, m, an, t0))),
            None => (None, None),
        };
        // The face: every morph set goes in (so playback can change it), weighted by the line's
        // expression when `--line` gives one.
        let mut face_rig = None;
        if let (Some(tables), Some(head), Some(facial)) = (&face_tables, row.attr("FileHead"), row.attr("Facial")) {
            let dir = model.rsplit_once('/').map_or("", |(d, _)| d);
            let fbytes = std::fs::read(root.join("Data/x64/Character").join(dir).join(format!("{facial}.elixir.gz")));
            if let (Ok(fbytes), Some(g1h)) = (fbytes, named(&container, &format!("{stem}_default.g1h"))) {
                let facials = Elixir::parse(&fbytes)?;
                let mut anims = Vec::new();
                if let Some(&(expr, talking)) = faces.get(&(who as i32)) {
                    let face = tables.expression(row_index as u32, head, expr, talking);
                    for (name, t) in [(&face.brow, 0.5), (&face.eye, 0.5), (&face.mouth, 1.0)] {
                        if let Some(bytes) = name.as_ref().and_then(|n| named(&facials, &format!("{n}.g1a"))) {
                            anims.push((bytes, t));
                        }
                    }
                    eprintln!("character {who}: expression {expr}{} {face:?}", if talking { ", talking" } else { "" });
                }
                set_face(scene, g1h, &anims)?;
                let mut layouts: Vec<(usize, usize)> = scene.morph.iter().map(|(&l, (t, _))| (l, t.len())).collect();
                layouts.sort();
                let mut offset = 0u32;
                let sets = layouts
                    .iter()
                    .map(|&(_, n)| {
                        let o = offset;
                        offset += n as u32;
                        (n, o)
                    })
                    .collect();
                drop(facials);
                face_rig = Some(FaceRig { row: row_index as u32, head: head.to_string(), facials: fbytes, sets, loaded: Default::default() });
            }
        }
        let instance = scene.add_with_pack(geometry, transform, tex_base, count, joints.as_deref())?;
        if let Some((skeleton, motions, anim, start)) = rig {
            actors.push(Actor { who, instance, skeleton, motions, column: column.clone(), anim, start, looping, previous: None, last_t: start, face: face_rig.take(), head: None, division: None, last_division_column: String::new() });
        }
        scene.morph.clear();
        // The accessories always worn (<stem>_P_M_Acc_*: Escha's hat), on the body's skeleton
        // and instance; the weapons and options (CharaPartsChange groups) are left off.
        let accessory = format!("{stem}_P_M_Acc_");
        let first = scene.draws.len();
        for e in container.entries.iter().filter(|e| e.name.starts_with(&accessory) && e.name.ends_with(".g1m")) {
            let Some(bytes) = named(&container, &e.name) else { continue };
            let part = G1m::parse(bytes)?;
            if let Some(pg) = part.geometry.as_ref() {
                scene.add_posed(pg, Mat4::IDENTITY, count, joints.as_deref(), instance)?;
            }
        }
        for d in &mut scene.draws[first..] {
            d.texture += tex_base;
            d.overlay = d.overlay.map(|o| o + tex_base);
            d.mask = d.mask.map(|m| m + tex_base);
        }
        eprintln!("character {who} ({tag}) at {pos:?}, {angle} degrees");
        centre += Vec3::from(pos) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0);
        placed += 1.0;
        tex_base += count;
        packs.push(tex.to_vec());
    }
    Ok((packs, (placed > 0.0).then(|| centre / placed), camera, placements))
}

/// The fields an event plays in, in the order it first goes to each (`docs/events.md`, "Which
/// field"): each field map number and its directory.
fn event_fields(event_path: &Path) -> Result<Vec<(u32, PathBuf)>> {
    let root = event_path.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
    let event = barrel_formats::ebd::Ebd::parse(&std::fs::read(event_path)?)?;
    let steps = barrel_game::event::Player::escha(&event, &[], &barrel_formats::ebd::OperandTable::escha())?.schedule();
    let mut maps: Vec<u32> = Vec::new();
    for t in &steps {
        if let barrel_game::event::Step::FieldChange { map, .. } = t.step {
            if !maps.contains(&map) {
                maps.push(map);
            }
        }
    }
    if maps.is_empty() {
        return Err("the event changes to no field; give --field DIR".into());
    }
    let names = std::fs::read_dir(root)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "exe"))
        .filter_map(|p| std::fs::read(p).ok())
        .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_game::event::field_maps(&pe)).filter(|m| !m.is_empty()))
        .unwrap_or_default();
    maps.into_iter()
        .map(|map| {
            let name = names.get(map as usize).ok_or("field map number past the executable's list")?;
            eprintln!("field {map}: {name}");
            Ok((map, root.join("Data/x64/Field").join(name)))
        })
        .collect()
}

/// Where in `anim` to sample at motion time `t`: looping motions wrap, others hold their end.
fn motion_clock(anim: &barrel_formats::g1a::Animation, t: f32, looping: bool) -> f32 {
    if looping && anim.duration > 0.0 { t.rem_euclid(anim.duration) } else { t.clamp(0.0, anim.duration) }
}

/// Blend facial animations (at `time`) into `scene.morph` for the next model added: each
/// animation goes to the morph set with one target fewer than it has tracks.
fn set_face(scene: &mut Scene, g1h: &[u8], anims: &[(&[u8], f32)]) -> Result<()> {
    let sets = barrel_formats::g1h::parse(g1h)?;
    // Every set goes to the GPU (so a face can change later), at rest unless an animation
    // weights it.
    for set in &sets {
        scene.morph.insert(set.layout as usize, (set.targets.clone(), vec![0.0; set.targets.len()]));
    }
    for (bytes, time) in anims {
        let anim = barrel_formats::g1a::parse(bytes)?;
        let Some(set) = sets.iter().find(|s| s.targets.len() + 1 == anim.tracks.len()) else { continue };
        if let Some((_, w)) = scene.morph.get_mut(&(set.layout as usize)) {
            *w = face_weights(&anim, set.targets.len(), *time);
        }
    }
    Ok(())
}

/// A facial animation's target weights at `time`: track k + 1 weights target k.
fn face_weights(anim: &barrel_formats::g1a::Animation, targets: usize, time: f32) -> Vec<f32> {
    let t = time.min(anim.duration);
    (0..targets).map(|k| anim.tracks.iter().find(|tr| tr.bone as usize == k + 1).and_then(|tr| tr.scalar(t)).unwrap_or(0.0)).collect()
}

/// The entry of `container` named `name`, if any.
fn named<'a>(container: &'a Elixir, name: &str) -> Option<&'a [u8]> {
    container.entries.iter().find(|e| e.name == name && !e.is_dummy()).map(|e| container.data(e))
}

/// A field's placed objects and its shared texture pack.
/// Adds a field's ground and objects to the scene, as field `scene.field`, its textures taken
/// to start at `base` in the scene's list; returns its texture pack.
fn field_scene(dir: &Path, scene: &mut Scene, base: usize) -> Result<Vec<u8>> {
    let first_draw = scene.draws.len();
    let area = dir.file_name().ok_or("field directory has no name")?.to_string_lossy().into_owned();
    let main_bytes = std::fs::read(dir.join(format!("{area}.elixir.gz")))?;
    let main = Elixir::parse(&main_bytes)?;
    let obj_bytes = std::fs::read(dir.join(format!("{area}_Obj.elixir.gz")))?;
    let objects = Elixir::parse(&obj_bytes)?;
    let textures = named(&main, &format!("{area}.g1t")).ok_or("field has no texture pack")?.to_vec();
    let texture_count = G1t::parse(&textures)?.textures.len();
    // A flat placeholder terrain (every height 0, one layer: TOWN_03, TOWN_07) is not drawn: the
    // town's ground is its models, and events stand characters below it (TOWN_07's at y -255).
    let terrain = named(&main, &format!("{area}.ktf")).map(|b| barrel_formats::ktf::parse(b).map(|t| (b, t))).transpose()?.filter(|(_, t)| {
        let flat = t.heightmap.heights.iter().all(|&v| v == 0.0) && t.layers.textures.len() <= 1;
        if flat {
            eprintln!("{area}: flat placeholder terrain, not drawn");
        }
        !flat
    });
    if let Some((bytes, t)) = terrain {
        let h = &t.heightmap;
        let (first_index, index_count) = scene.add_heightmap(h);
        let mut scales = [0.01f32; 6];
        for (i, s) in t.layer_scales().into_iter().take(6).enumerate() {
            scales[i] = s;
        }
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for z in 0..=h.cells_z {
            for x in 0..=h.cells_x {
                let v = h.height(x, z);
                (lo, hi) = (lo.min(v), hi.max(v));
            }
        }
        eprintln!(
            "{area}: terrain {}x{} cells of {}, heights {lo}..{hi}, {} layers, scales {scales:?}",
            h.cells_x, h.cells_z, h.cell_size, t.layers.textures.len()
        );
        scene.terrains.push((
            scene.field.unwrap_or(0),
            TerrainDraw { first_index, index_count, field_size: h.cells_x as f32 * h.cell_size, scales, ktf: bytes.to_vec() },
        ));
    }
    let placements = barrel_formats::opl2::parse(named(&main, &format!("{area}.opl2")).ok_or("field has no placement list")?)?;
    let (mut placed, mut missing) = (0usize, 0usize);
    for t in &placements.objects {
        let Some(bytes) = named(&objects, &format!("{}_hi.g1m", t.name)) else {
            missing += t.instances.len();
            continue;
        };
        let model = G1m::parse(bytes)?;
        let Some(geometry) = model.geometry.as_ref() else { continue };
        for i in &t.instances {
            // Scale, then rotate about x, y and z in that order, then move.
            let rotation = glam::Quat::from_euler(glam::EulerRot::ZYX, i.rotation[2], i.rotation[1], i.rotation[0]);
            let transform = Mat4::from_scale_rotation_translation(Vec3::from(i.scale), rotation, Vec3::from(i.position));
            let before = scene.draws.len();
            scene.add(geometry, transform, texture_count)?;
            // Only the objects marked xDS_ cast the sun's shadow (TOWN_11's workshop: its
            // furniture and a second, xDS_ ceiling; the ceiling drawn does not cast).
            let casts = t.name.contains("xDS_") || i.tags.contains("xDS_");
            let version = object_version(&i.tags);
            for d in &mut scene.draws[before..] {
                d.casts = casts;
                d.name = t.name.clone();
                d.version = version;
            }
            placed += 1;
        }
    }
    eprintln!("{area}: {placed} objects placed, {missing} without a model");
    for d in &mut scene.draws[first_draw..] {
        d.texture += base;
        d.overlay = d.overlay.map(|o| o + base);
        d.mask = d.mask.map(|m| m + base);
    }
    Ok(textures)
}

/// The version a placement's tags give it: `<N>` from `xAM_balloon_ver<N>` (TOWN_07's dock
/// parts: `xDS_xAM_balloon_ver2`, `_balloon_ver3_xAM_`).
fn object_version(tags: &str) -> Option<u32> {
    if !tags.contains("xAM_") {
        return None;
    }
    let at = tags.find("_ver")? + 4;
    let digits: String = tags[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Upload a texture as it is stored (BC blocks and all mip levels) for sampling.
fn upload_texture(gpu: &Gpu, pool: vk::CommandPool, t: &barrel_formats::g1t::Texture) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView)> {
    let d = &gpu.device;
    {
        let format = match t.format {
            Format::Bc1 => vk::Format::BC1_RGBA_UNORM_BLOCK,
            Format::Bc3 => vk::Format::BC3_UNORM_BLOCK,
            Format::Bc7 => vk::Format::BC7_UNORM_BLOCK,
            Format::Bc4 => vk::Format::BC4_UNORM_BLOCK,
            Format::Bc5 => vk::Format::BC5_UNORM_BLOCK,
            Format::Bc6h => vk::Format::BC6H_UFLOAT_BLOCK,
            Format::Float32 => vk::Format::R32_SFLOAT,
            Format::Rgba16f => vk::Format::R16G16B16A16_SFLOAT,
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
        Ok((image, mem, view))
    }
}

#[allow(deprecated)] // glam 0.33 renamed the camera helpers; the old ones are kept
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // --window FRAMES: open a window and present that many frames (the live loop's start).
    if let ["--window", frames] = args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        return window::run(frames.parse()?);
    }
    let field = args.first().is_some_and(|a| a == "--field");
    let positional = if field { 3 } else { 3 };
    if args.len() < positional {
        return Err("usage: barrel-view MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png | --field DIR OUT.png  [--size N] [--yaw DEG] [--pitch DEG]".into());
    }
    let out_path = if field { args[2].clone() } else { args[2].clone() };
    let mut size = 768u32;
    let mut size_h = 768u32;
    let mut yaw = 0f32;
    let mut pitch = if field { 45f32 } else { 0f32 };
    let mut alpha_cutoff = 0.3f32;
    // The render settings (`settings.rs`): `--preset desktop|desktop-high|handheld` (`--handheld`
    // for the last), then single fields over it, in the order given: `--scale S` (the scene at
    // S times the output's size, 0.25 to 2), `--fsr` (FSR 1 instead of the bilinear scale),
    // `--msaa N` (samples a pixel, resolved in the scene pass), `--shadows desktop|handheld`.
    let mut settings = settings::RenderSettings::DESKTOP;
    // `--show-motion`: the motion vectors shown in place of the frame (red across, green down).
    let mut show_motion = false;
    // Print the GPU's time per stage at the end (--timings).
    let mut timings = false;
    let mut hero = 0u32;
    let (mut skeleton_spec, mut anim_spec, mut time) = (None::<String>, None::<String>, 0f32);
    let mut event_spec = None::<String>;
    let mut event_line = None::<usize>;
    let mut event_at = None::<f32>;
    let (mut frames, mut fps) = (1u32, 60f64);
    // --window FRAMES: show that many frames in a window, at the display's rate, instead of
    // writing them to files.
    let mut window_frames = None::<u32>;
    let mut look_at = None::<u32>;
    let (mut morph_spec, mut face_specs) = (None::<String>, Vec::<String>::new());
    let mut use_event_camera = true;
    let mut no_terrain = false;
    let mut it = args[positional..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // N (square) or WxH, up to 8192 either way (4K, ultrawide 3440×1440 and 5120×1440, 8K wide).
            "--size" => {
                let v = it.next().ok_or("--size N|WxH")?;
                let (w, h) = v.split_once('x').unwrap_or((v, v));
                size = w.parse::<u32>()?.clamp(64, 8192);
                size_h = h.parse::<u32>()?.clamp(64, 8192);
            }
            "--yaw" => yaw = it.next().ok_or("--yaw DEG")?.parse()?,
            "--no-alpha" => alpha_cutoff = -1.0,
            "--preset" => {
                let name = it.next().ok_or("--preset desktop|desktop-high|handheld")?;
                settings = settings::RenderSettings::preset(name).ok_or_else(|| format!("{name}: no such preset"))?;
            }
            "--handheld" => settings = settings::RenderSettings::HANDHELD,
            "--fsr" => settings.upscaler = settings::Upscaler::Fsr1,
            "--gsr" => settings.upscaler = settings::Upscaler::Gsr1,
            "--motion" => settings.motion = true,
            "--jitter" => settings.jitter = true,
            "--taa" => settings.taa = true,
            "--gsr2" => settings.gsr2 = true,
            "--fsr3" => settings.fsr3 = true,
            "--timings" => timings = true,
            // The protagonist the player chose, whom "Master Character" (row 749) stands for.
            "--hero" => {
                hero = match it.next().ok_or("--hero escha|logy")?.as_str() {
                    "escha" => 0,
                    "logy" => 1,
                    other => return Err(format!("--hero {other}: escha or logy").into()),
                }
            }
            "--sharpen" => settings.sharpen = true,
            "--show-motion" => {
                settings.motion = true;
                show_motion = true;
            }
            "--msaa" => settings.msaa = it.next().ok_or("--msaa N")?.parse()?,
            "--scale" => settings.scale = it.next().ok_or("--scale S")?.parse::<f32>()?.clamp(0.25, 2.0),
            "--shadows" => {
                settings.shadows = match it.next().map(String::as_str) {
                    Some("desktop") => shadow::Shadows::DESKTOP,
                    Some("handheld") => shadow::Shadows::HANDHELD,
                    _ => return Err("--shadows desktop|handheld".into()),
                }
            }
            "--skeleton" => skeleton_spec = Some(it.next().ok_or("--skeleton FILE#N")?.clone()),
            "--anim" => anim_spec = Some(it.next().ok_or("--anim FILE#N")?.clone()),
            "--time" => time = it.next().ok_or("--time SECONDS")?.parse()?,
            "--morph" => morph_spec = Some(it.next().ok_or("--morph FILE#N")?.clone()),
            "--face" => face_specs.push(it.next().ok_or("--face FILE#N")?.clone()),
            "--look-at" => look_at = Some(it.next().ok_or("--look-at CHARA")?.parse()?),
            "--line" => event_line = Some(it.next().ok_or("--line N")?.parse()?),
            "--at" => event_at = Some(it.next().ok_or("--at SECONDS")?.parse()?),
            "--frames" => frames = it.next().ok_or("--frames N")?.parse::<u32>()?.clamp(1, 600),
            "--window" => window_frames = Some(it.next().ok_or("--window FRAMES")?.parse::<u32>()?.clamp(1, 100_000)),
            "--fps" => fps = it.next().ok_or("--fps HZ")?.parse::<f64>()?.clamp(1.0, 1000.0),
            "--event" => event_spec = Some(it.next().ok_or("--event FILE.ebd")?.clone()),
            "--free-camera" => use_event_camera = false,
            "--no-terrain" => no_terrain = true,
            "--pitch" => pitch = it.next().ok_or("--pitch DEG")?.parse::<f32>()?.clamp(-89.0, 89.0),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    // CPU side: the vertices and draws, and the texture pack.
    let mut scene = Scene::new();
    // The field map numbers loaded, in scene order (empty for `--field DIR`).
    let mut field_maps: Vec<u32> = Vec::new();
    let mut field_looks: Vec<Option<barrel_formats::field_look::FieldLook>> = Vec::new();
    // Each field's distant view's instance (it follows the camera), if it has one.
    let mut sky_instances: Vec<Option<u32>> = Vec::new();
    let mut pack_bytes = Vec::new();
    if field {
        // `--field auto` with an event: every field the event goes to.
        let dirs = if args[1] == "auto" {
            let e = event_spec.as_ref().ok_or("--field auto needs --event")?;
            event_fields(Path::new(e))?
        } else {
            vec![(u32::MAX, PathBuf::from(&args[1]))]
        };
        let mut base = 0;
        for (k, (map, dir)) in dirs.iter().enumerate() {
            // Its look, from the table of the same name (Saves/field/<FIELD>.xml).
            let table = dir
                .ancestors()
                .find(|p| p.join("Saves").is_dir())
                .zip(dir.file_name())
                .and_then(|(root, name)| std::fs::read(root.join("Saves/field").join(name).with_extension("xml")).ok())
                .and_then(|b| barrel_formats::table::parse(&b).ok());
            field_looks.push(table.map(|t| barrel_formats::field_look::read(&t)));
            scene.field = Some(k as u16);
            let bytes = field_scene(dir, &mut scene, base)?;
            base += G1t::parse(&bytes)?.textures.len();
            pack_bytes.push(bytes);
            field_maps.push(*map);
            // Its distant view (C_SPHERE/<set>, by the executable's table), all in one instance.
            let root = dir.ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf);
            let set = root.as_ref().and_then(|r| {
                let exe = std::fs::read(r.join("Atelier_Escha_and_Logy_EN.exe")).ok()?;
                let pe = barrel_formats::pe::Pe::parse(&exe).ok()?;
                let name = dir.file_name()?.to_str()?;
                barrel_formats::field_look::sky_sets(&pe, barrel_formats::field_look::SKY_SETS_EN).into_iter().find(|(f, _)| f == name).map(|(_, s)| s)
            });
            let mut sky = None;
            if let (Some(root), Some(set)) = (&root, set.filter(|s| !s.starts_with("set-black"))) {
                let bytes = std::fs::read(root.join("Data/x64/Field/C_SPHERE").join(&set))?;
                let container = Elixir::parse(&bytes)?;
                let instance = scene.instance(Mat4::IDENTITY);
                // Drawn without depth, so back to front by kind: the dome, then the clouds
                // (`xEF_`), then the ground, stones, trees and cliffs in front of them (in file
                // order the clouds came after the trees and veiled them: TOWN_07, event 212).
                let rank = |name: &str| if name.contains("Sphere_sky") { 0 } else if name.contains("xEF_") { 1 } else { 2 };
                let mut entries: Vec<_> = container.entries.iter().collect();
                entries.sort_by_key(|e| rank(&e.name));
                for e in entries {
                    let Some(stem) = e.name.strip_suffix(".g1m") else { continue };
                    let (Some(mesh), Some(tex)) = (named(&container, &format!("{stem}.g1m")), named(&container, &format!("{stem}.g1t"))) else { continue };
                    let count = G1t::parse(tex)?.textures.len();
                    let model = G1m::parse(mesh)?;
                    let Some(geometry) = model.geometry.as_ref() else { continue };
                    let first = scene.draws.len();
                    scene.add_posed(geometry, Mat4::IDENTITY, count, None, instance)?;
                    for d in &mut scene.draws[first..] {
                        d.texture += base;
                        d.overlay = d.overlay.map(|o| o + base);
                        d.mask = d.mask.map(|m| m + base);
                        d.sky = true;
                        d.name = format!("sky:{stem}");
                    }
                    base += count;
                    pack_bytes.push(tex.to_vec());
                }
                eprintln!("{}: distant view {set}", dir.display());
                sky = Some(instance);
            }
            sky_instances.push(sky);
        }
        scene.field = None;
    } else {
        let model_bytes = entry_bytes(&args[0])?;
        let model = G1m::parse(&model_bytes)?;
        let geometry = model.geometry.as_ref().ok_or("model has no geometry")?;
        let tex_bytes = entry_bytes(&args[1])?;
        let count = G1t::parse(&tex_bytes)?.textures.len();
        let joints = match (&skeleton_spec, &anim_spec) {
            (Some(sk), Some(an)) => {
                let sk_bytes = entry_bytes(sk)?;
                let skel = G1m::parse(&sk_bytes)?.skeleton.ok_or("no skeleton in that model")?;
                let anim = barrel_formats::g1a::parse(&entry_bytes(an)?)?;
                eprintln!("posing {} joints with {} tracks at {time}s of {}s", skel.joints.len(), anim.tracks.len(), anim.duration);
                Some(pose(&skel, &anim, time))
            }
            (None, None) => None,
            _ => return Err("--skeleton and --anim go together".into()),
        };
        if let Some(m) = &morph_spec {
            let anims: Vec<Vec<u8>> = face_specs.iter().map(|f| entry_bytes(f)).collect::<Result<_>>()?;
            let with_time: Vec<(&[u8], f32)> = anims.iter().map(|a| (&a[..], time)).collect();
            set_face(&mut scene, &entry_bytes(m)?, &with_time)?;
        } else if !face_specs.is_empty() {
            return Err("--face needs --morph".into());
        }
        let instance = if joints.is_some() || !scene.morph.is_empty() { scene.instance(Mat4::IDENTITY) } else { 0 };
        scene.add_posed(geometry, Mat4::IDENTITY, count, joints.as_deref(), instance)?;
        scene.morph.clear();
        pack_bytes.push(tex_bytes);
    }
    let mut focus = None;
    let mut event_camera: Option<EventCamera> = None;
    let mut actors: Vec<Actor> = Vec::new();
    // Characters loaded but not on the stage (yet, or any more): not drawn.
    let mut absent: std::collections::HashSet<u32> = Default::default();
    if let Some(e) = &event_spec {
        let base = pack_bytes.iter().map(|b| G1t::parse(b).map(|t| t.textures.len())).sum::<std::result::Result<usize, _>>()?;
        let (packs, centre, cam, placements) = event_characters(Path::new(e), &mut scene, base, event_line, event_at, &mut actors, &mut absent)?;
        pack_bytes.extend(packs);
        focus = centre;
        event_camera = cam;
        // --look-at N: a close-up of character N's head, from in front of them.
        if let Some(who) = look_at {
            let &(_, pos, angle) = placements.iter().find(|p| p.0 == who).ok_or("--look-at: that character is not placed")?;
            event_camera = Some(EventCamera {
                eye: None,
                target: Vec3::from(pos) + Vec3::new(0.0, 140.0, 0.0),
                distance: 160.0,
                yaw: 180.0 - angle,
                pitch: 5.0,
                fov: 30.0,
            });
        }
    }
    let packs: Vec<G1t> = pack_bytes.iter().map(|b| G1t::parse(b)).collect::<std::result::Result<_, _>>()?;
    let all_textures: Vec<&barrel_formats::g1t::Texture> = packs.iter().flat_map(|p| p.textures.iter()).collect();
    if no_terrain {
        scene.terrains.clear();
    }
    let Scene { vertices, indices, mut draws, terrains, lo, hi, mut instances, mut palette, deltas, mut morph_weights, .. } = scene;
    // Lay each overlay over its base texture into a new RGBA texture.
    let mut composites: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for draw in &mut draws {
        if let Some(o) = draw.overlay {
            let (base, top) = (all_textures[draw.texture], all_textures[o]);
            let (w, h) = (base.width, base.height);
            let mut px = base.to_rgba8();
            let over = top.to_rgba8();
            for y in 0..h {
                for x in 0..w {
                    let (sx, sy) = (x * top.width / w, y * top.height / h);
                    let src = &over[((sy * top.width + sx) * 4) as usize..][..4];
                    let dst = &mut px[((y * w + x) * 4) as usize..][..4];
                    let a = src[3] as u32;
                    for c in 0..3 {
                        dst[c] = ((src[c] as u32 * a + dst[c] as u32 * (255 - a)) / 255) as u8;
                    }
                }
            }
            draw.texture = all_textures.len() + composites.len();
            composites.push((w, h, px));
        }
    }
    let composite_textures: Vec<barrel_formats::g1t::Texture> = composites
        .iter()
        .map(|(w, h, px)| barrel_formats::g1t::Texture {
            width: *w,
            height: *h,
            format: Format::Rgba8,
            format_code: 1,
            layers: 1,
            volume: false,
            levels: vec![&px[..]],
        })
        .collect();
    let all_textures: Vec<&barrel_formats::g1t::Texture> = all_textures.into_iter().chain(composite_textures.iter()).collect();
    if draws.is_empty() || all_textures.is_empty() {
        return Err("nothing to draw".into());
    }
    eprintln!("{} draws, {} vertices, {} triangles, {} textures", draws.len(), vertices.len(), indices.len() / 3, all_textures.len());
    eprintln!("bounds {lo:?} .. {hi:?}");
    if std::env::var("BARREL_DRY").is_ok() {
        return Ok(());
    }

    // A window (with --window) or an offscreen image to draw into.
    let mut pump = window_frames.map(|_| window::Pump::open()).transpose()?;
    let display = pump.as_ref().map(|p| raw_window_handle::HasDisplayHandle::display_handle(p.window()).map(|h| h.as_raw())).transpose()?;
    let gpu = Gpu::new(display)?;
    let mut presenter = match (&pump, display) {
        (Some(p), Some(display)) => {
            let handle = raw_window_handle::HasWindowHandle::window_handle(p.window())?.as_raw();
            let size = p.window().inner_size();
            Some(present::Presenter::new(&gpu, display, handle, size.width, size.height)?)
        }
        _ => None,
    };
    let (mut width, mut height) = presenter.as_ref().map_or((size, size_h), |p| (p.extent.width, p.extent.height));
    let color_format = presenter.as_ref().map_or(COLOR_FORMAT, |p| p.format);
    if let Some(p) = &presenter {
        eprintln!("window: {width}x{height}, {:?}, {} swapchain images", p.format, p.images.len());
    }
    let frames = window_frames.unwrap_or(frames);
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
    for t in &all_textures {
        textures.push(upload_texture(&gpu, pool, t)?);
    }
    // The characters' outline texture (`Data/x64/Shader/texture/outline.g1t`), if found.
    let outline_tex = {
        let root = event_spec.as_ref().and_then(|e| Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf));
        let bytes = root.and_then(|r| std::fs::read(r.join("Data/x64/Shader/texture/outline.g1t")).ok());
        match bytes.as_ref().and_then(|b| G1t::parse(b).ok()).and_then(|g| g.textures.into_iter().next().map(|t| upload_texture(&gpu, pool, &t))) {
            Some(t) => {
                textures.push(t?);
                Some(textures.len() - 1)
            }
            None => None,
        }
    };
    // White, for masks a material does not have.
    let white_px = [255u8; 4];
    let white = textures.len();
    textures.push(upload_texture(
        &gpu,
        pool,
        &barrel_formats::g1t::Texture { width: 1, height: 1, format: Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&white_px[..]] },
    )?);
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

    // Instances: one model matrix each, in a storage buffer the vertex shader indexes by the
    // draw's instance number (set 1, bound once for every pipeline).
    for i in &mut instances {
        i.prev_palette = i.palette + palette.len() as u32;
        i.prev_model = i.model;
    }
    let ibytes_inst = unsafe { std::slice::from_raw_parts(instances.as_ptr() as *const u8, instances.len() * size_of::<Instance>()) };
    let (instance_buf, instance_mem) = gpu.buffer(ibytes_inst.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
    gpu.upload(instance_mem, ibytes_inst)?;
    // And the joint palette (binding 1): this frame's joints, then last frame's (the same
    // to begin with), for the motion vectors.
    let palette_bytes = |p: &[Mat4]| unsafe { std::slice::from_raw_parts(p.as_ptr() as *const u8, p.len() * size_of::<Mat4>()) };
    let half = (palette.len() * size_of::<Mat4>()) as u64;
    let (palette_buf, palette_mem) = gpu.buffer((2 * half).max(64), vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
    gpu.upload(palette_mem, palette_bytes(&palette))?;
    gpu.upload_at(palette_mem, half, palette_bytes(&palette))?;
    let mut prev_palette = palette.clone();
    let storage = |b: u32| {
        vk::DescriptorSetLayoutBinding::default()
            .binding(b)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
    };
    if deltas.len() > 1 {
        eprintln!(
            "morphs: {} vertex offsets, {} weights, {} not zero",
            deltas.len() - 1,
            morph_weights.len() - 1,
            morph_weights.iter().filter(|&&w| w != 0.0).count()
        );
    }
    // The morph targets' offsets (binding 2) and the instances' weights (binding 3).
    let dbytes = unsafe { std::slice::from_raw_parts(deltas.as_ptr() as *const u8, deltas.len() * 16) };
    let (delta_buf, delta_mem) = gpu.buffer(dbytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
    gpu.upload(delta_mem, dbytes)?;
    let wbytes = unsafe { std::slice::from_raw_parts(morph_weights.as_ptr() as *const u8, morph_weights.len() * 4) };
    let (weight_buf, weight_mem) = gpu.buffer(wbytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
    gpu.upload(weight_mem, wbytes)?;
    // And the field's character lighting (binding 4, for the fragment shader).
    let (look_buf, look_mem) = gpu.buffer(LOOK_SIZE, vk::BufferUsageFlags::UNIFORM_BUFFER, true)?;
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
    ];
    let instance_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&inst_binding), None)? };
    let inst_sizes = [
        vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 4 },
        vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1 },
        vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 1 },
    ];
    let inst_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&inst_sizes), None)? };
    let inst_layouts = [instance_layout];
    let instance_set =
        unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(inst_pool).set_layouts(&inst_layouts))?[0] };
    let inst_info = [vk::DescriptorBufferInfo::default().buffer(instance_buf).offset(0).range(vk::WHOLE_SIZE)];
    let pal_info = [vk::DescriptorBufferInfo::default().buffer(palette_buf).offset(0).range(vk::WHOLE_SIZE)];
    let delta_info = [vk::DescriptorBufferInfo::default().buffer(delta_buf).offset(0).range(vk::WHOLE_SIZE)];
    let weight_info = [vk::DescriptorBufferInfo::default().buffer(weight_buf).offset(0).range(vk::WHOLE_SIZE)];
    let look_info = [vk::DescriptorBufferInfo::default().buffer(look_buf).offset(0).range(vk::WHOLE_SIZE)];
    let shadow_target = shadow::ShadowTarget::new(&gpu, settings.shadows)?;
    let shadow_info = [vk::DescriptorImageInfo::default()
        .sampler(shadow_target.sampler)
        .image_view(shadow_target.view)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    unsafe {
        d.update_descriptor_sets(
            &[
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&inst_info),
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&pal_info),
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&delta_info),
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(3).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&weight_info),
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&look_info),
                vk::WriteDescriptorSet::default()
                    .dst_set(instance_set)
                    .dst_binding(5)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&shadow_info),
            ],
            &[],
        )
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

    // TAA brings the motion vectors and the jitter, and drops MSAA.
    let settings = settings.resolved();
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
            .format(SCENE_FORMAT)
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
            .format(SCENE_FORMAT)
            .samples(samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        single_attachments[1].samples(samples),
        vk::AttachmentDescription::default()
            .format(SCENE_FORMAT)
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
    let post_pass = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&post_attachment).subpasses(&post_subpass), None)? };
    // The post pass's one texture: the scene.
    let post_binding = [0, 1, 2].map(|b| {
        vk::DescriptorSetLayoutBinding::default()
            .binding(b)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
    });
    let post_set_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&post_binding), None)? };
    let post_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 3 }];
    let post_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&post_sizes), None)? };
    let post_layouts = [post_set_layout];
    let post_set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(post_pool).set_layouts(&post_layouts))?[0] };
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
    // The depth image and one framebuffer per target image (the offscreen image, or each
    // swapchain image), remade at the new size when the window changes.
    let colour_views = |presenter: &Option<present::Presenter>| -> Vec<vk::ImageView> {
        match (presenter, color_view) {
            (Some(p), _) => p.views.clone(),
            (None, Some(v)) => vec![v],
            (None, None) => Vec::new(),
        }
    };
    let glow_pipelines = glow::Pipelines::new(&gpu)?;
    let upscale_mode = if settings.upscaler == settings::Upscaler::Gsr1 && settings.upscales() { upscale::Mode::Gsr1 } else { upscale::Mode::Fsr1 };
    let upscale_pipelines = if settings.upscales() || settings.sharpen_only() { Some(upscale::Pipelines::new(&gpu, upscale_mode)?) } else { None };
    let taa_pipelines = if settings.taa { Some(taa::Pipelines::new(&gpu)?) } else { None };
    let gsr2_pipelines = if settings.gsr2 { Some(gsr2::Pipelines::new(&gpu)?) } else { None };
    let fsr3_pipelines = if settings.fsr3 { Some(fsr3::Pipelines::new(&gpu)?) } else { None };
    let mut timer = if timings { timing::Timer::new(&gpu)? } else { None };
    let mut targets = Targets::new(&gpu, render_pass, post_pass, &colour_views(&presenter), width, height, settings.scene_size(width, height), post_set, post_sampler, &glow_pipelines, upscale_pipelines.as_ref(), taa_pipelines.as_ref(), gsr2_pipelines.as_ref(), fsr3_pipelines.as_ref(), samples, settings.motion, show_motion)?;

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

    // The distant view: drawn first, behind everything (no depth test or write).
    let no_depth = vk::PipelineDepthStencilStateCreateInfo::default();
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
    let post_push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::FRAGMENT, offset: 0, size: 32 }];
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

    // Terrain: each field's own textures in a set of 2 blend maps + 6 layers, and one pipeline
    // with the terrain fragment shader.
    let mut terrain_gpu: Option<TerrainGpu> = None;
    if !terrains.is_empty() {
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(2)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(6)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let tlayout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)? };
        let n = terrains.len() as u32;
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 8 * n }];
        let tpool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)? };
        let tpl_layouts = [tlayout, instance_layout];
        let tpl = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&tpl_layouts).push_constant_ranges(&push), None)? };
        let tfs = module(TERRAIN_FRAG)?;
        let tstages = [stages[0], vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(tfs).name(c"main")];
        // The heightmap is wound the other way from the models and seen from above only: no
        // culling.
        let traster = raster.cull_mode(vk::CullModeFlags::NONE);
        let info = pipeline_info.stages(&tstages).layout(tpl).rasterization_state(&traster);
        let tpipe = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0] };
        let mut grounds = Vec::new();
        for (field, t) in &terrains {
            let bundle = barrel_formats::ktf::parse(&t.ktf)?;
            let mut tex = Vec::new();
            for b in bundle.blend.textures.iter().take(2) {
                tex.push(upload_texture(&gpu, pool, b)?);
            }
            // A field with one blend map (TOWN_03) weights only the first three layers: the
            // second map is black, so layers 4 to 6 get no weight.
            let black = [0u8, 0, 0, 255];
            while tex.len() < 2 && !tex.is_empty() {
                let t = barrel_formats::g1t::Texture {
                    width: 1,
                    height: 1,
                    format: Format::Rgba8,
                    format_code: 1,
                    layers: 1,
                    volume: false,
                    levels: vec![&black[..]],
                };
                tex.push(upload_texture(&gpu, pool, &t)?);
            }
            let layer_count = bundle.layers.textures.len().clamp(1, 6);
            for i in 0..6 {
                tex.push(upload_texture(&gpu, pool, &bundle.layers.textures[i.min(layer_count - 1)])?);
            }
            if tex.len() != 8 {
                return Err("terrain needs two blend maps".into());
            }
            let tl = [tlayout];
            let tset = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(tpool).set_layouts(&tl))?[0] };
            let infos: Vec<vk::DescriptorImageInfo> = tex
                .iter()
                .map(|(_, _, view)| vk::DescriptorImageInfo::default().sampler(sampler).image_view(*view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL))
                .collect();
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(tset).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos[..2]),
                vk::WriteDescriptorSet::default().dst_set(tset).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos[2..]),
            ];
            unsafe { d.update_descriptor_sets(&writes, &[]) };
            grounds.push(Ground { field: *field, set: tset, textures: tex, first_index: t.first_index, index_count: t.index_count, field_size: t.field_size, scales: t.scales });
        }
        terrain_gpu = Some(TerrainGpu { layout: tlayout, pool: tpool, pipeline_layout: tpl, shader: tfs, pipeline: tpipe, grounds });
    }

    // Camera: look at the model's bounds from its front, turned by --yaw. Models face +z.
    // With an event, look at its characters from a few metres away; otherwise frame it all.
    let (center, radius) = match focus {
        Some(c) => (c, 250.0),
        None => ((lo + hi) * 0.5, (hi - lo).length() * 0.5),
    };
    let fov = 30f32.to_radians();
    let dist = radius / (fov * 0.5).sin() * 1.05;
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    let dir = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
    let (mut eye, mut target, mut fov, mut dist) = (center + dir * dist, center, fov, dist);
    // The event's opening camera (see docs/events.md); --free-camera frames the characters.
    if let (Some(c), true) = (event_camera, use_event_camera) {
        // Game space: yaw about y from -z, pitch up; then mirror x like the vertices.
        let (yw, pt) = (c.yaw.to_radians(), c.pitch.to_radians());
        // As the field camera builds it (barrel_game::field_camera::FieldCamera::eye_target).
        let back = Vec3::new(yw.sin() * pt.cos(), pt.sin(), yw.cos() * pt.cos());
        let mirror = Vec3::new(-1.0, 1.0, 1.0);
        target = c.target * mirror;
        eye = c.eye.unwrap_or(c.target + back * c.distance) * mirror;
        fov = c.fov.clamp(10.0, 120.0).to_radians();
        dist = c.distance;
        eprintln!("event camera: target {:?}, distance {}, yaw {}, pitch {}, fov {}", c.target, c.distance, c.yaw, c.pitch, c.fov);
    }
    // The camera can be turned about its target and moved in or out from the window
    // (arrows, + and -): `orbit` holds the turn (yaw, pitch in radians) and the zoom factor.
    let mut orbit = (0f32, 0f32, 1f32);
    // The projection follows the target's current aspect (the window can change size).
    // The eye after the window's own turning and zooming about the target.
    let turned_eye = |eye: Vec3, target: Vec3, (yaw, pitch, zoom): (f32, f32, f32)| {
        let offset = eye - target;
        let flat = offset.length();
        let turned = glam::Quat::from_rotation_y(yaw) * offset;
        let right = turned.cross(Vec3::Y).normalize_or_zero();
        let tilted = glam::Quat::from_axis_angle(right, pitch) * turned;
        // Do not tip over the top or under the bottom.
        let offset = if tilted.normalize_or_zero().y.abs() < 0.98 { tilted } else { turned };
        target + offset.normalize_or_zero() * flat * zoom
    };
    // BARREL_HIDE=a,b: leave out the draws whose name holds any of these (finding which object
    // is which against an oracle frame).
    let hide_names: Vec<String> = std::env::var("BARREL_HIDE").map(|v| v.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect()).unwrap_or_default();
    let hide_by_name = |name: &str| !name.is_empty() && hide_names.iter().any(|h| name.contains(h.as_str()));
    let view_proj = |eye: Vec3, target: Vec3, dist: f32, fov: f32, width: u32, height: u32, orbit: (f32, f32, f32)| {
        let zoom = orbit.2;
        let view = Mat4::look_at_rh(turned_eye(eye, target, orbit), target, Vec3::Y);
        // Far enough for the distant view (its dome is about 35,000 units across).
        let mut proj = Mat4::perspective_rh(fov, width as f32 / height.max(1) as f32, dist * 0.01 * zoom, (dist * 20.0 * zoom).max(80_000.0));
        proj.y_axis.y *= -1.0; // Vulkan's y points down.
        // The game's x is mirrored into render space (right-handed); mirroring clip-space x back
        // shows the scene as the game does (left to right as in its frames; oracle shot op111).
        proj.x_axis.x *= -1.0;
        proj * view
    };

    // With --frames N, play the event on: the clock at --fps HZ decides the logic ticks
    // (docs/engine.md, "Timing"), each frame draws the last two ticks blended, and only the
    // characters' instance matrices and joint palettes change between frames.
    let mut player = None;
    // The text layer, the font, and the line it was last given (rebuilt when that changes).
    let mut text_layer: Option<(text::Layer, text::Font, Option<(Option<barrel_game::stage::Line>, u32, u32, Option<u32>, u16, Option<(u32, Option<u32>, u16)>)>)> = None;
    // The narration cards the event shows (u55), and each one's pieces once the layer has
    // their textures: (texture slot, uv, rectangle on a 1920 × 1080 screen).
    let telop_cards: Vec<u32>;
    // The still pictures the event shows (u1/0x30), and each one's texture slot and size on a
    // 1920 × 1080 screen once the layer has them.
    let still_pictures: Vec<u32>;
    let mut still_slots: std::collections::HashMap<u32, (usize, [f32; 2])> = std::collections::HashMap::new();
    let mut telop_pieces: std::collections::HashMap<u32, Vec<(usize, [f32; 4], [f32; 4])>> = std::collections::HashMap::new();
    // The event's music and voices, in a window only (frames written to files are silent):
    // the output, the mixer, the library, and the music now playing.
    let mut sound: Option<(barrel_audio::Output, barrel_audio::Mixer, barrel_audio::Library, Option<barrel_audio::Playing>)> = None;
    let mut motion_names: Vec<String> = Vec::new();
    let messages: Vec<barrel_formats::ebm::Message>;
    let mut speaker_names: Vec<String> = Vec::new();
    // The message window's names (the executable's speaker-name table).
    let mut window_names: Vec<String> = Vec::new();
    let mut face_tables_play: Option<barrel_game::face::Tables> = None;
    let mut gestures_play: Vec<[Option<barrel_game::gesture::Form>; 2]> = Vec::new();
    // The motions' divisions (s5, s11), from Saves/chara.
    // The camera quakes (t14), and the generator their shake draws from: the game's own kind,
    // seeded here (the game's shared generator state is not followed: docs/events.md).
    let quakes: Vec<barrel_formats::quake::Quake> = event_spec
        .as_ref()
        .and_then(|e| Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf))
        .and_then(|root| std::fs::read(root.join("Saves/cameraQuake/camera_quake_data.xml")).ok())
        .and_then(|b| barrel_formats::table::parse(&b).ok())
        .map(|t| barrel_formats::quake::read(&t))
        .unwrap_or_default();
    let mut quake_rng = barrel_game::rng::Rng::from_state([0x1234_5678, 0x9abc_def0, 0x0fed_cba9, 0x8765_4321]);
    let mut divisions = barrel_formats::chara_div::Divisions::new();
    let event_file = if frames > 1 { event_spec.as_ref().map(|e| std::fs::read(e)).transpose()? } else { None };
    let event_data = event_file.as_ref().map(|b| barrel_formats::ebd::Ebd::parse(b)).transpose()?;
    if let (Some(event), Some(e)) = (&event_data, &event_spec) {
        use barrel_game::stage::{Measures, Playback, Stage};
        let root = Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
        let xml = |rel: &str| std::fs::read(root.join(rel)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
        let measures = xml("Saves/chara/Chara.xml").map(|t| Measures::table(&t)).unwrap_or_default();
        let curves = xml("Saves/curve/curve_data.xml").map(|t| barrel_game::curve::Curve::table(&t)).unwrap_or_default();
        let stem = Path::new(e).file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ebm = Path::new(e).with_file_name(format!("{}.ebm", stem.replacen("EVENT_", "EVENT_MESSAGE_", 1)));
        // The English lines (Event_EN mirrors Event), else the original's.
        let english = Path::new(e).strip_prefix(root.join("Event")).ok().map(|rel| root.join("Event_EN").join(rel).with_file_name(ebm.file_name().unwrap_or_default()));
        let read_lines = |p: &Path| std::fs::read(p).ok().and_then(|b| barrel_formats::ebm::parse(&b).ok()).filter(|m| !m.is_empty());
        messages = english.as_deref().and_then(read_lines).or_else(|| read_lines(&ebm)).unwrap_or_default();
        window_names = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))
            .ok()
            .and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_formats::layout::speaker_names(&pe)))
            .unwrap_or_default();
        speaker_names = xml("Saves_EN/chara/Chara.xml")
            .or_else(|| xml("Saves/chara/Chara.xml"))
            .map(|t| t.children.iter().map(|c| c.attr("Name").unwrap_or("").to_string()).collect())
            .unwrap_or_default();
        motion_names = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_game::gesture::motions(&pe)).filter(|m| !m.is_empty()))
            .unwrap_or_default();
        for name in ["CharaDiv.xml", "SubCharaDiv.xml"] {
            if let Some(t) = std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok()) {
                barrel_formats::chara_div::read(&t, &mut divisions);
            }
        }
        // The motions' lengths for s6 (wait for a motion): every (character, motion, division)
        // the event plays, measured from the clips and the divisions before scheduling.
        let mut lengths: std::collections::HashMap<(u32, u32, Option<u32>), (f32, bool)> = Default::default();
        {
            use barrel_game::event::Step;
            let mut current: std::collections::HashMap<u32, u32> = Default::default();
            let mut wanted = Vec::new();
            for step in barrel_game::event::Player::escha(event, &messages, &barrel_formats::ebd::OperandTable::escha())?.hero(hero) {
                match step {
                    Step::Motion { character, motion, division, .. } => {
                        current.insert(character, motion);
                        wanted.push((character, motion, division));
                    }
                    Step::MotionDivision { character, division, .. } => {
                        if let Some(&motion) = current.get(&character) {
                            wanted.push((character, motion, Some(division)));
                        }
                    }
                    _ => {}
                }
            }
            for (character, motion, division) in wanted {
                let Some(a) = actors.iter_mut().find(|a| a.who == character) else { continue };
                let Some(column) = motion_names.get(motion as usize) else { continue };
                let Some(duration) = a.motions.get(column).map(|c| c.duration) else { continue };
                let found = division.and_then(|k| *divisions.get(&(a.motions.head.clone(), column.clone()))?.get(k as usize)?);
                let length = match found {
                    Some(d) => (d.end.unwrap_or(duration).min(duration) - d.start).max(0.0),
                    None => duration,
                };
                lengths.insert((character, motion, division), (length, found.is_some_and(|d| d.loops)));
            }
        }
        // Lines held for a reading time, as nobody presses a button here.
        let steps = barrel_game::event::Player::escha(event, &messages, &barrel_formats::ebd::OperandTable::escha())?
            .hero(hero)
            .reading_time(barrel_game::event::reading_time)
            .motion_length(Box::new(move |c, m, d| lengths.get(&(c, m, d)).copied()))
            .schedule();
        // BARREL_SCHEDULE_LOG: the times this playback shows each narration card, line and field
        // (they differ from `barrel play-event --timed`, which has no motion lengths).
        if std::env::var("BARREL_SCHEDULE_LOG").is_ok() {
            use barrel_game::event::Step;
            for t in &steps {
                match &t.step {
                    Step::Telop { card, .. } => eprintln!("{:8.2} card {card}", t.time),
                    Step::Message { index, .. } => eprintln!("{:8.2} line {index}", t.time),
                    Step::FieldChange { map, .. } => eprintln!("{:8.2} field {map}", t.time),
                    _ => {}
                }
            }
        }
        still_pictures = steps
            .iter()
            .filter_map(|t| match t.step {
                barrel_game::event::Step::Still { picture, .. } => Some(picture),
                _ => None,
            })
            .collect();
        telop_cards = steps
            .iter()
            .filter_map(|t| match t.step {
                barrel_game::event::Step::Telop { card, .. } => Some(card),
                _ => None,
            })
            .collect();
        let mut play = Playback::new(Stage { measures, curves, reading: Some(barrel_game::event::reading_time), ..Stage::default() }, steps);
        face_tables_play = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_game::face::Tables::read(&pe)).filter(|t| !t.brow.is_empty()));
        gestures_play = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_game::gesture::read(&b))
            .unwrap_or_default();
        // Frames written to files are not real time: no stall cap (a window keeps it).
        let mut clock = barrel_game::clock::Clock::default();
        if presenter.is_none() {
            clock.max_frame = f64::INFINITY;
        }
        while clock.time() + 1e-9 < event_at.unwrap_or(0.0) as f64 {
            clock.ticks += 1;
            play.tick();
        }
        let snap = play.stage.snapshot();
        eprintln!("playing {frames} frames at {fps} Hz from {:.2} s, {} actors", clock.time(), actors.len());
        player = Some((play, clock, snap.clone(), snap));
        if presenter.is_some() {
            let tracks = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))
                .ok()
                .and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().and_then(|pe| barrel_formats::bgm::tracks(&pe, barrel_formats::bgm::TRACKS_EN)))
                .unwrap_or_default();
            match barrel_audio::Output::open(0.1) {
                Ok(out) => sound = Some((out, barrel_audio::Mixer::new(), barrel_audio::Library::new(root, "Voice", tracks), None)),
                Err(e) => eprintln!("no sound: {e}"),
            }
        }
        // The event's lines in the game's English font.
        let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).ok();
        let glyphs = exe.as_ref().and_then(|b| barrel_formats::pe::Pe::parse(b).ok().and_then(|pe| barrel_formats::font::glyphs(&pe, barrel_formats::font::FONT_EN)));
        let atlas = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t")).ok();
        let common = std::fs::read(root.join("Data/x64/WinXls_EN/a15_common.g1t")).ok();
        let ui = common.as_ref().and_then(|c| barrel_formats::g1t::G1t::parse(c).ok());
        // The narration cards' pieces and their WinXls_EN textures (slots after the font and
        // the window).
        let mut sheet_names: Vec<String> = Vec::new();
        let mut card_pieces: Vec<(u32, Vec<barrel_formats::layout::Piece>)> = Vec::new();
        if let Some(pe) = exe.as_ref().and_then(|b| barrel_formats::pe::Pe::parse(b).ok()) {
            for &card in &telop_cards {
                let Some(id) = barrel_formats::layout::telop_layout(&pe, card) else { continue };
                let pieces = barrel_formats::layout::pieces(&pe, id);
                for p in &pieces {
                    if !sheet_names.contains(&p.texture) {
                        sheet_names.push(p.texture.clone());
                    }
                }
                card_pieces.push((card, pieces));
            }
        }
        // The still pictures: Data/x64/Ipu_EN first (the English ones), else Ipu.
        let mut still_list: Vec<(u32, barrel_formats::layout::Still)> = Vec::new();
        if let Some(pe) = exe.as_ref().and_then(|b| barrel_formats::pe::Pe::parse(b).ok()) {
            for &n in &still_pictures {
                if let Some(st) = barrel_formats::layout::still(&pe, n) {
                    if !still_list.iter().any(|(k, _)| *k == n) {
                        still_list.push((n, st));
                    }
                }
            }
        }
        let still_bytes: Vec<Option<Vec<u8>>> = still_list
            .iter()
            .map(|(_, st)| {
                let x64 = root.join("Data/x64");
                std::fs::read(x64.join(format!("{}_EN", st.folder)).join(&st.file)).or_else(|_| std::fs::read(x64.join(&st.folder).join(&st.file))).ok()
            })
            .collect();
        let still_g1t: Vec<Option<barrel_formats::g1t::G1t>> = still_bytes.iter().map(|b| b.as_ref().and_then(|b| barrel_formats::g1t::G1t::parse(b).ok())).collect();
        let sheet_bytes: Vec<Option<Vec<u8>>> = sheet_names.iter().map(|n| std::fs::read(root.join("Data/x64/WinXls_EN").join(n)).ok()).collect();
        let sheets: Vec<Option<barrel_formats::g1t::G1t>> = sheet_bytes.iter().map(|b| b.as_ref().and_then(|b| barrel_formats::g1t::G1t::parse(b).ok())).collect();
        match (glyphs, atlas.as_ref().and_then(|a| barrel_formats::g1t::G1t::parse(a).ok()), ui.as_ref().and_then(|u| u.textures.get(4))) {
            (Some(glyphs), Some(g), Some(window)) if !g.textures.is_empty() => {
                let size = g.textures[0].width as f32;
                let mut images = vec![&g.textures[0], window];
                // Each sheet's slot in the layer (None: missing).
                let mut slots = Vec::new();
                for sheet in &sheets {
                    match sheet.as_ref().and_then(|s| s.textures.first()) {
                        Some(t) => {
                            slots.push(Some((images.len(), t.width as f32, t.height as f32)));
                            images.push(t);
                        }
                        None => slots.push(None),
                    }
                }
                for ((n, st), g) in still_list.iter().zip(&still_g1t) {
                    if let Some(t) = g.as_ref().and_then(|g| g.textures.first()) {
                        still_slots.insert(*n, (images.len(), st.size));
                        images.push(t);
                    }
                }
                for (card, pieces) in &card_pieces {
                    let list = pieces
                        .iter()
                        .filter_map(|p| {
                            let (slot, w, h) = slots[sheet_names.iter().position(|n| *n == p.texture)?]?;
                            let [l, t, r, b] = p.src.map(f32::from);
                            let [x, y] = p.at.map(|v| v as f32);
                            Some((slot, [l / w, t / h, r / w, b / h], [x, y, x + r - l, y + b - t]))
                        })
                        .collect();
                    telop_pieces.insert(*card, list);
                }
                text_layer = Some((text::Layer::new(&gpu, pool, post_pass, &images)?, text::Font { glyphs, size }, None));
            }
            _ => eprintln!("no text: the English executable's glyph table, the Font_EN atlas or WinXls_EN/a15_common.g1t is missing"),
        }
    }
    // The field drawn (its place in `field_maps`): where the event is when playback starts,
    // else the first.
    let mut current_field: u16 = player
        .as_ref()
        .and_then(|p| p.0.stage.field)
        .and_then(|m| field_maps.iter().position(|&f| f == m))
        .map_or(0, |k| k as u16);
    let start_ticks = player.as_ref().map_or(0, |p| p.1.ticks);
    let mut hidden = vec![false; instances.len()];
    // The field object versions shown (q15): none until the event shows one.
    let mut versions_shown = player.as_ref().map(|p| p.0.stage.versions_shown.clone()).unwrap_or_default();
    // Whether the field's xEV_ objects show (q10): only while an event shows them.
    let mut event_objects = player.as_ref().is_some_and(|p| p.0.stage.event_objects);
    for a in &actors {
        hidden[a.instance as usize] = match &player {
            Some((_, _, _, now)) => !now.characters.contains_key(&a.who),
            None => absent.contains(&a.who),
        };
    }

    // Render each frame: read it back to a file, or present it in the window.
    let bytes = (width * height * 4) as u64;
    let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
    let sync = presenter
        .as_ref()
        .map(|_| -> Result<(vk::Semaphore, vk::Semaphore)> {
            unsafe { Ok((d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?, d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?)) }
        })
        .transpose()?;
    let mut last_frame = std::time::Instant::now();
    let mut stale = false;
    let mut paused = false;
    // The fades' presets (u22), from the executable.
    let fade_presets = event_spec
        .as_ref()
        .and_then(|e| Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf))
        .and_then(|r| std::fs::read(r.join("Atelier_Escha_and_Logy_EN.exe")).ok())
        .and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_formats::field_look::fade_presets(&pe, barrel_formats::field_look::FADE_PRESETS_EN)))
        .unwrap_or_default();
    // The start time of the last screen effect seen (a new one starts a cross-fade).
    let mut last_effect: Option<f32> = None;
    let mut prev_mvp: Option<Mat4> = None;
    let mut jitter_index = 0u32;
    let mut frame_jitter = (0f32, 0f32);
    let mut last_shot: Option<taa::Shot> = None;
    let mut shot_field = u16::MAX;
    for frame in 0..frames {
    // Last frame's transforms and joints, before this frame moves anything (motion vectors).
    for i in &mut instances {
        i.prev_model = i.model;
    }
    prev_palette.copy_from_slice(&palette);
    // The real time since the last frame (a window's frames are real time).
    let now = std::time::Instant::now();
    let frame_dt = (now - last_frame).as_secs_f64().min(0.25) as f32;
    last_frame = now;
    if let Some(p) = pump.as_mut() {
        if !p.pump() {
            eprintln!("window closed after {frame} frames");
            break;
        }
        // Space pauses the event, Escape quits; arrows turn the camera about its target
        // and + / - move it in and out (R puts it back).
        use winit::keyboard::KeyCode;
        let mut quit = false;
        for key in p.take_pressed() {
            match key {
                KeyCode::Space => {
                    paused = !paused;
                    eprintln!("frame {frame}: {}", if paused { "paused" } else { "playing" });
                }
                KeyCode::Escape => quit = true,
                KeyCode::KeyR => orbit = (0.0, 0.0, 1.0),
                _ => {}
            }
        }
        if quit {
            eprintln!("quit after {frame} frames");
            break;
        }
        let axis = |neg: KeyCode, pos: KeyCode| (p.held(pos) as i32 - p.held(neg) as i32) as f32;
        orbit.0 += axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) * 1.5 * frame_dt;
        orbit.1 = (orbit.1 + axis(KeyCode::ArrowDown, KeyCode::ArrowUp) * 1.0 * frame_dt).clamp(-1.2, 1.2);
        let zoom_in = axis(KeyCode::Minus, KeyCode::Equal) + axis(KeyCode::NumpadSubtract, KeyCode::NumpadAdd);
        orbit.2 = (orbit.2 * (1.0 - zoom_in * 1.2 * frame_dt)).clamp(0.1, 10.0);
    }
    if let Some((play, clock, before, after)) = player.as_mut() {
        if frame > 0 {
            // A window takes the real time since the last frame; files take 1/fps.
            let real = if presenter.is_some() {
                if paused { 0.0 } else { frame_dt as f64 }
            } else {
                1.0 / fps
            };
            let f = clock.advance(real);
            for _ in 0..f.ticks {
                *before = std::mem::replace(after, {
                    play.tick();
                    play.stage.snapshot()
                });
                let cues = std::mem::take(&mut play.stage.cues);
                if let Some((_, mixer, library, music)) = sound.as_mut() {
                    for cue in cues {
                        match cue {
                            barrel_game::stage::Cue::Music { track: Some(t), volume, .. } => {
                                if let Some(m) = music.take() {
                                    mixer.stop(m);
                                }
                                if let Some(m) = library.music(t) {
                                    eprintln!("music {t}");
                                    *music = Some(mixer.play_stream(m.stream, m.volume * volume, m.loop_from));
                                }
                            }
                            barrel_game::stage::Cue::Music { track: None, fade, .. } => {
                                if let Some(m) = music.take() {
                                    mixer.fade_out(m, fade);
                                }
                            }
                            barrel_game::stage::Cue::Effect { effect, volume, pan } => {
                                if let Some(e) = library.effect(effect) {
                                    let p = match e {
                                        barrel_audio::Effect::Clip(s) => mixer.play(s, volume, None),
                                        barrel_audio::Effect::Stream(s, from) => mixer.play_stream(s, volume, from),
                                    };
                                    mixer.set_pan(p, pan);
                                }
                            }
                            barrel_game::stage::Cue::Voice { clip } => {
                                if let Some(v) = library.voice(clip) {
                                    mixer.play(v, 1.0, None);
                                }
                            }
                        }
                    }
                }
            }
            if let Some((out, mixer, ..)) = sound.as_mut() {
                out.pump(mixer);
            }
            versions_shown.clone_from(&play.stage.versions_shown);
            event_objects = play.stage.event_objects;
            // The field the event is in now.
            if let Some(k) = play.stage.field.and_then(|m| field_maps.iter().position(|&f| f == m)) {
                if k as u16 != current_field {
                    eprintln!("frame {frame}: field {}", field_maps[k]);
                    current_field = k as u16;
                }
            }
            if let Some((layer, font, last)) = text_layer.as_mut() {
                let line = play.stage.line_shown().cloned();
                // The narration card and its opacity (in 1/1000ths, so a fade redraws each step).
                let card = play.stage.telop.filter(|_| play.stage.telop_alpha.now() > 0.0);
                let card_alpha = (play.stage.telop_alpha.now().clamp(0.0, 1.0) * 1000.0) as u16;
                // The still (and the one it fades in over), with its opacity in 1/1000ths.
                let still = play.stage.still.map(|(n, before, since, seconds)| {
                    let a = if seconds > 0.0 { ((play.stage.time - since) / seconds).clamp(0.0, 1.0) } else { 1.0 };
                    (n, before, (a * 1000.0) as u16)
                });
                let now = Some((line.clone(), width, height, card, card_alpha, still));
                if now != *last {
                    // The still picture first (under the window and the text): its size on a
                    // 1920 × 1080 screen, scaled to the height and centred.
                    let mut quads = Vec::new();
                    if let Some((n, before, a)) = still {
                        let k = height as f32 / 1080.0;
                        let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
                        let a = a as f32 / 1000.0;
                        let layers = [(before.filter(|_| a < 1.0), 1.0), (Some(n), a)];
                        for (picture, alpha) in layers {
                            if let Some(&(texture, [w, h])) = picture.and_then(|p| still_slots.get(&p)) {
                                let rect = [cx - w * k / 2.0, cy - h * k / 2.0, cx + w * k / 2.0, cy + h * k / 2.0];
                                quads.push(text::Quad { rect, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, alpha], texture });
                            }
                        }
                    }
                    quads.extend(match &line {
                        Some(l) => {
                            // The name the line gives (the speaker-name table: "???" before a
                            // character is introduced), else the speaker's own.
                            let name = l
                                .name
                                .and_then(|k| usize::try_from(k).ok())
                                .and_then(|k| window_names.get(k))
                                .or_else(|| usize::try_from(l.speaker).ok().and_then(|k| speaker_names.get(k)))
                                .map(String::as_str);
                            if std::env::var("BARREL_TEXT_LOG").is_ok() {
                                eprintln!("frame {frame}: line {:?}: {}", name, l.text);
                            }
                            font.dialogue(name, &l.text, width as f32, height as f32)
                        }
                        None => Vec::new(),
                    });
                    // The card: the 1920 × 1080 layout scaled to the height and centred.
                    if let Some(pieces) = card.and_then(|c| telop_pieces.get(&c)) {
                        let k = height as f32 / 1080.0;
                        let dx = (width as f32 - 1920.0 * k) / 2.0;
                        let a = card_alpha as f32 / 1000.0;
                        if std::env::var("BARREL_TEXT_LOG").is_ok() {
                            eprintln!("frame {frame}: narration card {:?} at {a:.2}", card);
                        }
                        for &(texture, uv, [l, t, r, b]) in pieces {
                            quads.push(text::Quad { rect: [dx + l * k, t * k, dx + r * k, b * k], uv, color: [1.0, 1.0, 1.0, a], texture });
                        }
                    }
                    layer.set(&gpu, &quads)?;
                    *last = now;
                }
            }
            let shown = barrel_game::stage::Snapshot::blend(before, after, f.alpha);
            // The event's camera as the stage has it (game space, x mirrored like the
            // vertices); the opening one until the event sets one.
            if let (Some(e), Some(t), true) = (shown.eye, shown.target, use_event_camera) {
                let mirror = Vec3::new(-1.0, 1.0, 1.0);
                (eye, target) = (Vec3::from(e) * mirror, Vec3::from(t) * mirror);
                // A camera quake: each axis a random whole number of field units between its two
                // tracks' values (metres) at the quake's frame, added to the eye and the target
                // (0x140035920; which camera values the game moves is taken, not read).
                if let Some((number, since, seconds)) = play.stage.quake {
                    let elapsed = play.stage.time - since;
                    if let (Some(q), true) = (quakes.iter().find(|q| q.number() == Some(number)), elapsed <= seconds) {
                        let frame = elapsed * 60.0;
                        let mut shake = [0.0f32; 3];
                        for (k, axis) in shake.iter_mut().enumerate() {
                            if let (Some(a), Some(b)) = (q.tracks.get(2 * k), q.tracks.get(2 * k + 1)) {
                                let (lo, hi) = ((a.at(frame) * 100.0) as i32, (b.at(frame) * 100.0) as i32);
                                *axis = quake_rng.range(lo, hi) as f32;
                            }
                        }
                        let offset = Vec3::from(shake) * mirror;
                        (eye, target) = (eye + offset, target + offset);
                    }
                }
                dist = (eye - target).length().max(1.0);
                if let Some(f) = shown.fov {
                    fov = f.clamp(5.0, 120.0).to_radians();
                }
            }
            let since = ((clock.ticks - start_ticks) as f32 + f.alpha) / clock.rate() as f32;
            // The time on screen: the logic time less the part of the last tick not yet shown.
            let shown_time = play.stage.time - (1.0 - f.alpha) / clock.rate() as f32;
            for a in actors.iter_mut() {
                let gone = !shown.characters.contains_key(&a.who);
                if gone != hidden[a.instance as usize] {
                    eprintln!("frame {frame}: character {} {}", a.who, if gone { "leaves" } else { "appears" });
                }
                hidden[a.instance as usize] = gone;
                // The stage's motion for the character, by its MOTION_* name; a new one is
                // loaded (once) and played from the time the stage started it.
                let staged = play.stage.characters.get(&a.who).and_then(|b| b.motion);
                // A line's gesture, when newer than the motion: its chain of motion divisions
                // plays from then on (until the next motion command or gesture), the last (or a
                // looping one) held.
                let gestured = play.stage.characters.get(&a.who).and_then(|b| b.gesture).and_then(|(g, since)| {
                    let form = gestures_play.get(usize::try_from(g).ok()?)?[0].as_ref()?;
                    let ((motion, division), began) = form.at(shown_time - since, |m, d| {
                        let column = motion_names.get(m as usize)?;
                        let clip = a.motions.get(column)?.duration;
                        match d.and_then(|k| *divisions.get(&(a.motions.head.clone(), column.clone()))?.get(k as usize)?) {
                            Some(div) => Some((div.end.unwrap_or(clip) - div.start, div.loops)),
                            None => Some((clip, false)),
                        }
                    });
                    Some(barrel_game::stage::Playing { motion, since: since + began, looping: true, blend: 0.2, division })
                });
                let staged = match (staged, gestured) {
                    (Some(m), Some(g)) if g.since > m.since => Some(g),
                    (None, Some(g)) => Some(g),
                    (m, _) => m,
                };
                let mut t = a.start + since;
                // Blend weight of the current motion against the one before it.
                let mut weight = 1.0;
                if let Some(m) = staged {
                    if let Some(column) = motion_names.get(m.motion as usize) {
                        if *column != a.column {
                            if let Some(anim) = a.motions.get(column).cloned() {
                                eprintln!("frame {frame}: character {} plays {column} (blend {:.2} s)", a.who, m.blend);
                                let old = std::mem::replace(&mut a.anim, anim);
                                a.previous = (m.blend > 0.0).then_some((old, a.last_t, a.looping));
                            }
                            a.column = column.clone();
                            a.looping = m.looping;
                        }
                        t = shown_time - m.since;
                        // Its division, if the command names one the tables have.
                        a.division = m.division.and_then(|k| {
                            *divisions.get(&(a.motions.head.clone(), column.clone()))?.get(k as usize)?
                        });
                        if let (Some(k), true) = (m.division, *column != a.last_division_column) {
                            eprintln!("frame {frame}: character {} {column} division {k}: {:?}", a.who, a.division);
                            a.last_division_column = column.clone();
                        }
                        if m.blend > 0.0 {
                            weight = (t / m.blend).clamp(0.0, 1.0);
                        }
                    }
                }
                a.last_t = t;
                if let Some(&(pos, facing)) = shown.characters.get(&a.who) {
                    instances[a.instance as usize].model =
                        render_space(Mat4::from_rotation_translation(glam::Quat::from_rotation_y(facing), Vec3::from(pos)));
                }
                let base = instances[a.instance as usize].palette as usize;
                let clock = match a.division {
                    Some(d) => d.clock(t, a.anim.duration),
                    None => motion_clock(&a.anim, t, a.looping),
                };
                let mut locals = local_pose(&a.skeleton, &a.anim, clock);
                match (&a.previous, weight < 1.0) {
                    // The old motion runs on from where it was while the new one fades in.
                    (Some((old, at_switch, old_loops)), true) => {
                        let before = local_pose(&a.skeleton, old, motion_clock(old, at_switch + t.max(0.0), *old_loops));
                        locals = mix_pose(&before, &locals, weight);
                    }
                    (Some(_), false) => a.previous = None,
                    _ => {}
                }
                // A head gesture (s116): its segment's pose laid over the joints it turns, at its
                // weight (the game lerps the joints' transforms, 0x1404177a0).
                if let Some((segment, rate, w, since)) = play.stage.characters.get(&a.who).and_then(|b| b.head) {
                    if a.head.is_none() {
                        a.head = a.motions.head_gestures();
                    }
                    if let Some((clip, joints)) = &a.head {
                        let (from, to) = barrel_game::event::HEAD_SEGMENTS[segment];
                        let at = (from / 30.0 + ((shown_time - since).max(0.0) * rate)).min(to / 30.0);
                        let over = local_pose(&a.skeleton, clip, at);
                        for &j in joints {
                            if let (Some(base), Some(top)) = (locals.get(j).copied(), over.get(j)) {
                                locals[j] = (base.0.lerp(top.0, w), base.1.slerp(top.1, w), base.2.lerp(top.2, w));
                            }
                        }
                    }
                }
                let joints = skin(&a.skeleton, &locals);
                // The face the stage gives the character now (a talking mouth moves).
                if let (Some(rig), Some(tables), Some(b)) = (a.face.as_mut(), &face_tables_play, play.stage.characters.get(&a.who)) {
                    let face = tables.expression(rig.row, &rig.head, b.expression.unwrap_or(0), b.talking);
                    let base = instances[a.instance as usize].weights as usize;
                    let total: usize = rig.sets.iter().map(|(n, _)| n).sum();
                    if let Some(block) = morph_weights.get_mut(base..base + total) {
                        rig.weights(&face, b.talking, shown_time, block);
                        if std::env::var("BARREL_FACE_LOG").is_ok() {
                            let on: f32 = block.iter().sum();
                            eprintln!("frame {frame}: character {} face {:?}{} weight sum {on:.3}", a.who, b.expression, if b.talking { " talking" } else { "" });
                        }
                    }
                }
                for (k, m) in joints.iter().enumerate() {
                    if let Some(slot) = palette.get_mut(base + k) {
                        *slot = render_space(*m);
                    }
                }
            }
            gpu.upload(instance_mem, unsafe { std::slice::from_raw_parts(instances.as_ptr() as *const u8, instances.len() * size_of::<Instance>()) })?;
            gpu.upload(weight_mem, unsafe { std::slice::from_raw_parts(morph_weights.as_ptr() as *const u8, morph_weights.len() * 4) })?;
            gpu.upload(palette_mem, palette_bytes(&palette))?;
            gpu.upload_at(palette_mem, half, palette_bytes(&prev_palette))?;
        }
    }
    // A resized window, or a swapchain gone stale: remake it and the framebuffers.
    if let (Some(p), Some(pp)) = (presenter.as_mut(), pump.as_mut()) {
        if pp.take_resized() || stale {
            let size = pp.window().inner_size();
            if size.width == 0 || size.height == 0 {
                continue; // minimised
            }
            p.rebuild(&gpu, size.width, size.height)?;
            targets.destroy(&gpu);
            (width, height) = (p.extent.width, p.extent.height);
            targets = Targets::new(&gpu, render_pass, post_pass, &colour_views(&presenter), width, height, settings.scene_size(width, height), post_set, post_sampler, &glow_pipelines, upscale_pipelines.as_ref(), taa_pipelines.as_ref(), gsr2_pipelines.as_ref(), fsr3_pipelines.as_ref(), samples, settings.motion, show_motion)?;
            stale = false;
            eprintln!("frame {frame}: window now {width}x{height}");
        }
    }
    // The image to draw into: the window's next, or the offscreen one.
    let image = match (&presenter, &sync) {
        (Some(p), Some((ready, _))) => match p.acquire(*ready)? {
            Some(i) => i as usize,
            None => {
                stale = true;
                continue;
            }
        },
        _ => 0,
    };
    let mvp = view_proj(eye, target, dist, fov, width, height, orbit);
    // A camera cut (or a new field): the last frame's view says nothing about this one, so TAA
    // starts its history afresh and the motion vectors take this frame's view for the last.
    let shot = taa::Shot { eye, target, fov };
    if last_shot.is_some_and(|before| taa::is_cut(before, shot)) || shot_field != current_field {
        if last_shot.is_some() {
            eprintln!("frame {frame}: camera cut");
            prev_mvp = None;
            targets.history_fresh.set(true);
        }
        shot_field = current_field;
    }
    last_shot = Some(shot);
    // A camera that has not moved since the last frame (GSR 2 keeps more of its history then).
    let same_camera = prev_mvp.is_some_and(|p| p.abs_diff_eq(mvp, 1e-5));
    if std::env::var_os("BARREL_SHOTS").is_some() {
        eprintln!("frame {frame}: shot {shot:?}");
    }
    // The view-projection drawn with: shifted by this frame's sub-pixel jitter when it is on
    // (`settings::jitter`, a clip-space translation by the offset in pixels × 2 / the scene's
    // size), the plain one otherwise.
    let draw_vp = if settings.jitter {
        // FSR 3.1 expects its own cycle length (8 × the upscale ratio squared).
        let (sw, _) = targets.scene_size;
        let (jx, jy) = if settings.fsr3 { settings::jitter_cycle(jitter_index, fsr3::jitter_phase_count(sw as i32, width as i32) as u32) } else { settings::jitter(jitter_index) };
        frame_jitter = (jx, jy);
        let (sw, sh) = targets.scene_size;
        let mut j = Mat4::IDENTITY;
        j.w_axis.x = 2.0 * jx / sw as f32;
        j.w_axis.y = 2.0 * jy / sh as f32;
        jitter_index += 1;
        j * mvp
    } else {
        mvp
    };
    // With motion vectors, this frame's and last frame's transforms and joints every frame (a
    // frame that moves nothing still has last frame's movement to forget).
    if settings.motion {
        gpu.upload(instance_mem, unsafe { std::slice::from_raw_parts(instances.as_ptr() as *const u8, instances.len() * size_of::<Instance>()) })?;
        gpu.upload(palette_mem, palette_bytes(&palette))?;
        gpu.upload_at(palette_mem, half, palette_bytes(&prev_palette))?;
    }
    // The sun's shadow map (deliberately one square around the camera's target: the game's
    // cascades are not read yet, docs/renderer.md "Shadows").
    let sun_dir = field_looks
        .get(current_field as usize)
        .and_then(|l| l.as_ref())
        .and_then(|l| {
            l.lights.iter().find_map(|p| match *p {
                barrel_formats::field_look::Light::Parallel { rot, .. } => {
                    let (p, y) = (rot[0].to_radians(), rot[1].to_radians());
Some(Vec3::new(-(p.cos() * y.sin()), -p.sin(), p.cos() * y.cos()))
                }
                _ => None,
            })
        })
        .unwrap_or(Vec3::NEG_Y);
    let shadow_vp = shadow::sun_view_proj(target, sun_dir, SHADOW_HALF, 6000.0);
    // A cross-fade (u13 kind 3): the frame it starts on keeps the last frame as it was, then
    // the live scene fades in over it.
    let (freeze_now, frozen_weight) = match player.as_ref().and_then(|p| p.0.stage.screen_effect.map(|e| (e, p.0.stage.time))) {
        Some(((3, _, seconds, since), now)) => {
            let fresh = last_effect != Some(since);
            if fresh {
                eprintln!("frame {frame}: cross-fade over {seconds} s");
            }
            last_effect = Some(since);
            let t = if seconds > 0.0 { ((now - since) / seconds).clamp(0.0, 1.0) } else { 1.0 };
            (fresh && frame > 0, 1.0 - t)
        }
        _ => (false, 0.0),
    };
    // The fade's colour over the screen now (u22): from the preset's first colour to its second.
    let overlay: [f32; 4] = match player.as_ref().and_then(|p| p.0.stage.fade.map(|f| (f, p.0.stage.time))) {
        Some(((preset, seconds, since), now)) => match fade_presets.get(preset as usize) {
            Some((from, to)) => {
                let t = if seconds > 0.0 { ((now - since) / seconds).clamp(0.0, 1.0) } else { 1.0 };
                std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t)
            }
            None => [0.0; 4],
        },
        None => [0.0; 4],
    };
    // The current field's distant view follows the eye (plus the field's sphere offset).
    if let Some(Some(inst)) = sky_instances.get(current_field as usize) {
        let offset = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).and_then(|l| l.sphere_offset).unwrap_or([0.0; 3]);
        let at = turned_eye(eye, target, orbit) + Vec3::new(-offset[0], offset[1], offset[2]) * 100.0;
        instances[*inst as usize].model = Mat4::from_translation(at);
        gpu.upload(instance_mem, unsafe { std::slice::from_raw_parts(instances.as_ptr() as *const u8, instances.len() * size_of::<Instance>()) })?;
    }
    // The field's character lighting, mirrored into render space, and the eye.
    {
        let field_look = field_looks.get(current_field as usize).and_then(|l| l.as_ref());
        let chara = field_look.and_then(|l| l.chara);
        // A field with none: a light from above and in front, no colour or rim.
        let c = chara.unwrap_or(barrel_formats::field_look::CharaShader {
            light_dir: [0.0, -1.0, -1.0],
            light_color: [1.0, 1.0, 1.0, 0.0],
            rim_light_dir: [0.0, -1.0, 1.0],
            rim_light_color: [1.0, 1.0, 1.0, 0.0],
            point_light_strength: 0.0,
        });
        let m = |d: [f32; 3]| [-d[0], d[1], d[2], 0.0];
        let e = turned_eye(eye, target, orbit);
        let mut look = [[0f32; 4]; 48];
        // Last frame's view-projection (this frame's on the first), for the motion vectors.
        for (k, col) in prev_mvp.unwrap_or(mvp).to_cols_array_2d().into_iter().enumerate() {
            look[40 + k] = col;
        }
        // This frame's view-projection without the jitter, for the motion vectors.
        for (k, col) in mvp.to_cols_array_2d().into_iter().enumerate() {
            look[44 + k] = col;
        }
        // The billboards' axes (docs/shading.md, "Billboards"): the camera's right as the screen
        // shows it (render space is mirrored, so the view's -x), up, and back toward the eye.
        let cam = Mat4::look_at_rh(e, target, Vec3::Y);
        let row = |k: usize, sign: f32| [sign * cam.row(k).x, sign * cam.row(k).y, sign * cam.row(k).z, 0.0];
        look[37] = row(0, -1.0);
        look[38] = row(1, 1.0);
        look[39] = row(2, 1.0);
        look[..5].copy_from_slice(&[m(c.light_dir), c.light_color, m(c.rim_light_dir), c.rim_light_color, [e.x, e.y, e.z, 1.0]]);
        // The field's first four point lights (metres to field units, x mirrored).
        let points = field_look.map_or(Vec::new(), |l| {
            l.lights
                .iter()
                .filter_map(|p| match *p {
                    barrel_formats::field_look::Light::Point { pos, color, ambient, att, range } => Some((pos, color, ambient, att, range)),
                    _ => None,
                })
                .take(4)
                .collect::<Vec<_>>()
        });
        look[5] = [c.point_light_strength, points.len() as f32, 0.0, 0.0];
        // The field's two directional lights: rot is taken as pitch (down from level) then yaw,
        // in degrees, the light travelling along that direction (the game's convention is not
        // read).
        let parallels: Vec<([f32; 3], [f32; 4])> = field_look.map_or(Vec::new(), |l| {
            l.lights
                .iter()
                .filter_map(|p| match *p {
                    barrel_formats::field_look::Light::Parallel { rot, color } => Some((rot, color)),
                    _ => None,
                })
                .collect()
        });
        for (k, slot) in [22usize, 24].into_iter().enumerate() {
            if let Some((rot, color)) = parallels.get(k) {
                let (p, y) = (rot[0].to_radians(), rot[1].to_radians());
                look[slot] = m([p.cos() * y.sin(), -p.sin(), p.cos() * y.cos()]);
                look[slot + 1] = *color;
            }
        }
        look[26] = field_look.and_then(|l| l.ambient).unwrap_or([0.6, 0.6, 0.6, 1.0]);
        // The fog, as the game turns the field's <fog> into its shader constants (read from
        // its memory: docs/shading.md, "Fog"); none: nothing taken off or added.
        if let Some(f) = field_look.and_then(|l| l.fog) {
            let (start, end) = (f.start * 100.0, f.end * 100.0);
            let span = (end - start).max(1.0);
            look[32] = [1.0 / span, -start / span, 0.0, 0.0];
            let sun = parallels.first().map_or([1.0; 4], |p| p.1);
            let (r, m, g) = (f.rayleigh_rgb, f.mie_rgb, f.mie_g);
            let ext = |i: usize| r[i] + m[i];
            let part = |x: f32, i: usize| if ext(i) > 0.0 { x / ext(i) } else { 0.0 };
            let pi = std::f32::consts::PI;
            look[33] = [-ext(0) * std::f32::consts::LOG2_E, -ext(1) * std::f32::consts::LOG2_E, -ext(2) * std::f32::consts::LOG2_E, 0.0];
            look[34] = [look[26][0] * f.ambient, look[26][1] * f.ambient, look[26][2] * f.ambient, 1.0];
            let k = 3.0 / (16.0 * pi) * f.rayleigh_pow;
            look[35] = [k * sun[0] * part(r[0], 0), k * sun[1] * part(r[1], 1), k * sun[2] * part(r[2], 2), -2.0 * g];
            let h = f.rayleigh_pow * (1.0 - g) * (1.0 - g) / (4.0 * pi);
            look[36] = [h * sun[0] * part(m[0], 0), h * sun[1] * part(m[1], 1), h * sun[2] * part(m[2], 2), 1.0 + g * g];
        }
        // The sun's shadow map: from the first directional light, over the camera's target.
        // On unless BARREL_NO_SHADOWS is set (for comparing).
        if !parallels.is_empty() && std::env::var("BARREL_NO_SHADOWS").is_err() {
            for (k, col) in shadow_vp.to_cols_array_2d().into_iter().enumerate() {
                look[27 + k] = col;
            }
            look[31] = [1.0 / shadow_target.size as f32, 1.0, 0.0, 0.0];
        }
        for (k, (pos, color, ambient, att, range)) in points.iter().enumerate() {
            // The range in field units too (squared); the attenuation counts in field units.
            look[6 + k] = [-pos[0] * 100.0, pos[1] * 100.0, pos[2] * 100.0, (range * 100.0) * (range * 100.0)];
            look[10 + k] = *color;
            look[14 + k] = [att[0], att[1], att[2], 0.0];
            look[18 + k] = *ambient;
        }
        gpu.upload(look_mem, unsafe { std::slice::from_raw_parts(look.as_ptr() as *const u8, LOOK_SIZE as usize) })?;
    }
    prev_mvp = Some(mvp);
    let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width, height } };
    let (scene_width, scene_height) = targets.scene_size;
    let scene_area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: scene_width, height: scene_height } };
    let frozen_ready = targets.frozen_ready;
    targets.frozen_ready = true;
    let record = |cb: vk::CommandBuffer| unsafe {
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
        // The sun's shadow map: every caster but the distant view, from the sun.
        {
            let clear = [vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }];
            let full = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: shadow_target.size, height: shadow_target.size } };
            d.cmd_begin_render_pass(
                cb,
                &vk::RenderPassBeginInfo::default().render_pass(shadow_target.pass).framebuffer(shadow_target.framebuffer).render_area(full).clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: shadow_target.size as f32, height: shadow_target.size as f32, min_depth: 0.0, max_depth: 1.0 }]);
            d.cmd_set_scissor(cb, 0, &[full]);
            d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
            d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, shadow_pipeline);
            let mut push = [0f32; 31];
            push[..16].copy_from_slice(&shadow_vp.to_cols_array());
            push[16] = alpha_cutoff;
            let stages_pc = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize));
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
            for draw in draws.iter().filter(|d| d.casts && !d.sky && !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && d.version.is_none_or(|v| versions_shown.contains(&v)) && (event_objects || !d.name.contains("xEV_")) && !hide_by_name(&d.name)) {
                // xEF_ field models (window light: TOWN_03's obj-xEF_windows_tw03) are not lit, flag
            // 32: their texture on their vertex colour, as the distant view's clouds.
            let flags = u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1;
                let mut per_draw = [0u8; 8];
                per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
                per_draw[4..].copy_from_slice(&flags.to_le_bytes());
                d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
                d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
            }
            d.cmd_end_render_pass(cb);
        }
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
        let mut push = [0f32; 31];
        push[..16].copy_from_slice(&draw_vp.to_cols_array());
        push[16] = alpha_cutoff;
        push[18] = width as f32 / height.max(1) as f32;
        push[19] = HDR_RANGE_INV;
        if let Some(tg) = &terrain_gpu {
            for g in tg.grounds.iter().filter(|g| g.field == current_field) {
                push[17] = g.field_size;
                push[20..26].copy_from_slice(&g.scales);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, tg.pipeline);
                d.cmd_push_constants(cb, tg.pipeline_layout, stages_pc, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize));
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, tg.pipeline_layout, 0, &[g.set, instance_set], &[]);
                d.cmd_draw_indexed(cb, g.index_count, 1, g.first_index, 0, 0);
            }
        }
        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
        let pc = std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize);
        d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
        // The current field's distant view first, then the rest over it.
        for (pass_sky, pipe) in [(true, sky_pipeline), (false, pipeline)] {
        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipe);
        for draw in draws.iter().filter(|d| d.sky == pass_sky && !hidden[d.instance as usize] && d.field.is_none_or(|f| f == current_field) && d.version.is_none_or(|v| versions_shown.contains(&v)) && (event_objects || !d.name.contains("xEV_")) && !hide_by_name(&d.name)) {
            // The instance, and whether it is shaded as a character (not part of a field) and
            // as an eye.
            // The distant view's solid pieces (all but the dome and the clouds) are lit and
            // alpha-tested like the field's models (docs/renderer.md, "Distant view").
            let solid = draw.sky && !draw.name.contains("Sphere_sky") && !draw.name.contains("xEF_");
            let flags = u32::from(draw.field.is_none()) | u32::from(draw.eye) << 1 | u32::from(draw.sky) << 3 | u32::from(solid) << 4 | u32::from(!draw.sky && draw.name.contains("xEF_")) << 5;
            let mut per_draw = [0u8; 8];
            per_draw[..4].copy_from_slice(&draw.instance.to_le_bytes());
            per_draw[4..].copy_from_slice(&flags.to_le_bytes());
            d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &per_draw);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[sets[draw.texture]], &[]);
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 2, &[sets[draw.mask.unwrap_or(white)]], &[]);
            d.cmd_draw_indexed(cb, draw.index_count, 1, draw.first_index, 0, 0);
        }
        }
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
        // The glow from the scene's bright parts (the field's <hdr>; none: no glow).
        let (threshold, rate) = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).and_then(|l| l.hdr).unwrap_or((1.0, 0.0));
        if let Some(g) = &targets.glow {
            g.record(d, &glow_pipelines, cb, threshold);
        }
        if let (Some(t), Some(p)) = (&targets.taa, &taa_pipelines) {
            let fresh = targets.history_fresh.replace(false);
            t.record(d, p, cb, fresh, fresh);
        }
        if let (Some(f), Some(p)) = (targets.fsr3.borrow_mut().as_mut(), &fsr3_pipelines) {
            let fresh = targets.history_fresh.replace(false);
            let zoom = orbit.2;
            let frame = fsr3::Frame {
                render: targets.scene_size,
                upscale: (width, height),
                jitter: frame_jitter,
                fov,
                near: dist * 0.01 * zoom,
                far: (dist * 20.0 * zoom).max(80_000.0),
                dt: frame_dt,
                reset: fresh,
            };
            if let Err(e) = f.record(&gpu, p, cb, &frame) {
                eprintln!("FSR 3: {e}");
            }
        }
        if let (Some(g), Some(p)) = (&targets.gsr2, &gsr2_pipelines) {
            let fresh = targets.history_fresh.replace(false);
            let frame = gsr2::Frame { jitter: frame_jitter, fov, reset: fresh, same_camera };
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
        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, post_layout, 0, &[post_set], &[]);
        let mut post_push = [0u8; 32];
        post_push[..4].copy_from_slice(&rate.to_le_bytes());
        post_push[4..8].copy_from_slice(&frozen_weight.to_le_bytes());
        post_push[8..12].copy_from_slice(&(if show_motion { 1.0f32 } else { 0.0 }).to_le_bytes());
        for (k, v) in overlay.iter().enumerate() {
            post_push[16 + 4 * k..20 + 4 * k].copy_from_slice(&v.to_le_bytes());
        }
        d.cmd_push_constants(cb, post_layout, vk::ShaderStageFlags::FRAGMENT, 0, &post_push);
        d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: width as f32, height: height as f32, min_depth: 0.0, max_depth: 1.0 }]);
        d.cmd_set_scissor(cb, 0, &[area]);
        d.cmd_draw(cb, 3, 1, 0, 0);
        if let Some((layer, ..)) = &text_layer {
            layer.record(d, cb, width, height);
        }
        d.cmd_end_render_pass(cb);
        if let Some(t) = &timer {
            t.mark(d, cb, 4);
        }
        if let Some((color, _)) = offscreen {
            let region = vk::BufferImageCopy::default()
                .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                .image_extent(extent);
            d.cmd_copy_image_to_buffer(cb, color, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, readback, &[region]);
        }
    };
    if let (Some(p), Some((ready, done))) = (&presenter, &sync) {
        // Wait for the image, draw, signal, present; the fence keeps one frame in flight.
        let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1))?[0] };
        unsafe {
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            record(cb);
            d.end_command_buffer(cb)?;
            let fence = d.create_fence(&vk::FenceCreateInfo::default(), None)?;
            let (waits, stages, cbs, signals) = ([*ready], [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT], [cb], [*done]);
            d.queue_submit(
                gpu.queue,
                &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)],
                fence,
            )?;
            stale |= p.present(&gpu, image as u32, *done)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
            d.destroy_fence(fence, None);
            d.free_command_buffers(pool, &cbs);
        }
        if let Some(t) = timer.as_mut() {
            t.collect(&gpu)?;
        }
        continue;
    }
    gpu.submit(pool, record)?;
    if let Some(t) = timer.as_mut() {
        t.collect(&gpu)?;
    }
    let mut pixels = vec![0u8; bytes as usize];
    unsafe {
        let p = d.map_memory(rmem, 0, bytes, vk::MemoryMapFlags::empty())? as *const u8;
        std::ptr::copy_nonoverlapping(p, pixels.as_mut_ptr(), pixels.len());
        d.unmap_memory(rmem);
    }
    // One frame: OUT.png; several: OUT_000.png, OUT_001.png, ...
    let path = if frames > 1 {
        let p = Path::new(&out_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
        p.with_file_name(format!("{stem}_{frame:03}.png"))
    } else {
        Path::new(&out_path).to_path_buf()
    };
    std::fs::write(&path, barrel_formats::png::encode_rgba(width, height, &pixels))?;
    eprintln!("wrote {}", path.display());
    }

    // Tear down in reverse order.
    unsafe {
        d.device_wait_idle()?;
        d.destroy_buffer(readback, None);
        d.free_memory(rmem, None);
        if let Some((layer, ..)) = text_layer {
            layer.destroy(&gpu);
        }
        if let Some(tg) = terrain_gpu {
            d.destroy_pipeline(tg.pipeline, None);
            d.destroy_pipeline_layout(tg.pipeline_layout, None);
            d.destroy_shader_module(tg.shader, None);
            d.destroy_descriptor_pool(tg.pool, None);
            d.destroy_descriptor_set_layout(tg.layout, None);
            for (image, mem, view) in tg.grounds.into_iter().flat_map(|g| g.textures) {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
        }
        d.destroy_pipeline(post_pipeline, None);
        d.destroy_pipeline_layout(post_layout, None);
        d.destroy_shader_module(post_vs, None);
        d.destroy_shader_module(post_fs, None);
        d.destroy_descriptor_pool(post_pool, None);
        d.destroy_descriptor_set_layout(post_set_layout, None);
        d.destroy_sampler(post_sampler, None);
        d.destroy_render_pass(post_pass, None);
        d.destroy_pipeline(outline_pipeline, None);
        d.destroy_pipeline(shadow_pipeline, None);
        d.destroy_shader_module(shadow_fs, None);
        d.destroy_pipeline(sky_pipeline, None);
        d.destroy_pipeline(pipeline, None);
        d.destroy_pipeline_layout(pipeline_layout, None);
        d.destroy_shader_module(vs, None);
        d.destroy_shader_module(fs, None);
        targets.destroy(&gpu);
        glow_pipelines.destroy(&gpu);
        if let Some(p) = upscale_pipelines {
            p.destroy(&gpu);
        }
        if let Some(p) = taa_pipelines {
            p.destroy(&gpu);
        }
        if let Some(p) = gsr2_pipelines {
            p.destroy(&gpu);
        }
        if let Some(p) = fsr3_pipelines {
            p.destroy(&gpu);
        }
        if let Some(t) = timer.take() {
            t.report();
            t.destroy(&gpu);
        }
        d.destroy_render_pass(render_pass, None);
        if let Some(v) = color_view {
            d.destroy_image_view(v, None);
        }
        if let Some((color, cmem)) = offscreen {
            d.destroy_image(color, None);
            d.free_memory(cmem, None);
        }
        if let Some((ready, done)) = sync {
            d.destroy_semaphore(ready, None);
            d.destroy_semaphore(done, None);
        }
        d.destroy_descriptor_pool(dpool, None);
        d.destroy_descriptor_set_layout(set_layout, None);
        d.destroy_sampler(sampler, None);
        for (image, mem, view) in textures {
            d.destroy_image_view(view, None);
            d.destroy_image(image, None);
            d.free_memory(mem, None);
        }
        d.destroy_buffer(look_buf, None);
        d.free_memory(look_mem, None);
    }
    shadow_target.destroy(&gpu);
    unsafe {
        for (b, m) in [(instance_buf, instance_mem), (palette_buf, palette_mem), (delta_buf, delta_mem), (weight_buf, weight_mem)] {
            d.destroy_buffer(b, None);
            d.free_memory(m, None);
        }
        d.destroy_descriptor_pool(inst_pool, None);
        d.destroy_descriptor_set_layout(instance_layout, None);
        d.destroy_buffer(vbuf, None);
        d.free_memory(vmem, None);
        d.destroy_buffer(ibuf, None);
        d.free_memory(imem, None);
        d.destroy_command_pool(pool, None);
        if let Some(p) = presenter {
            p.destroy(&gpu);
        }
        d.destroy_device(None);
        gpu.instance.destroy_instance(None);
    }
    let _ = gpu.physical;
    Ok(())
}
