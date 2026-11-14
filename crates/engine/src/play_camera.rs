//! Where the field's play camera stands (`CameraCFieldMap`, whose +0x130 holds the field's
//! camera table, `Saves/field/<FIELD>.xml` `<camera>`; the lookup is `0x140066f80`): the first
//! area, in table order, whose rectangle holds the player (strictly inside on x and z), then by
//! the area's kind a blend of its points along one axis (`0x140069680`; kinds `CLOSELY_X`,
//! `CLOSELY_Y`, `CLOSELY_Z` are axes 0, 1, 2): of the points whose `position` on that axis is
//! below the player's, the nearest, and of the rest the nearest, mixed linearly
//! (`0x140066cb0`) by how far the player is from the first over the gap between them; with only
//! one of the two, it alone. Kind 3 (`CLOSELY_XZ`, `0x140069960`) and 4 are not read (no field
//! uses them).

use barrel_formats::field_look::{CameraArea, CameraPoint};

/// The camera setting for the player at `player` (metres), or `None` outside every area.
pub fn setting(areas: &[CameraArea], points: &[CameraPoint], player: [f32; 3]) -> Option<CameraPoint> {
    let area = areas.iter().find(|a| {
        player[0] > a.top_left[0] && player[2] > a.top_left[2] && player[0] < a.bottom_right[0] && player[2] < a.bottom_right[2]
    })?;
    let axis = match area.kind {
        0..=2 => area.kind as usize,
        _ => 2,
    };
    let own = points.get(area.points.clone())?;
    let at = player[axis];
    let nearest = |below: bool| {
        own.iter()
            .filter(|p| (at > p.position[axis]) == below)
            .min_by(|a, b| (at - a.position[axis]).abs().total_cmp(&(at - b.position[axis]).abs()))
    };
    match (nearest(true), nearest(false)) {
        (Some(lo), Some(hi)) => {
            let (d0, d1) = ((at - lo.position[axis]).abs(), (hi.position[axis] - at).abs());
            Some(mix(lo, hi, d0 / (d0 + d1).max(1e-6)))
        }
        (Some(p), None) | (None, Some(p)) => Some(p.clone()),
        (None, None) => None,
    }
}

fn mix(a: &CameraPoint, b: &CameraPoint, t: f32) -> CameraPoint {
    let l = |x: f32, y: f32| x * (1.0 - t) + y * t;
    let l3 = |x: [f32; 3], y: [f32; 3]| [l(x[0], y[0]), l(x[1], y[1]), l(x[2], y[2])];
    CameraPoint {
        length: l(a.length, b.length),
        yaw: l(a.yaw, b.yaw),
        pitch: l(a.pitch, b.pitch),
        fov: l(a.fov, b.fov),
        diff: l3(a.diff, b.diff),
        position: l3(a.position, b.position),
        bumper: (l(a.bumper.0, b.bumper.0), l(a.bumper.1, b.bumper.1)),
        zoom: (l(a.zoom.0, b.zoom.0), l(a.zoom.1, b.zoom.1)),
    }
}

/// The camera following the player between frames: its look point and how far it has zoomed
/// out for movement. Read live (2026-11-13, `AREA_03_02`, `bumper_follow_speed` 5, the camera
/// object found by its vtable `0x14080c428`): the look point (+0x188) is the player's position
/// plus `diffPosition` every frame, with no trailing (+0xb0 holds the previous frame's position,
/// one frame behind +0xc0, the current one), and the zoom blend (+0x1a0) rises from 0 to 1 at
/// 1 / `zoom_time` a second while the player moves. What `bumper_follow_speed` and
/// `bumper_max_distance` change was not seen; the zoom's way back after a stop is a stand-in
/// (the same rate; the run showed it falling once and staying up once).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Follow {
    /// Where the camera's look point is (field units), once it has started.
    pub at: Option<[f32; 3]>,
    /// The zoom-out reached, 0 (none) to 1 (`zoom_distance`).
    pub zoom: f32,
}

impl Follow {
    /// One frame of `dt` seconds with the player at `player` (field units), moving or not,
    /// under setting `point`: the look point (the player's position) and the extra distance
    /// (field units).
    pub fn step(&mut self, player: [f32; 3], moving: bool, point: &CameraPoint, dt: f32) -> ([f32; 3], f32) {
        self.at = Some(player);
        let (time, distance) = point.zoom;
        let rate = if time > 0.0 { dt / time } else { 1.0 };
        self.zoom = (self.zoom + if moving { rate } else { -rate }).clamp(0.0, 1.0);
        (player, self.zoom * distance * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blends_along_the_area_axis() {
        // AREA_01_01's CLOSELY_Z area: 8 m, yaw 21° at z 30; 10 m, yaw 7° at z 45.
        let p = |length, yaw, z| CameraPoint { length, yaw, fov: 40.0, position: [0.0, 0.0, z], ..Default::default() };
        let points = vec![p(8.0, 21.0, 30.0), p(10.0, 7.0, 45.0)];
        let areas = vec![CameraArea { kind: 2, top_left: [-400.0, 0.0, -400.0], bottom_right: [400.0, 0.0, 400.0], points: 0..2 }];
        let at = |z| setting(&areas, &points, [40.0, 0.0, z]).unwrap();
        assert_eq!(at(20.0).length, 8.0);
        assert_eq!(at(50.0).yaw, 7.0);
        let mid = at(37.5);
        assert!((mid.length - 9.0).abs() < 1e-4 && (mid.yaw - 14.0).abs() < 1e-4);
        assert!(setting(&areas, &points, [500.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn the_look_point_stays_on_the_player_and_the_camera_zooms_out() {
        let point = CameraPoint { length: 5.0, yaw: 0.0, pitch: 0.0, fov: 40.0, diff: [0.0; 3], position: [0.0; 3], bumper: (5.0, 1.0), zoom: (2.0, 1.0) };
        let mut f = Follow::default();
        assert_eq!(f.step([0.0; 3], false, &point, 1.0 / 30.0), ([0.0; 3], 0.0));
        // A jump of 5 m: no trailing, as the game at bumper_follow_speed 5.
        assert_eq!(f.step([500.0, 0.0, 0.0], true, &point, 1.0 / 30.0).0, [500.0, 0.0, 0.0]);
        // Moving for a second: half the zoom (zoom_time 2).
        let mut g = Follow::default();
        let mut extra = 0.0;
        for _ in 0..30 {
            extra = g.step([0.0; 3], true, &point, 1.0 / 30.0).1;
        }
        assert!((extra - 50.0).abs() < 0.5, "{extra}");
    }
}
