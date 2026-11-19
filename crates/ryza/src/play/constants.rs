//! The constants dump (`BARREL_TRACE_CONSTANTS=<file>`): the lighting and post-processing values
//! a Ryza field frame uses, named and laid out as the game's constants of the corresponding
//! draw, for `barrel-parity constants` to compare against a capture. Docs: docs/parity.md,
//! "Constants parity".
//!
//! The file is one JSON object: role -> constant name -> floats (`null` for a component Barrel
//! has no counterpart for). A role names a draw of the game's frame (`ambient.stage`,
//! `lighting.chara`, `sky.plane`, `fog.stage`, `tone`...); the parity tool knows where each sits.
//! Barrel keeps its values in its own form, so the dump undoes the two differences:
//!
//! - the exposure: the scene is lit at the bottom of the set's exposure range (`k`), which is
//!   folded into every light colour; the dump divides it out again;
//! - the mirror: render space is the game's world with x negated, so positions and directions
//!   have their x negated back.
//!
//! The values are read from what the frame feeds (the look slots), or, where Barrel feeds a
//! value through a place of its own (the tone pass's push constants, the metering's rule), from
//! the same source the feed is made from. Costs nothing when the variable is unset.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use barrel_ryza::environment::{Pattern, StageLight};
use barrel_render::*;

use super::{Environment, GLOW_RANGE, LOOK_SLOTS};

/// What the dump needs besides the look slots.
pub(super) struct Inputs<'a> {
    pub look: &'a [[f32; 4]; LOOK_SLOTS],
    pub environment: &'a Environment,
    pub stage: &'a StageLight,
    pub pattern: Option<&'a Pattern>,
    /// The specular ambient's cube map: its mip count less one (`vAmbParam.z`).
    pub specular_mips: f32,
    pub seconds: f32,
    /// The bloom's threshold (a part of the white point) and the glow's rate.
    pub glow: (f32, f32),
    pub meter: Option<&'a barrel_render::meter::Params>,
    pub tone_white: f32,
    pub tone_gamma: f32,
    pub tone_scale: [f32; 3],
    pub tone_vivid: f32,
    /// The projection's planes and the scene's size (the occlusion pass's depth and view terms).
    pub near: f32,
    pub far: f32,
    pub scene_size: (u32, u32),
    /// The sun flare's settings, when the frame has them.
    pub sun_flare: Option<&'a barrel_render::flare::Settings>,
}

type Consts = BTreeMap<&'static str, Vec<Option<f32>>>;

/// The character lighting pass's constants (`DSChara-Cll`) that `model.frag`'s flag 128 path
/// holds as numbers, as the capture has them at Rasenboden's noon. STAND-IN: the game sets them
/// at run time (the environment's `ir_` settings; where is not found), so they are one frame's,
/// taken for every field. `IRDFade`, `IRShadowFade`, `rampCoeAdj` and `rampCoordBias` are 1 and
/// `rampTexHeight` 256: the shader folds them away.
pub(super) const CHARA_PASS: &[(&str, &[f32])] = &[
    ("fOccAlbScl", &[0.50278664]),
    ("IRAFade", &[0.24965833]),
    ("IRDFade", &[1.0]),
    ("IRShadowFade", &[1.0]),
    ("occThresForIRShadow", &[0.2]),
    ("rampCoordScaler", &[0.27, 0.116]),
    ("rampCoordBias", &[1.0, 1.0]),
    ("rampCoordRange", &[0.2, 0.5]),
    ("rampCoeAdj", &[1.0, 1.0]),
];
/// The forward pass over the characters (`sh470bcc97a7144d67` and `sh0d2367f637ea8f6a`: the hull's
/// thickness and the colour's scales), as the capture has them at Rasenboden at noon; run-time
/// settings of the set's `outline_` and `ir_` groups, which are not found, so one frame's, taken
/// for every field. `fOLCSatFade` is 0 and `fAdjByAspect` 1: the shaders fold them away.
// STAND-IN: The forward pass's constants (model.vert, model.frag) are one capture's, Rasenboden at noon; the real thing is the field's `outline_` and `ir_` settings, not found.
pub(super) const FORWARD_PASS: &[(&str, &[f32])] = &[
    ("vOLIntensity", &[0.0, 0.6]),
    ("fOLZDecayRatio", &[0.35]),
    ("fOLThicknessConstantRatio", &[0.01]),
    ("fAdjByAspect", &[1.0]),
    ("fOLZScale", &[0.1]),
    ("vOLColorScaler", &[0.5, 0.5, 0.5]),
    ("fOLLtEfFade", &[0.5]),
    ("vOLAmbEfScaler", &[0.0, 0.0, 0.0]),
    ("fOLCSatFade", &[0.0]),
    ("cSpec", &[1.0, 1.0, 1.0, 20.0]),
];
/// How far the water's refracted ray is walked at most, in transparencies: water.frag's
/// `RAY_TRANSPARENCIES` (the game's `vRrParam.z` over `fRrTransparency`).
pub(super) const WATER_RAY_TRANSPARENCIES: f32 = 8.0;
/// The ambient passes' shadow terms, the same on every kind of surface.
const SHADOW_TERMS: [&str; 12] = ["vSMDF", "vS2VZ", "sdplane", "scSMD", "scSM2D", "mSubW2S", "scSubSM", "mSemiStaticW2S", "scSemiStaticSM", "nLPV3DDepth", "fOverallShadow", "ilaScale"];
/// The look block's view-projection (slots 44 to 47, the shaders' `view_proj`).
const VIEW_PROJ_SLOT: usize = 44;
/// Barrel's clip y against the screen's v: +1 (Vulkan's y runs down the screen, as v does).
const SCREEN_V_SIGN: f32 = 1.0;
/// The lighting pass's `fOccAlbScl` on the stage's draws: occlusion.glsl's `STAGE_OCC_ALBEDO`.
pub(super) const STAGE_OCC_ALBEDO: f32 = 1.0;
/// `vAmbOccRat`'s first two values (the others are not used).
pub(super) const AMBIENT_OCCLUSION_RATIO: [f32; 2] = [0.75268143, 0.24731857];
/// The rim table's entries as the capture has them, all alike: strength (`x`) and power (`w`).
pub(super) const RIM_ENTRY: [f32; 2] = [1.0, 2.0];

