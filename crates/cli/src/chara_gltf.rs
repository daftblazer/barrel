//! `barrel-tool chara-gltf`: a Ryza DX character's visible mesh, skeleton, skin and textures as
//! one binary glTF (`docs/formats.md`, "glTF export").

use std::collections::BTreeMap;
use std::path::Path;

use barrel_formats::elixir::Elixir;
use barrel_formats::g1m::{G1m, Geometry, Skeleton};
use barrel_formats::g1t::G1t;
use barrel_formats::oid;
use glam::{Mat4, Quat, Vec3};
use serde_json::{Value, json};

use crate::gltf::{Builder, normalise_skin};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Game units are centimetres; glTF's are metres.
pub const SCALE: f32 = 0.01;

#[derive(Default)]
pub struct Options {
    /// Export only these submeshes (by index in the model).
    pub only: Option<Vec<usize>>,
    /// Export this motion (a number as in `motion::path`) as a glTF animation.
    pub pose: Option<u32>,
}

/// The game is left-handed with the character facing +z; its renderer mirrors x to draw
/// right-handed, and so does this: x is negated everywhere (positions, normals, joint
/// translations; a rotation (x, y, z, w) becomes (x, -y, -z, w)).
fn mirror_point(p: Vec3) -> Vec3 {
    Vec3::new(-p.x, p.y, p.z) * SCALE
}

fn mirror_rotation(q: Quat) -> Quat {
    Quat::from_xyzw(q.x, -q.y, -q.z, q.w)
}

fn mirror_dir(n: [f32; 3]) -> [f32; 3] {
    [-n[0], n[1], n[2]]
}

/// A joint's bind world matrix in glTF space (metres, x mirrored).
fn gltf_world(skeleton: &Skeleton) -> Vec<Mat4> {
    let mut out = vec![Mat4::IDENTITY; skeleton.joints.len()];
    for i in skeleton.order() {
        let j = &skeleton.joints[i];
        let local = Mat4::from_scale_rotation_translation(Vec3::from(j.scale), mirror_rotation(Quat::from_array(j.rotation).normalize()), mirror_point(Vec3::from(j.position)));
        out[i] = match j.parent {
            Some(p) => out[p] * local,
            None => local,
        };
    }
    out
}

/// The joint names: Oid.bin names bone ids (`id,name`); the skeleton's index map turns an id
/// into a joint. Joints with no name are `joint_N`.
fn joint_names(skeleton: &Skeleton, oid_bytes: Option<&[u8]>) -> Vec<String> {
    let mut names: Vec<String> = (0..skeleton.joints.len()).map(|j| format!("joint_{j}")).collect();
    let Some(list) = oid_bytes.and_then(|b| oid::parse(b).ok()) else { return names };
    let by_id = oid::by_id(&list);
    let mut taken = std::collections::HashSet::new();
    for (id, name) in by_id.iter().map(|(i, n)| (*i, *n)).collect::<BTreeMap<_, _>>() {
        if let Some(j) = skeleton.joint(id)
            && taken.insert(name.to_string())
        {
            names[j] = name.to_string();
        }
    }
    names
}

/// Move the cloth pieces' points into the model's space, as the game's load does
/// (`play::actors::place_cloths`), so cloth submeshes read where they hang at rest.
fn place_cloths(geometry: &mut Geometry, skeleton: &Skeleton) {
    let bind = barrel_engine::skin::bind_world(skeleton);
    for cloth in &mut geometry.cloths {
        if let Some((j, m)) = skeleton.joint(cloth.parent_bone).and_then(|j| Some((j, bind.get(j)?))) {
            cloth.place(j as u32, [m.row(0).into(), m.row(1).into(), m.row(2).into()]);
        }
    }
}

fn find<'a>(c: &'a Elixir, suffix: &str) -> Option<&'a [u8]> {
    c.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(suffix)).map(|e| c.data(e))
}

