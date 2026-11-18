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
//! | 0x10007 | index buffers: `{u32 index count, u32 bits (16; 32 in later games' large field models), u32}` then the indices, padded to 4 bytes |
//! | 0x10008 | submeshes: 14 u32 each (see [`Submesh`]) |
//!
//! A submesh names a vertex *layout*, not a buffer. A layout lists one or more vertex buffers
//! of the same length, and each attribute says which of them (by position in that list) it
//! is in: characters keep positions in a 12-byte buffer and the rest in a second one.
//!
//! | 0x10006 | bone palettes: `{u32 count}`, then per entry `{u32, u32 flags, u32 joint}`; the joint's top bit marks a joint of the base model's skeleton |
//!
//! | 0x10003 | shader parameter sets (a submesh names one): per set `{u32 count}`, then per parameter `{u32 record size, u32 name length, u32 0, u16 value count, u16 type, name (padded), values}`; type 1 floats (`atEdge` 0.2, `atOutCol` (0.8, 0.8, 0.8), `g_specularPower`, `g_UVOffsetScaleL`, ...) |
//!
//! | 0x10009 | mesh groups (`{u32 block count}` in the header's count): per block nine words `{lod, group, first entry, count1, count2, lod range start, lod range length, -1, 0}`, then `count1 + count2` entries `{16-byte name, u32 0, u32 -1, u32 n, n submesh numbers}`. The name is the shader the submeshes draw with: group 0 the colour pass (`MapSTD`, `MapSTD-T0`, `CharaMTDS`, `CharaEYEL` ...: the `String`s of `Saves/shader/shader.xml`), group 1 the shadow pass (`SM`) |
//!
//! | 0x10001 | lighting materials (`{u32 0x100}` then `count` 64-byte entries): diffuse (4 floats), ambient (3), a word of nibble flags, then three floats, a float and three floats (in field models 1, 1, 1, 20 and 0, 0, 0: specular, power and emissive by the order of the game's `cSpec`; not read) and a mask. The diffuse and ambient are `MapSTD`'s `cDiff` and `cAmb` ([`Geometry::lights`]) |

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
    /// Four unsigned 16-bit values (bone indices and others in Ryza 3's larger models).
    UShort4,
}

impl AttrType {
    fn from_code(code: u8) -> Option<AttrType> {
        Some(match code {
            0 => AttrType::Float1,
            1 => AttrType::Float2,
            2 => AttrType::Float3,
            3 => AttrType::Float4,
            5 => AttrType::UByte4,
            7 => AttrType::UShort4,
            13 => AttrType::UNorm4,
            _ => return None,
        })
    }

