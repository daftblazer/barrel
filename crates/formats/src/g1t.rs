//! KTGL texture packs (`*.g1t`, magic `GT1G`), loose in `Data/x64/{Ipu,WinXls,...}` and
//! inside elixir containers.
//!
//! ```text
//! header (28 bytes): magic "GT1G", version (4 ASCII digits: "0600", "4600"), file size,
//!                    offset of the offset table, texture count, platform (10 = DX11),
//!                    size of extra data (0)
//! u32 flags per texture (right after the header)
//! offset table: u32 per texture, relative to the table
//! per texture:  u8  mip count << 4 | kind (0 2D, 1 cube map, 2 volume, 3 array, 4 cube-map array)
//!               u8  format
//!               u8  log2 height << 4 | log2 width
//!               u8 ×4 (0, 0, 0x10, 0x21 in model textures; other values in terrain)
//!               u8  flags: 0x10 = an extended header follows (0x02 in terrain textures)
//!               [u32 extended size (12 or 20), then size-4 bytes; at 20, width and height
//!                as u32 at +12 and +16, used when they are not powers of two]
//!               the mip levels, largest first, each tightly packed
//! ```

use crate::{Error, u32_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// 8-bit RGBA, 4 bytes a pixel.
    Rgba8,
    /// BC1 / DXT1, 8 bytes a 4×4 block.
    Bc1,
    /// BC3 / DXT5, 16 bytes a 4×4 block.
    Bc3,
    /// BC4, 8 bytes a 4×4 block: one channel.
    Bc4,
    /// BC5, 16 bytes a 4×4 block: two channels.
    Bc5,
    /// BC6H, 16 bytes a 4×4 block: HDR colour (the later games' environment cube maps).
    Bc6h,
    /// BC7, 16 bytes a 4×4 block (Sophie 2 and later).
    Bc7,
    /// One 32-bit float a pixel (Ryza 3's heightmaps, format 0x02).
    Float32,
    /// Four 16-bit floats a pixel, RGBA (Sophie 2's irradiance volumes, format 0x03).
    Rgba16f,
}

impl Format {
    pub fn from_code(code: u8) -> Option<Format> {
        Some(match code {
            0x01 => Format::Rgba8,
            0x06 | 0x10 | 0x59 | 0x60 => Format::Bc1,
            0x08 | 0x5b => Format::Bc3,
            0x02 => Format::Float32,
            0x03 => Format::Rgba16f,
            0x5c => Format::Bc4,
            0x5d => Format::Bc5,
            0x5e => Format::Bc6h,
            0x5f => Format::Bc7,
            _ => return None,
        })
    }

    pub fn level_size(self, width: u32, height: u32) -> usize {
        let (w, h) = (width.max(1) as usize, height.max(1) as usize);
        match self {
            Format::Rgba8 | Format::Float32 => w * h * 4,
            Format::Rgba16f => w * h * 8,
            Format::Bc4 => w.div_ceil(4) * h.div_ceil(4) * 8,
            Format::Bc1 => w.div_ceil(4) * h.div_ceil(4) * 8,
            Format::Bc3 | Format::Bc5 | Format::Bc6h | Format::Bc7 => w.div_ceil(4) * h.div_ceil(4) * 16,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Texture<'a> {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub format_code: u8,
    /// Images per level: 1, 6 for a cube map, or the depth of a volume (halving each level,
    /// down to 1; [`Texture::layers`] gives a level's count).
    pub layers: u32,
    /// Whether `layers` is a volume's depth (which halves with each level).
    pub volume: bool,
    /// Mip levels, largest first, each holding all its layers one after another.
    pub levels: Vec<&'a [u8]>,
}

impl<'a> Texture<'a> {
    /// The texture without its `n` largest mip levels (a game's texture mip bias: half the size
    /// per level dropped), where it has levels to spare and stays at least 4 × 4 (a compressed
    /// block); as it is otherwise, and always for a cube map or volume.
    pub fn without_top_levels(&self, n: usize) -> Texture<'a> {
        let mut t = self.clone();
        if n == 0 || self.layers != 1 || self.volume || self.levels.len() <= n || (self.width >> n) < 4 || (self.height >> n) < 4 {
            return t;
        }
        t.width >>= n;
        t.height >>= n;
        t.levels.drain(..n);
        t
    }
}

#[derive(Clone, Debug)]
pub struct G1t<'a> {
    pub version: [u8; 4],
    pub platform: u32,
    pub textures: Vec<Texture<'a>>,
}

