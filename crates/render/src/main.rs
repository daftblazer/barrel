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

const VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.vert.spv"));
const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.frag.spv"));
const TERRAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/terrain.frag.spv"));
/// Push constants: mvp, params (alpha cutoff, field size), two vec4 of layer tile scales.
/// view_proj (64), params, scale_a, scale_b (48), then the draw's instance number (4).
const PUSH_SIZE: u32 = 116;
const PUSH_INSTANCE: u32 = 112;
const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

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
}

/// A model's placement on the GPU (std430: 80 bytes): its matrix and where its joints'
/// matrices start in the palette.
#[repr(C)]
#[derive(Clone, Copy)]
struct Instance {
    model: Mat4,
    palette: u32,
    _pad: [u32; 3],
}

struct Draw {
    first_index: u32,
    index_count: u32,
    texture: usize,
    /// A texture to lay over `texture` by its alpha (the iris over the eye white).
    overlay: Option<usize>,
    /// Which of [`Scene::instances`] places it.
    instance: u32,
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
    terrain: Option<TerrainDraw>,
    lo: Vec3,
    hi: Vec3,
    /// Per vertex layout, offsets to add to the next model's positions before skinning
    /// (its blended facial morphs).
    morph: std::collections::HashMap<usize, Vec<[f32; 3]>>,
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
            vertices: Vec::new(),
            indices: Vec::new(),
            draws: Vec::new(),
            terrain: None,
            instances: vec![Instance { model: Mat4::IDENTITY, palette: 0, _pad: [0; 3] }],
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
        self.instances.push(Instance { model: render_space(transform), palette: 0, _pad: [0; 3] });
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
            let morph = self.morph.get(&s.layout);
            for i in 0..mesh.positions.len() {
                let (mut pos, nrm) = (Vec3::from(mesh.positions[i]), Vec3::from(mesh.normals[i]));
                if let Some(d) = morph.and_then(|m| m.get(mesh.first_vertex + i)) {
                    pos += Vec3::from(*d);
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
                self.vertices.push(Vertex { pos: p.into(), normal: n.into(), uv: mesh.uvs[i], joints: joint_ix, weights: weight_q });
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
            self.draws.push(Draw { first_index, index_count: self.indices.len() as u32 - first_index, texture, overlay, instance });
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
                self.vertices.push(Vertex { pos: p.into(), normal: n.into(), uv, joints: [0; 4], weights: [0; 4] });
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
    /// The time shown at the last frame, in the current motion.
    last_t: f32,
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
}

impl Motions {
    fn new(root: &Path, model: &str, row: &barrel_formats::table::Element) -> Option<Motions> {
        let dir = model.rsplit_once('/').map_or("", |(d, _)| d);
        Some(Motions {
            char_dir: root.join("Data/x64/Character").join(dir),
            head: row.attr("FileHead")?.to_string(),
            container: row.attr("Motion1")?.to_string(),
            columns: row.attributes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            loaded: Default::default(),
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
                    pitch: i(2) / 100.0,
                    fov: 40.0,
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
        let steps = barrel_game::event::Player::escha(&event, &[], &ops)?.schedule();
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
        if let (Some(&(expr, talking)), Some(tables), Some(head), Some(facial)) =
            (faces.get(&(who as i32)), &face_tables, row.attr("FileHead"), row.attr("Facial"))
        {
            let face = tables.expression(row_index as u32, head, expr, talking);
            let dir = model.rsplit_once('/').map_or("", |(d, _)| d);
            let fbytes = std::fs::read(root.join("Data/x64/Character").join(dir).join(format!("{facial}.elixir.gz")));
            if let (Ok(fbytes), Some(g1h)) = (fbytes, named(&container, &format!("{stem}_default.g1h"))) {
                let facials = Elixir::parse(&fbytes)?;
                let mut anims = Vec::new();
                for (name, t) in [(&face.brow, 0.5), (&face.eye, 0.5), (&face.mouth, 1.0)] {
                    if let Some(bytes) = name.as_ref().and_then(|n| named(&facials, &format!("{n}.g1a"))) {
                        anims.push((bytes, t));
                    }
                }
                eprintln!("character {who}: expression {expr}{} {face:?}", if talking { ", talking" } else { "" });
                set_face(scene, g1h, &anims)?;
            }
        }
        let instance = scene.add_with_pack(geometry, transform, tex_base, count, joints.as_deref())?;
        if let Some((skeleton, motions, anim, start)) = rig {
            actors.push(Actor { who, instance, skeleton, motions, column: column.clone(), anim, start, looping, previous: None, last_t: start });
        }
        scene.morph.clear();
        eprintln!("character {who} ({tag}) at {pos:?}, {angle} degrees");
        centre += Vec3::from(pos) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0);
        placed += 1.0;
        tex_base += count;
        packs.push(tex.to_vec());
    }
    Ok((packs, (placed > 0.0).then(|| centre / placed), camera, placements))
}

/// Where in `anim` to sample at motion time `t`: looping motions wrap, others hold their end.
fn motion_clock(anim: &barrel_formats::g1a::Animation, t: f32, looping: bool) -> f32 {
    if looping && anim.duration > 0.0 { t.rem_euclid(anim.duration) } else { t.clamp(0.0, anim.duration) }
}

/// Blend facial animations (at `time`) into `scene.morph` for the next model added: each
/// animation goes to the morph set with one target fewer than it has tracks.
fn set_face(scene: &mut Scene, g1h: &[u8], anims: &[(&[u8], f32)]) -> Result<()> {
    let sets = barrel_formats::g1h::parse(g1h)?;
    for (bytes, time) in anims {
        let anim = barrel_formats::g1a::parse(bytes)?;
        let Some(set) = sets.iter().find(|s| s.targets.len() + 1 == anim.tracks.len()) else { continue };
        let t = time.min(anim.duration);
        let weight =
            |bone: usize| anim.tracks.iter().find(|tr| tr.bone as usize == bone).and_then(|tr| tr.scalar(t)).unwrap_or(0.0);
        scene.morph.insert(set.layout as usize, set.blend(weight));
    }
    Ok(())
}

/// The entry of `container` named `name`, if any.
fn named<'a>(container: &'a Elixir, name: &str) -> Option<&'a [u8]> {
    container.entries.iter().find(|e| e.name == name && !e.is_dummy()).map(|e| container.data(e))
}

/// A field's placed objects and its shared texture pack.
fn field_scene(dir: &Path, scene: &mut Scene) -> Result<Vec<u8>> {
    let area = dir.file_name().ok_or("field directory has no name")?.to_string_lossy().into_owned();
    let main_bytes = std::fs::read(dir.join(format!("{area}.elixir.gz")))?;
    let main = Elixir::parse(&main_bytes)?;
    let obj_bytes = std::fs::read(dir.join(format!("{area}_Obj.elixir.gz")))?;
    let objects = Elixir::parse(&obj_bytes)?;
    let textures = named(&main, &format!("{area}.g1t")).ok_or("field has no texture pack")?.to_vec();
    let texture_count = G1t::parse(&textures)?.textures.len();
    if let Some(bytes) = named(&main, &format!("{area}.ktf")) {
        let t = barrel_formats::ktf::parse(bytes)?;
        let h = &t.heightmap;
        let (first_index, index_count) = scene.add_heightmap(h);
        let mut scales = [0.01f32; 6];
        for (i, s) in t.layer_scales().into_iter().take(6).enumerate() {
            scales[i] = s;
        }
        eprintln!("{area}: terrain {}x{} cells of {}, {} layers, scales {scales:?}", h.cells_x, h.cells_z, h.cell_size, t.layers.textures.len());
        scene.terrain = Some(TerrainDraw {
            first_index,
            index_count,
            field_size: h.cells_x as f32 * h.cell_size,
            scales,
            ktf: bytes.to_vec(),
        });
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
            scene.add(geometry, transform, texture_count)?;
            placed += 1;
        }
    }
    eprintln!("{area}: {placed} objects placed, {missing} without a model");
    Ok(textures)
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
    let field = args.first().is_some_and(|a| a == "--field");
    let positional = if field { 3 } else { 3 };
    if args.len() < positional {
        return Err("usage: barrel-view MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png | --field DIR OUT.png  [--size N] [--yaw DEG] [--pitch DEG]".into());
    }
    let out_path = if field { args[2].clone() } else { args[2].clone() };
    let mut size = 768u32;
    let mut yaw = 0f32;
    let mut pitch = if field { 45f32 } else { 0f32 };
    let mut alpha_cutoff = 0.3f32;
    let (mut skeleton_spec, mut anim_spec, mut time) = (None::<String>, None::<String>, 0f32);
    let mut event_spec = None::<String>;
    let mut event_line = None::<usize>;
    let mut event_at = None::<f32>;
    let (mut frames, mut fps) = (1u32, 60f64);
    let mut look_at = None::<u32>;
    let (mut morph_spec, mut face_specs) = (None::<String>, Vec::<String>::new());
    let mut use_event_camera = true;
    let mut it = args[positional..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--size" => size = it.next().ok_or("--size N")?.parse::<u32>()?.clamp(64, 1280),
            "--yaw" => yaw = it.next().ok_or("--yaw DEG")?.parse()?,
            "--no-alpha" => alpha_cutoff = -1.0,
            "--skeleton" => skeleton_spec = Some(it.next().ok_or("--skeleton FILE#N")?.clone()),
            "--anim" => anim_spec = Some(it.next().ok_or("--anim FILE#N")?.clone()),
            "--time" => time = it.next().ok_or("--time SECONDS")?.parse()?,
            "--morph" => morph_spec = Some(it.next().ok_or("--morph FILE#N")?.clone()),
            "--face" => face_specs.push(it.next().ok_or("--face FILE#N")?.clone()),
            "--look-at" => look_at = Some(it.next().ok_or("--look-at CHARA")?.parse()?),
            "--line" => event_line = Some(it.next().ok_or("--line N")?.parse()?),
            "--at" => event_at = Some(it.next().ok_or("--at SECONDS")?.parse()?),
            "--frames" => frames = it.next().ok_or("--frames N")?.parse::<u32>()?.clamp(1, 600),
            "--fps" => fps = it.next().ok_or("--fps HZ")?.parse::<f64>()?.clamp(1.0, 1000.0),
            "--event" => event_spec = Some(it.next().ok_or("--event FILE.ebd")?.clone()),
            "--free-camera" => use_event_camera = false,
            "--pitch" => pitch = it.next().ok_or("--pitch DEG")?.parse::<f32>()?.clamp(-89.0, 89.0),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    // CPU side: the vertices and draws, and the texture pack.
    let mut scene = Scene::new();
    let tex_bytes = if field {
        field_scene(Path::new(&args[1]), &mut scene)?
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
        let instance = if joints.is_some() { scene.instance(Mat4::IDENTITY) } else { 0 };
        scene.add_posed(geometry, Mat4::IDENTITY, count, joints.as_deref(), instance)?;
        scene.morph.clear();
        tex_bytes
    };
    let mut pack_bytes = vec![tex_bytes];
    let mut focus = None;
    let mut event_camera: Option<EventCamera> = None;
    let mut actors: Vec<Actor> = Vec::new();
    // Characters loaded but not on the stage (yet, or any more): not drawn.
    let mut absent: std::collections::HashSet<u32> = Default::default();
    if let Some(e) = &event_spec {
        let base = G1t::parse(&pack_bytes[0])?.textures.len();
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
    let Scene { vertices, indices, mut draws, terrain, lo, hi, mut instances, mut palette, .. } = scene;
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
    for t in &all_textures {
        textures.push(upload_texture(&gpu, pool, t)?);
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

    // Instances: one model matrix each, in a storage buffer the vertex shader indexes by the
    // draw's instance number (set 1, bound once for every pipeline).
    let ibytes_inst = unsafe { std::slice::from_raw_parts(instances.as_ptr() as *const u8, instances.len() * size_of::<Instance>()) };
    let (instance_buf, instance_mem) = gpu.buffer(ibytes_inst.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
    gpu.upload(instance_mem, ibytes_inst)?;
    // And the joint palette (binding 1).
    let pbytes = unsafe { std::slice::from_raw_parts(palette.as_ptr() as *const u8, palette.len() * size_of::<Mat4>()) };
    let (palette_buf, palette_mem) = gpu.buffer(pbytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
    gpu.upload(palette_mem, pbytes)?;
    let storage = |b: u32| {
        vk::DescriptorSetLayoutBinding::default()
            .binding(b)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
    };
    let inst_binding = [storage(0), storage(1)];
    let instance_layout = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&inst_binding), None)? };
    let inst_sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 2 }];
    let inst_pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&inst_sizes), None)? };
    let inst_layouts = [instance_layout];
    let instance_set =
        unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(inst_pool).set_layouts(&inst_layouts))?[0] };
    let inst_info = [vk::DescriptorBufferInfo::default().buffer(instance_buf).offset(0).range(vk::WHOLE_SIZE)];
    let pal_info = [vk::DescriptorBufferInfo::default().buffer(palette_buf).offset(0).range(vk::WHOLE_SIZE)];
    unsafe {
        d.update_descriptor_sets(
            &[
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&inst_info),
                vk::WriteDescriptorSet::default().dst_set(instance_set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&pal_info),
            ],
            &[],
        )
    };

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
        vk::VertexInputAttributeDescription { location: 3, binding: 0, format: vk::Format::R16G16B16A16_UINT, offset: 32 },
        vk::VertexInputAttributeDescription { location: 4, binding: 0, format: vk::Format::R8G8B8A8_UNORM, offset: 40 },
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
    let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, offset: 0, size: PUSH_SIZE }];
    let set_layouts = [set_layout, instance_layout];
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

    // Terrain: its own textures, a set of 2 blend maps + 6 layers, and a pipeline with the
    // terrain fragment shader.
    let mut terrain_gpu = None;
    if let Some(t) = &terrain {
        let bundle = barrel_formats::ktf::parse(&t.ktf)?;
        let mut tex = Vec::new();
        for b in bundle.blend.textures.iter().take(2) {
            tex.push(upload_texture(&gpu, pool, b)?);
        }
        let layer_count = bundle.layers.textures.len().clamp(1, 6);
        for i in 0..6 {
            tex.push(upload_texture(&gpu, pool, &bundle.layers.textures[i.min(layer_count - 1)])?);
        }
        if tex.len() != 8 {
            return Err("terrain needs two blend maps".into());
        }
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
        let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 8 }];
        let tpool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None)? };
        let tl = [tlayout];
        let tpl_layouts = [tlayout, instance_layout];
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
        let tpl = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&tpl_layouts).push_constant_ranges(&push), None)? };
        let tfs = module(TERRAIN_FRAG)?;
        let tstages = [stages[0], vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(tfs).name(c"main")];
        let info = pipeline_info.stages(&tstages).layout(tpl);
        let tpipe = unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|(_, e)| e)?[0] };
        terrain_gpu = Some((tex, tlayout, tpool, tset, tpl, tfs, tpipe));
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
        let back = Vec3::new(yw.sin() * pt.cos(), pt.sin(), -yw.cos() * pt.cos());
        let mirror = Vec3::new(-1.0, 1.0, 1.0);
        target = c.target * mirror;
        eye = c.eye.unwrap_or(c.target + back * c.distance) * mirror;
        fov = c.fov.clamp(10.0, 120.0).to_radians();
        dist = c.distance;
        eprintln!("event camera: target {:?}, distance {}, yaw {}, pitch {}, fov {}", c.target, c.distance, c.yaw, c.pitch, c.fov);
    }
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let mut proj = Mat4::perspective_rh(fov, 1.0, dist * 0.01, dist * 20.0);
    proj.y_axis.y *= -1.0; // Vulkan's y points down.
    let mvp = proj * view;

    // With --frames N, play the event on: the clock at --fps HZ decides the logic ticks
    // (docs/engine.md, "Timing"), each frame draws the last two ticks blended, and only the
    // characters' instance matrices and joint palettes change between frames.
    let mut player = None;
    let mut motion_names: Vec<String> = Vec::new();
    let event_file = if frames > 1 { event_spec.as_ref().map(|e| std::fs::read(e)).transpose()? } else { None };
    let event_data = event_file.as_ref().map(|b| barrel_formats::ebd::Ebd::parse(b)).transpose()?;
    if let (Some(event), Some(e)) = (&event_data, &event_spec) {
        use barrel_game::stage::{Measures, Playback, Stage};
        let root = Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
        let xml = |rel: &str| std::fs::read(root.join(rel)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
        let measures = xml("Saves/chara/Chara.xml").map(|t| Measures::table(&t)).unwrap_or_default();
        let curves = xml("Saves/curve/curve_data.xml").map(|t| barrel_game::curve::Curve::table(&t)).unwrap_or_default();
        let steps = barrel_game::event::Player::escha(event, &[], &barrel_formats::ebd::OperandTable::escha())?.schedule();
        let mut play = Playback::new(Stage { measures, curves, ..Stage::default() }, steps);
        motion_names = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_game::gesture::motions(&pe)).filter(|m| !m.is_empty()))
            .unwrap_or_default();
        // Frames written to files are not real time: no stall cap.
        let mut clock = barrel_game::clock::Clock::default();
        clock.max_frame = f64::INFINITY;
        while clock.time() + 1e-9 < event_at.unwrap_or(0.0) as f64 {
            clock.ticks += 1;
            play.tick();
        }
        let snap = play.stage.snapshot();
        eprintln!("playing {frames} frames at {fps} Hz from {:.2} s, {} actors", clock.time(), actors.len());
        player = Some((play, clock, snap.clone(), snap));
    }
    let start_ticks = player.as_ref().map_or(0, |p| p.1.ticks);
    let mut hidden = vec![false; instances.len()];
    for a in &actors {
        hidden[a.instance as usize] = match &player {
            Some((_, _, _, now)) => !now.characters.contains_key(&a.who),
            None => absent.contains(&a.who),
        };
    }

    // Render each frame and read it back.
    let bytes = (size * size * 4) as u64;
    let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
    for frame in 0..frames {
    if let Some((play, clock, before, after)) = player.as_mut() {
        if frame > 0 {
            let f = clock.advance(1.0 / fps);
            for _ in 0..f.ticks {
                *before = std::mem::replace(after, {
                    play.tick();
                    play.stage.snapshot()
                });
            }
            let shown = barrel_game::stage::Snapshot::blend(before, after, f.alpha);
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
                let mut locals = local_pose(&a.skeleton, &a.anim, motion_clock(&a.anim, t, a.looping));
                match (&a.previous, weight < 1.0) {
                    // The old motion runs on from where it was while the new one fades in.
                    (Some((old, at_switch, old_loops)), true) => {
                        let before = local_pose(&a.skeleton, old, motion_clock(old, at_switch + t.max(0.0), *old_loops));
                        locals = mix_pose(&before, &locals, weight);
                    }
                    (Some(_), false) => a.previous = None,
                    _ => {}
                }
                let joints = skin(&a.skeleton, &locals);
                for (k, m) in joints.iter().enumerate() {
                    if let Some(slot) = palette.get_mut(base + k) {
                        *slot = render_space(*m);
                    }
                }
            }
            gpu.upload(instance_mem, unsafe { std::slice::from_raw_parts(instances.as_ptr() as *const u8, instances.len() * size_of::<Instance>()) })?;
            gpu.upload(palette_mem, unsafe { std::slice::from_raw_parts(palette.as_ptr() as *const u8, palette.len() * size_of::<Mat4>()) })?;
        }
    }
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
        d.cmd_bind_vertex_buffers(cb, 0, &[vbuf], &[0]);
        d.cmd_bind_index_buffer(cb, ibuf, 0, vk::IndexType::UINT32);
        let stages_pc = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        // view_proj, params, the terrain's scales; the instance number goes in per draw.
        let mut push = [0f32; 29];
        push[..16].copy_from_slice(&mvp.to_cols_array());
        push[16] = alpha_cutoff;
        if let (Some(t), Some((_, _, _, tset, tpl, _, tpipe))) = (&terrain, &terrain_gpu) {
            push[17] = t.field_size;
            push[20..26].copy_from_slice(&t.scales);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, *tpipe);
            d.cmd_push_constants(cb, *tpl, stages_pc, 0, std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize));
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, *tpl, 0, &[*tset, instance_set], &[]);
            d.cmd_draw_indexed(cb, t.index_count, 1, t.first_index, 0, 0);
        }
        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
        let pc = std::slice::from_raw_parts(push.as_ptr() as *const u8, PUSH_SIZE as usize);
        d.cmd_push_constants(cb, pipeline_layout, stages_pc, 0, pc);
        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 1, &[instance_set], &[]);
        for draw in draws.iter().filter(|d| !hidden[d.instance as usize]) {
            d.cmd_push_constants(cb, pipeline_layout, stages_pc, PUSH_INSTANCE, &draw.instance.to_le_bytes());
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
    // One frame: OUT.png; several: OUT_000.png, OUT_001.png, ...
    let path = if frames > 1 {
        let p = Path::new(&out_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
        p.with_file_name(format!("{stem}_{frame:03}.png"))
    } else {
        Path::new(&out_path).to_path_buf()
    };
    std::fs::write(&path, barrel_formats::png::encode_rgba(size, size, &pixels))?;
    eprintln!("wrote {}", path.display());
    }

    // Tear down in reverse order.
    unsafe {
        d.device_wait_idle()?;
        d.destroy_buffer(readback, None);
        d.free_memory(rmem, None);
        if let Some((tex, tlayout, tpool, _, tpl, tfs, tpipe)) = terrain_gpu {
            d.destroy_pipeline(tpipe, None);
            d.destroy_pipeline_layout(tpl, None);
            d.destroy_shader_module(tfs, None);
            d.destroy_descriptor_pool(tpool, None);
            d.destroy_descriptor_set_layout(tlayout, None);
            for (image, mem, view) in tex {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
        }
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