    pub fn size(self) -> usize {
        match self {
            AttrType::Float1 | AttrType::UByte4 | AttrType::UNorm4 => 4,
            AttrType::Float2 | AttrType::UShort4 => 8,
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
    /// Direct3D's PSIZE (the codes are its declaration usages): on billboard models
    /// (`MapSTD-B*`) each vertex's corner offset from its quad's centre, in billboard space.
    PointSize,
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
            4 => Semantic::PointSize,
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

/// A named shader parameter (G1MG section 0x10003): `values` as floats (type 1) or the raw
/// words read as floats otherwise (Ryza's `rampIndex`, `rmIndex`: one integer, `integer`).
/// The values never run past the parameter's record (an integer parameter's count is not its
/// number of words).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderParam {
    pub name: String,
    pub kind: u16,
    pub values: Vec<f32>,
}

impl ShaderParam {
    /// The first value's raw word, for a parameter that holds an integer (Ryza's `rampIndex`
    /// and `rmIndex` are type 1 like the floats, their words small integers).
    pub fn integer(&self) -> Option<u32> {
        self.values.first().map(|v| v.to_bits())
    }
}

impl Geometry<'_> {
    /// A submesh's parameter record by name.
    pub fn param_record(&self, s: &Submesh, name: &str) -> Option<&ShaderParam> {
        self.shader_params.get(s.shader_params)?.iter().find(|p| p.name == name)
    }

    /// A submesh's parameter by name.
    pub fn param(&self, s: &Submesh, name: &str) -> Option<&[f32]> {
        self.shader_params.get(s.shader_params)?.iter().find(|p| p.name == name).map(|p| p.values.as_slice())
    }
}

/// One entry of a bone palette (G1MG section 0x10006).
#[derive(Clone, Copy, Debug)]
pub struct BoneRef {
    /// First word; its use is not known yet.
    pub matrix: u32,
    /// Second word (0x80000000 in every Escha entry seen so far).
    pub flags: u32,
    /// The joint, as an index into the skeleton's joints.
    pub joint: u32,
    /// The joint lives in the base model's skeleton rather than this file's.
    pub external: bool,
}

#[derive(Clone, Debug)]
pub struct Geometry<'a> {
    pub bounds: [f32; 6],
    pub materials: Vec<Material>,
    pub vertex_buffers: Vec<VertexBuffer<'a>>,
    pub layouts: Vec<Layout>,
    pub index_buffers: Vec<Vec<u32>>,
    pub submeshes: Vec<Submesh>,
    /// Bone palettes; a submesh's `bone_map` picks one. A vertex's bone index byte divided
    /// by 3 is a slot in that palette.
    pub bone_maps: Vec<Vec<BoneRef>>,
    /// Shader parameter sets (a submesh's `shader_params` picks one).
    pub shader_params: Vec<Vec<ShaderParam>>,
    /// Mesh groups (section 0x10009): which shader draws which submeshes, per pass.
    pub mesh_groups: Vec<MeshGroup>,
    /// Lighting materials (section 0x10001): the light a field model takes, scaled
    /// (`MapSTD`'s `cDiff` and `cAmb`). One entry in field models; which entry a submesh takes
    /// when there are several is not known.
    pub lights: Vec<Light>,
    /// Sections not decoded yet: (type, count, body).
    pub other_sections: Vec<(u32, u32, &'a [u8])>,
    /// The cloth pieces' control points (the model's `NUNO` chunk), which the cloth
    /// submeshes' vertices are built from ([`Geometry::cloth_of`]).
    pub cloths: Vec<Cloth>,
}

/// One cloth piece (an entry of the `NUNO` chunk's section 0x30003): a grid of control
/// points the game's simulation moves; a cloth submesh's vertices hang between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Cloth {
    /// The bone (as [`Skeleton::joint`] takes it) whose space the points are stored in.
    pub parent_bone: u32,
    /// The points at rest, in the parent's space until [`Cloth::place`] moves them.
    pub points: Vec<[f32; 3]>,
    /// Once placed: the parent's joint and its bind matrix (three rows of four), which took
    /// the points into the model's space and takes the piece's ordinary vertices there too.
    pub placed: Option<(u32, [[f32; 4]; 3])>,
}

impl Cloth {
    /// Move the points from the parent joint's space into the model's by the parent's bind
    /// matrix (`rows`: its first three rows; `joint`: the parent in the skeleton's joints).
    pub fn place(&mut self, joint: u32, rows: [[f32; 4]; 3]) {
        if self.placed.is_none() {
            for p in &mut self.points {
                *p = apply(&rows, *p, 1.0);
            }
            self.placed = Some((joint, rows));
        }
    }
}

/// A point (`w` 1) or direction (`w` 0) through a matrix's first three rows.
fn apply(rows: &[[f32; 4]; 3], p: [f32; 3], w: f32) -> [f32; 3] {
    std::array::from_fn(|r| rows[r][0] * p[0] + rows[r][1] * p[1] + rows[r][2] * p[2] + rows[r][3] * w)
}