const MAX_TEXTURES: usize = 4096;
const MAX_SIDE: u32 = 16384;

impl<'a> G1t<'a> {
    pub fn parse(data: &'a [u8]) -> Result<G1t<'a>, Error> {
        if !data.starts_with(b"GT1G") {
            return Err(Error::NotThisFormat);
        }
        let version: [u8; 4] = data[4..8].try_into().unwrap();
        let total = u32_at(data, 8)? as usize;
        let table = u32_at(data, 12)? as usize;
        let count = u32_at(data, 16)? as usize;
        let platform = u32_at(data, 20)?;
        if total > data.len() || count > MAX_TEXTURES {
            return Err(Error::Corrupt("g1t header"));
        }
        let data = &data[..total];
        let mut textures = Vec::with_capacity(count);
        for i in 0..count {
            let at = table + u32_at(data, table + 4 * i)? as usize;
            let h = data.get(at..at + 8).ok_or(Error::Corrupt("g1t texture header past the end"))?;
            let mips = (h[0] >> 4).max(1) as u32;
            // The low nibble: 0 a 2D texture, 1 a cube map (six faces), 2 a volume of
            // 2^(byte 3's low nibble) slices.
            // 3 an array and 4 an array of cube maps, their count in the extended header's
            // third word (bits 4..16).
            let array = if h[7] & 0x10 != 0 && u32_at(data, at + 8)? >= 12 {
                (u32_at(data, at + 16)? >> 4) & 0xfff
            } else {
                0
            };
            let (layers, volume) = match h[0] & 15 {
                0 => (1, false),
                1 => (6, false),
                2 => (1u32 << (h[3] & 15), true),
                3 if array > 0 => (array, false),
                4 if array > 0 => (6 * array, false),
                _ => return Err(Error::Corrupt("g1t texture kind not known")),
            };
            let format_code = h[1];
            let format = Format::from_code(format_code).ok_or(Error::Corrupt("g1t format not known"))?;
            let (mut width, mut height) = (1u32 << (h[2] & 15), 1u32 << (h[2] >> 4));
            let mut pos = at + 8;
            if h[7] & 0x10 != 0 {
                let ext = u32_at(data, pos)? as usize;
                if !(4..=64).contains(&ext) {
                    return Err(Error::Corrupt("g1t extended header size"));
                }
                if ext >= 20 {
                    width = u32_at(data, pos + 12)?;
                    height = u32_at(data, pos + 16)?;
                }
                pos += ext;
            }
            if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
                return Err(Error::Corrupt("g1t texture size"));
            }
            let mut levels = Vec::with_capacity(mips as usize);
            for level in 0..mips {
                let n = if volume { (layers >> level).max(1) } else { layers } as usize;
                let size = format.level_size(width >> level, height >> level) * n;
                let bytes = data.get(pos..pos + size).ok_or(Error::Corrupt("g1t texture data past the end"))?;
                levels.push(bytes);
                pos += size;
            }
            textures.push(Texture { width, height, format, format_code, layers, volume, levels });
        }
        Ok(G1t { version, platform, textures })
    }
}

