//! `barrel-ryza`: Atelier Ryza DX on Barrel. For now it draws one field offscreen:
//!
//!     barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--yaw DEG] [--pitch DEG]
//!                 [--look-at X,Y,Z] [--distance D] [--time morning|noon|evening|night]
//!                 [--weather sunny|cloudy|foggy] [--hide NAME]...
//!                 [--ryza X,Y,Z [--turn DEG] [--motion N] [--motion-time S]] [--no-shadows]
//!                 [--window [--fullscreen]] [--preset NAME] [--fov DEG] [--draw-ranges pc|nx|off]
//!
//! FIELD is a field's folder name in the archives (`s01_i_attic`). The camera frames the field's
//! ground (its terrain's square; the backdrop's mountains far outside it are drawn, not framed),
//! or looks at X,Y,Z (field units, as the placement list gives them) from D away. The light is
//! the field environment's set for the time and weather (noon, sunny by default). `--hide`
//! leaves out the objects whose placement names contain NAME (to find which object is which).
//! `--ryza` stands Ryza (`pc00a`) at X,Y,Z turned DEG about y, posed by her motion N
//! (`pc00a_<N>`, default 00000) at S seconds. Every draw is shown unlit
//! (its texture as it is) until Ryza's own lighting is read (`docs/ryza/open.md`).
//!
//! `--ryza` with no `--look-at` frames her as the play camera does (the field's free camera,
//! behind her). `--window` opens a window instead of writing OUT, and with `--ryza` the player walks her:
//! W, A, S, D (or the left stick, a light lean walking) move her against the camera, Shift
//! walks, the arrows (or the right stick) turn the camera, - and = (the shoulders) bring it
//! nearer or farther, Escape quits. She stands on the field's heightmap and stops at its
//! `_le.koc` walls (`barrel_ryza::player`).

use std::path::Path;

use ash::vk;
use barrel_formats::elixir::Elixir;
use barrel_formats::g1m::G1m;
use barrel_formats::g1t::G1t;
use barrel_render::renderer::{Camera, Frame, Renderer};
use barrel_render::settings::RenderSettings;
use barrel_render::*;
use barrel_ryza::environment::{Light, Time, Weather};
use barrel_ryza::install::Install;
use glam::{Mat4, Vec3};

/// The renderer's material bit for a draw shown with its texture alone, unlit (`model.frag`).
const UNLIT: u32 = 32;
/// `model.vert`'s flag for a sky dome lit by the look's `DOME_LIGHT_SLOT`.
const DOME_LIT: u32 = 4096;
/// `model.frag`'s flag for a normal map in the mask slot (a field model's `sNMap`).
const NORMAL_MAPPED: u32 = 8192;
/// `model.vert`'s flag for billboards shrinking away over the look's `FADE_SLOT` range (the
/// plants, as `ObjectGrassPBS` does).
const FADED: u32 = 16384;
/// The plants' fade range from the eye, the game's highest grass preset (`Middle` in
/// `grassrangepreset.xml`: FarClipBegin 8000, FarClipEnd 15000).
const PLANT_FADE: [f32; 2] = [8000.0, 15000.0];

struct Args {
    install: String,
    field: String,
    out: String,
    size: (u32, u32),
    yaw: f32,
    pitch: f32,
    look_at: Option<Vec3>,
    distance: Option<f32>,
    time: Time,
    weather: Weather,
    hide: Vec<String>,
    ryza: Option<(Vec3, f32, String, f32)>,
    no_shadows: bool,
    /// The camera's vertical field of view, degrees (`--fov`).
    fov: Option<f32>,
    /// The field's draw distances: the PC game's (`pc`, the default), the Switch's (`nx`) or none
    /// (`off`) (`--draw-ranges`).
    draw_ranges: String,
    window: bool,
    /// Graphics: a preset by name (`RenderSettings::preset`), and fullscreen.
    preset: Option<RenderSettings>,
    fullscreen: bool,
    /// Frames to draw: in a window, quit after this many; offscreen, the same frame this many
    /// times, the last written (for `--timings`). And a lean held all along in a window (tests).
    frames: Option<u32>,
    /// Each pass's GPU time, reported at the end (`--timings`).
    timings: bool,
    lean: [f32; 2],
}

/// Ryza as posed and walked: her skeleton, her motions by number, her instance, the field's
/// ground and walls under her, and the player's state.
struct Rig {
    skeleton: barrel_formats::g1m::Skeleton,
    anims: Vec<(u32, barrel_formats::g1a::Animation)>,
    instance: u32,
    ground: Option<barrel_engine::walk::Ground>,
    walls: Option<barrel_engine::walk::Walls>,
    player: barrel_ryza::player::Player,
    chara: barrel_ryza::chara::Chara,
    /// Her face's expression and her shut eyes (`barrel_ryza::face`).
    face: Option<barrel_ryza::face::Face>,
    shut: Option<barrel_ryza::face::Face>,
}

fn args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let usage = "usage: barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--yaw DEG] [--pitch DEG] [--look-at X,Y,Z] [--distance D] [--time T] [--weather W]";
    let (install, field, out) = (it.next().ok_or(usage)?, it.next().ok_or(usage)?, it.next().ok_or(usage)?);
    let mut a = Args { install, field, out, size: (1280, 720), yaw: 0.0, pitch: 30.0, look_at: None, distance: None, time: Time::Noon, weather: Weather::Sunny, hide: Vec::new(), ryza: None, no_shadows: false, fov: None, draw_ranges: "pc".into(), window: false, preset: None, fullscreen: false, timings: false, frames: None, lean: [0.0; 2] };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                a.size = (w.parse()?, h.parse()?);
            }
            "--yaw" => a.yaw = value()?.parse()?,
            "--pitch" => a.pitch = value()?.parse()?,
            "--look-at" => {
                let v = value()?.split(',').map(str::parse).collect::<std::result::Result<Vec<f32>, _>>()?;
                let [x, y, z] = v[..] else { return Err("--look-at X,Y,Z".into()) };
                a.look_at = Some(Vec3::new(x, y, z));
            }
            "--distance" => a.distance = Some(value()?.parse()?),
            "--hide" => a.hide.push(value()?),
            "--no-shadows" => a.no_shadows = true,
            "--fov" => a.fov = Some(value()?.parse()?),
            "--draw-ranges" => a.draw_ranges = value()?,
            "--window" => a.window = true,
            "--fullscreen" => a.fullscreen = true,
            "--preset" => {
                let name = value()?;
                a.preset = Some(RenderSettings::preset(&name).ok_or(format!("--preset {name}: no such preset"))?);
            }
            "--timings" => a.timings = true,
            "--frames" => a.frames = Some(value()?.parse()?),
            "--lean" => {
                let v = value()?.split(',').map(str::parse).collect::<std::result::Result<Vec<f32>, _>>()?;
                let [x, y] = v[..] else { return Err("--lean X,Y".into()) };
                a.lean = [x, y];
            }
            "--ryza" => {
                let v = value()?.split(',').map(str::parse).collect::<std::result::Result<Vec<f32>, _>>()?;
                let [x, y, z] = v[..] else { return Err("--ryza X,Y,Z".into()) };
                a.ryza = Some((Vec3::new(x, y, z), 0.0, "00000".into(), 0.0));
            }
            "--turn" => a.ryza.as_mut().ok_or("--turn after --ryza")?.1 = value()?.parse()?,
            "--motion" => a.ryza.as_mut().ok_or("--motion after --ryza")?.2 = value()?,
            "--motion-time" => a.ryza.as_mut().ok_or("--motion-time after --ryza")?.3 = value()?.parse()?,
            "--time" => {
                a.time = match value()?.as_str() {
                    "morning" => Time::Morning,
                    "noon" => Time::Noon,
                    "evening" => Time::Evening,
                    "night" => Time::Night,
                    t => return Err(format!("--time {t}: morning, noon, evening or night").into()),
                }
            }
            "--weather" => {
                a.weather = match value()?.as_str() {
                    "sunny" => Weather::Sunny,
                    "cloudy" => Weather::Cloudy,
                    "foggy" => Weather::Foggy,
                    w => return Err(format!("--weather {w}: sunny, cloudy or foggy").into()),
                }
            }
            _ => return Err(format!("unknown option {flag}").into()),
        }
    }
    Ok(a)
}

