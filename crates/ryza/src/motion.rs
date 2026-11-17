//! A playable character's field motions: `\data\x64\character\<set>\<set>_<NNNNN>.elixir.gz`
//! (one `.g1a` each), the set from `saves\chara\charamotionset.xml` (Ryza: `PC00A`). The
//! numbers come in threes, start, loop and stop; the motions stay in place (the root bone
//! does not travel), so how fast she moves is the game's: `stride_speed` measures the speed at
//! which her planted feet would not slide. Checked by posing
//! Ryza's (docs/formats.md, "Ryza's motions").

/// The idle stand (2.52 s, looping).
pub const STAND: u32 = 0;
/// Walking: start (0.63 s), the loop (1.35 s), stop (0.67 s).
pub const WALK_START: u32 = 10;
pub const WALK: u32 = 11;
pub const WALK_STOP: u32 = 12;
/// Running: start (0.25 s), the loop (0.75 s), stop (0.65 s).
pub const RUN_START: u32 = 20;
pub const RUN: u32 = 21;
pub const RUN_STOP: u32 = 22;

/// The hip's bone id: the joint a motion's root translation drives (a motion of a set shared by
/// a body type is made for a hip `SkeletonBase` high, and moves a model's hip in proportion).
pub const HIP: u32 = 1;

/// The feet's bone ids (`SK_L_Foot`, `SK_R_Foot` in the model's `Oid.bin`).
pub const FEET: [u32; 2] = [11, 12];

/// How fast the body must travel for a planted foot to stay put in an in-place loop: the
/// motion is sampled at 240 Hz, a foot counts as planted within 1.5 units of its lowest, and
/// the speed is the median of how fast the planted feet slide along the ground.
pub fn stride_speed(skeleton: &barrel_formats::g1m::Skeleton, anim: &barrel_formats::g1a::Animation) -> Option<f32> {
    let n = (anim.duration * 240.0).ceil().max(2.0) as usize;
    let dt = anim.duration / n as f32;
    let feet: Vec<usize> = FEET.iter().filter_map(|&b| skeleton.joint(b)).collect();
    if feet.is_empty() {
        return None;
    }
    let at: Vec<Vec<glam::Vec3>> = (0..=n).map(|i| {
        let w = barrel_engine::skin::skin_world(skeleton, &barrel_engine::skin::local_pose(skeleton, anim, i as f32 * dt));
        feet.iter().map(|&j| w[j]).collect()
    }).collect();
    let paths: Vec<Vec<glam::Vec3>> = (0..feet.len()).map(|f| at.iter().map(|p| p[f]).collect()).collect();
    planted_slide(&paths, dt)
}

/// The median speed along the ground of feet while planted (within 1.5 units of their lowest),
/// each foot's path sampled every `dt` seconds.
pub fn planted_slide(paths: &[Vec<glam::Vec3>], dt: f32) -> Option<f32> {
    let mut slides = Vec::new();
    for path in paths {
        let low = path.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        for w in path.windows(2) {
            if w[0].y < low + 1.5 && w[1].y < low + 1.5 {
                slides.push(glam::Vec2::new(w[1].x - w[0].x, w[1].z - w[0].z).length() / dt);
            }
        }
    }
    if slides.is_empty() {
        return None;
    }
    slides.sort_by(f32::total_cmp);
    Some(slides[slides.len() / 2])
}

/// A motion's container in the archives.
pub fn path(set: &str, motion: u32) -> String {
    let set = set.to_ascii_lowercase();
    format!("\\data\\x64\\character\\{set}\\{set}_{motion:05}.elixir.gz")
}

#[cfg(test)]
mod tests {
    #[test]
    fn motion_paths() {
        // A foot planted for 30 samples sliding back 2 units each, then lifted and brought forward.
        let mut path: Vec<glam::Vec3> = (0..30).map(|i| glam::Vec3::new(0.0, 10.0, 30.0 - 2.0 * i as f32)).collect();
        path.extend((0..20).map(|i| glam::Vec3::new(0.0, 25.0, -30.0 + 3.0 * i as f32)));
        assert_eq!(super::planted_slide(&[path], 0.01), Some(200.0));
        assert_eq!(super::path("PC00A", super::RUN), "\\data\\x64\\character\\pc00a\\pc00a_00021.elixir.gz");
    }
}
