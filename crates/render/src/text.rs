//! Text on screen in a game's font (Escha's: `barrel_escha::data::font`): laying lines out into quads
//! (plain code, tested without a GPU) and a layer that draws them over the scene, alpha
//! blended, in screen pixels.
//!
//! The dialogue window is drawn from the game's own pieces (`WinXls_EN/a15_common.g1t`,
//! texture 28: a cream speech bubble, its end cap and a leather name strap); how they are put
//! together (sizes, places, colours) is ours, as the game's layout code is not read.

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

/// The font's line height in atlas pixels (the Latin glyphs reach 53 below a line's top; the
/// game's own spacing is not read).
pub const LINE: f32 = 53.0;

pub struct Font {
    pub glyphs: HashMap<char, Glyph>,
    /// The atlas's size in pixels.
    pub size: f32,
}

/// One rectangle to draw: `rect` and `uv` are (left, top, right, bottom), in pixels and
/// texture coordinates (a right edge left of the left one mirrors it); a negative `uv[0]`
/// fills the rectangle with `color` alone. `texture` is [`FONT`] or [`UI`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub texture: usize,
}

/// The text colours the markup names (`<CLGR>` ...), from the executable's table at
/// `0x1407b49b0` (English; 16-byte records `{u64 name, u8 r, g, b, 0}`, in this order).
pub const COLOURS: [(&str, [u8; 3]); 11] = [
    ("NR", [0xff, 0xff, 0xff]),
    ("BK", [0x00, 0x00, 0x00]),
    ("RE", [0xc8, 0x00, 0x00]),
    ("GR", [0x00, 0xaa, 0x00]),
    ("BL", [0x50, 0x50, 0xf0]),
    ("R1", [0xff, 0x50, 0x50]),
    ("G1", [0x0a, 0xf0, 0x00]),
    ("P1", [0x88, 0x22, 0x44]),
    ("YL", [0xff, 0xff, 0x00]),
    ("EG", [0x1e, 0xff, 0x1e]),
    ("BR", [0x3a, 0x1a, 0x08]),
];

/// The first button icon's character: `<IMnn>` is the font's glyph for U+0410 + nn (codes
/// `0xd090` on, which would be Cyrillic А to а; the game uses them for its 33 icons).
pub const ICONS: u32 = 0x410;
/// Colour changes ride in the laid-out text as these private-use characters (U+E000 + the
/// colour's place in [`COLOURS`]; U+E0FF goes back to the line's own colour). They have no
/// glyph, so they take no room.
const COLOUR_MARK: u32 = 0xe000;
const COLOUR_RESET: u32 = 0xe0ff;

fn colour(rgb: [u8; 3]) -> [f32; 4] {
    [f32::from(rgb[0]) / 255.0, f32::from(rgb[1]) / 255.0, f32::from(rgb[2]) / 255.0, 1.0]
}

/// The layer's textures: the font atlas and the message window pieces.
pub const FONT: usize = 0;
pub const UI: usize = 1;

/// The message window pieces' rectangles in `a15_common.g1t` texture 4 (1024 × 1024), and
/// where the game draws them on a 1280 × 720 screen (measured from an oracle frame of
/// `EVENT_MM01_OP1_111`, `docs/renderer.md`, "Dialogue").
pub mod window {
    pub const SIZE: f32 = 1024.0;
    /// The framed parchment panel.
    pub const PANEL: [f32; 4] = [0.0, 532.0, 757.0, 738.0];
    /// The brown name tag with its grommet.
    pub const TAG: [f32; 4] = [0.0, 298.0, 408.0, 372.0];
    /// The cord hanging from the tag's grommet.
    pub const CORD: [f32; 4] = [585.0, 2.0, 635.0, 260.0];
    /// On the 1280 × 720 screen: the panel, the tag (drawn level; the game tilts it about 5°),
    /// the cord, the text's left edge and first line's top, the text's widest line, and the
    /// name's centre.
    pub const AT_PANEL: [f32; 4] = [360.0, 562.0, 960.0, 710.0];
    pub const AT_TAG: [f32; 4] = [352.0, 535.0, 627.0, 585.0];
    pub const AT_CORD: [f32; 4] = [340.0, 548.0, 374.0, 722.0];
    pub const AT_TEXT: [f32; 2] = [409.0, 596.0];
    pub const TEXT_WIDTH: f32 = 540.0;
    pub const AT_NAME: [f32; 2] = [505.0, 560.0];
    /// The game's first line, "You haven't forgotten anything, have you, Escha?", spans 482
    /// pixels there: the text is sized to match.
    pub const SAMPLE: &str = "You haven't forgotten anything, have you, Escha?";
    pub const SAMPLE_WIDTH: f32 = 482.0;
}

