//! How a field is lit (`Saves/field/<FIELD>.xml`, besides its camera): the character shader's
//! light and rim light, the scene's lights, fog and HDR. Distances in these tables are metres
//! (100 field units); angles degrees.
//!
//! ```text
//! <fog start end ambient mieG rayleighPow mieRGB rayleighRGB />
//! <light ambient="r, g, b, a">
//!     <param type="PARALLEL" rot="x, y, z" color />
//!     <param type="POINT" pos color ambient att0 att1 att2 range />
//! </light>
//! <chara_shader light_dir light_color rim_light_dir rim_light_color point_light_strength />
//! <hdr threshold glow_add_rate />
//! ```

use crate::table::Element;

/// The character shader's constants (`docs/shading.md`): the directions as given (not
/// normalised), the colours with their weight in the fourth value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharaShader {
    pub light_dir: [f32; 3],
    pub light_color: [f32; 4],
    pub rim_light_dir: [f32; 3],
    pub rim_light_color: [f32; 4],
    pub point_light_strength: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Light {
    /// A directional light turned by `rot` (degrees about x, y, z).
    Parallel { rot: [f32; 3], color: [f32; 4] },
    /// A point light: `pos` and `range` in metres, `att` the shader's attenuation over field
    /// units, 1 / (att0 + att1·d + att2·d²) (see [`point_attenuation`]).
    Point { pos: [f32; 3], color: [f32; 4], ambient: [f32; 4], att: [f32; 3], range: f32 },
}

/// A `POINT`'s attenuation as the field loader makes it (`0x140068cc4` on): `att0` and `att1`
/// as written, but the table's `att2` is not the quadratic term. It is how much of the light is
/// left at the range (`fall`, 0.1 when not given): the quadratic term is chosen so that,
/// the linear term left out, 1 / (att0 + att2·R²) = fall, R the range in field units. A missing
/// `att0` is 1, `att1` 0.001 and `range` 1 m. `q54`'s point lights are made the same way.
pub fn point_attenuation(att0: f32, att1: f32, fall: f32, range: f32) -> [f32; 3] {
    let fall = fall + f32::EPSILON;
    let r = range * 100.0;
    [att0, att1, (1.0 - fall * att0) / (fall * r * r)]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub start: f32,
    pub end: f32,
    pub ambient: f32,
    pub mie_g: f32,
    pub rayleigh_pow: f32,
    pub mie_rgb: [f32; 4],
    pub rayleigh_rgb: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldLook {
    pub chara: Option<CharaShader>,
    pub ambient: Option<[f32; 4]>,
    pub lights: Vec<Light>,
    pub fog: Option<Fog>,
    /// `<hdr threshold glow_add_rate>`.
    pub hdr: Option<(f32, f32)>,
    /// `<sphere offset>`: where the distant view sits from the camera (metres), when it
    /// follows the camera.
    pub sphere_offset: Option<[f32; 3]>,
    /// `<camera><area><point>`: the play camera's settings, in table order (how the game picks
    /// and blends them is not read yet).
    pub camera_points: Vec<CameraPoint>,
    /// `<camera><area>`: each area's kind and rectangle (`tlPosition`, `brPosition`, metres) and
    /// the indices of its points in `camera_points`.
    pub camera_areas: Vec<CameraArea>,
}

/// A play-camera setting of a field (`<point>`), as `CameraCFieldMap` takes it (`0x1401e71ad`
/// on): `length` metres from what it looks at (its distance, +0x98), `radian` (degrees, despite
/// the name) its yaw (+0x20, the eye builder's turn about y, `0x1401e68f0`), `lookRad` its
/// pitch (+0x80), `fov` in degrees (+0x68), `diff` (metres) added to the player's place to give
/// what it looks at, `position` where along its area's axis it applies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraArea {
    /// Its `tag` (`FIELDMAP_CAMERA_DATA_*`, parsed at `0x140067f58`): `CLOSELY_X` 0,
    /// `CLOSELY_Y` 1, `CLOSELY_Z` 2, `CLOSELY_XZ` 3, `VERTEX_LENGTH` 4.
    pub kind: u32,
    pub top_left: [f32; 3],
    pub bottom_right: [f32; 3],
    pub points: std::ops::Range<usize>,
}

