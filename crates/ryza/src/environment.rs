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
