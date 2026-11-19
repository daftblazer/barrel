//! `barrel-parity constants`: the lighting and post-processing constants of a captured frame
//! against what Barrel uses for the same view (`BARREL_TRACE_CONSTANTS`), constant by constant.
//! Conventions are in docs/parity.md, "Constants parity".

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use super::draws3d::{game_draws, passes, GameDraw};
use super::{Result, Trace};

/// Where each role of Barrel's dump sits in the game's frame (the Rasenboden noon capture's
/// passes, counted as runs of one target set, and the draw within the pass; docs/shading.md,
/// "Ryza DX: the frame as the game draws it").
const ROLES: &[(&str, usize, usize)] = &[
    ("occlusion", 4, 0),
    ("ambient.stage", 11, 0),
    ("ambient.second", 11, 1),
    ("ambient.chara", 11, 2),
    ("lighting.stage", 12, 0),
    ("lighting.stage1", 12, 1),
    ("lighting.stage2", 12, 2),
    ("lighting.chara", 12, 3),
    ("lighting.water", 15, 0),
    ("sky.plane", 17, 0),
    ("sky.dome0", 17, 1),
    ("sky.dome1", 17, 2),
    ("sky.clouds", 17, 3),
    ("sky.clouds2", 17, 4),
    ("fog.stage", 18, 0),
    ("fog.sky", 18, 1),
    ("specular.stage", 22, 0),
    ("specular.chara", 22, 1),
    ("forward.chara", 22, 59),
    ("water.first", 13, 0),
    ("water.last", 13, 9),
    ("meter.log", 38, 0),
    ("meter.mean", 41, 0),
    ("meter.adapt", 42, 0),
    ("bloom.bright", 43, 0),
    ("dof.31", 31, 0),
    ("dof.32", 32, 0),
    ("dof.33", 33, 0),
    ("dof.34", 34, 0),
    ("dof.35", 35, 0),
    ("dof.36", 36, 0),
    ("dof.37", 37, 0),
    ("star.51", 51, 0),
    ("star.54", 54, 0),
    ("star.57", 57, 0),
    ("star.60", 60, 0),
    ("shaft.63", 63, 0),
    ("shaft.65", 65, 0),
    ("shaft.68", 68, 0),
    ("tone", 69, 0),
];

/// The lighting and post passes: from the ambient-and-shadow pass to the last pass before the
/// interface, and in the forward pass only the specular ambient's two draws.
const FIRST_PASS: usize = 11;
const LAST_PASS: usize = 76;
const FORWARD_PASS: usize = 22;
const FORWARD_KEPT: usize = 2;

/// Constants that are neither lighting nor post: the camera's matrices, and the 2D passes'
/// viewport plumbing.
const SKIPPED: &[&str] = &["mV2W", "mP2W", "mW2P", "mWV", "mRrW2T", "mL2W", "ktglViewport", "ktglColorScale", "vATest", "g_vPosOffset", "g_vTexParam", "g_vUVOffsetSampling"];

fn same(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5 + 1e-3 * a.abs().max(b.abs())
}

/// `3.4560e-1`-free text of a float.
fn text(v: f32) -> String {
    format!("{v}")
}

