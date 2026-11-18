//! The solver against the answer key (`BARREL_CLOTH_KEY`: Ryza's skirt captured from the game,
//! three frames of every stage's state) and the model it comes from (`BARREL_CLOTH_MODEL`).
//! Both are game data: without them the tests pass with a note.

use super::*;
use barrel_formats::g1m::G1m;
use std::collections::HashMap;

struct Frame {
    dt: f32,
    parent_old: DMat4,
    parent_new: DMat4,
    wind: Vec3,
    env: Vec<[f32; 4]>,
    colliders: Vec<LocalCollider>,
    /// Stage name -> P, R, V, S.
    states: HashMap<String, [Vec<Vec3>; 4]>,
}

struct Key {
    dists: Vec<Vec<String>>,
    bends: Vec<Vec<String>>,
    tris: Vec<Vec<String>>,
    limits: Vec<Vec<String>>,
    scalars: HashMap<String, Vec<f32>>,
    depth: Vec<u32>,
    frames: Vec<Frame>,
}

fn floats(t: &[&str]) -> Vec<f32> {
    t.iter().map(|x| x.parse().unwrap()).collect()
}

/// A row-vector matrix (the key's, 16 floats) as a column-vector one.
fn matrix(f: &[f32]) -> Mat4 {
    Mat4::from_cols_array(f.try_into().unwrap())
}

fn load_key(path: &str) -> Key {
    let text = std::fs::read_to_string(path).unwrap();
    let mut key = Key { dists: vec![], bends: vec![], tris: vec![], limits: vec![], scalars: HashMap::new(), depth: vec![], frames: vec![] };
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            None | Some("#") => {}
            Some("dist") => key.dists.push(t[1..].iter().map(|s| s.to_string()).collect()),
            Some("bend") => key.bends.push(t[1..].iter().map(|s| s.to_string()).collect()),
            Some("tri") => key.tris.push(t[1..].iter().map(|s| s.to_string()).collect()),
            Some("limit") => key.limits.push(t[1..].iter().map(|s| s.to_string()).collect()),
            Some("depth") => key.depth = t[1..].iter().map(|s| s.parse().unwrap()).collect(),
            Some("frame") => key.frames.push(Frame {
                dt: 0.0,
                parent_old: DMat4::IDENTITY,
                parent_new: DMat4::IDENTITY,
                wind: Vec3::ZERO,
                env: vec![],
                colliders: vec![],
                states: HashMap::new(),
            }),
            Some(name) if key.frames.is_empty() => {
                key.scalars.insert(name.to_string(), floats(&t[1..]));
            }
            Some(name) => {
                let f = key.frames.last_mut().unwrap();
                match name {
                    "dt" => f.dt = t[1].parse().unwrap(),
                    "parent_old" => f.parent_old = matrix(&floats(&t[1..])).as_dmat4(),
                    "parent_new" => f.parent_new = matrix(&floats(&t[1..])).as_dmat4(),
                    "wind_world" => f.wind = Vec3::from_slice(&floats(&t[1..])),
                    "env" => f.env = floats(&t[1..]).chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
                    "collider" => {
                        let v = floats(&t[1..]);
                        // type mask flags r h r local16 inverse16 (the first three are integers)
                        let shape: u32 = t[1].parse().unwrap();
                        let (radius, half) = (v[3], v[4]);
                        f.colliders.push(LocalCollider { shape, radius, half, local: matrix(&v[6..22]), inverse: matrix(&v[22..38]) });
                    }
                    "state" => {
                        let (stage, which) = (t[1].to_string(), t[2]);
                        let pts: Vec<Vec3> = floats(&t[3..]).chunks(4).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
                        let slot = f.states.entry(stage).or_insert_with(|| [vec![], vec![], vec![], vec![]]);
                        slot["PRVS".find(which).unwrap()] = pts;
                    }
                    "bbox" => {}
                    other => panic!("key line {other}"),
                }
            }
        }
    }
    key
}

