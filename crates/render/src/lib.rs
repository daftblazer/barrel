//! Barrel's Vulkan renderer: the device, a scene's geometry, textures and draws, the terrain,
//! shadows, glow, anti-aliasing and the upscalers, the text and 2D layer, the window, the pad
//! and the graphics settings. It draws what a game hands it and knows no game's rules
//! (`docs/renderer.md`).

use std::ffi::CStr;
use std::path::Path;

use ash::vk;
use barrel_formats::g1t::{Format, G1t};
use glam::{Mat4, Vec3};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub mod fsr;
pub mod glow;
pub mod fsr3;
pub mod gsr2;
pub mod present;
pub mod renderer;
pub mod ring;
pub mod scene_gpu;
pub mod settings;
pub mod shadow;
pub mod taa;
pub mod timing;
pub mod text;
pub mod upscale;
pub mod window;
pub mod pad;
pub mod graphics_options;

pub const VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.vert.spv"));
pub const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.frag.spv"));
pub const SKY_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv"));
pub const SKY_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv"));
pub const TERRAIN_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/terrain.frag.spv"));
pub const SHADOW_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.frag.spv"));
/// Push constants: mvp, params (alpha cutoff, field size), two vec4 of layer tile scales.
/// view_proj (64), params, scale_a, scale_b (48), then the draw's instance number (4).
pub const PUSH_SIZE: u32 = 128;
/// The glow's bright-pass threshold: `PostEffectGlow.kps` program 0's `fHdrThreshold` default.
/// The field table's `<hdr threshold>` is read by the game (`0x1400690f1`) but its setter
/// (`0x140287470`) stores it and ends in a stub, and no code sets the constant by name.
pub const GLOW_THRESHOLD: f32 = 0.5;
pub const PUSH_INSTANCE: u32 = 112;
/// The field look's uniform block (see `model.frag`): 6 vec4s, 4 point lights × 4 vec4s, then
/// the field's sun, fill and ambient (5 vec4s).
/// The look block's slots (vec4s; model.vert's `Look`, then the sky plane's 11 from
/// `SKY_SLOT`, sky.frag).
/// Then the far shadow square: its view-projection (87 to 90) and {texel, layer (0: none),
/// fade start, fade end} (91; the fade by distance from the eye, to lit).
/// Then the light on an upward-facing surface ({rgb}, 92: `model.vert`'s flag 4096, a sky dome
/// lit so).
/// Then where billboards shrink away ({begin, end} from the eye, `model.vert`'s flag 16384: a
/// field's plants).
/// Then the emission maps' strength ({scale}, `model.frag`'s flag 32768).
pub const LOOK_SLOTS: usize = 95;
pub const EMISSION_SLOT: usize = 94;
pub const DOME_LIGHT_SLOT: usize = 92;
pub const FADE_SLOT: usize = 93;
/// The far shadow layer's first look slot.
pub const SHADOW_FAR_SLOT: usize = 87;
/// The first of the sky plane's look slots (sky.frag): {on}, Scat[4], skyPos, fog, the sun's
/// way, the glow's direction, colour and parameters.
pub const SKY_SLOT: usize = 76;
pub const LOOK_SIZE: u64 = LOOK_SLOTS as u64 * 16;
/// Half the side of the sun's shadow map square, in field units (calibrated against oracle
/// frames; docs/renderer.md, "Shadows").
pub const SHADOW_HALF: f32 = 1500.0;
pub const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
/// The scene's colour target: 8 bits a channel, as the game's (it draws at the HDR range's
/// inverse and doubles the result after: `docs/shading.md`, "After the scene").
pub const SCENE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
/// The motion vectors' format (`--motion`): x and y in texture coordinates.
pub const MOTION_FORMAT: vk::Format = vk::Format::R16G16_SFLOAT;
/// HdrRangeInv: the scene is drawn at half its brightness and doubled by the post pass.
pub const HDR_RANGE_INV: f32 = 0.5;
pub const POST_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.vert.spv"));
pub const POST_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/post.frag.spv"));


#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Up to four joints of the model's skeleton (indices into its instance's palette) and
    /// their weights out of 255; all weights 0 for geometry that is not skinned.
    pub joints: [u16; 4],
    pub weights: [u8; 4],
    /// Facial morph targets: where this vertex's offsets begin in the delta buffer, then
    /// (target count | weight offset << 16) into its instance's weights; 0 targets: none.
    pub morph: [u32; 2],
    /// The vertex colour (field models bake their light into it); white when it has none.
    pub color: [u8; 4],
    /// A billboard corner (`MapSTD-B*`, PSIZE): the vertex is its quad's centre and this
    /// offset, already scaled with the model, turns to face the camera; zero otherwise.
    pub corner: [f32; 3],
}

