//! Direct3D shader programs (DXBC containers, Shader Model 5), as the game's shader packs
//! (`Data/x64/Shader/*.g1s`, `_S2G`) carry them: what a program reads and writes, so the
//! engine's own shaders can be written to do the same.
//!
//! ```text
//! "DXBC", 16-byte checksum, u32 1, u32 total size, u32 chunk count, u32 chunk offsets
//! chunk: 4-byte tag, u32 size, data
//!   RDEF  constant buffers {name, variables, size} and bindings {name, type, slot}
//!   ISGN / OSGN  inputs and outputs {semantic, index, register, mask}
//!   SHEX  the program (version token: stage in the high 16 bits)
//! ```
//!
//! Offsets inside RDEF and the signatures are from the start of the chunk's data.

use crate::{u32_at, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Pixel,
    Vertex,
    Other(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub name: String,
    /// Bytes from the start of its buffer.
    pub offset: u32,
    pub size: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstantBuffer {
    pub name: String,
    pub size: u32,
    pub variables: Vec<Variable>,
}

/// A resource the program binds: `kind` is D3D's input type (0 a constant buffer, 2 a
/// texture, 3 a sampler, ...), `slot` its register.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub name: String,
    pub kind: u32,
    pub slot: u32,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub semantic: String,
    pub index: u32,
    pub register: u32,
    /// Which of x, y, z, w it uses (bits 0 to 3).
    pub mask: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program<'a> {
    pub stage: Stage,
    pub buffers: Vec<ConstantBuffer>,
    pub bindings: Vec<Binding>,
    pub inputs: Vec<Element>,
    pub outputs: Vec<Element>,
    /// The SHEX chunk's tokens after its version and length (the instructions).
    pub code: &'a [u8],
}

fn text(data: &[u8], at: usize) -> String {
    let s = data.get(at..).unwrap_or_default();
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    String::from_utf8_lossy(&s[..end]).into_owned()
}

/// The chunks of a container: (tag, data).
pub fn chunks(data: &[u8]) -> Result<Vec<([u8; 4], &[u8])>, Error> {
    if data.get(..4) != Some(b"DXBC") {
        return Err(Error::NotThisFormat);
    }
    let total = (u32_at(data, 24)? as usize).min(data.len());
    let n = u32_at(data, 28)? as usize;
    if n > 64 {
        return Err(Error::Corrupt("DXBC chunk count"));
    }
    (0..n)
        .map(|i| {
            let at = u32_at(data, 32 + 4 * i)? as usize;
            let tag: [u8; 4] = data.get(at..at + 4).ok_or(Error::Corrupt("DXBC chunk past the end"))?.try_into().unwrap();
            let size = u32_at(data, at + 4)? as usize;
            let body = data.get(at + 8..at + 8 + size).filter(|_| at + 8 + size <= total).ok_or(Error::Corrupt("DXBC chunk past the end"))?;
            Ok((tag, body))
        })
        .collect()
}

fn signature(d: &[u8]) -> Result<Vec<Element>, Error> {
    let n = u32_at(d, 0)? as usize;
    (0..n.min(64))
        .map(|i| {
            let e = 8 + 24 * i;
            Ok(Element {
                semantic: text(d, u32_at(d, e)? as usize),
                index: u32_at(d, e + 4)?,
                register: u32_at(d, e + 16)?,
                mask: *d.get(e + 20).ok_or(Error::Corrupt("DXBC signature past the end"))?,
            })
        })
        .collect()
}

fn reflection(d: &[u8]) -> Result<(Vec<ConstantBuffer>, Vec<Binding>), Error> {
    let (cb_count, cb_at, bind_count, bind_at) = (u32_at(d, 0)? as usize, u32_at(d, 4)? as usize, u32_at(d, 8)? as usize, u32_at(d, 12)? as usize);
    let target = u32_at(d, 16)?;
    // Shader Model 5 variables are 40 bytes (with texture and sampler ranges), older 24.
    // (The target is u8 minor, u8 major, u16 program type.)
    let var_size = if (target >> 8) & 0xff >= 5 { 40 } else { 24 };
    let bindings = (0..bind_count.min(256))
        .map(|i| {
            let b = bind_at + 32 * i;
            Ok(Binding { name: text(d, u32_at(d, b)? as usize), kind: u32_at(d, b + 4)?, slot: u32_at(d, b + 20)?, count: u32_at(d, b + 24)? })
        })
        .collect::<Result<_, Error>>()?;
    let buffers = (0..cb_count.min(64))
        .map(|i| {
            let c = cb_at + 24 * i;
            let (vars, var_at) = (u32_at(d, c + 4)? as usize, u32_at(d, c + 8)? as usize);
            let variables = (0..vars.min(1024))
                .map(|k| {
                    let v = var_at + var_size * k;
                    Ok(Variable { name: text(d, u32_at(d, v)? as usize), offset: u32_at(d, v + 4)?, size: u32_at(d, v + 8)? })
                })
                .collect::<Result<_, Error>>()?;
            Ok(ConstantBuffer { name: text(d, u32_at(d, c)? as usize), size: u32_at(d, c + 12)?, variables })
        })
        .collect::<Result<_, Error>>()?;
    Ok((buffers, bindings))
}

/// Reads a DXBC container.
pub fn program(data: &[u8]) -> Result<Program<'_>, Error> {
    let mut p = Program { stage: Stage::Other(u32::MAX), buffers: Vec::new(), bindings: Vec::new(), inputs: Vec::new(), outputs: Vec::new(), code: &[] };
    for (tag, body) in chunks(data)? {
        match &tag {
            b"RDEF" => (p.buffers, p.bindings) = reflection(body)?,
            b"ISGN" => p.inputs = signature(body)?,
            b"OSGN" => p.outputs = signature(body)?,
            b"SHEX" | b"SHDR" => {
                let version = u32_at(body, 0)?;
                p.stage = match version >> 16 {
                    0 => Stage::Pixel,
                    1 => Stage::Vertex,
                    s => Stage::Other(s),
                };
                p.code = body.get(8..).unwrap_or_default();
            }
            _ => {}
        }
    }
    Ok(p)
}

