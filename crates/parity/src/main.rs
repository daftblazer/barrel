//! barrel-parity: what the original game draws (a draw capture from escha-recomp's null
//! Direct3D 11, `ESCHA_TRACE_DRAWS`) as a list Barrel's own frames can be diffed against.
//!
//!   barrel-parity textures <game folder> <index.tsv>   every texture's content hash and file
//!   barrel-parity shader <trace dir> <name>            a captured shader's buffers and code
//!   barrel-parity screen <trace dir> <frame> [index]   a frame's draws, screen-space
//!   barrel-parity diff <trace dir> <frame> <barrel trace> <frame> [index]
//!                                                      the game's 2D quads against Barrel's
//!
//! Screen positions are in a 1920 × 1080 frame whatever size the game drew at, so captures at
//! any resolution compare.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use barrel_formats::dxbc;
use barrel_formats::elixir::Elixir;
use barrel_formats::g1t::G1t;
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// FNV-1a 64, as the capture names textures (the bytes of the first mip of the first layer).
fn hash(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in b {
        h = (h ^ x as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Lines `hash \t file[:entry]#n \t width \t height \t format` for every texture of every g1t,
/// loose or in an elixir container.
fn textures(root: &Path, out: &Path) -> Result<()> {
    let mut files = Vec::new();
    walk(root, &mut files);
    let mut lines = String::new();
    let mut add = |name: &str, data: &[u8]| {
        let Ok(g) = G1t::parse(data) else { return };
        for (i, t) in g.textures.iter().enumerate() {
            let Some(top) = t.levels.first() else { continue };
            let n = t.format.level_size(t.width, t.height).min(top.len());
            lines.push_str(&format!("{:016x}\t{name}#{i}\t{}\t{}\t{:?}\n", hash(&top[..n]), t.width, t.height, t.format));
        }
    };
    for f in &files {
        let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().into_owned();
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        if name.ends_with(".g1t") {
            add(&rel, &std::fs::read(f)?);
        } else if name.ends_with(".elixir.gz") || name.ends_with(".elixir") {
            let bytes = std::fs::read(f)?;
            let Ok(e) = Elixir::parse(&bytes) else { continue };
            for entry in e.entries.iter().filter(|e| e.name.ends_with(".g1t")) {
                add(&format!("{rel}:{}", entry.name), e.data(entry));
            }
        }
    }
    std::fs::write(out, lines)?;
    Ok(())
}

fn index(path: Option<&str>) -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    if let Some(p) = path {
        for l in std::fs::read_to_string(p)?.lines() {
            let mut f = l.split('\t');
            if let (Some(h), Some(n)) = (f.next(), f.next()) {
                m.entry(format!("tex{h}")).or_insert_with(|| n.to_string());
            }
        }
    }
    Ok(m)
}

struct Trace {
    dir: PathBuf,
}

impl Trace {
    fn blob(&self, name: &str) -> Result<Vec<u8>> {
        let h = &name[name.len().saturating_sub(16)..];
        Ok(std::fs::read(self.dir.join("blobs").join(format!("{h}.bin")))?)
    }

    /// The events of one frame.
    fn frame(&self, n: u64) -> Result<Vec<Value>> {
        let text = std::fs::read_to_string(self.dir.join("draws.jsonl"))?;
        let mut out = Vec::new();
        let mut on = false;
        for l in text.lines() {
            let v: Value = serde_json::from_str(l)?;
            if let Some(f) = v.get("frame").and_then(Value::as_u64) {
                if on {
                    break;
                }
                on = f == n;
            } else if on {
                out.push(v);
            }
        }
        Ok(out)
    }
}

fn floats(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
}

/// An input layout's elements: (semantic with index, DXGI format, byte offset).
fn layout(text: &str) -> Vec<(String, u32, usize)> {
    text.split(';').filter(|e| !e.is_empty()).filter_map(|e| {
        let mut w = e.split(' ');
        let sem = w.next()?.to_string();
        let mut fmt = 0;
        let mut off = 0;
        for x in w {
            if let Some(v) = x.strip_prefix('f') {
                fmt = v.parse().ok()?;
            } else if let Some(v) = x.strip_prefix('o') {
                off = v.parse().ok()?;
            }
        }
        Some((sem, fmt, off))
    }).collect()
}

/// One vertex attribute as floats (the formats the 2D layouts use).
fn attribute(v: &[u8], fmt: u32, off: usize) -> Vec<f32> {
    let f = |n: usize| floats(&v[off..(off + 4 * n).min(v.len())]);
    match fmt {
        2 => f(4),
        6 => f(3),
        16 => f(2),
        41 => f(1),
        28 => v[off..off + 4].iter().map(|&b| b as f32 / 255.0).collect(),
        _ => Vec::new(),
    }
}

fn shader(trace: &Path, name: &str) -> Result<()> {
    let t = Trace { dir: trace.into() };
    let code = t.blob(name)?;
    let p = dxbc::program(&code)?;
    println!("{:?}", p.stage);
    for b in &p.bindings {
        println!("binding {} kind {} slot {} count {}", b.name, b.kind, b.slot, b.count);
    }
    for cb in &p.buffers {
        println!("cbuffer {} ({} bytes)", cb.name, cb.size);
        for v in &cb.variables {
            println!("  {} +{} ({})", v.name, v.offset, v.size);
        }
    }
    for e in &p.inputs {
        println!("in {}{} v{} mask {:x}", e.semantic, e.index, e.register, e.mask);
    }
    for e in &p.outputs {
        println!("out {}{} o{} mask {:x}", e.semantic, e.index, e.register, e.mask);
    }
    for l in dxbc::disassemble(p.code) {
        println!("  {l}");
    }
    Ok(())
}

/// A frame's draws: per draw its target, shaders, textures (by file where the index knows
/// them), blend, and for 2D draws (a float4 position) each vertex on a 1920 × 1080 screen with
/// its texture coordinate and colour; 3D draws only their counts.
fn screen(trace: &Path, frame: u64, idx: Option<&str>) -> Result<()> {
    let t = Trace { dir: trace.into() };
    let names = index(idx)?;
    let tex = |n: &str| names.get(n).cloned().unwrap_or_else(|| n.to_string());
    for e in t.frame(frame)? {
        if let Some(c) = e.get("clear") {
            println!("clear {} {}", c.as_str().unwrap_or("?"), e["color"]);
            continue;
        }
        let s = |k: &str| e[k].as_str().unwrap_or("-").to_string();
        let textures: Vec<String> = e["ps_tex"].as_object().map(|m| m.iter().map(|(k, v)| format!("{k}={}", tex(v.as_str().unwrap_or("")))).collect()).unwrap_or_default();
        println!("draw {} rt {} count {} topo {} vs {} ps {} blend {} tex [{}]", e["draw"], e["rt"], e["count"], e["topology"], s("vs"), s("ps"), s("blend"), textures.join(" "));
        let lay = layout(&String::from_utf8_lossy(&t.blob(&s("layout")).unwrap_or_default()));
        let Some(&(_, pos_fmt, pos_off)) = lay.iter().find(|l| l.0 == "POSITION0") else { continue };
        let vb = &e["vb"][0];
        let stride = vb["stride"].as_u64().unwrap_or(0) as usize;
        if pos_fmt != 2 || stride == 0 || e["count"].as_u64().unwrap_or(0) > 64 {
            continue;
        }
        let data = t.blob(vb["data"].as_str().unwrap_or("")).unwrap_or_default();
        let m = e["vs_cb"].get("0").and_then(Value::as_str).and_then(|h| t.blob(h).ok()).map(|b| floats(&b)).unwrap_or_default();
        let vp: Vec<f32> = e["viewport"].as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
        let _ = vp;
        for v in data.chunks_exact(stride) {
            let p = attribute(v, pos_fmt, pos_off);
            let (mut x, mut y) = (p[0], p[1]);
            if m.len() >= 16 {
                let c = |j: usize| p[0] * m[j] + p[1] * m[4 + j] + p[2] * m[8 + j] + p[3] * m[12 + j];
                let w = c(3);
                let (nx, ny) = (c(0) / w, c(1) / w);
                x = (nx + 1.0) / 2.0 * 1920.0;
                y = (1.0 - ny) / 2.0 * 1080.0;
            }
            let mut line = format!("  ({x:.2}, {y:.2})");
            for (sem, fmt, off) in lay.iter().filter(|l| l.0 != "POSITION0") {
                let a = attribute(v, *fmt, *off);
                line.push_str(&format!(" {sem} {:?}", a.iter().map(|f| (f * 10000.0).round() / 10000.0).collect::<Vec<_>>()));
            }
            println!("{line}");
        }
    }
    Ok(())
}

/// A 2D rectangle as both sides describe it.
#[derive(Clone, Debug)]
struct Q {
    tex: String,
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    /// the game's pixel shader and its constants (what the colour does not say: fades)
    how: String,
}

fn arr<const N: usize>(v: &Value) -> [f32; N] {
    std::array::from_fn(|i| v.get(i).and_then(Value::as_f64).unwrap_or(0.0) as f32)
}

/// The game's 2D draws of a frame as rectangles: each 4-vertex strip with a float4 position,
/// placed by its vertex shader's matrix (cb0, `mL2P`), on the screen the frame ends on; draws
/// that read a render target (the copy to the back buffer, post effects) and 3D draws are
/// counted, not compared.
fn game_quads(t: &Trace, frame: u64) -> Result<(Vec<Q>, usize)> {
    let mut out = Vec::new();
    let mut other = 0;
    for e in t.frame(frame)? {
        if e.get("draw").is_none() {
            continue;
        }
        let s = |k: &str| e[k].as_str().unwrap_or("-").to_string();
        let tex0 = e["ps_tex"].get("0").and_then(Value::as_str).unwrap_or("").to_string();
        let lay = layout(&String::from_utf8_lossy(&t.blob(&s("layout")).unwrap_or_default()));
        let vb = &e["vb"][0];
        let stride = vb["stride"].as_u64().unwrap_or(0) as usize;
        let pos = lay.iter().find(|l| l.0 == "POSITION0").map(|l| (l.1, l.2));
        let (Some((2, pos_off)), 4, 5) = (pos, e["count"].as_u64().unwrap_or(0), e["topology"].as_u64().unwrap_or(0)) else {
            other += 1;
            continue;
        };
        if tex0.starts_with("rt") || stride == 0 {
            other += 1;
            continue;
        }
        let data = t.blob(vb["data"].as_str().unwrap_or("")).unwrap_or_default();
        let m = e["vs_cb"].get("0").and_then(Value::as_str).and_then(|h| t.blob(h).ok()).map(|b| floats(&b)).unwrap_or_default();
        let find = |n: &str| lay.iter().find(|l| l.0 == n).map(|l| (l.1, l.2));
        let mut pts = Vec::new();
        for v in data.chunks_exact(stride) {
            let p = attribute(v, 2, pos_off);
            let (x, y) = if m.len() >= 16 {
                let c = |j: usize| p[0] * m[j] + p[1] * m[4 + j] + p[2] * m[8 + j] + p[3] * m[12 + j];
                let w = c(3);
                ((c(0) / w + 1.0) / 2.0 * 1920.0, (1.0 - c(1) / w) / 2.0 * 1080.0)
            } else {
                (p[0], p[1])
            };
            let uv = find("TEXCOORD0").map(|(f, o)| attribute(v, f, o)).unwrap_or_default();
            let col = find("COLOR0").map(|(f, o)| attribute(v, f, o)).unwrap_or_else(|| vec![1.0; 4]);
            pts.push((x, y, uv, col));
        }
        if pts.len() != 4 {
            other += 1;
            continue;
        }
        // the corner nearest the top left and the one opposite it
        let tl = pts.iter().min_by(|a, b| (a.0 + a.1).total_cmp(&(b.0 + b.1))).unwrap();
        let br = pts.iter().max_by(|a, b| (a.0 + a.1).total_cmp(&(b.0 + b.1))).unwrap();
        let uv = |p: &(f32, f32, Vec<f32>, Vec<f32>), i: usize| p.2.get(i).copied().unwrap_or(0.0);
        let ps_cb = e["ps_cb"].get("0").and_then(Value::as_str).and_then(|h| t.blob(h).ok()).map(|b| floats(&b)).unwrap_or_default();
        out.push(Q {
            tex: tex0,
            rect: [tl.0, tl.1, br.0, br.1],
            uv: [uv(tl, 0), uv(tl, 1), uv(br, 0), uv(br, 1)],
            color: std::array::from_fn(|i| tl.3.get(i).copied().unwrap_or(1.0)),
            how: format!("{} {:?}", s("ps"), ps_cb),
        });
    }
    Ok((out, other))
}

/// Barrel's quads on screen at `frame`: each layer's last list set by then.
fn barrel_quads(path: &Path, frame: u64) -> Result<Vec<Q>> {
    let mut layers: std::collections::BTreeMap<u64, Vec<Q>> = Default::default();
    for l in std::fs::read_to_string(path)?.lines() {
        let v: Value = serde_json::from_str(l)?;
        if v["frame"].as_u64().unwrap_or(0) > frame {
            break;
        }
        let id = v["layer"].as_u64().unwrap_or(0);
        if v.get("gone").is_some() && v["frame"].as_u64().unwrap_or(0) < frame {
            layers.remove(&id);
            continue;
        }
        if let Some(qs) = v["quads"].as_array() {
            layers.insert(id, qs.iter().map(|q| Q { tex: q["tex"].as_str().unwrap_or("").into(), rect: arr(&q["rect"]), uv: arr(&q["uv"]), color: arr(&q["color"]), how: String::new() }).collect());
        }
    }
    Ok(layers.into_values().flatten().collect())
}

fn close(a: &[f32; 4], b: &[f32; 4], tol: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

/// Matches the two lists in order (longest common subsequence on texture and place) and
/// reports each pair's differences and what only one side draws. Returns the mismatches.
fn diff(trace: &Path, gframe: u64, barrel: &Path, bframe: u64, idx: Option<&str>) -> Result<usize> {
    let t = Trace { dir: trace.into() };
    let names = index(idx)?;
    let tex = |n: &str| names.get(n).cloned().unwrap_or_else(|| n.to_string());
    let (g, other) = game_quads(&t, gframe)?;
    let b = barrel_quads(barrel, bframe)?;
    let same = |x: &Q, y: &Q| x.tex == y.tex && close(&x.rect, &y.rect, 8.0);
    let (n, m) = (g.len(), b.len());
    let mut l = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            l[i][j] = if same(&g[i], &b[j]) { l[i + 1][j + 1] + 1 } else { l[i + 1][j].max(l[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut bad) = (0, 0, 0);
    let show = |q: &Q| format!("{} rect {:?} uv {:?} color {:?}", tex(&q.tex), q.rect, q.uv, q.color);
    while i < n || j < m {
        if i < n && j < m && same(&g[i], &b[j]) {
            let (x, y) = (&g[i], &b[j]);
            let mut d = Vec::new();
            if !close(&x.rect, &y.rect, 0.5) {
                d.push(format!("rect {:?} vs {:?}", x.rect, y.rect));
            }
            if !close(&x.uv, &y.uv, 0.001) {
                d.push(format!("uv {:?} vs {:?}", x.uv, y.uv));
            }
            if !close(&x.color, &y.color, 1.5 / 255.0) {
                d.push(format!("color {:?} vs {:?}", x.color, y.color));
            }
            if d.is_empty() {
                println!("  same   {}", tex(&x.tex));
            } else {
                bad += 1;
                println!("  DIFF   {}: {}  [game {}]", tex(&x.tex), d.join("; "), x.how);
            }
            i += 1;
            j += 1;
        } else if i < n && (j >= m || l[i + 1][j] >= l[i][j + 1]) {
            bad += 1;
            println!("  GAME   {}  [{}]", show(&g[i]), g[i].how);
            i += 1;
        } else {
            bad += 1;
            println!("  BARREL {}", show(&b[j]));
            j += 1;
        }
    }
    println!("{} game quads, {} Barrel quads, {bad} mismatches ({other} game draws not compared: 3D or render-target reads)", n, m);
    Ok(bad)
}

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("textures") if a.len() == 3 => textures(Path::new(&a[1]), Path::new(&a[2])),
        Some("shader") if a.len() == 3 => shader(Path::new(&a[1]), &a[2]),
        Some("diff") if a.len() >= 5 => {
            let bad = diff(Path::new(&a[1]), a[2].parse()?, Path::new(&a[3]), a[4].parse()?, a.get(5).map(String::as_str))?;
            std::process::exit(if bad == 0 { 0 } else { 1 })
        }
        Some("screen") if a.len() >= 3 => screen(Path::new(&a[1]), a[2].parse()?, a.get(3).map(String::as_str)),
        _ => Err("usage: barrel-parity textures <game> <index.tsv> | shader <trace> <name> | screen <trace> <frame> [index.tsv] | diff <trace> <frame> <barrel trace> <frame> [index.tsv]".into()),
    }
}
