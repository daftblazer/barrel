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
    /// `trueHDR_vExposureRange`, `fGamma`, `vSaturationScale` (per channel) and `fBloomWeight`:
    /// the set's own, not the override's (Rasenboden's noon: 0.9 to 2.4, 1.8, (1.27, 1.3,
    /// 1.2), 0.4).
    pub exposure_range: (f32, f32),
    pub gamma: f32,
    pub saturation: [f32; 3],
    pub bloom_weight: f32,
    /// `trueHDR_fLumiThreshold`: the bloom's bright-pass threshold (0.3; Rasenboden's evening 0.8).
    pub lumi_threshold: f32,
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
const EXPOSURE_RANGE: usize = 0x20;
const GAMMA: usize = 0x40;
const LUMI_THRESHOLD: usize = 0x18;
const SATURATION: usize = 0x44;
const BLOOM_WEIGHT: usize = 0x5c;
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

/// How the stage takes the set's light (`indirectStage_`, between `fog_` and `skyFog_` in the
/// file, laid out as `indirectChara_`): the diffuse light map's scale and colour, and the scale
/// on the set's ambient colour. Rasenboden's noon: 7, (0.1, 0.2602, 1) and 0, so its shade is
/// lit by the light map tinted deep blue (a colour fitted to the game's still without these
/// came out 0.8, 2, 6.5); the default override's are 2.5, (1.642, 1.978, 4) and 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageLight {
    pub diffuse_map: f32,
    pub diffuse_map_color: [f32; 3],
    pub ambient: f32,
}

const INDIRECT_STAGE: usize = 0x54c;

impl StageLight {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<StageLight, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[INDIRECT_STAGE + o..INDIRECT_STAGE + o + 4].try_into().unwrap());
        Ok(StageLight { diffuse_map: f(0), diffuse_map_color: [f(8), f(12), f(16)], ambient: f(0x30) })
    }

    /// The stage's ambient as the nine coefficients the renderer takes: the light map's times
    /// its scale and colour, the set's ambient colour at its scale added evenly.
    pub fn harmonics(&self, light: &Light, harmonics: &[[f32; 3]; 9]) -> [[f32; 3]; 9] {
        std::array::from_fn(|i| std::array::from_fn(|ch| harmonics[i][ch] * self.diffuse_map * self.diffuse_map_color[ch] + if i == 0 { light.ambient[ch] * self.ambient / 0.282_095 } else { 0.0 }))
    }
}

/// How the characters take the set's light (`indirectChara_`, right after `skyFog_` in the
/// file; the members by the executable's reader, docs/shading.md): the diffuse light map's
/// scale and colour, and the scales on the set's ambient colour and on the sun. Rasenboden's
/// noon: 1.5, (0.8, 1, 1), 0.2 and 0.8, the last two as read from the running game's
/// character lighting blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharaLight {
    pub diffuse_map: f32,
    pub diffuse_map_color: [f32; 3],
    pub ambient: f32,
    pub key: f32,
}

const INDIRECT_CHARA: usize = 0x5d0;

