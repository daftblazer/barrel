//! Ryza's face: `\data\x64\character\<set>\face\<set>_face_<name>.elixir.gz`, each one `.g1a`
//! of one frame on the face's bones (ids 300 and up; bone 0 is in them too, left alone here).
//! Her field motions do not move those bones, so without a face her face keeps its bind pose.
//! The expression (`expression_default`) is laid over the body's pose, and a blink mixes toward
//! `closeeye_down` (the blink's timing is a stand-in, `docs/ryza/open.md`). Docs:
//! docs/formats.md, "Ryza's character files".

use barrel_engine::skin::{local_pose, Trs};
use barrel_formats::g1a::Animation;
use barrel_formats::g1m::Skeleton;

/// The first bone id of the face.
pub const FIRST_BONE: u32 = 300;

/// A face file's path in the archives (`set` as `PC00A`, `name` as `expression_default`).
pub fn path(set: &str, name: &str) -> String {
    let set = set.to_ascii_lowercase();
    format!("\\data\\x64\\character\\{set}\\face\\{set}_face_{name}.elixir.gz")
}

/// A face pose: the joints it sets and their local transforms.
pub struct Face(Vec<(usize, Trs)>);

impl Face {
    pub fn new(skeleton: &Skeleton, anim: &Animation) -> Face {
        let locals = local_pose(skeleton, anim, 0.0);
        Face(anim.tracks.iter().filter(|t| t.bone >= FIRST_BONE).filter_map(|t| skeleton.joint(t.bone)).filter_map(|j| Some((j, *locals.get(j)?))).collect())
    }

    /// `locals` with this face laid over them; `closed` mixed in by `blink` (0 open, 1 shut).
    pub fn apply(&self, locals: &mut [Trs], closed: Option<&Face>, blink: f32) {
        for &(j, t) in &self.0 {
            if let Some(slot) = locals.get_mut(j) {
                *slot = t;
            }
        }
        if let Some(c) = closed.filter(|_| blink > 0.0) {
            for &(j, (s, r, p)) in &c.0 {
                if let Some(slot) = locals.get_mut(j) {
                    *slot = (slot.0.lerp(s, blink), slot.1.slerp(r, blink), slot.2.lerp(p, blink));
                }
            }
        }
    }
}

/// How shut her eyes are `t` seconds in: a blink every 4 seconds, closing over 0.06 s, shut
/// for 0.04 s and opening over 0.08 s.
pub fn blink(t: f32) -> f32 {
    let k = t.rem_euclid(4.0);
    if k < 0.06 {
        k / 0.06
    } else if k < 0.10 {
        1.0
    } else if k < 0.18 {
        1.0 - (k - 0.10) / 0.08
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_blink_shuts_and_opens() {
        assert_eq!(super::blink(0.0), 0.0);
        assert_eq!(super::blink(0.08), 1.0);
        assert_eq!(super::blink(1.0), 0.0);
        assert!((super::blink(4.03) - 0.5).abs() < 1e-3);
        assert_eq!(super::path("PC00A", "expression_default"), "\\data\\x64\\character\\pc00a\\face\\pc00a_face_expression_default.elixir.gz");
    }
}