/// A model's placement on the GPU (std430: 80 bytes): its matrix and where its joints'
/// matrices start in the palette.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Instance {
    pub model: Mat4,
    pub palette: u32,
    /// Where its morph weights begin in the weight buffer.
    pub weights: u32,
    /// Where its last frame's joints begin in the palette buffer (the palette's second half),
    /// for the motion vectors.
    pub prev_palette: u32,
    pub _pad: u32,
    /// Its last frame's model matrix, for the motion vectors.
    pub prev_model: Mat4,
}

pub struct Draw {
    pub first_index: u32,
    pub index_count: u32,
    pub texture: usize,
    /// A texture to lay over `texture` by its alpha (the iris over the eye white).
    pub overlay: Option<usize>,
    /// Which of [`Scene::instances`] places it.
    pub instance: u32,
    /// The material's mask (`docs/shading.md`, "Which texture is which"), if it has one; for
    /// an eye, its highlights.
    pub mask: Option<usize>,
    /// The material's normal map (its kind-3 texture), if it has one. Drawn only where a game
    /// puts it in the mask slot (`model.frag`'s flag 8192, Ryza's field models).
    pub normal_map: Option<usize>,
    /// The material's emission map (its kind-19 texture), if it has one: bound as the draw's
    /// set 3 (white otherwise), drawn only where a game asks (`model.frag`'s flag 32768).
    pub emission: Option<usize>,
    /// Shaded as an eye (`CharaEYE`: `docs/shading.md`, "Eyes").
    pub eye: bool,
    /// Part of a field's distant view (unlit: its texture on its vertex colour).
    pub sky: bool,
    /// The outline pass's colour (the material's `atOutCol`, grey), when the material has an
    /// outline (an `atEdge` parameter).
    pub outline: Option<f32>,
    /// The field (in the order the scene loaded them) it belongs to; None for what is drawn
    /// in every field (the event's characters).
    pub field: Option<u16>,
    /// Drawn into the sun's shadow map: the characters, and the field objects marked `xDS_`
    /// (in their name or their placement's tags; docs/renderer.md, "Shadows").
    pub casts: bool,
    /// What it is part of, for `BARREL_HIDE` (a field object's placement name, `sky:<model>`
    /// for the distant view, empty for characters).
    pub name: String,
    /// The version a field object belongs to (its placement tag `xAM_…_ver<N>`): drawn only
    /// while the event shows that version (`q15`, `Stage::versions_shown`).
    pub version: Option<u32>,
    /// Which of its object's levels of detail it is (`LOD_HI`, `LOD_MID`, `LOD_LOW`, with
    /// `LOD_LAST` on the object's last one), when the field draws them by distance; 0 for a
    /// draw always shown.
    pub lod: u8,
    /// Its model's lighting material (`cDiff`, `cAmb`: g1m section 0x10001, grey in every
    /// field model), packed as two unorm16 for the push constants; [`FULL_LIGHT`] without one.
    pub light: u32,
    /// Its `atColor` set at run time (unorm8 RGBA; 0: none, the material's white).
    pub tint: u32,
    /// A field object's bounds, for leaving it out when it is wholly outside the view
    /// (`docs/renderer.md`, "View culling"); None (always drawn) for characters and the sky.
    pub bounds: Option<Bounds>,
    /// A field object's draw class (Ryza's placement lists: near, middle, far, landmark as 0 to
    /// 3, each with its field's draw distance); 0 elsewhere.
    pub class: u8,
}

/// A draw's box in its model's space (its vertices on the scene's starting joints) and how far
/// beyond it the GPU may still move them, in render units: the grass sway, the trees' node
/// springs, a billboard's corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub lo: Vec3,
    pub hi: Vec3,
    pub pad: f32,
}

