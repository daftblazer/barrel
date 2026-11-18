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
    /// `trueHDR_vExposureRange`, `fGamma`, the colour scale (per channel: the last pass's
    /// `vColorScale`, laid on before the tone curve) and `fBloomWeight`:
    /// the set's own, not the override's (Rasenboden's noon: 0.9 to 2.4, 1.8, (1.27, 1.3,
    /// 1.2), 0.4).
    pub exposure_range: (f32, f32),
    pub gamma: f32,
    pub color_scale: [f32; 3],
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
const COLOR_SCALE: usize = 0x44;
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

/// The sky dome's own light (the set's words the game's dome draws carry as `litCol`,
/// `litDir` and `ambLit`): a sun from `elevation` and `azimuth`, and an ambient colour.
/// Rasenboden's noon: (4.78, 5, 3.5) from 45 degrees up, ambient (3, 6, 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DomeLight {
    pub sun: [f32; 3],
    pub elevation: f32,
    pub azimuth: f32,
    pub ambient: [f32; 3],
}

const DOME_SUN: usize = 0x494;
const DOME_AMBIENT: usize = 0x528;

impl DomeLight {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<DomeLight, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[o..o + 4].try_into().unwrap());
        let rgb = |o: usize| [f(o), f(o + 4), f(o + 8)];
        Ok(DomeLight { sun: rgb(DOME_SUN), elevation: f(DOME_SUN + 0xc), azimuth: f(DOME_SUN + 0x10), ambient: rgb(DOME_AMBIENT) })
    }
}

const SKY_DOME_PLACE: usize = 0x45c;
const SKY_DOME_SCALE: usize = 0x478;

/// Where the set's sky dome stands in the field and its scale (`SkyDomeWorldPosition`,
/// `SkyDomeWorldScale`; placed by the game's own dome draws at Rasenboden's noon, which
/// stand at (30000, 0, 30000) three times the model's size: the middle word, 2300 there, is
/// not the height the game draws it at and is not returned).
pub fn sky_dome_place(env: &[u8], time: Time, weather: Weather) -> Result<([f32; 2], [f32; 3]), String> {
    let set = set_of(env, time, weather)?;
    let f = |o: usize| f32::from_le_bytes(set[o..o + 4].try_into().unwrap());
    Ok(([f(SKY_DOME_PLACE), f(SKY_DOME_PLACE + 8)], [f(SKY_DOME_SCALE), f(SKY_DOME_SCALE + 4), f(SKY_DOME_SCALE + 8)]))
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

/// Where a set's `gust_` block is (the game's structure has it at 0x34c).
const GUST: usize = 0x308;

/// The field's wind (`gust_`): it blows at `interval_power`, and after a pause rises over
/// `to_max` seconds to a gust, holds it `max_time` seconds and falls back over `to_interval`.
/// What the plants get from its power `p` at a moment (measured in the game's draws, frame by
/// frame through a gust in Rasenboden: power 1.5 between gusts, 2 with 0.5 either way in
/// them, 2 s up, 8 s held, 1 s down): their bend `g_vAttCache0.x` is 5 sqrt(2 p) (8.660 at
/// 1.5), their sway's phase turns sqrt(p) radians a second, and the power moves between its two
/// levels as half a cosine wave, (1 - cos(pi u)) / 2 (30 frames of a rise fit to 0.0001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gust {
    pub power: f32,
    pub power_random: f32,
    pub to_max: f32,
    pub to_interval: f32,
    pub max_time: f32,
    pub interval_power: f32,
    pub interval_time: f32,
    pub interval_random: f32,
}

impl Gust {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<Gust, String> {
        let set = set_of(env, time, weather)?;
        let f = |k: usize| f32::from_le_bytes(set[GUST + 4 * k..GUST + 4 * k + 4].try_into().unwrap());
        // after azimuth and elevation: power, powerRandom, powerToMaxTime, powerToIntervalTime,
        // powerMaxTime, intervalPower, intervalTime, intervalTimeRandom
        Ok(Gust { power: f(2), power_random: f(3), to_max: f(4), to_interval: f(5), max_time: f(6), interval_power: f(7), interval_time: f(8), interval_random: f(9) })
    }

