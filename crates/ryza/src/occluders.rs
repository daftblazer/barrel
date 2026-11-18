//! The field's occluders: the quads of `<field>_le.koc` hide the placed objects behind them.
//! The game keeps, each frame, the quads in the view, sorted nearest first, gives each five
//! planes (its face, and one through the eye and each edge) and drops those inside an earlier
//! one; an object is hidden when its box lies inside all five planes of a kept quad, the walk
//! stopping at the first quad that starts beyond the box (query 0x140a39fc0).
//! Docs: docs/renderer.md, "Ryza DX: objects behind the field's occluders".
//!
//! Everything here is in one space (the renderer's), the quads and boxes mirrored by the caller.

use glam::{Vec3, Vec4};

/// A quad's four corners.
pub type Corners = [Vec3; 4];

/// A placed object's oriented box: its centre and half edges as vectors.
#[derive(Clone, Copy, Debug)]
pub struct OrientedBox {
    pub center: Vec3,
    pub axes: [Vec3; 3],
}

impl OrientedBox {
    /// The least of `n · p + d` over the box.
    fn least(&self, plane: Vec4) -> f32 {
        let n = plane.truncate();
        n.dot(self.center) + plane.w - self.axes.iter().map(|a| n.dot(*a).abs()).sum::<f32>()
    }

    /// The least depth along `forward` from `eye` of any point of the box.
    fn near(&self, eye: Vec3, forward: Vec3) -> f32 {
        forward.dot(self.center - eye) - self.axes.iter().map(|a| forward.dot(*a).abs()).sum::<f32>()
    }
}

struct Kept {
    corners: Corners,
    near: f32,
    /// Face, then the four edges; inside where `n · p + d > 0`.
    planes: [Vec4; 5],
}

/// The quads that hide things from one eye.
#[derive(Default)]
pub struct Occluders {
    kept: Vec<Kept>,
}

fn plane(normal: Vec3, through: Vec3) -> Vec4 {
    normal.extend(-normal.dot(through))
}

/// A quad's five planes seen from `eye`: the face with its inside away from the eye, and each
/// edge's plane through the eye with the quad's centre inside.
fn planes_of(q: &Corners, eye: Vec3) -> Option<[Vec4; 5]> {
    let centre = q.iter().copied().sum::<Vec3>() * 0.25;
    let mut face = (q[1] - q[0]).cross(q[2] - q[0]).try_normalize()?;
    if face.dot(eye - centre) > 0.0 {
        face = -face;
    }
    let mut out = [plane(face, centre); 5];
    for k in 0..4 {
        let mut m = (q[k] - eye).cross(q[(k + 1) % 4] - eye).try_normalize()?;
        if m.dot(centre - eye) < 0.0 {
            m = -m;
        }
        out[k + 1] = plane(m, eye);
    }
    Some(out)
}

/// Whether a triangle may touch the view: not wholly outside any one of its planes.
fn touches(tri: [Vec3; 3], frustum: &[Vec4; 6]) -> bool {
    frustum.iter().all(|p| tri.iter().any(|v| p.truncate().dot(*v) + p.w >= 0.0))
}

impl Occluders {
    /// The kept quads for a view from `eye` looking along `forward`, with its `frustum`
    /// (`barrel_render::view_planes`).
    pub fn new(quads: &[Corners], eye: Vec3, forward: Vec3, frustum: &[Vec4; 6]) -> Occluders {
        let depth = |q: &Corners| q.iter().map(|c| forward.dot(*c - eye)).fold(f32::MAX, f32::min);
        let mut seen: Vec<&Corners> = quads.iter().filter(|q| touches([q[0], q[1], q[2]], frustum) || touches([q[0], q[2], q[3]], frustum)).collect();
        seen.sort_by(|a, b| depth(a).total_cmp(&depth(b)));
        let mut kept: Vec<Kept> = Vec::new();
        for q in seen {
            let Some(planes) = planes_of(q, eye) else { continue };
            // Wholly inside a nearer quad: it can hide nothing that one does not.
            if kept.iter().any(|k| q.iter().all(|c| k.planes.iter().all(|p| p.truncate().dot(*c) + p.w > 0.0))) {
                continue;
            }
            kept.push(Kept { corners: *q, near: depth(q), planes });
        }
        Occluders { kept }
    }

