//! The 3D side of barrel-parity: a capture frame's draws grouped into passes with their
//! constant-buffer values read through the shader's own reflection (`draws`), and the diff of
//! those draws against Barrel's 3D trace (`diff3d`). Conventions are in docs/parity.md,
//! "Draw parity (3D)".

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use barrel_formats::dxbc::{self, Binding, ConstantBuffer};
use serde_json::Value;

use super::{floats, index, Result, Trace};

/// A shader's constant buffers and bindings, owned (the reflection of one captured shader).
#[derive(Clone, Default)]
struct Reflection {
    buffers: Vec<ConstantBuffer>,
    bindings: Vec<Binding>,
}

/// One constant variable's value: its name, its size in bytes and its floats.
type Var = (String, u32, Vec<f32>);

/// The variables of the constant buffer bound at `slot`, read from `blob` by the shader's
/// reflection.
fn read_vars(r: &Reflection, slot: u32, blob: &[u8]) -> Vec<Var> {
    let Some(bind) = r.bindings.iter().find(|b| b.kind == 0 && b.slot == slot) else { return Vec::new() };
    let Some(cb) = r.buffers.iter().find(|c| c.name == bind.name) else { return Vec::new() };
    cb.variables
        .iter()
        .filter_map(|v| {
            let (a, b) = (v.offset as usize, (v.offset + v.size) as usize);
            Some((v.name.clone(), v.size, floats(blob.get(a..b)?)))
        })
        .collect()
}

/// Whether a variable is shown by default: a float4x4 or float4x3 (64 or 48 bytes), or one
/// named for the view, projection, eye or camera. The reflection reader keeps no type, so a
/// matrix is told by its size.
fn shown_by_default(name: &str, size: u32) -> bool {
    size == 64 || size == 48 || ["View", "Proj", "W2P", "L2W", "Eye", "Cam"].iter().any(|k| name.contains(k))
}

/// A draw of the capture with what the 3D comparison needs.
struct GameDraw {
    n: u64,
    count: u64,
    instances: u64,
    topology: u64,
    vs: String,
    ps: String,
    /// Texture hashes by slot, pixel stage then vertex stage.
    textures: Vec<(String, String)>,
    rt: Vec<String>,
    ds: String,
    vars: Vec<Var>,
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn game_draws(t: &Trace, frame: u64) -> Result<Vec<GameDraw>> {
    let mut cache: HashMap<String, Reflection> = HashMap::new();
    let mut out = Vec::new();
    for e in t.frame(frame)? {
        if e.get("draw").is_none() {
            continue;
        }
        let s = |k: &str| e[k].as_str().unwrap_or("-").to_string();
        let mut vars: Vec<Var> = Vec::new();
        for (cbs, stage) in [("vs_cb", "vs"), ("ps_cb", "ps")] {
            let shader = s(stage);
            if !cache.contains_key(&shader) {
                let r = t.blob(&shader).ok().and_then(|code| dxbc::program(&code).ok().map(|p| Reflection { buffers: p.buffers, bindings: p.bindings })).unwrap_or_default();
                cache.insert(shader.clone(), r);
            }
            for (slot, hash) in e[cbs].as_object().into_iter().flatten() {
                let (Ok(slot), Some(h)) = (slot.parse::<u32>(), hash.as_str()) else { continue };
                if let Ok(blob) = t.blob(h) {
                    vars.extend(read_vars(&cache[&shader], slot, &blob));
                }
            }
        }
        let mut textures = Vec::new();
        for k in ["ps_tex", "vs_tex"] {
            for (slot, h) in e[k].as_object().into_iter().flatten() {
                textures.push((format!("{}{slot}", &k[..2]), h.as_str().unwrap_or("").to_string()));
            }
        }
        out.push(GameDraw {
            n: e["draw"].as_u64().unwrap_or(0),
            count: e["count"].as_u64().unwrap_or(0),
            instances: e["instances"].as_u64().unwrap_or(1),
            topology: e["topology"].as_u64().unwrap_or(0),
            vs: s("vs"),
            ps: s("ps"),
            textures,
            rt: strs(&e["rt"]),
            ds: e["ds"].as_str().unwrap_or("-").to_string(),
            vars,
        });
    }
    Ok(out)
}

/// Runs of consecutive items with the same key: (key, first item, one past the last).
fn passes<K: PartialEq + Clone>(keys: &[K]) -> Vec<(K, usize, usize)> {
    let mut out: Vec<(K, usize, usize)> = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        match out.last_mut() {
            Some(p) if p.0 == *k => p.2 = i + 1,
            _ => out.push((k.clone(), i, i + 1)),
        }
    }
    out
}

