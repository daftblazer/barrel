//! A field's environment (`<field>_env.elixir.gz`, `docs/formats.md`, "Ryza field
//! environment"): its diffuse light cube map (`<FIELD>_ad.g1t`, HDR, six faces) as the nine
//! spherical-harmonic coefficients that reproduce it, for the renderer's ambient term.

use barrel_formats::g1t::G1t;

/// A cube face's direction for texel (u, v) in -1..1 (u right, v down), faces in the order
/// +x, -x, +y, -y, +z, -z (Direct3D's).
fn direction(face: usize, u: f32, v: f32) -> [f32; 3] {
    match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    }
}

/// The real spherical harmonics up to band 2 at unit direction (x, y, z).
pub fn basis([x, y, z]: [f32; 3]) -> [f32; 9] {
    [
        0.282_095,
        0.488_603 * y,
        0.488_603 * z,
        0.488_603 * x,
        1.092_548 * x * y,
        1.092_548 * y * z,
        0.315_392 * (3.0 * z * z - 1.0),
        1.092_548 * x * z,
        0.546_274 * (x * x - y * y),
    ]
}

/// The diffuse light map, as nine RGB coefficients over the cube map's own axes (the game's:
/// y up). The map already holds the light a surface facing each way receives, so it is
/// projected as it is (no cosine lobe applied). Also each face's mean, for checking the axes.
pub fn diffuse_harmonics(ad: &[u8]) -> Result<([[f32; 3]; 9], [[f32; 3]; 6]), String> {
    let g = G1t::parse(ad).map_err(|e| format!("diffuse light map: {e}"))?;
    let t = g.textures.first().ok_or("diffuse light map: no texture")?;
    let (w, h) = (t.width as usize, t.height as usize);
    let mut c = [[0f32; 3]; 9];
    let mut means = [[0f32; 3]; 6];
    let mut total = 0f32;
    for (face, mean) in means.iter_mut().enumerate() {
        let px = t.layer_rgb(face as u32).ok_or("diffuse light map: not a six-face HDR cube map")?;
        for (i, p) in px.iter().enumerate() {
            let (u, v) = (2.0 * ((i % w) as f32 + 0.5) / w as f32 - 1.0, 2.0 * ((i / w) as f32 + 0.5) / h as f32 - 1.0);
            let d = direction(face, u, v);
            let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            // The texel's solid angle, up to a constant (normalised below).
            let weight = 1.0 / (len2 * len2.sqrt());
            let n = d.map(|x| x / len2.sqrt());
            for (k, y) in basis(n).iter().enumerate() {
                for ch in 0..3 {
                    c[k][ch] += p[ch] * y * weight;
                }
            }
            total += weight;
            for ch in 0..3 {
                mean[ch] += p[ch] / px.len() as f32;
            }
        }
    }
    let norm = 4.0 * std::f32::consts::PI / total;
    Ok((c.map(|k| k.map(|x| x * norm)), means))
}

/// The environment file's sets (`<FIELD>_env.bin`): 12 of [`SET_SIZE`] bytes, set = weather × 4
/// + time (`docs/formats.md`, "Ryza field environment").
pub const SET_SIZE: usize = 0x1fe0;
pub const SETS: usize = 12;

/// The time of day, as the sets are numbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Time {
    Morning,
    Noon,
    Evening,
    Night,
}

/// The weather, as the sets are numbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    Sunny,
    Cloudy,
    Foggy,
}

/// A directional light (`EnvDataParallelLight`, 0x18 bytes): an HDR colour, where it comes
/// from (degrees above the horizon, and around), and a word called `extension`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParallelLight {
    pub color: [f32; 3],
    pub elevation: f32,
    pub azimuth: f32,
}

/// What one set says of the field's light (the parts read so far).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// `trueHDR_fLumiMax` and `fMiddleGray`.
    pub lumi_max: f32,
    pub middle_gray: f32,
    /// `mainAmbientLight_color`.
    pub ambient: [f32; 3],
    /// `parallelLight00` (the sun) and `parallelLight01`.
    pub parallel: [ParallelLight; 2],
}

/// The set's offsets of what [`Light`] reads. The executable's reader stores the blocks 0x44
/// bytes further on (`mainAmbientLight_` at 0x4c8, `parallelLight%02d_` at 0x4d8) than the
/// file has them; where the two start to differ is not placed yet.
const LUMI_MAX: usize = 0x00;
const MIDDLE_GRAY: usize = 0x1c;
const MAIN_AMBIENT: usize = 0x484;
const PARALLEL: usize = 0x494;
const PARALLEL_SIZE: usize = 0x18;

