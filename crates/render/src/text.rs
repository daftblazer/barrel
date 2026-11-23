//! Text and 2D pictures on screen: a font atlas's glyphs, the rectangles a game lays out with
//! them and its interface pictures ([`Quad`]), and a layer that draws them over the scene,
//! alpha blended, in screen pixels. What a game's text looks like (its markup, its line
//! breaking, its message window) is the game's: Escha's is `barrel_escha`'s `play::message`.

use std::collections::HashMap;

use ash::vk;

use crate::{upload_texture, Gpu, Result};

/// A character in a font atlas: its box in the atlas, where it sits against the pen and how
/// far the pen moves on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub x_offset: i32,
    pub y_offset: i32,
    pub advance: u32,
}

pub struct Font {
    pub glyphs: HashMap<char, Glyph>,
    /// The atlas's size in pixels.
    pub size: f32,
}

/// One rectangle to draw: `rect` and `uv` are (left, top, right, bottom), in pixels and
/// texture coordinates (a right edge left of the left one mirrors it); a negative `uv[0]`
/// fills the rectangle with `color` alone. `texture` is [`FONT`] or [`UI`]. `turn` turns the
/// picture a quarter clockwise into `rect` (its top edge down the rectangle's right side); its
/// `uv` then names the picture's corners as they land on the rectangle's top-left and bottom-right.
/// `spin` then turns the placed rectangle freely about a point (a node drawn tilted) or names
/// its corners outright.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub turn: bool,
    pub spin: Option<Spin>,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub texture: usize,
}

/// A stencil drawn with a quad, as the games draw a minimap's round edge: the `mask` quad marks
/// the screen where its picture is not clear and is not drawn itself (it stays in the draw
/// trace); the quads in `quads` draw only there. The mask is upright (no `turn` or `spin`).
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub mask: usize,
    pub quads: std::ops::Range<usize>,
}

/// How a quad's rectangle is placed other than upright.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spin {
    /// A free turn about `pivot` (pixels, in the layer's own screen), `angle` radians clockwise
    /// on the screen.
    About { pivot: [f32; 2], angle: f32 },
    /// The four corners outright (top-left, top-right, bottom-right, bottom-left of the
    /// picture), for a node turned out of the screen's plane and seen in perspective.
    Corners([[f32; 2]; 4]),
}

impl Quad {
    /// The rectangle's corners (top-left, top-right, bottom-right, bottom-left), turned by
    /// `spin` about its pivot; exactly the rectangle's own without one.
    pub fn corners(&self) -> [[f32; 2]; 4] {
        let [l, t, r, b] = self.rect;
        let c = [[l, t], [r, t], [r, b], [l, b]];
        let (px, py, angle) = match self.spin {
            None => return c,
            Some(Spin::Corners(placed)) => return placed,
            Some(Spin::About { pivot: [px, py], angle }) => (px, py, angle),
        };
        let (s, k) = angle.sin_cos();
        c.map(|[x, y]| {
            let (dx, dy) = (x - px, y - py);
            [px + dx * k - dy * s, py + dx * s + dy * k]
        })
    }
}

/// The layer's textures: the font atlas and the message window pieces.
pub const FONT: usize = 0;
pub const UI: usize = 1;

impl Font {
    /// How far the pen moves on after `c`, in atlas pixels (0 for a character without a glyph).
    pub fn advance(&self, c: char) -> f32 {
        self.glyphs.get(&c).map_or(0.0, |g| g.advance as f32)
    }

    /// The width of `s` set in one run, in atlas pixels.
    pub fn width(&self, s: &str) -> f32 {
        s.chars().map(|c| self.advance(c)).sum()
    }
}

const TEXT_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.vert.spv"));
const TEXT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.frag.spv"));

/// A vertex: position (pixels), atlas coordinates, colour, and its clip's mask (where it falls in
/// the mask's rectangle, the mask's texture coordinates; `NO_MASK` without one).
#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    mask_at: [f32; 2],
    mask_uv: [f32; 4],
}

const NO_MASK: [f32; 4] = [-1.0; 4];

/// What draws text and the window: its textures ([`FONT`], [`UI`]), a pipeline (no depth,
/// alpha blending) and vertex buffers rewritten when what it shows changes: one per frame in
/// flight (`ring::SLOTS`), the first change in a frame going to the one the frames before are
/// not drawing from.
pub struct Layer {
    textures: Vec<(vk::Image, vk::DeviceMemory, vk::ImageView)>,
    sampler: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    sets: Vec<vk::DescriptorSet>,
    /// Runs of vertices by texture and mask texture: (texture, mask, first vertex, count).
    /// (and whether the run is drawn additively, [`Layer::set_blended`]).
    runs: Vec<(usize, usize, u32, u32, bool)>,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    /// The same with the picture added to what is under it (a glow).
    additive: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    /// The vertex buffers (buffer, memory, capacity, mapped), the one drawn from, and the frame
    /// it was last written in.
    buffers: [Option<(vk::Buffer, vk::DeviceMemory, usize, *mut u8)>; crate::ring::SLOTS],
    current: usize,
    written: Option<u64>,
    count: u32,
    /// The draw trace (`crate::trace`): this layer's id, its textures' names, and the quads set
    /// last with the frame they were set in, until a record writes them.
    trace: Option<(u64, Vec<String>, std::cell::RefCell<Option<(u64, Vec<Quad>)>>)>,
}