/// The key, its model's piece and its colliders, or None (with a note) when not set.
fn setup() -> Option<(Key, ClothSim, Cloth)> {
    let (Ok(key), Ok(model)) = (std::env::var("BARREL_CLOTH_KEY"), std::env::var("BARREL_CLOTH_MODEL")) else {
        eprintln!("cloth tests skipped: BARREL_CLOTH_KEY and BARREL_CLOTH_MODEL not set");
        return None;
    };
    let bytes = std::fs::read(model).unwrap();
    let g1m = G1m::parse(&bytes).unwrap();
    let g = g1m.geometry.as_ref().unwrap();
    let cloth = g.cloths[0].clone();
    let colliders: Vec<Collider> = cloth.collider_groups.iter().flat_map(|&i| g.collider_groups[i as usize].iter().copied()).collect();
    let key = load_key(&key);
    let local_gravity = key.scalars["local_gravity"][0];
    let sim = ClothSim::new(&cloth, &colliders, local_gravity);
    Some((key, sim, cloth))
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol * (1.0 + b.abs())
}

#[test]
fn tables_equal_the_keys() {
    let Some((key, sim, _)) = setup() else { return };
    let f = |s: &str| s.parse::<f32>().unwrap();
    let u = |s: &str| s.parse::<u32>().unwrap();
    assert_eq!(sim.dists.len(), key.dists.len());
    for (i, (d, k)) in sim.dists.iter().zip(&key.dists).enumerate() {
        assert_eq!((d.a, d.b, d.flags as u32, d.kind as u32), (u(&k[0]), u(&k[1]), u(&k[5]), u(&k[6])), "dist {i}");
        for (x, y) in [(d.wa, f(&k[2])), (d.wb, f(&k[3])), (d.rest, f(&k[4]))] {
            assert!(close(x, y, 1e-6), "dist {i}: {x} vs {y}");
        }
    }
    assert_eq!(sim.bends.len(), key.bends.len());
    for (i, (b, k)) in sim.bends.iter().zip(&key.bends).enumerate() {
        assert_eq!((b.a, b.b, b.c, b.flags as u32), (u(&k[0]), u(&k[1]), u(&k[2]), u(&k[3])), "bend {i}");
        for (x, y) in [(b.wa, f(&k[4])), (b.wb, f(&k[5])), (b.wc, f(&k[6])), (b.rest2, f(&k[7]))] {
            assert!(close(x, y, 1e-6), "bend {i}: {x} vs {y}");
        }
    }
    assert_eq!(sim.tris.len(), key.tris.len());
    for (i, (t, k)) in sim.tris.iter().zip(&key.tris).enumerate() {
        assert_eq!((t.a, t.b, t.c, t.d, t.flags as u32, t.second as u32), (u(&k[0]), u(&k[1]), u(&k[2]), u(&k[3]), u(&k[4]), u(&k[5])), "tri {i}");
    }
    assert_eq!(sim.limits.len(), key.limits.len());
    for (i, (l, k)) in sim.limits.iter().zip(&key.limits).enumerate() {
        assert_eq!((l.point, l.anchor), (u(&k[0]), u(&k[1])), "limit {i}");
        assert!(close(l.max2, f(&k[2]), 1e-6), "limit {i}: {} vs {}", l.max2, k[2]);
    }
    assert_eq!(sim.depth, key.depth);
    assert_eq!(sim.blend, key.scalars["blend"][0] as u32);
    assert!(close(sim.step_limit, key.scalars["step_limit"][0], 1e-6), "{} vs {}", sim.step_limit, key.scalars["step_limit"][0]);
    assert_eq!(sim.gravity_world.to_array().map(|x| x + 0.0), key.scalars["gravity_world"].iter().map(|x| x + 0.0).collect::<Vec<_>>()[..]);
    assert_eq!(sim.gravity_local.to_array().map(|x| x + 0.0), key.scalars["gravity_local"].iter().map(|x| x + 0.0).collect::<Vec<_>>()[..]);
    assert_eq!(sim.settings.spin_factor, key.scalars["spin_factor"][0]);
}

fn max_err(a: &[Vec3], b: &[Vec3]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (*x - *y).abs().max_element()).fold(0.0, f32::max)
}

fn env_of(f: &Frame) -> Env {
    let row = |i: usize| Vec3::new(f.env[i][0], f.env[i][1], f.env[i][2]);
    Env { velocity: row(0), accel: Vec3::ZERO, spin: row(2), gravity: row(4), wind: row(5) }
}

