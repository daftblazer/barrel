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
//! The block from 0x48 to the layer textures holds, in the executable's read order: a 0x50-byte
//! settings block, a 0x1c-byte vertex format, the layer records (count at 0x28, 32 bytes), the
//! 48-byte records (count at 0x2c), the patch tree (count at 0x30), a 16-byte record per patch
//! (count at 0x38), the draws (count at 0x3c, 24 bytes), 8 bytes per patch, the vertices (byte
//! count at 0x40, 16 bytes each) and the indices (u16, byte count at 0x44): [`Mesh`].

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

/// A terrain vertex: x and z in cells, the height as stored (the whole s16 range), the normal
/// and colour as stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: i16,
    pub height: i16,
    pub z: i16,
    pub normal: [u8; 4],
    pub colour: [u8; 4],
}

/// One draw of a patch: its pass kind (0x00 depth, 0x10 one layer alone, 0x20 lit), the program
/// within the kind (in the lit kind, the field shader pack's program less 8), the two bytes
/// after it (not settled: which blend map, presumably), the layers it mixes (none, one or two)
/// and its vertex and index ranges (indices count from `first_vertex`).
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub program: u8,
    pub kind: u8,
    pub extra: [u8; 2],
    pub layers: Vec<u8>,
    pub first_vertex: u32,
    pub vertices: u32,
    pub first_index: u32,
    pub indices: u32,
}

/// A patch (a leaf of the patch tree): its draws (a range of [`Mesh::draws`]), its vertices (a
/// range of [`Mesh::vertices`]: the patches' vertices follow each other, and a draw's
/// `first_vertex` counts from the patch's first) and its leaf's height bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub first_draw: u32,
    pub draws: u16,
    pub first_vertex: u32,
    pub low: f32,
    pub high: f32,
}

impl Patch {
    /// A stored height in world units: the s16 range spread over the leaf's height bounds.
    pub fn height(&self, stored: i16) -> f32 {
        self.low + (f32::from(stored) + 32768.0) / 65535.0 * (self.high - self.low)
    }
}

/// The terrain's own mesh: patches, their draws, the vertices and the indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub cell_size: f32,
    pub patches: Vec<Patch>,
    pub draws: Vec<Draw>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

/// The mesh from the whole file (the block from 0x48 to the layer textures).
pub fn mesh(d: &[u8]) -> Result<Mesh, Error> {
    let c = |o: usize| u32_at(d, o).map(|v| v as usize);
    let (layers, records, nodes, patches, draws, vbytes, ibytes, end) = (c(0x28)?, c(0x2c)?, c(0x30)?, c(0x38)?, c(0x3c)?, c(0x40)?, c(0x44)?, c(0x10)?);
    let cell_size = f32::from_bits(u32_at(d, 0x84)?);
    let bad = Error::Corrupt("ktf mesh");
    let mut at = 0xb4 + 32 * layers + 48 * records;
    let f = |o: usize| u32_at(d, o).map(f32::from_bits);
    // Each node's patch (+0x20, -1 for none), parent (the first integer after the record) and
    // height bounds (centre y +4, half-size y +0x14).
    let mut tree: Vec<(i32, i32, (f32, f32))> = Vec::with_capacity(nodes);
    for _ in 0..nodes {
        let flags = *d.get(at + 0x25).ok_or(Error::Corrupt("ktf node"))?;
        let (centre, half) = (f(at + 4)?, f(at + 0x14)?);
        tree.push((u32_at(d, at + 0x20)? as i32, u32_at(d, at + 0x28)? as i32, (centre - half, centre + half)));
        at += 0x28 + 4 * if flags & 1 != 0 { 9 } else { 5 };
    }
    // A patch's heights are stored against the bounds of its topmost ancestor that holds a
    // patch too (the coarse patch a finer one refines; itself when there is none).
    let mut bounds = vec![(0.0, 0.0); patches];
    for (k, &(patch, _, own)) in tree.iter().enumerate() {
        let Some(b) = usize::try_from(patch).ok().and_then(|p| bounds.get_mut(p)) else { continue };
        let (mut at_node, mut range) = (k, own);
        for _ in 0..nodes {
            let parent = tree[at_node].1;
            match usize::try_from(parent).ok().and_then(|q| tree.get(q).map(|n| (q, n))) {
                Some((q, n)) => {
                    if n.0 >= 0 {
                        range = n.2;
                    }
                    at_node = q;
                }
                None => break,
            }
        }
        *b = range;
    }
    let total = at + 16 * patches + 24 * draws + 8 * patches + vbytes + ibytes;
    if total != end || end > d.len() || vbytes % 16 != 0 || ibytes % 2 != 0 {
        return Err(bad);
    }
    let u16_at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let w = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let mut m = Mesh { cell_size, ..Default::default() };
    for k in 0..patches {
        let o = at + 16 * k;
        m.patches.push(Patch { first_draw: w(o), draws: u16_at(o + 4), first_vertex: 0, low: bounds[k].0, high: bounds[k].1 });
    }
    at += 16 * patches;
    for k in 0..draws {
        let o = at + 24 * k;
        let n = (d[o + 4] as usize).min(3);
        m.draws.push(Draw {
            program: d[o],
            kind: d[o + 1],
            extra: [d[o + 2], d[o + 3]],
            layers: d[o + 5..o + 5 + n].to_vec(),
            first_vertex: w(o + 8),
            vertices: w(o + 12),
            first_index: w(o + 16),
            indices: w(o + 20),
        });
    }
    at += 24 * draws + 8 * patches;
    // A patch's vertices end where its farthest-reaching draw does.
    let mut next = 0u32;
    for p in &mut m.patches {
        p.first_vertex = next;
        let range = m.draws.get(p.first_draw as usize..(p.first_draw as usize + usize::from(p.draws))).ok_or(Error::Corrupt("ktf patch draws"))?;
        next += range.iter().map(|d| d.first_vertex + d.vertices).max().unwrap_or(0);
    }
    if next as usize != vbytes / 16 {
        return Err(Error::Corrupt("ktf patch vertices"));
    }
    for k in 0..vbytes / 16 {
        let o = at + 16 * k;
        let s = |i: usize| u16_at(o + i) as i16;
        m.vertices.push(Vertex { x: s(0), height: s(2), z: s(4), normal: [d[o + 8], d[o + 9], d[o + 10], d[o + 11]], colour: [d[o + 12], d[o + 13], d[o + 14], d[o + 15]] });
    }
    at += vbytes;
    m.indices = (0..ibytes / 2).map(|k| u16_at(at + 2 * k)).collect();
    Ok(m)
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
