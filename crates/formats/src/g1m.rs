//! KTGL models (`*.g1m`, magic `_M1G`), inside elixir containers.
//!
//! A model is a header and a list of chunks, each `{magic, version, size}` with the size
//! counting its 12-byte header:
//!
//! | Chunk | What |
//! |---|---|
//! | `FM1G` (G1MF) | counts and statistics |
//! | `SM1G` (G1MS) | the skeleton, see [`Skeleton`] |
//! | `MM1G` (G1MM) | matrices |
//! | `GM1G` (G1MG) | the geometry |
//! | `RTXE` (EXTR) | extra data |
//!
//! G1MG starts with a platform tag (`DX11`, or `DX9` in a few older models), a u32, a bounding box (six floats) and a section
//! count. Each section is `{u32 type, u32 size, u32 count}` and `count` entries:
//!
//! | Type | Entries |
//! |---|---|
//! | 0x10002 | materials: `{u32, u32 texture count, u32, u32}` then per texture `{u16 index, u16 layer, u16 type, u16 subtype, u16 tile u, u16 tile v}` |
//! | 0x10004 | vertex buffers: `{u32, u32 stride, u32 vertex count, u32}` then the vertices |
//! | 0x10005 | vertex layouts: `{u32 buffer count, u32 buffers[], u32 attribute count}` then per attribute `{u16 buffer, u16 offset, u8 type, u8, u8 semantic, u8 layer}` |
//! | 0x10007 | index buffers: `{u32 index count, u32 bits (16), u32}` then the indices, padded to 4 bytes |
//! | 0x10008 | submeshes: 14 u32 each (see [`Submesh`]) |
//!
//! A submesh names a vertex *layout*, not a buffer. A layout lists one or more vertex buffers
//! of the same length, and each attribute says which of them (by position in that list) it
//! is in: characters keep positions in a 12-byte buffer and the rest in a second one.
//!
//! Types 0x10001, 0x10003, 0x10006 and 0x10009 (shader parameters, bone maps, LOD groups)
//! are kept as raw sections for now.