    /// How many quads are kept.
    pub fn len(&self) -> usize {
        self.kept.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// Whether the object is hidden: walking the quads nearest first, one whose five planes
    /// hold the box wholly inside hides it; the walk stops at the first quad that begins
    /// beyond the box's nearest point.
    pub fn hides(&self, object: &OrientedBox, eye: Vec3, forward: Vec3) -> bool {
        let near = object.near(eye, forward);
        for k in &self.kept {
            if k.near > near {
                return false;
            }
            if k.planes.iter().all(|p| object.least(*p) > 0.0) {
                return true;
            }
        }
        false
    }

    /// The kept quads' corners, for tests and traces.
    pub fn corners(&self) -> impl Iterator<Item = &Corners> {
        self.kept.iter().map(|k| &k.corners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE: Vec3 = Vec3::ZERO;
    const FORWARD: Vec3 = Vec3::Z;

    /// A wall `half` wide and 20 high at depth `z`, facing the eye.
    fn wall(z: f32, half: f32) -> Corners {
        [Vec3::new(-half, 10.0, z), Vec3::new(-half, -10.0, z), Vec3::new(half, -10.0, z), Vec3::new(half, 10.0, z)]
    }

    fn cube(c: Vec3, h: f32) -> OrientedBox {
        OrientedBox { center: c, axes: [Vec3::X * h, Vec3::Y * h, Vec3::Z * h] }
    }

    fn wide_view() -> [Vec4; 6] {
        let planes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y].map(|n| (n + Vec3::Z).normalize().extend(0.0));
        [planes[0], planes[1], planes[2], planes[3], Vec4::new(0.0, 0.0, 1.0, 0.0), Vec4::new(0.0, 0.0, -1.0, 1000.0)]
    }

    #[test]
    fn planes_hold_the_shadow_of_the_quad() {
        let p = planes_of(&wall(10.0, 10.0), EYE).unwrap();
        let inside = |v: Vec3| p.iter().all(|p| p.truncate().dot(v) + p.w > 0.0);
        assert!(inside(Vec3::new(0.0, 0.0, 20.0)));
        assert!(!inside(Vec3::new(0.0, 0.0, 5.0)), "in front of the quad");
        assert!(!inside(Vec3::new(30.0, 0.0, 20.0)), "outside its edges");
    }

    #[test]
    fn a_box_behind_is_hidden_and_one_in_front_is_not() {
        let o = Occluders::new(&[wall(10.0, 10.0)], EYE, FORWARD, &wide_view());
        assert!(o.hides(&cube(Vec3::new(0.0, 0.0, 30.0), 2.0), EYE, FORWARD));
        assert!(!o.hides(&cube(Vec3::new(0.0, 0.0, 5.0), 1.0), EYE, FORWARD), "the walk stops at a quad beyond the box");
        assert!(!o.hides(&cube(Vec3::new(29.0, 0.0, 30.0), 2.0), EYE, FORWARD), "sticking out of the side");
    }

    #[test]
    fn a_quad_behind_a_nearer_one_is_dropped() {
        let o = Occluders::new(&[wall(20.0, 3.0), wall(10.0, 10.0)], EYE, FORWARD, &wide_view());
        assert_eq!(o.len(), 1);
        assert_eq!(o.corners().next().unwrap()[0].z, 10.0);
    }

    #[test]
    fn a_quad_out_of_the_view_is_left_out() {
        let far_side = wall(10.0, 1.0).map(|c| c + Vec3::X * 500.0);
        assert!(Occluders::new(&[far_side], EYE, FORWARD, &wide_view()).is_empty());
    }
}