impl CharaLight {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<CharaLight, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[INDIRECT_CHARA + o..INDIRECT_CHARA + o + 4].try_into().unwrap());
        Ok(CharaLight { diffuse_map: f(0), diffuse_map_color: [f(8), f(12), f(16)], ambient: f(0x30), key: f(0x34) })
    }

    /// The flat ambient a character takes: the set's ambient colour and the light map's mean
    /// (`harmonics`' first coefficient), each at its scale.
    pub fn ambient_light(&self, light: &Light, harmonics: &[[f32; 3]; 9]) -> [f32; 3] {
        std::array::from_fn(|ch| light.ambient[ch] * self.ambient + harmonics[0][ch] * 0.282_095 * self.diffuse_map * self.diffuse_map_color[ch])
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
        Ok(Light {
            lumi_max: f(LUMI_MAX),
            middle_gray: f(MIDDLE_GRAY),
            exposure_range: (f(EXPOSURE_RANGE), f(EXPOSURE_RANGE + 4)),
            gamma: f(GAMMA),
            saturation: rgb(SATURATION),
            bloom_weight: f(BLOOM_WEIGHT),
            lumi_threshold: f(LUMI_THRESHOLD),
            ambient: rgb(MAIN_AMBIENT), parallel: [parallel(0), parallel(1)] })
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
        for (k, v) in [0.9, 2.4].into_iter().enumerate() {
            put(0x20 + 4 * k, v);
        }
        put(0x40, 2.2);
        for (k, v) in [1.2, 1.1, 1.0].into_iter().enumerate() {
            put(0x44 + 4 * k, v);
        }
        put(0x5c, 0.6);
        put(0x18, 0.8);
        for (k, v) in [5.0, 2.074, 1.0, 30.0, -30.0].into_iter().enumerate() {
            put(0x494 + 4 * k, v);
        }
        let l = Light::read(&env, Time::Evening, Weather::Sunny).unwrap();
        assert_eq!((l.lumi_max, l.middle_gray, l.ambient[0]), (4.2, 0.18, 2.0));
        assert_eq!((l.exposure_range, l.gamma, l.saturation, l.bloom_weight), ((0.9, 2.4), 2.2, [1.2, 1.1, 1.0], 0.6));
        assert_eq!(l.lumi_threshold, 0.8);
        assert_eq!(l.parallel[0], ParallelLight { color: [5.0, 2.074, 1.0], elevation: 30.0, azimuth: -30.0 });
        assert!(Light::read(&env[1..], Time::Noon, Weather::Sunny).is_err());
    }

    #[test]
    fn stage_light_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        let at = SET_SIZE + 0x54c;
        for (k, v) in [(0, 7.0f32), (8, 0.1), (12, 0.2602), (16, 1.0), (0x30, 0.25)] {
            env[at + k..at + k + 4].copy_from_slice(&v.to_le_bytes());
        }
        let st = StageLight::read(&env, Time::Noon, Weather::Sunny).unwrap();
        assert_eq!(st, StageLight { diffuse_map: 7.0, diffuse_map_color: [0.1, 0.2602, 1.0], ambient: 0.25 });
        let mut light = Light::read(&env, Time::Noon, Weather::Sunny).unwrap();
        light.ambient = [3.0, 3.0, 4.0];
        let mut h = [[0.0; 3]; 9];
        h[0] = [1.0; 3];
        h[1] = [2.0; 3];
        let out = st.harmonics(&light, &h);
        assert!((out[0][2] - (7.0 + 1.0 / 0.282_095)).abs() < 1e-4 && (out[1][0] - 1.4).abs() < 1e-5);
    }

    #[test]
    fn chara_light_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        // Noon, sunny: set 1.
        let at = SET_SIZE + 0x5d0;
        for (k, v) in [(0, 1.5f32), (8, 0.8), (12, 1.0), (16, 1.0), (0x30, 0.2), (0x34, 0.8)] {
            env[at + k..at + k + 4].copy_from_slice(&v.to_le_bytes());
        }
        let c = CharaLight::read(&env, Time::Noon, Weather::Sunny).unwrap();
        assert_eq!(c, CharaLight { diffuse_map: 1.5, diffuse_map_color: [0.8, 1.0, 1.0], ambient: 0.2, key: 0.8 });
        let mut light = Light::read(&env, Time::Noon, Weather::Sunny).unwrap();
        light.ambient = [3.0, 3.0, 4.0];
        let mut h = [[0.0; 3]; 9];
        h[0] = [1.0 / 0.282_095; 3];
        let a = c.ambient_light(&light, &h);
        assert!((a[0] - 1.8).abs() < 1e-5 && (a[2] - 2.3).abs() < 1e-5);
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

// STAND-IN: The rim table is taken as one entry per override value; the real thing is the table the game fills at run time.
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

/// The stage's ambient from an environment override: the diffuse light map is scaled by
/// `indirectStage_IDLScalerOfAmbientDiffuseMap` times `..ColorRGB` (the default: 2.5 × (1.642,
/// 1.978, 4.0), a blue tint), and the set's `mainAmbientLight` by
/// `indirectStage_AmbientLightScale` (0 in the default: unused). None if the override lacks
/// them.
pub fn stage_ambient(bytes: &[u8]) -> Option<([f32; 3], f32)> {
    let scale = override_number(bytes, "indirectStage_IDLScalerOfAmbientDiffuseMap").ok()?;
    let rgb = override_values(bytes, "indirectStage_IDLScalerOfAmbientDiffuseMapColorRGB").ok().filter(|v| v.len() == 3)?;
    let main = override_number(bytes, "indirectStage_AmbientLightScale").ok()?;
    Some(([rgb[0] * scale, rgb[1] * scale, rgb[2] * scale], main))
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
        let a = br#"<root indirectStage_IDLScalerOfAmbientDiffuseMap="2.5" indirectStage_IDLScalerOfAmbientDiffuseMapColorRGB="1.642000, 1.978000, 4.000000" indirectStage_AmbientLightScale="0"></root>"#;
        let (rgb, main) = super::stage_ambient(a).unwrap();
        assert!((rgb[0] - 4.105).abs() < 1e-4 && (rgb[2] - 10.0).abs() < 1e-4 && main == 0.0);
    }
}
