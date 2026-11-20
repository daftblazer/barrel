//! A small binary glTF 2.0 writer: one binary buffer, and JSON built from what callers push
//! into its lists. Only what `chara-gltf` needs (meshes, skins, images, an animation).

use serde_json::{Value, json};

pub const FLOAT: u32 = 5126;
pub const UNSIGNED_SHORT: u32 = 5123;
pub const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

#[derive(Default)]
pub struct Builder {
    pub bin: Vec<u8>,
    pub views: Vec<Value>,
    pub accessors: Vec<Value>,
    pub images: Vec<Value>,
    pub textures: Vec<Value>,
    pub materials: Vec<Value>,
    pub meshes: Vec<Value>,
    pub nodes: Vec<Value>,
    pub skins: Vec<Value>,
    pub animations: Vec<Value>,
    pub scene: Vec<usize>,
}

impl Builder {
    /// Append `bytes` to the buffer (4-byte aligned) as a buffer view; its index.
    pub fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let mut v = json!({"buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len()});
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(v);
        self.views.len() - 1
    }

    #[allow(clippy::too_many_arguments)]
    fn accessor(&mut self, bytes: &[u8], target: Option<u32>, component: u32, count: usize, kind: &str, normalized: bool, bounds: Option<(Vec<f32>, Vec<f32>)>) -> usize {
        let view = self.view(bytes, target);
        let mut a = json!({"bufferView": view, "componentType": component, "count": count, "type": kind});
        if normalized {
            a["normalized"] = json!(true);
        }
        if let Some((min, max)) = bounds {
            a["min"] = json!(min);
            a["max"] = json!(max);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }

    /// A float accessor: `data` holds `components` floats per element (`kind` "SCALAR", "VEC2",
    /// "VEC3", "VEC4" or "MAT4"); `bounds` adds min and max (POSITION and animation inputs need
    /// them); `vertex` marks a vertex attribute.
    pub fn floats(&mut self, data: &[f32], kind: &str, bounds: bool, vertex: bool) -> usize {
        let n = components(kind);
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let b = bounds.then(|| {
            let (mut lo, mut hi) = (vec![f32::MAX; n], vec![f32::MIN; n]);
            for e in data.chunks_exact(n) {
                for k in 0..n {
                    lo[k] = lo[k].min(e[k]);
                    hi[k] = hi[k].max(e[k]);
                }
            }
            (lo, hi)
        });
        self.accessor(&bytes, vertex.then_some(ARRAY_BUFFER), FLOAT, data.len() / n, kind, false, b)
    }

    /// Four 16-bit joint numbers a vertex.
    pub fn joints(&mut self, data: &[[u16; 4]]) -> usize {
        let bytes: Vec<u8> = data.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        self.accessor(&bytes, Some(ARRAY_BUFFER), UNSIGNED_SHORT, data.len(), "VEC4", false, None)
    }

    pub fn indices(&mut self, data: &[u32]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.accessor(&bytes, Some(ELEMENT_ARRAY_BUFFER), UNSIGNED_INT, data.len(), "SCALAR", false, None)
    }

    /// An image stored in the buffer; its index.
    pub fn image(&mut self, png: &[u8], name: &str) -> usize {
        let view = self.view(png, None);
        self.images.push(json!({"bufferView": view, "mimeType": "image/png", "name": name}));
        self.images.len() - 1
    }

    /// The finished `.glb`: header, the JSON chunk (space padded) and the binary chunk (zero padded).
    pub fn finish(mut self) -> Vec<u8> {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let mut root = json!({
            "asset": {"version": "2.0", "generator": "barrel-tool chara-gltf"},
            "scene": 0,
            "scenes": [{"nodes": self.scene}],
            "buffers": [{"byteLength": self.bin.len()}],
            "bufferViews": self.views,
            "accessors": self.accessors,
            "nodes": self.nodes,
        });
        for (key, list) in [("images", self.images), ("textures", self.textures), ("materials", self.materials), ("meshes", self.meshes), ("skins", self.skins), ("animations", self.animations)] {
            if !list.is_empty() {
                root[key] = Value::Array(list);
            }
        }
        let mut text = serde_json::to_vec(&root).expect("json");
        while text.len() % 4 != 0 {
            text.push(b' ');
        }
        let total = 12 + 8 + text.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&text);
        out.extend_from_slice(&(self.bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&self.bin);
        out
    }
}

fn components(kind: &str) -> usize {
    match kind {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        "MAT4" => 16,
        _ => panic!("accessor type {kind}"),
    }
}

/// A vertex's skin as glTF wants it: at most four distinct joints below `joint_count`, the
/// largest first, weights summing to 1. A vertex nothing weighs on rides joint 0.
pub fn normalise_skin(joints: [u32; 4], weights: [f32; 4], joint_count: usize) -> ([u16; 4], [f32; 4]) {
    let mut merged: Vec<(u32, f32)> = Vec::new();
    for (&j, &w) in joints.iter().zip(&weights) {
        if w > 0.0 && (j as usize) < joint_count {
            match merged.iter_mut().find(|m| m.0 == j) {
                Some(m) => m.1 += w,
                None => merged.push((j, w)),
            }
        }
    }
    merged.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f32 = merged.iter().map(|m| m.1).sum();
    if total <= 0.0 {
        return ([0; 4], [1.0, 0.0, 0.0, 0.0]);
    }
    let (mut j, mut w) = ([0u16; 4], [0f32; 4]);
    for (k, m) in merged.iter().enumerate() {
        j[k] = m.0 as u16;
        w[k] = m.1 / total;
    }
    (j, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A triangle skinned to two joints: the file parses, every view lies inside the buffer,
    /// every accessor inside its view, and the weights sum to 1.
    #[test]
    fn a_skinned_triangle() {
        let mut b = Builder::default();
        let pos = b.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], "VEC3", true, true);
        let (j0, w0) = normalise_skin([1, 1, 0, 0], [0.5, 0.25, 0.5, 0.0], 2);
        let (j1, w1) = normalise_skin([0, 0, 0, 0], [0.0; 4], 2);
        let joints = b.joints(&[j0, j1, j0]);
        let weights = b.floats(&[w0, w1, w0].concat(), "VEC4", false, true);
        let idx = b.indices(&[0, 1, 2]);
        let ibm = b.floats(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0].repeat(2), "MAT4", false, false);
        b.meshes.push(json!({"name": "sub00", "primitives": [{"attributes": {"POSITION": pos, "JOINTS_0": joints, "WEIGHTS_0": weights}, "indices": idx}]}));
        b.nodes.push(json!({"name": "a", "children": [1]}));
        b.nodes.push(json!({"name": "b"}));
        b.nodes.push(json!({"name": "m", "mesh": 0, "skin": 0}));
        b.skins.push(json!({"joints": [0, 1], "inverseBindMatrices": ibm}));
        b.scene = vec![0, 2];
        let glb = b.finish();

        assert_eq!(&glb[..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize, glb.len());
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let root: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let bin = &glb[20 + json_len + 8..];
        assert_eq!(root["buffers"][0]["byteLength"].as_u64().unwrap() as usize, bin.len());
        for v in root["bufferViews"].as_array().unwrap() {
            assert!(v["byteOffset"].as_u64().unwrap() + v["byteLength"].as_u64().unwrap() <= bin.len() as u64);
        }
        let view = |a: &Value| root["bufferViews"][a["bufferView"].as_u64().unwrap() as usize].clone();
        for a in root["accessors"].as_array().unwrap() {
            let size = match a["componentType"].as_u64().unwrap() as u32 {
                FLOAT | UNSIGNED_INT => 4,
                _ => 2,
            } * components(a["type"].as_str().unwrap());
            assert!(a["count"].as_u64().unwrap() as usize * size <= view(a)["byteLength"].as_u64().unwrap() as usize);
        }
        let at = view(&root["accessors"][weights])["byteOffset"].as_u64().unwrap() as usize;
        for v in 0..3 {
            let sum: f32 = (0..4).map(|k| f32::from_le_bytes(bin[at + 16 * v + 4 * k..][..4].try_into().unwrap())).sum();
            assert!((sum - 1.0).abs() < 1e-6, "vertex {v} weights sum to {sum}");
        }
        assert_eq!(root["accessors"][pos]["max"], json!([1.0, 1.0, 0.0]));
        assert_eq!(j0, [1, 0, 0, 0]);
    }
}
