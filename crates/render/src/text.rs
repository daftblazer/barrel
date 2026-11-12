//! Text on screen in the game's font (`barrel_formats::font`): laying lines out into quads
//! (plain code, tested without a GPU) and a layer that draws them over the scene, alpha
//! blended, in screen pixels.
//!
//! The dialogue window is drawn from the game's own pieces (`WinXls_EN/a15_common.g1t`,
//! texture 28: a cream speech bubble, its end cap and a leather name strap); how they are put
//! together (sizes, places, colours) is ours, as the game's layout code is not read.

use std::collections::HashMap;

use ash::vk;
use barrel_formats::font::Glyph;

use crate::{upload_texture, Gpu, Result};

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

/// The layer's textures: the font atlas and the message window pieces.
pub const FONT: usize = 0;
pub const UI: usize = 1;

/// The message window pieces' rectangles in `a15_common.g1t` texture 28 (1024 × 1024).
pub mod window {
    pub const SIZE: f32 = 1024.0;
    /// The speech bubble (its tail on the left, 41 to 64) and its end cap (rounded on the
    /// left; drawn mirrored as the right end; its last columns, 787 to 791, fade out and are
    /// left off so the join does not show).
    pub const BUBBLE: [f32; 4] = [41.0, 4.0, 208.0, 173.0];
    pub const CAP: [f32; 4] = [732.0, 4.0, 786.0, 173.0];
    /// Where the bubble's left part ends and a plain column of its middle (stretched).
    pub const SPLIT: f32 = 140.0;
    pub const FILL: [f32; 2] = [140.0, 150.0];
    /// The leather strap the name sits on.
    pub const STRAP: [f32; 4] = [14.0, 188.0, 344.0, 252.0];
}

impl Font {
    fn advance(&self, c: char) -> f32 {
        self.glyphs.get(&c).map_or(0.0, |g| g.advance as f32)
    }

    fn width(&self, s: &str) -> f32 {
        s.chars().map(|c| self.advance(c)).sum()
    }