impl Texture<'_> {
    /// The largest level's first layer as 8-bit RGBA, rows top to bottom. BC6H is not
    /// decoded in software yet and comes out black; a float heightmap comes out grey, its
    /// range stretched to 0..255.
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.layer_rgba8(0).unwrap_or_default()
    }

    /// Layer `layer` of the largest level (a cube face, an array slice or a volume slice) as
    /// RGBA8; `None` past the last layer.
    pub fn layer_rgba8(&self, layer: u32) -> Option<Vec<u8>> {
        let size = self.format.level_size(self.width, self.height);
        let start = size * layer as usize;
        let level = self.levels.first()?;
        if layer >= self.layers.max(1) || start >= level.len() {
            return None;
        }
        Some(self.decode(&level[start..(start + size).min(level.len())]))
    }

    /// Layer `layer` of the largest level as linear RGB, for the HDR formats (BC6H); `None`
    /// for others or past the last layer.
    pub fn layer_rgb(&self, layer: u32) -> Option<Vec<[f32; 3]>> {
        let size = self.format.level_size(self.width, self.height);
        let start = size * layer as usize;
        let level = self.levels.first()?;
        if self.format != Format::Bc6h || layer >= self.layers.max(1) || start + size > level.len() {
            return None;
        }
        Some(crate::bc6h::decode_rgb(&level[start..start + size], self.width as usize, self.height as usize, false))
    }

    fn decode(&self, src: &[u8]) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        match self.format {
            Format::Rgba8 => src.to_vec(),
            Format::Bc4 => crate::bc::decode_bc45(src, w, h, false),
            Format::Bc5 => crate::bc::decode_bc45(src, w, h, true),
            // HDR: tone-mapped for viewing (unsigned BC6H; no signed sample has been seen).
            Format::Bc6h => crate::bc6h::decode(src, w, h, false),
            // HDR: each channel tone-mapped (x / (1 + x), gamma 2.2) for viewing; alpha as is.
            Format::Rgba16f => src
                .chunks_exact(8)
                .flat_map(|p| {
                    let h = |i: usize| crate::bc6h::half_to_f32(u16::from_le_bytes([p[2 * i], p[2 * i + 1]]));
                    let t = |f: f32| {
                        let f = f.max(0.0);
                        ((f / (1.0 + f)).powf(1.0 / 2.2) * 255.0 + 0.5) as u8
                    };
                    [t(h(0)), t(h(1)), t(h(2)), (h(3).clamp(0.0, 1.0) * 255.0 + 0.5) as u8]
                })
                .collect(),
            Format::Float32 => {
                let v: Vec<f32> = src.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
                let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
                let span = (hi - lo).max(1e-6);
                v.iter().flat_map(|&x| { let g = ((x - lo) / span * 255.0) as u8; [g, g, g, 255] }).collect()
            }
            Format::Bc1 => crate::bc::decode(src, w, h, false),
            Format::Bc3 => crate::bc::decode(src, w, h, true),
            Format::Bc7 => crate::bc7::decode(src, w, h),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn drops_top_levels_while_blocks_remain() {
        let px = [0u8; 64];
        let t = Texture { width: 16, height: 8, format: Format::Bc1, format_code: 0, layers: 1, volume: false, levels: vec![&px[..64], &px[..16], &px[..8]] };
        let one = t.without_top_levels(1);
        assert_eq!((one.width, one.height, one.levels.len()), (8, 4, 2));
        assert_eq!(t.without_top_levels(2).width, 16);
        assert_eq!(t.without_top_levels(0).levels.len(), 3);
    }

    use super::*;

    fn pack(format: u8, dims: u8, ext: Option<(u32, u32)>, body: &[u8]) -> Vec<u8> {
        let mut tex = vec![0x10, format, dims, 0, 0, 0x10, 0x21, if ext.is_some() { 0x10 } else { 0x02 }];
        if let Some((w, h)) = ext {
            for v in [20u32, 0, 0, w, h] {
                tex.extend_from_slice(&v.to_le_bytes());
            }
        }
        tex.extend_from_slice(body);
        let mut f = b"GT1G0600".to_vec();
        let total = 28 + 4 + 4 + tex.len();
        for v in [total as u32, 32, 1, 10, 0, 0] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&4u32.to_le_bytes());
        f.extend_from_slice(&tex);
        f
    }

    #[test]
    fn rgba_and_extended_size() {
        let px = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let f = pack(0x01, 0x01, None, &px); // 2 wide, 1 high
        let g = G1t::parse(&f).unwrap();
        assert_eq!((g.textures[0].width, g.textures[0].height), (2, 1));
        assert_eq!(g.textures[0].to_rgba8(), px);
        let f = pack(0x59, 0, Some((6, 2)), &[0u8; 16]); // 2×1 BC1 blocks
        let g = G1t::parse(&f).unwrap();
        assert_eq!((g.textures[0].width, g.textures[0].height, g.textures[0].format), (6, 2, Format::Bc1));
        assert!(G1t::parse(&pack(0x59, 0, Some((8, 8)), &[0u8; 16])).is_err());
    }
}