impl Layer {
    pub fn new(gpu: &Gpu, pool: vk::CommandPool, render_pass: vk::RenderPass, images: &[&barrel_formats::g1t::Texture]) -> Result<Layer> {
        let d = &gpu.device;
        let textures = images.iter().map(|t| upload_texture(gpu, pool, t, false)).collect::<Result<Vec<_>>>()?;
        let trace = crate::trace::enabled().then(|| (crate::trace::layer_id(), images.iter().map(|t| crate::trace::texture_name(t)).collect(), Default::default()));
        let n = textures.len() as u32;
        unsafe {
            let sampler = d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )?;
            let binding = [vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&binding), None)?;
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: n }];
            let dpool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(n).pool_sizes(&sizes), None)?;
            let layouts = vec![set_layout; textures.len()];
            let sets = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(dpool).set_layouts(&layouts))?;
            for (set, t) in sets.iter().zip(&textures) {
                let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(t.2).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
                d.update_descriptor_sets(
                    &[vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)],
                    &[],
                );
            }
            // Set 0: the picture; set 1: a clip's mask (the picture's own set when unclipped).
            let layouts = [set_layout, set_layout];
            let push = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::VERTEX, offset: 0, size: 8 }];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None)?;
            let module = |code: &[u8]| {
                let words: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
            };
            let modules = [module(TEXT_VERT)?, module(TEXT_FRAG)?];
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(modules[0]).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(modules[1]).name(c"main"),
            ];
            let vbind = [vk::VertexInputBindingDescription { binding: 0, stride: size_of::<Vertex>() as u32, input_rate: vk::VertexInputRate::VERTEX }];
            let vattr = [
                vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: 0 },
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: 8 },
                vk::VertexInputAttributeDescription { location: 2, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 16 },
                vk::VertexInputAttributeDescription { location: 3, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: 32 },
                vk::VertexInputAttributeDescription { location: 4, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: 40 },
            ];
            let vinput = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbind).vertex_attribute_descriptions(&vattr);
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let vstate = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
            let raster = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
            let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth = vk::PipelineDepthStencilStateCreateInfo::default();
            let blend_att = [vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::RGBA)];
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
            let add_att = [blend_att[0].dst_color_blend_factor(vk::BlendFactor::ONE)];
            let add_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&add_att);
            let additive = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info.color_blend_state(&add_blend)], None).map_err(|(_, e)| e)?[0];
            Ok(Layer { textures, sampler, set_layout, pool: dpool, sets, runs: Vec::new(), layout, pipeline, additive, modules, buffers: [None; crate::ring::SLOTS], current: 0, written: None, count: 0, trace })
        }
    }

    /// Sets what the next frames draw (the buffer grows as needed).
    pub fn set(&mut self, gpu: &Gpu, quads: &[Quad]) -> Result<()> {
        self.set_clipped(gpu, quads, &[])
    }

    /// As [`Layer::set`], with some quads clipped by others ([`Clip`]).
    pub fn set_clipped(&mut self, gpu: &Gpu, quads: &[Quad], clips: &[Clip]) -> Result<()> {
        self.set_blended(gpu, quads, clips, 0..0)
    }

    /// As [`Layer::set_clipped`], with the quads in `additive` drawn added to what is under
    /// them (source alpha times the picture, as the games' glows).
    pub fn set_blended(&mut self, gpu: &Gpu, quads: &[Quad], clips: &[Clip], additive: std::ops::Range<usize>) -> Result<()> {
        self.set_blended_by(gpu, quads, clips, &|i| additive.contains(&i))
    }

    /// As [`Layer::set_blended`], with the additive quads picked by `additive` (any of them, not
    /// a run).
    pub fn set_blended_by(&mut self, gpu: &Gpu, quads: &[Quad], clips: &[Clip], additive: &dyn Fn(usize) -> bool) -> Result<()> {
        if let Some((_, _, pending)) = &self.trace {
            *pending.borrow_mut() = Some((gpu.frame.get(), quads.to_vec()));
        }
        let mut verts = Vec::with_capacity(quads.len() * 6);
        self.runs.clear();
        let last = self.textures.len().saturating_sub(1);
        for (i, q) in quads.iter().enumerate() {
            if clips.iter().any(|c| c.mask == i) {
                continue;
            }
            let texture = q.texture.min(last);
            let mask = clips.iter().find(|c| c.quads.contains(&i)).and_then(|c| quads.get(c.mask));
            let mask_texture = mask.map_or(texture, |m| m.texture.min(last));
            let add = additive(i);
            match self.runs.last_mut() {
                Some((t, m, _, count, a)) if *t == texture && *m == mask_texture && *a == add => *count += 6,
                _ => self.runs.push((texture, mask_texture, verts.len() as u32, 6, add)),
            }
            let [tl, tr, br, bl] = q.corners();
            let [u0, v0, u1, v1] = q.uv;
            let v = |[x, y]: [f32; 2], u, w| {
                let (mask_at, mask_uv) = match mask {
                    Some(m) => {
                        let [l, t, r, b] = m.rect;
                        ([(x - l) / (r - l), (y - t) / (b - t)], m.uv)
                    }
                    None => ([0.0; 2], NO_MASK),
                };
                Vertex { pos: [x, y], uv: [u, w], color: q.color, mask_at, mask_uv }
            };
            let (u1, v1) = if u0 < 0.0 { (u0, v0) } else { (u1, v1) };
            if q.turn {
                // Clockwise: the picture's bottom-left at the top-left, its top-left at the
                // top-right, its top-right at the bottom-right.
                verts.extend([v(tl, u0, v0), v(tr, u0, v1), v(br, u1, v1), v(tl, u0, v0), v(br, u1, v1), v(bl, u1, v0)]);
            } else {
                verts.extend([v(tl, u0, v0), v(tr, u1, v0), v(br, u1, v1), v(tl, u0, v0), v(br, u1, v1), v(bl, u0, v1)]);
            }
        }
        self.count = verts.len() as u32;
        if verts.is_empty() {
            return Ok(());
        }
        let bytes = unsafe { std::slice::from_raw_parts(verts.as_ptr() as *const u8, verts.len() * size_of::<Vertex>()) };
        // Written already this frame: that buffer again (no frame has drawn from it yet); else
        // the other one, whose last frame the window loop has waited for.
        let frame = gpu.frame.get();
        let slot = if self.written == Some(frame) { self.current } else { (self.current + 1) % crate::ring::SLOTS };
        if self.buffers[slot].is_none_or(|(_, _, cap, _)| cap < bytes.len()) {
            if let Some((b, m, _, _)) = self.buffers[slot].take() {
                unsafe {
                    gpu.device.device_wait_idle()?;
                    gpu.device.destroy_buffer(b, None);
                    gpu.device.free_memory(m, None);
                }
            }
            let cap = bytes.len().next_power_of_two().max(4096);
            let (b, m) = gpu.buffer(cap as u64, vk::BufferUsageFlags::VERTEX_BUFFER, true)?;
            let ptr = unsafe { gpu.device.map_memory(m, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())? as *mut u8 };
            self.buffers[slot] = Some((b, m, cap, ptr));
        }
        let (_, _, _, ptr) = self.buffers[slot].unwrap();
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
        (self.current, self.written) = (slot, Some(frame));
        Ok(())
    }

    /// Records the draw into a render pass that is under way (viewport and scissor set).
    ///
    /// # Safety
    /// `cb` is recording inside a render pass compatible with the one the layer was made for.
    pub unsafe fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, width: u32, height: u32) {
        if let Some((id, names, pending)) = &self.trace {
            if let Some((frame, quads)) = pending.borrow_mut().take() {
                crate::trace::quads(frame, *id, width, height, names, &quads);
            }
        }
        let Some((buf, ..)) = self.buffers[self.current].filter(|_| self.count > 0) else { return };
        unsafe {
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            let screen = [width as f32, height as f32];
            d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX, 0, std::slice::from_raw_parts(screen.as_ptr() as *const u8, 8));
            d.cmd_bind_vertex_buffers(cb, 0, &[buf], &[0]);
            let mut bound = false;
            for &(texture, mask, first, count, add) in &self.runs {
                if add != bound {
                    d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, if add { self.additive } else { self.pipeline });
                    bound = add;
                }
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[self.sets[texture], self.sets[mask]], &[]);
                d.cmd_draw(cb, count, 1, first, 0);
            }
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        if let Some((id, ..)) = &self.trace {
            crate::trace::gone(gpu.frame.get(), *id);
        }
        let d = &gpu.device;
        unsafe {
            for (b, m, ..) in self.buffers.into_iter().flatten() {
                d.unmap_memory(m);
                d.destroy_buffer(b, None);
                d.free_memory(m, None);
            }
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline(self.additive, None);
            d.destroy_pipeline_layout(self.layout, None);
            for m in self.modules {
                d.destroy_shader_module(m, None);
            }
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.sampler, None);
            for (image, mem, view) in self.textures {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(mem, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(spin: Option<Spin>) -> Quad {
        Quad { turn: false, spin, rect: [10.0, 20.0, 110.0, 70.0], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], texture: 0 }
    }

    #[test]
    fn an_unturned_quad_keeps_its_rectangle_exactly() {
        assert_eq!(quad(None).corners(), [[10.0, 20.0], [110.0, 20.0], [110.0, 70.0], [10.0, 70.0]]);
    }

    #[test]
    fn a_turn_is_clockwise_about_its_pivot() {
        // A quarter about the top-left: the top edge runs down the screen.
        let c = quad(Some(Spin::About { pivot: [10.0, 20.0], angle: std::f32::consts::FRAC_PI_2 })).corners();
        let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3;
        assert!(near(c[0], [10.0, 20.0]) && near(c[1], [10.0, 120.0]) && near(c[2], [-40.0, 120.0]));
    }
}
