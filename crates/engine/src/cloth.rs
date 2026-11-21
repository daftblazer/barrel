//! Cloth pieces (Ryza DX's `NUNO` chunk): a grid of control points the game moves with a small
//! constraint solver, which the cloth submeshes' vertices hang between. Everything runs in the
//! parent joint's space; positions are `Vec3`, matrices `glam`'s (column vectors: a game's
//! row-vector matrix is the transpose, `DMat4::from_cols_array_2d(&rows)`). One `step` is one
//! 1/60 s of game time. The method is documented in `docs/formats.md` ("Cloth pieces").

use barrel_formats::g1m::{Cloth, ClothSettings, Collider};
use glam::{DMat4, DVec3, Mat4, Vec3};

#[cfg(test)]
mod tests;

/// The game's float epsilon for "length is zero" tests.
const EPS: f32 = 1.19209e-05;

/// A distance constraint between two points.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dist {
    a: u32,
    b: u32,
    /// How far each end moves per unit of error (-0.5 / 0.5 for a movable end, 0 for a pinned one).
    wa: f32,
    wb: f32,
    rest: f32,
    /// Bit 0: `a` moves, bit 1: `b` moves.
    flags: u8,
    /// 0 horizontal, 1 shear, 2 vertical.
    kind: u8,
}

/// A bend constraint: the third point is the centre, pulled toward the mean of the three.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bend {
    a: u32,
    b: u32,
    c: u32,
    /// Bits 0..2: `a`, `b`, `c` move; bit 3: the centre is pinned.
    flags: u8,
    wa: f32,
    wb: f32,
    wc: f32,
    /// Squared distance of the centre from the mean at rest.
    rest2: f32,
}

/// A triangle the capsules' surface test pushes through: three corners, the pinned point up
/// the third corner's column, which corners move.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tri {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    flags: u8,
    second: bool,
}

/// A point kept within a distance of an anchor (the squared distance is kept).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Limit {
    point: u32,
    anchor: u32,
    max2: f32,
}

/// A collider's place for one step: in the parent's space and the way back.
#[derive(Clone, Copy, Debug)]
pub struct LocalCollider {
    /// 5 a capsule along y, 6 a sphere.
    pub shape: u32,
    /// Radius and (capsule) half length.
    pub radius: f32,
    pub half: f32,
    pub local: Mat4,
    pub inverse: Mat4,
}

/// What the parent's move and the world do to the piece, in the parent's (new) space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Env {
    pub velocity: Vec3,
    /// The parent's acceleration, as far as a jump is damped (see [`ClothSim::step`]).
    pub accel: Vec3,
    pub spin: Vec3,
    pub gravity: Vec3,
    pub wind: Vec3,
}

/// The broad phase of the collision stage, made once per step: a 10x10x10 grid over the
/// points and the cells each collider and each movable point occupy.
struct Broad {
    grid: Grid,
    point: Vec<u32>,
    any: u32,
    masks: Vec<u32>,
}

struct Grid {
    lo: Vec3,
    hi: Vec3,
    inv: Vec3,
}

fn cell(v: f32, lo: f32, inv: f32) -> u32 {
    let t = (v - lo) * inv;
    if t.is_finite() { (t.trunc() as i32).clamp(0, 9) as u32 } else { 0 }
}

impl Grid {
    fn new(points: &[Vec3]) -> Grid {
        let mn = points.iter().fold(Vec3::splat(f32::MAX), |a, &p| a.min(p));
        let mx = points.iter().fold(Vec3::splat(f32::MIN), |a, &p| a.max(p));
        let size = (mx - mn) * 1.1;
        let centre = (mx + mn) * 0.5;
        Grid { lo: centre - size * 0.5, hi: size * 0.5 + centre, inv: Vec3::ONE / (size * 0.1) }
    }

    fn point(&self, p: Vec3) -> u32 {
        (1 << cell(p.x, self.lo.x, self.inv.x)) | (1 << (10 + cell(p.y, self.lo.y, self.inv.y))) | (1 << (20 + cell(p.z, self.lo.z, self.inv.z)))
    }