pub fn export(root: &Path, chara: &str, out: &Path, opts: &Options) -> Result<()> {
    let install = barrel_ryza::install::Install::open(root)?;
    let set = chara.to_ascii_lowercase();
    let dir = format!("\\data\\x64\\character\\{set}\\{set}_model");
    let container_bytes = install.read(&format!("{dir}.elixir.gz"))?;
    let pack_bytes = install.read(&format!("{dir}.g1t"))?;
    let container = Elixir::parse(&container_bytes)?;
    let skeleton = G1m::parse(find(&container, "_model.g1m").ok_or("no skeleton entry")?)?.skeleton.ok_or("no skeleton")?;
    let mut geometry = G1m::parse(find(&container, "_model_default.g1m").ok_or("no mesh entry")?)?.geometry.ok_or("no geometry")?;
    place_cloths(&mut geometry, &skeleton);
    let pack = G1t::parse(&pack_bytes)?;
    let names = joint_names(&skeleton, find(&container, "oid.bin"));
    let n_joints = skeleton.joints.len();

    // The visible mesh: the submeshes the colour pass (group 0) draws at LOD 0. The rest are
    // other LODs or the shadow pass's.
    let mut visible: Vec<usize> = geometry.mesh_groups.iter().filter(|m| m.lod == 0 && m.group == 0).flat_map(|m| m.submeshes.iter().map(|&s| s as usize)).collect();
    visible.sort();
    visible.dedup();
    let skipped: Vec<usize> = (0..geometry.submeshes.len()).filter(|s| !visible.contains(s)).collect();
    eprintln!("{chara}: {} submeshes, {} joints; LOD 0 colour pass draws {visible:?}; skipped (other LODs, shadow only) {skipped:?}", geometry.submeshes.len(), n_joints);
    if let Some(only) = &opts.only {
        visible.retain(|s| only.contains(s));
    }

    let mut b = Builder::default();

    // The skeleton: joint nodes 0..n in bind pose, then the meshes.
    let world = gltf_world(&skeleton);
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n_joints];
    let mut roots = Vec::new();
    for (i, j) in skeleton.joints.iter().enumerate() {
        match j.parent {
            Some(p) => children[p].push(i),
            None => roots.push(i),
        }
    }
    for (i, j) in skeleton.joints.iter().enumerate() {
        let q = mirror_rotation(Quat::from_array(j.rotation).normalize());
        let mut node = json!({
            "name": names[i],
            "translation": mirror_point(Vec3::from(j.position)).to_array(),
            "rotation": q.to_array(),
            "scale": j.scale,
        });
        if !children[i].is_empty() {
            node["children"] = json!(children[i]);
        }
        b.nodes.push(node);
    }
    let ibm: Vec<f32> = world.iter().flat_map(|m| m.inverse().to_cols_array()).collect();
    let ibm = b.floats(&ibm, "MAT4", false, false);
    b.skins.push(json!({"name": format!("{chara}_skin"), "joints": (0..n_joints).collect::<Vec<_>>(), "inverseBindMatrices": ibm, "skeleton": roots[0]}));
    b.scene.extend(&roots);

    // Materials and images: one glTF material per model material a drawn submesh uses.
    let mut images: BTreeMap<usize, usize> = BTreeMap::new();
    let mut materials: BTreeMap<usize, usize> = BTreeMap::new();
    let mut image_of = |b: &mut Builder, t: usize| -> Option<usize> {
        if let Some(&i) = images.get(&t) {
            return Some(i);
        }
        let tex = pack.textures.get(t)?;
        let png = barrel_formats::png::encode_rgba(tex.width, tex.height, &tex.to_rgba8());
        let i = b.image(&png, &format!("tex{t}"));
        images.insert(t, i);
        Some(i)
    };

    let mut stats = Vec::new();
    for &si in &visible {
        let s = geometry.submeshes[si];
        let mesh = geometry.simple_mesh(&s)?;
        if mesh.uvs.is_empty() || mesh.normals.is_empty() {
            eprintln!("submesh {si}: no UVs or normals, skipped");
            continue;
        }
        let skin = geometry.skin(&s)?;
        let shader = geometry.shader_of(si).unwrap_or("?").to_string();

        // Material.
        let m = s.material;
        let material = match materials.get(&m) {
            Some(&k) => k,
            None => {
                let mat = &geometry.materials[m];
                let mut textures = Vec::new();
                let mut albedo = None;
                for t in &mat.textures {
                    let image = image_of(&mut b, t.index as usize);
                    let mut e = json!({"kind": t.kind, "subtype": t.subtype, "layer": t.layer, "pack_index": t.index, "image": image});
                    if t.kind == 1 && albedo.is_none() {
                        albedo = image;
                        e["role"] = json!("albedo");
                    }
                    textures.push(e);
                }
                let albedo = albedo.or_else(|| mat.textures.first().and_then(|t| image_of(&mut b, t.index as usize)));
                let params: Vec<Value> = geometry.shader_params.get(s.shader_params).map(|set| set.iter().map(|p| json!({"name": p.name, "values": p.values})).collect()).unwrap_or_default();
                let mut j = json!({
                    "name": format!("mat{m}"),
                    "doubleSided": true,
                    "pbrMetallicRoughness": {"metallicFactor": 0.0, "roughnessFactor": 1.0},
                    // The game's shader hash is not in the model; its shader name and parameter set are.
                    "extras": {"material": m, "shader": shader, "textures": textures, "params": params},
                });
                if let Some(img) = albedo {
                    b.textures.push(json!({"source": img}));
                    j["pbrMetallicRoughness"]["baseColorTexture"] = json!({"index": b.textures.len() - 1});
                    let opaque = mat.textures.iter().find(|t| t.kind == 1).or(mat.textures.first()).and_then(|t| pack.textures.get(t.index as usize)).is_none_or(|t| t.opaque());
                    if !opaque {
                        j["alphaMode"] = json!("MASK");
                        j["alphaCutoff"] = json!(0.5);
                    }
                }
                b.materials.push(j);
                materials.insert(m, b.materials.len() - 1);
                b.materials.len() - 1
            }
        };

        // Vertices.
        let n = mesh.positions.len();
        let positions: Vec<f32> = mesh.positions.iter().flat_map(|&p| mirror_point(Vec3::from(p)).to_array()).collect();
        let normals: Vec<f32> = mesh.normals.iter().flat_map(|&p| mirror_dir(p)).collect();
        let uvs: Vec<f32> = mesh.uvs.iter().flat_map(|t| t.iter().copied()).collect();
        let mut attrs = json!({
            "POSITION": b.floats(&positions, "VEC3", true, true),
            "NORMAL": b.floats(&normals, "VEC3", false, true),
            "TEXCOORD_0": b.floats(&uvs, "VEC2", false, true),
        });
        if mesh.colors.len() == n {
            let colors: Vec<f32> = mesh.colors.iter().flatten().copied().collect();
            attrs["COLOR_0"] = json!(b.floats(&colors, "VEC4", false, true));
        }
        let skinned = skin.as_ref().filter(|k| k.len() == n);
        let mut js = Vec::with_capacity(n);
        let mut ws = Vec::with_capacity(n * 4);
        for v in 0..n {
            let (j, w) = match skinned {
                Some(k) => normalise_skin(k[v].0, k[v].1, n_joints),
                None => ([0; 4], [1.0, 0.0, 0.0, 0.0]),
            };
            js.push(j);
            ws.extend(w);
        }
        attrs["JOINTS_0"] = json!(b.joints(&js));
        attrs["WEIGHTS_0"] = json!(b.floats(&ws, "VEC4", false, true));
        // Mirroring x turns the winding over: swap two corners of each triangle.
        let indices: Vec<u32> = mesh.triangles.iter().flat_map(|t| [t[0], t[2], t[1]]).collect();
        let idx = b.indices(&indices);
        let name = format!("sub{si:02}_mat{m}");
        b.meshes.push(json!({"name": name, "primitives": [{"attributes": attrs, "indices": idx, "material": material}], "extras": {"submesh": si, "shader": shader}}));
        b.nodes.push(json!({"name": name, "mesh": b.meshes.len() - 1, "skin": 0}));
        b.scene.push(b.nodes.len() - 1);
        stats.push(format!("{si}: {n} vertices, {} triangles, shader {shader}, material {m}{}", mesh.triangles.len(), if skinned.is_some() { "" } else { ", no skin (on joint 0)" }));
    }
    for s in &stats {
        eprintln!("submesh {s}");
    }

    if let Some(motion) = opts.pose {
        animation(&mut b, &install, chara, &skeleton, motion)?;
    }
    std::fs::write(out, b.finish())?;
    Ok(())
}

