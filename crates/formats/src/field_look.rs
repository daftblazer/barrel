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
    Point { pos: [f32; 3], color: [f32; 4], ambient: [f32; 4], att: [f32; 3], range: f32 },
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
}

/// Each field's distant view (sky, far ground, clouds: `Data/x64/Field/C_SPHERE/<set>`), by
/// field name, from the English executable's table at `0x14076eff0`: 32-byte records `{u64
/// field name, u64 set file name, f32 × 4 (not read)}`. Indoor fields have `set-black`.
pub const SKY_SETS_EN: u64 = 0x1407_6eff0;

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
                            att: [number(p, "att0").unwrap_or(0.0), number(p, "att1").unwrap_or(0.0), number(p, "att2").unwrap_or(0.0)],
                            range: number(p, "range").unwrap_or(0.0),
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
        assert_eq!(look.fog.unwrap().end, 50.0);
        assert_eq!(look.hdr, Some((0.7, 1.0)));
    }
}