    /// The cells a collider's bounding box covers, per axis.
    fn collider(&self, c: &LocalCollider) -> u32 {
        let r = c.radius;
        let e = if c.shape == 5 { Vec3::new(r, c.half + r, r) } else { Vec3::splat(r) };
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..8 {
            let corner = Vec3::new(if k & 1 == 0 { e.x } else { -e.x }, if k & 2 == 0 { e.y } else { -e.y }, if k & 4 == 0 { e.z } else { -e.z });
            let w = c.local.transform_vector3(corner);
            lo = lo.min(w);
            hi = hi.max(w);
        }
        let t = c.local.w_axis.truncate();
        let (lo, hi) = (lo + t, hi + t);
        let mut mask = 0;
        for a in 0..3 {
            if lo[a] > self.hi[a] || self.lo[a] > hi[a] {
                continue;
            }
            for k in cell(lo[a], self.lo[a], self.inv[a])..=cell(hi[a], self.lo[a], self.inv[a]) {
                mask |= 1 << (10 * a as u32 + k);
            }
        }
        mask
    }
}

/// A point `q` in a capsule's space against the capsule: the distance along its axis past the
/// end and the squared distance from the axis (None when outside).
fn capsule(c: &LocalCollider, q: Vec3) -> Option<(f32, f32)> {
    let (r, h) = (c.radius, c.half);
    if q.y.abs() > h + r {
        return None;
    }
    let mut xz = q.x * q.x + q.z * q.z;
    let mut e = 0.0;
    if -h > q.y {
        e = q.y + h;
        xz += e * e;
    } else if q.y > h {
        e = q.y - h;
        xz += e * e;
    }
    (r * r > xz).then_some((e, xz))
}

/// The unit vector of `(x, e, z)` (`l2` its squared length), +x when it is empty.
fn unit(x: f32, e: f32, z: f32, l2: f32) -> Vec3 {
    if x.abs() > EPS || e.abs() > EPS || z.abs() > EPS {
        let s = 1.0 / l2.sqrt();
        Vec3::new(s * x, s * e, s * z)
    } else {
        Vec3::X
    }
}

fn len2(v: Vec3) -> f32 {
    v.y * v.y + v.x * v.x + v.z * v.z
}

/// A cloth piece's solver and state.
pub struct ClothSim {
    settings: ClothSettings,
    movable: Vec<bool>,
    /// Rows from the pinned points (0 for them), the weight of the parent's motion on a point.
    depth: Vec<u32>,
    blend: u32,
    links: Vec<[Option<u32>; 4]>,
    dists: Vec<Dist>,
    bends: Vec<Bend>,
    tris: Vec<Tri>,
    limits: Vec<Limit>,
    /// The longest a point moves in a step: the mean link length x 1.2.
    step_limit: f32,
    gravity_world: Vec3,
    gravity_local: Vec3,
    /// How much of the local gravity the world gravity is blended toward (`charabase`'s `localGravity`).
    local_gravity: f32,
    /// Half the diagonal of the piece at rest: what a parent's jump is measured against.
    extent: f32,
    shapes: Vec<(u32, f32, f32)>,
    /// The colliders as the last step had them, in the parent's space (for tracing).
    placed: Vec<LocalCollider>,
    rest: Vec<Vec3>,
    p: Vec<Vec3>,
    r: Vec<Vec3>,
    v: Vec<Vec3>,
    s: Vec<Vec3>,
    parent: Option<DMat4>,
    /// The parent's smoothed velocity and spin (world), what its acceleration is taken against.
    velocity_state: DVec3,
    spin_state: DVec3,
}