/// The set for `time` in `weather` from a whole `_env.bin`.
fn set_of(env: &[u8], time: Time, weather: Weather) -> Result<&[u8], String> {
    if env.len() != SET_SIZE * SETS {
        return Err(format!("environment: {} bytes, not {} sets of {SET_SIZE}", env.len(), SETS));
    }
    Ok(&env[(weather as usize * 4 + time as usize) * SET_SIZE..][..SET_SIZE])
}

/// Where a set names its sky dome (`SkyDome_`'s `SkyDomeDisplaySetFactory`, structure
/// +0x41c in the executable): a zero-ended name, at most 64 bytes.
const SKY_DOME: usize = 0x3d8;
const SKY_DOME_LEN: usize = 0x40;

/// The sky dome model the set shows, as its container in the archives
/// (`\data\x64\field\skydome\<name>.elixir.gz`, the name lower-cased); none when the set
/// names none (most sets: only the field's usual ones are filled in).
pub fn sky_dome(env: &[u8], time: Time, weather: Weather) -> Result<Option<String>, String> {
    let set = set_of(env, time, weather)?;
    let raw = &set[SKY_DOME..SKY_DOME + SKY_DOME_LEN];
    let name = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())];
    if name.is_empty() {
        return Ok(None);
    }
    let name = std::str::from_utf8(name).map_err(|_| "environment: the sky dome's name is not text".to_string())?;
    Ok(Some(format!("\\data\\x64\\field\\skydome\\{}.elixir.gz", name.to_ascii_lowercase())))
}

/// One of a set's two scattering blocks (`fog_` for the scene, `skyFog_` for the sky): the
/// executable's `EnvDataFog` reader, members `start`, `end`, `mie`, `g`, `rayleigh`, `power`,
/// `ambient` (names from the environment XMLs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub start: f32,
    pub end: f32,
    pub mie: [f32; 3],
    /// The Mie phase function's asymmetry.
    pub g: f32,
    pub rayleigh: [f32; 3],
    pub power: f32,
    pub ambient: f32,
}

/// Where the set holds `fog_` and `skyFog_` (the file is laid out tighter than the
/// executable's structure, where they are at 0xd4c and 0x1604).
const FOG: usize = 0x4fc;
const SKY_FOG: usize = 0x5a4;

impl Fog {
    fn at(set: &[u8], o: usize) -> Fog {
        let f = |k: usize| f32::from_le_bytes(set[o + 4 * k..o + 4 * k + 4].try_into().unwrap());
        Fog { start: f(0), end: f(1), mie: [f(2), f(3), f(4)], g: f(5), rayleigh: [f(6), f(7), f(8)], power: f(9), ambient: f(10) }
    }

    /// The scene's (`fog_`) and the sky's (`skyFog_`) scattering for `time` in `weather`.
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<(Fog, Fog), String> {
        let set = set_of(env, time, weather)?;
        Ok((Fog::at(set, FOG), Fog::at(set, SKY_FOG)))
    }
}