impl Bounds {
    /// The draw's vertices (`first_index`, `index_count`) skinned as the vertex shader does on
    /// `palette` from `base`, then padded: a quarter of the box's longest side plus a metre for
    /// the sway and springs (a spring bends a tree at most its `max_angle`, ten-odd degrees),
    /// and the longest billboard corner.
    pub fn of(vertices: &[Vertex], indices: &[u32], first_index: u32, index_count: u32, palette: &[Mat4], base: u32) -> Option<Bounds> {
        let (mut lo, mut hi, mut corner) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN), 0f32);
        for &i in indices.get(first_index as usize..(first_index + index_count) as usize)? {
            let v = vertices.get(i as usize)?;
            let mut p = Vec3::from(v.pos);
            let total: u32 = v.weights.iter().map(|&w| u32::from(w)).sum();
            if total > 0 {
                let mut skinned = Vec3::ZERO;
                for (&j, &w) in v.joints.iter().zip(&v.weights) {
                    if w > 0 {
                        let m = palette.get(base as usize + usize::from(j)).copied().unwrap_or(Mat4::IDENTITY);
                        skinned += m.transform_point3(p) * f32::from(w);
                    }
                }
                p = skinned / total as f32;
            }
            lo = lo.min(p);
            hi = hi.max(p);
            corner = corner.max(Vec3::from(v.corner).length());
        }
        (lo.x <= hi.x).then(|| Bounds { lo, hi, pad: (hi - lo).max_element() * 0.25 + 100.0 + corner })
    }

    /// Whether any of it may be inside the view of `planes` ([`view_planes`]) placed by `model`:
    /// its box's corners through the model matrix, padded, against each plane.
    /// The box's longest side once placed by `model` (its axes' scale), unpadded.
    pub fn extent(&self, model: &Mat4) -> f32 {
        let size = self.hi - self.lo;
        (model.x_axis.truncate() * size.x).length().max((model.y_axis.truncate() * size.y).length()).max((model.z_axis.truncate() * size.z).length())
    }

    pub fn seen(&self, model: &Mat4, planes: &[glam::Vec4; 6]) -> bool {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { self.lo.x } else { self.hi.x },
                if k & 2 == 0 { self.lo.y } else { self.hi.y },
                if k & 4 == 0 { self.lo.z } else { self.hi.z },
            );
            let w = model.transform_point3(c);
            lo = lo.min(w);
            hi = hi.max(w);
        }
        let (lo, hi) = (lo - Vec3::splat(self.pad), hi + Vec3::splat(self.pad));
        planes.iter().all(|p| {
            let far = Vec3::new(if p.x >= 0.0 { hi.x } else { lo.x }, if p.y >= 0.0 { hi.y } else { lo.y }, if p.z >= 0.0 { hi.z } else { lo.z });
            p.truncate().dot(far) + p.w >= 0.0
        })
    }
}

/// The six planes of a view-projection's clip volume (Vulkan's: depth 0 to 1), each as
/// (normal, distance) with the inside where `n · p + d >= 0`.
pub fn view_planes(m: &Mat4) -> [glam::Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2]
}

/// `cDiff` and `cAmb` both 1.
pub const FULL_LIGHT: u32 = 0xffff_ffff;

/// A model's lighting material as the push constants take it: `cDiff` (its red) and `cAmb`
/// (its red) as unorm16 (`unpackUnorm2x16`).
pub fn light_word(geometry: &barrel_formats::g1m::Geometry) -> u32 {
    geometry.lights.first().map_or(FULL_LIGHT, |l| {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as u32;
        q(l.diffuse[0]) | q(l.ambient[0]) << 16
    })
}

pub struct Gpu {
    pub _entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub queue_family: u32,
    pub memory: vk::PhysicalDeviceMemoryProperties,
    /// The most anisotropic filtering the device allows (1 without the feature).
    pub max_anisotropy: f32,
    /// The alignment of a uniform or storage buffer binding's offset (`ring::Ring` slots).
    pub offset_alignment: u64,
    /// The window loop's frame being prepared, counted up as each is submitted: the CPU writes
    /// its per-frame data into slot `frame % ring::SLOTS` while the GPU may still draw the frame
    /// before (`docs/renderer.md`, "Frames in flight").
    pub frame: std::cell::Cell<u64>,
}