use crate::{Error, u32_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrType {
    Float1,
    Float2,
    Float3,
    Float4,
    /// Four unsigned bytes (bone indices).
    UByte4,
    /// Four unsigned bytes read as 0..1 (colours).
    UNorm4,
}

impl AttrType {
    fn from_code(code: u8) -> Option<AttrType> {
        Some(match code {
            0 => AttrType::Float1,
            1 => AttrType::Float2,
            2 => AttrType::Float3,
            3 => AttrType::Float4,
            5 => AttrType::UByte4,
            13 => AttrType::UNorm4,
            _ => return None,
        })
    }

    pub fn size(self) -> usize {
        match self {
            AttrType::Float1 | AttrType::UByte4 | AttrType::UNorm4 => 4,
            AttrType::Float2 => 8,
            AttrType::Float3 => 12,
            AttrType::Float4 => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Semantic {
    Position,
    Weights,
    BoneIndices,
    Normal,
    Tangent,
    TexCoord,
    Color,
    Other(u8),
}

impl Semantic {
    fn from_code(code: u8) -> Semantic {
        match code {
            0 => Semantic::Position,
            1 => Semantic::Weights,
            2 => Semantic::BoneIndices,
            3 => Semantic::Normal,
            4 => Semantic::Tangent,
            5 => Semantic::TexCoord,
            10 => Semantic::Color,
            other => Semantic::Other(other),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Attribute {
    /// Index into the layout's `buffers`.
    pub buffer: u16,
    pub offset: u16,
    pub kind: AttrType,
    pub semantic: Semantic,
    pub layer: u8,
}

#[derive(Clone, Debug)]
pub struct Layout {
    /// Vertex buffer numbers.
    pub buffers: Vec<u32>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug)]
pub struct VertexBuffer<'a> {
    pub stride: usize,
    pub count: usize,
    pub data: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct TextureRef {
    pub index: u16,
    pub layer: u16,
    pub kind: u16,
    pub subtype: u16,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub textures: Vec<TextureRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Triangles,
    TriangleStrip,
}

/// A draw: a range of an index buffer over a range of a vertex buffer.
#[derive(Clone, Copy, Debug)]
pub struct Submesh {
    pub flags: u32,
    /// The vertex layout (and through it the vertex buffers) the submesh draws from.
    pub layout: usize,
    pub bone_map: usize,
    pub shader_params: usize,
    pub material: usize,
    pub index_buffer: usize,
    pub primitive: Primitive,
    pub vertex_start: usize,
    pub vertex_count: usize,
    pub index_start: usize,
    pub index_count: usize,
}

#[derive(Clone, Debug)]
pub struct Geometry<'a> {
    pub bounds: [f32; 6],
    pub materials: Vec<Material>,
    pub vertex_buffers: Vec<VertexBuffer<'a>>,
    pub layouts: Vec<Layout>,
    pub index_buffers: Vec<Vec<u16>>,
    pub submeshes: Vec<Submesh>,
    /// Sections not decoded yet: (type, count, body).
    pub other_sections: Vec<(u32, u32, &'a [u8])>,
}

#[derive(Clone, Debug)]
pub struct Chunk<'a> {
    pub magic: [u8; 4],
    pub version: [u8; 4],
    pub body: &'a [u8],
}

/// A joint of the skeleton, relative to its parent.
#[derive(Clone, Copy, Debug)]
pub struct Joint {
    pub scale: [f32; 3],
    pub parent: Option<usize>,
    /// Quaternion x, y, z, w.
    pub rotation: [f32; 4],
    pub position: [f32; 3],
}

/// G1MS. A model's own skeleton has its joints; a `_default` model that shares the base
/// model's skeleton has a single placeholder joint and only the index map.
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
    /// Maps a joint number used by the geometry to an index into `joints` (0xffff: none).
    pub index_map: Vec<u16>,
    pub layer: u16,
}

impl Skeleton {
    /// Each joint's position in model space, composed down the parent chain.
    pub fn world_positions(&self) -> Vec<[f32; 3]> {
        fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
            let u = [q[0], q[1], q[2]];
            let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let c1 = cross(u, v);
            let t = [c1[0] + q[3] * v[0], c1[1] + q[3] * v[1], c1[2] + q[3] * v[2]];
            let c2 = cross(u, t);
            [v[0] + 2.0 * c2[0], v[1] + 2.0 * c2[1], v[2] + 2.0 * c2[2]]
        }
        fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
            [
                a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
                a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
                a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
                a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
            ]
        }
        let mut pos: Vec<[f32; 3]> = Vec::with_capacity(self.joints.len());
        let mut rot: Vec<[f32; 4]> = Vec::with_capacity(self.joints.len());
        for j in &self.joints {
            match j.parent {
                None => {
                    pos.push(j.position);
                    rot.push(j.rotation);
                }
                Some(p) => {
                    let r = rotate(rot[p], j.position);
                    pos.push([pos[p][0] + r[0], pos[p][1] + r[1], pos[p][2] + r[2]]);
                    rot.push(mul(rot[p], j.rotation));
                }
            }
        }
        pos
    }
}

fn parse_skeleton(chunk: &[u8]) -> Result<Skeleton, Error> {
    // `chunk` includes its 12-byte header: offsets in G1MS count from the chunk's start.
    let joints_at = u32_at(chunk, 12)? as usize;
    let count = u16_at(chunk, 20)? as usize;
    let map_count = u16_at(chunk, 22)? as usize;
    let layer = u16_at(chunk, 24)?;
    let mut index_map = Vec::with_capacity(map_count);
    for i in 0..map_count {
        index_map.push(u16_at(chunk, 28 + 2 * i)?);
    }
    let mut joints = Vec::with_capacity(count);
    for i in 0..count {
        let at = joints_at + 48 * i;
        let f = |k: usize| f32_at(chunk, at + 4 * k);
        let parent = u32_at(chunk, at + 12)?;
        let parent = if parent & 0x8000_0000 != 0 {
            None
        } else if (parent as usize) < i {
            Some(parent as usize)
        } else {
            return Err(Error::Corrupt("g1ms joint's parent does not come before it"));
        };
        joints.push(Joint {
            scale: [f(0)?, f(1)?, f(2)?],
            parent,
            rotation: [f(4)?, f(5)?, f(6)?, f(7)?],
            position: [f(8)?, f(9)?, f(10)?],
        });
    }
    Ok(Skeleton { joints, index_map, layer })
}

#[derive(Clone, Debug)]
pub struct G1m<'a> {
    pub version: [u8; 4],
    pub chunks: Vec<Chunk<'a>>,
    pub geometry: Option<Geometry<'a>>,
    pub skeleton: Option<Skeleton>,
}

const MAX_COUNT: usize = 1 << 20;

fn f32_at(d: &[u8], at: usize) -> Result<f32, Error> {
    Ok(f32::from_bits(u32_at(d, at)?))
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Error::Corrupt("read past the end"))
}