/// The way a directional light travels, in render space: from `elevation` degrees above the
/// horizon and `azimuth` degrees around y from +z toward +x (game space; the convention is a
/// stand-in, not read from the game, `docs/ryza/open.md`), x mirrored.
fn from_light(elevation: f32, azimuth: f32) -> Vec3 {
    let (e, a) = (elevation.to_radians(), azimuth.to_radians());
    let toward = Vec3::new(e.cos() * a.sin(), e.sin(), e.cos() * a.cos());
    -toward * Vec3::new(-1.0, 1.0, 1.0)
}

/// How much of the stage ambient tint's hue the set's ambient takes (a power on it): fitted
/// to six shaded regions of the gate's noon still (RGB error 77 at 0.4, 80 at 0.6, 118 at the
/// full hue, 95 without).
const TINT_STRENGTH: f32 = 0.4;

/// What the frame's exposure maps to the set's middle gray: the light falling on an
/// upward-facing surface over this. Fitted with the tone curve (`Frame::tone_white`, the set's
/// `fLumiMax`) and the exposure cap (`vExposureRange`'s top) taken in the game's units as they
/// are, against the game's still at the garden gate (noon, the player's save): its grey
/// percentiles 25 / 50 / 90 come out 58 / 69 / 168 against 55 / 70 / 219, nine regions' RGB
/// error 163 with the float scene (`RenderSettings::hdr_scene`; 183 in 8 bits; 26 and up
/// brighter but worse, 30 and up at the exposure cap). The sky stays darker than the still's.
const EXPOSURE_DIVISOR: f32 = 22.0;

/// The sky plane's in-scattered light scaled up, fitted to the gate's noon still (a stand-in:
/// the game's own scale is not read). The sky region comes out 162, 201, 199 against the
/// still's 172, 218, 233, ten regions' RGB error 145 to 137 (2 and 3 the same, 5 worse); the
/// path length's factor changes nothing there (docs/ryza/open.md).
const SKY_SCALE: f32 = 2.5;

/// One exposure for the frame, standing in for the game's adaptation (`trueHDR_`, not read):
/// see `EXPOSURE_DIVISOR`.
fn exposure(c: &[[f32; 3]; 9], light: &Light) -> f32 {
    let luma = |v: [f32; 3]| 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
    let up = barrel_ryza::environment::basis([0.0, 1.0, 0.0]);
    let sky: [f32; 3] = std::array::from_fn(|ch| (0..9).map(|i| c[i][ch] * up[i]).sum::<f32>() * light.ambient[ch]);
    let sun = light.parallel.iter().map(|p| luma(p.color) * p.elevation.to_radians().sin().max(0.0)).sum::<f32>();
    let lit = (luma(sky) + sun) / EXPOSURE_DIVISOR;
    if lit > 0.0 { light.middle_gray / lit } else { 1.0 }
}

/// The plant card each kind draws from `_grass.g1t`'s atlas (4 × 2 cards: 0 to 3 the top row,
/// weeds, a leafy stem, a flower, a bush; 4 to 7 the bottom, wheat, a grass tuft, cornflowers,
/// lupins). The game picks them in its code (not read): a stand-in by the stills, kind 7 (the
/// large yellow fields) as wheat and kind 6 (along the paths) as the grass tuft.
const PLANT_CARDS: [u32; 19] = [1, 0, 2, 3, 0, 0, 5, 4, 2, 6, 7, 1, 3, 0, 5, 5, 6, 4, 5];
/// The most plants a field draws (Rasenboden: about 36,000 at the kinds' densities).
const PLANT_LIMIT: usize = 200_000;
/// Plants are grouped into draws of this many grid cells a side, so the view culls them.
const PLANT_BLOCK: usize = 8;
/// The plants' normal in the card's frame (turned by the billboard axes): up, so a plant takes
/// the light of the ground it stands on rather than of a wall facing the camera.
const PLANT_NORMAL: [f32; 3] = [0.0, 1.0, 0.0];

/// The field's plants as camera-facing cards on its heightmap (`barrel_ryza::grass`), one draw
/// per block of grid cells, the atlas being texture `atlas`.
fn add_plants(scene: &mut Scene, g: &barrel_ryza::grass::Grass, main: &Elixir, area: &str, atlas: usize) {
    let Some(ground) = barrel_engine::field::named(main, &format!("{}.khm", area.to_ascii_uppercase()))
        .or_else(|| main.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".khm")).map(|e| main.data(e)))
        .and_then(|b| barrel_formats::khm::parse(b).ok())
        .map(barrel_engine::walk::Ground::of_heightmap)
    else {
        return;
    };
    let plants = g.plants(PLANT_LIMIT, |x, z| ground.height(x, z, f32::MAX / 2.0, 0.0));
    let [cx, cz] = g.cell();
    let mut blocks: std::collections::BTreeMap<(usize, usize), Vec<&barrel_ryza::grass::Plant>> = Default::default();
    for p in &plants {
        blocks.entry(((p.at[0] / cx) as usize / PLANT_BLOCK, (p.at[2] / cz) as usize / PLANT_BLOCK)).or_default().push(p);
    }
    let instance = scene.instance(Mat4::IDENTITY);
    for list in blocks.values() {
        let first_index = scene.indices.len() as u32;
        for p in list {
            let card = PLANT_CARDS.get(p.kind as usize).copied().unwrap_or(0);
            let (u, v) = ((card % 4) as f32 * 0.25, (card / 4) as f32 * 0.5);
            let [w, h] = p.size;
            // Vertices are built in render space: game x mirrored (`render_space`).
            let centre = [-p.at[0], p.at[1] + h * 0.5, p.at[2]];
            let base = scene.vertices.len() as u32;
            for (dx, dy, uv) in [(-0.5, -0.5, [u, v + 0.5]), (0.5, -0.5, [u + 0.25, v + 0.5]), (0.5, 0.5, [u + 0.25, v]), (-0.5, 0.5, [u, v])] {
                scene.vertices.push(Vertex { pos: centre, normal: PLANT_NORMAL, uv, joints: [0; 4], weights: [0; 4], morph: [0; 2], color: [255; 4], corner: [dx * w, dy * h, 0.0] });
            }
            scene.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        scene.draws.push(Draw {
            first_index,
            index_count: scene.indices.len() as u32 - first_index,
            texture: atlas,
            overlay: None,
            instance,
            mask: None,
            normal_map: None,
            eye: false,
            sky: false,
            outline: None,
            field: Some(0),
            casts: false,
            name: "plants".into(),
            version: None,
            light: 0,
            tint: 0,
            bounds: None,
            class: 0,
        });
    }
    eprintln!("plants: {} in {} draws", plants.len(), blocks.len());
}