impl Gpu {
    /// A device for offscreen work (`display` None) or for presenting to a window on
    /// `display` (adds the surface and swapchain extensions).
    pub fn new(display: Option<raw_window_handle::RawDisplayHandle>) -> Result<Gpu> {
        let entry = unsafe { ash::Entry::load()? };
        let validation = std::env::var("BARREL_VALIDATION").is_ok_and(|v| v == "1");
        let layers: Vec<*const std::ffi::c_char> = if validation { vec![c"VK_LAYER_KHRONOS_validation".as_ptr()] } else { vec![] };
        // The application's name as drivers and compositors see it (per-application settings):
        // the program's own (barrel-escha, barrel-ryza, barrel).
        let name = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_else(|| "barrel".into());
        let name = std::ffi::CString::new(name).unwrap_or_else(|_| c"barrel".into());
        let app = vk::ApplicationInfo::default().application_name(&name).api_version(vk::API_VERSION_1_1);
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
            .shader_storage_image_read_without_format(features.shader_storage_image_read_without_format != 0)
            .sampler_anisotropy(features.sampler_anisotropy != 0);
        let max_anisotropy = if features.sampler_anisotropy != 0 { props.limits.max_sampler_anisotropy } else { 1.0 };
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
        let offset_alignment = props.limits.min_uniform_buffer_offset_alignment.max(props.limits.min_storage_buffer_offset_alignment).max(16);
        Ok(Gpu { _entry: entry, instance, physical, device, queue, queue_family, memory, max_anisotropy, offset_alignment, frame: std::cell::Cell::new(0) })
    }

    pub fn memory_type(&self, bits: u32, flags: vk::MemoryPropertyFlags) -> Result<u32> {
        (0..self.memory.memory_type_count)
            .find(|&i| bits & (1 << i) != 0 && self.memory.memory_types[i as usize].property_flags.contains(flags))
            .ok_or_else(|| "no suitable memory type".into())
    }