fn some(v: &[f32]) -> Vec<Option<f32>> {
    v.iter().copied().map(Some).collect()
}

/// A look slot's colour with the exposure undone.
fn rgb(slot: [f32; 4], k: f32) -> [f32; 3] {
    [slot[0] / k, slot[1] / k, slot[2] / k]
}

/// A render-space direction as the game's world has it.
fn world(v: [f32; 4]) -> [f32; 3] {
    [-v[0], v[1], v[2]]
}

/// `Scat` as four registers from four consecutive look slots (the first slot's `w`, which Barrel
/// does not carry for the scene fog, left out): the in-scattering colours undone of the exposure.
fn scat(rows: &[[f32; 4]], k: f32, row0_w: bool) -> Vec<Option<f32>> {
    let mut v = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let c = if i >= 2 { rgb(*r, k) } else { [r[0], r[1], r[2]] };
        v.extend(c.map(Some));
        v.push(if i == 0 && !row0_w { None } else { Some(r[3]) });
    }
    v
}

fn lights(dir: [[f32; 3]; 2], col: [[f32; 3]; 2]) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
    let d = [dir[0][0], dir[0][1], dir[0][2], 0.0, dir[1][0], dir[1][1], dir[1][2], 0.0];
    let c = [col[0][0], col[0][1], col[0][2], 1.0, col[1][0], col[1][1], col[1][2], 1.0];
    (some(&d), some(&c))
}

