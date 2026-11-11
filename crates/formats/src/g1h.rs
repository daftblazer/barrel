//! KTGL morph targets (`*.g1h`, magic `_H1G`): the facial shapes of a character model, which
//! the facial animations (single-value `g1a` tracks) blend.
//!
//! ```text
//! header:  "_H1G", version ("2200"), file size, u16 header size, u16 set count,
//!          u32 (0), then a u32 offset per set
//! set:     "PH1G", version ("0100"), size, u16 header size (0x14), u16 layout,
//!          u32 n, then n - 1 u32 offsets from the set's start, one per target
//! target:  a whole g1m ("_M1G") whose geometry has one vertex buffer: an offset (x, y, z
//!          floats) for every vertex of the base model's vertex layout `layout`
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

fn u16_at(d: &[u8], at: usize) -> Result<u16, Error> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Error::Corrupt("read past the end"))
}

fn target(data: &[u8]) -> Result<Vec<[f32; 3]>, Error> {
    let model = G1m::parse(data)?;
    let geometry = model.geometry.ok_or(Error::Corrupt("g1h target without geometry"))?;
    let vb = geometry.vertex_buffers.first().ok_or(Error::Corrupt("g1h target without vertices"))?;
    if vb.stride != 12 {
        return Err(Error::Corrupt("g1h target stride is not 12"));
    }
    Ok(vb
        .data
        .chunks_exact(12)
        .take(vb.count)
        .map(|v| {
            let f = |i: usize| f32::from_le_bytes(v[i..i + 4].try_into().unwrap());
            [f(0), f(4), f(8)]
        })
        .collect())
}

pub fn parse(data: &[u8]) -> Result<Vec<MorphSet>, Error> {
    if !data.starts_with(b"_H1G") {
        return Err(Error::NotThisFormat);
    }
    let sets = u16_at(data, 14)? as usize;
    if sets > MAX_SETS {
        return Err(Error::Corrupt("g1h set count"));
    }
    let mut out = Vec::with_capacity(sets);
    for i in 0..sets {
        let at = u32_at(data, 0x14 + 4 * i)? as usize;
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