/// Load a key state into the solver.
fn load_state(sim: &mut ClothSim, f: &Frame, stage: &str) {
    let [p, r, v, s] = &f.states[stage];
    sim.p.clone_from(p);
    sim.r.clone_from(r);
    sim.v.clone_from(v);
    sim.s.clone_from(s);
}

#[test]
fn each_stage_follows_the_previous_key_state() {
    let Some((key, mut sim, _)) = setup() else { return };
    for f in &key.frames {
        let check = |sim: &ClothSim, stage: &str, tol: f32| {
            let e = max_err(&sim.p, &f.states[stage][0]);
            eprintln!("stage {stage}: P error {e}");
            assert!(e <= tol, "stage {stage}: P error {e}");
        };
        // Parent re-expression and normals. The game's float32 product of the two parent
        // matrices carries ~0.004 of noise at world coordinates near 32,000 (the solver's is exact).
        load_state(&mut sim, f, "start");
        let d = (f.parent_new.inverse() * f.parent_old).as_mat4();
        sim.reexpress(d);
        sim.normals();
        check(&sim, "prepared", 0.01);
        let normals = max_err(&sim.s, &f.states["prepared"][3]);
        assert!(normals < 0.02, "normals {normals}");

        load_state(&mut sim, f, "prepared");
        sim.integrate(&env_of(f), f.dt, false);
        check(&sim, "integrated", 1e-4);
        let v = max_err(&sim.v, &f.states["integrated"][2]);
        assert!(v < 1e-3, "integrated V {v}");

        let mut broad = None;
        let stages = [("integrated", "bent0"), ("bent0", "stretched0"), ("stretched0", "collided0"), ("collided0", "bent1"), ("bent1", "stretched1"), ("stretched1", "collided1")];
        for (i, (from, to)) in stages.iter().enumerate() {
            load_state(&mut sim, f, from);
            match i % 3 {
                0 => sim.bend(),
                1 => sim.dist(),
                _ => sim.collide(&f.colliders, i == 2, &mut broad),
            }
            check(&sim, to, 1e-4);
        }
        load_state(&mut sim, f, "collided1");
        sim.limit();
        check(&sim, "limited", 1e-4);

        load_state(&mut sim, f, "limited");
        sim.velocity(f.dt);
        check(&sim, "velocity", 1e-4);
        let v = max_err(&sim.v, &f.states["velocity"][2]);
        assert!(v < 1e-2, "velocity V {v}");
    }
}

#[test]
fn environment_from_the_parents_matrices() {
    let Some((key, mut sim, _)) = setup() else { return };
    for f in &key.frames {
        let (env, _, _) = sim.parent_motion(f.dt, f.parent_old, f.parent_new, f.wind);
        let want = env_of(f);
        for (name, a, b) in [("velocity", env.velocity, want.velocity), ("spin", env.spin, want.spin), ("gravity", env.gravity, want.gravity), ("wind", env.wind, want.wind)] {
            let e = (a - b).abs().max_element();
            assert!(e < 1e-3 * (1.0 + b.abs().max_element()), "{name}: {a} vs {b}");
        }
    }
}

#[test]
fn whole_frames_from_the_pins_moved() {
    let Some((key, mut sim, _)) = setup() else { return };
    for f in &key.frames {
        load_state(&mut sim, f, "start");
        sim.parent = Some(f.parent_old);
        sim.velocity_state = DVec3::ZERO;
        sim.spin_state = DVec3::ZERO;
        let (env, d, jump) = sim.parent_motion(f.dt, f.parent_old, f.parent_new, f.wind);
        assert!(!jump);
        sim.reexpress(d);
        sim.normals();
        sim.run(f.dt, &env, jump, &f.colliders);
        let e = max_err(&sim.p, &f.states["end"][0]);
        let v = max_err(&sim.v, &f.states["end"][2]);
        eprintln!("whole frame: end P error {e}, V error {v}");
        assert!(e <= 0.01, "end P error {e}");
        assert!(v <= 0.6, "end V error {v}");
    }
}