    pub fn buffer(&self, size: u64, usage: vk::BufferUsageFlags, host: bool) -> Result<(vk::Buffer, vk::DeviceMemory)> {
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

    /// The slot of the frame being prepared (`ring::SLOTS` of them).
    pub fn slot(&self) -> usize {
        (self.frame.get() % ring::SLOTS as u64) as usize
    }

    pub fn upload(&self, mem: vk::DeviceMemory, bytes: &[u8]) -> Result<()> {
        self.upload_at(mem, 0, bytes)
    }

    pub fn upload_at(&self, mem: vk::DeviceMemory, offset: u64, bytes: &[u8]) -> Result<()> {
        unsafe {
            let p = self.device.map_memory(mem, offset, bytes.len() as u64, vk::MemoryMapFlags::empty())? as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            self.device.unmap_memory(mem);
        }
        Ok(())
    }

    pub fn image(&self, info: &vk::ImageCreateInfo) -> Result<(vk::Image, vk::DeviceMemory)> {
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
    pub fn submit(&self, pool: vk::CommandPool, f: impl FnOnce(vk::CommandBuffer)) -> Result<()> {
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

pub fn barrier(
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

/// The terrain's pipeline (the renderer's; each scene's grounds are in `SceneGpu`).
pub struct TerrainGpu {
    pub pipeline_layout: vk::PipelineLayout,
    pub shader: vk::ShaderModule,
    pub pipeline: vk::Pipeline,
}

/// The field's ground: the terrain's own mesh (`ktf::Mesh`, its finest patches' lit draws)
/// drawn with the terrain shader. Each vertex carries its draw's layers and blend (`uv`, see
/// `add_terrain`).
pub struct TerrainDraw {
    pub patches: Vec<scene_gpu::TerrainPatch>,
    /// The cell size (a layer repeats every n cells).
    pub cell: f32,
    /// The field's size in world units along x and z (the blend maps span it).
    pub extent: [f32; 2],
    /// The terrain bundle, for its layer and blend textures.
    pub ktf: Vec<u8>,
}

/// Blend maps and layers a terrain set holds (the most any field has: 7 maps, 20 layers).
pub const TERRAIN_MAPS: usize = 8;
pub const TERRAIN_LAYERS: usize = 20;

pub struct Scene {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<Draw>,
    /// Each field's ground, by field.
    pub terrains: Vec<(u16, TerrainDraw)>,
    /// The field geometry added now belongs to (see [`Draw::field`]).
    pub field: Option<u16>,
    pub lo: Vec3,
    pub hi: Vec3,
    /// The next model's facial morph sets by vertex layout: each target's offset per vertex,
    /// and the set's starting weights (from its facial animations).
    pub morph: std::collections::HashMap<usize, (Vec<Vec<[f32; 3]>>, Vec<f32>)>,
    /// Every morphed vertex's target offsets (vertex by vertex, target by target), for the
    /// GPU; never empty.
    pub deltas: Vec<[f32; 4]>,
    /// Every morphed model's target weights, one block per instance; never empty.
    pub morph_weights: Vec<f32>,
    /// Model matrices in render space (x mirrored), one per movable model; 0 is the identity,
    /// for geometry baked in place. The renderer uploads them to a storage buffer the vertex
    /// shader reads, so a model moves by changing its matrix, not its vertices.
    pub instances: Vec<Instance>,
    /// Every skinned model's joint matrices (bind pose to posed, render space), one after
    /// another; an instance's `palette` is where its own begin. Never empty (the GPU buffer
    /// needs a size), so it starts with one identity.
    pub palette: Vec<Mat4>,
    /// While set, geometry without skin weights is bound wholly to this joint of the pose
    /// (an accessory hung from a node: its vertices are already in that joint's bind frame).
    pub rigid_joint: Option<u16>,
}

impl Scene {
    pub fn new() -> Scene {
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
            rigid_joint: None,
        }
    }

    /// Add a model's textured submeshes, placed by `transform`.
    pub fn add(&mut self, geometry: &barrel_formats::g1m::Geometry, transform: Mat4, textures: usize) -> Result<()> {
        self.add_posed(geometry, transform, textures, None, 0)
    }

    /// A new instance placed by `transform` (game space); returns its number.
    pub fn instance(&mut self, transform: Mat4) -> u32 {
        self.instances.push(Instance { model: render_space(transform), palette: 0, weights: 0, prev_palette: 0, _pad: 0, prev_model: render_space(transform) });
        (self.instances.len() - 1) as u32
    }

    /// Add a movable model (its own instance, placed by `transform`) whose texture pack starts
    /// at `base` in the scene's texture list, skinned by `joints` if given. Returns its instance.
    pub fn add_with_pack(&mut self, geometry: &barrel_formats::g1m::Geometry, transform: Mat4, base: usize, count: usize, joints: Option<&[Mat4]>) -> Result<u32> {
        let first = self.draws.len();
        let instance = self.instance(transform);
        self.add_posed(geometry, Mat4::IDENTITY, count, joints, instance)?;
        for d in &mut self.draws[first..] {
            d.texture += base;
            d.overlay = d.overlay.map(|o| o + base);
            d.mask = d.mask.map(|m| m + base);
            d.normal_map = d.normal_map.map(|m| m + base);
            d.emission = d.emission.map(|m| m + base);
        }
        Ok(instance)
    }

    /// Add a model, skinned by `joints` (one matrix per joint, bind pose to posed) if given.
    /// `transform` is baked into the vertices; `instance` places them at draw time.
    pub fn add_posed(&mut self, geometry: &barrel_formats::g1m::Geometry, transform: Mat4, textures: usize, joints: Option<&[Mat4]>, instance: u32) -> Result<()> {
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
                if let (None, Some(j), Some(joints)) = (&skin, self.rigid_joint, joints)
                    && (j as usize) < joints.len() {
                        joint_ix[0] = j;
                        weight_q[0] = 255;
                        skinned = joints[j as usize].transform_point3(pos);
                    }
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
            let normal_map = material.and_then(|m| m.textures.iter().find(|t| t.kind == 3)).map(|t| (t.index as usize).min(textures.saturating_sub(1)));
            let emission = material.and_then(|m| m.textures.iter().find(|t| t.kind == 19)).map(|t| (t.index as usize).min(textures.saturating_sub(1)));
            let outline = geometry.param(s, "atEdge").and(Some(geometry.param(s, "atOutCol").and_then(|c| c.first().copied()).unwrap_or(1.0)));
            self.draws.push(Draw { first_index, index_count: self.indices.len() as u32 - first_index, texture, overlay, instance, mask, normal_map, emission, eye, sky: false, outline, field: self.field, casts: true, name: String::new(), version: None, lod: 0, light: light_word(geometry), tint: 0, bounds: None, class: 0 });
        }
        Ok(())
    }
}

impl Scene {
    /// The terrain mesh's lit draws over its finest patches, a range of indices per patch with
    /// its bake list. A vertex's `uv` says how its draw mixes: x = layer a + 32 × layer b + 1024
    /// × blend (the map × 4 + the channel pair, 3 for one layer alone) + 32768 when the draw adds
    /// the bake (programs 4 to 7); y = layer a's repeat + 256 × layer b's (in cells). Normals come
    /// from the heightmap (the stored ones are not decoded).
    pub fn add_terrain(&mut self, m: &barrel_formats::ktf::Mesh, repeats: &[u8]) -> Vec<scene_gpu::TerrainPatch> {
        let mirror = Vec3::new(-1.0, 1.0, 1.0);
        let mut out = Vec::new();
        let mut dropped = 0usize;
        for (k, p) in m.patches.iter().enumerate() {
            if !p.leaf {
                continue;
            }
            let verts = m.patch_vertices(k);
            let draws = m.draws.get(p.first_draw as usize..p.first_draw as usize + usize::from(p.draws)).unwrap_or(&[]);
            let first_index = self.indices.len() as u32;
            // The bake: the one-layer draws (kind 0x10), each its layer under one map channel
            // (the program: 0 red, 1 green, 2 blue).
            let bake = draws
                .iter()
                .filter(|d| d.kind == 0x10 && !d.layers.is_empty() && d.program < 3)
                .map(|d| scene_gpu::bake_entry(d.layers[0], d.extra[1], d.program, *repeats.get(d.layers[0] as usize).unwrap_or(&1)))
                .take(scene_gpu::BAKE_MAX)
                .collect();
            for d in draws.iter().filter(|d| d.kind == 0x20 && !d.layers.is_empty()) {
                let (a, b) = (d.layers[0], *d.layers.get(1).unwrap_or(&d.layers[0]));
                // The map is the draw's fourth byte; programs 1-3 and 5-7 read its channel pairs (5-7
                // also add the bake of the other map's layers, not drawn yet).
                let blend = if d.layers.len() < 2 || d.program % 4 == 0 { 3 } else { u32::from(d.extra[1]) * 4 + u32::from(d.program % 4 - 1) };
                let rep = |l: u8| f32::from(*repeats.get(l as usize).unwrap_or(&1));
                let baked = if (4..8).contains(&d.program) { 32768.0 } else { 0.0 };
                let uv = [f32::from(a) + 32.0 * f32::from(b) + 1024.0 * blend as f32 + baked, rep(a) + 256.0 * rep(b)];
                let base = self.vertices.len() as u32;
                // The draw's own copy of its patch's vertices (its indices are the patch's, and
                // its vertex range does not always bound them).
                for v in verts {
                    let pos = Vec3::new(f32::from(v.x) * m.cell_size, p.height(v.height), f32::from(v.z) * m.cell_size) * mirror;
                    let n = Vec3::from(v.normal()) * mirror;
                    self.lo = self.lo.min(pos);
                    self.hi = self.hi.max(pos);
                    // The vertex's own colour: baked shade (darker under walls and in
                    // corners), lit as a field model's vertex colour is (the near colour; the
                    // LOD morph toward the second is not done).
                    let [r, g, b, _] = v.colour;
                    self.vertices.push(Vertex { pos: pos.into(), normal: n.into(), uv, joints: [0; 4], weights: [0; 4], morph: [0; 2], color: [r, g, b, 255], corner: [0.0; 3] });
                }
                for t in barrel_formats::ktf::patch_triangles(m, p, d) {
                    if t.iter().all(|&i| (i as usize) < verts.len()) {
                        self.indices.extend(t.map(|i| base + i));
                    } else {
                        dropped += 1;
                    }
                }
            }
            out.push(scene_gpu::TerrainPatch { first_index, index_count: self.indices.len() as u32 - first_index, bake });
        }
        if dropped > 0 {
            eprintln!("terrain: {dropped} triangles index past their patch's vertices, left out");
        }
        out
    }
}

/// What a frame draws into besides the colour image: a depth image at the target's size and
/// a framebuffer per colour image (one offscreen, or one per swapchain image).
/// What the frames draw into, remade at a new size: the scene's colour (sampled by the post
/// pass) and depth with their framebuffer, and one post framebuffer per output image (the
/// offscreen image, or each swapchain image).
pub struct Targets {
    pub scene: vk::Image,
    pub scene_memory: vk::DeviceMemory,
    pub scene_view: vk::ImageView,
    pub depth: vk::Image,
    pub depth_memory: vk::DeviceMemory,
    pub depth_view: vk::ImageView,
    pub scene_framebuffer: vk::Framebuffer,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub glow: Option<glow::Targets>,
    /// FSR 1, when it is on and the scene is drawn below the output's size.
    pub upscale: Option<upscale::Targets>,
    /// TAA (`--taa`), and whether its (or GSR 2's) history is still empty (new targets: the
    /// first frame; a cut).
    pub taa: Option<taa::Targets>,
    /// GSR 2 (`--gsr2`): the temporal upscaler.
    pub gsr2: Option<gsr2::Targets>,
    /// The post pass's sets: the one to bind is [`Targets::post_set`].
    pub post_sets: [vk::DescriptorSet; 2],
    /// FSR 3.1 (`--fsr3`): the desktop's temporal upscaler (its state changes as it records).
    pub fsr3: std::cell::RefCell<Option<fsr3::Targets>>,
    pub history_fresh: std::cell::Cell<bool>,
    /// The scene as it was when a cross-fade began (`u13` kind 3), faded out over the live one;
    /// whether it has been given a layout yet.
    pub frozen: vk::Image,
    pub frozen_memory: vk::DeviceMemory,
    pub frozen_view: vk::ImageView,
    pub frozen_ready: bool,
    /// The scene's own size: the output's times the render scale (`--scale`); the post pass
    /// scales it to the output, which keeps the text sharp.
    pub scene_size: (u32, u32),
    /// The multisampled colour (`--msaa`), resolved into `scene` by the scene pass.
    pub multisample: Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    /// The motion vectors (`--motion`).
    pub motion: Option<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
}

impl Targets {
    /// The post pass's set for this frame: the one reading GSR 2's output just written.
    pub fn post_set(&self) -> vk::DescriptorSet {
        self.post_sets[self.gsr2.as_ref().map_or(0, gsr2::Targets::written)]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gpu: &Gpu,
        scene_pass: vk::RenderPass,
        post_pass: vk::RenderPass,
        output: &[vk::ImageView],
        width: u32,
        height: u32,
        scene_size: (u32, u32),
        post_sets: [vk::DescriptorSet; 2],
        sampler: vk::Sampler,
        glow_pipelines: &glow::Pipelines,
        upscale_pipelines: Option<&upscale::Pipelines>,
        taa_pipelines: Option<&taa::Pipelines>,
        gsr2_pipelines: Option<&gsr2::Pipelines>,
        fsr3_pipelines: Option<&fsr3::Pipelines>,
        samples: vk::SampleCountFlags,
        motion: bool,
        show_motion: bool,
        scene_format: vk::Format,
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
            image(scene_format, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)?;
        let (frozen, frozen_memory) = image(scene_format, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)?;
        let frozen_view = view(frozen, scene_format, vk::ImageAspectFlags::COLOR)?;
        let scene_view = view(scene, scene_format, vk::ImageAspectFlags::COLOR)?;
        // Depth is cleared and never stored (the scene pass): transient.
        // Depth is transient unless GSR 2 reads it (then the scene pass stores it).
        let depth_usage = if gsr2_pipelines.is_some() || fsr3_pipelines.is_some() { vk::ImageUsageFlags::SAMPLED } else { vk::ImageUsageFlags::TRANSIENT_ATTACHMENT };
        let (depth, depth_memory) = image_with(DEPTH_FORMAT, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | depth_usage, samples)?;
        // With samples, the multisampled colour the pass resolves into the scene: transient too.
        let multisample = if samples != vk::SampleCountFlags::TYPE_1 {
            let (image, memory) = image_with(scene_format, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT, samples)?;
            Some((image, memory, view(image, scene_format, vk::ImageAspectFlags::COLOR)?))
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
        // GSR 2's output alternates: post set k reads its output k.
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
        // The post pass samples the new scene image (TAA's, GSR 2's or FSR's when they run) and
        // the glow.
        let glow_info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(glow.result()).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let frozen_info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(frozen_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        for (k, &post_set) in post_sets.iter().enumerate() {
            let shown = match (show_motion, motion_target) {
                (true, Some((_, _, motion_view))) => motion_view,
                _ => match &gsr2 {
                    Some(g) => g.result(k),
                    None => upscale.as_ref().map_or(resolved, upscale::Targets::result),
                },
            };
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(shown).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
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
        }
        Ok(Targets {
            post_sets,
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

    pub fn destroy(&mut self, gpu: &Gpu) {
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

/// `Draw::lod`: an object's full model, its middle and its low level of detail, and the mark
/// of its last level (shown at any distance past its start).
pub const LOD_HI: u8 = 1;
pub const LOD_MID: u8 = 2;
pub const LOD_LOW: u8 = 3;
pub const LOD_LAST: u8 = 4;

/// A game-space placement as a render-space matrix: the vertices are mirrored in x as they are
/// built, so a placement T becomes S·T·S with S the x mirror (a rotation stays a rotation).
pub fn render_space(transform: Mat4) -> Mat4 {
    let s = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    s * transform * s
}

/// Upload a texture as it is stored (BC blocks and all mip levels) for sampling. `linear`: a
/// colour texture sampled in linear light (its sRGB format; a game that lights in linear light,
/// the Ryza games); otherwise its values as they are.
pub fn upload_texture(gpu: &Gpu, pool: vk::CommandPool, t: &barrel_formats::g1t::Texture, linear: bool) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView)> {
    let d = &gpu.device;
    {
        let format = match t.format {
            Format::Bc1 if linear => vk::Format::BC1_RGBA_SRGB_BLOCK,
            Format::Bc3 if linear => vk::Format::BC3_SRGB_BLOCK,
            Format::Bc7 if linear => vk::Format::BC7_SRGB_BLOCK,
            Format::Rgba8 if linear => vk::Format::R8G8B8A8_SRGB,
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

/// Lays each draw's overlay over its base texture into a new RGBA texture (width, height,
/// pixels), numbered after `textures`, and points the draw at it.
pub fn composite_overlays(draws: &mut [Draw], all_textures: &[&barrel_formats::g1t::Texture]) -> Vec<(u32, u32, Vec<u8>)> {
    let mut composites: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for draw in draws {
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
    composites
}

/// Uploads a scene's textures (BC blocks with all mip levels), then the characters' outline
/// texture (`Data/x64/Shader/texture/outline.g1t` under ROOT, if found) and a white one for
/// masks a material does not have: the textures, the outline's slot and white's. `linear`
/// says which are colour textures to sample in linear light (`upload_texture`; none when empty).
pub fn upload_textures(
    gpu: &Gpu,
    pool: vk::CommandPool,
    all_textures: &[&barrel_formats::g1t::Texture],
    root: Option<&Path>,
    linear: &[bool],
) -> Result<(Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>, Option<usize>, usize)> {
    let mut textures = Vec::new();
    for (k, t) in all_textures.iter().enumerate() {
        textures.push(upload_texture(gpu, pool, t, linear.get(k).copied().unwrap_or(false))?);
    }
    // The characters' outline texture (`Data/x64/Shader/texture/outline.g1t`), if found.
    let outline_tex = {
        let bytes = root.and_then(|r: &Path| std::fs::read(r.join("Data/x64/Shader/texture/outline.g1t")).ok());
        match bytes.as_ref().and_then(|b| G1t::parse(b).ok()).and_then(|g| g.textures.into_iter().next().map(|t| upload_texture(gpu, pool, &t, false))) {
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
        gpu,
        pool,
        &barrel_formats::g1t::Texture { width: 1, height: 1, format: Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&white_px[..]] },
        false,
    )?);
    Ok((textures, outline_tex, white))
}

/// Uploads each field's ground textures: its 2 blend maps (a black second one where the field
/// has one) and 6 layers (the last repeated where it has fewer; `linear`: sampled in linear
/// light, `upload_texture`).
pub fn upload_grounds(gpu: &Gpu, pool: vk::CommandPool, terrains: &[(u16, TerrainDraw)], linear: bool) -> Result<Vec<scene_gpu::GroundData>> {
    let mut grounds = Vec::new();
    for (field, t) in terrains {
        let bundle = barrel_formats::ktf::parse(&t.ktf)?;
        let mut tex = Vec::new();
        // The blend maps, then black for the slots past them.
        let black = [0u8, 0, 0, 255];
        let black_tex = barrel_formats::g1t::Texture { width: 1, height: 1, format: Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&black[..]] };
        for i in 0..TERRAIN_MAPS {
            tex.push(upload_texture(gpu, pool, bundle.blend.textures.get(i).unwrap_or(&black_tex), false)?);
        }
        // Each layer's colour tile, by the layer records (the Ryza games put a normal map after
        // each; not drawn yet), the last again for the slots past them.
        // Records naming a texture outside the pack are not layer records (Escha's battle
        // terrain): texture k for layer k then.
        let count = bundle.layers.textures.len();
        let colours: Vec<usize> = bundle.layer_list().iter().map(|l| usize::from(l.colour)).collect();
        let colours = if colours.is_empty() || colours.iter().any(|&c| c >= count) { (0..count.max(1)).collect() } else { colours };
        for i in 0..TERRAIN_LAYERS {
            tex.push(upload_texture(gpu, pool, &bundle.layers.textures[colours[i.min(colours.len() - 1)]], linear)?);
        }
        grounds.push(scene_gpu::GroundData { field: *field, textures: tex, patches: t.patches.clone(), cell: t.cell, extent: t.extent });
    }
    Ok(grounds)
}