fn floats_text(v: &[f32]) -> String {
    let n: Vec<String> = v.iter().map(|f| format!("{f:.4}")).collect();
    format!("[{}]", n.join(" "))
}

/// `barrel-parity draws`: a frame's passes and (unless `summary`) their draws.
pub fn draws(dir: &Path, frame: u64, idx: Option<&str>, vars: Option<&str>, pass: Option<usize>, summary: bool) -> Result<()> {
    let t = Trace { dir: dir.into() };
    let (names, _) = index(idx)?;
    let tex = |h: &str| names.get(h).cloned().unwrap_or_else(|| h.to_string());
    let wanted: Option<Vec<&str>> = vars.map(|v| v.split(',').collect());
    let ds = game_draws(&t, frame)?;
    let keys: Vec<(Vec<String>, String)> = ds.iter().map(|d| (d.rt.clone(), d.ds.clone())).collect();
    for (p, ((rt, depth), a, b)) in passes(&keys).into_iter().enumerate() {
        if pass.is_some_and(|n| n != p) {
            continue;
        }
        println!("pass {p}: {} ds {depth}: {} draws (#{}..#{})", rt.join(","), b - a, ds[a].n, ds[b - 1].n);
        if summary {
            continue;
        }
        for d in &ds[a..b] {
            let tx: Vec<String> = d.textures.iter().map(|(s, h)| format!("{s}={}", tex(h))).collect();
            println!("  #{} count {} x{} topo {} vs {} ps {} tex [{}]", d.n, d.count, d.instances, d.topology, d.vs, d.ps, tx.join(" "));
            for (name, size, v) in &d.vars {
                let on = match &wanted {
                    Some(w) => w.contains(&name.as_str()),
                    None => shown_by_default(name, *size),
                };
                if on {
                    println!("      {name} = {}", floats_text(v));
                }
            }
        }
    }
    Ok(())
}

type M = [f32; 16];

/// The game's mL2W as a column-vector matrix in column-major order. The constant buffer holds
/// three registers, each one output row `(m0 m1 m2 t)`: world.x = row0 . (p, 1), and so on.
fn game_world(l2w: &[f32]) -> Option<M> {
    if l2w.len() < 12 {
        return None;
    }
    let mut m = [0.0; 16];
    for c in 0..4 {
        for r in 0..3 {
            m[c * 4 + r] = l2w[r * 4 + c];
        }
    }
    m[15] = 1.0;
    Some(m)
}

/// The game's mW2P as a column-vector matrix in column-major order. It is used row-vector
/// style (clip = (p, 1) . M), so the registers are M's rows: its memory order is already the
/// column-major order of the column-vector matrix.
fn game_view_proj(w2p: &[f32]) -> Option<M> {
    w2p.get(..16)?.try_into().ok()
}

/// `S * m * S` with S the x mirror Barrel's render space applies.
fn mirrored(m: &M) -> M {
    let mut o = *m;
    for c in 0..4 {
        for r in 0..4 {
            if (c == 0) != (r == 0) {
                o[c * 4 + r] = -o[c * 4 + r];
            }
        }
    }
    o
}

fn max_diff(a: &M, b: &M) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

/// The smaller of the differences to the game's matrix as it is and with Barrel's x mirror
/// applied to it, and whether the mirrored one won.
fn world_diff(game: &M, barrel: &M) -> (f32, bool) {
    let (a, b) = (max_diff(game, barrel), max_diff(&mirrored(game), barrel));
    if b < a { (b, true) } else { (a, false) }
}

struct BarrelDraw {
    pass: String,
    idx: u64,
    tex: Vec<String>,
    world: M,
}