    // STAND-IN: Each gust's strength and the pause before it are drawn by a generator of Barrel's own, and the first pause starts with the field; the real thing is the game's shared generator and where its cycle stands when the field comes up.
    /// The wind's power `seconds` after the field came up, and how far the plants' sway has
    /// turned by then (radians, the power's root added up a 60th of a second at a time).
    pub fn at(&self, seconds: f32) -> (f32, f32) {
        let smooth = |u: f32| (1.0 - (std::f32::consts::PI * u.clamp(0.0, 1.0)).cos()) * 0.5;
        // a number in 0..1 for the n-th draw: the same every run
        let draw = |n: u32| {
            let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
            x ^= x >> 15;
            x = x.wrapping_mul(0x2c1b_3c6d);
            x ^= x >> 12;
            (x >> 8) as f32 / (1u32 << 24) as f32
        };
        let (mut cycle, mut start) = (0u32, 0.0f32);
        let mut phases = [0.0f32; 4];
        let mut peak = self.power;
        let mut lay = |cycle: u32, phases: &mut [f32; 4], peak: &mut f32| {
            *phases = [self.interval_time + draw(2 * cycle) * self.interval_random, self.to_max, self.max_time, self.to_interval].map(|t| t.max(0.0));
            *peak = self.power + (2.0 * draw(2 * cycle + 1) - 1.0) * self.power_random;
        };
        lay(0, &mut phases, &mut peak);
        let (step, mut turned, mut power) = (1.0 / 60.0, 0.0f32, self.interval_power);
        let frames = (seconds.max(0.0) * 60.0).round() as u32;
        for k in 0..=frames {
            let t = k as f32 * step;
            while phases.iter().sum::<f32>() > 0.0 && t - start >= phases.iter().sum::<f32>() {
                start += phases.iter().sum::<f32>();
                cycle += 1;
                lay(cycle, &mut phases, &mut peak);
            }
            let into = t - start;
            let [pause, up, hold, _] = phases;
            power = if into < pause {
                self.interval_power
            } else if into < pause + up {
                self.interval_power + (peak - self.interval_power) * smooth((into - pause) / up)
            } else if into < pause + up + hold {
                peak
            } else {
                peak + (self.interval_power - peak) * smooth((into - pause - up - hold) / phases[3].max(1e-6))
            };
            if k < frames {
                turned += power.max(0.0).sqrt() * step;
            }
        }
        (power, turned)
    }

    /// The plants' bend for a power: `g_vAttCache0.x`.
    pub fn bend(power: f32) -> f32 {
        5.0 * (2.0 * power.max(0.0)).sqrt()
    }
}

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

/// The sky plane's haze (the set's sky plane glow block, `SkyPlaneGlow*` in the executable's
/// reader): the sky's colour is mixed toward `color` around the direction a light at
/// `elevation` and `azimuth` shines along, from nothing `start` degrees off that direction's
/// horizon to `fade` of the way at `end`. Rasenboden's noon: straight down (the haze lies on
/// the horizon), (2.46, 6.84, 10), 25 to -6 degrees, 0.5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyGlow {
    pub elevation: f32,
    pub azimuth: f32,
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
    pub fade: f32,
}

const SKY_GLOW: usize = 0x174;

impl SkyGlow {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<SkyGlow, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[SKY_GLOW + o..SKY_GLOW + o + 4].try_into().unwrap());
        Ok(SkyGlow { elevation: f(0x4), azimuth: f(0x8), color: [f(0xc), f(0x10), f(0x14)], start: f(0x18), end: f(0x1c), fade: f(0x20) })
    }

    /// The shader's `hGlowParam`: the scale and offset taking the cosine of the angle from the
    /// glow's direction to 0 at `start` and 1 at `end`.
    pub fn ramp(&self) -> [f32; 2] {
        let (from, to) = (self.start.to_radians().sin(), self.end.to_radians().sin());
        let scale = if to != from { 1.0 / (to - from) } else { 0.0 };
        [scale, -from * scale]
    }
}