fn bounded(n: u32) -> Result<usize, Error> {
    let n = n as usize;
    if n > MAX_COUNT { Err(Error::Corrupt("g1m count too large")) } else { Ok(n) }
}

fn parse_geometry(body: &[u8]) -> Result<Geometry<'_>, Error> {
    // "DX11", or "DX9\0" in a few older models (Escha's battle_Obj); the layout is the same.
    if !body.starts_with(b"DX11") && !body.starts_with(b"DX9\0") {
        return Err(Error::Corrupt("g1mg platform is not DX9 or DX11"));
    }
    let mut bounds = [0f32; 6];
    for (i, b) in bounds.iter_mut().enumerate() {
        *b = f32_at(body, 8 + 4 * i)?;
    }
    let sections = bounded(u32_at(body, 32)?)?;
    let mut g = Geometry {
        bounds,
        materials: Vec::new(),
        vertex_buffers: Vec::new(),
        layouts: Vec::new(),
        index_buffers: Vec::new(),
        submeshes: Vec::new(),
        other_sections: Vec::new(),
    };
    let mut at = 36;
    for _ in 0..sections {
        let kind = u32_at(body, at)?;
        let size = u32_at(body, at + 4)? as usize;
        let count = u32_at(body, at + 8)?;
        let sec = body.get(at..at + size).filter(|_| size >= 12).ok_or(Error::Corrupt("g1mg section past the end"))?;
        let n = bounded(count)?;
        let mut p = 12;
        match kind {
            0x10002 => {
                for _ in 0..n {
                    let textures = bounded(u32_at(sec, p + 4)?)?;
                    p += 16;
                    let mut m = Material { textures: Vec::with_capacity(textures) };
                    for _ in 0..textures {
                        m.textures.push(TextureRef {
                            index: u16_at(sec, p)?,
                            layer: u16_at(sec, p + 2)?,
                            kind: u16_at(sec, p + 4)?,
                            subtype: u16_at(sec, p + 6)?,
                        });
                        p += 12;
                    }
                    g.materials.push(m);
                }
            }
            0x10004 => {
                for _ in 0..n {
                    let stride = u32_at(sec, p + 4)? as usize;
                    let count = bounded(u32_at(sec, p + 8)?)?;
                    p += 16;
                    let data = sec.get(p..p + stride * count).ok_or(Error::Corrupt("g1mg vertices past the end"))?;
                    g.vertex_buffers.push(VertexBuffer { stride, count, data });
                    p += stride * count;
                }
            }
            0x10005 => {
                for _ in 0..n {
                    let nb = bounded(u32_at(sec, p)?)?;
                    p += 4;
                    let mut buffers = Vec::with_capacity(nb);
                    for _ in 0..nb {
                        buffers.push(u32_at(sec, p)?);
                        p += 4;
                    }
                    let na = bounded(u32_at(sec, p)?)?;
                    p += 4;
                    let mut attributes = Vec::with_capacity(na);
                    for _ in 0..na {
                        let a = sec.get(p..p + 8).ok_or(Error::Corrupt("g1mg attribute past the end"))?;
                        attributes.push(Attribute {
                            buffer: u16::from_le_bytes([a[0], a[1]]),
                            offset: u16::from_le_bytes([a[2], a[3]]),
                            kind: AttrType::from_code(a[4]).ok_or(Error::Corrupt("g1mg attribute type not known"))?,
                            semantic: Semantic::from_code(a[6]),
                            layer: a[7],
                        });
                        p += 8;
                    }
                    g.layouts.push(Layout { buffers, attributes });
                }
            }
            0x10007 => {
                for _ in 0..n {
                    let count = bounded(u32_at(sec, p)?)?;
                    if u32_at(sec, p + 4)? != 16 {
                        return Err(Error::Corrupt("g1mg index size is not 16 bits"));
                    }
                    p += 12;
                    let raw = sec.get(p..p + 2 * count).ok_or(Error::Corrupt("g1mg indices past the end"))?;
                    g.index_buffers.push(raw.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect());
                    p = (p + 2 * count + 3) & !3;
                }
            }
            0x10008 => {
                for _ in 0..n {
                    let mut f = [0u32; 14];
                    for (i, v) in f.iter_mut().enumerate() {
                        *v = u32_at(sec, p + 4 * i)?;
                    }
                    p += 56;
                    let primitive = match f[9] {
                        3 => Primitive::Triangles,
                        4 => Primitive::TriangleStrip,
                        _ => return Err(Error::Corrupt("g1mg primitive type not known")),
                    };
                    g.submeshes.push(Submesh {
                        flags: f[0],
                        layout: f[1] as usize,
                        bone_map: f[2] as usize,
                        shader_params: f[5] as usize,
                        material: f[6] as usize,
                        index_buffer: f[7] as usize,
                        primitive,
                        vertex_start: f[10] as usize,
                        vertex_count: f[11] as usize,
                        index_start: f[12] as usize,
                        index_count: f[13] as usize,
                    });
                }
            }
            _ => g.other_sections.push((kind, count, &sec[12..])),
        }
        at += size;
    }
    for l in &g.layouts {
        let mut count = None;
        for &b in &l.buffers {
            let vb = g.vertex_buffers.get(b as usize).ok_or(Error::Corrupt("layout names a missing vertex buffer"))?;
            if count.is_some_and(|c| c != vb.count) {
                return Err(Error::Corrupt("layout's vertex buffers differ in length"));
            }
            count = Some(vb.count);
        }
        for a in &l.attributes {
            let vb = l.buffers.get(a.buffer as usize).map(|&b| &g.vertex_buffers[b as usize]);
            if vb.is_none_or(|vb| a.offset as usize + a.kind.size() > vb.stride) {
                return Err(Error::Corrupt("attribute outside its vertex"));
            }
        }
    }
    for s in &g.submeshes {
        let count = g.vertex_count(s.layout).ok_or(Error::Corrupt("submesh layout"))?;
        let ib = g.index_buffers.get(s.index_buffer).ok_or(Error::Corrupt("submesh index buffer"))?;
        if s.index_start + s.index_count > ib.len() || s.vertex_start + s.vertex_count > count {
            return Err(Error::Corrupt("submesh range"));
        }
        if ib[s.index_start..s.index_start + s.index_count].iter().any(|&i| i as usize >= count) {
            return Err(Error::Corrupt("index past its vertex buffer"));
        }
    }
    Ok(g)
}

