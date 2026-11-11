//! Field terrain (`<AREA>.ktf`, magic `KFTK`, version "0820" in Escha & Logy DX).
//!
//! The terrain is the field's heightmap textured by blending ground layers. The file bundles
//! everything for it:
//!
//! | Offset | What |
//! |---|---|
//! | 0x0c | u32 file size |
//! | 0x10 | u32 offset of the layer textures (a `g1t`: sand, dirt, grass and the like) |
//! | 0x14 | u32 offset of the blend maps (a `g1t` of two 512×512 BC1 maps; RGB are layer weights) |
//! | 0x1c | u32 offset of the heightmap (a `khm`, the same as the field's own `.khm`) |
//! | 0x84 | f32 cell size (100), f32 25, f32 1/256 |
//! | 0x90 | u16 cells across, u16 cells down (64, 64) |
//!
//! The block from 0x98 to the layer textures (layer settings and a tree of patches with
//! bounds) is not decoded yet.

use crate::{Error, g1t::G1t, khm, u32_at};

pub struct Terrain<'a> {
    pub layers: G1t<'a>,
    pub blend: G1t<'a>,
    pub heightmap: khm::Heightmap,
    /// The undecoded block between the header and the layer textures.
    pub settings: &'a [u8],
}

impl Terrain<'_> {
    /// Each layer's tile scale (1 / world units per tile repeat), from the layer records
    /// that start 0x1c into the settings block: 32 bytes each, `{u16 layer, u16 0xffff,
    /// u16, u8, u8, f32 u scale, f32 v scale, ...}`, one per layer texture.
    pub fn layer_scales(&self) -> Vec<f32> {
        let mut out = Vec::new();
        let mut at = 0x1c;
        while at + 16 <= self.settings.len() && out.len() < self.layers.textures.len() {
            let s = &self.settings[at..];
            if u16::from_le_bytes([s[2], s[3]]) != 0xffff {
                break;
            }
            out.push(f32::from_le_bytes([s[8], s[9], s[10], s[11]]));
            at += 0x20;
        }
        out
    }
}

pub fn parse(d: &[u8]) -> Result<Terrain<'_>, Error> {
    if !d.starts_with(b"KFTK") {
        return Err(Error::NotThisFormat);
    }
    let size = u32_at(d, 0x0c)? as usize;
    let (layers_at, blend_at, height_at) = (u32_at(d, 0x10)? as usize, u32_at(d, 0x14)? as usize, u32_at(d, 0x1c)? as usize);
    if size > d.len() || !(0x98 <= layers_at && layers_at < blend_at && blend_at < height_at && height_at < size) {
        return Err(Error::Corrupt("ktf section offsets"));
    }
    Ok(Terrain {
        layers: G1t::parse(&d[layers_at..blend_at])?,
        blend: G1t::parse(&d[blend_at..height_at])?,
        heightmap: khm::parse(&d[height_at..size])?,
        settings: &d[0x98..layers_at],
    })
}