/// The most characters on one line of the message window, as the game breaks its English
/// text (measured in the openings: docs/renderer.md, "Message window").
pub const LINE_CHARS: usize = 48;

impl Font {
    fn advance(&self, c: char) -> f32 {
        self.glyphs.get(&c).map_or(0.0, |g| g.advance as f32)
    }

    fn width(&self, s: &str) -> f32 {
        s.chars().map(|c| self.advance(c)).sum()
    }

    /// Breaks `line` into lines of at most [`LINE_CHARS`] characters and `max` atlas pixels: at
    /// spaces, or anywhere in a word longer than a line (Japanese has no spaces).
    pub fn wrap(&self, line: &str, max: f32) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for word in line.split(' ') {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            // The game breaks English lines by count, at most LINE_CHARS characters (it keeps
            // "…, have you, Escha?", 48 and 480 pixels, yet moves "trip," after "…for a long",
            // 54 and 464 pixels); the width only guards wide glyphs.
            if candidate.chars().count() <= LINE_CHARS && self.width(&candidate) <= max {
                cur = candidate;
                continue;
            }
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            for c in word.chars() {
                if !cur.is_empty() && self.width(&cur) + self.advance(c) > max {
                    out.push(std::mem::take(&mut cur));
                }
                cur.push(c);
            }
        }
        if !cur.is_empty() || out.is_empty() {
            out.push(cur);
        }
        out
    }

    /// Glyph quads for `s` with its line's top-left at (x, y), `scale` screen pixels per atlas
    /// pixel.
    fn run(&self, s: &str, x: f32, y: f32, scale: f32, base: [f32; 4], out: &mut Vec<Quad>) {
        let mut pen = x;
        let mut color = base;
        for c in s.chars() {
            match u32::from(c) {
                COLOUR_RESET => color = base,
                // `<CLNR>` (normal) is the line's own colour too: white is for dark windows.
                n if n == COLOUR_MARK => color = base,
                n if (COLOUR_MARK..COLOUR_MARK + COLOURS.len() as u32).contains(&n) => color = colour(COLOURS[(n - COLOUR_MARK) as usize].1),
                _ => {}
            }
            let Some(g) = self.glyphs.get(&c) else { continue };
            // Icons keep their own colours.
            let tint = if (ICONS..ICONS + 33).contains(&u32::from(c)) { [1.0; 4] } else { color };
            if g.width > 3 {
                let (gx, gy, gw, gh) = (f32::from(g.x), f32::from(g.y), f32::from(g.width), f32::from(g.height));
                let left = pen + g.x_offset as f32 * scale;
                let top = y + g.y_offset as f32 * scale;
                out.push(Quad {
                    rect: [left, top, left + gw * scale, top + gh * scale],
                    uv: [gx / self.size, gy / self.size, (gx + gw) / self.size, (gy + gh) / self.size],
                    color: tint,
                    texture: FONT,
                });
            }
            pen += g.advance as f32 * scale;
        }
    }

    /// The widest line of layout text `text`, in atlas pixels.
    pub fn width_of(&self, text: &str) -> f32 {
        plain_lines(text).iter().map(|l| self.width(l)).fold(0.0, f32::max)
    }

    /// One line of layout text (`<IMnn>` icons and `<CLxx>` colours read) with its top-left at
    /// (x, y), `scale` screen pixels per atlas pixel.
    pub fn line(&self, text: &str, x: f32, y: f32, scale: f32, colour: [f32; 4], out: &mut Vec<Quad>) {
        let mut y = y;
        for l in plain_lines(text) {
            self.run(&l, x, y, scale, colour, out);
            y += LINE * scale;
        }
    }

    /// The message window for a line on a `width` × `height` screen, as the game lays it out
    /// (`window`): the parchment panel, the name tag and its cord, the name, and the text,
    /// wrapped to the panel. The 1280 × 720 layout is scaled to the height and centred.
    pub fn dialogue(&self, speaker: Option<&str>, text: &str, width: f32, height: f32) -> Vec<Quad> {
        use window::*;
        let k = height / 720.0;
        let dx = (width - 1280.0 * k) / 2.0;
        let place = |r: [f32; 4]| [dx + r[0] * k, r[1] * k, dx + r[2] * k, r[3] * k];
        let uv = |r: [f32; 4]| r.map(|v| v / SIZE);
        let white = [1.0; 4];
        // Text at the size the game's sample line has (in screen pixels per atlas pixel).
        let text_scale = SAMPLE_WIDTH / self.width(SAMPLE).max(1.0) * k;
        let lines: Vec<String> = plain_lines(text).iter().flat_map(|l| self.wrap(l, TEXT_WIDTH * k / text_scale)).collect();
        let mut out = vec![Quad { rect: place(AT_PANEL), uv: uv(PANEL), color: white, texture: UI }];
        if let Some(name) = speaker.filter(|n| !n.is_empty()) {
            out.push(Quad { rect: place(AT_CORD), uv: uv(CORD), color: white, texture: UI });
            out.push(Quad { rect: place(AT_TAG), uv: uv(TAG), color: white, texture: UI });
            let name_scale = text_scale * 0.95;
            let (cx, cy) = (dx + AT_NAME[0] * k, AT_NAME[1] * k);
            let w = self.width(name) * name_scale;
            self.run(name, cx - w / 2.0, cy - LINE * name_scale * 0.5, name_scale, [0.96, 0.93, 0.86, 1.0], &mut out);
        }
        let mut y = AT_TEXT[1] * k;
        for l in &lines {
            // The game's brown (`BR`) on the parchment.
            self.run(l, dx + AT_TEXT[0] * k, y, text_scale, colour(COLOURS[10].1), &mut out);
            y += LINE * text_scale;
        }
        out
    }
}

