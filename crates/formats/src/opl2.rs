//! Field placement lists (`<AREA>.opl2`, in each field's container): where every object and
//! effect of a field stands.
//!
//! Two lists of the same shape follow each other, the objects and then the effects:
//!
//! Ryza DX's has the objects only (the file ends after them).
//!
//! ```text
//! u32 type count
//! per type:     u32 name length, name, u32, then 16 bytes (Escha: 0xff in most; the name
//!               ends with its 0) or 336 (Ryza DX: 48 of 0xff, 288 of 0; no 0 after the
//!               name), u32 instance count
//! per instance: position (3 floats, 1 pad), rotation in radians (3 floats, 1 pad),
//!               scale (3 floats, 1 pad), u32 tag length, tags, 32 bytes
//! ```
//!
//! Object names are the model's name without its `_hi.g1m` (`obj-a12_deadtree_a` is
//! `obj-a12_deadtree_a_hi.g1m` in the field's `_Obj` container). The tags are 4-character
//! codes run together (`xDS_`, `xS3_`, `xV1_`); effects carry `eff`.

use crate::{Error, u32_at};

#[derive(Clone, Debug)]
pub struct Instance {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub tags: String,
}

#[derive(Clone, Debug)]
pub struct PlacedType {
    pub name: String,
    pub instances: Vec<Instance>,
}

#[derive(Clone, Debug)]
pub struct Placements {
    pub objects: Vec<PlacedType>,
    pub effects: Vec<PlacedType>,
}

fn f32_at(d: &[u8], at: usize) -> Result<f32, Error> {
    Ok(f32::from_bits(u32_at(d, at)?))
}

/// The bytes after a type's name and its word, by game: Escha & Logy DX's 16, Ryza DX's 336.
const TYPE_EXTRA: [usize; 2] = [16, 336];

fn list(d: &[u8], p: &mut usize, extra: usize) -> Result<Vec<PlacedType>, Error> {
    let count = u32_at(d, *p)? as usize;
    *p += 4;
    if count > d.len() / 24 {
        return Err(Error::Corrupt("opl2 type count"));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let len = u32_at(d, *p)? as usize;
        let raw = d.get(*p + 4..*p + 4 + len).ok_or(Error::Corrupt("opl2 name past the end"))?;
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let name = String::from_utf8_lossy(&raw[..end]).into_owned();
        *p += 4 + len + 4 + extra;
        let n = u32_at(d, *p)? as usize;
        *p += 4;
        if n > d.len() / 84 {
            return Err(Error::Corrupt("opl2 instance count"));
        }
        let mut instances = Vec::with_capacity(n);
        for _ in 0..n {
            let v = |k: usize| f32_at(d, *p + 4 * k);
            let (position, rotation, scale) = ([v(0)?, v(1)?, v(2)?], [v(4)?, v(5)?, v(6)?], [v(8)?, v(9)?, v(10)?]);
            let tag_len = u32_at(d, *p + 48)? as usize;
            let tags = d.get(*p + 52..*p + 52 + tag_len).ok_or(Error::Corrupt("opl2 tags past the end"))?;
            let end = tags.iter().position(|&b| b == 0).unwrap_or(tags.len());
            let tags = String::from_utf8_lossy(&tags[..end]).into_owned();
            *p += 52 + tag_len + 32;
            if *p > d.len() {
                return Err(Error::Corrupt("opl2 instance past the end"));
            }
            instances.push(Instance { position, rotation, scale, tags });
        }
        out.push(PlacedType { name, instances });
    }
    Ok(out)
}

/// A placement list in whichever game's type header reads it to its end exactly.
pub fn parse(d: &[u8]) -> Result<Placements, Error> {
    let mut first_error = None;
    for extra in TYPE_EXTRA {
        let mut p = 0;
        // Ryza DX's lists end after the objects (no effects list).
        let read = list(d, &mut p, extra).and_then(|objects| Ok((objects, if p == d.len() && extra != TYPE_EXTRA[0] { Vec::new() } else { list(d, &mut p, extra)? })));
        match read {
            Ok((objects, effects)) if p == d.len() => return Ok(Placements { objects, effects }),
            Ok(_) => first_error = first_error.or(Some(Error::Corrupt("opl2 has data after its lists"))),
            Err(e) => first_error = first_error.or(Some(e)),
        }
    }
    Err(first_error.unwrap_or(Error::Corrupt("opl2")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn two_lists() {
        let mut f = 1u32.to_le_bytes().to_vec();
        f.extend_from_slice(&3u32.to_le_bytes());
        f.extend_from_slice(b"ab\0");
        f.extend_from_slice(&[0u8; 4]);
        f.extend_from_slice(&[0xffu8; 16]);
        f.extend_from_slice(&1u32.to_le_bytes());
        for v in [1.0f32, 2.0, 3.0, 0.0, 0.0, 1.5, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&4u32.to_le_bytes());
        f.extend_from_slice(b"xDS_");
        f.extend_from_slice(&[0u8; 32]);
        f.extend_from_slice(&0u32.to_le_bytes());
        let p = super::parse(&f).unwrap();
        assert_eq!(p.objects[0].name, "ab");
        assert_eq!(p.objects[0].instances[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(p.objects[0].instances[0].rotation[1], 1.5);
        assert_eq!(p.objects[0].instances[0].tags, "xDS_");
        assert!(p.effects.is_empty());
    }

    #[test]
    fn ryza_type_header() {
        // Ryza DX: the name without its 0, a word, 48 bytes of 0xff and 288 of 0.
        let mut f = 1u32.to_le_bytes().to_vec();
        f.extend_from_slice(&2u32.to_le_bytes());
        f.extend_from_slice(b"ab");
        f.extend_from_slice(&13u32.to_le_bytes());
        f.extend_from_slice(&[0xffu8; 48]);
        f.extend_from_slice(&[0u8; 288]);
        f.extend_from_slice(&1u32.to_le_bytes());
        for v in [4360.5f32, 1998.5, 3803.9, 0.0, 0.0, 1.5708, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&[0u8; 32]);
        let p = super::parse(&f).unwrap();
        assert_eq!(p.objects[0].name, "ab");
        assert_eq!(p.objects[0].instances[0].position[0], 4360.5);
        assert_eq!(p.objects[0].instances[0].tags, "");
    }
}
