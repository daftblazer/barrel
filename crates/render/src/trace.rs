//! The 2D draw trace: with BARREL_TRACE_DRAWS=file, each [`crate::text::Layer`] writes the quads
//! it draws, one JSON line per change, for `barrel-parity` to diff against the original
//! game's draw capture (escha-recomp's ESCHA_TRACE_DRAWS).
//!
//! `{"frame":f,"layer":id,"quads":[…]}`: from frame f on (the renderer's frame count), layer
//! id draws these quads, in order, until its next line; `{"frame":f,"layer":id,"gone":true}`
//! when it is destroyed. Each quad: its texture by content (`tex` and the FNV-1a 64 hash of its
//! first mip's first layer, as the capture names it), rect and uv as (left, top, right,
//! bottom) with the rect on a 1920 × 1080 screen, colour, and whether it is turned.

use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::text::Quad;

fn out() -> Option<&'static Mutex<std::io::BufWriter<std::fs::File>>> {
    static O: OnceLock<Option<Mutex<std::io::BufWriter<std::fs::File>>>> = OnceLock::new();
    O.get_or_init(|| {
        let p = std::env::var_os("BARREL_TRACE_DRAWS")?;
        Some(Mutex::new(std::io::BufWriter::new(std::fs::File::create(p).ok()?)))
    })
    .as_ref()
}

pub fn enabled() -> bool {
    out().is_some()
}

/// A new layer's id.
pub fn layer_id() -> u64 {
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// FNV-1a 64 (the capture's texture names).
pub fn hash(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in b {
        h = (h ^ x as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A texture's trace name.
pub fn texture_name(t: &barrel_formats::g1t::Texture) -> String {
    let top = t.levels.first().copied().unwrap_or_default();
    let n = t.format.level_size(t.width, t.height).min(top.len());
    format!("tex{:016x}", hash(&top[..n]))
}

pub fn quads(frame: u64, layer: u64, width: u32, height: u32, names: &[String], quads: &[Quad]) {
    let Some(o) = out() else { return };
    let (sx, sy) = (1920.0 / width as f32, 1080.0 / height as f32);
    let mut line = format!("{{\"frame\":{frame},\"layer\":{layer},\"quads\":[");
    for (i, q) in quads.iter().enumerate() {
        let [l, t, r, b] = q.rect;
        let tex = if q.uv[0] < 0.0 { "fill" } else { names.get(q.texture).map_or("?", String::as_str) };
        line.push_str(&format!(
            "{}{{\"tex\":\"{tex}\",\"rect\":[{},{},{},{}],\"uv\":{:?},\"color\":{:?},\"turn\":{}}}",
            if i > 0 { "," } else { "" },
            l * sx, t * sy, r * sx, b * sy, q.uv, q.color, q.turn
        ));
    }
    line.push_str("]}\n");
    let mut g = o.lock().unwrap();
    let _ = g.write_all(line.as_bytes());
    let _ = g.flush();
}

pub fn gone(frame: u64, layer: u64) {
    if let Some(o) = out() {
        let mut g = o.lock().unwrap();
        let _ = writeln!(g, "{{\"frame\":{frame},\"layer\":{layer},\"gone\":true}}");
        let _ = g.flush();
    }
}