#[cfg(test)]
mod sky_tests {
    use super::*;

    /// The shader's `hGlowParam` as the game sets it for Rasenboden's noon (25 to -6 degrees).
    #[test]
    fn the_haze_ramp_is_the_games() {
        let g = SkyGlow { elevation: -89.98, azimuth: 0.0, color: [2.4582355, 6.841112, 10.0], start: 25.0, end: -6.0, fade: 0.5 };
        let r = g.ramp();
        assert!((r[0] + 1.8970026).abs() < 1e-4 && (r[1] - 0.8017092).abs() < 1e-4, "{r:?}");
    }
}

/// The sky's large clouds (the set's `cloud_` block and the light it names for them at 0x538;
/// docs/shading.md, "Ryza DX: the sky, the clouds"). Rasenboden's sunny noon: density 0.35,
/// colour (0.32, 0.4688, 0.8), wind (0, -150), a surface 6000 up, 50,000 out, the map
/// tiled 1.2 times across the grid, 1050 thick, light (35, 48.84, 70) from 30 degrees up and
/// round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clouds {
    pub density: f32,
    pub color: [f32; 3],
    pub wind: [f32; 2],
    pub offset_y: f32,
    pub view_range: f32,
    pub frequency: [f32; 2],
    pub thickness: f32,
    pub attenuation_distance: f32,
    pub scattering_scale: f32,
    pub max_delta_uv: f32,
    /// The density map's noise: `CloudHeightNoiseSeed*`.
    pub seed_octave: f32,
    pub seed_persistence: f32,
    pub seed_frequency: [f32; 2],
    pub seed_scale: f32,
    /// The second layer: its wind and its map's tiling.
    pub second_wind: [f32; 2],
    pub noise_frequency: [f32; 2],
    pub light: [f32; 3],
    pub light_elevation: f32,
    pub light_azimuth: f32,
}

const CLOUDS: usize = 0x198;
const CLOUD_LIGHT: usize = 0x538;

/// How wide the cloud grid is, in field units (the game's `vPosOf` at Rasenboden's noon; not
/// found in the set: 1.28 times the view range there).
pub const CLOUD_GRID: f32 = 64_000.0;
/// STAND-IN: how far above the eye the cloud surface's middle stands, before the set's
/// `CloudOffsetY` (12,000 over the eye at noon, where the offset is 6000; the real rule is not
/// found, and could as well be twice the offset).
pub const CLOUD_RISE: f32 = 6_000.0;
/// STAND-IN: how much of the surface's height is gone at the view range (5/6: the captured
/// parabola's fall; the real rule, maybe the map's frequency 1.2 as a divisor, is not found).
pub const CLOUD_FALL: f32 = 5.0 / 6.0;
/// The density map's octaves (the game's `nOctave` at noon; the set's `SeedOctave` is 8).
pub const CLOUD_OCTAVES: u32 = 11;

