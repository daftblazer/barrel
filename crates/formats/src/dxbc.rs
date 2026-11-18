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

/// Shader Model 4/5 instruction names by opcode (D3D's `D3D10_SB_OPCODE_TYPE` and the
/// Shader Model 5 additions used here); the rest print as `op_N`.
const NAMES: [&str; 112] = [
    "add", "and", "break", "breakc", "call", "callc", "case", "continue", "continuec", "cut", "default", "deriv_rtx", "deriv_rty", "discard",
    "div", "dp2", "dp3", "dp4", "else", "emit", "emitthencut", "endif", "endloop", "endswitch", "eq", "exp", "frc", "ftoi", "ftou", "ge",
    "iadd", "if", "ieq", "ige", "ilt", "imad", "imax", "imin", "imul", "ine", "ineg", "ishl", "ishr", "itof", "label", "ld", "ld_ms", "log",
    "loop", "lt", "mad", "min", "max", "customdata", "mov", "movc", "mul", "ne", "nop", "not", "or", "resinfo", "ret", "retc", "round_ne",
    "round_ni", "round_pi", "round_z", "rsq", "sample", "sample_c", "sample_c_lz", "sample_l", "sample_d", "sample_b", "sqrt", "switch",
    "sincos", "udiv", "ult", "uge", "umul", "umad", "umax", "umin", "ushr", "utof", "xor", "dcl_resource", "dcl_constantbuffer",
    "dcl_sampler", "dcl_index_range", "dcl_gs_output_topology", "dcl_gs_input_primitive", "dcl_max_output_vertex_count", "dcl_input",
    "dcl_input_sgv", "dcl_input_siv", "dcl_input_ps", "dcl_input_ps_sgv", "dcl_input_ps_siv", "dcl_output", "dcl_output_sgv",
    "dcl_output_siv", "dcl_temps", "dcl_indexable_temp", "dcl_global_flags", "reserved", "lod", "gather4", "sample_pos", "sample_info",
];

/// One operand from `t` (at `*i`), moving `*i` past it.
fn operand(t: &[u32], i: &mut usize) -> String {
    let Some(&tok) = t.get(*i) else { return "?".into() };
    *i += 1;
    let comps = tok & 3;
    let kind = (tok >> 12) & 0xff;
    let dims = (tok >> 20) & 3;
    let mut modifier = 0;
    let mut ext = tok >> 31 == 1;
    while ext {
        let Some(&e) = t.get(*i) else { break };
        *i += 1;
        if e & 0x3f == 1 {
            modifier = (e >> 6) & 0xff;
        }
        ext = e >> 31 == 1;
    }
    let xyzw = |n: u32| ['x', 'y', 'z', 'w'][(n & 3) as usize];
    let select = if comps == 2 {
        match (tok >> 2) & 3 {
            0 => {
                let m = (tok >> 4) & 0xf;
                if m == 0 || m == 0xf { String::new() } else { format!(".{}", (0..4).filter(|k| m >> k & 1 == 1).map(xyzw).collect::<String>()) }
            }
            1 => {
                let s = (tok >> 4) & 0xff;
                let sw: String = (0..4).map(|k| xyzw(s >> (2 * k))).collect();
                if sw == "xyzw" { String::new() } else { format!(".{sw}") }
            }
            _ => format!(".{}", xyzw((tok >> 4) & 3)),
        }
    } else {
        String::new()
    };
    let body = match kind {
        // Immediates: one or four 32-bit values.
        4 => {
            let n = if comps == 1 { 1 } else { 4 };
            let vals: Vec<String> = (0..n)
                .map(|_| {
                    let v = t.get(*i).copied().unwrap_or(0);
                    *i += 1;
                    let f = f32::from_bits(v);
                    if v != 0 && (f.abs() < 1e-6 || f.abs() > 1e7 || f.is_nan()) { format!("{v:#x}") } else { format!("{f}") }
                })
                .collect();
            format!("l({})", vals.join(", "))
        }
        _ => {
            let prefix = match kind {
                0 => "r",
                1 => "v",
                2 => "o",
                3 => "x",
                6 => "s",
                7 => "t",
                8 => "cb",
                9 => "icb",
                0xc => "oDepth",
                _ => "",
            };
            let prefix = if prefix.is_empty() { format!("type{kind}_") } else { prefix.to_string() };
            let mut s = prefix;
            for d in 0..dims {
                let rep = (tok >> (22 + 3 * d)) & 7;
                let mut idx = String::new();
                if rep == 0 || rep == 3 {
                    idx = t.get(*i).copied().unwrap_or(0).to_string();
                    *i += 1;
                }
                if rep == 2 || rep == 3 {
                    let r = operand(t, i);
                    idx = if idx.is_empty() { r } else { format!("{r} + {idx}") };
                }
                if d == 0 {
                    s.push_str(&idx);
                } else {
                    s.push_str(&format!("[{idx}]"));
                }
            }
            s
        }
    };
    let s = format!("{body}{select}");
    match modifier {
        1 => format!("-{s}"),
        2 => format!("|{s}|"),
        3 => format!("-|{s}|"),
        _ => s,
    }
}