fn main() -> Result<()> {
    let a = args()?;
    let install = Install::open(Path::new(&a.install))?;
    let files = install.field(&a.field)?;
    let (main, objects) = (Elixir::parse(&files.main)?, Elixir::parse(&files.objects)?);
    let mut scene = Scene::new();
    scene.field = Some(0);
    // Ryza's placements carry no tags (Escha marks its casters `xDS_`): every placed model
    // casts the sun's shadow; nothing sways yet (`docs/ryza/open.md`).
    // Each keeps its placement's draw class (near, middle, far, landmark), for the field's draw
    // distances.
    let place = |_: &barrel_formats::opl2::PlacedType, i: &barrel_formats::opl2::Instance| barrel_engine::field::Placement { casts: true, class: (i.word & 3) as u8, ..Default::default() };
    let (pack, _) = barrel_engine::field::load(&files.area, &main, &objects, &mut scene, 0, &place)?;
    let pack = G1t::parse(&pack)?;
    // The field's plants (`_grass.bin`, `_grass.g1t`: barrel_ryza::grass), when it has them.
    let grass = barrel_engine::field::named(&main, &format!("{}_grass.bin", files.area)).and_then(|b| barrel_ryza::grass::Grass::parse(b).ok());
    let grass_pack = barrel_engine::field::named(&main, &format!("{}_grass.g1t", files.area)).and_then(|b| G1t::parse(b).ok());
    let mut textures: Vec<&barrel_formats::g1t::Texture> = pack.textures.iter().collect();
    // Ryza herself: her mesh, posed by her skeleton and a motion, her textures after the
    // field's. Lit as the field's models are until her own shaders are read
    // (`docs/ryza/open.md`).
    let ryza_files = match &a.ryza {
        Some((_, _, motion, _)) => {
            let dir = "\\data\\x64\\character\\pc00a\\";
            // The motion asked for, and in a window the ones the player plays.
            let mut motions = vec![(motion.parse::<u32>()?, install.read(&format!("{dir}pc00a_{motion}.elixir.gz"))?)];
            if a.window {
                use barrel_ryza::motion as m;
                for n in [m::STAND, m::WALK_START, m::WALK, m::WALK_STOP, m::RUN_START, m::RUN, m::RUN_STOP] {
                    motions.push((n, install.read(&barrel_ryza::motion::path("PC00A", n))?));
                }
            }
            Some((install.read(&format!("{dir}pc00a_model.elixir.gz"))?, install.read(&format!("{dir}pc00a_model.g1t"))?, motions))
        }
        None => None,
    };
    let ryza_pack = ryza_files.as_ref().map(|(_, t, _)| G1t::parse(t)).transpose()?;
    // The characters' toon ramp (`DSChara-Cll`'s `sRamp`, docs/shading.md).
    let ramp_bytes = match ryza_files {
        Some(_) => Some(install.read("\\data\\x64\\shader\\texture\\chara_rmp.g1t")?),
        None => None,
    };
    let ramp_pack = ramp_bytes.as_deref().map(G1t::parse).transpose()?;
    // Her draws' ramp rows by (first index, instance): each submesh's `rampIndex`, and whether
    // it has a rim (`rmIndex`).
    let mut ramp_rows: std::collections::HashMap<(u32, u32), (u32, bool)> = Default::default();
    // The characters' rim (`environment::Rim`, the default environment override's).
    let rim = match ryza_files {
        Some(_) => install.read(barrel_ryza::environment::Rim::DEFAULT_PATH).ok().and_then(|b| barrel_ryza::environment::Rim::parse(&b).map_err(|e| eprintln!("{e}")).ok()),
        None => None,
    };
    eprintln!("character rim: {rim:?}");
    // The exposure's cap (`exposure`): the default override's `trueHDR_vExposureRange` top.
    // The sets hold their own range (`Light::exposure_range`, 0.9 to 2.4 at Rasenboden), whose
    // top would leave evening uncapped: how the game's adaptation uses it is not read.
    let exposure_range = install.read(barrel_ryza::environment::Rim::DEFAULT_PATH).ok().and_then(|b| barrel_ryza::environment::override_values(&b, "trueHDR_vExposureRange").ok()).filter(|v| v.len() == 2).map(|v| (v[0], v[1]));
    let rim_word = |sharpness: f32, strength: f32| u32::from_le_bytes([(sharpness / 64.0 * 255.0).round().clamp(0.0, 255.0) as u8, (strength / 4.0 * 255.0).round().clamp(0.0, 255.0) as u8, 0, 0]);
    let mut rig: Option<Rig> = None;
    if let (Some((at, turn, motion, time)), Some((model, _, motions)), Some(rp)) = (&a.ryza, &ryza_files, &ryza_pack) {
        let model = Elixir::parse(model)?;
        let skeleton = G1m::parse(barrel_engine::field::named(&model, "PC00A_MODEL.g1m").ok_or("no PC00A_MODEL.g1m")?)?.skeleton.ok_or("Ryza's model has no skeleton")?;
        let mesh = G1m::parse(barrel_engine::field::named(&model, "PC00A_MODEL_default.g1m").ok_or("no PC00A_MODEL_default.g1m")?)?;
        let geometry = mesh.geometry.as_ref().ok_or("Ryza's mesh has no geometry")?;
        let mut anims = Vec::new();
        for (n, bytes) in motions {
            let c = Elixir::parse(bytes)?;
            anims.push((*n, barrel_formats::g1a::parse(c.entries.iter().find(|e| !e.is_dummy()).map(|e| c.data(e)).ok_or("empty motion")?)?));
        }
        let anim = &anims[0].1;
        // Her face (`barrel_ryza::face`): the default expression over the body's pose, and the
        // shut eyes for blinking.
        let face_of = |name: &str| -> Option<barrel_ryza::face::Face> {
            let bytes = install.read(&barrel_ryza::face::path("PC00A", name)).ok()?;
            let c = Elixir::parse(&bytes).ok()?;
            let anim = barrel_formats::g1a::parse(c.data(c.entries.iter().find(|e| !e.is_dummy())?)).ok()?;
            Some(barrel_ryza::face::Face::new(&skeleton, &anim))
        };
        let (face, shut) = (face_of("expression_default"), face_of("closeeye_down"));
        let mut locals = barrel_engine::skin::local_pose(&skeleton, anim, *time);
        if let Some(f) = &face {
            f.apply(&mut locals, None, 0.0);
        }
        let joints = barrel_engine::skin::skin(&skeleton, &locals);
        // Her feet on the ground: the field's heightmap under her (the given height when there
        // is none, or she is off it).
        let mut ground = barrel_engine::field::named(&main, &format!("{}.khm", files.area.to_ascii_uppercase()))
            .or_else(|| main.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".khm")).map(|e| main.data(e)))
            .and_then(|b| barrel_formats::khm::parse(b).ok())
            .map(barrel_engine::walk::Ground::of_heightmap);
        // And the field's walls (`<field>_le.koc`), when it has them.
        let walls = install
            .read(&format!("\\data\\x64\\field\\{0}\\{0}_le.koc", a.field.to_ascii_lowercase()))
            .ok()
            .and_then(|b| barrel_ryza::koc::parse(&b).ok())
            .map(|q| barrel_engine::walk::Walls::from_walls(barrel_ryza::koc::walls(&q), 2000.0));
        // And the placed models' collision meshes (`barrel_ryza::solids`): more walls, and floors
        // on models, added after she is stood on the terrain (so she does not start on a
        // tree's top).
        let placements = barrel_engine::field::named(&main, &format!("{}.opl2", files.area)).map(barrel_formats::opl2::parse).transpose()?;
        let mut solids = placements.map(|p| barrel_ryza::solids::placed_meshes(&p, &objects)).unwrap_or_default();
        // And the field's own shapes (`<FIELD>_coll.bin`).
        if let Some(b) = main.entries.iter().find(|e| !e.is_dummy() && e.name.to_ascii_lowercase().ends_with("_coll.bin")).map(|e| main.data(e)) {
            solids.extend(barrel_ryza::solids::field_shapes(b)?);
        }
        let at = &match ground.as_ref().and_then(|g| g.height(at.x, at.z, f32::MAX / 2.0, 0.0)) {
            Some(y) => Vec3::new(at.x, y, at.z),
            None => *at,
        };
        eprintln!("Ryza stands at {at:?}");
        let slope = barrel_engine::walk::DEFAULT_SLOPE;
        if let Some(g) = ground.as_mut() {
            for c in &solids {
                g.add_floors(c, slope);
            }
        }
        let walls = solids.iter().fold(walls, |w, c| {
            let more = barrel_engine::walk::Walls::new(c, slope);
            Some(match w {
                Some(w) => w.join(more),
                None => more,
            })
        });
        eprintln!("model collision: {} meshes, {} triangles", solids.len(), solids.iter().map(|c| c.triangles.len()).sum::<usize>());
        let first = scene.draws.len();
        // Her settings from the character tables (`barrel_ryza::chara`): her model's scale, her
        // size against the walls, the camera's look height.
        let chara = barrel_ryza::chara::Chara::read(&install.read("\\saves\\chara\\chara.xml")?, &install.read("\\saves\\chara\\charabase.xml")?, "PC00A")?;
        eprintln!("Ryza: {chara:?}");
        let instance = scene.instance(Mat4::from_scale_rotation_translation(Vec3::from(chara.size), glam::Quat::from_rotation_y(turn.to_radians()), *at));
        scene.add_posed(geometry, Mat4::IDENTITY, rp.textures.len(), Some(&joints), instance)?;
        // Her submeshes' ramp rows (`rampIndex`), one draw each in order for those with UVs and
        // normals (the others draw nothing, `Scene::add_posed`); the ramp texture follows her
        // own.
        let drawn: Vec<&barrel_formats::g1m::Submesh> = geometry.submeshes.iter().filter(|s| geometry.simple_mesh(s).is_ok_and(|m| !m.uvs.is_empty() && !m.normals.is_empty())).collect();
        let rows: Vec<(u32, bool)> = drawn.iter().map(|s| (geometry.param_record(s, "rampIndex").and_then(|p| p.integer()).unwrap_or(1).min(255), geometry.param_record(s, "rmIndex").is_some())).collect();
        // Her hair's highlight (`sAlbedoRetouch2HighlightMap`, texture kind 66): laid over the
        // hair's colour by its alpha. The game shifts it along v by `dot(V, alr2HDir) ×
        // alr2HAdj` (-0.11 at most, about 0.015 at the field camera's pitch): laid on unshifted
        // (docs/shading.md).
        let highlights: Vec<Option<usize>> = drawn.iter().map(|s| geometry.materials.get(s.material as usize).and_then(|m| m.textures.iter().find(|t| t.kind == 66)).map(|t| t.index as usize)).collect();
        if scene.draws.len() - first == highlights.len() {
            for (d, h) in scene.draws[first..].iter_mut().zip(&highlights) {
                if let Some(h) = h {
                    d.overlay = Some(*h);
                }
            }
        }
        if scene.draws.len() - first == rows.len() {
            for (d, &row) in scene.draws[first..].iter().zip(&rows) {
                ramp_rows.insert((d.first_index, d.instance), row);
            }
        } else {
            eprintln!("Ryza: {} draws for {} submeshes: no toon ramp", scene.draws.len() - first, rows.len());
        }
        for d in &mut scene.draws[first..] {
            if ramp_rows.contains_key(&(d.first_index, d.instance)) {
                d.mask = Some(rp.textures.len());
            }
            d.texture += textures.len();
            d.mask = d.mask.map(|m| m + textures.len());
            d.overlay = d.overlay.map(|o| o + textures.len());
            d.field = Some(0);
            d.name = "ryza".into();
            d.casts = true;
        }
        eprintln!("Ryza: {} draws, {} joints, motion {motion} {:.2}s long", scene.draws.len() - first, joints.len(), anim.duration);
        let lengths: Vec<(u32, f32)> = anims.iter().map(|(n, a)| (*n, a.duration)).collect();
        let mut player = barrel_ryza::player::Player::new(*at, turn.to_radians(), &lengths);
        (player.radius, player.height) = (chara.radius, chara.height);
        // Her speeds: those at which her planted feet keep still in the walk and run loops
        // (`motion::stride_speed`), at her model's scale.
        let stride = |m: u32| anims.iter().find(|(n, _)| *n == m).and_then(|(_, a)| barrel_ryza::motion::stride_speed(&skeleton, a)).map(|v| v * chara.size[2]);
        if let Some(v) = stride(barrel_ryza::motion::WALK) { player.walk_speed = v; }
        if let Some(v) = stride(barrel_ryza::motion::RUN) { player.run_speed = v; }
        eprintln!("Ryza: walk {:.0}, run {:.0} units a second", player.walk_speed, player.run_speed);
        rig = Some(Rig { skeleton, anims, instance, ground, walls, player, chara, face, shut });
        textures.extend(rp.textures.iter());
        if let Some(r) = &ramp_pack {
            textures.extend(r.textures.first());
        }
    }
    // The set's sky dome (its clouds; the blue is the sky plane's): its model and textures,
    // drawn as the distant view (unlit, after the sky plane, alpha-blended). Where the game
    // puts it (`SkyDomeWorldPosition`, `WorldScale`) is not read: centred under the view's
    // target at height 0 (`docs/ryza/open.md`).
    let dome_files = match files.env.as_deref().map(Elixir::parse).transpose()? {
        Some(env) => match barrel_engine::field::named(&env, &format!("{}_env.bin", files.area)) {
            Some(bin) => match barrel_ryza::environment::sky_dome(bin, a.time, a.weather)? {
                Some(path) => Some((path.clone(), install.read(&path)?)),
                None => None,
            },
            None => None,
        },
        None => None,
    };
    let dome_container = dome_files.as_ref().map(|(_, b)| Elixir::parse(b)).transpose()?;
    let dome_parts = match &dome_container {
        Some(c) => {
            let real: Vec<_> = c.entries.iter().filter(|e| !e.is_dummy()).collect();
            let model = real.iter().find_map(|e| G1m::parse(c.data(e)).ok().filter(|m| m.geometry.is_some()));
            let pack = real.iter().find_map(|e| G1t::parse(c.data(e)).ok());
            model.zip(pack)
        }
        None => None,
    };
    let mut dome_instance = None;
    if let Some((model, dp)) = &dome_parts {
        let at = a.look_at.or(a.ryza.as_ref().map(|r| r.0)).unwrap_or(Vec3::ZERO);
        let first = scene.draws.len();
        let instance = scene.instance(Mat4::from_translation(Vec3::new(at.x, 0.0, at.z)));
        dome_instance = Some(instance);
        scene.add_posed(model.geometry.as_ref().unwrap(), Mat4::IDENTITY, dp.textures.len(), None, instance)?;
        for d in &mut scene.draws[first..] {
            d.texture += textures.len();
            d.mask = d.mask.map(|m| m + textures.len());
            d.overlay = d.overlay.map(|o| o + textures.len());
            d.field = Some(0);
            d.sky = true;
            d.casts = false;
            d.name = "sky dome".into();
        }
        eprintln!("sky dome {}: {} draws", dome_files.as_ref().unwrap().0, scene.draws.len() - first);
        textures.extend(dp.textures.iter());
    }
    if let (Some(g), Some(gp)) = (&grass, &grass_pack) {
        if let Some(atlas) = gp.textures.first() {
            add_plants(&mut scene, g, &main, &files.area, textures.len());
            textures.push(atlas);
        }
    }
    let Scene { mut vertices, indices, mut draws, terrains, lo, hi, mut instances, mut palette, deltas, morph_weights, .. } = scene;
    // The models' Color0 does not reach their materials: the G-buffer programs (`pbb-*_gb.g1s`,
    // docs/shading.md) take no COLOR0, and some models hold masks there (the attic's
    // cabinet). So white; the terrain's (its baked shade) is kept.
    for d in &draws {
        for &i in &indices[d.first_index as usize..(d.first_index + d.index_count) as usize] {
            vertices[i as usize].color = [255; 4];
        }
    }
    // A field model's normal map goes in the mask slot (unused by the field's models
    // otherwise; `NORMAL_MAPPED`); her draws keep the toon ramp there.
    for d in draws.iter_mut() {
        if !d.sky && d.normal_map.is_some() && !ramp_rows.contains_key(&(d.first_index, d.instance)) {
            d.mask = d.normal_map;
        }
    }
    for d in draws.iter_mut() {
        // Ryza's models' lighting material (G1MG section 0x10001) holds ambient 0: their
        // shaders light from the environment, not `cAmb`. Full until those shaders are read
        // (`docs/ryza/open.md`).
        d.light = FULL_LIGHT;
        let base = instances.get(d.instance as usize).map_or(0, |i| i.palette);
        d.bounds = Bounds::of(&vertices, &indices, d.first_index, d.index_count, &palette, base);
    }
    if draws.is_empty() {
        return Err("nothing to draw".into());
    }
    // The field's light: its environment's diffuse cube map, and the set for the time and
    // weather (sun, second light, ambient colour).
    let environment = match files.env.as_deref().map(Elixir::parse).transpose()? {
        Some(env) => {
            let ad = barrel_engine::field::named(&env, &format!("{}_ad.g1t", files.area)).ok_or("environment has no diffuse light map")?;
            let (c, _) = barrel_ryza::environment::diffuse_harmonics(ad)?;
            let bin = barrel_engine::field::named(&env, &format!("{}_env.bin", files.area)).ok_or("environment has no settings")?;
            let mut light = Light::read(bin, a.time, a.weather)?;
            // The ambient's colour: the set's (per time of day) tinted by the hue of the
            // stage's ambient tint (`environment::stage_ambient`, the default override: the
            // tint over its luma, about 0.80, 0.96, 1.95). A stand-in: the game scales the light
            // map by the tint and leaves the set's colour out (`AmbientLightScale` 0), but the
            // field has one light map for all times and the tint is the same for all, so the
            // time of day's colour would be lost. Against the gate's noon still the shade comes
            // out blue-grey as the game's (the set's colour alone drew it brown).
            if let Some((tint, _)) = install.read(barrel_ryza::environment::Rim::DEFAULT_PATH).ok().and_then(|b| barrel_ryza::environment::stage_ambient(&b)) {
                let luma = 0.2126 * tint[0] + 0.7152 * tint[1] + 0.0722 * tint[2];
                if luma > 0.0 {
                    light.ambient = std::array::from_fn(|ch| light.ambient[ch] * (tint[ch] / luma).powf(TINT_STRENGTH));
                }
            }
            let (_, sky_fog) = barrel_ryza::environment::Fog::read(bin, a.time, a.weather)?;
            eprintln!("light ({:?}, {:?}): {light:?}", a.time, a.weather);
            Some((c, light, sky_fog))
        }
        None => None,
    };
    eprintln!("{} draws, {} vertices, {} triangles, {} textures", draws.len(), vertices.len(), indices.len() / 3, textures.len());

    let (mut width, mut height) = a.size;
    // A window to play in (--window), or an offscreen image.
    // Graphics as Escha's: --preset, else Barrel's graphics config (in a window, or offscreen
    // when `BARREL_CONFIG` names one), else a launcher's default (`BARREL_DEFAULT_PRESET`,
    // `BARREL_DEFAULT_FULLSCREEN=1` on a handheld), else the desktop's.
    let config = (a.window || std::env::var_os("BARREL_CONFIG").is_some()).then(|| settings::RenderSettings::config_path().and_then(|p| std::fs::read_to_string(p).ok())).flatten();
    let graphics = a
        .preset
        .or(config.as_deref().map(RenderSettings::from_config))
        .or(std::env::var("BARREL_DEFAULT_PRESET").ok().and_then(|n| RenderSettings::preset(&n)))
        .unwrap_or(RenderSettings::DESKTOP)
        .resolved();
    // The game's scene is HDR (`trueHDR_`): drawn into a float target, so the tone curve
    // (`Frame::tone_white`) sees past the half range's 2.0.
    let graphics = RenderSettings { hdr_scene: true, ..graphics };
    let display = match config.as_deref() {
        Some(text) => settings::Display::from_config(text),
        None => settings::Display { width, height, fullscreen: std::env::var_os("BARREL_DEFAULT_FULLSCREEN").is_some_and(|v| v == "1") },
    };
    let display = settings::Display { fullscreen: display.fullscreen || a.fullscreen, ..display };
    if a.window {
        eprintln!("graphics: {graphics:?}, {display:?}");
    }
    let mut pump = a.window.then(|| window::Pump::open(display)).transpose()?;
    let display = pump.as_ref().map(|p| raw_window_handle::HasDisplayHandle::display_handle(p.window()).map(|h| h.as_raw())).transpose()?;
    let gpu = Gpu::new(display)?;
    let mut presenter = match (&pump, display) {
        (Some(p), Some(display)) => {
            let handle = raw_window_handle::HasWindowHandle::window_handle(p.window())?.as_raw();
            let size = p.window().inner_size();
            Some(present::Presenter::new(&gpu, display, handle, size.width, size.height)?)
        }
        _ => None,
    };
    if let Some(p) = &presenter {
        (width, height) = (p.extent.width, p.extent.height);
        eprintln!("window: {width}x{height}");
    }
    let mut renderer = Renderer::new(&gpu, graphics, presenter.as_ref(), width, height, a.timings, false)?;
    let d = &gpu.device;
    // In a window the joints move: last frame's go to the palette's second half (the motion
    // vectors').
    let mut prev_palette = palette.clone();
    if pump.is_some() {
        for i in &mut instances {
            i.prev_palette = i.palette + palette.len() as u32;
        }
    }
    let vbytes = unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * size_of::<Vertex>()) };
    let ibytes = unsafe { std::slice::from_raw_parts(indices.as_ptr() as *const u8, indices.len() * 4) };
    // Her hair's highlights laid over its colour (`composite_overlays`, as Escha's eyes).
    let composites = barrel_render::composite_overlays(&mut draws, &textures);
    let composite_textures: Vec<barrel_formats::g1t::Texture> = composites
        .iter()
        .map(|(w, h, px)| barrel_formats::g1t::Texture { width: *w, height: *h, format: barrel_formats::g1t::Format::Rgba8, format_code: 1, layers: 1, volume: false, levels: vec![&px[..]] })
        .collect();
    let textures: Vec<&barrel_formats::g1t::Texture> = textures.iter().copied().chain(composite_textures.iter()).collect();
    let (uploaded, outline_tex, white) = upload_textures(&gpu, renderer.pool, &textures, None)?;
    let grounds = upload_grounds(&gpu, renderer.pool, &terrains)?;
    let data = scene_gpu::SceneData { vertices: vbytes, indices: ibytes, textures: uploaded, instances: &instances, palette: &palette, deltas: &deltas, weights: &morph_weights, grounds, patterns: vec![None], white };
    let scene = scene_gpu::SceneGpu::new(&gpu, data, &renderer.shared)?;

    // The camera: on the given point (or on Ryza in a window), framing the ground (render
    // space mirrors x: the terrain spans x -extent..0, z 0..extent), or everything when there
    // is no terrain.
    let ground = terrains.first().map(|(_, t)| t.extent);
    let (center, radius) = match (a.look_at, ground) {
        (Some(p), _) => (p * Vec3::new(-1.0, 1.0, 1.0), 0.0),
        (None, Some([ex, ez])) => {
            let (mut ylo, mut yhi) = (f32::MAX, f32::MIN);
            for v in vertices.iter().filter(|v| v.pos[0] <= 0.0 && v.pos[0] >= -ex && v.pos[2] >= 0.0 && v.pos[2] <= ez) {
                (ylo, yhi) = (ylo.min(v.pos[1]), yhi.max(v.pos[1]));
            }
            let y = if ylo <= yhi { (ylo + yhi) * 0.5 } else { (lo.y + hi.y) * 0.5 };
            (Vec3::new(-ex * 0.5, y, ez * 0.5), Vec3::new(ex, yhi - ylo, ez).length() * 0.5)
        }
        (None, None) => ((lo + hi) * 0.5, (hi - lo).length() * 0.5),
    };
    // Following Ryza (in a window, or offscreen with no --look-at): the field's free camera (its field of view, distance and pitch
    // with their ranges), behind her (its yaw is against her facing).
    let follow = (rig.is_some() && (pump.is_some() || a.look_at.is_none())).then(|| {
        install.read(&barrel_ryza::camera::FreeCamera::path(&a.field)).ok().and_then(|b| barrel_ryza::camera::FreeCamera::parse(&b).map_err(|e| eprintln!("{e}")).ok()).unwrap_or(barrel_ryza::camera::FreeCamera::DEFAULT)
    });
    let fov = a.fov.unwrap_or(follow.map_or(30.0, |c| c.fov)).to_radians();
    // How high above her feet the camera looks: a stand-in (110). Her `CamH` (160) with the
    // field camera's `at_diff_y` put the camera inside the garden gate's tree, where the game's
    // view is clear: what they are is not known (`docs/ryza/open.md`).
    let look_height = LOOK_HEIGHT;
    let (mut yaw, mut pitch) = match (follow, &rig) {
        (Some(c), Some(r)) => (c.yaw - r.player.turn.to_degrees(), c.pitch),
        _ => (a.yaw, a.pitch),
    };
    let mut dist = a.distance.or(follow.map(|c| c.length)).unwrap_or(radius / (fov * 0.5).sin() * 1.05).max(1.0);
    let (pitch_range, length_range) = follow.map_or(((-10.0, 70.0), (150.0, 3000.0)), |c| (c.pitch_range, c.length_range));
    let mut center = match (follow, &rig) {
        (Some(_), Some(r)) => (r.player.at + Vec3::new(0.0, look_height, 0.0)) * Vec3::new(-1.0, 1.0, 1.0),
        _ => center,
    };
    let shadow_size = (!a.no_shadows).then_some(renderer.shadow_target.size);

    let hidden = vec![false; instances.len()];
    // The field's draw distances (`camera::DrawRanges`): a field object farther from the eye
    // than its class's range is left out. Field objects stay where they were placed, so each
    // draw's box is found once (render space, as the eye).
    let ranges = match a.draw_ranges.as_str() {
        "off" => None,
        which => {
            let bytes = install.read(&barrel_ryza::camera::FreeCamera::path(&a.field))?;
            Some(barrel_ryza::camera::DrawRanges::parse(&bytes, which == "nx")?)
        }
    };
    let boxes: std::collections::HashMap<(u32, u32), (Vec3, Vec3)> = match ranges {
        None => Default::default(),
        Some(_) => draws.iter().filter(|d| d.field.is_some() && !d.sky).filter_map(|d| {
            let b = d.bounds?;
            let m = instances.get(d.instance as usize)?.model;
            let corners = (0..8).map(|k| m.transform_point3(Vec3::new(if k & 1 == 0 { b.lo.x } else { b.hi.x }, if k & 2 == 0 { b.lo.y } else { b.hi.y }, if k & 4 == 0 { b.lo.z } else { b.hi.z })));
            let (lo, hi) = corners.fold((Vec3::MAX, Vec3::MIN), |(l, h), c| (l.min(c), h.max(c)));
            Some(((d.first_index, d.instance), (lo, hi)))
        }).collect(),
    };
    eprintln!("draw ranges: {ranges:?}");
    let eye_now = std::cell::Cell::new(Vec3::ZERO);
    let shown = |d: &Draw| {
        !a.hide.iter().any(|h| d.name.contains(h.as_str()))
            && match (ranges, boxes.get(&(d.first_index, d.instance))) {
                (Some(r), Some(&(lo, hi))) => r.shows(d.class, eye_now.get(), lo, hi),
                _ => true,
            }
    };
    // Lit as the field's models: the environment's light on their vertex colour (no sun yet).
    // Her draws take the toon ramp (flag 128, the row in bits 16-23).
    let material = |d: &Draw| {
        // The sky dome's clouds lit by the light on an upward-facing surface (flag 4096, look
        // slot `DOME_LIGHT_SLOT`): the game lights them (`PBSkyDome`, its scales not read).
        let base = if environment.is_some() {
            if d.sky {
                DOME_LIT
            } else if d.name == "plants" {
                FADED
            } else if d.normal_map.is_some() && !ramp_rows.contains_key(&(d.first_index, d.instance)) {
                NORMAL_MAPPED
            } else {
                0
            }
        } else {
            UNLIT
        };
        match ramp_rows.get(&(d.first_index, d.instance)) {
            Some(&(row, has_rim)) if ramp_pack.is_some() && environment.is_some() => {
                let r = rim.map_or(0, |r| if has_rim { rim_word(r.sharpness, r.strength) } else { rim_word(r.hair_sharpness, r.hair_strength) });
                (base | 128 | row << 16, r)
            }
            _ => (base, 0),
        }
    };
    let layers: Vec<&text::Layer> = Vec::new();
    let bytes = u64::from(width) * u64::from(height) * 4;
    let readback = match pump {
        Some(_) => None,
        None => Some(gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?),
    };
    let mut sync = ring::Frames::new(&gpu)?;
    let mut prev_vp = None;
    let mut last = std::time::Instant::now();
    let started = last;
    let (mut stale, mut frame_no) = (false, 0u32);
    // The camera's distance as drawn: pulled in by the ground behind her, easing back out.
    let (mut view_dist, mut frame_dt) = (None::<f32>, 0.0f32);
    loop {
        // The player's input, in a window: her lean against the camera, the camera's turn.
        if let Some(p) = pump.as_mut() {
            use winit::keyboard::KeyCode as K;
            if !p.pump() || p.take_pressed().contains(&K::Escape) || a.frames.is_some_and(|n| frame_no >= n) {
                break;
            }
            let now = std::time::Instant::now();
            let dt = (now - last).as_secs_f32().min(0.1);
            last = now;
            frame_dt = dt;
            let key = |k: K| if p.held(k) { 1.0 } else { 0.0 };
            let (left, right) = p.sticks();
            yaw -= (key(K::ArrowRight) - key(K::ArrowLeft) + right[0]) * CAMERA_TURN * dt;
            pitch = (pitch + (key(K::ArrowUp) - key(K::ArrowDown) - right[1]) * CAMERA_TURN * dt).clamp(pitch_range.0, pitch_range.1);
            dist = (dist * (1.0 + (key(K::Equal) - key(K::Minus)) * dt)).clamp(length_range.0, length_range.1);
            if let Some(r) = rig.as_mut() {
                let mut lean = [key(K::KeyD) - key(K::KeyA) + left[0] + a.lean[0], key(K::KeyW) - key(K::KeyS) + left[1] + a.lean[1]];
                let length = (lean[0] * lean[0] + lean[1] * lean[1]).sqrt();
                let full = if p.held(K::ShiftLeft) || p.held(K::ShiftRight) { 0.4 } else { 1.0 };
                if length > full {
                    lean = lean.map(|v| v / length * full);
                }
                // Against the camera, in render space (x mirrored), then game space.
                let y = yaw.to_radians();
                let (forward, side) = (Vec3::new(-y.sin(), 0.0, -y.cos()), Vec3::new(y.cos(), 0.0, -y.sin()));
                let way = forward * lean[1] + side * lean[0];
                r.player.step([-way.x, way.z], dt, r.ground.as_ref(), r.walls.as_ref());
                // Her pose: the motion playing, the last one mixed out over it.
                let local = |(m, t): (u32, f32)| barrel_engine::skin::local_pose(&r.skeleton, &r.anims.iter().find(|(n, _)| *n == m).unwrap_or(&r.anims[0]).1, t);
                let (now, from) = r.player.pose();
                let mut locals = match from {
                    Some((before, left)) => barrel_engine::skin::mix_pose(&local(now), &local(before), left),
                    None => local(now),
                };
                if let Some(f) = &r.face {
                    f.apply(&mut locals, r.shut.as_ref(), barrel_ryza::face::blink(started.elapsed().as_secs_f32()));
                }
                let joints = barrel_engine::skin::skin(&r.skeleton, &locals);
                prev_palette.copy_from_slice(&palette);
                let base = instances[r.instance as usize].palette as usize;
                for (slot, j) in palette[base..].iter_mut().zip(&joints) {
                    *slot = render_space(*j);
                }
                let i = &mut instances[r.instance as usize];
                i.prev_model = i.model;
                i.model = render_space(Mat4::from_scale_rotation_translation(Vec3::from(r.chara.size), glam::Quat::from_rotation_y(r.player.turn), r.player.at));
                center = (r.player.at + Vec3::new(0.0, look_height, 0.0)) * Vec3::new(-1.0, 1.0, 1.0);
                // The sky dome goes with her (centred under the look point, as it was placed).
                if let Some(i) = dome_instance.and_then(|k| instances.get_mut(k as usize)) {
                    i.prev_model = i.model;
                    i.model = render_space(Mat4::from_translation(Vec3::new(r.player.at.x, 0.0, r.player.at.z)));
                }
                if frame_no % 60 == 0 {
                    eprintln!("frame {frame_no} ({:.2}s): Ryza at {:?}, {:?}, motion {} at {:.2}s", started.elapsed().as_secs_f32(), r.player.at, r.player.gait, r.player.motion, r.player.clock);
                }
            }
            // A resized window, or a swapchain gone stale: remake it and the targets.
            if let Some(pr) = presenter.as_mut()
                && (p.take_resized() || stale)
            {
                let size = p.window().inner_size();
                if size.width == 0 || size.height == 0 {
                    continue;
                }
                pr.rebuild(&gpu, size.width, size.height)?;
                (width, height) = (pr.extent.width, pr.extent.height);
                renderer.rebuild_targets(&gpu, Some(&*pr), width, height)?;
                stale = false;
            }
        }
        sync.wait_slot(&gpu, renderer.pool)?;
        if pump.is_some() {
            scene.write_instances(&gpu, &instances)?;
            scene.write_palette(&gpu, &palette, &prev_palette)?;
        }
        // Following her: no nearer than the ground behind her allows (`camera::clear_distance`).
        let drawn_dist = match (follow, rig.as_ref().and_then(|r| r.ground.as_ref().map(|g| (r, g)))) {
            (Some(_), Some((r, g))) => {
                let (y, p) = (yaw.to_radians(), pitch.to_radians());
                let way = Vec3::new(-y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
                let clear = barrel_ryza::camera::clear_distance(g, r.walls.as_ref(), r.player.at + Vec3::new(0.0, look_height, 0.0), way, dist);
                let d = match view_dist {
                    Some(v) if clear > v => v + (clear - v) * (frame_dt * 4.0).min(1.0),
                    _ => clear,
                };
                view_dist = Some(d);
                d
            }
            _ => dist,
        };
        let mut view = look_for(center, yaw, pitch, drawn_dist, fov, (width, height), prev_vp, environment.as_ref(), exposure_range, shadow_size);
        eye_now.set(Vec3::from_slice(&view.look[4][..3]));
        if frame_no == 0
            && let Some(k) = view.exposure
        {
            eprintln!("exposure {k:.4}");
        }
        prev_vp = Some(view.vp);
        // The far shadow square the renderer has ready (its layer; 0 none yet).
        if let Some((want, mut info)) = view.shadow_far
            && let Some((vp, layer)) = renderer.far_shadow(Some(want))
        {
            for (k, col) in vp.to_cols_array_2d().into_iter().enumerate() {
                view.look[SHADOW_FAR_SLOT + k] = col;
            }
            info[1] = layer as f32;
            view.look[SHADOW_FAR_SLOT + 4] = info;
        }
        renderer.look_ring.write(&gpu, 0, unsafe { std::slice::from_raw_parts(view.look.as_ptr() as *const u8, LOOK_SIZE as usize) });
        // The image to draw into: the window's next, or the offscreen one.
        let image = match &presenter {
            Some(p) => match p.acquire(sync.acquired(&gpu))? {
                Some(i) => i as usize,
                None => {
                    stale = true;
                    continue;
                }
            },
            None => 0,
        };
        let frame = Frame {
            image,
            width,
            height,
            scene: &scene,
            instance_set: scene.instance_set(&gpu, 0),
            draws: &draws,
            instances: &instances,
            sets: &scene.sets,
            white,
            outline_tex,
            current_field: 0,
            hidden: &hidden,
            shown: &shown,
            material: &material,
            draw_vp: view.vp,
            view_planes: view_planes(&view.vp),
            shadow_vp: view.shadow_vp,
            alpha_cutoff: 0.3,
            // The set's bloom: `fLumiThreshold` as the bright pass's threshold (on the scene as
            // stored, half its range as Escha's) and `fBloomWeight` as the rate.
            glow: environment.as_ref().map_or((1.0, 0.0), |(_, l, _)| (l.lumi_threshold, l.bloom_weight)),
            // No saturation: the set's triple at 0x44 (`Light::saturation`, placed by its values
            // only) laid on as saturation worsens the gate's noon against the still (ten
            // regions' RGB error 145 without, 151 with, 187 before the tone curve), and as a
            // colour scale it does no better (144 at the divisor refitted).
            saturation: [0.0; 3],
            // A tone curve with the set's `fLumiMax` as its white point (fitted with the
            // exposure, `EXPOSURE_DIVISOR`: without it the sunlit grass clipped to 241, 254, 22
            // against the still's 182, 216, 69). How the game's tone chain uses `fLumiMax` is
            // not read.
            tone_white: environment.as_ref().map_or(0.0, |(_, l, _)| l.lumi_max),
            camera: Camera { fov, near: view.near, far: view.far, ..Camera::default() },
            freeze_now: false,
            frozen_weight: 0.0,
            overlay: [0.0; 4],
            layers: &layers,
            readback: readback.map(|(b, _)| b),
            sky_plane: view.sky_plane,
        };
        let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(renderer.pool).command_buffer_count(1))?[0] };
        unsafe { d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))? };
        let drawn = renderer.record(&gpu, cb, &frame);
        unsafe { d.end_command_buffer(cb)? };
        match (&presenter, readback) {
            (Some(p), _) => {
                let done = sync.submit(&gpu, cb, image, p.images.len())?;
                stale |= p.present(&gpu, image as u32, done)?;
            }
            // Offscreen: the same frame `--frames` times (for `--timings`), the last one written.
            (None, Some(_)) if frame_no + 1 < a.frames.unwrap_or(1) => {
                sync.submit_offscreen(&gpu, cb)?;
                sync.wait_submitted(&gpu)?;
            }
            (None, Some((_, rmem))) => {
                sync.submit_offscreen(&gpu, cb)?;
                sync.wait_submitted(&gpu)?;
                eprintln!("{} draws in the scene", drawn[0]);
                let mut pixels = vec![0u8; bytes as usize];
                unsafe {
                    let p = d.map_memory(rmem, 0, bytes, vk::MemoryMapFlags::empty())? as *const u8;
                    std::ptr::copy_nonoverlapping(p, pixels.as_mut_ptr(), pixels.len());
                    d.unmap_memory(rmem);
                }
                std::fs::write(&a.out, barrel_formats::png::encode_rgba(width, height, &pixels))?;
                eprintln!("wrote {}", a.out);
                break;
            }
            (None, None) => unreachable!("offscreen frames have a readback"),
        }
        if let Some(t) = renderer.timer.as_mut() {
            t.collect(&gpu)?;
        }
        gpu.frame.set(gpu.frame.get() + 1);
        frame_no += 1;
    }
    if let Some(t) = renderer.timer.as_mut()
        && presenter.is_none()
    {
        t.collect(&gpu)?;
    }

    unsafe {
        d.device_wait_idle()?;
        if let Some((b, m)) = readback {
            d.destroy_buffer(b, None);
            d.free_memory(m, None);
        }
    }
    sync.destroy(&gpu, renderer.pool)?;
    if let Some(p) = presenter {
        p.destroy(&gpu);
    }
    scene.destroy(&gpu);
    renderer.destroy(&gpu)?;
    unsafe {
        d.destroy_device(None);
        gpu.instance.destroy_instance(None);
    }
    Ok(())
}