/// The dump as JSON text.
pub(super) fn json(i: &Inputs) -> String {
    let (_, _, light, sky_fog, _, chara, [key, second]) = i.environment;
    let (key, second) = (*key, *second);
    let look = i.look;
    let k = light.exposure_range.0;
    let eye = look[4];
    let v_eye = some(&[-eye[0], eye[1], eye[2], 1.0]);
    let mut roles: BTreeMap<&'static str, Consts> = BTreeMap::new();
    let mut put = |role: &'static str, c: Consts| {
        roles.insert(role, c);
    };

    // The stage's and the characters' light: the sun and the second light, both scaled as the
    // game's lighting pass takes them (the characters' sun by their own scale over the stage's,
    // what the draws' `light` word does).
    let dirs = [world(look[22]), world(look[24])];
    let stage_cols = [rgb(look[23], k), rgb(look[25], k)];
    // A character's draw scales both lights by its sun's scale over the stage's (the draw's
    // `light` word), and its second light by `look[66].y` more.
    let chara_k = (chara.key / key).clamp(0.0, 1.0);
    let chara_cols = [stage_cols[0].map(|v| v * chara_k), stage_cols[1].map(|v| v * chara_k * look[66][1])];
    let (lit_dir, lit_col) = lights(dirs, stage_cols);
    let (_, chara_col) = lights(dirs, chara_cols);
    let mut lighting = Consts::new();
    lighting.insert("litDir", lit_dir.clone());
    lighting.insert("litCol", lit_col);
    lighting.insert("vEye", v_eye.clone());
    // The stage's occlusion: tinted by the albedo at `STAGE_OCC_ALBEDO` and taken whole on the
    // ambient (`vAmbOccRat` 1, 0: the characters' split it).
    lighting.insert("fOccAlbScl", some(&[STAGE_OCC_ALBEDO]));
    lighting.insert("vAmbOccRat", vec![Some(1.0), Some(0.0), None, None]);
    put("lighting.stage", lighting.clone());
    // The lighting pass's second and third draws light the same stage with the same lights,
    // and add the emission maps at their strength (`emScl`, `EMISSION_SLOT`).
    lighting.insert("emScl", some(&[look[EMISSION_SLOT][0]]));
    put("lighting.stage1", lighting.clone());
    put("lighting.stage2", lighting.clone());
    lighting.remove("emScl");
    // The water's lighting pass takes the same lights and the camera (its position, and the
    // way it looks as the shadow slots carry it).
    let mut water_light = lighting.clone();
    // (Not the stage's occlusion: the water's own is not done.)
    water_light.remove("fOccAlbScl");
    water_light.remove("vAmbOccRat");
    water_light.insert("vCameraPos", some(&[-eye[0], eye[1], eye[2]]));
    let fwd = look[SHADOW_CASCADE_SLOT + 9];
    if fwd[..3] != [0.0; 3] {
        water_light.insert("vCameraDir", some(&world(fwd)));
    }
    // The forward pass over the characters takes the stage's lights at the set's own colours (the
    // lighting pass's are those times the stage's key and second light scales).
    let mut forward = lighting.clone();
    forward.insert("litCol", lights(dirs, [stage_cols[0].map(|v| v / key), stage_cols[1].map(|v| v / second)]).1);
    for (name, values) in FORWARD_PASS {
        forward.insert(name, values.iter().map(|&v| Some(v)).collect());
    }
    put("forward.chara", forward);
    lighting.insert("litCol", chara_col);
    for (name, values) in CHARA_PASS {
        lighting.insert(name, values.iter().map(|&v| Some(v)).collect());
    }
    lighting.insert("vAmbOccRat", vec![Some(AMBIENT_OCCLUSION_RATIO[0]), Some(AMBIENT_OCCLUSION_RATIO[1]), None, None]);
    lighting.insert("emScl", some(&[look[EMISSION_SLOT][0]]));
    lighting.insert("cRimTbl", (0..64).map(|i| match i % 4 {
        0 => Some(RIM_ENTRY[0]),
        3 => Some(RIM_ENTRY[1]),
        _ => None,
    }).collect());
    put("lighting.chara", lighting);

    // The ambient-and-shadow pass. Barrel lights every stage surface with the light map at the
    // stage's scale and tint plus the set's ambient colour at its scale, and the cloud pattern
    // over the sun's share everywhere.
    let stage_ambient = light.ambient.map(|v| v * i.stage.ambient);
    let pattern = |strength: f32| -> Consts {
        let mut c = Consts::new();
        c.insert("fPtn", some(&[strength]));
        if let Some(p) = i.pattern {
            // mW2Pt as the game lays it: u = x / size_x - y d.x / (d.y size_x) + offset, v with
            // z and size_y (the offsets, set by the field's clock, are not compared).
            let d = dirs[0];
            let mut m = vec![None; 16];
            m[0] = Some(1.0 / p.size[0]);
            m[4] = Some(-d[0] / d[1] / p.size[0]);
            m[5] = Some(-d[2] / d[1] / p.size[1]);
            m[9] = Some(1.0 / p.size[1]);
            c.insert("mW2Pt", m);
        }
        c
    };
    let strength = look[49][1];
    let mut ambient = pattern(strength);
    ambient.insert("vAmbParam", vec![Some(i.stage.diffuse_map), Some(0.0), None, None]);
    ambient.insert("vAmbDifScl", vec![Some(i.stage.diffuse_map_color[0]), Some(i.stage.diffuse_map_color[1]), Some(i.stage.diffuse_map_color[2]), None]);
    ambient.insert("ambLit", some(&[stage_ambient[0], stage_ambient[1], stage_ambient[2], 1.0]));
    // `vSMDF`: how far each cascade reaches past the first one's end (the shader's picking and
    // blend); the third word is not placed. Barrel's ends are `CASCADE_ENDS` as the look carries
    // them (`SHADOW_CASCADE_SLOT + 8`, times the shadow reach).
    let ends = look[SHADOW_CASCADE_SLOT + 8];
    if ends[0] > 0.0 {
        ambient.insert("vSMDF", vec![Some(ends[1] - ends[0]), Some(ends[2] - ends[0]), None, None]);
        // The cascade pick and the far fade as planes in the first cascade's map (u, v, depth,
        // 1): `vS2VZ` gives the view depth past the first cascade's end, `sdplane` the weight
        // (10000 - depth) / 3500 that fades the cascades out (Barrel's `shadow_ends.z`, `.w`).
        let cascade = glam::Mat4::from_cols_array_2d(&[look[27], look[28], look[29], look[30]]);
        let forward = glam::Vec3::from_slice(&look[SHADOW_CASCADE_SLOT + 9][..3]);
        let eye = glam::Vec3::from_slice(&eye[..3]);
        let pick = shadow_planes(cascade, forward, eye, ends[0], ends[2], ends[3]);
        ambient.insert("vS2VZ", some(&pick[0]));
        ambient.insert("sdplane", some(&pick[1]));
        // `scSMD`: each cascade's map, its texel and its size ({1/w, 1/h, w, h}).
        let maps: Vec<f32> = (0..3).flat_map(|k| { let [_, _, tu, tv] = look[SHADOW_SIDES_SLOT + k]; [tu, tv, 1.0 / tu, 1.0 / tv] }).collect();
        ambient.insert("scSMD", some(&maps));
        // `scSM2D`: where the sixteen taps start, 1.5 texels before the point (`cascade_light`);
        // its second half is never read by the game's shader.
        let start: Vec<Option<f32>> = (0..3).flat_map(|k| { let [_, _, tu, tv] = look[SHADOW_SIDES_SLOT + k]; [Some(-1.5 * tu), Some(-1.5 * tv), None, None] }).collect();
        ambient.insert("scSM2D", start);
        // The characters' map and the static layer as the game's `mSubW2S` and `mSemiStaticW2S`
        // (game space to the map's u, v and depth, row vectors) and their sizes.
        if look[SHADOW_SUB_SLOT + 4][2] > 0.5 {
            let sub = glam::Mat4::from_cols_array_2d(&[look[SHADOW_SUB_SLOT], look[SHADOW_SUB_SLOT + 1], look[SHADOW_SUB_SLOT + 2], look[SHADOW_SUB_SLOT + 3]]);
            ambient.insert("mSubW2S", some(&game_map(sub)));
            let t = look[SHADOW_SUB_SLOT + 4][0];
            ambient.insert("scSubSM", some(&[t, t, 1.0 / t, 1.0 / t]));
        }
        let fixed = glam::Mat4::from_cols_array_2d(&[look[SHADOW_FAR_SLOT], look[SHADOW_FAR_SLOT + 1], look[SHADOW_FAR_SLOT + 2], look[SHADOW_FAR_SLOT + 3]]);
        if look[SHADOW_FAR_SLOT + 4][1] > 0.5 {
            ambient.insert("mSemiStaticW2S", some(&game_map(fixed)));
            let [_, _, tu, tv] = look[SHADOW_SIDES_SLOT + 3];
            ambient.insert("scSemiStaticSM", some(&[tu, tv, 1.0 / tu, 1.0 / tv]));
        }
    }
    // What Barrel does not draw, as the values that turn it off in the game's shader: no
    // light volume (`nLPV3DDepth` 0 layers and no volume bound: `fDiffuseScale`, which only
    // scales it, is left out, 0.2 / pi on the characters), no overall darkening
    // (`fOverallShadow` 0: the shadow is taken whole) and `ilaScale` 0 (its term 1). The
    // gate's capture has them all so; a field that turns one on shows here.
    ambient.insert("nLPV3DDepth", some(&[0.0]));
    ambient.insert("fOverallShadow", some(&[0.0]));
    ambient.insert("ilaScale", some(&[0.0, 0.0]));
    put("ambient.stage", ambient.clone());
    // The second kind of surface (its `ambLit` is the sky dome's own ambient): lit by the dome's
    // light slot, with no light map and no cloud pattern.
    let dome_ambient = rgb(look[DOME_LIGHT_SLOT], k);
    let mut second = pattern(0.0);
    second.insert("vAmbParam", vec![Some(0.0), Some(0.0), None, None]);
    second.insert("vAmbDifScl", vec![Some(0.0), Some(0.0), Some(0.0), None]);
    second.insert("ambLit", some(&[dome_ambient[0], dome_ambient[1], dome_ambient[2], 1.0]));
    for name in SHADOW_TERMS {
        if let Some(v) = ambient.get(name) {
            second.insert(name, v.clone());
        }
    }
    put("ambient.second", second);
    // The characters' flat ambient: the set's ambient at the characters' scale; their light map
    // is folded into it (no counterpart in the dump).
    let chara_ambient = light.ambient.map(|v| v * chara.ambient);
    let mut ch = pattern(strength);
    ch.insert("vAmbParam", vec![Some(chara.diffuse_map), Some(0.0), None, None]);
    ch.insert("vAmbDifScl", vec![Some(chara.diffuse_map_color[0]), Some(chara.diffuse_map_color[1]), Some(chara.diffuse_map_color[2]), None]);
    ch.insert("ambLit", some(&[chara_ambient[0], chara_ambient[1], chara_ambient[2], 1.0]));
    for name in SHADOW_TERMS {
        if let Some(v) = ambient.get(name) {
            ch.insert(name, v.clone());
        }
    }
    put("ambient.chara", ch);
    // The water's ambient and shadow pass (after its surface goes into the G-buffer): the same
    // shadow maps and terms. Its ambient is not compared: Barrel's water takes the cube map's
    // in its own lighting (docs/shading.md, "Ryza DX: the water").
    let mut water = Consts::new();
    for name in SHADOW_TERMS {
        if let Some(v) = ambient.get(name) {
            water.insert(name, v.clone());
        }
    }
    put("ambient.water", water);

    // The scene's fog (the lighting passes' and the specular ambient's), the sky's, and the sky's
    // own light and haze.
    let fog_xy = |s: [f32; 4]| some(&[s[0], s[1], 0.0, 0.0]);
    let mut scene_fog = Consts::new();
    scene_fog.insert("fog", fog_xy(look[32]));
    scene_fog.insert("Scat", scat(&look[33..37], k, false));
    scene_fog.insert("vEye", v_eye.clone());
    scene_fog.insert("litDir", some(&[dirs[0][0], dirs[0][1], dirs[0][2], 0.0]));
    let sun = light.parallel[0].color;
    scene_fog.insert("litCol", some(&[sun[0], sun[1], sun[2], 1.0]));
    put("fog.stage", scene_fog.clone());
    scene_fog.remove("litDir");
    scene_fog.remove("litCol");
    let mut sky_scene = Consts::new();
    let s0 = SKY_SLOT;
    sky_scene.insert("fog", fog_xy(look[s0 + 6]));
    sky_scene.insert("Scat", scat(&look[s0 + 1..s0 + 5], k, true));
    sky_scene.insert("vEye", v_eye.clone());
    let mut sky_lit = sky_scene.clone();
    sky_lit.insert("litDir", some(&[dirs[0][0], dirs[0][1], dirs[0][2], 0.0]));
    sky_lit.insert("litCol", some(&[sun[0], sun[1], sun[2], 1.0]));
    put("fog.sky", sky_lit);

    let mut plane = Consts::new();
    plane.insert("fog", fog_xy(look[s0 + 6]));
    plane.insert("Scat", scat(&look[s0 + 1..s0 + 5], k, true));
    plane.insert("skyPos", some(&[look[s0 + 5][0], look[s0 + 5][1], look[s0 + 5][2], 0.0]));
    let glow_way = world(look[s0 + 8]);
    plane.insert("vGlowDir", some(&glow_way));
    let c_glow = rgb(look[s0 + 9], k);
    plane.insert("cGlow", some(&[c_glow[0], c_glow[1], c_glow[2], look[s0 + 9][3]]));
    plane.insert("hGlowParam", some(&[look[s0 + 10][0], look[s0 + 10][1]]));
    let sun_way = world(look[s0 + 7]);
    plane.insert("litDir", some(&[sun_way[0], sun_way[1], sun_way[2], 0.0]));
    plane.insert("litCol", some(&[sun[0], sun[1], sun[2], 1.0]));
    put("sky.plane", plane);

    // The dome's two layers: the set's dome light, the sky's fog.
    let mut dome = Consts::new();
    let way = world(look[DOME_SUN_SLOT + 1]);
    dome.insert("litDir", some(&[-way[0], -way[1], -way[2], 0.0]));
    let c = rgb(look[DOME_SUN_SLOT], k);
    dome.insert("litCol", some(&[c[0], c[1], c[2], 1.0]));
    let a = rgb(look[DOME_LIGHT_SLOT], k);
    dome.insert("ambLit", some(&[a[0], a[1], a[2], 1.0]));
    dome.insert("fog", fog_xy(look[s0 + 6]));
    dome.insert("Scat", scat(&look[s0 + 1..s0 + 5], k, true));
    dome.insert("vEye", v_eye.clone());
    // The dome's surface takes no light map (as the second kind of stage surface).
    dome.insert("vAmbParam", vec![Some(0.0), Some(0.0), None, None]);
    dome.insert("vAmbDifScl", vec![Some(0.0), Some(0.0), Some(0.0), None]);
    put("sky.dome0", dome.clone());
    put("sky.dome1", dome);

    // The cloud strip, from the slots the strip's draw reads.
    let sky = sky_fog.0;
    let s = sky_fog.3.shader(&sky, [-eye[0], eye[1], eye[2]], i.seconds);
    let c0 = CLOUD_SLOT;
    let mut strip = Consts::new();
    strip.insert("vEye", v_eye.clone());
    strip.insert("fog", fog_xy([look[c0 + 7][2], look[c0 + 7][3], 0.0, 0.0]));
    strip.insert("vPosOf", some(&[s.pos_of[0], s.pos_of[1], s.pos_of[2], s.pos_of[3]]));
    // (The layers' offsets, `vTexOf[4..]`, are the wind's travel since the field came up, not compared.)
    strip.insert("vTexOf", s.tex_of.iter().enumerate().map(|(n, &v)| (n < 4).then_some(v)).collect());
    strip.insert("vRange", vec![None, Some(look[c0 + 3][3]), Some(look[c0 + 4][2]), Some(look[c0 + 4][3])]);
    strip.insert("cDif", vec![Some(look[c0 + 3][0]), Some(look[c0 + 3][1]), Some(look[c0 + 3][2]), None]);
    let cd = world(look[c0]);
    strip.insert("litDir", some(&[cd[0], cd[1], cd[2], 0.0]));
    let cl = rgb(look[c0 + 1], k);
    strip.insert("litCol", some(&[cl[0], cl[1], cl[2], 1.0]));
    let ca = rgb(look[c0 + 2], k);
    strip.insert("ambLit", some(&[ca[0], ca[1], ca[2], 1.0]));
    strip.insert("Scat", scat(&look[c0 + 8..c0 + 12], k, true));
    strip.insert("vTraceParams", some(&look[c0 + 6]));
    strip.insert("vTraceDeltaScale", some(&[look[c0 + 7][0], look[c0 + 7][1]]));
    // A second draw of the cloud pieces takes the same light.
    let mut strip2 = Consts::new();
    for name in ["litDir", "litCol", "ambLit"] {
        strip2.insert(name, strip[name].clone());
    }
    put("sky.clouds2", strip2);
    put("sky.clouds", strip);

    // The ambient occlusion's pass (its program's constants: the depth terms
    // from the projection, the target's size, the rest as captured; `vRadiusParam`'s first three
    // words are not read by the program).
    {
        let (w, h) = barrel_render::ao::half(i.scene_size);
        let (w, h) = (w as f32, h as f32);
        let mut c = Consts::new();
        c.insert("vResolutionParam", some(&[w, h, 1.0 / w, 1.0 / h]));
        c.insert("vDepthParam", vec![Some(1.0 / i.far - 1.0 / i.near), Some(1.0 / i.near), Some(barrel_render::ao::DEPTH_FAR), None]);
        // (`vViewParam` is not dumped: the program takes it from the frame's projection, 45 degrees
        // in the capture, and the board's view carries its own field of view.)
        c.insert("vRadiusParam", vec![None, None, None, Some(barrel_render::ao::MAX_PIXELS)]);
        c.insert("vRadiusParam2", some(&barrel_render::ao::RADIUS));
        c.insert("vRayParam", some(&barrel_render::ao::RAY));
        c.insert("vOccParam", some(&[barrel_render::ao::OCC[0], barrel_render::ao::OCC[1], w / 4.0, h / 4.0]));
        put("occlusion", c);
    }

    // The specular ambient: the stage's draw and the characters'.
    let spec = |diffuse: f32, scale: f32, tint: [f32; 3], mips: f32| -> Consts {
        let mut c = scene_fog.clone();
        // The pass's fog takes the sun at 1 where the fog pass takes it at the stage's key scale
        // (the shader divides it out again).
        c.insert("Scat", scat(&[look[33], look[34], look[35], look[36]], k, false).into_iter().enumerate().map(|(n, v)| if (8..11).contains(&n) || (12..15).contains(&n) { v.map(|x| x / key) } else { v }).collect());
        c.insert("vAmbParam", vec![Some(diffuse), Some(scale), Some(mips), None]);
        c.insert("vAmbSpcScl", vec![Some(tint[0]), Some(tint[1]), Some(tint[2]), None]);
        // STAND-IN (as the frame's): the light's weight is 1.
        c.insert("vAmbSpcWgt", vec![Some(1.0), None, None, None]);
        c
    };
    let sp = look[SPECULAR_SLOT];
    let tint = look[SPECULAR_SLOT + 1];
    // Both draws take the sun at the set's own colour (as the forward pass) and their
    // occlusion's share on the ambient (the stage's whole, the characters' split).
    let set_sun = [stage_cols[0][0] / key, stage_cols[0][1] / key, stage_cols[0][2] / key, 1.0];
    let with_sun = |mut c: Consts, occ: [f32; 2]| {
        c.insert("litDir", some(&[dirs[0][0], dirs[0][1], dirs[0][2], 0.0]));
        c.insert("litCol", some(&set_sun));
        c.insert("vAmbOccRat", vec![Some(occ[0]), Some(occ[1]), None, None]);
        c
    };
    put("specular.stage", with_sun(spec(i.stage.diffuse_map, sp[0], [tint[0], tint[1], tint[2]], sp[1]), [1.0, 0.0]));
    put("specular.chara", with_sun(spec(chara.diffuse_map, chara.specular_map, chara.specular_map_color, i.specular_mips), AMBIENT_OCCLUSION_RATIO));

    // The water: the first and the last piece's body (its albedo and reflectance at normal
    // incidence, from the slots the water's vertex program reads).
    let pieces = look[WATER_SLOT][0] as usize;
    // The view depth from the depth buffer's (`vD2Z`), and world to the screen's texture
    // coordinates times w (`mRrW2T`: rows u w, v w, w; game space is x mirrored).
    let d2z = some(&[1.0 / i.near, 1.0 / i.far - 1.0 / i.near]);
    let vp = glam::Mat4::from_cols_array_2d(&[look[VIEW_PROJ_SLOT], look[VIEW_PROJ_SLOT + 1], look[VIEW_PROJ_SLOT + 2], look[VIEW_PROJ_SLOT + 3]]) * glam::Mat4::from_diagonal(glam::Vec4::new(-1.0, 1.0, 1.0, 1.0));
    let row = |k: usize| glam::Vec4::new(vp.x_axis[k], vp.y_axis[k], vp.z_axis[k], vp.w_axis[k]);
    let (x, y, w) = (row(0), row(1), row(3));
    let screen: Vec<Option<f32>> = [(x + w) * 0.5, (w + SCREEN_V_SIGN * y) * 0.5, w].iter().flat_map(|r| r.to_array()).map(Some).collect();
    water_light.insert("vD2Z", d2z.clone());
    water_light.insert("mRrW2T", screen.clone());
    put("lighting.water", water_light);
    if pieces > 0 {
        for (role, piece) in [("water.first", 0), ("water.last", pieces - 1)] {
            let base = WATER_SLOT + 1 + 5 * piece;
            let body = look[base + 4];
            let mut c = Consts::new();
            // The refraction: the index (slot c.w holds it less 1) and the transparency (d.w),
            // with `vRrParam` built from them as the game packs it.
            let (index, clear) = (look[base + 2][3] + 1.0, look[base + 3][3]);
            c.insert("fRrRefractiveIndex", some(&[index]));
            c.insert("fRrTransparency", some(&[clear]));
            c.insert("vRrParam", some(&[1.0 / index, index - 1.0, WATER_RAY_TRANSPARENCIES * clear, -1.0 / clear]));
            c.insert("vD2Z", d2z.clone());
            c.insert("mRrW2T", screen.clone());
            c.insert("vAlbedoColor", vec![Some(body[0]), Some(body[1]), Some(body[2]), None]);
            c.insert("vReflectance", vec![Some(body[3]), None, None, None]);
            c.insert("vEye", v_eye.clone());
            let f = world(look[SHADOW_CASCADE_SLOT + 9]);
            if f != [0.0; 3] {
                let xz = (f[0] * f[0] + f[2] * f[2]).sqrt();
                c.insert("vRrCameraPos", v_eye.clone());
                c.insert("vRrCameraDir", some(&[f[0], f[1], f[2], 0.0]));
                c.insert("vCameraDirXZ", vec![Some(f[0] / xz), Some(f[2] / xz), None, None]);
            }
            put(role, c);
        }
    }

    // The post chain.
    if let Some(m) = i.meter {
        let mut c = Consts::new();
        c.insert("fMiddleGray", some(&[m.rule.key]));
        c.insert("fAdaptBaseRate", some(&[barrel_render::meter::ADAPT_BASE_RATE]));
        c.insert("fFrameTime", some(&[m.dt]));
        c.insert("fAdaptTime", some(&[barrel_render::meter::ADAPT_TIME]));
        c.insert("fLightAdaptedScale", some(&[m.rule.toward_darker]));
        c.insert("fDarkAdaptedScale", some(&[m.rule.toward_brighter]));
        c.insert("vExposureRange", some(&[m.rule.range.0, m.rule.range.1]));
        put("meter.adapt", c);
    }
    // `fLumDelta` is the literal 1 in meter.frag.
    put("meter.mean", Consts::from([("fLumDelta", some(&[1.0]))]));
    put("meter.log", Consts::from([("fLumDelta", some(&[1.0]))]));
    put("bloom.bright", Consts::from([("fThreshold", some(&[i.glow.0 * i.tone_white])), ("fMaxLum", some(&[i.tone_white]))]));
    let mut tone = Consts::new();
    // The glow is stored at half its range, so the weight the pass applies is its rate over two.
    tone.insert("fBloomWeight", some(&[i.glow.1 / GLOW_RANGE]));
    tone.insert("vColorScale", some(&i.tone_scale));
    // `saturation` is off in the frame (the game's 1 on each channel).
    tone.insert("vSaturationScale", some(&[1.0, 1.0, 1.0]));
    tone.insert("fSaturationScaleEx", some(&[i.tone_vivid.max(1.0)]));
    if i.tone_gamma > 0.0 {
        tone.insert("fGamma", some(&[1.0 / i.tone_gamma]));
    }
    // The curve's normalisation: the post pass divides by the curve at the white point.
    let f = |x: f32| (x * (0.22 * x + 0.03) + 0.002) / (x * (0.22 * x + 0.3) + 0.06) - 0.0333;
    tone.insert("SimulateHDRParams", some(&[1.0 / f(i.tone_white), f(i.tone_white)]));
    // The effects the set switches off here, which Barrel does not draw: their settings as the
    // set's `trueHDR_` block and `lightShaft_` hold them (a weight of 0 is what makes not drawing
    // right), and the tone pass's distant blur settings, also read from the set. Not dumped:
    // `fHexRenderWeight`, the star's weight ramps, the shaft's power and light position, the
    // blur's weights and the vignette (no member found for them).
    let screen = some(&[i.scene_size.0 as f32, i.scene_size.1 as f32]);
    let dof = Consts::from([("fKIDSDOFType", some(&[light.dof_type]))]);
    put("dof.31", dof.clone());
    let mut dof_hex = dof.clone();
    dof_hex.insert("fHexScale", some(&[light.hex_scale]));
    for role in ["dof.32", "dof.33", "dof.34", "dof.35", "dof.36", "dof.37"] {
        put(role, dof_hex.clone());
    }
    // The merge pass of each of the star's lines (one in `nStarLines`) takes its share of the
    // sum (inferred from nStarLines 4 and the capture's 0.25; the other passes' weight is 1).
    let merge = Consts::from([("g_fStarMergeWeight", some(&[1.0 / light.star_lines]))]);
    if light.star_lines > 0.0 {
        for role in ["star.51", "star.54", "star.57", "star.60"] {
            put(role, merge.clone());
        }
    }
    let [scale, frequency, weight] = light.shaft_noise;
    put("shaft.63", Consts::from([("vLightShaftNoizeParams", some(&[scale, frequency, weight, 0.0]))]));
    put("shaft.65", Consts::from([("vScreenSize", screen.clone())]));
    put("shaft.68", Consts::from([("vScreenSize", screen.clone())]));
    tone.insert("vScreenSize", screen);
    tone.insert("fKIDSDOFType", some(&[light.dof_type]));
    tone.insert("fStarWeight", some(&[light.star_weight]));
    tone.insert("fLensFlareWeight", some(&[light.lens_flare_weight]));
    tone.insert("fDistantBlurZThreshold", some(&[light.distant_blur_z_threshold]));
    tone.insert("fDistantBlurIntensity", some(&[light.distant_blur_intensity]));
    tone.insert("fFar", some(&[i.far]));
    put("tone", tone);

    // The sun flare: the visibility pass (the reference point in the first cascade's map and the
    // speeds) and the wash's second draw. The game's `mRotation` is a 4 x 4 matrix of which the
    // tilt is the upper-left 2 x 2.
    if let Some(s) = i.sun_flare {
        let [c, sn, ns, _] = s.rotation;
        let rotation = [c, sn, 0.0, 0.0, ns, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        put(
            "sunvis",
            Consts::from([
                // Barrel's cascade is the mirror image of the game's (render space has x negated, and
                // its map's y runs the other way): both uv components are turned back.
                ("vShadowReferenceUV", some(&[1.0 - s.reference[0], 1.0 - s.reference[1], s.reference[2], s.reference[3]])),
                ("bReverseFlag", some(&[barrel_ryza::flare::REVERSE_FLAG])),
                ("fShadowIntensityUpSpeed", some(&[s.up_speed])),
                ("fShadowIntensityDownSpeed", some(&[s.down_speed])),
            ]),
        );
        let flare = |intensity: f32| {
            Consts::from([
                ("mRotation", some(&rotation)),
                ("fFlareCurveCoefficient", some(&[s.flare.curve_coefficient])),
                ("fFlareCurveCenter", some(&[s.flare.curve_center])),
                ("fFlareAmount", some(&[s.flare.amount])),
                ("fIntensity", some(&[intensity])),
                ("fFlareShadowInfluenceRatio", some(&[s.flare.shadow_influence])),
                ("fParaffinCurveCoefficient", some(&[s.paraffin.curve_coefficient])),
                ("fParaffinCurveCenter", some(&[s.paraffin.curve_center])),
                ("fParaffinAmount", some(&[s.paraffin.amount])),
                ("fParaffinShadowInfluenceRatio", some(&[s.paraffin.shadow_influence])),
                ("vFlareColor", some(&s.flare.color)),
                ("vParaffinColor", some(&s.paraffin.color)),
            ])
        };
        // The game draws the scene and the sky apart, the sky at its own intensity.
        put("flare", flare(s.intensity));
        put("flare.sky", flare(s.sky_intensity));
    }

    let mut out = String::from("{\n");
    for (n, (role, consts)) in roles.iter().enumerate() {
        let _ = write!(out, "{}  \"{role}\": {{", if n > 0 { ",\n" } else { "" });
        for (m, (name, v)) in consts.iter().enumerate() {
            let items: Vec<String> = v.iter().map(|x| x.map_or("null".to_string(), |x| format!("{x:e}"))).collect();
            let _ = write!(out, "{}\"{name}\": [{}]", if m > 0 { ", " } else { " " }, items.join(", "));
        }
        out.push_str(" }");
    }
    out.push_str("\n}\n");
    out
}