    /// Breaks `line` into lines no wider than `max` (atlas pixels): at spaces, or anywhere in
    /// a word longer than a line (Japanese has no spaces).
    pub fn wrap(&self, line: &str, max: f32) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for word in line.split(' ') {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if self.width(&candidate) <= max {
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
    fn run(&self, s: &str, x: f32, y: f32, scale: f32, color: [f32; 4], out: &mut Vec<Quad>) {
        let mut pen = x;
        for c in s.chars() {
            let Some(g) = self.glyphs.get(&c) else { continue };
            if g.width > 3 {
                let (gx, gy, gw, gh) = (f32::from(g.x), f32::from(g.y), f32::from(g.width), f32::from(g.height));
                let left = pen + g.x_offset as f32 * scale;
                let top = y + g.y_offset as f32 * scale;
                out.push(Quad {
                    rect: [left, top, left + gw * scale, top + gh * scale],
                    uv: [gx / self.size, gy / self.size, (gx + gw) / self.size, (gy + gh) / self.size],
                    color,
                    texture: FONT,
                });
            }
            pen += g.advance as f32 * scale;
        }
    }

    /// The message window for a line on a `width` × `height` screen: the bubble across the
    /// lower part (three lines tall, more if the text needs), the speaker's name on the strap
    /// over its left end, and the text, wrapped, inside.
    pub fn dialogue(&self, speaker: Option<&str>, text: &str, width: f32, height: f32) -> Vec<Quad> {
        use window::*;
        // Sized for 1080 lines and scaled to the screen.
        let scale = height / 1080.0;
        let (left, right) = (width * 0.12, width * 0.88);
        // The bubble is drawn 1.3 times its size; the text sits inside its rounded edge.
        let k = 1.3 * scale;
        let inset_left = (SPLIT - BUBBLE[0]) * 0.55 * k;
        let max = (right - left - inset_left - 40.0 * k) / scale;
        let lines: Vec<String> = plain_lines(text).iter().flat_map(|l| self.wrap(l, max)).collect();
        let text_height = lines.len().max(3) as f32 * LINE * scale;
        let bubble_height = (BUBBLE[3] - BUBBLE[1]) * k;
        let box_height = bubble_height.max(text_height + 40.0 * k);
        let bottom = height - 30.0 * scale;
        let top = bottom - box_height;
        let uv = |r: [f32; 4]| r.map(|v| v / SIZE);
        let white = [1.0; 4];
        let left_end = left + (SPLIT - BUBBLE[0]) * k;
        let right_start = right - (CAP[2] - CAP[0]) * k;
        let mut out = vec![
            Quad { rect: [left, top, left_end, bottom], uv: uv([BUBBLE[0], BUBBLE[1], SPLIT, BUBBLE[3]]), color: white, texture: UI },
            Quad { rect: [left_end, top, right_start, bottom], uv: uv([FILL[0], BUBBLE[1], FILL[1], BUBBLE[3]]), color: white, texture: UI },
            // The cap mirrored: its rounded side on the right.
            Quad { rect: [right_start, top, right, bottom], uv: uv([CAP[2], CAP[1], CAP[0], CAP[3]]), color: white, texture: UI },
        ];
        let text_left = left + inset_left;
        let mut y = top + (box_height - lines.len().max(1) as f32 * LINE * scale) * 0.5;
        if lines.len() < 3 {
            // Short lines start where the first of three would.
            y = top + (box_height - text_height) * 0.5;
        }
        if let Some(name) = speaker.filter(|n| !n.is_empty()) {
            let sk = 0.8 * k;
            let (sw, sh) = ((STRAP[2] - STRAP[0]) * sk, (STRAP[3] - STRAP[1]) * sk);
            let (sx, sy) = (left + 30.0 * k, top - sh * 0.75);
            out.push(Quad { rect: [sx, sy, sx + sw, sy + sh], uv: uv(STRAP), color: white, texture: UI });
            let name_scale = scale * 0.85;
            let name_top = sy + (sh - LINE * name_scale) * 0.5 + 4.0 * name_scale;
            self.run(name, sx + 50.0 * sk, name_top, name_scale, [1.0, 0.95, 0.85, 1.0], &mut out);
        }
        for l in &lines {
            self.run(l, text_left, y, scale, [0.27, 0.19, 0.12, 1.0], &mut out);
            y += LINE * scale;
        }
        out
    }
}

/// A message's text as plain lines: `<CR>` breaks a line, other markup (colours, inline
/// icons) is left out.
pub fn plain_lines(text: &str) -> Vec<String> {
    text.split("<CR>")
        .map(|l| {
            let mut out = String::new();
            let mut rest = l;
            while let Some(i) = rest.find('<') {
                out.push_str(&rest[..i]);
                match rest[i..].find('>') {
                    Some(j) => rest = &rest[i + j + 1..],
                    None => {
                        rest = &rest[i..];
                        break;
                    }
                }
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
/// alpha blending) and a vertex buffer rewritten when what it shows changes.
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
    buffer: Option<(vk::Buffer, vk::DeviceMemory, usize)>,
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
            Ok(Layer { textures, sampler, set_layout, pool: dpool, sets, runs: Vec::new(), layout, pipeline, modules, buffer: None, count: 0 })
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
        if self.buffer.as_ref().is_none_or(|(_, _, cap)| *cap < bytes.len()) {
            if let Some((b, m, _)) = self.buffer.take() {
                unsafe {
                    gpu.device.device_wait_idle()?;
                    gpu.device.destroy_buffer(b, None);
                    gpu.device.free_memory(m, None);
                }
            }
            let cap = bytes.len().next_power_of_two().max(4096);
            let (b, m) = gpu.buffer(cap as u64, vk::BufferUsageFlags::VERTEX_BUFFER, true)?;
            self.buffer = Some((b, m, cap));
        }
        let (_, mem, _) = self.buffer.as_ref().unwrap();
        gpu.upload(*mem, bytes)
    }

    /// Records the draw into a render pass that is under way (viewport and scissor set).
    ///
    /// # Safety
    /// `cb` is recording inside a render pass compatible with the one the layer was made for.
    pub unsafe fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, width: u32, height: u32) {
        let Some((buf, _, _)) = self.buffer.filter(|_| self.count > 0) else { return };
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
            if let Some((b, m, _)) = self.buffer {
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
        assert_eq!(plain_lines("Hello<CR><CLGR>there<CLNR>!"), ["Hello", "there!"]);
        assert_eq!(plain_lines("a <IM12>b"), ["a b"]);
        assert_eq!(plain_lines("x < y"), ["x < y"]);
    }

    #[test]
    fn wrapping() {
        let f = font();
        // Each letter and space is 10 wide: "abc de" is 60.
        assert_eq!(f.wrap("abc de fgh", 60.0), ["abc de", "fgh"]);
        assert_eq!(f.wrap("abcdefgh", 30.0), ["abc", "def", "gh"]);
        assert_eq!(f.wrap("あああ", 80.0), ["ああ", "あ"]);
        assert_eq!(f.wrap("", 80.0), [""]);
    }

    #[test]
    fn dialogue_box() {
        let f = font();
        let q = f.dialogue(Some("ab"), "abc<CR>d", 1920.0, 1080.0);
        // Three window pieces and the strap, two name glyphs, four text glyphs.
        assert_eq!(q.len(), 4 + 2 + 4);
        assert!(q[..4].iter().all(|q| q.texture == UI));
        assert!(q[4..].iter().all(|q| q.texture == FONT));
        // The cap is mirrored, and the pieces meet.
        assert!(q[2].uv[2] < q[2].uv[0]);
        assert_eq!((q[0].rect[2], q[1].rect[2]), (q[1].rect[0], q[2].rect[0]));
        // The text's lines are a line apart, inside the window.
        assert_eq!(q[9].rect[1] - q[6].rect[1], LINE);
        assert!(q[6].rect[1] > q[0].rect[1] && q[9].rect[3] < q[0].rect[3]);
    }
}
