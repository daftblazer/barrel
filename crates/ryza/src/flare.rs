//! The sun flare and paraffin wash of Ryza's fields (the game's `PostEffectFlareParaffin`,
//! `docs/renderer.md`, "The sun flare"): the settings Barrel's renderer is fed each frame, from
//! the sun and the view.
//!
//! What the game does, as far as it is read: two screen quads after the temporal anti-aliasing.
//! The first (the visibility) asks the first shadow cascade about one point near the player and
//! eases a one-pixel value toward 1 while the point is in shadow, toward 0 while it is lit. The
//! second lays a warm flare along the top of the screen, along a curve tilted toward the sun,
//! and a paraffin tint where the sun is not blocked.
//!
//! The lighting sets hold none of these numbers (all twelve of Rasenboden's carry no lens-flare
//! values), and no other file was found that does; the executable's reader of the block
//! (`EnvFlareParaffin`) has defaults for some, which are used where they agree with the
//! capture, and the rest are stand-ins.

use barrel_render::flare::{Layer, Settings};
use glam::{Mat4, Vec3};

/// `fShadowIntensityUpSpeed` and `fShadowIntensityDownSpeed`: the visibility's step a frame.
// STAND-IN: One sixtieth a frame, as the game's pass has it at Rasenboden's gate (0.0167); where the game takes it from (a frame time, or a setting) is not found.
pub const SPEED: f32 = 1.0 / 60.0;
/// `bReverseFlag`: 0 (the visibility rises in shadow).
// STAND-IN: 0 as the game's pass has it at the gate; its source is not found.
pub const REVERSE_FLAG: f32 = 0.0;

/// The flare's colour (0 to 255 and a weight): `vFlareColor` as the game's pass has it, over 255.
// STAND-IN: The colour and weight the game's pass has at Rasenboden's gate (30, 40, 50 and 25); the lighting sets do not hold them and the executable's default is set at start-up (not read).
const FLARE_COLOR: [f32; 4] = [30.0 / 255.0, 40.0 / 255.0, 50.0 / 255.0, 25.0 / 255.0];
/// `vParaffinColor`: the same for the paraffin tint.
// STAND-IN: As above (50, 65, 60 and 20).
const PARAFFIN_COLOR: [f32; 4] = [50.0 / 255.0, 65.0 / 255.0, 60.0 / 255.0, 20.0 / 255.0];
/// `fFlareAmount`: how far down the screen the flare reaches.
// STAND-IN: The game's pass has 0.6 at Rasenboden's gate; the executable's default is 0.1, and the sets hold no value.
const FLARE_AMOUNT: f32 = 0.6;
/// `fIntensity`: the flare's strength. The executable's default of the setting is 20.
// STAND-IN: 20, from the picture: the game's final image has the flare at 20 (the second draw's recorded constant says 15, the first's 20; see `barrel-parity`'s `OVERWRITTEN`); the sets hold no value.
const INTENSITY: f32 = 20.0;

/// The curves and the shadow influence: the executable's defaults for the block (`flare` curve
/// centre 0.5 and coefficient -0.2, `paraffin` amount 0.2, centre 0.5 and coefficient 0.2, both
/// influence ratios 1), which are also what the capture has.
const FLARE_CENTER: f32 = 0.5;
const FLARE_COEFFICIENT: f32 = -0.2;
const PARAFFIN_AMOUNT: f32 = 0.2;
const PARAFFIN_CENTER: f32 = 0.5;
const PARAFFIN_COEFFICIENT: f32 = 0.2;
const SHADOW_INFLUENCE: f32 = 1.0;

/// The screen's tilt toward the sun as the 2x2 of the game's `mRotation` (row 0, then row 1).
///
/// `to_sun` is the direction to the sun in the view (`x` right, `y` up on the screen) and
/// `aspect` the screen's width over its height. The angle is that of the sun's direction on the
/// screen measured from straight up, with the screen's height and width taken as equal lengths
/// (the uv square): `atan(-x / (y * aspect))`.
// STAND-IN: The angle is fitted to three captured views (Rasenboden at noon, the camera turned about; the fit holds to 0.2 percent, at the aspect 16:9); the game's own rule is not read.
pub fn rotation(to_sun: Vec3, aspect: f32) -> [f32; 4] {
    let angle = (-to_sun.x).atan2(to_sun.y * aspect);
    let (s, c) = angle.sin_cos();
    [c, s, -s, c]
}

/// The point `world` (render space) in the first cascade's map as the game's pass takes it:
/// the uv and the depth, then 1.
pub fn reference(cascade: Mat4, world: Vec3) -> [f32; 4] {
    let p = cascade.project_point3(world);
    [p.x * 0.5 + 0.5, p.y * 0.5 + 0.5, p.z, 1.0]
}

/// The frame's settings. `probe` is the point the sun's shadow is asked about and `cascade`
/// the first cascade's view-projection (both in render space); `view` the camera's view
/// matrix, `sun` the direction the sunlight travels (render space), `aspect` the screen's.
pub fn settings(probe: Vec3, cascade: Mat4, view: Mat4, sun: Vec3, aspect: f32) -> Settings {
    // Render space is the game's with x negated, and the picture is mirrored back by the
    // projection: the screen's right is the view's -x.
    let v = view.transform_vector3(-sun);
    let to_sun = Vec3::new(-v.x, v.y, v.z);
    Settings {
        reference: reference(cascade, probe),
        up_speed: SPEED,
        down_speed: SPEED,
        rotation: rotation(to_sun, aspect),
        flare: Layer { color: FLARE_COLOR, amount: FLARE_AMOUNT, curve_center: FLARE_CENTER, curve_coefficient: FLARE_COEFFICIENT, shadow_influence: SHADOW_INFLUENCE },
        paraffin: Layer { color: PARAFFIN_COLOR, amount: PARAFFIN_AMOUNT, curve_center: PARAFFIN_CENTER, curve_coefficient: PARAFFIN_COEFFICIENT, shadow_influence: SHADOW_INFLUENCE },
        intensity: INTENSITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sun_straight_up_does_not_tilt() {
        assert_eq!(rotation(Vec3::new(0.0, 1.0, 0.0), 16.0 / 9.0), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn the_tilt_follows_the_suns_side_with_the_aspect() {
        // The game's three captured views (the sun's x and y in the view, the tilt's sine).
        for (x, y, sine) in [(-0.364f32, 0.785f32, 0.2527f32), (-0.284, 0.610, 0.2531), (0.343, 0.786, -0.2381)] {
            let r = rotation(Vec3::new(x, y, 0.5), 16.0 / 9.0);
            assert!((r[1] - sine).abs() < 2e-3, "{r:?} vs {sine}");
            assert_eq!(r[1], -r[2]);
        }
    }

    #[test]
    fn a_point_at_the_maps_centre_is_at_half() {
        let r = reference(Mat4::IDENTITY, Vec3::new(0.0, 0.0, 0.5));
        assert_eq!(r, [0.5, 0.5, 0.5, 1.0]);
    }
}