impl Light {
    /// The light of `time` in `weather` from a whole `_env.bin`.
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<Light, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes([set[o], set[o + 1], set[o + 2], set[o + 3]]);
        let rgb = |o: usize| [f(o), f(o + 4), f(o + 8)];
        let parallel = |k: usize| {
            let at = PARALLEL + PARALLEL_SIZE * k;
            ParallelLight { color: rgb(at), elevation: f(at + 12), azimuth: f(at + 16) }
        };
        Ok(Light { lumi_max: f(LUMI_MAX), middle_gray: f(MIDDLE_GRAY), ambient: rgb(MAIN_AMBIENT), parallel: [parallel(0), parallel(1)] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        // Evening, sunny: set 2.
        let set = 2 * SET_SIZE;
        let mut put = |o: usize, v: f32| env[set + o..set + o + 4].copy_from_slice(&v.to_le_bytes());
        put(0, 4.2);
        put(0x1c, 0.18);
        put(0x484, 2.0);
        for (k, v) in [5.0, 2.074, 1.0, 30.0, -30.0].into_iter().enumerate() {
            put(0x494 + 4 * k, v);
        }
        let l = Light::read(&env, Time::Evening, Weather::Sunny).unwrap();
        assert_eq!((l.lumi_max, l.middle_gray, l.ambient[0]), (4.2, 0.18, 2.0));
        assert_eq!(l.parallel[0], ParallelLight { color: [5.0, 2.074, 1.0], elevation: 30.0, azimuth: -30.0 });
        assert!(Light::read(&env[1..], Time::Noon, Weather::Sunny).is_err());
    }

    #[test]
    fn fog_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        // Rasenboden's sunny morning sky.
        for (k, v) in [1453.7f32, 400000.0, 4.0, 3.414, 2.535, 0.984, 1.5, 1.983, 3.0, 10.0, 1.0].into_iter().enumerate() {
            env[SKY_FOG + 4 * k..SKY_FOG + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        let (_, sky) = Fog::read(&env, Time::Morning, Weather::Sunny).unwrap();
        assert_eq!(sky, Fog { start: 1453.7, end: 400000.0, mie: [4.0, 3.414, 2.535], g: 0.984, rayleigh: [1.5, 1.983, 3.0], power: 10.0, ambient: 1.0 });
    }

    #[test]
    fn sky_dome_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        env[SKY_DOME..SKY_DOME + 16].copy_from_slice(b"MORNING_SUNNY_02");
        assert_eq!(sky_dome(&env, Time::Morning, Weather::Sunny).unwrap().as_deref(), Some("\\data\\x64\\field\\skydome\\morning_sunny_02.elixir.gz"));
        assert_eq!(sky_dome(&env, Time::Noon, Weather::Sunny).unwrap(), None);
    }

    #[test]
    fn harmonics_of_a_constant() {
        // A constant light gives only the first coefficient: 1 × sqrt(4π).
        let b = basis([0.0, 1.0, 0.0]);
        assert!((b[0] - 0.282_095).abs() < 1e-6 && (b[1] - 0.488_603).abs() < 1e-6);
    }
}

/// The characters' rim light from an environment override (`saves\fieldmap\override_env\
/// env_<name>.xml`, one `<root>` of `group_name="value"` attributes; `env_default.xml` for a
/// field with none): `ir_rimSharpness` (the power on `1 - N·V`), `ir_rimScaler` times
/// `ir_rimColorScaler` (its strength), and the hair's (`ir_hairRim*`). `DSChara-Cll`'s rim table
/// (`cRimTbl`, docs/shading.md) is filled from them at run time: taken as one entry each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rim {
    pub sharpness: f32,
    pub strength: f32,
    pub hair_sharpness: f32,
    pub hair_strength: f32,
}

impl Rim {
    /// The override file's path in the archives.
    pub const DEFAULT_PATH: &'static str = "\\saves\\fieldmap\\override_env\\env_default.xml";

    pub fn parse(bytes: &[u8]) -> Result<Rim, String> {
        let num = |k: &str| override_number(bytes, k);
        Ok(Rim {
            sharpness: num("ir_rimSharpness")?,
            strength: num("ir_rimScaler")? * num("ir_rimColorScaler")?,
            hair_sharpness: num("ir_hairRimSharpness")?,
            hair_strength: num("ir_hairRimScaler")? * num("ir_hairRimColorScaler")?,
        })
    }
}

/// One number from an environment override (its first component for a vector).
pub fn override_number(bytes: &[u8], key: &str) -> Result<f32, String> {
    override_values(bytes, key)?.first().copied().ok_or(format!("override env: {key} empty"))
}

/// A value's components from an environment override ("0.900000, 1.200000").
pub fn override_values(bytes: &[u8], key: &str) -> Result<Vec<f32>, String> {
    let root = barrel_formats::table::parse(bytes).map_err(|e| format!("override env: {e}"))?;
    let e = if root.name == "root" { &root } else { root.children.iter().find(|c| c.name == "root").ok_or("override env: no root")? };
    let v = e.attr(key).ok_or(format!("override env: no {key}"))?;
    v.split(',').map(|c| c.trim().parse::<f32>().map_err(|_| format!("override env: {key} = {v}"))).collect()
}

#[cfg(test)]
mod rim_tests {
    #[test]
    fn default_rim() {
        let xml = br#"<root ver="100" ir_rimScaler="15" ir_rimSharpness="10" ir_rimColorScaler="0.100000, 0.100000, 0.100000" ir_hairRimScaler="15" ir_hairRimSharpness="10" ir_hairRimColorScaler="0.000000, 0.000000, 0.000000"></root>"#;
        let r = super::Rim::parse(xml).unwrap();
        assert_eq!((r.sharpness, r.hair_sharpness, r.hair_strength), (10.0, 10.0, 0.0));
        assert!((r.strength - 1.5).abs() < 1e-5);
        let s = br#"<root trueHDR_vSaturationScale="1.200000, 1.200000, 1.200000"></root>"#;
        assert_eq!(super::override_number(s, "trueHDR_vSaturationScale"), Ok(1.2));
        let r = br#"<root trueHDR_vExposureRange="0.900000, 1.200000"></root>"#;
        assert_eq!(super::override_values(r, "trueHDR_vExposureRange"), Ok(vec![0.9, 1.2]));
    }
}