/// The DXBC programs inside a file (a `_S2G` shader pack holds several one after another),
/// as byte ranges.
pub fn find(data: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = data[at..].windows(4).position(|w| w == b"DXBC").map(|k| k + at) {
        let size = u32_at(data, k + 24).map_or(0, |s| s as usize);
        if size >= 32 && k + size <= data.len() {
            out.push(k..k + size);
            at = k + size;
        } else {
            at = k + 4;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A container built by hand: one ISGN chunk with one element.
    #[test]
    fn reads_a_signature() {
        let mut sig = Vec::new();
        sig.extend(1u32.to_le_bytes());
        sig.extend(8u32.to_le_bytes());
        // name at 32, index 2, system value 0, component type 3, register 5, mask 0b0111.
        for v in [32u32, 2, 0, 3, 5] {
            sig.extend(v.to_le_bytes());
        }
        sig.extend([7u8, 7, 0, 0]);
        sig.extend(b"TEXCOORD\0");
        let mut d = Vec::new();
        d.extend(b"DXBC");
        d.extend([0u8; 16]);
        d.extend(1u32.to_le_bytes());
        let total = 36 + 8 + sig.len();
        d.extend((total as u32).to_le_bytes());
        d.extend(1u32.to_le_bytes());
        d.extend(36u32.to_le_bytes());
        d.extend(b"ISGN");
        d.extend((sig.len() as u32).to_le_bytes());
        d.extend(&sig);
        let p = program(&d).unwrap();
        assert_eq!(p.inputs, [Element { semantic: "TEXCOORD".into(), index: 2, register: 5, mask: 7 }]);
        assert_eq!(find(&d), [0..total]);
    }
}