impl<'a> G1m<'a> {
    pub fn parse(data: &'a [u8]) -> Result<G1m<'a>, Error> {
        if !data.starts_with(b"_M1G") {
            return Err(Error::NotThisFormat);
        }
        let version: [u8; 4] = data[4..8].try_into().unwrap();
        let size = (u32_at(data, 8)? as usize).min(data.len());
        let mut at = u32_at(data, 12)? as usize;
        let count = bounded(u32_at(data, 20)?)?;
        let mut chunks = Vec::with_capacity(count);
        let mut geometry = None;
        let mut skeleton = None;
        for _ in 0..count {
            let head = data.get(at..at + 12).ok_or(Error::Corrupt("g1m chunk header past the end"))?;
            let len = u32_at(head, 8)? as usize;
            if len < 12 || at + len > size {
                return Err(Error::Corrupt("g1m chunk size"));
            }
            let chunk = Chunk {
                magic: head[..4].try_into().unwrap(),
                version: head[4..8].try_into().unwrap(),
                body: &data[at + 12..at + len],
            };
            if &chunk.magic == b"GM1G" {
                geometry = Some(parse_geometry(chunk.body)?);
            }
            if &chunk.magic == b"SM1G" {
                skeleton = Some(parse_skeleton(&data[at..at + len])?);
            }
            chunks.push(chunk);
            at += len;
        }
        Ok(G1m { version, chunks, geometry, skeleton })
    }
}

/// A submesh flattened for tools: positions, normals, UVs and triangles.
#[derive(Clone, Debug, Default)]
pub struct SimpleMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub material: usize,
}

