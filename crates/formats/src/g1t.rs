//! KTGL texture packs (`*.g1t`, magic `GT1G`), loose in `Data/x64/{Ipu,WinXls,...}` and
//! inside elixir containers.
//!
//! ```text
//! header (28 bytes): magic "GT1G", version (4 ASCII digits: "0600", "4600"), file size,
//!                    offset of the offset table, texture count, platform (10 = DX11),
//!                    size of extra data (0)
//! u32 flags per texture (right after the header)
//! offset table: u32 per texture, relative to the table
//! per texture:  u8  mip count << 4 | depth
//!               u8  format
//!               u8  log2 height << 4 | log2 width
//!               u8 ×4 (0, 0, 0x10, 0x21 in every Escha texture)
//!               u8  extended header follows (0 or 1)
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
}

impl Format {
    pub fn from_code(code: u8) -> Option<Format> {
        Some(match code {
            0x01 => Format::Rgba8,
            0x06 | 0x10 | 0x59 => Format::Bc1,
            0x08 | 0x5b => Format::Bc3,
            _ => return None,
        })
    }

    pub fn level_size(self, width: u32, height: u32) -> usize {
        let (w, h) = (width.max(1) as usize, height.max(1) as usize);
        match self {
            Format::Rgba8 => w * h * 4,
            Format::Bc1 => w.div_ceil(4) * h.div_ceil(4) * 8,
            Format::Bc3 => w.div_ceil(4) * h.div_ceil(4) * 16,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Texture<'a> {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub format_code: u8,
    /// Mip levels, largest first.
    pub levels: Vec<&'a [u8]>,
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
            let format_code = h[1];
            let format = Format::from_code(format_code).ok_or(Error::Corrupt("g1t format not known"))?;
            let (mut width, mut height) = (1u32 << (h[2] & 15), 1u32 << (h[2] >> 4));
            let mut pos = at + 8;
            if h[7] != 0 {
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
                let size = format.level_size(width >> level, height >> level);
                let bytes = data.get(pos..pos + size).ok_or(Error::Corrupt("g1t texture data past the end"))?;
                levels.push(bytes);
                pos += size;
            }
            textures.push(Texture { width, height, format, format_code, levels });
        }
        Ok(G1t { version, platform, textures })
    }
}

impl Texture<'_> {
    /// The largest level as 8-bit RGBA, rows top to bottom.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let src = self.levels[0];
        match self.format {
            Format::Rgba8 => src.to_vec(),
            Format::Bc1 => crate::bc::decode(src, w, h, false),
            Format::Bc3 => crate::bc::decode(src, w, h, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(format: u8, dims: u8, ext: Option<(u32, u32)>, body: &[u8]) -> Vec<u8> {
        let mut tex = vec![0x10, format, dims, 0, 0, 0x10, 0x21, ext.is_some() as u8];
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