impl Clouds {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<Clouds, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[o..o + 4].try_into().unwrap());
        let c = |o: usize| f(CLOUDS + o);
        Ok(Clouds {
            density: c(0),
            color: [c(0x4), c(0x8), c(0xc)],
            wind: [c(0x14), c(0x18)],
            offset_y: c(0x1c),
            view_range: c(0x20),
            frequency: [c(0x24), c(0x28)],
            thickness: c(0x34),
            attenuation_distance: c(0x38),
            scattering_scale: c(0x3c),
            max_delta_uv: c(0x44),
            seed_octave: c(0x50),
            seed_persistence: c(0x54),
            seed_frequency: [c(0x58), c(0x5c)],
            seed_scale: c(0x60),
            second_wind: [c(0x64), c(0x68)],
            noise_frequency: [c(0x70), c(0x74)],
            light: [f(CLOUD_LIGHT), f(CLOUD_LIGHT + 4), f(CLOUD_LIGHT + 8)],
            light_elevation: f(CLOUD_LIGHT + 0xc),
            light_azimuth: f(CLOUD_LIGHT + 0x10),
        })
    }

    /// The density map's noise (`vSamplingRangeU/V`, `vPersistence`, `vNoiseScale`): the
    /// sampling range is the seed's frequency scale over its octave count (0.96 times 8 is
    /// the captured 7.68).
    pub fn noise(&self) -> ([f32; 2], f32, f32) {
        ([self.seed_frequency[0] * self.seed_octave, self.seed_frequency[1] * self.seed_octave], self.seed_persistence, self.seed_scale)
    }

    /// The constants the strip's programs take, in the game's space (x not mirrored), for a
    /// camera at `eye`, `seconds` after the field came up, in the sky's scattering `sky` (the
    /// light's colour times its power).
    pub fn shader(&self, sky: &Fog, eye: [f32; 3], seconds: f32) -> CloudShader {
        let (e, a) = (self.light_elevation.to_radians(), self.light_azimuth.to_radians());
        let lit_dir = [-e.cos() * a.sin(), -e.sin(), -e.cos() * a.cos()];
        let [fx, fz] = self.frequency;
        let [nx, nz] = self.noise_frequency;
        // The wind carries each layer's map over the grid: its rate in grid widths, times the
        // map's tiling. (The second layer is not read by the strip's program.)
        let scroll = |wind: f32, freq: f32| -wind * seconds * freq / CLOUD_GRID;
        let top = eye[1] + CLOUD_RISE + self.offset_y;
        let thick = (sky.end - sky.start).max(1.0);
        // The scattering as the sky's is, with this light as the sun (docs/shading.md).
        let sun = self.light.map(|v| v * sky.power);
        let ext: [f32; 3] = std::array::from_fn(|i| sky.rayleigh[i] + sky.mie[i]);
        let part = |b: [f32; 3], i: usize| if ext[i] > 0.0 { b[i] / ext[i] } else { 0.0 };
        let ray_k = 3.0 / (16.0 * std::f32::consts::PI);
        let mie_k = (1.0 - sky.g) * (1.0 - sky.g) / (4.0 * std::f32::consts::PI);
        let log2e = std::f32::consts::LOG2_E;
        CloudShader {
            lit_dir,
            lit_col: self.light,
            c_dif: self.color,
            pos_of: [CLOUD_GRID, CLOUD_GRID, eye[0] - CLOUD_GRID / 2.0, eye[2] - CLOUD_GRID / 2.0],
            tex_of: [fx, fz, nx, nz, scroll(self.wind[0], fx), scroll(self.wind[1], fz), scroll(self.second_wind[0], nx), scroll(self.second_wind[1], nz)],
            range: [0.0, self.density, -CLOUD_FALL * top / (self.view_range * self.view_range), top],
            fog: [1.0 / thick, -sky.start / thick],
            trace_params: [self.thickness, self.max_delta_uv, 1.0 / self.attenuation_distance, self.scattering_scale],
            trace_delta_scale: [fx / CLOUD_GRID, fz / CLOUD_GRID],
            scat: [
                [-ext[0] * log2e, -ext[1] * log2e, -ext[2] * log2e, 1.0],
                [0.0, 0.0, 0.0, 1.0],
                [ray_k * sun[0] * part(sky.rayleigh, 0), ray_k * sun[1] * part(sky.rayleigh, 1), ray_k * sun[2] * part(sky.rayleigh, 2), -2.0 * sky.g],
                [mie_k * sun[0] * part(sky.mie, 0), mie_k * sun[1] * part(sky.mie, 1), mie_k * sun[2] * part(sky.mie, 2), 1.0 + sky.g * sky.g],
            ],
        }
    }
}

/// The strip's constants as the game's programs name them (`vEye`'s companions): see
/// [`Clouds::shader`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudShader {
    pub lit_dir: [f32; 3],
    pub lit_col: [f32; 3],
    pub c_dif: [f32; 3],
    /// `vPosOf`: the grid's width and depth, and its corner's x and z.
    pub pos_of: [f32; 4],
    /// `vTexOf`: the map's tiling in x and z, the second layer's, then each layer's offset.
    pub tex_of: [f32; 8],
    /// `vRange`: unused, the density, the surface's fall with the square of the distance, and
    /// its height over the eye's field.
    pub range: [f32; 4],
    pub fog: [f32; 2],
    /// `vTraceParams`: the layer's thickness, the most one step moves the map's coordinate,
    /// the extinction per unit of thickness, the scattering's scale.
    pub trace_params: [f32; 4],
    pub trace_delta_scale: [f32; 2],
    /// `Scat`.
    pub scat: [[f32; 4]; 4],
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
    /// The scale on the sun (`KeyLight0Scale`: 3 at Rasenboden's noon, where the game's
    /// lighting pass takes the sun at three times the set's colour).
    pub key: f32,
}

