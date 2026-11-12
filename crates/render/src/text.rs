//! Text on screen in the game's font (`barrel_formats::font`): laying lines out into quads
//! (plain code, tested without a GPU) and a layer that draws them over the scene, alpha
//! blended, in screen pixels.
//!
//! The dialogue box is ours for now (a dark translucent panel with the speaker's name over
//! the text); the game's own window art is not read yet.

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

/// One rectangle to draw: `rect` and `uv` are (left, top, right, bottom), in pixels and atlas
/// coordinates; a negative `uv[0]` fills the rectangle with `color` alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
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
                });
            }
            pen += g.advance as f32 * scale;
        }
    }

    /// The dialogue box for a line on a `width` × `height` screen: a panel across the lower
    /// part, the speaker's name (if any) and the text, wrapped, under it.
    pub fn dialogue(&self, speaker: Option<&str>, text: &str, width: f32, height: f32) -> Vec<Quad> {
        // Sized for 1080 lines and scaled to the screen.
        let scale = height / 1080.0;
        let pad = 28.0 * scale;
        let (left, right) = (width * 0.12, width * 0.88);
        let max = (right - left - 2.0 * pad) / scale;
        let lines: Vec<String> = plain_lines(text).iter().flat_map(|l| self.wrap(l, max)).collect();
        let rows = lines.len().max(3) as f32 + if speaker.is_some() { 1.0 } else { 0.0 };
        let bottom = height - 36.0 * scale;
        let top = bottom - rows * LINE * scale - 2.0 * pad;
        let mut out = vec![Quad { rect: [left, top, right, bottom], uv: [-1.0; 4], color: [0.04, 0.04, 0.07, 0.78] }];
        let mut y = top + pad;
        if let Some(name) = speaker.filter(|n| !n.is_empty()) {
            self.run(name, left + pad, y, scale, [1.0, 0.82, 0.55, 1.0], &mut out);
            y += LINE * scale;
        }
        for l in &lines {
            self.run(l, left + pad, y, scale, [1.0, 1.0, 1.0, 1.0], &mut out);
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

/// What draws text: the atlas, its pipeline (no depth, alpha blending) and a vertex buffer
/// rewritten each frame.
pub struct Layer {
    atlas: (vk::Image, vk::DeviceMemory, vk::ImageView),
    sampler: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    buffer: Option<(vk::Buffer, vk::DeviceMemory, usize)>,
    count: u32,
}

impl Layer {
    pub fn new(gpu: &Gpu, pool: vk::CommandPool, render_pass: vk::RenderPass, atlas: &barrel_formats::g1t::Texture) -> Result<Layer> {
        let d = &gpu.device;
        let atlas = upload_texture(gpu, pool, atlas)?;
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
            let sizes = [vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 1 }];
            let dpool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None)?;
            let layouts = [set_layout];
            let set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(dpool).set_layouts(&layouts))?[0];
            let info = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(atlas.2).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            d.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&info)],
                &[],
            );
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
            Ok(Layer { atlas, sampler, set_layout, pool: dpool, set, layout, pipeline, modules, buffer: None, count: 0 })
        }
    }

    /// Sets what the next frames draw (the buffer grows as needed).
    pub fn set(&mut self, gpu: &Gpu, quads: &[Quad]) -> Result<()> {
        let mut verts = Vec::with_capacity(quads.len() * 6);
        for q in quads {
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
            d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[self.set], &[]);
            d.cmd_bind_vertex_buffers(cb, 0, &[buf], &[0]);
            d.cmd_draw(cb, self.count, 1, 0, 0);
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
            d.destroy_image_view(self.atlas.2, None);
            d.destroy_image(self.atlas.0, None);
            d.free_memory(self.atlas.1, None);
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
        // The panel, two name glyphs, four text glyphs (spaces draw nothing).
        assert_eq!(q.len(), 1 + 2 + 4);
        assert!(q[0].uv[0] < 0.0);
        // The name sits a line above the text.
        assert_eq!(q[3].rect[1] - q[1].rect[1], LINE);
    }
}