/// The program's code (after its version and length tokens) as one line per instruction.
pub fn disassemble(code: &[u8]) -> Vec<String> {
    let t: Vec<u32> = code.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    let mut out = Vec::new();
    let mut at = 0;
    let mut depth = 0usize;
    while at < t.len() {
        let tok = t[at];
        let op = tok & 0x7ff;
        let mut len = ((tok >> 24) & 0x7f) as usize;
        if op == 0x35 {
            // customdata (an immediate constant buffer): its length is the next token.
            len = t.get(at + 1).copied().unwrap_or(2) as usize;
            out.push(format!("{}customdata ({} values)", "  ".repeat(depth), len.saturating_sub(2)));
            // its rows of four floats
            for row in t.get(at + 2..(at + len).min(t.len())).unwrap_or(&[]).chunks(4) {
                let values: Vec<String> = row.iter().map(|w| format!("{}", f32::from_bits(*w))).collect();
                out.push(format!("{}  {{{}}}", "  ".repeat(depth), values.join(", ")));
            }
            at += len.max(1);
            continue;
        }
        let len = len.max(1);
        let end = (at + len).min(t.len());
        let name = NAMES.get(op as usize).map_or_else(|| format!("op_{op}"), |n| n.to_string());
        let mut i = at + 1;
        // Extended opcode tokens (sample offsets, resource types).
        let mut ext = tok >> 31 == 1;
        while ext && i < end {
            ext = t[i] >> 31 == 1;
            i += 1;
        }
        let saturate = if tok >> 13 & 1 == 1 && op < 88 { "_sat" } else { "" };
        let test = if matches!(op, 3 | 5 | 8 | 13 | 31 | 63) { if tok >> 18 & 1 == 1 { "_nz" } else { "_z" } } else { "" };
        if matches!(op, 18 | 21 | 22 | 23) {
            depth = depth.saturating_sub(1);
        }
        let line = if op >= 88 && !matches!(op, 108..=111) {
            // Declarations: their first operand when they have one, then the rest raw.
            let has_operand = !matches!(op, 104 | 106 | 92 | 93 | 94);
            let mut s = name.clone();
            if has_operand && i < end {
                s.push(' ');
                s.push_str(&operand(&t, &mut i));
            }
            let rest: Vec<String> = t[i.min(end)..end].iter().map(|v| v.to_string()).collect();
            if !rest.is_empty() {
                s.push_str(&format!(" {}", rest.join(" ")));
            }
            s
        } else {
            let mut ops = Vec::new();
            while i < end {
                ops.push(operand(&t, &mut i));
            }
            format!("{name}{saturate}{test} {}", ops.join(", ")).trim_end().to_string()
        };
        out.push(format!("{}{line}", "  ".repeat(depth)));
        if matches!(op, 18 | 31 | 48 | 76) {
            depth += 1;
        }
        at = end.max(at + 1);
    }
    out
}

#[cfg(test)]
mod asm_tests {
    use super::*;

    #[test]
    fn instructions() {
        // mad r0.xyz, v1.xyzx, cb0[2].xxxx, l(1.0)
        let words: [u32; 10] = [
            (50 | (10 << 24)),
            0x0010_0072, 0,             // r0.xyz
            0x0010_1246, 1,             // v1.xyzx
            0x0020_8006, 0, 2,          // cb0[2].xxxx
            0x0000_4001, 1.0f32.to_bits(),
        ];
        let code: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(disassemble(&code), ["mad r0.xyz, v1.xyzx, cb0[2].xxxx, l(1)"]);
    }
}