const INDIRECT_STAGE: usize = 0x54c;

impl StageLight {
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<StageLight, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[INDIRECT_STAGE + o..INDIRECT_STAGE + o + 4].try_into().unwrap());
        Ok(StageLight { diffuse_map: f(0), diffuse_map_color: [f(8), f(12), f(16)], ambient: f(0x30), key: f(0x34) })
    }

    /// The stage's ambient as the nine coefficients the renderer takes: the light map's times
    /// its scale and colour, the set's ambient colour at its scale added evenly.
    pub fn harmonics(&self, light: &Light, harmonics: &[[f32; 3]; 9]) -> [[f32; 3]; 9] {
        std::array::from_fn(|i| std::array::from_fn(|ch| harmonics[i][ch] * self.diffuse_map * self.diffuse_map_color[ch] + if i == 0 { light.ambient[ch] * self.ambient / 0.282_095 } else { 0.0 }))
    }
}

/// The cloud shadows drifting over the sun's light (the game's `fPtn` and `mW2Pt`; the texture
/// is the field's `_pat.g1t`, sampled as sRGB): how much of the texture's shade is laid on, the
/// size of one repeat over the ground and the drift a second, in repeats. Rasenboden's noon:
/// 0.7, 50000 × 50000, 0.005 each way, as the game's constants there show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pattern {
    pub strength: f32,
    pub size: [f32; 2],
    pub speed: [f32; 2],
}

const PATTERN: usize = INDIRECT_STAGE + 0x38;

impl Pattern {
    // STAND-IN: The word after the strength is taken as the pattern's switch (1 in the noon sets, 0 in the others); the real thing is the game's own reading of it.
    pub fn read(env: &[u8], time: Time, weather: Weather) -> Result<Option<Pattern>, String> {
        let set = set_of(env, time, weather)?;
        let f = |o: usize| f32::from_le_bytes(set[PATTERN + o..PATTERN + o + 4].try_into().unwrap());
        Ok((f(4) != 0.0 && f(8) > 0.0 && f(12) > 0.0).then(|| Pattern { strength: f(0), size: [f(8), f(12)], speed: [f(16), f(20)] }))
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
            color_scale: rgb(COLOR_SCALE),
            bloom_weight: f(BLOOM_WEIGHT),
            lumi_threshold: f(LUMI_THRESHOLD),
            ambient: rgb(MAIN_AMBIENT), parallel: [parallel(0), parallel(1)] })
    }
}

#[cfg(test)]
mod gust_tests {
    use super::Gust;

    #[test]
    fn a_gust_rises_holds_and_falls_as_measured() {
        // Rasenboden's noon wind with no chance in it: 1 s pause, then the gust
        let g = Gust { power: 2.127, power_random: 0.0, to_max: 2.0, to_interval: 1.0, max_time: 8.0, interval_power: 1.5, interval_time: 1.0, interval_random: 0.0 };
        let bend = |t: f32| Gust::bend(g.at(t).0);
        assert!((bend(0.5) - 8.660).abs() < 1e-3);
        // the game's frames 1750 and 1787 of a rise that began at 1727: 8.818 and 9.522
        assert!((bend(1.0 + 23.0 / 60.0) - 8.818).abs() < 3e-3, "{}", bend(1.0 + 23.0 / 60.0));
        assert!((bend(2.0) - 9.522).abs() < 3e-3, "{}", bend(2.0));
        assert!((bend(5.0) - 10.313).abs() < 1e-3);
        assert!((bend(12.5) - 8.660).abs() < 1e-3);
        // held at the gust the sway turns sqrt(power) a second: 0.024308 a frame
        let turn = g.at(6.0).1 - g.at(5.0).1;
        assert!((turn / 60.0 - 0.024308).abs() < 1e-5, "{}", turn / 60.0);
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
        assert_eq!((l.exposure_range, l.gamma, l.color_scale, l.bloom_weight), ((0.9, 2.4), 2.2, [1.2, 1.1, 1.0], 0.6));
        assert_eq!(l.lumi_threshold, 0.8);
        assert_eq!(l.parallel[0], ParallelLight { color: [5.0, 2.074, 1.0], elevation: 30.0, azimuth: -30.0 });
        assert!(Light::read(&env[1..], Time::Noon, Weather::Sunny).is_err());
    }

