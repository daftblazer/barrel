//! KTGL animations (`*.g1a`, magic `_A1G`, version "0042" in Escha & Logy DX).
//!
//! Sizes and offsets count 16-byte units.
//!
//! ```text
//! header:  "_A1G", "0042", u32 file size (units), u16 type, u16, f32 duration in seconds,
//!          u32 data offset (units), u32 ×4
//! at 0x30: u16 bone entry count, u16 highest bone number,
//!          then per entry {u32 bone, u32 offset of its block in units from 0x30}
//! block:   u32 flag, then one {u32 key count, u32 offset in units from the block} per
//!          component. Flag 4 has seven: the rotation quaternion (x, y, z, w) and the
//!          translation (x, y, z). Flag 6 has ten: scale (x, y, z), rotation, translation.
//!          Flag 2 (Sophie 2's body motions) has four: rotation only. Flag 0x66 (Sophie 2's
//!          cameras) has nine: eye, target and three more.
//!          Flag 0x191 has one: a single scalar, used by the facial animations
//!          (`*_M_Brw_*`, `*_M_Eye_*`, `*_M_Mth_*`, header type 0x104) for morph weights.
//! keys:    key count × 4 floats (a, b, c, d), then key count floats (each segment's end
//!          time in seconds), padded to 16 bytes
//! ```
//!
//! A component's value inside a segment is a·s³ + b·s² + c·s + d, with s going 0 to 1 from
//! the previous key's end time (0 for the first) to this key's.

use crate::{Error, u32_at};

#[derive(Clone, Debug)]
pub struct Key {
    pub coefficients: [f32; 4],
    pub end: f32,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub bone: u32,
    pub flag: u32,
    /// The components as stored (see the module notes for their order per flag).
    pub components: Vec<Vec<Key>>,
}

impl Track {
    /// Indices of the scale, rotation and translation components, if the track has them.
    fn layout(&self) -> Option<(Option<usize>, usize, usize)> {
        match (self.flag, self.components.len()) {
            (4, 7) => Some((None, 0, 4)),
            (6, 10) => Some((Some(0), 3, 7)),
            _ => None,
        }
    }

    /// A rotation-only track's quaternion (x, y, z, w) at `time` (flag 2, Sophie 2): the
    /// joint keeps its bind translation and scale.
    pub fn rotation(&self, time: f32) -> Option<[f32; 4]> {
        (self.flag == 2 && self.components.len() == 4).then(|| std::array::from_fn(|i| sample(&self.components[i], time)))
    }

    /// A single-scalar track's value at `time` (a morph weight).
    pub fn scalar(&self, time: f32) -> Option<f32> {
        (self.flag == SCALAR && self.components.len() == 1).then(|| sample(&self.components[0], time))
    }

    /// Rotation (quaternion x, y, z, w), translation and scale at `time`.
    pub fn pose(&self, time: f32) -> Option<([f32; 4], [f32; 3], [f32; 3])> {
        let (scale, rot, trans) = self.layout()?;
        let c = |i: usize| sample(&self.components[i], time);
        let s = scale.map_or([1.0; 3], |s| [c(s), c(s + 1), c(s + 2)]);
        Some(([c(rot), c(rot + 1), c(rot + 2), c(rot + 3)], [c(trans), c(trans + 1), c(trans + 2)], s))
    }
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub duration: f32,
    pub tracks: Vec<Track>,
}

/// How many components a block with this flag has.
fn components(flag: u32) -> Option<usize> {
    match flag {
        2 => Some(4),
        4 => Some(7),
        6 => Some(10),
        CAMERA => Some(9),
        SCALAR => Some(1),
        _ => None,
    }
}

/// The flag of a camera track (Sophie 2): nine components, the eye's x, y, z, the target's
/// x, y, z, then three more (0, 0.7 and 1.0 in the samples: roll, field of view in radians,
/// and one still to be named).
pub const CAMERA: u32 = 0x66;

/// The flag of a single-scalar track (facial morph weights).
pub const SCALAR: u32 = 0x191;

