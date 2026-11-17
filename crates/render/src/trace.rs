//! The 2D draw trace: with BARREL_TRACE_DRAWS=file, each [`crate::text::Layer`] writes the quads
//! it draws, one JSON line per change, for `barrel-parity` to diff against the original
//! game's draw capture (escha-recomp's ESCHA_TRACE_DRAWS).
//!
//! `{"frame":f,"layer":id,"quads":[…]}`: from frame f on (the renderer's frame count), layer
//! id draws these quads, in order, until its next line; `{"frame":f,"layer":id,"gone":true}`
//! when it is destroyed. Each quad: its texture by content (`tex` and the FNV-1a 64 hash of its
//! first mip's first layer, as the capture names it), rect and uv as (left, top, right,
//! bottom) with the rect on a 1920 × 1080 screen, colour, and whether it is turned.
//!
//! The scene's 3D draws, one line per frame: `{"frame":f,"draws3d":[…],"view_proj":[16],
//! "shadow_vp":[16]}`, matrices column-major (glam's `to_cols_array`, render space: x mirrored).
//! Each draw: `pass` (`shadow` the sun's shadow map, `scene` the lit colour pass, `sky` the
//! distant view, `outline`, `terrain`), `idx` its index count, `inst` its instances, `tex` its
//! textures by content name (as the capture names them; the colour texture, then mask and
//! emission where it has them) and `world` its model matrix. `view_proj` is the colour pass's
//! (the camera's jitter included); the renderer keeps no separate view and projection.

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
        let mut c = q.corners().map(|[x, y]| [x * sx, y * sy]);
        let mut uv = q.uv;
        if q.spin.is_some() && !q.turn {
            // The capture's own form (`barrel-parity`'s `rotated`): the picture's corner nearest the
            // screen's top left names (u0, v0) and the farthest (u1, v1), whatever the turn.
            let [u0, v0, u1, v1] = q.uv;
            let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
            let key = |i: usize| c[i][0] + c[i][1];
            let near = (0..4).min_by(|&a, &b| key(a).total_cmp(&key(b))).unwrap();
            let far = (0..4).max_by(|&a, &b| key(a).total_cmp(&key(b))).unwrap();
            let (n, f) = (uvs[near], uvs[far]);
            let at = |u: f32, v: f32| (0..4).min_by(|&a, &b| ((uvs[a][0] - u).abs() + (uvs[a][1] - v).abs()).total_cmp(&((uvs[b][0] - u).abs() + (uvs[b][1] - v).abs()))).unwrap();
            let old = c;
            c = [old[at(n[0], n[1])], old[at(f[0], n[1])], old[at(f[0], f[1])], old[at(n[0], f[1])]];
            uv = [n[0], n[1], f[0], f[1]];
        }
        // A turned quad's rect is the box round its corners, and its corners are listed.
        let [l, t, r, b] = if q.spin.is_some() {
            let f = |i: usize, g: fn(f32, f32) -> f32| c.iter().map(|p| p[i]).reduce(g).unwrap_or(0.0);
            [f(0, f32::min) / sx, f(1, f32::min) / sy, f(0, f32::max) / sx, f(1, f32::max) / sy]
        } else {
            q.rect
        };
        let corners = if q.spin.is_some() { format!(",\"corners\":{c:?}") } else { String::new() };
        let tex = if q.uv[0] < 0.0 { "fill" } else { names.get(q.texture).map_or("?", String::as_str) };
        line.push_str(&format!(
            "{}{{\"tex\":\"{tex}\",\"rect\":[{},{},{},{}],\"uv\":{:?},\"color\":{:?},\"turn\":{}{corners}}}",
            if i > 0 { "," } else { "" },
            l * sx, t * sy, r * sx, b * sy, uv, q.color, q.turn
        ));
    }
    line.push_str("]}\n");
    let mut g = o.lock().unwrap();
    let _ = g.write_all(line.as_bytes());
    let _ = g.flush();
}

/// The names of the scene's textures by texture number (set by `upload_textures`), for
/// [`Draws3d`].
static SCENE_TEXTURES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Records the scene textures' trace names (the textures `Draw::texture` numbers).
pub fn scene_textures(names: Vec<String>) {
    *SCENE_TEXTURES.lock().unwrap() = names;
}

/// One frame's 3D draws, collected while the scene is recorded; `None` with the trace off, so
/// the hot path pays one branch per draw.
pub struct Draws3d(Option<Vec<String>>);

impl Draws3d {
    pub fn new() -> Draws3d {
        Draws3d(enabled().then(Vec::new))
    }

    pub fn on(&self) -> bool {
        self.0.is_some()
    }

    /// One mesh draw: its pass, index count, textures (scene texture numbers) and world matrix.
    pub fn add(&mut self, pass: &str, index_count: u32, textures: &[Option<usize>], world: &glam::Mat4) {
        let Some(v) = self.0.as_mut() else { return };
        let names = SCENE_TEXTURES.lock().unwrap();
        let tex: Vec<&str> = textures.iter().flatten().filter_map(|&t| names.get(t).map(String::as_str)).collect();
        v.push(format!("{{\"pass\":\"{pass}\",\"idx\":{index_count},\"inst\":1,\"tex\":{tex:?},\"world\":{:?}}}", world.to_cols_array()));
    }

    pub fn finish(self, frame: u64, view_proj: &glam::Mat4, shadow_vp: &glam::Mat4) {
        let (Some(v), Some(o)) = (self.0, out()) else { return };
        let line = format!(
            "{{\"frame\":{frame},\"draws3d\":[{}],\"view_proj\":{:?},\"shadow_vp\":{:?}}}\n",
            v.join(","),
            view_proj.to_cols_array(),
            shadow_vp.to_cols_array()
        );
        let mut g = o.lock().unwrap();
        let _ = g.write_all(line.as_bytes());
        let _ = g.flush();
    }
}

pub fn gone(frame: u64, layer: u64) {
    if let Some(o) = out() {
        let mut g = o.lock().unwrap();
        let _ = writeln!(g, "{{\"frame\":{frame},\"layer\":{layer},\"gone\":true}}");
        let _ = g.flush();
    }
}