    #[test]
    fn stage_light_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        let at = SET_SIZE + 0x54c;
        for (k, v) in [(0, 7.0f32), (8, 0.1), (12, 0.2602), (16, 1.0), (0x30, 0.25), (0x34, 3.0)] {
            env[at + k..at + k + 4].copy_from_slice(&v.to_le_bytes());
        }
        let st = StageLight::read(&env, Time::Noon, Weather::Sunny).unwrap();
        assert_eq!(st, StageLight { diffuse_map: 7.0, diffuse_map_color: [0.1, 0.2602, 1.0], ambient: 0.25, key: 3.0 });
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

/// The table of the saturation laid on after the gamma (`fSaturationScaleEx` in the game's
/// last pass), by light set name, weather and time.
pub const SATURATION_PATH: &str = "\\saves\\envoverride\\env_override.xml";

/// The outdoor fields' light set in that table: Rasenboden's noon shows its 1.6 in the game's
/// own constants.
pub const SATURATION_SET: &str = "LGT01_DEFAULT_01";

/// A light set's `SaturationScaleEx` for a time and weather: the row that names both, else
/// one, else neither (-1 stands for any).
pub fn saturation_ex(bytes: &[u8], name: &str, time: Time, weather: Weather) -> Result<f32, String> {
    let root = barrel_formats::table::parse(bytes).map_err(|e| format!("saturation table: {e}"))?;
    let number = |e: &barrel_formats::table::Element, key: &str| e.attr(key).and_then(|v| v.trim().parse::<f32>().ok());
    let mut best: Option<(u32, f32)> = None;
    let mut rows: Vec<&barrel_formats::table::Element> = vec![&root];
    while let Some(e) = rows.pop() {
        rows.extend(e.children.iter());
        if e.attr("Name") != Some(name) {
            continue;
        }
        let (Some(w), Some(t), Some(s)) = (number(e, "Weather"), number(e, "Time"), number(e, "SaturationScaleEx")) else { continue };
        let fits = |v: f32, want: usize| if v < 0.0 { Some(0) } else if v as usize == want { Some(1) } else { None };
        let (Some(a), Some(b)) = (fits(w, weather as usize), fits(t, time as usize)) else { continue };
        if best.is_none_or(|(score, _)| a + b > score) {
            best = Some((a + b, s));
        }
    }
    best.map(|(_, s)| s).ok_or(format!("saturation table: no row for {name}"))
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
mod grade_tests {
    use super::*;

    #[test]
    fn saturation_by_set_time_and_weather() {
        let xml = br#"<root><row Name="A" Weather="-1" Time="-1" SaturationScaleEx="1.2"/><row Name="A" Weather="0" Time="1" SaturationScaleEx="1.6"/><row Name="A" Weather="0" Time="-1" SaturationScaleEx="1.3"/><row Name="B" Weather="-1" Time="-1" SaturationScaleEx="2"/></root>"#;
        assert_eq!(saturation_ex(xml, "A", Time::Noon, Weather::Sunny), Ok(1.6));
        assert_eq!(saturation_ex(xml, "A", Time::Night, Weather::Sunny), Ok(1.3));
        assert_eq!(saturation_ex(xml, "A", Time::Noon, Weather::Foggy), Ok(1.2));
        assert!(saturation_ex(xml, "C", Time::Noon, Weather::Sunny).is_err());
    }

    #[test]
    fn pattern_of_a_set() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        let at = SET_SIZE + 0x584;
        for (k, v) in [0.7f32, 1.0, 50000.0, 50000.0, 0.005, 0.005].into_iter().enumerate() {
            env[at + 4 * k..at + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(Pattern::read(&env, Time::Noon, Weather::Sunny), Ok(Some(Pattern { strength: 0.7, size: [50000.0; 2], speed: [0.005; 2] })));
        assert_eq!(Pattern::read(&env, Time::Morning, Weather::Sunny), Ok(None));
    }
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

#[cfg(test)]
mod cloud_tests {
    use super::*;

    /// Rasenboden's sunny noon: the set's words, and the constants the game's strip draws
    /// carried in a captured frame, to four figures.
    #[test]
    fn the_clouds_constants_are_the_games() {
        let mut env = vec![0u8; SET_SIZE * SETS];
        let set = SET_SIZE;
        let mut put = |o: usize, v: &[f32]| {
            for (k, x) in v.iter().enumerate() {
                env[set + o + 4 * k..set + o + 4 * k + 4].copy_from_slice(&x.to_le_bytes());
            }
        };
        put(CLOUDS, &[0.35, 0.32, 0.4688, 0.8, 1.0, 0.0, -150.0, 6000.0, 50000.0, 1.2, 1.2]);
        put(CLOUDS + 0x34, &[1050.0, 500.0, 1.0, 16.0, 0.02, 0.1, 0.05, 8.0, 0.56, 0.96, 0.96, 1.5, 1000.0, 500.0, 0.3, 4.0, 4.0]);
        put(CLOUD_LIGHT, &[35.0, 48.840305, 70.0, 30.000133, 30.0004]);
        put(SKY_FOG, &[1000.0, 300000.0, 0.713094, 0.713094, 0.713094, 0.973688, 0.02, 0.118, 1.0, 40.0, 0.0]);
        let clouds = Clouds::read(&env, Time::Noon, Weather::Sunny).unwrap();
        let (_, sky) = Fog::read(&env, Time::Noon, Weather::Sunny).unwrap();
        let s = clouds.shader(&sky, [32526.793, 6363.3193, 29944.662], 1.0 / 60.0);
        let near = |what: &str, got: &[f32], want: &[f32]| {
            assert_eq!(got.len(), want.len(), "{what}");
            for (g, w) in got.iter().zip(want) {
                assert!((g - w).abs() <= 1e-4 * w.abs().max(1e-6), "{what}: {got:?} against {want:?}");
            }
        };
        near("litDir", &s.lit_dir, &[-0.43301824, -0.5000033, -0.7499947]);
        near("litCol", &s.lit_col, &[35.0, 48.840305, 70.0]);
        near("cDif", &s.c_dif, &[0.32, 0.4688, 0.8]);
        near("vPosOf", &s.pos_of, &[64000.0, 64000.0, 526.79297, -2055.338]);
        near("vTexOf", &s.tex_of, &[1.2, 1.2, 4.0, 4.0, 0.0, 0.000046875004, -0.0010416667, -0.00052083336]);
        near("vRange", &s.range[1..], &[0.35, -0.0000061211067, 18363.32]);
        near("fog", &s.fog, &[0.0000033444817, -0.0033444816]);
        near("vTraceParams", &s.trace_params, &[1050.0, 0.02, 0.002, 1.0]);
        near("vTraceDeltaScale", &s.trace_delta_scale, &[0.00001875, 0.00001875]);
        near("Scat0", &s.scat[0], &[-1.0576435, -1.1990275, -2.4714847, 1.0]);
        near("Scat1", &s.scat[1], &[0.0, 0.0, 0.0, 1.0]);
        near("Scat2", &s.scat[2], &[2.2795267, 16.55454, 97.54973, -1.9473761]);
        near("Scat3", &s.scat[3], &[0.07502586, 0.09234883, 0.06421291, 1.9480684]);
        let (range, persistence, scale) = clouds.noise();
        near("noise", &[range[0], range[1], persistence, scale], &[7.68, 7.68, 0.56, 1.5]);
    }
}
