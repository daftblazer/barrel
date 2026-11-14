//! Posing a KTGL skeleton (`barrel_formats::g1m::Skeleton`): a motion's local transforms at a
//! time, two poses mixed, and the skinning matrices a pose gives the renderer.

use barrel_formats::g1a::Animation;
use barrel_formats::g1m::{Joint, Skeleton};
use glam::{Mat4, Quat, Vec3};

pub fn pose(skeleton: &Skeleton, anim: &Animation, time: f32) -> Vec<Mat4> {
    skin(skeleton, &local_pose(skeleton, anim, time))
}

/// A joint's local transform: scale, rotation, translation.
pub type Trs = (Vec3, Quat, Vec3);

/// Each joint's local transform for an animation at `time`. Joints without a track keep their
/// bind pose; rotation-only tracks keep the joint's bind translation and scale.
pub fn local_pose(skeleton: &Skeleton, anim: &Animation, time: f32) -> Vec<Trs> {
    let bind = |j: &Joint| (Vec3::from(j.scale), Quat::from_array(j.rotation).normalize(), Vec3::from(j.position));
    let mut out: Vec<Trs> = skeleton.joints.iter().map(bind).collect();
    for t in &anim.tracks {
        let Some(slot) = out.get_mut(t.bone as usize) else { continue };
        if let Some((r, p, s)) = t.pose(time) {
            *slot = (Vec3::from(s), Quat::from_array(r).normalize(), Vec3::from(p));
        } else if let Some(r) = t.rotation(time) {
            slot.1 = Quat::from_array(r).normalize();
        }
    }
    out
}

/// Each joint's position in model space for local transforms.
pub fn skin_world(skeleton: &Skeleton, locals: &[Trs]) -> Vec<Vec3> {
    let mut out = vec![Mat4::IDENTITY; locals.len()];
    for i in skeleton.order() {
        let (sc, r, t) = locals[i];
        let m = Mat4::from_scale_rotation_translation(sc, r, t);
        out[i] = match skeleton.joints[i].parent {
            Some(p) => out[p] * m,
            None => m,
        };
    }
    out.iter().map(|m| m.w_axis.truncate()).collect()
}

/// Two local poses mixed: `w` of the way from `a` to `b` (rotations by slerp).
pub fn mix_pose(a: &[Trs], b: &[Trs], w: f32) -> Vec<Trs> {
    a.iter().zip(b).map(|(x, y)| (x.0.lerp(y.0, w), x.1.slerp(y.1, w), x.2.lerp(y.2, w))).collect()
}

/// Skinning matrices for local joint transforms: each joint's posed world transform times the
/// inverse of its bind world transform.
pub fn skin(skeleton: &Skeleton, locals: &[Trs]) -> Vec<Mat4> {
    let local = |j: &Joint| {
        Mat4::from_scale_rotation_translation(Vec3::from(j.scale), Quat::from_array(j.rotation).normalize(), Vec3::from(j.position))
    };
    let posed_local: Vec<Mat4> = locals.iter().map(|&(s, r, t)| Mat4::from_scale_rotation_translation(s, r, t)).collect();
    let world = |locals: &[Mat4]| {
        let mut out = vec![Mat4::IDENTITY; locals.len()];
        for i in skeleton.order() {
            out[i] = match skeleton.joints[i].parent {
                Some(p) => out[p] * locals[i],
                None => locals[i],
            };
        }
        out
    };
    let bind_local: Vec<Mat4> = skeleton.joints.iter().map(local).collect();
    let (bind, posed) = (world(&bind_local), world(&posed_local));
    bind.iter().zip(&posed).map(|(b, p)| *p * b.inverse()).collect()
}

/// A skeleton's joints in their bind pose, in model space.
pub fn bind_world(skeleton: &Skeleton) -> Vec<Mat4> {
    let mut out = vec![Mat4::IDENTITY; skeleton.joints.len()];
    for i in skeleton.order() {
        let j = &skeleton.joints[i];
        let local = Mat4::from_scale_rotation_translation(Vec3::from(j.scale), Quat::from_array(j.rotation).normalize(), Vec3::from(j.position));
        out[i] = match j.parent {
            Some(p) => out[p] * local,
            None => local,
        };
    }
    out
}
