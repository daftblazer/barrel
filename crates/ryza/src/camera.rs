//! A field's player camera, `saves\fieldmap\cameraparam\<field>.xml` (Shift-JIS):
//! `free_camera/base_setting` gives its field of view, its default turn, its pitch range and
//! default, and its distance range and default (Rasenboden: 45°, yaw 180, pitch -30 to 45 from
//! 8, distance 200 to 550 from 400). `at_diff_y` (0.1) is read but not placed: what it is a
//! tenth of is not known (`docs/ryza/open.md`). The file's `landmark_camera`s (cameras chosen
//! by gimmicks), display ranges, LOD and ambient effects are not read yet. Docs:
//! docs/formats.md, "Ryza field cameras".

use barrel_formats::table::{self, Element};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeCamera {
    pub at_diff_y: f32,
    /// Degrees.
    pub fov: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub pitch_range: (f32, f32),
    /// Units (centimetres).
    pub length: f32,
    pub length_range: (f32, f32),
}

impl FreeCamera {
    /// Rasenboden's, for a field without a file.
    pub const DEFAULT: FreeCamera = FreeCamera { at_diff_y: 0.1, fov: 45.0, yaw: 180.0, pitch: 8.0, pitch_range: (-30.0, 45.0), length: 400.0, length_range: (200.0, 550.0) };

    /// The free camera of a cameraparam file; the default's value for anything it lacks.
    pub fn parse(bytes: &[u8]) -> Result<FreeCamera, String> {
        let root = table::parse(bytes).map_err(|e| format!("cameraparam: {e}"))?;
        let base = find(&root, &["free_camera", "base_setting"]).ok_or("cameraparam: no free_camera base_setting")?;
        let d = FreeCamera::DEFAULT;
        let f = |name: &str, or: f32| base.attr(name).and_then(|v| v.trim().parse().ok()).unwrap_or(or);
        Ok(FreeCamera {
            at_diff_y: f("at_diff_y", d.at_diff_y),
            fov: f("fov_default", d.fov),
            yaw: f("yaw_default", d.yaw),
            pitch: f("pitch_default", d.pitch),
            pitch_range: (f("pitch_min", d.pitch_range.0), f("pitch_max", d.pitch_range.1)),
            length: f("length_default", d.length),
            length_range: (f("length_min", d.length_range.0), f("length_max", d.length_range.1)),
        })
    }

    /// The file's name in the archives.
    pub fn path(field: &str) -> String {
        format!("\\saves\\fieldmap\\cameraparam\\{}.xml", field.to_ascii_lowercase())
    }
}

/// How far the camera may stand from `target` along `way` (game space, a unit vector from the
/// target toward the eye), up to `dist`: the line is followed in steps, and the first point
/// with ground (`ground`: terrain and floors) from 30 below it to 400 above it stops it a step
/// short (no less than `MIN_CLEAR`). Not the game's (its camera collision is not read,
/// `docs/ryza/open.md`): ground only, no walls, and a tree's top far above is not ground here.
pub fn clear_distance(ground: &barrel_engine::walk::Ground, target: glam::Vec3, way: glam::Vec3, dist: f32) -> f32 {
    const STEP: f32 = 25.0;
    let mut t = STEP;
    while t <= dist {
        let p = target + way * t;
        if ground.height(p.x, p.z, p.y + 400.0, 0.0).is_some_and(|h| h > p.y - 30.0) {
            return (t - STEP).max(MIN_CLEAR);
        }
        t += STEP;
    }
    dist
}

/// The nearest the camera comes to her when the ground pulls it in.
pub const MIN_CLEAR: f32 = 60.0;

/// The first element down `path` from `e` (the root itself may be the first name's parent).
fn find<'a>(e: &'a Element, path: &[&str]) -> Option<&'a Element> {
    match path.split_first() {
        None => Some(e),
        Some((first, rest)) => e.children.iter().filter(|c| c.name == *first).find_map(|c| find(c, rest)).or_else(|| e.children.iter().find_map(|c| (c.name == "root").then(|| find(c, path)).flatten())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bank_behind_her_pulls_the_camera_in() {
        // A floor 100 above her look point, from 200 behind her on.
        let c = barrel_formats::g1co::Collision {
            nodes: Vec::new(),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            vertices: vec![[-1000.0, 100.0, -200.0], [1000.0, 100.0, -200.0], [1000.0, 100.0, -5000.0], [-1000.0, 100.0, -5000.0]],
        };
        let g = barrel_engine::walk::Ground::new(None, &c, 75.0);
        let back = glam::Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(clear_distance(&g, glam::Vec3::ZERO, back, 400.0), 175.0);
        // Looking the other way, nothing in the way.
        assert_eq!(clear_distance(&g, glam::Vec3::ZERO, -back, 400.0), 400.0);
    }

    #[test]
    fn free_camera_of_a_file() {
        let xml = br#"<?xml version="1.0" encoding="SHIFT_JIS" ?>
<root><free_camera><base_setting at_diff_y="0.1" fov_default="40.0" yaw_default="180.0"
 pitch_min="-20.0" pitch_max="45.0" pitch_default="8.0" length_min="200.0" length_max="500.0"
 length_default="350.0" /></free_camera>
<landmark_camera><base_setting fov_default="50.0" /></landmark_camera></root>"#;
        let c = FreeCamera::parse(xml).unwrap();
        assert_eq!((c.fov, c.pitch_range, c.length, c.length_range), (40.0, (-20.0, 45.0), 350.0, (200.0, 500.0)));
    }
}
