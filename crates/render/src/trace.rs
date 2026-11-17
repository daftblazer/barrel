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
//! textures by content name (as the capture names them; the colour texture, then mask, normal
//! map and emission where it has them) and `world` its model matrix. A skinned draw also has
//! `bones`: its bone palette in slot order (the joints its submesh names), each bone as 16
//! floats, column-major, the instance's model matrix times the joint's skinning matrix (render
//! space); `simulated` the slots the game moves by its cloth simulation, which Barrel does not. `view_proj` is the colour pass's
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

/// A turned quad narrower than this (pixels) in either direction is a card seen edge on.
const EDGE_ON: f32 = 8.0;

pub fn quads(frame: u64, layer: u64, width: u32, height: u32, names: &[String], quads: &[Quad]) {
    let Some(o) = out() else { return };
    let (sx, sy) = (1920.0 / width as f32, 1080.0 / height as f32);
    let mut line = format!("{{\"frame\":{frame},\"layer\":{layer},\"quads\":[");
    for (i, q) in quads.iter().enumerate() {
        let mut c = q.corners().map(|[x, y]| [x * sx, y * sy]);
        let mut uv = q.uv;
        let extent = |i: usize| c.iter().map(|p| p[i]).fold(f32::MIN, f32::max) - c.iter().map(|p| p[i]).fold(f32::MAX, f32::min);
        // (A card seen edge on is left as it is.)
        if q.spin.is_some() && !q.turn && extent(0) >= EDGE_ON && extent(1) >= EDGE_ON {
            // The capture's own form (`barrel-parity`'s `rotated`): the picture's corner nearest the
            // screen's top left names (u0, v0) and the farthest (u1, v1), whatever the turn.
            let [u0, v0, u1, v1] = q.uv;
            // In the game's strip order (top-left, top-right, bottom-left, bottom-right), so a tie
            // between two corners (a quad turned a quarter) falls as it does there.
            let uvs = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];
            let pos = [c[0], c[1], c[3], c[2]];
            // Corners within 0.01 pixels of one another's distance tie, and the leftmost (rightmost
            // for the far one) is taken, as `barrel-parity` reads the capture.
            let key = |i: usize| pos[i][0] + pos[i][1];
            let (lo, hi) = ((0..4).map(key).fold(f32::MAX, f32::min), (0..4).map(key).fold(f32::MIN, f32::max));
            let left_first = |&a: &usize, &b: &usize| pos[a][0].total_cmp(&pos[b][0]).then(pos[a][1].total_cmp(&pos[b][1]));
            let near = (0..4).filter(|&i| key(i) < lo + 0.01).min_by(left_first).unwrap();
            let far = (0..4).filter(|&i| key(i) > hi - 0.01).max_by(left_first).unwrap();
            let (n, f) = (uvs[near], uvs[far]);
            let at = |u: f32, v: f32| (0..4).min_by(|&a, &b| ((uvs[a][0] - u).abs() + (uvs[a][1] - v).abs()).total_cmp(&((uvs[b][0] - u).abs() + (uvs[b][1] - v).abs()))).unwrap();
            c = [pos[at(n[0], n[1])], pos[at(f[0], n[1])], pos[at(f[0], f[1])], pos[at(n[0], f[1])]];
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

    /// Adds the bone palette to the draw just added (`add`).
    pub fn bones(&mut self, palette: &[glam::Mat4], base: usize, joints: &[u32], model: &glam::Mat4) {
        let Some(last) = self.0.as_mut().and_then(|v| v.last_mut()) else { return };
        let joint = |j: &u32| (j & !crate::Draw::SIMULATED) as usize;
        let all: Vec<f32> = joints.iter().flat_map(|j| (*model * palette.get(base + joint(j)).copied().unwrap_or(glam::Mat4::IDENTITY)).to_cols_array()).collect();
        let simulated: Vec<usize> = (0..joints.len()).filter(|&k| joints[k] & crate::Draw::SIMULATED != 0).collect();
        last.pop();
        last.push_str(&format!(",\"bones\":{all:?},\"simulated\":{simulated:?}}}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    /// A skinned draw's trace line carries its bones in palette slot order: the model times the
    /// joint's matrix, the simulated ones named by slot.
    #[test]
    fn a_draws_bones_follow_its_palette_slots() {
        let mut t = Draws3d(Some(Vec::new()));
        let model = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let palette = [Mat4::IDENTITY, Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0)), Mat4::from_translation(Vec3::new(0.0, 0.0, 7.0))];
        t.add("scene", 3, &[], &model);
        // slot 0 is joint 2, slot 1 joint 1 (swinging)
        t.bones(&palette[..], 0, &[2, 1 | crate::Draw::SIMULATED], &model);
        let line = t.0.unwrap().pop().unwrap();
        let bones = line.split("\"bones\":[").nth(1).unwrap().split(']').next().unwrap();
        let v: Vec<f32> = bones.split(',').map(|x| x.trim().parse().unwrap()).collect();
        assert_eq!(v.len(), 32);
        assert_eq!(&v[12..15], &[10.0, 0.0, 7.0]);
        assert_eq!(&v[28..31], &[10.0, 5.0, 0.0]);
        assert!(line.ends_with("\"simulated\":[1]}"), "{line}");
    }
}