/// A message's text as lines ready to lay out: `<CR>` breaks a line, `<IMnn>` becomes the
/// icon's character ([`ICONS`]), `<CLxx>` a colour mark, and other markup is left out.
pub fn plain_lines(text: &str) -> Vec<String> {
    text.split("<CR>")
        .map(|l| {
            let mut out = String::new();
            let mut rest = l;
            while let Some(i) = rest.find('<') {
                out.push_str(&rest[..i]);
                let Some(j) = rest[i..].find('>') else {
                    rest = &rest[i..];
                    break;
                };
                let tag = &rest[i + 1..i + j];
                if let Some(n) = tag.strip_prefix("IM").and_then(|n| n.parse::<u32>().ok()) {
                    out.extend(char::from_u32(ICONS + n));
                } else if let Some(name) = tag.strip_prefix("CL") {
                    let mark = COLOURS.iter().position(|(c, _)| *c == name).map_or(COLOUR_RESET, |k| COLOUR_MARK + k as u32);
                    out.extend(char::from_u32(mark));
                } else if tag.chars().any(|c| !c.is_ascii_alphanumeric()) {
                    // Not markup ("x < y > z").
                    out.push_str(&rest[i..i + j + 1]);
                }
                rest = &rest[i + j + 1..];
            }
            out.push_str(rest);
            out
        })
        .collect()
}

const TEXT_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.vert.spv"));
const TEXT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.frag.spv"));

/// A vertex: position (pixels), atlas coordinates, colour.
#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

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
    /// Runs of vertices by texture: (texture, first vertex, count).
    runs: Vec<(usize, u32, u32)>,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    /// The vertex buffers (buffer, memory, capacity, mapped), the one drawn from, and the frame
    /// it was last written in.
    buffers: [Option<(vk::Buffer, vk::DeviceMemory, usize, *mut u8)>; crate::ring::SLOTS],
    current: usize,
    written: Option<u64>,
    count: u32,
}