fn barrel_frame(path: &Path, frame: u64) -> Result<(Vec<BarrelDraw>, Option<M>)> {
    let mut found = None;
    for l in std::fs::read_to_string(path)?.lines() {
        let v: Value = serde_json::from_str(l)?;
        if v.get("draws3d").is_some() && v["frame"].as_u64() == Some(frame) {
            found = Some(v);
        }
    }
    let v = found.ok_or_else(|| format!("no 3D draws for Barrel frame {frame}"))?;
    let m = |x: &Value| -> Option<M> {
        let a = x.as_array()?;
        let mut o = [0.0; 16];
        if a.len() != 16 {
            return None;
        }
        for (i, f) in a.iter().enumerate() {
            o[i] = f.as_f64()? as f32;
        }
        Some(o)
    };
    let draws = v["draws3d"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| BarrelDraw { pass: d["pass"].as_str().unwrap_or("").to_string(), idx: d["idx"].as_u64().unwrap_or(0), tex: strs(&d["tex"]), world: m(&d["world"]).unwrap_or([0.0; 16]) })
        .collect();
    Ok((draws, m(&v["view_proj"])))
}

/// What matches two draws: the textures the index names (sorted, so slot order does not
/// matter) and the index count.
type Key = (Vec<String>, u64);

fn key(names: &HashMap<String, String>, textures: impl Iterator<Item = String>, count: u64) -> Key {
    let mut n: Vec<String> = textures.filter_map(|h| names.get(&h).cloned()).collect();
    n.sort();
    n.dedup();
    (n, count)
}

/// The folder of a texture's archive name, for grouping what has no match.
fn folder(name: &str) -> String {
    let n = name.rsplit_once('\\').map_or(name, |p| p.0);
    n.to_string()
}

fn group(keys: &[&Key]) -> String {
    let mut g: BTreeMap<String, usize> = BTreeMap::new();
    for k in keys {
        let f = k.0.first().map_or_else(|| "(no named texture)".to_string(), |n| folder(n));
        *g.entry(f).or_default() += 1;
    }
    g.iter().map(|(f, n)| format!("    {n:5} {f}")).collect::<Vec<_>>().join("\n")
}

