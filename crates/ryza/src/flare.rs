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
//! The numbers are the environment template's (`envtemplate.xml`, its `flare_paraffin` block: a
//! row per time of day and weather), read as the executable's `EnvFlareParaffin` reads them: by
//! its own key names (two are spelled `..._corve_center`, and the file writes `Paraffin_amount`,
//! which it does not match), the block's defaults standing in for any key a row leaves out.

use crate::environment::{Time, Weather};
use barrel_formats::table::Element;
use barrel_render::flare::{Layer, Settings};
use glam::{Mat4, Vec3};

/// `fShadowIntensityUpSpeed` and `fShadowIntensityDownSpeed`: the visibility's step a frame.
// STAND-IN: One sixtieth a frame, as the game's pass has it at Rasenboden's gate (0.0167); where the game takes it from (a frame time, or a setting) is not found.
pub const SPEED: f32 = 1.0 / 60.0;
/// `bReverseFlag`: 0 (the visibility rises in shadow).
// STAND-IN: 0 as the game's pass has it at the gate; its source is not found.
pub const REVERSE_FLAG: f32 = 0.0;

/// The environment template, whose `flare_paraffin` block holds the wash's settings.
pub const TEMPLATE_PATH: &str = "\\saves\\fieldmap\\cameraparam\\env\\envtemplate.xml";

/// One row of the template's `flare_paraffin` block: the two layers and the strengths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub flare: Layer,
    pub paraffin: Layer,
    /// `intensity`: the wash's strength over the scene.
    pub intensity: f32,
    /// `sky_intensity`: its strength over the sky (the game's second draw, where the depth is
    /// the far plane's).
    pub sky_intensity: f32,
}

impl Default for Look {
    /// The block's defaults (the reader's, `EnvFlareParaffin`): curve centres 0.5, coefficients
    /// -0.2 and 0.2, amounts 0.1 and 0.2, influence ratios 1, intensity 20.
    fn default() -> Look {
        // STAND-IN: The colours' defaults are set at start-up and not read; a zero weight leaves the picture as it is.
        let color = [0.0; 4];
        Look {
            flare: Layer { color, amount: 0.1, curve_center: 0.5, curve_coefficient: -0.2, shadow_influence: 1.0 },
            paraffin: Layer { color, amount: 0.2, curve_center: 0.5, curve_coefficient: 0.2, shadow_influence: 1.0 },
            intensity: 20.0,
            // STAND-IN: The sky's default is not read; the scene's intensity is taken.
            sky_intensity: 20.0,
        }
    }
}

/// The template's row for `time` in `weather` (the block's defaults if it has none).
pub fn look(bytes: &[u8], time: Time, weather: Weather) -> Result<Look, String> {
    let root = barrel_formats::table::parse(bytes).map_err(|e| format!("environment template: {e}"))?;
    let times = ["morning", "noon", "evening", "night"];
    let weathers = ["sunny", "cloudy", "foggy"];
    let mut rows: Vec<&Element> = vec![&root];
    let mut found = None;
    while let Some(e) = rows.pop() {
        if e.name == "flare_paraffin" {
            found = e.children.iter().find(|r| {
                r.attr("time").is_some_and(|t| t.eq_ignore_ascii_case(times[time as usize])) && r.attr("weather").is_some_and(|w| w.eq_ignore_ascii_case(weathers[weather as usize]))
            });
            break;
        }
        rows.extend(e.children.iter());
    }
    let mut l = Look::default();
    let Some(row) = found else { return Ok(l) };
    let numbers = |key: &str| -> Result<Option<Vec<f32>>, String> {
        row.attr(key).map(|v| v.split(',').map(|c| c.trim().parse::<f32>().map_err(|_| format!("environment template: {key} = {v}"))).collect()).transpose()
    };
    let one = |key: &str, into: &mut f32| -> Result<(), String> {
        if let Some(v) = numbers(key)?.and_then(|v| v.first().copied()) {
            *into = v;
        }
        Ok(())
    };
    // Colours are 0 to 255 with a weight, as the pass takes them over 255.
    let color = |key: &str, into: &mut [f32; 4]| -> Result<(), String> {
        if let Some(v) = numbers(key)? {
            let v: [f32; 4] = v.try_into().map_err(|_| format!("environment template: {key} is not four numbers"))?;
            *into = v.map(|c| c / 255.0);
        }
        Ok(())
    };
    color("flare_color", &mut l.flare.color)?;
    one("flare_amount", &mut l.flare.amount)?;
    one("flare_corve_center", &mut l.flare.curve_center)?;
    one("flare_curve_coefficient", &mut l.flare.curve_coefficient)?;
    one("flare_shadow_influence_ratio", &mut l.flare.shadow_influence)?;
    color("paraffin_color", &mut l.paraffin.color)?;
    one("paraffin_amount", &mut l.paraffin.amount)?;
    one("paraffin_corve_center", &mut l.paraffin.curve_center)?;
    one("paraffin_curve_coefficient", &mut l.paraffin.curve_coefficient)?;
    one("paraffin_shadow_influence_ratio", &mut l.paraffin.shadow_influence)?;
    one("intensity", &mut l.intensity)?;
    one("sky_intensity", &mut l.sky_intensity)?;
    Ok(l)
}

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

/// The frame's settings, `look` the template's row. `probe` is the point the sun's shadow is asked about and `cascade`
/// the first cascade's view-projection (both in render space); `view` the camera's view
/// matrix, `sun` the direction the sunlight travels (render space), `aspect` the screen's.
pub fn settings(look: &Look, probe: Vec3, cascade: Mat4, view: Mat4, sun: Vec3, aspect: f32) -> Settings {
    // Render space is the game's with x negated, and the picture is mirrored back by the
    // projection: the screen's right is the view's -x.
    let v = view.transform_vector3(-sun);
    let to_sun = Vec3::new(-v.x, v.y, v.z);
    Settings {
        reference: reference(cascade, probe),
        up_speed: SPEED,
        down_speed: SPEED,
        rotation: rotation(to_sun, aspect),
        flare: look.flare,
        paraffin: look.paraffin,
        intensity: look.intensity,
        sky_intensity: look.sky_intensity,
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

    #[test]
    fn the_template_row_by_time_and_weather_keys_as_the_game_spells_them() {
        let xml = br#"<?xml version="1.0" encoding="SHIFT-JIS"?>
<root><flare_paraffin>
<param time="Noon" weather="SUNNY" flare_color="30, 40, 50, 25" paraffin_color="50, 65, 60, 20" intensity="20.00" sky_intensity="15.00" flare_amount="0.6" Paraffin_amount="0.5"/>
<param time="Noon" weather="CLOUDY" flare_color="30, 40, 50, 25" paraffin_color="50, 65, 60, 20" intensity="15.00" flare_amount="0" Paraffin_amount="0.5"/>
</flare_paraffin></root>"#;
        let l = look(xml, Time::Noon, Weather::Sunny).unwrap();
        assert_eq!(l.flare.color, [30.0 / 255.0, 40.0 / 255.0, 50.0 / 255.0, 25.0 / 255.0]);
        assert_eq!((l.flare.amount, l.intensity, l.sky_intensity), (0.6, 20.0, 15.0));
        // `Paraffin_amount` is not the reader's key: the default stays.
        assert_eq!(l.paraffin.amount, 0.2);
        let c = look(xml, Time::Noon, Weather::Cloudy).unwrap();
        assert_eq!((c.flare.amount, c.intensity, c.sky_intensity), (0.0, 15.0, 20.0));
        assert_eq!(look(xml, Time::Night, Weather::Foggy).unwrap(), Look::default());
    }
}