impl Layer {
    pub fn new(gpu: &Gpu, pool: vk::CommandPool, render_pass: vk::RenderPass, images: &[&barrel_formats::g1t::Texture]) -> Result<Layer> {
        let d = &gpu.device;
        let textures = images.iter().map(|t| upload_texture(gpu, pool, t)).collect::<Result<Vec<_>>>()?;
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
            let layouts = [set_layout];
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
            Ok(Layer { textures, sampler, set_layout, pool: dpool, sets, runs: Vec::new(), layout, pipeline, modules, buffers: [None; crate::ring::SLOTS], current: 0, written: None, count: 0 })
        }
    }

    /// Sets what the next frames draw (the buffer grows as needed).
    pub fn set(&mut self, gpu: &Gpu, quads: &[Quad]) -> Result<()> {
        let mut verts = Vec::with_capacity(quads.len() * 6);
        self.runs.clear();
        for q in quads {
            let texture = q.texture.min(self.textures.len().saturating_sub(1));
            match self.runs.last_mut() {
                Some((t, _, count)) if *t == texture => *count += 6,
                _ => self.runs.push((texture, verts.len() as u32, 6)),
            }
            let [l, t, r, b] = q.rect;
            let [u0, v0, u1, v1] = q.uv;
            let v = |x, y, u, w| Vertex { pos: [x, y], uv: [u, w], color: q.color };
            let (u1, v1) = if u0 < 0.0 { (u0, v0) } else { (u1, v1) };
            verts.extend([v(l, t, u0, v0), v(r, t, u1, v0), v(r, b, u1, v1), v(l, t, u0, v0), v(r, b, u1, v1), v(l, b, u0, v1)]);
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
        let Some((buf, ..)) = self.buffers[self.current].filter(|_| self.count > 0) else { return };
        unsafe {
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            let screen = [width as f32, height as f32];
            d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX, 0, std::slice::from_raw_parts(screen.as_ptr() as *const u8, 8));
            d.cmd_bind_vertex_buffers(cb, 0, &[buf], &[0]);
            for &(texture, first, count) in &self.runs {
                d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[self.sets[texture]], &[]);
                d.cmd_draw(cb, count, 1, first, 0);
            }
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            for (b, m, ..) in self.buffers.into_iter().flatten() {
                d.unmap_memory(m);
                d.destroy_buffer(b, None);
                d.free_memory(m, None);
            }
            d.destroy_pipeline(self.pipeline, None);
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

    fn font() -> Font {
        let g = |advance| Glyph { x: 0, y: 0, width: 10, height: 20, x_offset: 0, y_offset: 10, advance };
        let glyphs = "abcdefghijklmnopqrstuvwxyz".chars().map(|c| (c, g(10))).chain([(' ', g(10)), ('あ', g(40))]).collect();
        Font { glyphs, size: 2048.0 }
    }

    #[test]
    fn markup_and_breaks() {
        assert_eq!(plain_lines("Hello<CR><CLGR>there<CLNR>!"), ["Hello", "\u{e003}there\u{e000}!"]);
        assert_eq!(plain_lines("Press <IM15> now"), ["Press \u{41f} now"]);
        assert_eq!(plain_lines("a<XX>b<CLZZ>c"), ["ab\u{e0ff}c"]);
        assert_eq!(plain_lines("x < y"), ["x < y"]);
        assert_eq!(plain_lines("x < y > z"), ["x < y > z"]);
    }

    #[test]
    fn colours_and_icons() {
        let mut f = font();
        f.glyphs.insert('\u{410}', Glyph { x: 0, y: 0, width: 10, height: 20, x_offset: 0, y_offset: 10, advance: 10 });
        let mut out = Vec::new();
        let line = &plain_lines("a<CLGR>b<CLNR>c<IM00>")[0];
        f.run(line, 0.0, 0.0, 1.0, [0.5; 4], &mut out);
        let colours: Vec<[f32; 4]> = out.iter().map(|q| q.color).collect();
        assert_eq!(colours, [[0.5; 4], colour([0, 0xaa, 0]), [0.5; 4], [1.0; 4]]);
        // Marks take no room: the four glyphs sit 10 apart.
        assert_eq!(out.iter().map(|q| q.rect[0]).collect::<Vec<_>>(), [0.0, 10.0, 20.0, 30.0]);
    }

    #[test]
    fn wrapping() {
        let f = font();
        // Each letter and space is 10 wide: "abc de" is 60.
        assert_eq!(f.wrap("abc de fgh", 60.0), ["abc de", "fgh"]);
        assert_eq!(f.wrap("abcdefgh", 30.0), ["abc", "def", "gh"]);
        assert_eq!(f.wrap("あああ", 80.0), ["ああ", "あ"]);
        assert_eq!(f.wrap("", 80.0), [""]);
        let long = "...Finally, I'm here. I knew I was in for a long trip, it was farther than I expected.";
        assert_eq!(f.wrap(long, 1e9)[0], "...Finally, I'm here. I knew I was in for a long");
    }

    #[test]
    fn dialogue_box() {
        let f = font();
        let q = f.dialogue(Some("ab"), "abc<CR>d", 1920.0, 1080.0);
        // The panel, the cord and the tag, two name glyphs, four text glyphs.
        assert_eq!(q.len(), 3 + 2 + 4);
        assert!(q[..3].iter().all(|q| q.texture == UI));
        assert!(q[3..].iter().all(|q| q.texture == FONT));
        // At 1080 lines the 720 layout is 1.5 times as large: the panel from x 540 (360 × 1.5).
        assert_eq!(q[0].rect[0], 540.0);
        // The text's lines are a (scaled) line apart, inside the panel.
        let scale = window::SAMPLE_WIDTH / f.width(window::SAMPLE) * 1.5;
        assert!((q[8].rect[1] - q[5].rect[1] - LINE * scale).abs() < 1e-3);
        assert!(q[5].rect[1] > q[0].rect[1] && q[8].rect[3] < q[0].rect[3]);
    }
}