impl ClothSim {
    /// A solver for a piece whose colliders (its groups' records in order) are `colliders`.
    pub fn new(cloth: &Cloth, colliders: &[Collider], local_gravity: f32) -> ClothSim {
        let n = cloth.points.len();
        let rest: Vec<Vec3> = cloth.parent_points().iter().map(|&p| Vec3::from(p)).collect();
        let mut movable = vec![false; n];
        for &(first, last) in &cloth.ranges {
            for m in movable.iter_mut().take(last as usize + 1).skip(first as usize) {
                *m = true;
            }
        }
        let links: Vec<[Option<u32>; 4]> = cloth.links.iter().map(|l| [l.left, l.right, l.up, l.down]).collect();
        let at = |p: u32| links.get(p as usize).copied().unwrap_or([None; 4]);
        let (left, right, up, down) = (|p: u32| at(p)[0], |p: u32| at(p)[1], |p: u32| at(p)[2], |p: u32| at(p)[3]);
        let mv = |p: u32| movable.get(p as usize).copied().unwrap_or(false);

        // The depth: breadth first from the pinned points over the links.
        let mut depth = vec![u32::MAX; n];
        let mut queue = std::collections::VecDeque::new();
        for (i, d) in depth.iter_mut().enumerate() {
            if !movable[i] {
                *d = 0;
                queue.push_back(i as u32);
            }
        }
        while let Some(p) = queue.pop_front() {
            for q in at(p).into_iter().flatten() {
                if depth[q as usize] == u32::MAX {
                    depth[q as usize] = depth[p as usize] + 1;
                    queue.push_back(q);
                }
            }
        }
        for d in &mut depth {
            if *d == u32::MAX {
                *d = 1;
            }
        }
        let blend = (depth.iter().copied().max().unwrap_or(0) + 2) / 2;

        // The distance constraints: for each point its right, its up's right (the shear) and its down.
        let mut dists = Vec::new();
        let weights = |a: u32, b: u32| {
            let (ma, mb) = (mv(a), mv(b));
            (if ma { -0.5 } else { -0.0 }, if mb { 0.5 } else { 0.0 }, ma as u8 | (mb as u8) << 1)
        };
        for p in 0..n as u32 {
            let l = &cloth.links[p as usize];
            let mut add = |b: u32, rest: f32, kind: u8| {
                if mv(p) || mv(b) {
                    let (wa, wb, flags) = weights(p, b);
                    dists.push(Dist { a: p, b, wa, wb, rest, flags, kind });
                }
            };
            if let Some(r) = right(p) {
                add(r, l.right_len, 0);
            }
            if let Some(u) = up(p)
                && let Some(ur) = right(u)
            {
                let ul = &cloth.links[u as usize];
                add(ur, (ul.right_len * ul.right_len + ul.down_len * ul.down_len).sqrt(), 1);
            }
            if let Some(d) = down(p) {
                add(d, l.down_len, 2);
            }
        }

        // The bend constraints: for each centre the pairs left/right, down-left/up-right, up/down.
        let mut bends = Vec::new();
        for c in 0..n as u32 {
            let pairs = [
                (left(c), right(c)),
                (down(c).and_then(left), up(c).and_then(right)),
                (up(c), down(c)),
            ];
            for (a, b) in pairs {
                let (Some(a), Some(b)) = (a, b) else { continue };
                if !(mv(a) || mv(b) || mv(c)) {
                    continue;
                }
                let (pa, pb, pc) = (rest[a as usize], rest[b as usize], rest[c as usize]);
                let centre = (pb + pa + pc) * 0.333_333_34;
                let flags = mv(a) as u8 | (mv(b) as u8) << 1 | (mv(c) as u8) << 2 | (!mv(c) as u8) << 3;
                bends.push(Bend {
                    a,
                    b,
                    c,
                    flags,
                    wa: if mv(a) { 0.5 } else { 0.0 },
                    wb: if mv(b) { 0.5 } else { 0.0 },
                    wc: if mv(c) { -1.0 } else { -0.0 },
                    rest2: len2(pc - centre),
                });
            }
        }

        // The triangles of each grid cell, for the capsules' surface test.
        let mut tris = Vec::new();
        let column_top = |mut p: u32| {
            while mv(p) {
                match up(p) {
                    Some(u) => p = u,
                    None => break,
                }
            }
            p
        };
        for tl in 0..n as u32 {
            let (Some(r), Some(d)) = (right(tl), down(tl)) else { continue };
            let Some(dr) = right(d) else { continue };
            let flag = |a: u32, b: u32, c: u32| mv(a) as u8 | (mv(b) as u8) << 1 | (mv(c) as u8) << 2;
            tris.push(Tri { a: d, b: tl, c: r, d: column_top(r), flags: flag(d, tl, r), second: false });
            tris.push(Tri { a: d, b: r, c: dr, d: column_top(dr), flags: flag(d, r, dr), second: true });
        }

        let limits = cloth.limits.iter().map(|l| Limit { point: l.point, anchor: l.anchor, max2: l.length * l.length }).collect();
        let step_limit = if dists.is_empty() { 0.0 } else { (dists.iter().map(|d| d.rest as f64).sum::<f64>() / dists.len() as f64) as f32 * 1.2 };

        let s = cloth.settings;
        let gravity_world = Vec3::new(0.0, -980.0 * s.gravity_scale, 0.0);
        let axis = match s.gravity_axis {
            1 => Vec3::X,
            2 => -Vec3::X,
            3 => Vec3::Y,
            4 => -Vec3::Y,
            5 => Vec3::Z,
            6 => -Vec3::Z,
            _ => Vec3::ZERO,
        };
        let gravity_local = axis * gravity_world.length() * s.local_gravity_scale;
        let (mn, mx) = rest.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), &p| (a.min(p), b.max(p)));
        let extent = if rest.is_empty() { 0.0 } else { (mx - mn).length() * 0.5 };

        ClothSim {
            settings: s,
            movable,
            depth,
            blend,
            links,
            dists,
            bends,
            tris,
            limits,
            step_limit,
            gravity_world,
            gravity_local,
            local_gravity,
            extent,
            shapes: colliders.iter().map(|c| (c.shape, c.size[0], c.size[1])).collect(),
            placed: Vec::new(),
            p: rest.clone(),
            r: rest.clone(),
            v: vec![Vec3::ZERO; n],
            s: vec![Vec3::ZERO; n],
            rest,
            parent: None,
            velocity_state: DVec3::ZERO,
            spin_state: DVec3::ZERO,
        }
    }

    /// The colliders as the last step placed them, in the parent's space.
    pub fn colliders(&self) -> &[LocalCollider] {
        &self.placed
    }

    /// The points now (the parent's space).
    pub fn points(&self) -> &[Vec3] {
        &self.p
    }

    /// The points at rest, as the file has them.
    pub fn rest(&self) -> &[Vec3] {
        &self.rest
    }

    /// Back to rest: the points as the file has them but for the pinned ones (`pins`, the first
    /// points), no motion, the parent where it stands.
    pub fn reset(&mut self, parent: DMat4, pins: &[Vec3]) {
        self.p.clone_from(&self.rest);
        self.p[..pins.len()].copy_from_slice(pins);
        self.r.clone_from(&self.p);
        self.v.fill(Vec3::ZERO);
        self.parent = Some(parent);
        self.velocity_state = DVec3::ZERO;
        self.spin_state = DVec3::ZERO;
    }

    /// One 1/60 s step. `parent` is the parent joint's new world matrix, `pins` the pinned
    /// points' new places (the first points, in the parent's space), `colliders` the world
    /// matrices of the piece's colliders (the order they were given to `new`), `wind` the
    /// world wind (a velocity).
    pub fn step(&mut self, dt: f32, parent: DMat4, pins: &[Vec3], colliders: &[DMat4], wind: Vec3) {
        let inverse = parent.inverse();
        let cols: Vec<LocalCollider> = self
            .shapes
            .iter()
            .zip(colliders)
            .map(|(&(shape, radius, half), w)| {
                let local = (inverse * *w).as_mat4();
                LocalCollider { shape, radius, half, local, inverse: local.inverse() }
            })
            .collect();
        self.placed.clone_from(&cols);
        self.p[..pins.len()].copy_from_slice(pins);
        let old = self.parent.unwrap_or(parent);
        let (env, d, jump) = self.parent_motion(dt, old, parent, wind);
        self.parent = Some(parent);
        if !jump {
            self.reexpress(d);
        }
        self.normals();
        self.run(dt, &env, jump, &cols);
    }

    /// The parent's motion this step: the environment in its new space, `old x inverse(new)` and
    /// whether the move is a jump (the points then ride the parent as they are).
    fn parent_motion(&mut self, dt: f32, old: DMat4, new: DMat4, wind: Vec3) -> (Env, Mat4, bool) {
        let inverse = new.inverse();
        let to_local = |v: DVec3| inverse.transform_vector3(v).as_vec3();
        let s = &self.settings;
        let dt64 = dt as f64;
        let mut jump = false;

        // The parent's move, clamped to four times the piece's size, scaled.
        let mut delta = new.w_axis.truncate() - old.w_axis.truncate();
        let len = delta.length();
        if len > self.extent as f64 * 0.4 {
            jump = true;
        }
        if len > self.extent as f64 * 4.0 {
            delta *= self.extent as f64 * 4.0 / len;
        }
        delta *= s.parent_velocity_scale as f64;

        // Its turn: the rotation vector of `inverse(old) x new`.
        let e = (new * old.inverse()).to_cols_array_2d();
        let t = e[1][1] + e[0][0] + e[2][2] + 1.0;
        let anti = DVec3::new(e[1][2] - e[2][1], e[2][0] - e[0][2], e[0][1] - e[1][0]);
        let mut rot = if t > 1e-6 && t > 2.478 {
            anti * (2.0 / t)
        } else if t > 1e-6 {
            let theta = (t / 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            anti * (theta / ((4.0 - t) * t).sqrt())
        } else {
            DVec3::ZERO
        };
        if rot.length() > std::f64::consts::PI / 8.0 {
            jump = true;
        }
        rot *= s.parent_spin_scale as f64;

        // Accelerations against the smoothed velocity and spin, damped above the limits.
        let mut lin = (delta - self.velocity_state * dt64) / (dt64 * dt64);
        let mut ang = (rot - self.spin_state * dt64) / (dt64 * dt64);
        let (a2, r2) = (lin.length_squared() as f32, ang.length_squared() as f32);
        if s.accel_zero_check && (a2 > s.accel_hard[0] * s.accel_hard[0] || r2 > s.accel_hard[1] * s.accel_hard[1]) {
            lin = DVec3::ZERO;
            ang = DVec3::ZERO;
            jump = true;
        }
        if s.accel_clamp_check {
            let (la, ra) = (s.accel_soft[0] * s.accel_soft[0], s.accel_soft[1] * s.accel_soft[1]);
            if a2 > la || r2 > ra {
                jump = true;
            }
            if a2 > la {
                lin *= (s.accel_clamp[0] / a2.sqrt()) as f64;
            }
            if r2 > ra {
                ang *= (s.accel_clamp[1] / r2.sqrt()) as f64;
            }
        }
        self.velocity_state += lin * dt64;
        self.spin_state += ang * dt64;

        let mut gravity = to_local(self.gravity_world.as_dvec3());
        if s.gravity_axis != 0 && self.local_gravity > 0.0 {
            let k = self.local_gravity.clamp(0.0, 1.0);
            gravity = gravity * (1.0 - k) + self.gravity_local * k;
        }
        let mut wind = to_local(wind.as_dvec3());
        if s.wind_damping > 0.0 && s.wind_damping <= 1.0 {
            wind *= 1.0 - s.wind_damping;
        }
        let env = Env { velocity: to_local(self.velocity_state), accel: to_local(lin), spin: to_local(self.spin_state), gravity, wind };

        // Old parent space to the new one, built relative to the translation so a far-off
        // world costs nothing (the game's float32 product carries ~0.004 of noise at 32,000).
        let d = (inverse * old).as_mat4();
        (env, d, jump)
    }

    /// The parent's motion carried to the points: the top rows follow the parent, deep ones
    /// keep their place in the world (`t = (depth / blend)^3`).
    fn reexpress(&mut self, d: Mat4) {
        let n = self.blend;
        for i in 0..self.p.len() {
            if !self.movable[i] {
                continue;
            }
            let (q, u) = (d.transform_point3(self.p[i]), d.transform_vector3(self.v[i]));
            if self.depth[i] < n {
                let t = self.depth[i] as f32 * (1.0 / n as f32);
                let t = t * t * t;
                self.p[i] = (q - self.p[i]) * t + self.p[i];
                self.v[i] = (u - self.v[i]) * t + self.v[i];
            } else {
                self.p[i] = q;
                self.v[i] = u;
            }
        }
    }

    /// The normals of the movable points: across the grid cross down it.
    fn normals(&mut self) {
        for i in 0..self.p.len() {
            if !self.movable[i] {
                continue;
            }
            let [l, r, u, d] = self.links[i].map(|q| q.map_or(i, |q| q as usize));
            let a = self.p[r] - self.p[l];
            let b = self.p[d] - self.p[u];
            let n = Vec3::new(b.y * a.z - b.z * a.y, a.x * b.z - b.x * a.z, b.x * a.y - a.x * b.y);
            let l2 = len2(n);
            self.s[i] = if l2 > 1e-6 { n / l2.sqrt() } else { Vec3::ZERO };
        }
    }

    /// Stages 4 to 7 (integrate, constraints, limits, velocity) from prepared points.
    fn run(&mut self, dt: f32, env: &Env, jump: bool, cols: &[LocalCollider]) {
        self.integrate(env, dt, jump);
        let mut broad = None;
        for it in 0..self.settings.iterations.max(1) {
            for _ in 0..self.settings.bend_passes.max(1) {
                self.bend();
            }
            self.dist();
            self.collide(cols, it == 0, &mut broad);
        }
        self.limit();
        self.velocity(dt);
    }

    /// Stage 4: gravity and spin accelerate, damping, then the wind's drag along and across the normal.
    fn integrate(&mut self, env: &Env, dt: f32, jump: bool) {
        let s = self.settings;
        let keep = 1.0 - s.damping.clamp(0.0, 1.0);
        let mut w = env.spin;
        let l = w.length();
        let (mut factor, mut w2) = (-0.25, 0.0);
        if l.abs() > EPS {
            let q = l / 18.849556;
            w *= q.sqrt() / q.max(1.0);
            w2 = -w.length_squared();
            if l < std::f32::consts::PI {
                factor = (1.0 - (l / std::f32::consts::PI).powi(2)) * 10.0 * s.spin_factor - 0.25;
            }
        }
        let k = dt * -0.05 * s.drag;
        let rel = env.velocity - env.wind;
        let speed2 = s.speed_limit * s.speed_limit;
        self.r.clone_from(&self.p);
        for i in 0..self.p.len() {
            if !self.movable[i] {
                continue;
            }
            let r = self.r[i];
            let mut force = env.gravity + factor * (w * w.dot(r) + r * w2);
            if jump {
                force -= env.accel;
            }
            let mut v = (self.v[i] + force * dt) * keep;
            if s.speed_clamp && speed2 > 1.0 && v.length_squared() > speed2 {
                v *= (speed2 / v.length_squared()).sqrt();
            }
            self.v[i] = v;
            let n = self.s[i];
            let along = n * n.dot(rel);
            self.p[i] = r + v * dt + (k * along + 0.1 * k * (rel - along));
        }
    }

    /// Stage 5a: bends pull each centre toward the mean of its three points.
    fn bend(&mut self) {
        let s = self.settings;
        let thr = s.bend_skip * 0.1;
        let (soft, hard) = (s.bend.clamp(0.0, 1.0), (s.bend * s.bend_pinned).clamp(0.0, 1.0));
        for b in &self.bends {
            let (a, bb, c) = (b.a as usize, b.b as usize, b.c as usize);
            let centre = (self.p[bb] + self.p[a] + self.p[c]) * 0.333_333_34;
            let d = self.p[c] - centre;
            let l2 = len2(d);
            if l2.abs() <= EPS {
                continue;
            }
            let mut k = 1.0 - b.rest2 / l2;
            if k.abs() <= thr {
                continue;
            }
            k *= if b.flags & 8 != 0 { hard } else { soft };
            if b.flags & 1 != 0 && self.movable[a] {
                self.p[a] += d * (k * b.wa);
            }
            if b.flags & 2 != 0 && self.movable[bb] {
                self.p[bb] += d * (k * b.wb);
            }
            if b.flags & 4 != 0 && self.movable[c] {
                self.p[c] += d * (k * b.wc);
            }
        }
    }

    /// Stage 5b: distance constraints, `P_a += u k w_a` with `k = (L - rest) stiffness`.
    fn dist(&mut self) {
        let s = self.settings;
        let stiff = [s.stiff_horizontal.clamp(0.0, 1.0), s.stiff_shear.clamp(0.0, 1.0), s.stiff_vertical.clamp(0.0, 1.0)];
        for _ in 0..s.dist_passes.max(1) {
            for d in &self.dists {
                let st = stiff[d.kind as usize];
                if st.abs() <= EPS {
                    continue;
                }
                let (a, b) = (d.a as usize, d.b as usize);
                let delta = self.p[a] - self.p[b];
                let l2 = len2(delta);
                let (u, l) = if l2.is_finite() && l2.abs() > EPS {
                    let l = l2.sqrt();
                    (delta * (1.0 / l), l)
                } else {
                    (Vec3::X, 0.0)
                };
                let k = (l - d.rest) * st;
                if d.flags & 1 != 0 && self.movable[a] {
                    self.p[a] += u * (k * d.wa);
                }
                if d.flags & 2 != 0 && self.movable[b] {
                    self.p[b] += u * (k * d.wb);
                }
            }
        }
    }

    /// Stage 5c: capsules and spheres push points out along their normals and edges out by
    /// their midpoints; on the first iteration a capsule also pushes the triangles its axis passes through.
    fn collide(&mut self, cols: &[LocalCollider], first: bool, broad: &mut Option<Broad>) {
        if first || broad.is_none() {
            let grid = Grid::new(&self.p);
            let mut point = vec![0; self.p.len()];
            let mut any = 0;
            for i in (0..self.p.len()).filter(|&i| self.movable[i]) {
                point[i] = grid.point(self.p[i]);
                any |= point[i];
            }
            let masks = cols.iter().map(|c| grid.collider(c)).collect();
            *broad = Some(Broad { grid, point, any, masks });
            self.collide_triangles(cols, broad.as_ref().unwrap());
        }
        let broad = broad.as_mut().unwrap();
        for _ in 0..self.settings.collision_passes.max(1) {
            for (k, c) in cols.iter().enumerate() {
                if broad.masks[k] & broad.any == 0 {
                    continue;
                }
                let r = c.radius;
                for i in (0..self.p.len()).filter(|&i| self.movable[i]) {
                    let pm = broad.point[i];
                    if broad.masks[k] & pm != pm {
                        continue;
                    }
                    let q = c.inverse.transform_point3(self.p[i]);
                    if c.shape == 5 {
                        let Some((e, d2)) = capsule(c, q) else { continue };
                        let depth = r - d2.sqrt();
                        if depth <= 0.0 {
                            continue;
                        }
                        let n = unit(q.x, e, q.z, e * e + q.x * q.x + q.z * q.z);
                        self.p[i] += c.local.transform_vector3(n) * depth;
                    } else {
                        if q.x > r || -r > q.x || q.y > r || -r > q.y || q.z > r || -r > q.z {
                            continue;
                        }
                        let d2 = q.y * q.y + q.x * q.x + q.z * q.z;
                        if r * r <= d2 {
                            continue;
                        }
                        let n = unit(q.x, q.y, q.z, d2);
                        let depth = r - d2.sqrt();
                        if depth > 0.0 {
                            self.p[i] = c.local.transform_point3(n * depth + q);
                        }
                    }
                    broad.point[i] = broad.grid.point(self.p[i]);
                    broad.any |= broad.point[i];
                }
                for d in &self.dists {
                    let (a, b) = (d.a as usize, d.b as usize);
                    let m = (self.p[a] + self.p[b]) * 0.5;
                    let q = c.inverse.transform_point3(m);
                    let push = if c.shape == 5 {
                        let Some((e, d2)) = capsule(c, q) else { continue };
                        let depth = r - d2.sqrt();
                        if depth < 0.0 {
                            continue;
                        }
                        c.local.transform_vector3(unit(q.x, e, q.z, e * e + q.x * q.x + q.z * q.z)) * (depth * 0.75)
                    } else {
                        if q.x > r || -r > q.x || q.y > r || -r > q.y || q.z > r || -r > q.z {
                            continue;
                        }
                        let d2 = q.y * q.y + q.x * q.x + q.z * q.z;
                        if r * r <= d2 {
                            continue;
                        }
                        let depth = r - d2.sqrt();
                        if depth <= 0.0 {
                            continue;
                        }
                        c.local.transform_vector3(unit(q.x, q.y, q.z, d2)) * (depth * 0.75 * 0.75)
                    };
                    if d.flags & 1 != 0 {
                        self.p[a] += push * if d.flags & 2 != 0 { 1.0 } else { 2.0 };
                    }
                    if d.flags & 2 != 0 {
                        self.p[b] += push * if d.flags & 1 != 0 { 1.0 } else { 2.0 };
                    }
                }
            }
        }
    }

    /// A capsule's axis passing through a cell of the grid pushes its corners out: for the
    /// triangles whose corners lie within the capsule's length and enclose the axis.
    fn collide_triangles(&mut self, cols: &[LocalCollider], broad: &Broad) {
        for (k, c) in cols.iter().enumerate() {
            if c.shape != 5 || broad.masks[k] & broad.any == 0 {
                continue;
            }
            let a_all: Vec<Vec3> = self.p.iter().map(|&p| c.inverse.transform_point3(p)).collect();
            let (r, h) = (c.radius, c.half);
            for t in &self.tris {
                let (a, b, cc) = (t.a as usize, t.b as usize, t.c as usize);
                let (pa, pb, pc) = (a_all[a], a_all[b], a_all[cc]);
                if pa.y.abs() > h || pb.y.abs() > h || pc.y.abs() > h {
                    continue;
                }
                let (e1x, e1z) = (pb.x - pa.x, pb.z - pa.z);
                let l1 = e1x * e1x + e1z * e1z;
                if l1.abs() <= EPS {
                    continue;
                }
                let (e2x, e2z) = (pc.x - pa.x, pc.z - pa.z);
                let l2 = e2x * e2x + e2z * e2z;
                if l2.abs() <= EPS {
                    continue;
                }
                let (i1, i2) = (1.0 / l1.sqrt(), 1.0 / l2.sqrt());
                let cs = ((e1z * i1) * (e2z * i2) + (e1x * i1) * (e2x * i2)).abs();
                if (1.0 - EPS..=1.0 + EPS).contains(&cs) {
                    continue;
                }
                let (ax, az) = (-pa.x, -pa.z);
                let d12 = e1z * e2z + e1x * e2x;
                let (pe1, pe2) = (ax * e1x + az * e1z, az * e2z + ax * e2x);
                let inv = 1.0 / (l2 * l1 - d12 * d12);
                let u = (pe1 * l2 - pe2 * d12) * inv;
                if !(0.0 < u && u < 1.0) {
                    continue;
                }
                let v = (pe2 * l1 - pe1 * d12) * inv;
                if !(0.0 < v && v < 1.0) {
                    continue;
                }
                let w = 1.0 - u - v;
                if !(0.0 < w && w < 1.0) {
                    continue;
                }
                let mut q = c.inverse.transform_point3(self.p[t.d as usize]);
                let m = q.z * q.z + q.x * q.x;
                if m.abs() > EPS {
                    let s = 1.0 / m.sqrt();
                    q = Vec3::new(q.x * s, 0.0, q.z * s);
                }
                let dw = c.local.transform_vector3(q);
                let lim = r * -0.35;
                for (vtx, bit, pos) in [(a, 1, pa), (b, 2, pb), (cc, 4, pc)] {
                    if t.flags & bit != 0 {
                        let dot = q.x * pos.x + q.z * pos.z;
                        if dot < 0.0 {
                            self.p[vtx] += dw * (lim.max(dot) * -1.8);
                        }
                    }
                }
            }
        }
    }

    /// Stage 6: a point farther than its length from its anchor goes back onto that sphere.
    fn limit(&mut self) {
        for l in &self.limits {
            let (a, b) = (l.point as usize, l.anchor as usize);
            let d = self.p[a] - self.p[b];
            let l2 = len2(d);
            if !l2.is_finite() || l2.abs() <= EPS || l2 <= l.max2 {
                continue;
            }
            self.p[a] = d * (l.max2 / l2).sqrt() + self.p[b];
        }
    }

    /// Stage 7: the velocity is the step, clamped to the step limit, per second.
    fn velocity(&mut self, dt: f32) {
        let inv = 1.0 / dt;
        for i in 0..self.p.len() {
            if !self.movable[i] {
                continue;
            }
            let mut v = self.p[i] - self.r[i];
            let l2 = len2(v);
            if l2 > self.step_limit * self.step_limit {
                v *= self.step_limit / l2.sqrt();
            }
            self.v[i] = v * inv;
        }
    }
}