/// A component's value at `time` seconds.
pub fn sample(keys: &[Key], time: f32) -> f32 {
    let mut start = 0.0;
    for (i, k) in keys.iter().enumerate() {
        if time <= k.end || i + 1 == keys.len() {
            let span = (k.end - start).max(1e-6);
            let s = ((time - start) / span).clamp(0.0, 1.0);
            let [a, b, c, d] = k.coefficients;
            return ((a * s + b) * s + c) * s + d;
        }
        start = k.end;
    }
    0.0
}

/// The component count of a block whose flag is not in [`components`] (the later games add
/// some, such as Sophie 2's 0x66 camera tracks): the list of {key count, offset} ends where
/// the first component's keys begin, and each component's keys follow the previous one's.
fn infer_components(d: &[u8], at: usize) -> Option<usize> {
    let first = u32_at(d, at + 8).ok()? as usize;
    // 4 bytes of flag and 8 a component, padded to 16: n components fit when
    // 4 + 8n <= 16 * first < 4 + 8n + 16.
    let n = (16 * first).checked_sub(4)? / 8;
    if n == 0 || n > 64 {
        return None;
    }
    let mut expected = first;
    for c in 0..n {
        let keys = u32_at(d, at + 4 + 8 * c).ok()? as usize;
        let off = u32_at(d, at + 8 + 8 * c).ok()? as usize;
        if keys == 0 || off != expected {
            // Padding may leave room for one more pair than there are components.
            return (c > 0 && c + 1 == n).then_some(c);
        }
        expected = off + (keys * 20).div_ceil(16);
    }
    Some(n)
}

pub fn parse(d: &[u8]) -> Result<Animation, Error> {
    if !d.starts_with(b"_A1G") {
        return Err(Error::NotThisFormat);
    }
    let size = (u32_at(d, 8)? as usize).saturating_mul(16);
    if size > d.len() {
        return Err(Error::Corrupt("g1a size"));
    }
    let d = &d[..size];
    let duration = f32::from_bits(u32_at(d, 16)?);
    let count = d.get(0x30..0x32).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or(Error::Corrupt("g1a header"))?;
    let mut tracks = Vec::with_capacity(count);
    for i in 0..count {
        let bone = u32_at(d, 0x34 + 8 * i)?;
        let block = u32_at(d, 0x38 + 8 * i)? as usize;
        let at = 0x30 + 16 * block;
        let flag = u32_at(d, at)?;
        let n = match components(flag) {
            Some(n) => n,
            None => infer_components(d, at).ok_or(Error::Corrupt("g1a block flag not known"))?,
        };
        let mut comps = Vec::with_capacity(n);
        for c in 0..n {
            let keys = u32_at(d, at + 4 + 8 * c)? as usize;
            let off = u32_at(d, at + 8 + 8 * c)? as usize;
            let data = 0x30 + 16 * (block + off);
            if keys == 0 || keys > 4096 || data + 20 * keys > d.len() {
                return Err(Error::Corrupt("g1a keys past the end"));
            }
            let mut list = Vec::with_capacity(keys);
            for k in 0..keys {
                let f = |j: usize| u32_at(d, data + 16 * k + 4 * j).map(f32::from_bits);
                list.push(Key { coefficients: [f(0)?, f(1)?, f(2)?, f(3)?], end: f32::from_bits(u32_at(d, data + 16 * keys + 4 * k)?) });
            }
            comps.push(list);
        }
        tracks.push(Track { bone, flag, components: comps });
    }
    Ok(Animation { duration, tracks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_join() {
        let keys = vec![
            Key { coefficients: [0.0, 0.0, 1.0, 0.0], end: 1.0 },
            Key { coefficients: [0.0, 0.0, -2.0, 1.0], end: 3.0 },
        ];
        assert!((sample(&keys, 0.5) - 0.5).abs() < 1e-6);
        assert!((sample(&keys, 1.0) - 1.0).abs() < 1e-6);
        assert!((sample(&keys, 2.0) - 0.0).abs() < 1e-6);
        assert!((sample(&keys, 3.0) + 1.0).abs() < 1e-6);
    }
}
