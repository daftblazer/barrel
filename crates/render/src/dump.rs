//! A debug aid: `BARREL_DUMP_STAGES=<dir>` makes a Ryza field render also write its own targets
//! at the stages the game's pixel mirror writes (docs/parity.md, "The pixel board"), as `.f32`
//! (little-endian f32, RGBA, rows top to bottom) with a `.txt` beside it (`width height
//! channels format`), the layout the mirror uses, in the game's units. Nothing is made or
//! recorded while the variable is unset.

use std::cell::RefCell;
use std::path::PathBuf;

use ash::vk;

use crate::{barrier, Gpu, Result};

/// How far a frame is recorded, for the stage dumps: a stage's picture is the scene as it stands
/// after the draws up to it (the later draws are left out, the frame being drawn again).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Everything: the scene, the glow, the post pass.
    Full,
    /// The field's terrain, models and water (the game's lit scene before the sky; the fog
    /// is part of each fragment's light here).
    Lit,
    /// The terrain and the models drawn as the sun's share at each pixel (the shadow and the
    /// cloud pattern, 0 to 1 in all three channels), the game's ambient-and-shadow pass's alpha.
    Shadow,
    /// And the sky plane and the dome, without the clouds' strip.
    Dome,
    /// And the clouds' strip: the sky as it ends.
    Sky,
}

impl Stop {
    /// The file name of the scene taken at this stop.
    pub fn name(self) -> &'static str {
        match self {
            Stop::Full => "scene",
            Stop::Lit => "lit",
            Stop::Shadow => "shadow",
            Stop::Dome => "dome",
            Stop::Sky => "sky",
        }
    }

    /// Whether the second pass (the sky's) is drawn, and its clouds, and the outlines.
    pub fn sky(self) -> bool {
        !matches!(self, Stop::Lit | Stop::Shadow)
    }
    pub fn clouds(self) -> bool {
        matches!(self, Stop::Sky | Stop::Full)
    }
    pub fn outlines(self) -> bool {
        self == Stop::Full
    }
}

/// How a target's texels are stored.
#[derive(Clone, Copy)]
pub enum Texels {
    Rgba8,
    /// `B10G11R11_UFLOAT_PACK32`.
    Packed,
    Rgba16F,
    /// Four 32-bit floats.
    Rgba32F,
}

impl Texels {
    pub fn of(format: vk::Format) -> Texels {
        match format {
            vk::Format::B10G11R11_UFLOAT_PACK32 => Texels::Packed,
            vk::Format::R16G16B16A16_SFLOAT => Texels::Rgba16F,
            _ => Texels::Rgba8,
        }
    }

    fn bytes(self) -> u64 {
        match self {
            Texels::Rgba8 | Texels::Packed => 4,
            Texels::Rgba16F => 8,
            Texels::Rgba32F => 16,
        }
    }
}

struct Grab {
    name: String,
    size: (u32, u32),
    texels: Texels,
    /// What the stored value is multiplied by to be in the game's units.
    scale: f32,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

pub struct Dump {
    pub dir: PathBuf,
    pending: RefCell<Vec<Grab>>,
}

impl Dump {
    pub fn from_env() -> Option<Dump> {
        let dir = PathBuf::from(std::env::var_os("BARREL_DUMP_STAGES").filter(|v| !v.is_empty())?);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Dump { dir, pending: RefCell::new(Vec::new()) })
    }

    /// Records the copy of `image` (in the shader-read layout, which it is left in) into a buffer
    /// to be written by `write`; `scale` takes its values to the game's units.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after the passes that wrote `image`.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn grab(&self, gpu: &Gpu, cb: vk::CommandBuffer, name: &str, image: vk::Image, size: (u32, u32), texels: Texels, scale: f32) {
        let d = &gpu.device;
        let Ok((buffer, memory)) = gpu.buffer(u64::from(size.0) * u64::from(size.1) * texels.bytes(), vk::BufferUsageFlags::TRANSFER_DST, true) else { return };
        let colour = vk::ImageAspectFlags::COLOR;
        barrier(d, cb, image, colour, 1, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
        let region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(colour).layer_count(1))
            .image_extent(vk::Extent3D { width: size.0, height: size.1, depth: 1 });
        unsafe { d.cmd_copy_image_to_buffer(cb, image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, buffer, &[region]) };
        barrier(d, cb, image, colour, 1, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        self.pending.borrow_mut().push(Grab { name: name.to_string(), size, texels, scale, buffer, memory });
    }