/// Writes the dump when `BARREL_TRACE_CONSTANTS` names a file.
pub(super) fn write_if_asked(i: impl FnOnce() -> Option<String>) {
    if let Some(path) = std::env::var_os("BARREL_TRACE_CONSTANTS")
        && let Some(text) = i()
    {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_character_shader_holds_the_same_constants() {
        let source = include_str!("../../../render/shaders/model.frag");
        // The numbers of `const <type> NAME = [vecN](a, b, ...);`.
        let numbers = |name: &str| -> Vec<f32> {
            let line = source.lines().find(|l| l.contains(&format!(" {name} = "))).unwrap_or_else(|| panic!("model.frag has no {name}"));
            let rhs = line.split(" = ").nth(1).unwrap().split(';').next().unwrap();
            rhs.split_once('(').map_or(rhs, |(_, inner)| inner.trim_end_matches(')')).split(',').map(|v| v.trim().parse().unwrap_or_else(|_| panic!("{name}: {v}"))).collect()
        };
        let pass = |name: &str| CHARA_PASS.iter().find(|(n, _)| *n == name).unwrap().1.to_vec();
        assert_eq!(numbers("OCC_COLOUR"), pass("fOccAlbScl"));
        assert_eq!(numbers("OCC_ON_SUN"), pass("IRAFade"));
        assert_eq!(numbers("OCC_DEEP"), pass("occThresForIRShadow"));
        assert_eq!(numbers("RAMP_SCALE"), pass("rampCoordScaler"));
        assert_eq!(numbers("RAMP_RANGE"), pass("rampCoordRange"));
        assert_eq!(numbers("OCC_ON_AMBIENT"), AMBIENT_OCCLUSION_RATIO);
        assert_eq!(numbers("RIM"), RIM_ENTRY);
        let occlusion = include_str!("../../../render/shaders/occlusion.glsl");
        assert!(occlusion.contains(&format!("const float STAGE_OCC_ALBEDO = {STAGE_OCC_ALBEDO:?};")));
        let water = include_str!("../../../render/shaders/water.frag");
        assert!(water.contains(&format!("const float RAY_TRANSPARENCIES = {WATER_RAY_TRANSPARENCIES:?};")));
    }

    #[test]
    fn the_forward_pass_shaders_hold_the_same_constants() {
        let numbers = |source: &str, name: &str| -> Vec<f32> {
            let line = source.lines().find(|l| l.contains(&format!(" {name} = "))).unwrap_or_else(|| panic!("no {name}"));
            let rhs = line.split(" = ").nth(1).unwrap().split(';').next().unwrap();
            rhs.split_once('(').map_or(rhs, |(_, inner)| inner.trim_end_matches(')')).split(',').map(|v| v.trim().parse().unwrap_or_else(|_| panic!("{name}: {v}"))).collect()
        };
        let pass = |name: &str| FORWARD_PASS.iter().find(|(n, _)| *n == name).unwrap().1.to_vec();
        let (vert, frag) = (include_str!("../../../render/shaders/model.vert"), include_str!("../../../render/shaders/model.frag"));
        assert_eq!(numbers(vert, "OL_INTENSITY"), pass("vOLIntensity"));
        assert_eq!(numbers(vert, "OL_Z_DECAY"), pass("fOLZDecayRatio"));
        assert_eq!(numbers(vert, "OL_CONSTANT"), pass("fOLThicknessConstantRatio"));
        assert_eq!(numbers(vert, "OL_ASPECT"), pass("fAdjByAspect"));
        assert_eq!(numbers(vert, "OL_Z_SCALE"), pass("fOLZScale"));
        assert_eq!(numbers(frag, "OL_COLOUR"), pass("vOLColorScaler")[..1]);
        assert_eq!(numbers(frag, "OL_LIGHT_FADE"), pass("fOLLtEfFade"));
        assert_eq!(numbers(frag, "OL_AMBIENT"), pass("vOLAmbEfScaler")[..1]);
        assert_eq!(numbers(frag, "OL_SHINE"), pass("cSpec")[3..]);
    }

    #[test]
    fn scat_undoes_the_exposure_on_the_light_rows_only() {
        let rows = [[-1.0, -2.0, -3.0, 0.0], [0.0, 0.0, 0.0, 1.0], [0.9, 1.8, 2.7, -2.0], [0.09, 0.18, 0.27, 2.0]];
        let v = scat(&rows, 0.9, false);
        assert_eq!(v.len(), 16);
        // the extinction row stays, its w (Barrel does not carry it) is left out
        assert_eq!(&v[..4], &[Some(-1.0), Some(-2.0), Some(-3.0), None]);
        // the light rows are divided by the exposure, their w stays
        assert!((v[8].unwrap() - 1.0).abs() < 1e-6 && (v[10].unwrap() - 3.0).abs() < 1e-6 && v[11] == Some(-2.0));
        assert!((v[12].unwrap() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn the_mirror_negates_x_only() {
        assert_eq!(world([0.5, -0.25, 1.0, 0.0]), [-0.5, -0.25, 1.0]);
    }

    #[test]
    fn two_lights_lay_out_as_the_games_arrays() {
        let (d, c) = lights([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], [[7.0, 8.0, 9.0], [10.0, 11.0, 12.0]]);
        assert_eq!(d.len(), 8);
        assert_eq!(c[3], Some(1.0));
        assert_eq!(c[4], Some(10.0));
        assert_eq!(d[7], Some(0.0));
    }
}

/// The planes `vS2VZ` (view depth minus `first_end`) and `sdplane` ((`last_end` - depth) /
/// `blend`) over the first cascade's map coordinates as the game's shader takes them (u, v,
/// depth, 1), `cascade` the map's view-projection in render space, `forward` and `eye` the
/// view's. The game's map is Barrel's mirrored: u = 0.5 - x / 2, v = 0.5 - y / 2.
pub fn shadow_planes(cascade: glam::Mat4, forward: glam::Vec3, eye: glam::Vec3, first_end: f32, last_end: f32, blend: f32) -> [[f32; 4]; 2] {
    let to_map = glam::Mat4::from_cols_array_2d(&[[-0.5, 0.0, 0.0, 0.0], [0.0, -0.5, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.5, 0.5, 0.0, 1.0]]);
    // A plane w over render space (w . (p, 1)) is s . (map(p), 1) with s = map^-T w.
    let back = (to_map * cascade).inverse().transpose();
    let depth = glam::Vec4::new(forward.x, forward.y, forward.z, -forward.dot(eye));
    let pick = back * (depth - glam::Vec4::W * first_end);
    let fade = back * ((glam::Vec4::W * last_end - depth) / blend);
    [pick.to_array(), fade.to_array()]
}

/// A shadow map's view-projection (render space to clip) as the game's matrix: game space (x
/// negated) to the map's u, v (mirrored, as the cascades': `shadow_planes`) and depth, stored
/// for row vectors.
fn game_map(vp: glam::Mat4) -> [f32; 16] {
    let to_map = glam::Mat4::from_cols_array_2d(&[[-0.5, 0.0, 0.0, 0.0], [0.0, -0.5, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.5, 0.5, 0.0, 1.0]]);
    (to_map * vp * glam::Mat4::from_diagonal(glam::Vec4::new(-1.0, 1.0, 1.0, 1.0))).to_cols_array()
}