pub fn constants(dir: &Path, frame: u64, barrel: &Path) -> Result<()> {
    let t = Trace { dir: dir.into() };
    let ds = game_draws(&t, frame)?;
    let keys: Vec<(Vec<String>, String)> = ds.iter().map(|d| (d.rt.clone(), d.ds.clone())).collect();
    let runs = passes(&keys);
    let draw_at = |pass: usize, draw: usize| -> Option<&GameDraw> {
        let (_, a, b) = runs.get(pass)?;
        (a + draw < *b).then(|| &ds[a + draw])
    };
    let dump: Value = serde_json::from_slice(&std::fs::read(barrel)?)?;
    let dump = dump.as_object().ok_or("the constants file is not an object")?;

    let (mut compared, mut differ) = (0usize, 0usize);
    // (pass, draw) -> the names Barrel dumped for it.
    let mut dumped: BTreeMap<(usize, usize), BTreeSet<String>> = BTreeMap::new();
    for (role, consts) in dump {
        let Some(&(_, pass, draw)) = ROLES.iter().find(|r| r.0 == role) else {
            println!("role {role}: not one the tool knows");
            continue;
        };
        let Some(g) = draw_at(pass, draw) else {
            println!("role {role}: the capture has no pass {pass} draw {draw}");
            continue;
        };
        let program = g.ps.trim_start_matches("sh").chars().take(8).collect::<String>();
        for (name, value) in consts.as_object().into_iter().flatten() {
            let mine: Vec<Option<f32>> = value.as_array().into_iter().flatten().map(|v| v.as_f64().map(|f| f as f32)).collect();
            if mine.iter().all(Option::is_none) {
                continue;
            }
            dumped.entry((pass, draw)).or_default().insert(name.clone());
            // The game's copy of the constant (a name can sit in both stages' buffers): the one
            // that agrees best.
            let copies: Vec<&Vec<f32>> = g.vars.iter().filter(|v| &v.0 == name).map(|v| &v.2).collect();
            let off = |c: &Vec<f32>| mine.iter().enumerate().filter(|(i, m)| m.is_some_and(|m| c.get(*i).is_none_or(|&x| !same(x, m)))).count();
            let Some(best) = copies.iter().min_by_key(|c| off(c)) else {
                println!("pass {pass} draw {draw} {program} {name}: the game's draw has no such constant");
                continue;
            };
            for (i, m) in mine.iter().enumerate() {
                let Some(m) = *m else { continue };
                compared += 1;
                match best.get(i) {
                    Some(&x) if same(x, m) => {}
                    Some(&x) => {
                        differ += 1;
                        println!("pass {pass} draw {draw} {program} {name}[{i}]: game {} barrel {}", text(x), text(m));
                    }
                    None => {
                        differ += 1;
                        println!("pass {pass} draw {draw} {program} {name}[{i}]: game has {} components, barrel {}", best.len(), mine.len());
                    }
                }
            }
        }
    }

    // What the game's lighting and post draws have that Barrel's dump does not name.
    let mut missing: BTreeMap<(usize, String), BTreeSet<usize>> = BTreeMap::new();
    let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
    for (p, (_, a, b)) in runs.iter().enumerate() {
        if !(FIRST_PASS..=LAST_PASS).contains(&p) {
            continue;
        }
        sizes.insert(p, b - a);
        for (d, g) in ds[*a..*b].iter().enumerate() {
            if p == FORWARD_PASS && d >= FORWARD_KEPT {
                continue;
            }
            for (name, ..) in &g.vars {
                if SKIPPED.contains(&name.as_str()) || dumped.get(&(p, d)).is_some_and(|n| n.contains(name)) {
                    continue;
                }
                missing.entry((p, name.clone())).or_default().insert(d);
            }
        }
    }
    println!();
    println!("not dumped (pass, draws it holds in, names; a draw count only where some draws lack it):");
    let mut by_pass: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for ((p, name), draws) in &missing {
        let all = sizes[p].min(if *p == FORWARD_PASS { FORWARD_KEPT } else { usize::MAX });
        let label = if draws.len() == all { name.clone() } else { format!("{name}[{}]", draws.iter().map(usize::to_string).collect::<Vec<_>>().join(",")) };
        by_pass.entry(*p).or_default().push(label);
    }
    for (p, names) in &by_pass {
        println!("  pass {p} ({} draws): {}", sizes[p], names.join(" "));
    }
    let every: BTreeSet<&str> = missing.keys().map(|(_, n)| n.as_str()).collect();
    println!("{compared} constants compared, {differ} differ, {} of the game's lighting/post constants not dumped ({} names: {})", missing.len(), every.len(), every.into_iter().collect::<Vec<_>>().join(" "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::same;

    #[test]
    fn values_agree_within_a_thousandth_or_near_zero() {
        assert!(same(14.343661, 14.35));
        assert!(!same(14.343661, 14.4));
        // near zero the absolute 1e-5 holds
        assert!(same(0.0, 5e-6));
        assert!(!same(0.0, 5e-5));
        assert!(same(-1.0576435, -1.0576));
    }

    #[test]
    fn every_role_has_one_place() {
        let mut names: Vec<&str> = super::ROLES.iter().map(|r| r.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), super::ROLES.len());
    }
}