    /// Writes what was grabbed (the frame having finished) and frees the buffers.
    pub fn write(&self, gpu: &Gpu) -> Result<()> {
        let d = &gpu.device;
        for g in self.pending.take() {
            let n = (g.size.0 * g.size.1) as usize;
            let mut raw = vec![0u8; n * g.texels.bytes() as usize];
            unsafe {
                let p = d.map_memory(g.memory, 0, raw.len() as u64, vk::MemoryMapFlags::empty())? as *const u8;
                std::ptr::copy_nonoverlapping(p, raw.as_mut_ptr(), raw.len());
                d.unmap_memory(g.memory);
                d.destroy_buffer(g.buffer, None);
                d.free_memory(g.memory, None);
            }
            let mut out = Vec::with_capacity(n * 16);
            for i in 0..n {
                let rgba = match g.texels {
                    Texels::Rgba8 => std::array::from_fn(|c| f32::from(raw[4 * i + c]) / 255.0),
                    Texels::Packed => {
                        let w = u32::from_le_bytes(raw[4 * i..4 * i + 4].try_into().unwrap());
                        [unpack_float(w & 0x7ff, 6), unpack_float(w >> 11 & 0x7ff, 6), unpack_float(w >> 22 & 0x3ff, 5), 1.0]
                    }
                    Texels::Rgba32F => std::array::from_fn(|c| f32::from_le_bytes(raw[16 * i + 4 * c..16 * i + 4 * c + 4].try_into().unwrap())),
                    Texels::Rgba16F => std::array::from_fn(|c| half(u16::from_le_bytes([raw[8 * i + 2 * c], raw[8 * i + 2 * c + 1]]))),
                };
                for (c, v) in rgba.iter().enumerate() {
                    out.extend_from_slice(&(if c < 3 { v * g.scale } else { *v }).to_le_bytes());
                }
            }
            std::fs::write(self.dir.join(format!("{}.f32", g.name)), out)?;
            std::fs::write(self.dir.join(format!("{}.txt", g.name)), format!("{} {} 4 {}\n", g.size.0, g.size.1, g.name))?;
        }
        Ok(())
    }
}

/// An unsigned small float: `mantissa_bits` of mantissa under a 5-bit exponent (the 10 and 11
/// bit floats of `B10G11R11`, the 10-bit one's mantissa being 5).
fn unpack_float(v: u32, mantissa_bits: u32) -> f32 {
    let (e, m) = (v >> mantissa_bits, v & ((1 << mantissa_bits) - 1));
    let frac = m as f32 / (1 << mantissa_bits) as f32;
    match e {
        0 => frac * 2f32.powi(-14),
        31 => f32::INFINITY,
        _ => (1.0 + frac) * 2f32.powi(e as i32 - 15),
    }
}

/// An IEEE half as a float.
fn half(h: u16) -> f32 {
    let (s, e, m) = (h >> 15, i32::from(h >> 10 & 0x1f), f32::from(h & 0x3ff) / 1024.0);
    let v = match e {
        0 => m * 2f32.powi(-14),
        31 => f32::INFINITY,
        _ => (1.0 + m) * 2f32.powi(e - 15),
    };
    if s == 1 { -v } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_floats_read_back() {
        assert_eq!(half(0x3c00), 1.0);
        assert_eq!(half(0xc000), -2.0);
        assert_eq!(unpack_float(0b01111_000000, 6), 1.0);
        assert_eq!(unpack_float(0b10000_100000, 6), 3.0);
        assert_eq!(unpack_float(0b01111_00000, 5), 1.0);
    }
}