/// The cloth pieces of a `NUNO` chunk's body: section 0x30003's entries in order. An entry
/// is a header (the parent bone with the top bit set, the point count `n`, then counts
/// `a`, `b`, a number, `d`, `e` and the simulation's settings), `n` points of four floats,
/// `n` links of 24 bytes (the neighbours left, right, up and down, two floats) and a tail of
/// `48 a + 4 b + 8 d + 12 e` bytes. The header is 0x14c bytes in 34 of Ryza DX's character
/// models and 0x180 in 3 (Ryza's among them), under one chunk version: what tells them
/// apart is not found, so the size that walks the section's entries to its exact end is
/// taken. Anything that fits neither reads as no cloth.
fn parse_cloths(body: &[u8]) -> Vec<Cloth> {
    let word = |at: usize| body.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let read = |header: usize| -> Option<Vec<Cloth>> {
        let mut out = Vec::new();
        let mut at = 4;
        for _ in 0..word(0)? {
            let (kind, size, entries) = (word(at)?, word(at + 4)? as usize, word(at + 8)? as usize);
            let end = at.checked_add(size).filter(|&e| e <= body.len() && size >= 12)?;
            if kind == 0x30003 {
                let mut p = at + 12;
                for _ in 0..entries {
                    let n = word(p + 4)? as usize;
                    let tail = 48 * word(p + 8)? as usize + 4 * word(p + 12)? as usize + 8 * word(p + 20)? as usize + 12 * word(p + 24)? as usize;
                    let next = p.checked_add(header + n.checked_mul(40)? + tail).filter(|&e| e <= end)?;
                    let points = (0..n).map(|k| std::array::from_fn(|i| f32::from_bits(word(p + header + 16 * k + 4 * i).unwrap_or(0)))).collect();
                    out.push(Cloth { parent_bone: word(p)? & 0x7fff_ffff, points, placed: None });
                    p = next;
                }
                if p != end {
                    return None;
                }
            }
            at = end;
        }
        Some(out)
    };
    read(0x14c).or_else(|| read(0x180)).unwrap_or_default()
}

/// A lighting material (G1MG section 0x10001): what the field light is scaled by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// `cDiff`: the directional and point lights' diffuse (and alpha).
    pub diffuse: [f32; 4],
    /// `cAmb`: the ambient.
    pub ambient: [f32; 3],
    /// The word of nibble flags after the ambient.
    pub flags: u32,
}

/// Submeshes drawn with one shader in one pass (G1MG section 0x10009).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshGroup {
    pub lod: u32,
    /// 0 the colour pass, 1 the shadow pass.
    pub group: u32,
    /// The shader's name (`Saves/shader/shader.xml`'s `String`: `MapSTD`, `CharaMTDS`, `SM` ...).
    pub shader: String,
    /// The two words after the shader's name: 0 in the colour pass, in the shadow pass a
    /// count in the top byte (0 to 10 seen); the second always -1 (over 238 entries).
    pub words: [u32; 2],
    pub submeshes: Vec<u32>,
}

