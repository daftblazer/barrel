//! KTGL morph targets (`*.g1h`, magic `_H1G`): the facial shapes of a character model, which
//! the facial animations (single-value `g1a` tracks) blend.
//!
//! ```text
//! header:  "_H1G", version ("2200"; "0200" in some Shallie models), file size, u16 header
//!          size (0x14; 0x10), u16 set count, then a u32 offset per set from the header's end
//!          (in 2200 after a u32 0 the header size counts)
//! set:     "PH1G", version ("0100"), size, u16 header size (0x14), u16 layout,
//!          u32 n, then n - 1 u32 offsets from the set's start, one per target
//! target:  a whole g1m ("_M1G") whose geometry has one vertex buffer: an offset (x, y, z
//!          floats, the layout's position attribute; most targets hold only that, 12 bytes a
//!          vertex) for every vertex of the base model's vertex layout `layout`
//! ```
//!
//! The base model is the `.g1m` of the same name (`PC20_MODEL_default.g1m` for
//! `PC20_MODEL_default.g1h`). A set's layout is the layout index submeshes name, so a set
//! deforms every submesh drawn from that layout; its targets have exactly as many vertices
//! as that layout's buffers.

use crate::g1m::G1m;
use crate::{Error, u32_at};

const MAX_SETS: usize = 64;
const MAX_TARGETS: usize = 1024;

/// The targets of one vertex layout.
#[derive(Clone, Debug)]
pub struct MorphSet {
    /// The base model's vertex layout these targets move.
    pub layout: u16,
    /// Per target, the offset of each vertex.
    pub targets: Vec<Vec<[f32; 3]>>,
}

impl MorphSet {
    /// The offset of each vertex with the targets weighted by `weight(bone)`, where bone
    /// k + 1 weights target k (bone 0 is the rest shape): the facial animations' tracks.
    pub fn blend(&self, weight: impl Fn(usize) -> f32) -> Vec<[f32; 3]> {
        let n = self.targets.first().map_or(0, |t| t.len());
        let mut out = vec![[0f32; 3]; n];
        for (k, t) in self.targets.iter().enumerate() {
            let w = weight(k + 1);
            if w == 0.0 {
                continue;
            }
            for (o, d) in out.iter_mut().zip(t) {
                for c in 0..3 {
                    o[c] += w * d[c];
                }
            }
        }
        out
    }
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Error::Corrupt("read past the end"))
}

fn target(data: &[u8]) -> Result<Vec<[f32; 3]>, Error> {
    let model = G1m::parse(data)?;
    let geometry = model.geometry.ok_or(Error::Corrupt("g1h target without geometry"))?;
    let vb = geometry.vertex_buffers.first().ok_or(Error::Corrupt("g1h target without vertices"))?;
    // The offsets are the layout's position (a float3 or float4; tightly packed float3s when
    // the target names no layout). Some Shallie targets carry more than the position.
    let at = match geometry.layouts.first().and_then(|l| l.attributes.iter().find(|a| a.semantic == crate::g1m::Semantic::Position && a.buffer == 0)) {
        Some(a) if matches!(a.kind, crate::g1m::AttrType::Float3 | crate::g1m::AttrType::Float4) => a.offset as usize,
        Some(_) => return Err(Error::Corrupt("g1h target position is not float")),
        None if vb.stride == 12 => 0,
        None => return Err(Error::Corrupt("g1h target without a position")),
    };
    if vb.stride < at + 12 {
        return Err(Error::Corrupt("g1h target stride"));
    }
    Ok(vb
        .data
        .chunks_exact(vb.stride)
        .take(vb.count)
        .map(|v| {
            let f = |i: usize| f32::from_le_bytes(v[at + i..at + i + 4].try_into().unwrap());
            [f(0), f(4), f(8)]
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_weights_targets_from_bone_one() {
        let set = MorphSet { layout: 0, targets: vec![vec![[1.0, 0.0, 0.0]], vec![[0.0, 2.0, 0.0]]] };
        let out = set.blend(|bone| [9.0, 0.5, 1.0][bone]);
        assert_eq!(out, [[0.5, 2.0, 0.0]]);
    }
}

pub fn parse(data: &[u8]) -> Result<Vec<MorphSet>, Error> {
    if !data.starts_with(b"_H1G") {
        return Err(Error::NotThisFormat);
    }
    // The set offsets follow the header, whose size is the u16 at +0xc (0x14 in version
    // "2200", 0x10 in Shallie's older "0200", which has no zero word before them).
    let header = u16_at(data, 0xc)? as usize;
    let sets = u16_at(data, 14)? as usize;
    if sets > MAX_SETS {
        return Err(Error::Corrupt("g1h set count"));
    }
    let mut out = Vec::with_capacity(sets);
    for i in 0..sets {
        let at = u32_at(data, header + 4 * i)? as usize;
        let head = data.get(at..at + 0x14).ok_or(Error::Corrupt("g1h set past the end"))?;
        if !head.starts_with(b"PH1G") {
            return Err(Error::Corrupt("g1h set magic"));
        }
        let size = u32_at(head, 8)? as usize;
        let set = data.get(at..at + size).ok_or(Error::Corrupt("g1h set size"))?;
        let layout = u16_at(set, 0xe)?;
        let n = u32_at(set, 0x10)? as usize;
        if n == 0 || n > MAX_TARGETS {
            return Err(Error::Corrupt("g1h target count"));
        }
        let mut targets = Vec::with_capacity(n - 1);
        for k in 0..n - 1 {
            let t = u32_at(set, 0x14 + 4 * k)? as usize;
            let len = u32_at(set, t + 8)? as usize;
            let body = set.get(t..t + len).ok_or(Error::Corrupt("g1h target past the end"))?;
            targets.push(target(body)?);
        }
        out.push(MorphSet { layout, targets });
    }
    Ok(out)
}