/// The area kinds' tag names, by number.
pub const CAMERA_AREA_TAGS: [&str; 5] = ["CLOSELY_X", "CLOSELY_Y", "CLOSELY_Z", "CLOSELY_XZ", "VERTEX_LENGTH"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraPoint {
    pub length: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub diff: [f32; 3],
    pub position: [f32; 3],
    /// `bumper_follow_speed`, `bumper_max_distance`, `zoom_time`, `zoom_distance`: the setting's
    /// +0x30 to +0x3c, which the camera keeps as tweens at its +0x150, +0x168, +0x1b0 and +0x1c8
    /// (`0x1401e7321`); what moves them is not read. Missing, they are -1, 1, 2 and 0 (and the
    /// field of view 22.5; `0x14006d7e0` builds a setting, angles in radians).
    pub bumper: (f32, f32),
    pub zoom: (f32, f32),
}

/// Each field's distant view (sky, far ground, clouds: `Data/x64/Field/C_SPHERE/<set>`), by
/// field name, from the English executable's table at `0x14076eff0`: 32-byte records `{u64
/// field name, u64 set file name, f32 × 4}`. Indoor fields have `set-black`. The game copies the
/// table into the field object (+0x1a8); the first float (75 in all but a few) is the steepest
/// slope in degrees a character walks, taken against its `CharaBase` `FMslp` (the larger wins,
/// `0x14026c3c0`); the other three (1, 0, 1 in most; up to 100, 4, 120) are not read.
pub const SKY_SETS_EN: u64 = 0x1407_6eff0;

/// The screen fades' presets (`u22`, `ECT_EVENT_CONTRAST_SET_MIX`): 21 pairs of colours (with
/// opacity) a fade goes from and to, at `0x1407272b0` in the English executable
/// (`0x14003ce20` picks one; preset 2 fades to black, 3 back from it, 0 and 1 the same with
/// white).
pub const FADE_PRESETS_EN: u64 = 0x1407_272b0;

pub fn fade_presets(pe: &crate::pe::Pe, table: u64) -> Vec<([f32; 4], [f32; 4])> {
    let Some(at) = pe.offset(table) else { return Vec::new() };
    let f = |o: usize| pe.data.get(o..o + 4).map(|b| f32::from_le_bytes(b.try_into().unwrap()));
    (0..21)
        .map_while(|k| {
            let r = at + 32 * k;
            let v: Vec<f32> = (0..8).map(|i| f(r + 4 * i)).collect::<Option<_>>()?;
            Some(([v[0], v[1], v[2], v[3]], [v[4], v[5], v[6], v[7]]))
        })
        .collect()
}

pub fn sky_sets(pe: &crate::pe::Pe, table: u64) -> Vec<(String, String)> {
    let d = pe.data;
    let Some(at) = pe.offset(table) else { return Vec::new() };
    let text = |va: u64| -> Option<String> {
        let o = pe.offset(va)?;
        let end = d.get(o..)?.iter().take(64).position(|&b| b == 0)?;
        String::from_utf8(d[o..o + end].to_vec()).ok()
    };
    let q = |o: usize| d.get(o..o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
    let mut out = Vec::new();
    for k in 0..512 {
        let r = at + 32 * k;
        let (Some(field), Some(set)) = (q(r).and_then(text), q(r + 8).and_then(text)) else { break };
        if !set.starts_with("set-") {
            break;
        }
        out.push((field, set));
    }
    out
}

fn numbers<const N: usize>(e: &Element, name: &str) -> Option<[f32; N]> {
    let v: Vec<f32> = e.attr(name)?.split(',').map(|s| s.trim().parse().ok()).collect::<Option<_>>()?;
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(&v) {
        *o = *x;
    }
    (v.len() >= N.min(3)).then_some(out)
}

fn number(e: &Element, name: &str) -> Option<f32> {
    e.attr(name)?.trim().parse().ok()
}

/// The look of a field from its table's root.
pub fn read(root: &Element) -> FieldLook {
    let mut look = FieldLook::default();
    for e in &root.children {
        match e.name.as_str() {
            "chara_shader" => {
                look.chara = (|| {
                    Some(CharaShader {
                        light_dir: numbers(e, "light_dir")?,
                        light_color: numbers(e, "light_color")?,
                        rim_light_dir: numbers(e, "rim_light_dir")?,
                        rim_light_color: numbers(e, "rim_light_color")?,
                        point_light_strength: numbers::<4>(e, "point_light_strength").map_or(0.0, |v| v[0]),
                    })
                })()
            }
            "light" => {
                look.ambient = numbers(e, "ambient");
                for p in e.children.iter().filter(|p| p.name == "param") {
                    let color = numbers(p, "color").unwrap_or([1.0; 4]);
                    match p.attr("type") {
                        Some("PARALLEL") => look.lights.push(Light::Parallel { rot: numbers(p, "rot").unwrap_or_default(), color }),
                        Some("POINT") => look.lights.push(Light::Point {
                            pos: numbers(p, "pos").unwrap_or_default(),
                            color,
                            ambient: numbers(p, "ambient").unwrap_or_default(),
                            att: point_attenuation(
                                number(p, "att0").unwrap_or(1.0),
                                number(p, "att1").unwrap_or(0.001),
                                number(p, "att2").unwrap_or(0.1),
                                number(p, "range").unwrap_or(1.0),
                            ),
                            range: number(p, "range").unwrap_or(1.0),
                        }),
                        _ => {}
                    }
                }
            }
            "fog" => {
                look.fog = Some(Fog {
                    start: number(e, "start").unwrap_or(0.0),
                    end: number(e, "end").unwrap_or(0.0),
                    ambient: number(e, "ambient").unwrap_or(0.0),
                    mie_g: number(e, "mieG").unwrap_or(0.0),
                    rayleigh_pow: number(e, "rayleighPow").unwrap_or(0.0),
                    mie_rgb: numbers(e, "mieRGB").unwrap_or_default(),
                    rayleigh_rgb: numbers(e, "rayleighRGB").unwrap_or_default(),
                })
            }
            "hdr" => look.hdr = Some((number(e, "threshold").unwrap_or(1.0), number(e, "glow_add_rate").unwrap_or(0.0))),
            "sphere" if e.attr("tracking_camera") != Some("false") => look.sphere_offset = numbers(e, "offset"),
            "camera" => {
                for a in e.children.iter().filter(|a| a.name == "area") {
                    let first = look.camera_points.len();
                    for p in a.children.iter().filter(|p| p.name == "point") {
                        look.camera_points.push(CameraPoint {
                            length: number(p, "length").unwrap_or(8.0),
                            yaw: number(p, "radian").unwrap_or(0.0),
                            pitch: number(p, "lookRad").unwrap_or(20.0),
                            fov: number(p, "fov").unwrap_or(22.5),
                            diff: numbers(p, "diffPosition").unwrap_or_default(),
                            position: numbers(p, "position").unwrap_or_default(),
                            bumper: (number(p, "bumper_follow_speed").unwrap_or(-1.0), number(p, "bumper_max_distance").unwrap_or(1.0)),
                            zoom: (number(p, "zoom_time").unwrap_or(2.0), number(p, "zoom_distance").unwrap_or(0.0)),
                        });
                    }
                    let tag = a.attr("tag").unwrap_or("");
                    look.camera_areas.push(CameraArea {
                        kind: CAMERA_AREA_TAGS.iter().position(|t| *t == tag).unwrap_or(0) as u32,
                        top_left: numbers(a, "tlPosition").unwrap_or_default(),
                        bottom_right: numbers(a, "brPosition").unwrap_or_default(),
                        points: first..look.camera_points.len(),
                    });
                }
            }
            _ => {}
        }
    }
    look
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn town_11() {
        let xml = r#"<root>
            <fog start="19.0" end="50.0" ambient="1.0" mieG="0.5" rayleighPow="20.0" mieRGB="0.0, 0.2, 0.0, 1.0" rayleighRGB="0.1, 0.1, 0.7, 1.0" />
            <light ambient="0.45, 0.5, 0.3, 1.0">
                <param type="PARALLEL" rot="40.0, 270.0, 0.0" color="0.7, 0.5, 0.4, 1.0" />
                <param type="POINT" pos="17.0, 2.0,  5.5" color="0.5, 0.0, 0.0, 1.0" ambient="1.0, 1.0, 0.0, 1.0" att0="0.01" att1="0.01" att2="0.02" range="7.5" />
            </light>
            <chara_shader light_dir="2.0, -1.0, -1.0" light_color="0.65, 0.4, 0.3, 0.3" rim_light_dir="2.56, -2.67, 1.0" rim_light_color="0.9, 0.9, 0.0, 0.25" point_light_strength="0.1, 0.0, 0.0, 0.0" />
            <hdr threshold="0.7" glow_add_rate="1.0" />
        </root>"#;
        let look = read(&crate::table::parse_str(xml).unwrap());
        let c = look.chara.unwrap();
        assert_eq!((c.light_dir, c.light_color[3], c.point_light_strength), ([2.0, -1.0, -1.0], 0.3, 0.1));
        assert_eq!(look.lights.len(), 2);
        assert!(matches!(look.lights[1], Light::Point { range: 7.5, .. }));
        // att2 0.02 is the light left at the 7.5 m range.
        let Light::Point { att, .. } = look.lights[1] else { unreachable!() };
        assert_eq!(&att[..2], &[0.01, 0.01]);
        let r = 750.0f32;
        assert!((1.0 / (att[0] + att[2] * r * r) - 0.02).abs() < 1e-6);
        assert_eq!(look.fog.unwrap().end, 50.0);
        assert_eq!(look.hdr, Some((0.7, 1.0)));
    }
}