impl Geometry<'_> {
    /// The shader a submesh is drawn with in the colour pass (group 0), if the model says.
    pub fn shader_of(&self, submesh: usize) -> Option<&str> {
        self.mesh_groups.iter().find(|m| m.group == 0 && m.submeshes.contains(&(submesh as u32))).map(|m| m.shader.as_str())
    }
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
    /// The joint a bone id (as motions and the geometry name bones) stands for, through the
    /// index map: ids the map leaves out (0xffff) or runs short of have no joint. Escha's
    /// skeletons map their animated ids to themselves; Ryza's leave gaps (ids 25-49 unused,
    /// 50 on for joints 25 on).
    pub fn joint(&self, bone: u32) -> Option<usize> {
        match self.index_map.get(bone as usize) {
            Some(&j) if j != 0xffff && (j as usize) < self.joints.len() => Some(j as usize),
            Some(_) => None,
            None if self.index_map.is_empty() && (bone as usize) < self.joints.len() => Some(bone as usize),
            None => None,
        }
    }

    /// The joints in an order where every parent comes before its children (file order in
    /// most models; a few list a child before its parent). The parser has ruled out cycles.
    pub fn order(&self) -> Vec<usize> {
        let n = self.joints.len();
        let mut done = vec![false; n];
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let before = out.len();
            for (i, j) in self.joints.iter().enumerate() {
                if !done[i] && j.parent.is_none_or(|p| done[p]) {
                    done[i] = true;
                    out.push(i);
                }
            }
            if out.len() == before {
                break; // a cycle; the parser rejects these
            }
        }
        out
    }

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
        let mut pos = vec![[0f32; 3]; self.joints.len()];
        let mut rot = vec![[0f32, 0.0, 0.0, 1.0]; self.joints.len()];
        for i in self.order() {
            let j = &self.joints[i];
            match j.parent {
                None => {
                    pos[i] = j.position;
                    rot[i] = j.rotation;
                }
                Some(p) => {
                    let r = rotate(rot[p], j.position);
                    pos[i] = [pos[p][0] + r[0], pos[p][1] + r[1], pos[p][2] + r[2]];
                    rot[i] = mul(rot[p], j.rotation);
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
        } else if (parent as usize) < count && parent as usize != i {
            Some(parent as usize)
        } else {
            return Err(Error::Corrupt("g1ms joint's parent out of range"));
        };
        joints.push(Joint {
            scale: [f(0)?, f(1)?, f(2)?],
            parent,
            rotation: [f(4)?, f(5)?, f(6)?, f(7)?],
            position: [f(8)?, f(9)?, f(10)?],
        });
    }
    let skeleton = Skeleton { joints, index_map, layer };
    if skeleton.order().len() != count {
        return Err(Error::Corrupt("g1ms joints' parents form a cycle"));
    }
    Ok(skeleton)
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
        bone_maps: Vec::new(),
        shader_params: Vec::new(),
        mesh_groups: Vec::new(),
        lights: Vec::new(),
        other_sections: Vec::new(),
        cloths: Vec::new(),
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
            0x10003 => {
                for _ in 0..n {
                    let params = bounded(u32_at(sec, p)?)?;
                    p += 4;
                    let mut set = Vec::with_capacity(params);
                    for _ in 0..params {
                        let size = u32_at(sec, p)? as usize;
                        let name_len = u32_at(sec, p + 4)? as usize;
                        let count = u16_at(sec, p + 12)? as usize;
                        let kind = u16_at(sec, p + 14)?;
                        if size < 16 + name_len || name_len > 256 {
                            return Err(Error::Corrupt("g1mg shader parameter size"));
                        }
                        let name_bytes = sec.get(p + 16..p + 16 + name_len).ok_or(Error::Corrupt("g1mg shader parameter past the end"))?;
                        let name = String::from_utf8_lossy(name_bytes.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
                        let room = (size - 16 - name_len) / 4;
                        let values = (0..count.min(64).min(room)).map(|k| f32_at(sec, p + 16 + name_len + 4 * k)).collect::<Result<_, _>>()?;
                        set.push(ShaderParam { name, kind, values });
                        p += size;
                    }
                    g.shader_params.push(set);
                }
            }
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
            0x10006 => {
                for _ in 0..n {
                    let count = bounded(u32_at(sec, p)?)?;
                    p += 4;
                    let mut map = Vec::with_capacity(count);
                    for _ in 0..count {
                        let joint = u32_at(sec, p + 8)?;
                        map.push(BoneRef {
                            matrix: u32_at(sec, p)?,
                            flags: u32_at(sec, p + 4)?,
                            joint: joint & 0x7fff_ffff,
                            external: joint & 0x8000_0000 != 0,
                        });
                        p += 12;
                    }
                    g.bone_maps.push(map);
                }
            }
            0x10007 => {
                for _ in 0..n {
                    let count = bounded(u32_at(sec, p)?)?;
                    // 16-bit indices in Escha; Sophie 2 and the Ryza games also use 32-bit
                    // ones for large field models.
                    let width = match u32_at(sec, p + 4)? {
                        8 => 1,
                        16 => 2,
                        32 => 4,
                        _ => return Err(Error::Corrupt("g1mg index size not known")),
                    };
                    p += 12;
                    let raw = sec.get(p..p + width * count).ok_or(Error::Corrupt("g1mg indices past the end"))?;
                    g.index_buffers.push(
                        raw.chunks_exact(width)
                            .map(|b| match width {
                                1 => b[0] as u32,
                                2 => u16::from_le_bytes([b[0], b[1]]) as u32,
                                _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                            })
                            .collect(),
                    );
                    p = (p + width * count + 3) & !3;
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
            0x10001 => {
                for e in 0..bounded(count)? {
                    let at = 16 + 64 * e;
                    let f = |k: usize| f32_at(sec, at + 4 * k);
                    g.lights.push(Light { diffuse: [f(0)?, f(1)?, f(2)?, f(3)?], ambient: [f(4)?, f(5)?, f(6)?], flags: u32_at(sec, at + 28)? });
                }
            }
            0x10009 => {
                for _ in 0..n {
                    let (lod, group) = (u32_at(sec, p)?, u32_at(sec, p + 4)?);
                    let entries = bounded(u32_at(sec, p + 12)?)? + bounded(u32_at(sec, p + 16)?)?;
                    p += 36;
                    for _ in 0..entries {
                        let name = sec.get(p..p + 16).ok_or(Error::Corrupt("g1mg mesh group past the end"))?;
                        let shader = String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
                        let words = [u32_at(sec, p + 16)?, u32_at(sec, p + 20)?];
                        let k = bounded(u32_at(sec, p + 24)?)?;
                        let submeshes = (0..k).map(|i| u32_at(sec, p + 28 + 4 * i)).collect::<Result<_, _>>()?;
                        g.mesh_groups.push(MeshGroup { lod, group, shader, words, submeshes });
                        p += 28 + 4 * k;
                    }
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
        if !g.bone_maps.is_empty() && s.bone_map >= g.bone_maps.len() {
            return Err(Error::Corrupt("submesh bone map"));
        }
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
        if let (Some(g), Some(c)) = (geometry.as_mut(), chunks.iter().find(|c| &c.magic == b"ONUN" && &c.version == b"6300")) {
            g.cloths = parse_cloths(c.body);
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
    /// Vertex colours (0 to 1), when the layout has them (field models do: the light baked
    /// into them, `docs/shading.md`).
    pub colors: Vec<[f32; 4]>,
    /// Billboard corners (PSIZE), when the layout has them: the vertex sits at its quad's
    /// centre (`positions`) and this offset turns with the camera (`docs/shading.md`,
    /// "Billboards").
    pub corners: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub material: usize,
    /// The layout vertex the first of `positions` is (morph targets count from the
    /// layout's start).
    pub first_vertex: usize,
}

/// The four bone indices of vertex `v`, bytes or 16-bit values by the attribute's type.
fn bone_indices(vb: &VertexBuffer, a: &Attribute, v: usize) -> [u32; 4] {
    let at = v * vb.stride + a.offset as usize;
    match a.kind {
        AttrType::UShort4 => std::array::from_fn(|i| u16::from_le_bytes([vb.data[at + 2 * i], vb.data[at + 2 * i + 1]]) as u32),
        _ => std::array::from_fn(|i| vb.data[at + i] as u32),
    }
}

impl Geometry<'_> {
    /// The largest bone palette slot any vertex of a submesh uses, if it has bone indices.
    pub fn max_bone_slot(&self, s: &Submesh) -> Option<usize> {
        if !self.palette_indexed(s) {
            return None;
        }
        let layout = self.layouts.get(s.layout)?;
        let a = layout.attributes.iter().find(|a| a.semantic == Semantic::BoneIndices && a.layer == 0)?;
        let vb = &self.vertex_buffers[layout.buffers[a.buffer as usize] as usize];
        let mut max = 0usize;
        for v in s.vertex_start..s.vertex_start + s.vertex_count {
            for b in bone_indices(vb, a, v) {
                max = max.max(b as usize / 3);
            }
        }
        Some(max)
    }

    /// Whether a submesh's bone indices pick palette slots (every one a multiple of 3, as in
    /// all of Escha's models). In the later games some submeshes (cloth and other physics
    /// parts, by their shape) carry other numbers there: dense values from 0 past their
    /// palette's size.
    pub fn palette_indexed(&self, s: &Submesh) -> bool {
        let Some(layout) = self.layouts.get(s.layout) else { return false };
        let Some(a) = layout.attributes.iter().find(|a| a.semantic == Semantic::BoneIndices && a.layer == 0) else {
            return false;
        };
        let vb = &self.vertex_buffers[layout.buffers[a.buffer as usize] as usize];
        (s.vertex_start..s.vertex_start + s.vertex_count).all(|v| bone_indices(vb, a, v).iter().all(|b| b % 3 == 0))
    }

    /// Each vertex's joints and weights for a submesh, over the same vertex range as
    /// [`Geometry::simple_mesh`]: the bone-index byte / 3 picks a slot of the submesh's
    /// palette, whose joint is returned; a missing last weight is 1 minus the others.
    /// A submesh with a palette but no bone indices or weights (Escha's hat, `_P_M_Acc_Hat1`)
    /// is rigid on its palette's first joint. None when the submesh has no palette joint.
    pub fn skin(&self, s: &Submesh) -> Result<Option<Vec<([u32; 4], [f32; 4])>>, Error> {
        let layout = self.layouts.get(s.layout).ok_or(Error::Corrupt("submesh layout"))?;
        let find = |sem: Semantic| layout.attributes.iter().find(|a| a.semantic == sem && a.layer == 0);
        let ib = &self.index_buffers[s.index_buffer][s.index_start..s.index_start + s.index_count];
        let lo = ib.iter().copied().min().unwrap_or(0) as usize;
        let hi = ib.iter().copied().max().map_or(0, |h| h as usize + 1);
        let Some(palette) = self.bone_maps.get(s.bone_map) else { return Ok(None) };
        let (Some(bones), Some(weights)) = (find(Semantic::BoneIndices), find(Semantic::Weights)) else {
            let Some(first) = palette.first().map(|r| r.joint) else { return Ok(None) };
            // Indices without weights: each vertex wholly on its first index's joint (a
            // windmill's sails turn on one joint, its tower in the same submesh stands on
            // another).
            return Ok(Some(match find(Semantic::BoneIndices).filter(|_| self.palette_indexed(s)) {
                Some(bones) => {
                    let vb = &self.vertex_buffers[layout.buffers[bones.buffer as usize] as usize];
                    (lo..hi).map(|v| ([palette.get(bone_indices(vb, bones, v)[0] as usize / 3).map_or(first, |r| r.joint); 4], [1.0, 0.0, 0.0, 0.0])).collect()
                }
                None => vec![([first; 4], [1.0, 0.0, 0.0, 0.0]); hi - lo],
            }));
        };
        let buffer = |a: &Attribute| &self.vertex_buffers[layout.buffers[a.buffer as usize] as usize];
        let (bvb, wvb) = (buffer(bones), buffer(weights));
        let count = match weights.kind {
            AttrType::Float1 => 1,
            AttrType::Float2 => 2,
            AttrType::Float3 => 3,
            AttrType::Float4 => 4,
            _ => return Err(Error::Corrupt("weights are not floats")),
        };
        let mut out = Vec::with_capacity(hi - lo);
        // A cloth piece: the vertices built from its points ride its parent joint; its
        // ordinary ones (no second group of points) are skinned through the palette.
        let cloth = self.cloth_of(s).and_then(|c| Some((c.placed?.0, find(Semantic::PointSize)?)));
        // Submeshes whose indices are not palette slots stay rigid on the palette's first
        // joint until what their numbers mean is worked out.
        if cloth.is_none() && !self.palette_indexed(s) {
            let joint = palette.first().map_or(0, |r| r.joint);
            return Ok(Some(vec![([joint; 4], [1.0, 0.0, 0.0, 0.0]); hi - lo]));
        }
        for v in lo..hi {
            if let Some((joint, group)) = cloth
                && bone_indices(buffer(group), group, v) != [0; 4] {
                    out.push(([joint; 4], [1.0, 0.0, 0.0, 0.0]));
                    continue;
                }
            let b = bone_indices(bvb, bones, v);
            let mut w = [0f32; 4];
            for (i, x) in w.iter_mut().enumerate().take(count) {
                *x = f32_at(wvb.data, v * wvb.stride + weights.offset as usize + 4 * i)?;
            }
            if count < 4 {
                w[count] = (1.0 - w[..count].iter().sum::<f32>()).max(0.0);
            }
            let mut j = [0u32; 4];
            for i in 0..4 {
                let slot = b[i] as usize / 3;
                j[i] = palette.get(slot).map_or(0, |r| r.joint);
            }
            out.push((j, w));
        }
        Ok(Some(out))
    }

    /// The cloth piece a submesh hangs on, if it is a cloth submesh whose piece is read and
    /// placed: its mesh group entry's first word is 1 (cloth) and its second 20000 plus the
    /// piece's number.
    pub fn cloth_of(&self, s: &Submesh) -> Option<&Cloth> {
        let k = self.submeshes.iter().position(|x| (x.index_buffer, x.index_start, x.index_count) == (s.index_buffer, s.index_start, s.index_count))? as u32;
        let group = self.mesh_groups.iter().find(|m| m.submeshes.contains(&k) && m.words[0] & 0xffff == 1 && m.words[1] >= 20000)?;
        self.cloths.get((group.words[1] - 20000) as usize).filter(|c| c.placed.is_some())
    }

    /// A cloth vertex's position and normal, as the game's vertex shader builds them from
    /// the control points `c`: the four index attributes (bone indices, PSIZE, FOG,
    /// TEXCOORD5) each name four points; the "position" weighs a group's points into one
    /// corner and the binormal into that corner's lean; the blend weights mix the corners
    /// into the vertex's place on the sheet and the leans into one direction along it, the
    /// second colour mixes the corners into the other direction; the vertex stands off the
    /// sheet along their cross product by the normal's fourth word, and its normal is the
    /// stored one in that frame. None for a vertex with no second group: an ordinary skinned
    /// vertex (the piece's row sewn to the body), kept in the parent joint's space.
    fn cloth_vertex(&self, layout: &Layout, cloth: &Cloth, v: usize) -> Option<([f32; 3], [f32; 3])> {
        let find = |sem: Semantic, layer: u8| layout.attributes.iter().find(|a| a.semantic == sem && a.layer == layer);
        let buffer = |a: &Attribute| &self.vertex_buffers[layout.buffers[a.buffer as usize] as usize];
        let floats = |a: &Attribute| -> Option<[f32; 4]> {
            let vb = buffer(a);
            let at = v * vb.stride + a.offset as usize;
            (a.kind == AttrType::Float4).then_some(())?;
            let b = vb.data.get(at..at + 16)?;
            Some(std::array::from_fn(|i| f32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap())))
        };
        let groups = [find(Semantic::BoneIndices, 0)?, find(Semantic::PointSize, 0)?, find(Semantic::Other(11), 0)?, find(Semantic::TexCoord, 5)?];
        let groups = groups.map(|a| bone_indices(buffer(a), a, v));
        if groups[1] == [0; 4] {
            return None;
        }
        let (along, lean) = (floats(find(Semantic::Position, 0)?)?, floats(find(Semantic::Other(7), 0)?)?);
        let (mix, across, normal) = (floats(find(Semantic::Weights, 0)?)?, floats(find(Semantic::Color, 1)?)?, floats(find(Semantic::Normal, 0)?)?);
        let add = |a: [f32; 3], b: [f32; 3], k: f32| [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k];
        let (mut centre, mut d0, mut d1) = ([0.0; 3], [0.0; 3], [0.0; 3]);
        for (g, group) in groups.iter().enumerate() {
            let (mut corner, mut leaning) = ([0.0; 3], [0.0; 3]);
            for (i, &point) in group.iter().enumerate() {
                let p = *cloth.points.get(point as usize)?;
                corner = add(corner, p, along[i]);
                leaning = add(leaning, p, lean[i]);
            }
            centre = add(centre, corner, mix[g]);
            d1 = add(d1, corner, across[g]);
            d0 = add(d0, leaning, mix[g]);
        }
        let off = [d1[1] * d0[2] - d1[2] * d0[1], d1[2] * d0[0] - d1[0] * d0[2], d1[0] * d0[1] - d1[1] * d0[0]];
        let n = add(add(add([0.0; 3], d0, normal[0]), d1, normal[1]), off, normal[2]);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        Some((add(centre, off, normal[3]), if len > 0.0 { n.map(|x| x / len) } else { n }))
    }

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
        // (A Ryza character's cloth layouts keep the normal in a Float4, as they do the position.)
        let nrm = find(Semantic::Normal).filter(|a| a.kind == AttrType::Float3 || (a.kind == AttrType::Float4 && pos.kind == AttrType::Float4));
        let uv = find(Semantic::TexCoord).filter(|a| a.kind == AttrType::Float2);
        let color = find(Semantic::Color).filter(|a| matches!(a.kind, AttrType::UNorm4 | AttrType::Float4));
        let corner = find(Semantic::PointSize).filter(|a| a.kind == AttrType::Float3);
        let ib = &self.index_buffers[s.index_buffer][s.index_start..s.index_start + s.index_count];
        let lo = ib.iter().copied().min().unwrap_or(0) as usize;
        let hi = ib.iter().copied().max().map_or(0, |h| h as usize + 1);
        m.first_vertex = lo;
        let cloth = self.cloth_of(s);
        for v in lo..hi {
            // A cloth vertex is built from its piece's control points.
            if let Some((p, n)) = cloth.and_then(|c| self.cloth_vertex(layout, c, v)) {
                m.positions.push(p);
                m.normals.push(n);
            } else if let Some((_, rows)) = cloth.and_then(|c| c.placed.as_ref()) {
                m.positions.push(apply(rows, read(v, pos, 3)?, 1.0));
                if let Some(a) = nrm {
                    m.normals.push(apply(rows, read(v, a, 3)?, 0.0));
                }
            } else {
                m.positions.push(read(v, pos, 3)?);
                if let Some(a) = nrm {
                    m.normals.push(read(v, a, 3)?);
                }
            }
            if let Some(a) = uv {
                let t = read(v, a, 2)?;
                m.uvs.push([t[0], t[1]]);
            }
            if let Some(a) = corner {
                m.corners.push(read(v, a, 3)?);
            }
            if let Some(a) = color {
                let vb = &self.vertex_buffers[layout.buffers[a.buffer as usize] as usize];
                let base = v * vb.stride + a.offset as usize;
                let c = if a.kind == AttrType::UNorm4 {
                    let b = vb.data.get(base..base + 4).ok_or(Error::Corrupt("vertex colour past the end"))?;
                    [b[0], b[1], b[2], b[3]].map(|x| f32::from(x) / 255.0)
                } else {
                    let mut c = [0f32; 4];
                    for (i, o) in c.iter_mut().enumerate() {
                        *o = f32_at(vb.data, base + 4 * i)?;
                    }
                    c
                };
                m.colors.push(c);
            }
        }
        let rel = |i: u32| (i as usize - lo) as u32;
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

    /// A cloth section of one entry: `n` points, a header of `header` bytes, no tail.
    fn cloth_section(header: usize, points: &[[f32; 3]]) -> Vec<u8> {
        let mut entry = vec![0u8; header];
        entry[..4].copy_from_slice(&0x8000_0007u32.to_le_bytes());
        entry[4..8].copy_from_slice(&(points.len() as u32).to_le_bytes());
        for p in points {
            entry.extend(p.iter().chain(&[1.0]).flat_map(|f| f.to_le_bytes()));
        }
        entry.extend(std::iter::repeat_n(0u8, 24 * points.len()));
        let mut body = 1u32.to_le_bytes().to_vec();
        body.extend([0x30003u32, entry.len() as u32 + 12, 1].iter().flat_map(|w| w.to_le_bytes()));
        body.extend(entry);
        body
    }

    #[test]
    fn cloth_points_under_either_header() {
        let points = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        for header in [0x14c, 0x180] {
            let cloths = parse_cloths(&cloth_section(header, &points));
            assert_eq!(cloths, vec![Cloth { parent_bone: 7, points: points.to_vec(), placed: None }]);
        }
        assert!(parse_cloths(&cloth_section(0x160, &points)).is_empty());
    }

    #[test]
    fn placing_a_cloth_moves_its_points_once() {
        let mut c = Cloth { parent_bone: 7, points: vec![[1.0, 2.0, 3.0]], placed: None };
        let rows = [[0.0, 1.0, 0.0, 10.0], [1.0, 0.0, 0.0, 20.0], [0.0, 0.0, 1.0, 30.0]];
        c.place(4, rows);
        c.place(4, rows);
        assert_eq!((c.points[0], c.placed), ([12.0, 21.0, 33.0], Some((4, rows))));
    }

    #[test]
    fn bone_ids_go_through_the_index_map() {
        let joint = Joint { scale: [1.0; 3], parent: None, rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3] };
        // Ryza's shape: ids 0-1 to themselves, 2 unused, 3 to joint 2.
        let sk = Skeleton { joints: vec![joint; 3], index_map: vec![0, 1, 0xffff, 2], layer: 0 };
        assert_eq!([0, 1, 2, 3, 4].map(|b| sk.joint(b)), [Some(0), Some(1), None, Some(2), None]);
    }

    #[test]
    fn a_child_may_come_before_its_parent() {
        let j = |parent| Joint { scale: [1.0; 3], parent, rotation: [0.0, 0.0, 0.0, 1.0], position: [1.0, 0.0, 0.0] };
        // As in Ryza 3's FACILITY-DOOR-02: joint 1's parent is joint 2.
        let s = Skeleton { joints: vec![j(None), j(Some(2)), j(Some(0))], index_map: vec![], layer: 0 };
        assert_eq!(s.order(), [0, 2, 1]);
        assert_eq!(s.world_positions(), [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    }

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