/// The motion `motion` of the character's motion sets as a glTF animation, one key every
/// 1/30 s over its length, for every joint (translation, rotation, scale), posed as the game
/// poses (`play::actors::Files::joints_of`, less the expression and the head turns).
fn animation(b: &mut Builder, install: &barrel_ryza::install::Install, chara: &str, skeleton: &Skeleton, motion: u32) -> Result<()> {
    let tag = chara.to_ascii_uppercase();
    let base = install.read("\\saves\\chara\\charabase.xml")?;
    let sets = install.read("\\saves\\chara\\charamotionset.xml")?;
    let folders = barrel_ryza::chara::motion_sets(&base, &sets, &tag).or_else(|_| barrel_ryza::chara::motion_sets(&base, &sets, &tag[..5]))?;
    let (bytes, set) = folders
        .iter()
        .find_map(|f| install.read(&barrel_ryza::motion::path(&f.dir, motion)).ok().map(|bytes| (bytes, f)))
        .ok_or_else(|| format!("motion {motion} is in none of {:?}", folders.iter().map(|f| &f.dir).collect::<Vec<_>>()))?;
    let c = Elixir::parse(&bytes)?;
    let entry = c.entries.iter().find(|e| !e.is_dummy()).ok_or("empty motion")?;
    let anim = barrel_formats::g1a::parse(c.data(entry))?;
    let hip = skeleton.joint(barrel_ryza::motion::HIP);
    let frames = ((anim.duration * 30.0).ceil() as usize).max(1) + 1;
    eprintln!("motion {motion} from {}: {:.2} s, {frames} keys", set.dir, anim.duration);
    let times: Vec<f32> = (0..frames).map(|f| (f as f32 / 30.0).min(anim.duration)).collect();
    let mut tr = vec![Vec::new(); skeleton.joints.len()];
    let mut ro = vec![Vec::new(); skeleton.joints.len()];
    let mut sc = vec![Vec::new(); skeleton.joints.len()];
    for &t in &times {
        let mut locals = barrel_engine::skin::local_pose(skeleton, &anim, t);
        // A shared set's motion is made for a hip `skeleton_base` high (the game scales the hip's lift to the model's).
        if let (Some(base), Some(h)) = (set.skeleton_base, hip) {
            locals[h].2.y *= skeleton.joints[h].position[1] / base;
        }
        for (j, (s, r, p)) in locals.into_iter().enumerate() {
            tr[j].extend(mirror_point(p).to_array());
            ro[j].extend(mirror_rotation(r).to_array());
            sc[j].extend(s.to_array());
        }
    }
    let input = b.floats(&times, "SCALAR", true, false);
    let (mut samplers, mut channels) = (Vec::new(), Vec::new());
    for j in 0..skeleton.joints.len() {
        for (path, data, kind) in [("translation", &tr[j], "VEC3"), ("rotation", &ro[j], "VEC4"), ("scale", &sc[j], "VEC3")] {
            let output = b.floats(data, kind, false, false);
            samplers.push(json!({"input": input, "output": output, "interpolation": "LINEAR"}));
            channels.push(json!({"sampler": samplers.len() - 1, "target": {"node": j, "path": path}}));
        }
    }
    b.animations.push(json!({"name": format!("motion_{motion:05}"), "samplers": samplers, "channels": channels}));
    Ok(())
}