impl Geometry<'_> {
    /// How many vertices a layout's buffers hold.
    pub fn vertex_count(&self, layout: usize) -> Option<usize> {
        let first = *self.layouts.get(layout)?.buffers.first()?;
        Some(self.vertex_buffers.get(first as usize)?.count)
    }

    /// Read one submesh's vertices and triangles (indices made relative to its vertices).
    pub fn simple_mesh(&self, s: &Submesh) -> Result<SimpleMesh, Error> {
        let layout = self.layouts.get(s.layout).ok_or(Error::Corrupt("submesh layout"))?;
        let mut m = SimpleMesh { material: s.material, ..Default::default() };
        let read = |v: usize, a: &Attribute, n: usize| -> Result<[f32; 3], Error> {
            let vb = &self.vertex_buffers[layout.buffers[a.buffer as usize] as usize];
            let base = v * vb.stride + a.offset as usize;
            let mut out = [0f32; 3];
            for (i, o) in out.iter_mut().enumerate().take(n) {
                *o = f32_at(vb.data, base + 4 * i)?;
            }
            Ok(out)
        };
        let find = |sem: Semantic| layout.attributes.iter().find(|a| a.semantic == sem && a.layer == 0);
        let pos = find(Semantic::Position).ok_or(Error::Corrupt("submesh has no positions"))?;
        let nrm = find(Semantic::Normal).filter(|a| a.kind == AttrType::Float3);
        let uv = find(Semantic::TexCoord).filter(|a| a.kind == AttrType::Float2);
        let ib = &self.index_buffers[s.index_buffer][s.index_start..s.index_start + s.index_count];
        let lo = ib.iter().copied().min().unwrap_or(0) as usize;
        let hi = ib.iter().copied().max().map_or(0, |h| h as usize + 1);
        for v in lo..hi {
            m.positions.push(read(v, pos, 3)?);
            if let Some(a) = nrm {
                m.normals.push(read(v, a, 3)?);
            }
            if let Some(a) = uv {
                let t = read(v, a, 2)?;
                m.uvs.push([t[0], t[1]]);
            }
        }
        let rel = |i: u16| (i as usize - lo) as u32;
        match s.primitive {
            Primitive::Triangles => {
                for t in ib.chunks_exact(3) {
                    m.triangles.push([rel(t[0]), rel(t[1]), rel(t[2])]);
                }
            }
            Primitive::TriangleStrip => {
                for k in 2..ib.len() {
                    let (a, b, c) = (ib[k - 2], ib[k - 1], ib[k]);
                    if a == b || b == c || a == c {
                        continue;
                    }
                    m.triangles.push(if k % 2 == 0 { [rel(a), rel(b), rel(c)] } else { [rel(b), rel(a), rel(c)] });
                }
            }
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_positions_follow_parents() {
        // Root turned 90° about y; its child 10 along x ends up at -10 along z.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let s = Skeleton {
            joints: vec![
                Joint { scale: [1.0; 3], parent: None, rotation: [0.0, h, 0.0, h], position: [0.0, 5.0, 0.0] },
                Joint { scale: [1.0; 3], parent: Some(0), rotation: [0.0, 0.0, 0.0, 1.0], position: [10.0, 0.0, 0.0] },
            ],
            index_map: vec![0, 1],
            layer: 0,
        };
        let w = s.world_positions();
        assert!((w[1][0]).abs() < 1e-4 && (w[1][1] - 5.0).abs() < 1e-4 && (w[1][2] + 10.0).abs() < 1e-4, "{w:?}");
    }
}