/// `barrel-parity diff3d`. Game draws are the ones into a G-buffer (a depth target and four or
/// more colour targets); Barrel's are its `scene` and `sky` pass draws.
pub fn diff3d(dir: &Path, gframe: u64, barrel: &Path, bframe: u64, idx: Option<&str>) -> Result<()> {
    let t = Trace { dir: dir.into() };
    let (names, _) = index(idx)?;
    let game: Vec<GameDraw> = game_draws(&t, gframe)?.into_iter().filter(|d| d.rt.len() >= 4 && d.ds != "-").collect();
    let (bdraws, bvp) = barrel_frame(barrel, bframe)?;
    let bdraws: Vec<&BarrelDraw> = bdraws.iter().filter(|d| d.pass == "scene" || d.pass == "sky").collect();
    let gkeys: Vec<Key> = game.iter().map(|d| key(&names, d.textures.iter().map(|x| x.1.clone()), d.count)).collect();
    let bkeys: Vec<Key> = bdraws.iter().map(|d| key(&names, d.tex.iter().cloned(), d.idx)).collect();
    let mut bused = vec![false; bdraws.len()];
    let (mut matched, mut worst, mut compared) = (0usize, 0f32, 0usize);
    let mut worst_at = String::new();
    let mut mirror_wins = 0usize;
    let mut unmatched_game: Vec<&Key> = Vec::new();
    for (i, g) in game.iter().enumerate() {
        let world = g.vars.iter().find(|v| v.0 == "mL2W").and_then(|v| game_world(&v.2));
        // The unused Barrel draw of the same key nearest in world matrix (the first without one).
        let mut best: Option<(usize, f32, bool)> = None;
        for (j, b) in bdraws.iter().enumerate() {
            if bused[j] || bkeys[j] != gkeys[i] {
                continue;
            }
            let (d, m) = world.map_or((0.0, false), |w| world_diff(&w, &b.world));
            if best.is_none_or(|x| d < x.1) {
                best = Some((j, d, m));
            }
        }
        let Some((j, d, m)) = best else {
            unmatched_game.push(&gkeys[i]);
            continue;
        };
        bused[j] = true;
        matched += 1;
        if world.is_some() {
            compared += 1;
            mirror_wins += usize::from(m);
            if d > worst {
                worst = d;
                worst_at = format!("game draw #{} ({} indices) vs Barrel draw {j}", g.n, g.count);
            }
        }
    }
    let unmatched_barrel: Vec<&Key> = bkeys.iter().zip(&bused).filter(|(_, u)| !**u).map(|(k, _)| k).collect();
    println!("game draws without a Barrel match: {}\n{}", unmatched_game.len(), group(&unmatched_game));
    println!("Barrel draws without a game match: {}\n{}", unmatched_barrel.len(), group(&unmatched_barrel));
    println!("world matrices compared: {compared} ({mirror_wins} fit best with Barrel's x mirror); worst: {worst_at}");
    let vp = game.iter().flat_map(|d| d.vars.iter()).find(|v| v.0 == "mW2P").and_then(|v| game_view_proj(&v.2));
    if let (Some(g), Some(b)) = (vp, bvp) {
        println!("view-projection (first game mW2P vs Barrel's): max difference {:.4}", world_diff(&g, &b).0);
    }
    println!("{} game draws, {} Barrel draws, {} matched, max world diff {:.4}", game.len(), bdraws.len(), matched, worst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::dxbc::Variable;

    fn reflection() -> Reflection {
        Reflection {
            buffers: vec![ConstantBuffer {
                name: "$Globals".into(),
                size: 112,
                variables: vec![Variable { name: "mW2P".into(), offset: 0, size: 64 }, Variable { name: "mL2W".into(), offset: 64, size: 48 }],
            }],
            bindings: vec![Binding { name: "$Globals".into(), kind: 0, slot: 0, count: 1 }],
        }
    }

    #[test]
    fn variables_are_read_by_offset() {
        let blob: Vec<u8> = (0..28).flat_map(|i| (i as f32).to_le_bytes()).collect();
        let v = read_vars(&reflection(), 0, &blob);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, "mW2P");
        assert_eq!(v[0].2[15], 15.0);
        assert_eq!(v[1].0, "mL2W");
        assert_eq!(v[1].2, (16..28).map(|i| i as f32).collect::<Vec<_>>());
        // A slot nothing binds, and a blob too short for a variable
        assert!(read_vars(&reflection(), 3, &blob).is_empty());
        assert_eq!(read_vars(&reflection(), 0, &blob[..80]).len(), 1);
    }

    #[test]
    fn matrices_and_named_variables_show_by_default() {
        assert!(shown_by_default("mL2W", 48));
        assert!(shown_by_default("g_vEyePosition", 16));
        assert!(!shown_by_default("g_fPhase", 4));
        assert!(!shown_by_default("g_mBones", 1536));
    }

    #[test]
    fn passes_are_runs_of_one_target_set() {
        let k = ["a", "a", "b", "a"];
        assert_eq!(passes(&k), vec![("a", 0, 2), ("b", 2, 3), ("a", 3, 4)]);
        assert!(passes::<u8>(&[]).is_empty());
    }

    #[test]
    fn world_registers_are_rows_with_the_translation_last() {
        // rows (m0 m1 m2 t) of a scale of 2 moved by (10, 20, 30)
        let l2w = [2.0, 0.0, 0.0, 10.0, 0.0, 2.0, 0.0, 20.0, 0.0, 0.0, 2.0, 30.0];
        let m = game_world(&l2w).unwrap();
        assert_eq!(&m[12..], &[10.0, 20.0, 30.0, 1.0]);
        assert_eq!(m[0], 2.0);
        // Barrel's x mirror flips the x translation and the x axis' y and z parts
        let r = mirrored(&m);
        assert_eq!(&r[12..], &[-10.0, 20.0, 30.0, 1.0]);
        assert_eq!(r[0], 2.0);
        assert_eq!(world_diff(&m, &r), (0.0, true));
    }

    #[test]
    fn mirror_leaves_symmetric_matrices_alone() {
        let id = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        assert_eq!(mirrored(&id), id);
        assert_eq!(world_diff(&id, &id), (0.0, false));
    }
}