/// How high above her feet the camera behind Ryza looks (a stand-in).
const LOOK_HEIGHT: f32 = 110.0;
/// How fast the camera turns, degrees a second.
const CAMERA_TURN: f32 = 120.0;

/// The field's environment: the diffuse cube map's light, the set's light and its sky fog.
type Environment = ([[f32; 3]; 9], Light, barrel_ryza::environment::Fog);

/// A frame's view: the look block and what goes with it.
struct View {
    look: [[f32; 4]; LOOK_SLOTS],
    vp: Mat4,
    shadow_vp: Mat4,
    /// The far shadow square wanted and its look slot (texel, layer to fill, fades).
    shadow_far: Option<(Mat4, [f32; 4])>,
    sky_plane: bool,
    near: f32,
    far: f32,
    exposure: Option<f32>,
}

/// The view from `dist` away from `center` (render space) at `yaw` and `pitch` degrees, and
/// its look block (`barrel_render`'s `Look`): the eye, the view-projection and last frame's
/// (`prev_vp`; this one's when there is none), and the environment's light, sky and the sun's
/// shadow (`shadow_size`: the shadow map's, none without shadows). With no environment, a
/// white ambient alone (the terrain shows its colours as they are).
#[allow(clippy::too_many_arguments)]
fn look_for(center: Vec3, yaw: f32, pitch: f32, dist: f32, fov: f32, (width, height): (u32, u32), prev_vp: Option<Mat4>, environment: Option<&Environment>, exposure_range: Option<(f32, f32)>, shadow_size: Option<u32>) -> View {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    let eye = center + Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist;
    let view = Mat4::look_at_rh(eye, center, Vec3::Y);
    let (near, far) = (dist * 0.01, (dist * 20.0).max(80_000.0));
    let mut proj = Mat4::perspective_rh(fov, width as f32 / height as f32, near, far);
    proj.y_axis.y *= -1.0;
    proj.x_axis.x *= -1.0;
    let vp = proj * view;
    let mut look = [[0f32; 4]; LOOK_SLOTS];
    let mut shadow_vp = vp;
    let mut shadow_far = None;
    for (k, (now, before)) in vp.to_cols_array_2d().into_iter().zip(prev_vp.unwrap_or(vp).to_cols_array_2d()).enumerate() {
        look[40 + k] = before;
        look[44 + k] = now;
    }
    let mut sky_plane = false;
    let mut exposure_k = None;
    look[4] = [eye.x, eye.y, eye.z, 1.0];
    // The billboards' axes (model.vert; the field's plants): upright cards turning about the
    // vertical, the camera's right flattened (render space is mirrored, so the view's -x).
    {
        let r = Vec3::new(view.row(0).x, 0.0, view.row(0).z).normalize_or(Vec3::X);
        let back = r.cross(Vec3::Y);
        look[37] = [-r.x, -r.y, -r.z, 0.0];
        look[38] = [0.0, 1.0, 0.0, 0.0];
        look[39] = [back.x, back.y, back.z, 0.0];
    }
    look[FADE_SLOT] = [PLANT_FADE[0], PLANT_FADE[1], 0.0, 0.0];
    look[26] = [1.0, 1.0, 1.0, 1.0];
    // The environment's light instead, when the field has one: the cube map's light times the
    // ambient colour (model.vert's `sh`), the sun and the second light as the field's two
    // directional lights, all scaled by one exposure (`exposure`).
    if let Some((c, light, sky_fog)) = environment {
        // Capped by the override's exposure range's top (1.2): evening and night would
        // otherwise be lifted to noon's brightness. Its low end is
        // not applied: the morning set's ambient is ten times noon's (21.6, 30, 27.2 against
        // 3, 3, 4) and wants an exposure of 0.30. The game's adaptation itself is not read.
        let k = exposure(c, light);
        let k = exposure_range.map_or(k, |(_, hi)| k.min(hi));
        // The light on an upward-facing surface, for the sky dome's clouds: the light map
        // upward times the ambient colour, and the two lights by their elevation.
        {
            let up = barrel_ryza::environment::basis([0.0, 1.0, 0.0]);
            let rgb: [f32; 3] = std::array::from_fn(|ch| {
                let sky = (0..9).map(|i| c[i][ch] * up[i]).sum::<f32>() * light.ambient[ch];
                let sun = light.parallel.iter().map(|p| p.color[ch] * p.elevation.to_radians().sin().max(0.0)).sum::<f32>();
                (sky + sun) * k
            });
            look[DOME_LIGHT_SLOT] = [rgb[0], rgb[1], rgb[2], 1.0];
        }
        look[66] = [1.0, 0.0, 0.0, 0.0];
        for (i, [r, g, b]) in c.iter().copied().enumerate() {
            look[67 + i] = [r * light.ambient[0] * k, g * light.ambient[1] * k, b * light.ambient[2] * k, 0.0];
        }
        for (slot, p) in [22usize, 24].into_iter().zip(light.parallel) {
            let d = from_light(p.elevation, p.azimuth);
            look[slot] = [d.x, d.y, d.z, 0.0];
            look[slot + 1] = [p.color[0] * k, p.color[1] * k, p.color[2] * k, 1.0];
        }
        // The sky plane (docs/shading.md, "Ryza DX: the sky"): Scat as measured in the game,
        // from `skyFog_` and the sun's colour times its power (scaled by the frame's exposure).
        // Stand-ins, not read: the path length's terms (an atmosphere `end` thick over a ground
        // ten times as wide, so the zenith is one fog span and the horizon about 4.6) and no
        // glow (`docs/ryza/open.md`).
        {
            let f = *sky_fog;
            let sun = light.parallel[0].color.map(|v| v * f.power * k * SKY_SCALE);
            let ext: [f32; 3] = std::array::from_fn(|i| f.rayleigh[i] + f.mie[i]);
            let part = |b: [f32; 3], i: usize| if ext[i] > 0.0 { b[i] / ext[i] } else { 0.0 };
            let ray_k = 3.0 / (16.0 * std::f32::consts::PI);
            let mie_k = (1.0 - f.g) * (1.0 - f.g) / (4.0 * std::f32::consts::PI);
            let s0 = SKY_SLOT;
            look[s0] = [1.0, 0.0, 0.0, 0.0];
            look[s0 + 1] = [-ext[0] * std::f32::consts::LOG2_E, -ext[1] * std::f32::consts::LOG2_E, -ext[2] * std::f32::consts::LOG2_E, 1.0];
            look[s0 + 2] = [0.0, 0.0, 0.0, 1.0];
            look[s0 + 3] = [ray_k * sun[0] * part(f.rayleigh, 0), ray_k * sun[1] * part(f.rayleigh, 1), ray_k * sun[2] * part(f.rayleigh, 2), -2.0 * f.g];
            look[s0 + 4] = [mie_k * sun[0] * part(f.mie, 0), mie_k * sun[1] * part(f.mie, 1), mie_k * sun[2] * part(f.mie, 2), 1.0 + f.g * f.g];
            let thick = (f.end - f.start).max(1.0);
            let ground = 10.0 * thick;
            let top = ground + thick;
            look[s0 + 5] = [ground, ground * ground, top * top - ground * ground, 0.0];
            look[s0 + 6] = [1.0 / thick, 0.0, 0.0, 0.0];
            let sun_way = from_light(light.parallel[0].elevation, light.parallel[0].azimuth);
            look[s0 + 7] = [sun_way.x, sun_way.y, sun_way.z, 0.0];
            sky_plane = true;
        }
        // The sun's shadow map: one square over the camera's target, wide enough for the
        // view (the game's cascades, `shadowMap_`, are not read), unless --no-shadows.
        if let Some(size) = shadow_size {
            let sun = from_light(light.parallel[0].elevation, light.parallel[0].azimuth);
            shadow_vp = shadow::sun_view_proj(center, sun, (dist * 1.5).clamp(SHADOW_HALF, 6000.0), 8000.0);
            for (k, col) in shadow_vp.to_cols_array_2d().into_iter().enumerate() {
                look[27 + k] = col;
            }
            look[31] = [1.0 / size as f32, 1.0, 0.0, 0.0];
            // Past it, the far layer: a square out to where the game's shadows end
            // (`shadowMap_ShadowEraseFarEnd`), fading to lit from `ShadowEraseFarStart` (the
            // default override's 20000 and 30000; its three near cascades, 1200 / 6000 /
            // 13000, are the near square here).
            let (fade_start, fade_end) = (20000.0, 30000.0);
            // Its centre steps on a 2000-unit grid: the layer is drawn again only when it
            // moves (`Renderer::far_shadow`; a whole-field draw, about 10 ms at 1080p), so
            // far swaying trees' shadows hold still between steps.
            let step = 2000.0;
            let snapped = Vec3::new((center.x / step).round() * step, (center.y / step).round() * step, (center.z / step).round() * step);
            // The renderer says which far square the receivers read (`Renderer::far_shadow`).
            shadow_far = Some((shadow::sun_view_proj(snapped, sun, fade_end, 20000.0), [1.0 / size as f32, 0.0, fade_start, fade_end]));
        }
        exposure_k = Some(k);
    }
    View { look, vp, shadow_vp, shadow_far, sky_plane, near, far, exposure: exposure_k }
}
