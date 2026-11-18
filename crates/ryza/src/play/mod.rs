//! `barrel-ryza`: Atelier Ryza DX on Barrel. For now it draws one field offscreen:
//!
//! ```text
//!     barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--yaw DEG] [--pitch DEG]
//!                 [--look-at X,Y,Z] [--distance D] [--time morning|noon|evening|night]
//!                 [--weather sunny|cloudy|foggy] [--done EVENT[,EVENT...]] [--hide NAME]...
//!                 [--ryza X,Y,Z [--turn DEG] [--motion N] [--motion-time S]] [--no-shadows]
//!                 [--window [--fullscreen]] [--preset NAME] [--fov DEG] [--draw-ranges auto|pc|nx|off]
//! ```
//!
//! FIELD is a field's folder name in the archives (`s01_i_attic`). The camera frames the field's
//! ground (its terrain's square; the backdrop's mountains far outside it are drawn, not framed),
//! or looks at X,Y,Z (field units, as the placement list gives them) from D away. The light is
//! the field environment's set for the time and weather (noon, sunny by default). `--hide`
//! leaves out the objects whose placement names contain NAME (to find which object is which).
//! `--done` names the story events that have played (`MM00_010`): the people and objects the
//! field's tables hang on the story follow them (none played by default). `--save FILE`
//! takes them from a save of the game's (a `data.dat`) instead.
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
use barrel_render::clouds::{CloudFrame, Density as CloudDensity, Shape as CloudShape};
use barrel_render::renderer::{Camera, Frame, Renderer};
use barrel_render::settings::RenderSettings;
use barrel_render::*;
use barrel_ryza::environment::{Light, Time, Weather};
use barrel_ryza::install::Install;
use glam::{Mat4, Vec3};

mod actors;
mod constants;

/// The renderer's material bit for a draw shown with its texture alone, unlit (`model.frag`).
/// The alpha test's reference (`vATest`) on every field and character model's draw of the
/// game's frame, and on its plants' (docs/shading.md): 50.1 and 63.1 of 255.
const MODEL_ALPHA_REF: f32 = 50.1 / 255.0;
const PLANT_ALPHA_REF: f32 = 63.1 / 255.0;

const UNLIT: u32 = 32;
/// `model.vert`'s flag for a sky dome lit by the look's `DOME_LIGHT_SLOT`.
const DOME_LIT: u32 = 4096;
/// `model.frag`'s flag for a normal map in the mask slot (a field model's `sNMap`).
const NORMAL_MAPPED: u32 = 8192;
/// `model.vert`'s flag for billboards shrinking away over the look's `FADE_SLOT` range (the
/// plants, as `ObjectGrassPBS` does).
const FADED: u32 = 16384;
/// `model.frag`'s flag for a field model that reflects the environment's specular cube map.
const SPECULAR_AMBIENT: u32 = 1 << 24;
/// `model.frag`'s flag for a field model lit by its emission map (`PBB-EmN`).
const EMISSIVE: u32 = 32768;
// STAND-IN: The emission strengths by time of day are chosen by the stills; the real thing is the game's run-time emScl.
/// The emission maps' strength by time of day (morning, noon, evening, night), in the light's
/// units: a stand-in, the game's `emScl` is set at run time (not read): its windows and lamps
/// unlit by day, lit at evening and night.
const EMISSION_BY_TIME: [f32; 4] = [0.0, 0.0, 2.0, 3.0];

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
    /// The story's progress (`--done`, or the save's with `--save`).
    story: barrel_ryza::story::Story,
    /// A save file to take the story's progress from (`--save`).
    save: Option<String>,
    /// `--no-skyline`: objects behind the ground are drawn too.
    no_skyline: bool,
    ryza: Option<(Vec3, f32, String, f32)>,
    no_shadows: bool,
    /// The camera's vertical field of view, degrees (`--fov`).
    fov: Option<f32>,
    /// The field's draw distances (`--draw-ranges`): as the PC game uses them (`auto`, the
    /// default: none at the top object-distance setting), always the PC's (`pc`) or the
    /// Switch's (`nx`), or none (`off`).
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
    /// `--telop N`: a telop (`u65`'s number) shown over the frame, as an event shows it.
    telop: Option<u32>,
    /// `--event NAME --at T`: the event's stage T seconds in (its telop shown as it would be).
    event: Option<(String, f32)>,
    /// `--title`: the title screen's 2D layer over a blank frame (the field is not drawn), and
    /// `--title-at S` (or `BARREL_TITLE_AT`): the seconds into the title the first frame shows.
    title: bool,
    title_at: f32,
    /// What the player does on the title when no one is at the keys: `--title-press F` the
    /// title frame of the button at the prompt, `--title-keys F:down:N,F:up:N,...` a direction
    /// going down on frame F for N frames, `--title-saves` as if a save existed.
    title_input: barrel_ryza::title_menu::Input,
    /// `--boot`: the boot screens (system-data notice, the two logos, the autosave window) before
    /// the title, and `--boot-at S` (or `BARREL_BOOT_AT`): the seconds into the boot the first
    /// frame shows (the title follows `barrel_ryza::boot::TITLE_START` seconds in).
    boot: bool,
    boot_at: f32,
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

fn args(args: Vec<String>) -> Result<Args> {
    let mut it = args.into_iter();
    let usage = "usage: barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--telop N] [--event NAME --at T] [--yaw DEG] [--pitch DEG] [--look-at X,Y,Z] [--distance D] [--time T] [--weather W]";
    let (install, field, out) = (it.next().ok_or(usage)?, it.next().ok_or(usage)?, it.next().ok_or(usage)?);
    let mut a = Args { install, field, out, size: (1280, 720), yaw: 0.0, pitch: 30.0, look_at: None, distance: None, time: Time::Noon, weather: Weather::Sunny, hide: Vec::new(), story: Default::default(), save: None, no_skyline: false, ryza: None, no_shadows: false, fov: None, draw_ranges: "auto".into(), window: false, preset: None, fullscreen: false, timings: false, frames: None, lean: [0.0; 2], telop: None, event: None, title: false, title_input: Default::default(), title_at: std::env::var("BARREL_TITLE_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0), boot: false, boot_at: std::env::var("BARREL_BOOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0) };
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
            "--title" => a.title = true,
            "--title-at" => a.title_at = value()?.parse()?,
            "--title-press" => a.title_input.press = Some(value()?.parse()?),
            "--title-saves" => a.title_input.saves = true,
            "--title-keys" => {
                for key in value()?.split(',') {
                    let [from, way, held] = key.split(':').collect::<Vec<_>>()[..] else { return Err("--title-keys F:down:N,...".into()) };
                    let (from, way) = (from.parse::<i32>()?, if way == "up" { -1 } else { 1 });
                    a.title_input.holds.push(barrel_ryza::title_menu::Hold { from, until: Some(from + held.parse::<i32>()?), way });
                }
            }
            "--boot" => (a.boot, a.title) = (true, true),
            "--boot-at" => a.boot_at = value()?.parse()?,
            "--telop" => a.telop = Some(value()?.parse()?),
            "--event" => a.event = Some((value()?, 0.0)),
            "--at" => a.event.as_mut().ok_or("--at after --event")?.1 = value()?.parse()?,
            "--lean" => {
                let v = value()?.split(',').map(str::parse).collect::<std::result::Result<Vec<f32>, _>>()?;
                let [x, y] = v[..] else { return Err("--lean X,Y".into()) };
                a.lean = [x, y];
            }
            "--done" => a.story = barrel_ryza::story::Story::after(&value()?),
            "--save" => a.save = Some(value()?),
            "--no-skyline" => a.no_skyline = true,
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
    // The title draws its own scene whatever field was named: the title's field.
    if a.title {
        a.field = barrel_ryza::title::FIELD.into();
    }
    Ok(a)
}

/// A layer showing telop `n` (`u65`'s number) at opacity `alpha` on a `screen`-sized frame.
fn telop_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer, n: u32, alpha: f32, screen: (u32, u32)) -> Result<text::Layer> {
    use barrel_ryza::telop;
    let node = match telop::Node::of(n) {
        Some(telop::Node::Caption(k)) => format!("telop{k:02}"),
        // STAND-IN: Name-card telops are not drawn; the real thing is drawing them.
        Some(telop::Node::Name(_)) => return Err(format!("--telop {n}: a name card (not drawn yet)").into()),
        None => return Err(format!("--telop {n}: no such telop").into()),
    };
    let layout = install.read("\\saves\\ui_en\\a21_telop\\uil_a21_telop01.xml")?;
    let (at, image) = telop::place(&layout, &node)?.ok_or(format!("--telop {n}: no {node} in the layout"))?;
    let strips = telop::strips(&install.read("\\saves\\ui_en\\etc\\uis_a21_telop.xml")?)?;
    let strip = strips.get(image as usize).ok_or(format!("--telop {n}: no strip {image}"))?;
    let hd = screen.1 > 1080;
    let file = strip.texture.trim_start_matches("ui/");
    let path = if hd { format!("\\data\\x64\\res_en\\ui\\neo\\neo_{file}") } else { format!("\\data\\x64\\res_en\\ui\\{file}") };
    let bytes = install.read(&path)?;
    let g = barrel_formats::g1t::G1t::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let t = g.textures.first().ok_or(format!("{path}: no texture"))?;
    let (rect, uv) = telop::quad(strip, at, screen, (t.width, t.height), if hd { 1.0 } else { 0.5 });
    eprintln!("telop {n}: {node}, strip {image} of {path} at {rect:?}");
    let mut layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &[t])?;
    layer.set(gpu, &[text::Quad { turn: false, spin: None, rect, uv, color: [1.0, 1.0, 1.0, alpha], texture: 0 }])?;
    Ok(layer)
}

/// The title screen's layer (empty until each frame's quads are set) with its layout.
fn title_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer, boot: bool) -> Result<(text::Layer, barrel_ryza::title::Title, barrel_ryza::title_menu::Menu, Option<barrel_ryza::boot::Boot>)> {
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = barrel_ryza::font::glyphs(&exe).ok_or("the executable's font table")?;
    let (font_bytes, ui_bytes) = (install.read("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t")?, install.read("\\data\\x64\\res_en\\ui\\a21_title.g1t")?);
    let font = barrel_formats::g1t::G1t::parse(&font_bytes).map_err(|e| format!("mainfont01: {e}"))?;
    let ui = barrel_formats::g1t::G1t::parse(&ui_bytes).map_err(|e| format!("a21_title: {e}"))?;
    let (font, ui) = (font.textures.first().ok_or("mainfont01: no texture")?, ui.textures.first().ok_or("a21_title: no texture")?);
    let (layout, styles, anims) = (install.read("\\saves\\ui_en\\a21_title\\uil_a21_title.xml")?, install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_title.xml")?, install.read("\\saves\\ui_en\\uia_common.xml")?);
    // The menu's cursor has its own atlas, the layer's last texture.
    let cursor_bytes = install.read("\\data\\x64\\res_en\\ui\\a21_balloon_frame.g1t")?;
    let cursor = barrel_formats::g1t::G1t::parse(&cursor_bytes).map_err(|e| format!("a21_balloon_frame: {e}"))?;
    let cursor = cursor.textures.first().ok_or("a21_balloon_frame: no texture")?;
    let menu = |texture: usize| {
        barrel_ryza::title_menu::Menu::read(
            &layout,
            &styles,
            &anims,
            barrel_ryza::title_menu::Cursor {
                styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_balloon_frame.xml")?,
                anims: &install.read("\\saves\\ui_cmn\\a21_balloon_frame\\uia_a21_balloon_frame.xml")?,
                atlas: (cursor.width * 2, cursor.height * 2),
                texture,
            },
            (ui.width * 2, ui.height * 2),
        )
        .map_err(Box::<dyn std::error::Error>::from)
    };
    let title = barrel_ryza::title::Title::read(
        &layout,
        &styles,
        &anims,
        glyphs,
        // The styles' pixels are the 2160-line atlas's; the 1080-line one drawn is half its size.
        (ui.width * 2, ui.height * 2),
        (font.width, font.height),
    )?;
    // Textures in the order of `text::FONT`, `text::UI`, then the boot screens' (`boot::TEXTURES`).
    // Without the boot the Gust logo comes third: the fade's black is drawn from a point of it
    // (the game has it bound then too).
    if !boot {
        let logo_bytes = install.read("\\data\\x64\\res_en\\ipu\\logo_gust.g1t")?;
        let logo = barrel_formats::g1t::G1t::parse(&logo_bytes).map_err(|e| format!("logo_gust: {e}"))?;
        let logo = logo.textures.first().ok_or("logo_gust: no texture")?;
        return Ok((text::Layer::new(gpu, renderer.pool, renderer.post_pass, &[font, ui, logo, cursor])?, title, menu(3)?, None));
    }
    // (file, texture number) of each boot texture, in the order of `boot::TEXTURES` after the title's two.
    let wanted = [
        ("\\data\\x64\\res_en\\ui\\a21_saveload.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_saveload.g1t", 2),
        ("\\data\\x64\\res_en\\ipu\\logo_kt.g1t", 0),
        ("\\data\\x64\\res_en\\ipu\\logo_gust.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_memorymap.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_saveload.g1t", 0),
    ];
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let size = |t: &&barrel_formats::g1t::Texture| (t.width, t.height);
    let boot = barrel_ryza::boot::Boot::read(barrel_ryza::boot::Sources {
        progress: &install.read("\\saves\\ui_en\\a21_saveload\\uil_a21_saveload_progress.xml")?,
        saveload_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_saveload.xml")?,
        saveload_anims: &install.read("\\saves\\ui_en\\a21_saveload\\uia_a21_saveload.xml")?,
        common_anims: &install.read("\\saves\\ui_en\\uia_common.xml")?,
        notification: &install.read("\\saves\\ui_cmn\\a21_saveload\\uil_a21_notification_display.xml")?,
        window_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_memorymap.xml")?,
        spinner_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_saveload.xml")?,
        strings: &install.read("\\saves\\text_en\\str_ui.xml")?,
        glyphs: barrel_ryza::font::glyphs(&exe).ok_or("the executable's font table")?,
        sizes: [size(&tex[0]), size(&tex[1]), size(&tex[4]), size(&tex[5]), (font.width, font.height)],
    })?;
    let all = [font, ui, tex[0], tex[1], tex[2], tex[3], tex[4], tex[5], cursor];
    Ok((text::Layer::new(gpu, renderer.pool, renderer.post_pass, &all)?, title, menu(barrel_ryza::boot::TEXTURES)?, Some(boot)))
}

/// The way a directional light travels, in render space: from `elevation` degrees above the
/// horizon and `azimuth` degrees around y from +z toward +x (game space: the lighting pass's
/// `litDir` read from the running game matches both lights, docs/shading.md), x mirrored.
/// The townsfolk of `field` there at `time` in `weather` at this point of the `story`, standing
/// or walking their paths.
fn standing_npcs(install: &barrel_ryza::install::Install, field: &str, time: Time, weather: Weather, story: &barrel_ryza::story::Story) -> Vec<barrel_ryza::npc::Npc> {
    let read = |name: &str| install.read(&format!("\\saves\\fieldmap\\npc\\{name}.xml"));
    let tables = (|| Ok::<_, Box<dyn std::error::Error>>((read("npcbasedata")?, read("npcpeople")?, read("npcmarker")?, read("npccondition")?, read("npcevrange")?)))();
    let Ok((b, p, m, c, e)) = tables else { return Vec::new() };
    match barrel_ryza::npc::read(&b, &p, &m, &c, &e, &field.to_ascii_uppercase()) {
        Ok(all) => all.into_iter().filter(|n| n.present(time, weather, story)).collect(),
        Err(e) => {
            eprintln!("townsfolk of {field}: {e}");
            Vec::new()
        }
    }
}

/// The gimmicks with a model that `field` has out at `time` at this point of the `story`.
fn standing_gimmicks(install: &barrel_ryza::install::Install, field: &str, time: Time, story: &barrel_ryza::story::Story) -> Vec<barrel_ryza::gimmick::Gimmick> {
    let name = format!("\\saves\\fieldmap\\gimmick\\gimmick_fieldmap_{}.xml", field.to_ascii_lowercase());
    match install.read(&name).map_err(|e| e.to_string()).and_then(|b| barrel_ryza::gimmick::read(&b)) {
        Ok(all) => all.into_iter().filter(|g| g.present(time, story)).collect(),
        Err(e) => {
            eprintln!("gimmicks of {field}: {e}");
            Vec::new()
        }
    }
}

fn from_light(elevation: f32, azimuth: f32) -> Vec3 {
    let (e, a) = (elevation.to_radians(), azimuth.to_radians());
    let toward = Vec3::new(e.cos() * a.sin(), e.sin(), e.cos() * a.cos());
    -toward * Vec3::new(-1.0, 1.0, 1.0)
}

/// The glow's rate over the set's `fBloomWeight`: the glow is stored at half its range, as the
/// scene is.
const GLOW_RANGE: f32 = 2.0;

// STAND-IN: The cloud map's threshold is the game's first frame at Rasenboden's noon; the real thing is the set's density moved by its height noise (`CloudHeightNoiseChangeSpeed`, `CloudHeightNoiseScale`), not found.
const CLOUD_THRESHOLD: f32 = 0.6413691;
/// The cloud map's sharpness follows its threshold as `3 / (1 - threshold)` (read from the
/// seventeen frames of the game's capture at Rasenboden's noon, to six figures: 0.6413 gives
/// 8.3639, 0.6560 gives 8.7215).
fn cloud_sharpness(threshold: f32) -> f32 {
    3.0 / (1.0 - threshold)
}

/// The sky plane's ground radius and atmosphere height (the game's `skyPos`, as its sky
/// plane's constants hold them: 6,378 km and 10 km in metres).
const GROUND_RADIUS: f32 = 6_378_000.0;
const SKY_HEIGHT: f32 = 10_000.0;

/// The most plants a field draws (Rasenboden: about 36,000 at the kinds' densities).
const PLANT_LIMIT: usize = 200_000;
/// The plants' normal in the card's frame (turned by the billboard axes): up, so a plant takes
/// the light of the ground it stands on rather than of a wall facing the camera.
const PLANT_NORMAL: [f32; 3] = [0.0, 1.0, 0.0];

// STAND-IN: The plant cells' reach past the view's sides is a measured bound; the real thing is the game's own cull test.
/// A standing person's sphere for the skyline test: its centre's height above the feet and
/// its radius (a person is some 170 units tall; the arms and what they carry reach wider).
const PERSON_SPHERE: (f32, f32) = (90.0, 120.0);

/// How far outside a side of the view a plant cell's centre may be, across the view, and
/// still be drawn: on the title the game draws cells out to 656 and hides them from 695.
const PLANT_EDGE: f32 = 675.0;

/// The field's plants as camera-facing cards on its heightmap (`barrel_ryza::grass`), one draw
/// a grid cell as the game's, the atlas being texture `atlas`.
/// Returns each draw's first index with its cell's box in render space: the cell's square from
/// the lowest foot to the highest top of its plants.
fn add_plants(scene: &mut Scene, g: &barrel_ryza::grass::Grass, main: &Elixir, area: &str, atlas: usize, normals: Option<usize>, density: f32) -> std::collections::HashMap<u32, (Vec3, Vec3)> {
    let mut boxes = std::collections::HashMap::new();
    let Some(ground) = barrel_engine::field::named(main, &format!("{}.khm", area.to_ascii_uppercase()))
        .or_else(|| main.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".khm")).map(|e| main.data(e)))
        .and_then(|b| barrel_formats::khm::parse(b).ok())
        .map(barrel_engine::walk::Ground::of_heightmap)
    else {
        return boxes;
    };
    let [cx, cz] = g.cell();
    let plants = g.plants(PLANT_LIMIT, density, |x, z| ground.height(x, z, f32::MAX / 2.0, 0.0));
    let mut blocks: std::collections::BTreeMap<(u32, u32), Vec<&barrel_ryza::grass::Plant>> = Default::default();
    for p in &plants {
        blocks.entry((p.cell[0], p.cell[1])).or_default().push(p);
    }
    let instance = scene.instance(Mat4::IDENTITY);
    for (&(i, j), list) in &blocks {
        let first_index = scene.indices.len() as u32;
        let (foot, top) = list.iter().fold((f32::MAX, f32::MIN), |(f, t), p| (f.min(p.at[1]), t.max(p.at[1] + p.size[1])));
        // (render space: game x mirrored)
        boxes.insert(first_index, (Vec3::new(-((i + 1) as f32) * cx, foot, j as f32 * cz), Vec3::new(-(i as f32) * cx, top, (j + 1) as f32 * cz)));
        for p in list {
            let [u, v, cw, ch] = p.card;
            let [w, h] = p.size;
            // Vertices are built in render space: game x mirrored (`render_space`).
            let centre = [-p.at[0], p.at[1] + h * 0.5, p.at[2]];
            let base = scene.vertices.len() as u32;
            for (dx, dy, uv) in [(-0.5, -0.5, [u, v + ch]), (0.5, -0.5, [u + cw, v + ch]), (0.5, 0.5, [u + cw, v]), (-0.5, 0.5, [u, v])] {
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
            mask: normals,
            normal_map: normals,
            emission: None,
            specular: None,
            eye: false,
            sky: false,
            outline: None,
            field: Some(0),
            casts: false,
            name: "plants".into(),
            version: None,
            lod: 0,
            light: 0,
            tint: 0,
            bounds: None,
            class: 0,
            object: 0,
            part: None,
            bones: Vec::new(),
            shader: String::new(),
        });
    }
    eprintln!("plants: {} in {} draws", plants.len(), blocks.len());
    boxes
}

/// The most ticks the title's event is run ahead for its people's placing (ten seconds: it
/// places them within its first second).
const PLACING_TICKS: u32 = 600;

/// Plays Ryza with `raw`, the command line after the program's name. `chosen` is the install the
/// `barrel` program found and set up (`locate::start`); without it this finds the game itself:
/// `--game DIR`, the command line's INSTALL, `BARREL_GAME_DIR`, the config's, Steam's, else the
/// "Locate game files" screen; every game file is read through the layers over it
/// (`barrel_files`).
pub fn run(mut raw: Vec<String>, chosen: Option<locate::Resolved>) -> Result<()> {
    let game = match chosen {
        Some(game) => game,
        None => {
            let given = locate::take_option(&mut raw, "--game").map(std::path::PathBuf::from);
            let launched = locate::take_game_program(&mut raw);
            let named = raw.first().filter(|a| !a.starts_with("--")).map(std::path::PathBuf::from).or(launched);
            let Some(game) = locate::resolve(barrel_files::find::Title::Ryza, given, named)? else { return Ok(()) };
            game
        }
    };
    let mut kept_pump = game.pump;
    // No scene on the command line: Ryza walking Rasenboden from her garden gate, in a window.
    if raw.first().is_none_or(|a| a.starts_with("--")) {
        let start = [&*game.root.to_string_lossy(), "s06_t_rasen", "-", "--ryza", "32537,0,30300", "--time", "noon", "--window"].map(String::from);
        raw.splice(0..0, start);
    }
    let mut a = args(raw)?;
    // Graphics as Escha's: --preset, else Barrel's graphics config (in a window, or offscreen
    // when `BARREL_CONFIG` names one), else a launcher's default (`BARREL_DEFAULT_PRESET`,
    // `BARREL_DEFAULT_FULLSCREEN=1` on a handheld), else the desktop's.
    let config = (a.window || std::env::var_os("BARREL_CONFIG").is_some()).then(|| barrel_files::config::graphics_path().and_then(|p| std::fs::read_to_string(p).ok())).flatten();
    let graphics = a
        .preset
        .or(config.as_deref().map(RenderSettings::from_config))
        .or(std::env::var("BARREL_DEFAULT_PRESET").ok().and_then(|n| RenderSettings::preset(&n)))
        .unwrap_or(RenderSettings::DESKTOP)
        .resolved();
    // The game's scene is HDR (`trueHDR_`): drawn into a float target, so the tone curve
    // (`Frame::tone_white`) sees past the half range's 2.0.
    let graphics = RenderSettings { hdr_scene: true, ..graphics };
    let install = Install::open(Path::new(&a.install))?;
    if let Some(save) = &a.save {
        let state = barrel_ryza::save::decode(&barrel_files::read(Path::new(save))?).map_err(|e| format!("{save}: {e}"))?;
        let exe = barrel_files::read(Path::new(&a.install).join("Atelier_Ryza_DX.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
        a.story = barrel_ryza::save::story(&state, &pe).map_err(|e| format!("{save}: {e}"))?;
        eprintln!("{save}: {} story events played", barrel_ryza::save::events_done(&state).map_or(0, |e| e.len()));
    }
    // `--event NAME --at T`: the event's stage T seconds in; with FIELD `-`, its field (the
    // field map it last changed to, by the executable's `FIELDMAP_*` names).
    let event_stage = match &a.event {
        Some((name, at)) => {
            let path = barrel_ryza::event::path(name).ok_or(format!("--event {name}: not an event name"))?;
            let event = barrel_formats::ebd::Ebd::parse(&install.read(&path)?).map_err(|e| format!("{path}: {e}"))?;
            let stage = barrel_ryza::event::stage_at(barrel_ryza::event::schedule(&event)?, *at);
            eprintln!("event {name} at {at} s: field {:?}, telop {:?}", stage.field, barrel_ryza::event::telop(&stage));
            Some(stage)
        }
        None => None,
    };
    // The event's camera, unless --look-at gives one: its eye and target as this view's
    // target, turn, pitch and distance (render space mirrors x), and its field of view.
    if let (Some(stage), None) = (&event_stage, a.look_at) {
        let snap = stage.snapshot();
        if let (Some(eye), Some(target)) = (snap.eye, snap.target) {
            let d = Vec3::new(-(eye[0] - target[0]), eye[1] - target[1], eye[2] - target[2]);
            let dist = d.length().max(1.0);
            a.look_at = Some(Vec3::from(target));
            a.distance = Some(dist);
            a.pitch = (d.y / dist).asin().to_degrees();
            a.yaw = d.x.atan2(d.z).to_degrees();
            a.fov = a.fov.or(snap.fov);
            eprintln!("event camera: eye {eye:?}, target {target:?}, fov {:?}", snap.fov);
        }
    }
    // Ryza (character 0) where the event has put her, unless --ryza places her: the stage's
    // facing (radians about y, 0 toward +z, game space) as her turn.
    if let (Some(stage), None) = (&event_stage, &a.ryza) {
        if let Some(&(p, facing)) = stage.snapshot().characters.get(&0) {
            let turn = facing.to_degrees();
            a.ryza = Some((Vec3::from(p), turn, "00000".into(), 0.0));
            eprintln!("event: Ryza at {p:?}, turn {turn:.1}°");
        }
    }
    if a.field == "-" {
        let map = event_stage.as_ref().and_then(|s| s.field).ok_or("FIELD -: needs an --event that sets a field")?;
        let exe = barrel_files::read(Path::new(&a.install).join("Atelier_Ryza_DX.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
        a.field = barrel_ryza::maps::field_maps(&pe).get(map as usize).cloned().ok_or(format!("field map {map}: not named"))?;
        eprintln!("field map {map}: {}", a.field);
    }
    let files = install.field(&a.field)?;
    let (main, objects) = (Elixir::parse(&files.main)?, Elixir::parse(&files.objects)?);
    let mut scene = Scene::new();
    scene.field = Some(0);
    // STAND-IN: Every placed model casts the sun's shadow and nothing sways; the real thing is the game's own caster choice and sway.
    // Ryza's placements carry no tags (Escha marks its casters `xDS_`): every placed model
    // casts the sun's shadow; nothing sways yet (`docs/ryza/open.md`).
    // Each keeps its placement's draw class (near, middle, far, landmark), for the field's draw
    // distances.
    let place = |_: &barrel_formats::opl2::PlacedType, i: &barrel_formats::opl2::Instance| barrel_engine::field::Placement { casts: true, class: (i.word & 3) as u8, lods: true, moves: true, ..Default::default() };
    let barrel_engine::field::Loaded { textures: pack, moving, .. } = barrel_engine::field::load(&files.area, &main, &objects, &mut scene, 0, &place)?;
    let pack = G1t::parse(&pack)?;
    // The field's plants (`_grass.bin`, `_grass.g1t`: barrel_ryza::grass), when it has them.
    let grass = barrel_engine::field::named(&main, &format!("{}_grass.bin", files.area)).and_then(|b| barrel_ryza::grass::Grass::parse(b).ok());
    let grass_pack = barrel_engine::field::named(&main, &format!("{}_grass.g1t", files.area)).and_then(|b| G1t::parse(b).ok());
    // The field's water (`<FIELD>_river.bin`): the pieces of its rivers and sea, in render space.
    let water = barrel_engine::field::named(&main, &format!("{}_river.bin", files.area)).and_then(|b| barrel_ryza::river::parse(b).ok()).map(|bodies| water_slots(&bodies));
    // The game's general graphics preset for the graphics' foliage (full: High, reduced:
    // Middle, least: Low), its plants and levels of detail (`barrel_ryza::presets::Graphics`).
    let presets = {
        let general = match graphics.foliage {
            settings::Foliage::Full => "High",
            settings::Foliage::Reduced => "Middle",
            settings::Foliage::Least => "Low",
        };
        let read = |n: &str| install.read(&format!("\\saves\\graphicssetting\\{n}")).ok();
        barrel_ryza::presets::Graphics::read(general, &read).unwrap_or_else(|e| {
            eprintln!("graphics presets: {e}; the High one as shipped");
            barrel_ryza::presets::Graphics::HIGH
        })
    };
    let plant_preset = presets.grass;
    let lod_dist = presets.lod;
    eprintln!("presets: {:?} {presets:?}", graphics.foliage);
    // The title's backdrop: its event's stage run to the first frame's moment, and the characters
    // it stages (`actors`: Ryza, Lent and Tao), loaded before the textures that borrow them.
    let title_event = match a.title {
        true => Some(barrel_formats::ebd::Ebd::parse(&install.read(&barrel_ryza::event::path(actors::TITLE_EVENT).ok_or("title event")?)?)?),
        false => None,
    };
    // The event loops: a random wait, then one of three actions drawn each time. BARREL_EVENT_DRAWS
    // (numbers with commas between) gives the first draws in the order the event asks, for
    // playing what a run of the game drew; after them the clock draws as the game's does. An
    // hour of it is laid out.
    let title_steps = match &title_event {
        Some(e) => {
            let given: Vec<f32> = std::env::var("BARREL_EVENT_DRAWS").ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default();
            let clock = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
            Some(barrel_ryza::event::schedule_drawn_taken(e, &given, clock, 3600.0)?)
        }
        None => None,
    };
    let (title_steps, title_taken) = title_steps.map_or((None, Vec::new()), |(s, t)| (Some(s), t));
    let title_motions = title_steps.as_ref().map(|s| actors::motions_of(s));
    let title_heads = actors::head_turns(&title_taken);
    let mut title_play = title_steps.map(|s| barrel_engine::stage::Playback::new(barrel_engine::stage::Stage::default(), s));
    let added = title_event.as_ref().map(barrel_ryza::event::added).transpose()?.unwrap_or_default();
    let mut cast_files: Vec<actors::Files> = Vec::new();
    if let (Some(motions), Some(play)) = (title_motions, title_play.as_mut()) {
        // after the boot the title's own time starts where the boot ends
        let boot_ahead = if a.boot { a.boot_at - barrel_ryza::boot::TITLE_START } else { 0.0 };
        actors::advance(play, a.title_at + boot_ahead - actors::EVENT_START);
        // Before the event has placed anyone (the boot's logos, the title's first frames) the
        // stage is run on to where it has, so its people are loaded and in the scene by the
        // time the title shows; the clock then catches the stage up (it never runs back).
        for _ in 0..PLACING_TICKS {
            if !play.stage.characters.is_empty() {
                break;
            }
            play.tick();
        }
        // the people the stage has placed, then the props only the event's `q5` adds
        let slots: std::collections::BTreeSet<u32> = play.stage.characters.keys().chain(added.keys()).copied().collect();
        for slot in slots {
            cast_files.extend(actors::Files::read(&install, slot, motions.get(&slot).unwrap_or(&Default::default()), added.get(&slot))?);
        }
    }
    let cast_models = cast_files.iter().map(actors::Files::container).collect::<std::result::Result<Vec<_>, _>>()?;
    let mut cast = cast_files.iter().zip(&cast_models).map(|(f, m)| f.open(m)).collect::<std::result::Result<Vec<_>, _>>()?;
    for a in cast.iter_mut() {
        a.head = title_heads.get(&a.slot).cloned().unwrap_or_default();
    }
    let mut textures: Vec<&barrel_formats::g1t::Texture> = pack.textures.iter().collect();
    // STAND-IN: Ryza is lit as the field's models are; the real thing is her own shaders and lighting.
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
    // The field's townsfolk, the ones standing there now (`barrel_ryza::npc`, docs/tables.md).
    let townsfolk = if a.title { Vec::new() } else { standing_npcs(&install, &a.field, a.time, a.weather, &a.story) };
    let gimmicks = if a.title { Vec::new() } else { standing_gimmicks(&install, &a.field, a.time, &a.story) };
    let ramp_bytes = match ryza_files.is_some() || a.title || !townsfolk.is_empty() || !gimmicks.is_empty() {
        true => Some(install.read("\\data\\x64\\shader\\texture\\chara_rmp.g1t")?),
        false => None,
    };
    let ramp_pack = ramp_bytes.as_deref().map(G1t::parse).transpose()?;
    // Her draws' ramp rows by (first index, instance): each submesh's `rampIndex`, and whether
    // it has a rim (`rmIndex`).
    let mut ramp_rows: std::collections::HashMap<(u32, u32), (u32, bool)> = Default::default();
    // The characters' rim (`environment::Rim`, the default environment override's).
    let rim = match ryza_files.is_some() || a.title {
        true => install.read(barrel_ryza::environment::Rim::DEFAULT_PATH).ok().and_then(|b| barrel_ryza::environment::Rim::parse(&b).map_err(|e| eprintln!("{e}")).ok()),
        false => None,
    };
    eprintln!("character rim: {rim:?}");
    // The saturation after the gamma, from the light sets' table.
    let tone_vivid = install.read(barrel_ryza::environment::SATURATION_PATH).map_err(|e| e.to_string()).and_then(|b| barrel_ryza::environment::saturation_ex(&b, barrel_ryza::environment::SATURATION_SET, a.time, a.weather)).map_err(|e| eprintln!("{e}")).unwrap_or(0.0);
    eprintln!("saturation after the gamma: {tone_vivid}");
    let rim_word = |sharpness: f32, strength: f32| u32::from_le_bytes([(sharpness / 64.0 * 255.0).round().clamp(0.0, 255.0) as u8, (strength / 4.0 * 255.0).round().clamp(0.0, 255.0) as u8, 0, 0]);
    let mut rig: Option<Rig> = None;
    if let (Some((at, turn, motion, time)), Some((model, _, motions)), Some(rp)) = (&a.ryza, &ryza_files, &ryza_pack) {
        let model = Elixir::parse(model)?;
        let skeleton = G1m::parse(barrel_engine::field::named(&model, "PC00A_MODEL.g1m").ok_or("no PC00A_MODEL.g1m")?)?.skeleton.ok_or("Ryza's model has no skeleton")?;
        let mut mesh = G1m::parse(barrel_engine::field::named(&model, "PC00A_MODEL_default.g1m").ok_or("no PC00A_MODEL_default.g1m")?)?;
        if let Some(g) = mesh.geometry.as_mut() {
            actors::place_cloths(g, &skeleton);
        }
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
        let (mut solids, fit) = placements.map(|p| barrel_ryza::solids::placed_meshes(&p, &objects)).unwrap_or_default();
        eprintln!("model collision: {fit:?}");
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
        // Her settings from the character tables (`barrel_ryza::chara`): her model's scale, her
        // size against the walls, the camera's look height.
        let chara = barrel_ryza::chara::Chara::read(&install.read("\\saves\\chara\\chara.xml")?, &install.read("\\saves\\chara\\charabase.xml")?, "PC00A")?;
        eprintln!("Ryza: {chara:?}");
        let instance = scene.instance(Mat4::from_scale_rotation_translation(Vec3::from(chara.size), glam::Quat::from_rotation_y(turn.to_radians()), *at));
        // Her pack follows the scene's textures, the toon ramp her pack.
        let drawn = actors::add_character(&mut scene, geometry, rp.textures.len(), &joints, instance, textures.len(), textures.len() + rp.textures.len(), "ryza", &[], &mut ramp_rows)?;
        eprintln!("Ryza: {drawn} draws, {} joints, motion {motion} {:.2}s long", joints.len(), anim.duration);
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
    // The title's characters, one instance each, posed as the event's stage has them (the ramp
    // texture first, then each one's pack).
    if !cast.is_empty() {
        if let Some(r) = &ramp_pack {
            let ramp = textures.len();
            textures.extend(r.textures.first());
            for actor in cast.iter_mut() {
                let Some(play) = &title_play else { continue };
                // a prop has no body on the stage: the event's `q5` places it for good
                let body = play.stage.characters.get(&actor.slot);
                if body.is_none() && !actor.is_prop() {
                    continue;
                }
                let joints = actor.joints(body, play.stage.time);
                actor.instance = scene.instance(actor.model(body));
                let name = format!("slot {}", actor.slot);
                let drawn = actors::add_character(&mut scene, &actor.geometry, actor.pack.textures.len(), &joints, actor.instance, textures.len(), ramp, &name, &actor.swing, &mut ramp_rows)?;
                eprintln!("{name}: {drawn} draws, {:?} at stage {:.3} s", body.map(|b| (b.position, b.facing.to_degrees(), &b.motion)), play.stage.time);
                textures.extend(actor.pack.textures.iter());
            }
        }
    }
    // The townsfolk: one set of files and one texture pack a character, one instance (and its
    // own copy of the mesh) a person, each in its looped motion from the field's clock.
    // A walker's path (`move_data`), and the motion it goes in.
    let walks: Vec<Option<barrel_ryza::npc::Path>> = townsfolk.iter().map(|n| {
        let name = n.path.as_ref()?;
        let read = install.read(&format!("\\saves\\fieldmap\\npc\\move_data\\{name}.xml")).map_err(|e| e.to_string()).and_then(|b| barrel_ryza::npc::path(&b));
        read.map_err(|e| eprintln!("{name}: {e}")).ok()
    }).collect();
    // STAND-IN: A walker runs from 2 m a second and walks below; the real thing is the game's choice of motion for a path's speed (its paths have 1 and 3).
    let gait = |w: &barrel_ryza::npc::Path| if w.speed >= 200.0 { barrel_ryza::motion::RUN } else { barrel_ryza::motion::WALK };
    let mut npc_files: Vec<actors::Files> = Vec::new();
    for tag in townsfolk.iter().map(|n| n.character()).collect::<std::collections::BTreeSet<_>>() {
        let motions = townsfolk.iter().zip(&walks).filter(|(n, _)| n.character() == tag).flat_map(|(n, w)| match w {
            Some(w) => vec![barrel_ryza::motion::STAND, gait(w)],
            None => vec![n.motion],
        }).collect();
        match actors::Files::read_npc(&install, tag, &motions) {
            Ok(f) => npc_files.push(f),
            Err(e) => eprintln!("{tag}: not drawn: {e}"),
        }
    }
    let npc_models = npc_files.iter().map(actors::Files::container).collect::<std::result::Result<Vec<_>, _>>()?;
    let npc_actors = npc_files.iter().zip(&npc_models).map(|(f, m)| f.open(m)).collect::<std::result::Result<Vec<_>, _>>()?;
    // (actor, instance, motion) of each person drawn
    let mut npc_drawn: Vec<(usize, u32, u32, Option<&barrel_ryza::npc::Path>)> = Vec::new();
    if let (Some(r), false) = (&ramp_pack, npc_actors.is_empty()) {
        let ramp = textures.len();
        textures.extend(r.textures.first());
        for (k, actor) in npc_actors.iter().enumerate() {
            let tag = npc_files[k].tag();
            let base = textures.len();
            textures.extend(actor.pack.textures.iter());
            for (n, walk) in townsfolk.iter().zip(&walks).filter(|(n, _)| n.character() == tag) {
                // A walker without its path has nowhere to be.
                if n.path.is_some() && walk.is_none() {
                    continue;
                }
                // STAND-IN: A person stands at its marker's height even when `no_gravity` is 0; the real thing is dropping it onto the ground.
                let (at, yaw, motion) = match walk {
                    Some(w) => (w.at(0.0).0, w.at(0.0).1, gait(w)),
                    None => (n.position, n.yaw, n.motion),
                };
                let instance = scene.instance(actor.placed(Vec3::from(at), yaw.to_radians()));
                let joints = actor.joints_of(Some(motion), 0.0, 0.0);
                actors::add_character(&mut scene, &actor.geometry, actor.pack.textures.len(), &joints, instance, base, ramp, tag, &actor.swing, &mut ramp_rows)?;
                npc_drawn.push((k, instance, motion, walk.as_ref()));
            }
        }
        eprintln!("townsfolk: {} drawn of {} here", npc_drawn.len(), townsfolk.len());
    }
    // The gimmicks' models, one set of files each however many places use it, and one instance
    // a place, posed in the bind pose.
    let gimmick_files: Vec<actors::Files> = gimmicks.iter().map(|g| g.tag.as_str()).collect::<std::collections::BTreeSet<_>>().into_iter().filter_map(|tag| actors::Files::read_gimmick(&install, tag).map_err(|e| eprintln!("{tag}: not drawn: {e}")).ok()).collect();
    let gimmick_models = gimmick_files.iter().map(actors::Files::container).collect::<std::result::Result<Vec<_>, _>>()?;
    let gimmick_actors = gimmick_files.iter().zip(&gimmick_models).map(|(f, m)| f.open(m)).collect::<std::result::Result<Vec<_>, _>>()?;
    let mut gimmick_instances: Vec<u32> = Vec::new();
    if let (Some(r), false) = (&ramp_pack, gimmick_actors.is_empty()) {
        let ramp = textures.len();
        textures.extend(r.textures.first());
        for (file, actor) in gimmick_files.iter().zip(&gimmick_actors) {
            let base = textures.len();
            textures.extend(actor.pack.textures.iter());
            let joints = actor.joints_of(None, 0.0, 0.0);
            for g in gimmicks.iter().filter(|g| g.tag == file.tag()) {
                let instance = scene.instance(actor.gimmick_model(Vec3::from(g.position), g.yaw, g.rotation, g.scale));
                actors::add_character(&mut scene, &actor.geometry, actor.pack.textures.len(), &joints, instance, base, ramp, file.tag(), &actor.swing, &mut ramp_rows)?;
                gimmick_instances.push(instance);
            }
        }
        eprintln!("gimmicks: {} drawn of {} here", gimmick_instances.len(), gimmicks.len());
    }
    // STAND-IN: The sky dome's clouds stand still; the real thing is the game's scroll of their two layers (`txTrs01`, the second at half the first's rate).
    // The set's sky dome (its clouds; the blue is the sky plane's): its model and textures,
    // drawn as the distant view (after the sky plane, alpha-blended), where the set puts it
    // (`environment::sky_dome_place`: a place in the field, at height 0, and a scale).
    let dome_files = match files.env.as_deref().map(Elixir::parse).transpose()? {
        Some(env) => match barrel_engine::field::named(&env, &format!("{}_env.bin", files.area)) {
            Some(bin) => match barrel_ryza::environment::sky_dome(bin, a.time, a.weather)? {
                Some(path) => Some((path.clone(), install.read(&path)?, barrel_ryza::environment::sky_dome_place(bin, a.time, a.weather)?)),
                None => None,
            },
            None => None,
        },
        None => None,
    };
    let dome_container = dome_files.as_ref().map(|(_, b, _)| Elixir::parse(b)).transpose()?;
    let dome_parts = match &dome_container {
        Some(c) => {
            let real: Vec<_> = c.entries.iter().filter(|e| !e.is_dummy()).collect();
            let model = real.iter().find_map(|e| G1m::parse(c.data(e)).ok().filter(|m| m.geometry.is_some()));
            let pack = real.iter().find_map(|e| G1t::parse(c.data(e)).ok());
            model.zip(pack)
        }
        None => None,
    };
    if let Some((model, dp)) = &dome_parts {
        let (at, scale) = dome_files.as_ref().map_or(([0.0; 2], [1.0; 3]), |f| f.2);
        let first = scene.draws.len();
        let instance = scene.instance(Mat4::from_scale_rotation_translation(Vec3::from(scale), glam::Quat::IDENTITY, Vec3::new(at[0], 0.0, at[1])));
        scene.add_posed(model.geometry.as_ref().unwrap(), Mat4::IDENTITY, dp.textures.len(), None, instance)?;
        for d in &mut scene.draws[first..] {
            d.texture += textures.len();
            d.mask = d.mask.map(|m| m + textures.len());
            d.overlay = d.overlay.map(|o| o + textures.len());
            d.specular = None;
            d.field = Some(0);
            d.sky = true;
            d.casts = false;
            d.name = "sky dome".into();
        }
        eprintln!("sky dome {}: {} draws", dome_files.as_ref().unwrap().0, scene.draws.len() - first);
        textures.extend(dp.textures.iter());
    }
    let mut plant_boxes = std::collections::HashMap::new();
    if let (Some(g), Some(gp)) = (&grass, &grass_pack) {
        if let Some(atlas) = gp.textures.first() {
            // The cards' normal maps (the pack's third texture), for `NORMAL_MAPPED`.
            let normals = gp.textures.get(2).map(|_| textures.len() + 1);
            plant_boxes = add_plants(&mut scene, g, &main, &files.area, textures.len(), normals, plant_preset.density);
            textures.push(atlas);
            textures.extend(gp.textures.get(2));
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
    // otherwise; `NORMAL_MAPPED`), a sky dome's too; her draws keep the toon ramp there.
    for d in draws.iter_mut() {
        if d.normal_map.is_some() && !ramp_rows.contains_key(&(d.first_index, d.instance)) {
            d.mask = d.normal_map;
        }
    }
    for d in draws.iter_mut() {
        // STAND-IN: Ryza's models are lit at full ambient; the real thing is the game's own lighting from the environment.
        // Ryza's models' lighting material (G1MG section 0x10001) holds ambient 0: their
        // shaders light from the environment, not `cAmb`. Full until those shaders are read
        // (`docs/ryza/open.md`).
        d.light = FULL_LIGHT;
        let base = instances.get(d.instance as usize).map_or(0, |i| i.palette);
        // A plant cell has its own rule for the view's edges (`shown`, below): its box is kept
        // for the draw trace, padded past any view.
        d.bounds = match plant_boxes.get(&d.first_index).filter(|_| d.name == "plants") {
            Some(&(lo, hi)) => Some(Bounds { lo, hi, pad: f32::INFINITY, round: false }),
            // A rigid draw is where its vertices are (nothing but the plants sways in Ryza's
            // fields yet), so its box alone decides, as in the game: on the title it hides
            // the models whose boxes are outside the view. A skinned one keeps the padding.
            None => Bounds::of(&vertices, &indices, d.first_index, d.index_count, &palette, base).map(|b| {
                let rigid = indices.get(d.first_index as usize).and_then(|&i| vertices.get(i as usize)).is_some_and(|v| v.weights == [0; 4]);
                if rigid { Bounds { pad: 0.0, ..b } } else { b }
            }),
        };
    }
    // A placed object is in or out of the view as a whole, by the sphere around all its parts
    // (docs/renderer.md, "Ryza DX: which objects are drawn"): the game draws every part of an
    // object that reaches into the view, parts behind the camera among them.
    let mut whole: std::collections::HashMap<(u32, u8), Bounds> = Default::default();
    for d in draws.iter().filter(|d| d.object != 0) {
        let Some(b) = d.bounds else { continue };
        whole.entry((d.object, d.lod & 3)).and_modify(|w| *w = Bounds { lo: w.lo.min(b.lo), hi: w.hi.max(b.hi), pad: w.pad.max(b.pad), round: true }).or_insert(Bounds { round: true, ..b });
    }
    for d in draws.iter_mut().filter(|d| d.object != 0 && d.bounds.is_some()) {
        d.part = d.bounds.map(|b| (b.lo, b.hi));
        d.bounds = whole.get(&(d.object, d.lod & 3)).copied();
    }
    if draws.is_empty() {
        return Err("nothing to draw".into());
    }
    // The field's light: its environment's diffuse cube map, and the set for the time and
    // weather (sun, second light, ambient colour).
    let mut gust: Option<barrel_ryza::environment::Gust> = None;
    let mut cloud: Option<(barrel_ryza::environment::Pattern, Vec<u8>)> = None;
    // The specular light map (`_as.g1t`, the file's bytes) with the stage's scale and tint of it.
    let mut specular: Option<(Vec<u8>, f32, [f32; 3])> = None;
    let mut stage_light = None;
    let environment = match files.env.as_deref().map(Elixir::parse).transpose()? {
        Some(env) => {
            let ad = barrel_engine::field::named(&env, &format!("{}_ad.g1t", files.area)).ok_or("environment has no diffuse light map")?;
            let (c, _) = barrel_ryza::environment::diffuse_harmonics(ad)?;
            let bin = barrel_engine::field::named(&env, &format!("{}_env.bin", files.area)).ok_or("environment has no settings")?;
            let light = Light::read(bin, a.time, a.weather)?;
            let (fog, sky_fog) = barrel_ryza::environment::Fog::read(bin, a.time, a.weather)?;
            let sky_fog = (sky_fog, barrel_ryza::environment::SkyGlow::read(bin, a.time, a.weather)?, barrel_ryza::environment::DomeLight::read(bin, a.time, a.weather)?, barrel_ryza::environment::Clouds::read(bin, a.time, a.weather)?);
            gust = Some(barrel_ryza::environment::Gust::read(bin, a.time, a.weather)?);
            // The cloud shadows: the set's pattern over the field's light texture.
            if let (Some(p), Some(t)) = (barrel_ryza::environment::Pattern::read(bin, a.time, a.weather)?, barrel_engine::field::named(&env, &format!("{}_pat.g1t", files.area)).filter(|b| G1t::parse(b).is_ok_and(|g| !g.textures.is_empty()))) {
                eprintln!("cloud shadows: {p:?}");
                cloud = Some((p, t.to_vec()));
            }
            eprintln!("light ({:?}, {:?}): {light:?}; wind {gust:?}", a.time, a.weather);
            let chara = barrel_ryza::environment::CharaLight::read(bin, a.time, a.weather)?;
            eprintln!("character light: {chara:?}");
            // The stage's ambient (`StageLight`): the light map at the set's scale and colour.
            let stage = barrel_ryza::environment::StageLight::read(bin, a.time, a.weather)?;
            eprintln!("stage light: {stage:?}");
            specular = barrel_engine::field::named(&env, &format!("{}_as.g1t", files.area)).map(|b| (b.to_vec(), stage.specular_map, stage.specular_map_color));
            // The sun on the stage is the set's times the stage's key scale (3 at Rasenboden's
            // noon); a character's draws take their own scale of it (0.8 there).
            let key = if stage.key > 0.0 { stage.key } else { 1.0 };
            // The second light's scale on the stage likewise (1 at noon).
            let second = if stage.second > 0.0 { stage.second } else { 1.0 };
            for d in draws.iter_mut().filter(|d| ramp_rows.contains_key(&(d.first_index, d.instance))) {
                d.light = ((chara.key / key).clamp(0.0, 1.0) * 65535.0).round() as u32 | 0xffff << 16;
            }
            stage_light = Some(stage);
            let stage = stage.harmonics(&light, &c);
            Some((c, stage, light, sky_fog, fog, chara, [key, second]))
        }
        None => None,
    };
    eprintln!("{} draws, {} vertices, {} triangles, {} textures", draws.len(), vertices.len(), indices.len() / 3, textures.len());

    let (mut width, mut height) = a.size;
    // A window to play in (--window), or an offscreen image.
    let display = match config.as_deref() {
        Some(text) => settings::Display::from_config(text),
        None => settings::Display { width, height, fullscreen: std::env::var_os("BARREL_DEFAULT_FULLSCREEN").is_some_and(|v| v == "1") },
    };
    let display = settings::Display { fullscreen: display.fullscreen || a.fullscreen, ..display };
    if a.window {
        eprintln!("graphics: {graphics:?}, {display:?}");
    }
    // The window the locate screen opened is the one to play in (a process has one event loop).
    let mut pump = match kept_pump.take() {
        Some(mut p) if a.window => {
            p.reopen(display)?;
            Some(p)
        }
        _ => a.window.then(|| window::Pump::open(display)).transpose()?,
    };
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
    // Without their largest mip levels on a lower preset (`presets.texture_bias`: the game's
    // texture mip bias, 1 on the handheld's Middle), saving memory and bandwidth.
    let biased: Vec<barrel_formats::g1t::Texture> = textures.iter().copied().chain(composite_textures.iter()).map(|t| t.without_top_levels(presets.texture_bias)).collect();
    let textures: Vec<&barrel_formats::g1t::Texture> = biased.iter().collect();
    eprintln!("textures: {} at {:.1} MB (mip bias {})", textures.len(), textures.iter().map(|t| t.levels.iter().map(|l| l.len()).sum::<usize>()).sum::<usize>() as f64 / 1e6, presets.texture_bias);
    // The game lights in linear light: the colour textures (a draw's own, and its emission
    // map) are sampled through their sRGB formats; normal maps, masks and the ramp as stored.
    let mut linear = vec![false; textures.len()];
    for d in &draws {
        for k in [Some(d.texture), d.emission].into_iter().flatten() {
            linear[k] = true;
        }
    }
    for d in &draws {
        // (a character's emission slot holds its occlusion map: as stored)
        let occlusion = d.emission.filter(|_| ramp_rows.contains_key(&(d.first_index, d.instance)));
        for k in [d.mask, d.normal_map, occlusion].into_iter().flatten() {
            linear[k] = false;
        }
    }
    let (uploaded, outline_tex, white) = upload_textures(&gpu, renderer.pool, &textures, None, &linear)?;
    // The cloud shadows' texture, sampled as sRGB as the game does.
    let cloud_texture = match &cloud {
        Some((_, bytes)) => Some(barrel_render::upload_texture(&gpu, renderer.pool, &G1t::parse(bytes)?.textures[0], true)?),
        None => None,
    };
    // The specular light map as a cube map; its mip count less one is how far the gloss may
    // blur the reflection (`vAmbParam.z`).
    let specular_cube = specular.as_ref().and_then(|(bytes, ..)| {
        let g = G1t::parse(bytes).map_err(|e| eprintln!("specular light map: {e}")).ok()?;
        let t = g.textures.first()?;
        let cube = upload_cube(&gpu, renderer.pool, t).map_err(|e| eprintln!("specular light map: {e}")).ok()?;
        Some((cube, t.levels.len().saturating_sub(1) as f32))
    });
    let specular_mips = specular_cube.as_ref().map_or(0.0, |c| c.1);
    let specular_cube = specular_cube.map(|c| c.0);
    let spec_on = specular_cube.is_some();
    let grounds = upload_grounds(&gpu, renderer.pool, &terrains, true)?;
    let data = scene_gpu::SceneData { vertices: vbytes, indices: ibytes, textures: uploaded, instances: &instances, palette: &palette, deltas: &deltas, weights: &morph_weights, grounds, patterns: vec![cloud_texture], specular_cubes: vec![specular_cube], white };
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
    let fov = a.fov.unwrap_or(if a.title { barrel_ryza::title::CAMERA.fov } else { follow.map_or(30.0, |c| c.fov) }).to_radians();
    // How high above her feet the camera looks (`LOOK_HEIGHT`).
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
    // The title's camera is fixed (`title::CAMERA`).
    let title_clip = a.title.then_some((barrel_ryza::title::CAMERA.near, barrel_ryza::title::CAMERA.far));
    if a.title {
        (center, yaw, pitch, dist) = barrel_ryza::title::CAMERA.look();
    }
    let shadow_size = (!a.no_shadows).then_some(renderer.shadow_target.size);

    let hidden = vec![false; instances.len()];
    // The field's draw distances (`camera::DrawRanges`): a field object farther from the eye
    // than its class's range is left out. Field objects stay where they were placed, so each
    // draw's box is found once (render space, as the eye).
    // The PC game uses them only below the top object-distance setting, or when the field's
    // file asks (docs/renderer.md, "Ryza DX: object draw ranges"); the setting's near, middle
    // and far ranges then stand in place of the field's.
    let ranges = match (a.draw_ranges.as_str(), install.read(&barrel_ryza::camera::FreeCamera::path(&a.field))) {
        ("off", _) | (_, Err(_)) => None,
        ("auto", Ok(bytes)) if !presets.ranged && !barrel_ryza::camera::DrawRanges::asked_for(&bytes) => None,
        (which, Ok(bytes)) => Some(barrel_ryza::camera::DrawRanges::parse(&bytes, which == "nx")?.preset(presets.objects)),
    };
    // STAND-IN: The townsfolk and gimmicks have no draw range; the real thing is the field file's `chara` and `gimmick` ranges, which like the objects' count only when ranges are in use.
    // The plants have their own range (`shown`, below), not an object's.
    let boxes: std::collections::HashMap<(u32, u32), (Vec3, Vec3)> = draws.iter().filter(|d| d.field.is_some() && !d.sky && d.name != "plants" && !npc_drawn.iter().any(|&(_, i, ..)| i == d.instance) && !gimmick_instances.contains(&d.instance)).filter_map(|d| {
        let b = d.bounds?;
        let m = instances.get(d.instance as usize)?.model;
        let corners = (0..8).map(|k| m.transform_point3(Vec3::new(if k & 1 == 0 { b.lo.x } else { b.hi.x }, if k & 2 == 0 { b.lo.y } else { b.hi.y }, if k & 4 == 0 { b.lo.z } else { b.hi.z })));
        let (lo, hi) = corners.fold((Vec3::MAX, Vec3::MIN), |(l, h), c| (l.min(c), h.max(c)));
        Some(((d.first_index, d.instance), (lo, hi)))
    }).collect();
    eprintln!("draw ranges: {ranges:?}");
    // STAND-IN: A placed object wholly below the ground's skyline is left out of the scene; the real thing is the game's own test of what is hidden, not read (this one, on the sphere the view test uses, hides 65 of the 107 objects the game hides in Rasenboden's capture and none it draws).
    // The skyline is the field's heightmap seen from the eye (docs/renderer.md, "Ryza DX:
    // objects behind the ground"); `--no-skyline` draws them all.
    let horizon = (!a.no_skyline && !a.title)
        .then(|| barrel_engine::field::named(&main, &format!("{}.khm", files.area.to_ascii_uppercase())).or_else(|| main.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".khm")).map(|e| main.data(e))))
        .flatten()
        .and_then(|b| barrel_formats::khm::parse(b).ok())
        .map(|g| std::cell::RefCell::new(barrel_ryza::horizon::Horizon::new(g)));
    let npc_spots = std::cell::RefCell::new(std::collections::HashMap::<u32, Vec3>::new());
    let in_sight = |d: &Draw| {
        let Some(h) = &horizon else { return true };
        // STAND-IN: A townsperson is tested as a sphere of PERSON_SPHERE.1 units around a point PERSON_SPHERE.0 above the feet; the real thing is the game's own bounds for a character.
        if let Some(&feet) = npc_spots.borrow().get(&d.instance) {
            return !h.borrow().hides(feet + Vec3::Y * PERSON_SPHERE.0, PERSON_SPHERE.1);
        }
        // The boxes are in the renderer's space, the skyline in the field's: x is mirrored.
        !horizon.as_ref().zip(boxes.get(&(d.first_index, d.instance))).is_some_and(|(h, &(lo, hi))| h.borrow().hides((lo + hi) * Vec3::new(-0.5, 0.5, 0.5), lo.distance(hi) * 0.5))
    };
    let eye_now = std::cell::Cell::new(Vec3::ZERO);
    // The plants' cells are drawn only as deep into the view as the grass preset's range ends:
    // a cell shows while its box's centre is nearer than that by the box's diagonal (the
    // title's capture: the deepest cell drawn is at 15000.0 by this measure, to the unit).
    let forward_now = std::cell::Cell::new(Vec3::ZERO);
    let sides_now = std::cell::Cell::new([glam::Vec4::ZERO; 4]);
    // The boot sequence's screens come before the scene exists: it is left out until the title.
    let scene_on = std::cell::Cell::new(true);
    let shown = |d: &Draw| {
        scene_on.get()
            && !a.hide.iter().any(|h| d.name.contains(h.as_str()))
            // A plant cell is drawn while its box's centre is short of the grass range's end by
            // the box's diagonal in depth, and no further outside a side of the view than
            // `PLANT_EDGE` across it (measured on the title: docs/renderer.md, "Ryza DX: which
            // plant cells are drawn").
            && plant_boxes.get(&d.first_index).filter(|_| d.name == "plants").is_none_or(|&(lo, hi)| {
                let c = (lo + hi) * 0.5;
                forward_now.get().dot(c - eye_now.get()) - lo.distance(hi) <= plant_preset.far[1] && sides_now.get().iter().all(|p| p.truncate().dot(c) + p.w >= -PLANT_EDGE)
            })
            && match (ranges, boxes.get(&(d.first_index, d.instance))) {
                (Some(r), Some(&(lo, hi))) => r.shows(d.class, eye_now.get(), lo, hi),
                _ => true,
            }
            // An object's level of detail by its distance from the eye (the game's LOD
            // distances, `presets.lod`): its full model nearer than the first, its middle one to
            // the second, its low one past it; a draw without a box, and every draw at the top setting,
            // the full one.
            && (d.lod == 0
                || boxes.get(&(d.first_index, d.instance)).filter(|_| presets.lods).map_or(d.lod & 3 == LOD_HI, |&(lo, hi)| {
                    let far = ((lo + hi) * 0.5).distance(eye_now.get());
                    let level = d.lod & 3;
                    let start = match level {
                        LOD_HI => 0.0,
                        LOD_MID => lod_dist[0],
                        _ => lod_dist[1],
                    };
                    let end = if d.lod & LOD_LAST != 0 { f32::INFINITY } else if level == LOD_HI { lod_dist[0] } else { lod_dist[1] };
                    far >= start && far < end
                }))
    };
    // Lit as the field's models: the environment's light on their vertex colour (no sun yet).
    // Her draws take the toon ramp (flag 128, the row in bits 16-23).
    let material = |d: &Draw| {
        // The sky dome's clouds lit by the set's light for them (flag 4096, look
        // slots `DOME_LIGHT_SLOT`, `DOME_SUN_SLOT`), as the game's `PBSkyDome` does.
        let base = if environment.is_some() {
            if d.sky {
                DOME_LIT | if d.normal_map.is_some() { NORMAL_MAPPED } else { 0 }
            } else if d.name == "plants" {
                // Normal-mapped by the cards' own normal maps, as `ObjectGrassPBS` does.
                FADED | if d.normal_map.is_some() { NORMAL_MAPPED } else { 0 }
            } else if d.normal_map.is_some() && !ramp_rows.contains_key(&(d.first_index, d.instance)) {
                NORMAL_MAPPED | if d.emission.is_some() { EMISSIVE } else { 0 }
            } else {
                0
            }
        } else {
            UNLIT
        };
        // A field model with a specular map reflects the environment's cube map.
        let base = if spec_on && d.specular.is_some() && !d.sky && d.name != "plants" && !ramp_rows.contains_key(&(d.first_index, d.instance)) { base | SPECULAR_AMBIENT } else { base };
        match ramp_rows.get(&(d.first_index, d.instance)) {
            Some(&(row, has_rim)) if ramp_pack.is_some() && environment.is_some() => {
                let r = rim.map_or(0, |r| if has_rim { rim_word(r.sharpness, r.strength) } else { rim_word(r.hair_sharpness, r.hair_strength) });
                (base | 128 | row << 16, r)
            }
            _ => (base, 0),
        }
    };
    // A telop over the frame (`--telop`): its strip from the layout, drawn from the hd atlas
    // (`neo_`) on screens taller than 1080 lines, else the plain one (English).
    // `--event`: the telop the event shows at that moment, at its opacity.
    let telop = match &event_stage {
        Some(stage) => barrel_ryza::event::telop(stage),
        None => a.telop.map(|n| (n, 1.0)),
    };
    let telop_layer = match telop {
        Some((n, alpha)) => Some(telop_layer(&install, &gpu, &renderer, n, alpha, (width, height))?),
        None => None,
    };
    // The title screen (`--title`): its glyphs from the executable, its layout from the archives.
    let mut title_layer = match a.title {
        true => Some(title_layer(&install, &gpu, &renderer, a.boot)?),
        false => None,
    };
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
    // The keys pressed this frame, for the title's menu, and what the player has done there.
    let mut title_keys: Vec<winit::keyboard::KeyCode> = Vec::new();
    let mut title_input = a.title_input.clone();
    // The title's music and voice (on the device in a window; offscreen only the cue log).
    let mut title_sound = a.title.then(|| barrel_ryza::sound::TitleSound::new(&install.root, pump.is_some()));
    // The camera's distance as drawn: pulled in by the ground behind her, easing back out.
    let (mut view_dist, mut frame_dt) = (None::<f32>, 0.0f32);
    loop {
        // Last frame's joints, then the field's moving objects at this frame's moment (the
        // game turns Rasenboden's windmills once in their motion's 12 s, all together).
        prev_palette.copy_from_slice(&palette);
        let field_time = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
        for m in &moving {
            m.pose(field_time, &instances, &mut palette);
        }
        for &(k, instance, motion, walk) in &npc_drawn {
            let actor = &npc_actors[k];
            // A walker is where its path has it by now, standing still through its pauses.
            let walking = walk.map(|w| w.at(field_time));
            let motion = if walking.is_some_and(|(.., moving)| !moving) { barrel_ryza::motion::STAND } else { motion };
            actor.pose_standing(instance, motion, field_time, &mut instances, &mut palette);
            if let Some((at, yaw, _)) = walking {
                instances[instance as usize].model = barrel_render::render_space(actor.placed(Vec3::from(at), yaw.to_radians()));
            }
        }
        // The player's input, in a window: her lean against the camera, the camera's turn.
        if let Some(p) = pump.as_mut() {
            use winit::keyboard::KeyCode as K;
            title_keys = p.take_pressed();
            if !p.pump() || title_keys.contains(&K::Escape) || a.frames.is_some_and(|n| frame_no >= n) {
                break;
            }
            let now = std::time::Instant::now();
            let dt = (now - last).as_secs_f32().min(0.1);
            last = now;
            frame_dt = dt;
            let key = |k: K| if p.held(k) { 1.0 } else { 0.0 };
            // The pad's sticks with the dead zone taken out (a stick resting a little off centre
            // neither walks her nor turns the camera).
            let (left, right) = p.sticks();
            let (left, right) = (barrel_render::pad::stick(left[0], left[1]), barrel_render::pad::stick(right[0], right[1]));
            // Right (the stick or the arrow key) swings the camera the way the game's does (the
            // other sign was mirrored against it, tried on a handheld).
            // (The title's camera is fixed: its keys work the menu, and the play camera's
            // limits are not its own.)
            if !a.title {
                yaw += (key(K::ArrowRight) - key(K::ArrowLeft) + right[0]) * CAMERA_TURN * dt;
                pitch = (pitch + (key(K::ArrowUp) - key(K::ArrowDown) - right[1]) * CAMERA_TURN * dt).clamp(pitch_range.0, pitch_range.1);
                dist = (dist * (1.0 + (key(K::Equal) - key(K::Minus)) * dt)).clamp(length_range.0, length_range.1);
            }
            if let Some(r) = rig.as_mut() {
                let mut lean = [key(K::KeyD) - key(K::KeyA) + left[0] + a.lean[0], key(K::KeyW) - key(K::KeyS) + left[1] + a.lean[1]];
                let length = (lean[0] * lean[0] + lean[1] * lean[1]).sqrt();
                let full = if p.held(K::ShiftLeft) || p.held(K::ShiftRight) { 0.4 } else { 1.0 };
                if length > full {
                    lean = lean.map(|v| v / length * full);
                }
                // Against the camera, in render space (x mirrored), then game space. Right on
                // screen is the right-handed view's left, (-cos, 0, sin): the projection mirrors
                // clip-space x (as in Escha's walk).
                let y = yaw.to_radians();
                let (forward, side) = (Vec3::new(-y.sin(), 0.0, -y.cos()), Vec3::new(-y.cos(), 0.0, y.sin()));
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
                let base = instances[r.instance as usize].palette as usize;
                for (slot, j) in palette[base..].iter_mut().zip(&joints) {
                    *slot = render_space(*j);
                }
                let i = &mut instances[r.instance as usize];
                i.prev_model = i.model;
                i.model = render_space(Mat4::from_scale_rotation_translation(Vec3::from(r.chara.size), glam::Quat::from_rotation_y(r.player.turn), r.player.at));
                center = (r.player.at + Vec3::new(0.0, look_height, 0.0)) * Vec3::new(-1.0, 1.0, 1.0);
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
        // The title's characters on the event's stage at this frame's moment.
        if let Some(play) = title_play.as_mut().filter(|_| !cast.is_empty()) {
            let clock = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
            let boot_ahead = if a.boot { a.boot_at - barrel_ryza::boot::TITLE_START } else { 0.0 };
            actors::advance(play, a.title_at + boot_ahead + clock - actors::EVENT_START);
            for actor in &cast {
                if let Some(body) = play.stage.characters.get(&actor.slot) {
                    actor.pose(body, play.stage.time, &mut instances, &mut palette);
                }
            }
        }
        sync.wait_slot(&gpu, renderer.pool)?;
        if pump.is_some() || !cast.is_empty() || !moving.is_empty() || !npc_drawn.is_empty() {
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
        // The field's clock for the sky (the clouds' wind): a window's own, 60 Hz offscreen.
        let sky_clock = phase_override("BARREL_SKY_CLOCK").unwrap_or(if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 });
        let mut view = look_for(center, yaw, pitch, drawn_dist, fov, (width, height), prev_vp, environment.as_ref(), shadow_size, presets.shadow, title_clip, sky_clock);
        eye_now.set(Vec3::from_slice(&view.look[4][..3]));
        if let Some(h) = &horizon {
            h.borrow_mut().look_from(eye_now.get() * Vec3::new(-1.0, 1.0, 1.0));
            // Where the townsfolk stand this frame, in the field's space (x mirrored back).
            let mut spots = npc_spots.borrow_mut();
            spots.clear();
            spots.extend(npc_drawn.iter().map(|&(_, i, ..)| (i, instances[i as usize].model.w_axis.truncate() * Vec3::new(-1.0, 1.0, 1.0))));
        }
        forward_now.set((center - eye_now.get()).normalize_or_zero());
        // The view's four side planes, each scaled to measure across the view (along its own
        // axis) rather than square to the plane.
        let (across, up, deep) = (view.vp.row(0), view.vp.row(1), view.vp.row(3));
        sides_now.set([(deep + across) / across.truncate().length(), (deep - across) / across.truncate().length(), (deep + up) / up.truncate().length(), (deep - up) / up.truncate().length()]);
        // The plants' wind (sway slots 0 and 1; nothing else sways in Ryza's fields yet).
        // `g_vAttCache0` (a, a, a/2, -a/2) and {`g_fAttCache1` 1, `g_fPhase` 0, `g_fOscPhase`}.
        // From the field's wind (`environment::Gust`): the bend from its power now, the sway's
        // phase what it has turned since the field came up (a window's own clock, 60 Hz offscreen).
        let wind_clock = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
        let (wind_power, wind_turned) = gust.map_or((0.0, 0.0), |g| g.at(wind_clock));
        let bend = barrel_ryza::environment::Gust::bend(wind_power);
        // STAND-IN: The cloud shadows' drift starts with the field; the real thing is the game's own phase.
        if let Some((p, _)) = &cloud {
            let drift = |speed: f32| phase_override("BARREL_PATTERN_PHASE").unwrap_or_else(|| (wind_clock * speed).rem_euclid(1.0));
            view.look[48] = [1.0 / p.size[0], 1.0 / p.size[1], drift(p.speed[0]), drift(p.speed[1])];
            view.look[49] = [1.0, p.strength, 0.0, 0.0];
        }
        view.look[50] = [bend, bend, bend * 0.5, -bend * 0.5];
        // The water, where the set has a sky to reflect and the field has water.
        let water_pieces = match &water {
            Some(w) if view.sky_plane => {
                view.look[WATER_SLOT..WATER_SLOT + w.len()].copy_from_slice(w);
                view.look[WATER_SLOT][1] = sky_clock;
                view.look[WATER_SLOT][0] as u32
            }
            _ => 0,
        };
        view.look[51] = [1.0, 0.0, wind_turned % std::f32::consts::TAU, 0.0];
        view.look[FADE_SLOT] = [plant_preset.far[0], plant_preset.far[1], 0.0, 0.0];
        view.look[EMISSION_SLOT] = [EMISSION_BY_TIME[a.time as usize], 0.0, 0.0, 0.0];
        if let Some((_, scale, tint)) = &specular {
            // STAND-IN: the light's weight (`vAmbSpcWgt.x`, 1 in the capture at Rasenboden's noon) is 1; the real thing is a run-time fade of it, not found in the set.
            view.look[SPECULAR_SLOT] = [*scale, specular_mips, 0.0, 0.0];
            view.look[SPECULAR_SLOT + 1] = [tint[0], tint[1], tint[2], 0.0];
        }
        // The ground's per-pixel lighting (terrain.frag: its normal maps and the direct specular), on in every field.
        view.look[SPECULAR_SLOT][2] = 1.0;
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
        // The title's quads for this frame's moment: the first frame at `--title-at`, then on the
        // clock (a window's own, 60 Hz offscreen).
        if let Some((layer, title, menu, boot)) = title_layer.as_mut() {
            let clock = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
            let title_seconds = match boot {
                Some(_) => a.boot_at + clock - barrel_ryza::boot::TITLE_START + a.title_at,
                None => a.title_at + clock,
            };
            // The player's keys in a window (the pad's buttons stand for them): Enter at the
            // prompt, the arrows on the menu. A direction is held while its key is down.
            if let Some(p) = pump.as_ref() {
                use winit::keyboard::KeyCode as K;
                let frame = (title_seconds * 60.0).round() as i32;
                if title_keys.contains(&K::Enter) && menu.pressed(&title_input).is_none() {
                    title_input.press = Some(frame);
                }
                for (key, way) in [(K::ArrowDown, 1), (K::ArrowUp, -1)] {
                    let held = p.held(key);
                    for h in title_input.holds.iter_mut().filter(|h| h.way == way && h.until.is_none() && !held) {
                        h.until = Some(frame);
                    }
                    if title_keys.contains(&key) {
                        // (a pad's direction is not held as a key is: it counts as a tap)
                        title_input.holds.push(barrel_ryza::title_menu::Hold { from: frame, until: (!held).then_some(frame + 1), way });
                    }
                }
            }
            let with_menu = Some((&*menu, &title_input));
            let quads = match boot {
                // The boot sequence, then the title from its own start.
                Some(boot) => {
                    let t = a.boot_at + clock;
                    scene_on.set(t >= barrel_ryza::boot::TITLE_START);
                    if t < barrel_ryza::boot::TITLE_START { boot.quads(t, (width, height)) } else { title.quads_with(title_seconds, (width, height), with_menu) }
                }
                None => title.quads_with(title_seconds, (width, height), with_menu),
            };
            if let Some(sound) = title_sound.as_mut() {
                sound.tick(title_seconds);
            }
            // The event's fade over the title: black from a point of the Gust logo.
            let mut quads = quads;
            let showing_title = boot.as_ref().is_none_or(|_| a.boot_at + clock >= barrel_ryza::boot::TITLE_START);
            let alpha = title_play.as_ref().filter(|_| showing_title).map_or(0.0, |p| actors::cover(&p.stage));
            if alpha > 0.0 {
                let texture = if boot.is_some() { barrel_ryza::boot::LOGO_GUST } else { 2 };
                quads.push(text::Quad { turn: false, spin: None, rect: [0.0, 0.0, width as f32, height as f32], uv: [0.5; 4], color: [0.0, 0.0, 0.0, alpha], texture });
            }
            layer.set(&gpu, &quads)?;
        }
        let layers: Vec<&text::Layer> = telop_layer.iter().chain(title_layer.as_ref().map(|(l, _, _, _)| l)).collect();
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
            in_sight: &in_sight,
            material: &material,
            draw_vp: view.vp,
            view_planes: view_planes(&view.vp),
            shadow_vp: view.shadow_vp,
            alpha_cutoff: MODEL_ALPHA_REF,
            sway_cutoff: PLANT_ALPHA_REF,
            // The set's bloom: `fLumiThreshold` as the bright pass's threshold (on the scene as
            // stored) and `fBloomWeight` as the rate.
            glow: environment.as_ref().map_or((1.0, 0.0), |(_, _, l, _, _, _, _)| (l.lumi_threshold, l.bloom_weight * GLOW_RANGE)),
            // The exposure's metering by the set's rule (a window's frame time; offscreen the
            // first frame meters to the scene).
            meter: environment.as_ref().zip(view.exposure).map(|((_, _, l, _, _, _, _), baked)| barrel_render::meter::Params {
                rule: barrel_render::meter::Rule { key: l.middle_gray, range: l.exposure_range, toward_darker: l.light_adapted_scale, toward_brighter: l.dark_adapted_scale },
                baked,
                dt: if frame_dt > 0.0 { frame_dt } else { 1.0 / 60.0 },
            }),
            saturation: [0.0; 3],
            // The game's tone curve: filmic, the set's `fLumiMax` as its white point, then the
            // set's `fGamma` (docs/shading.md, "Ryza DX: the tone curve").
            tone_white: environment.as_ref().map_or(0.0, |(_, _, l, _, _, _, _)| l.lumi_max),
            tone_gamma: environment.as_ref().map_or(0.0, |(_, _, l, _, _, _, _)| l.gamma),
            // The game's last pass scales the colour by the set's `vColorScale` before the
            // curve and lays `SaturationScaleEx` on after the gamma (docs/shading.md).
            tone_scale: environment.as_ref().map_or([0.0; 3], |(_, _, l, _, _, _, _)| l.color_scale),
            tone_vivid,
            filter: 0.0,
            camera: Camera { fov, near: view.near, far: view.far, ..Camera::default() },
            freeze_now: false,
            frozen_weight: 0.0,
            overlay: [0.0; 4],
            layers: &layers,
            readback: readback.map(|(b, _)| b),
            sky_plane: view.sky_plane,
            clouds: view.clouds,
            water_pieces,
        };
        constants::write_if_asked(|| {
            Some(constants::json(&constants::Inputs {
                look: &view.look,
                environment: environment.as_ref()?,
                stage: stage_light.as_ref()?,
                pattern: cloud.as_ref().map(|c| &c.0),
                specular_mips,
                seconds: sky_clock,
                glow: frame.glow,
                meter: frame.meter.as_ref(),
                tone_white: frame.tone_white,
                tone_gamma: frame.tone_gamma,
                tone_scale: frame.tone_scale,
                tone_vivid: frame.tone_vivid,
            }))
        });
        // The stage dumps (`BARREL_DUMP_STAGES`): the frame drawn again up to each stage, its
        // scene written; the last, full frame below adds the rest.
        if renderer.dump.is_some() && readback.is_some() && presenter.is_none() && frame_no + 1 >= a.frames.unwrap_or(1) {
            use barrel_render::dump::Stop;
            for stage in [Stop::Lit, Stop::Dome, Stop::Sky] {
                renderer.upto.set(stage);
                let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(renderer.pool).command_buffer_count(1))?[0] };
                unsafe { d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))? };
                renderer.record(&gpu, cb, &frame);
                unsafe { d.end_command_buffer(cb)? };
                sync.submit_offscreen(&gpu, cb)?;
                sync.wait_submitted(&gpu)?;
                if let Some(dump) = &renderer.dump {
                    dump.write(&gpu)?;
                }
            }
            renderer.upto.set(Stop::Full);
        }
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
                if let Some((e, lum)) = renderer.targets.glow.as_ref().and_then(|g| g.metered(&gpu)) {
                    eprintln!("metered exposure {e:.4} (scene luminance {lum:.4})");
                }
                let mut pixels = vec![0u8; bytes as usize];
                unsafe {
                    let p = d.map_memory(rmem, 0, bytes, vk::MemoryMapFlags::empty())? as *const u8;
                    std::ptr::copy_nonoverlapping(p, pixels.as_mut_ptr(), pixels.len());
                    d.unmap_memory(rmem);
                }
                std::fs::write(&a.out, barrel_formats::png::encode_rgba(width, height, &pixels))?;
                if let Some(dump) = &renderer.dump {
                    dump.write(&gpu)?;
                    // The final picture, as the game's tone pass leaves it.
                    std::fs::write(dump.dir.join("tone.png"), barrel_formats::png::encode_rgba(width, height, &pixels))?;
                }
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

/// A debug aid for comparing a frame with the game's: the sky's clock (seconds; the clouds'
/// wind and the water's flow) from `BARREL_SKY_CLOCK`, the cloud shadows' pattern offset
/// (both axes, in the pattern's own widths) from `BARREL_PATTERN_PHASE`, and the cloud map's
/// threshold from `BARREL_CLOUD_THRESHOLD`, in place of the field's own (the game's differ by
/// run: docs/parity.md, "The pixel board").
fn phase_override(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.trim().parse().ok()
}

// STAND-IN: The camera's look height is 0.8 of her CamH, measured from one frame; the real thing is the game's own rule.
/// How high above her feet the camera behind Ryza looks: measured from the game's eye (32526.8,
/// 6363.3, 29944.7) with her at (32518.2, 6179.6, 30331.5) and the field camera's 8° and 400;
/// about 0.8 of her `CamH` (160). The rule itself is not read (`docs/ryza/open.md`).
const LOOK_HEIGHT: f32 = 128.0;
/// How fast the camera turns, degrees a second.
const CAMERA_TURN: f32 = 120.0;

/// The field's environment: the diffuse cube map's light, the set's light and its sky fog.
type Environment = ([[f32; 3]; 9], [[f32; 3]; 9], Light, (barrel_ryza::environment::Fog, barrel_ryza::environment::SkyGlow, barrel_ryza::environment::DomeLight, barrel_ryza::environment::Clouds), barrel_ryza::environment::Fog, barrel_ryza::environment::CharaLight, [f32; 2]);

/// A frame's view: the look block and what goes with it.
struct View {
    look: [[f32; 4]; LOOK_SLOTS],
    vp: Mat4,
    shadow_vp: Mat4,
    /// The far shadow square wanted and its look slot (texel, layer to fill, fades).
    shadow_far: Option<(Mat4, [f32; 4])>,
    sky_plane: bool,
    /// The sky's large clouds, when the set has them: what their maps are made from.
    clouds: Option<CloudFrame>,
    near: f32,
    far: f32,
    exposure: Option<f32>,
}

/// A field's water as look slots from `WATER_SLOT` (`water.vert`): {pieces, 0}, then a piece's five:
/// its curve (x mirrored into render space) with its end widths, and its body's colour and
/// reflectance at normal incidence.
fn water_slots(bodies: &[barrel_ryza::river::Body]) -> Vec<[f32; 4]> {
    let mut slots = vec![[0.0; 4]];
    for body in bodies {
        for p in body.pieces() {
            if slots.len() + 5 > 1 + 5 * MAX_WATER_PIECES {
                break;
            }
            let mirror = |v: [f32; 3], w: f32| [-v[0], v[1], v[2], w];
            slots.extend([mirror(p.a, p.widths[0]), mirror(p.b, p.widths[1]), mirror(p.c, 0.0), mirror(p.d, 0.0), [body.albedo[0], body.albedo[1], body.albedo[2], body.reflectance[0]]]);
        }
    }
    slots[0][0] = ((slots.len() - 1) / 5) as f32;
    slots
}

/// The view from `dist` away from `center` (render space) at `yaw` and `pitch` degrees, and
/// its look block (`barrel_render`'s `Look`): the eye, the view-projection and last frame's
/// (`prev_vp`; this one's when there is none), and the environment's light, sky and the sun's
/// shadow (`shadow_size`: the shadow map's, none without shadows; `shadow_reach`: the preset's
/// scale of the far layer's reach, `clip` the near and far planes where the scene fixes them). With no environment, a
/// white ambient alone (the terrain shows its colours as they are).
#[allow(clippy::too_many_arguments)]
fn look_for(center: Vec3, yaw: f32, pitch: f32, dist: f32, fov: f32, (width, height): (u32, u32), prev_vp: Option<Mat4>, environment: Option<&Environment>, shadow_size: Option<u32>, shadow_reach: f32, clip: Option<(f32, f32)>, seconds: f32) -> View {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    let eye = center + Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist;
    let view = Mat4::look_at_rh(eye, center, Vec3::Y);
    let (near, far) = clip.unwrap_or((dist * 0.01, (dist * 20.0).max(80_000.0)));
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
    let mut clouds = None;
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
    look[26] = [1.0, 1.0, 1.0, 1.0];
    look[66][1] = 1.0;
    // The environment's light instead, when the field has one, in linear light: the cube map's
    // light as the stage takes it (`StageLight`; model.vert's `sh`), the sun and the second light
    // as the field's two directional lights, all scaled by one exposure.
    if let Some((c, stage, light, sky_fog, fog, chara, [key, second])) = environment {
        let (key, second) = (*key, *second);
        // The scene is lit at the bottom of the set's range, and the renderer's metering
        // (`barrel_render::meter`) takes the frame to the exposure its brightness calls for.
        let k = light.exposure_range.0;
        // The sky dome's clouds: the set's own ambient and sun for them (`DomeLight`).
        {
            let dome = sky_fog.2;
            let way = -from_light(dome.elevation, dome.azimuth);
            look[DOME_LIGHT_SLOT] = [dome.ambient[0] * k, dome.ambient[1] * k, dome.ambient[2] * k, 1.0];
            look[DOME_SUN_SLOT] = [dome.sun[0] * k, dome.sun[1] * k, dome.sun[2] * k, 0.0];
            look[DOME_SUN_SLOT + 1] = [way.x, way.y, way.z, 0.0];
        }
        // The characters' ambient (model.vert: a flat one for flag 128).
        let ca = chara.ambient_light(light, c);
        look[26] = [ca[0] * k, ca[1] * k, ca[2] * k, 1.0];
        // y: what the characters' second light is scaled by beyond the draws' `light` word
        // (which scales the sun and the second light alike by their sun's scale over the
        // stage's): their second light's scale over the stage's, over that.
        let chara_sun = (chara.key / key).clamp(0.0, 1.0);
        look[66] = [1.0, if chara_sun > 0.0 { chara.second / second / chara_sun } else { 1.0 }, 0.0, 0.0];
        for (i, [r, g, b]) in stage.iter().copied().enumerate() {
            look[67 + i] = [r * k, g * k, b * k, 0.0];
        }
        for (slot, p) in [22usize, 24].into_iter().zip(light.parallel) {
            let d = from_light(p.elevation, p.azimuth);
            look[slot] = [d.x, d.y, d.z, 0.0];
            let k = if slot == 22 { k * key } else { k * second };
            look[slot + 1] = [p.color[0] * k, p.color[1] * k, p.color[2] * k, 1.0];
        }
        // The scene's fog (`fog_`; heightfog.g1s and the lighting passes): extinction and
        // in-scattering over the eye distance mapped across its start and end, built as the
        // sky's Scat is (docs/shading.md).
        {
            let f = *fog;
            let span = (f.end - f.start).max(1.0);
            look[32] = [1.0 / span, -f.start / span, 0.0, 0.0];
            // The fog pass of the stage and characters takes the sun at the stage's key scale (the
            // lighting pass's: 3 at noon); the specular ambient's own fog takes it at 1
            // (`look[32].z` is the scale, for `specular_ambient.glsl` to divide out).
            let sun = light.parallel[0].color.map(|v| v * f.power * k * key);
            look[32][2] = key;
            let ext: [f32; 3] = std::array::from_fn(|i| f.rayleigh[i] + f.mie[i]);
            let part = |b: [f32; 3], i: usize| if ext[i] > 0.0 { b[i] / ext[i] } else { 0.0 };
            let ray_k = 3.0 / (16.0 * std::f32::consts::PI);
            let mie_k = (1.0 - f.g) * (1.0 - f.g) / (4.0 * std::f32::consts::PI);
            look[33] = [-ext[0] * std::f32::consts::LOG2_E, -ext[1] * std::f32::consts::LOG2_E, -ext[2] * std::f32::consts::LOG2_E, 0.0];
            look[34] = [0.0, 0.0, 0.0, 1.0];
            look[35] = [ray_k * sun[0] * part(f.rayleigh, 0), ray_k * sun[1] * part(f.rayleigh, 1), ray_k * sun[2] * part(f.rayleigh, 2), -2.0 * f.g];
            look[36] = [mie_k * sun[0] * part(f.mie, 0), mie_k * sun[1] * part(f.mie, 1), mie_k * sun[2] * part(f.mie, 2), 1.0 + f.g * f.g];
        }
        // The sky plane (docs/shading.md, "Ryza DX: the sky"): Scat as measured in the game,
        // from `skyFog_` and the sun's colour times its power (scaled by the frame's exposure).
        {
            let (f, glow, _, _) = *sky_fog;
            let sun = light.parallel[0].color.map(|v| v * f.power * k);
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
            // The path out through the atmosphere (`skyPos`: the ground's radius and the
            // atmosphere's height, the game's own) as a part of the fog's span (`fog`).
            let thick = (f.end - f.start).max(1.0);
            look[s0 + 5] = [-GROUND_RADIUS, GROUND_RADIUS * GROUND_RADIUS, (2.0 * GROUND_RADIUS + SKY_HEIGHT) * SKY_HEIGHT, 0.0];
            look[s0 + 6] = [1.0 / thick, -f.start / thick, 0.0, 0.0];
            // The haze (`vGlowDir`, `cGlow`, `hGlowParam`).
            let glow_way = from_light(glow.elevation, glow.azimuth);
            let ramp = glow.ramp();
            look[s0 + 8] = [glow_way.x, glow_way.y, glow_way.z, 0.0];
            look[s0 + 9] = [glow.color[0] * k, glow.color[1] * k, glow.color[2] * k, glow.fade];
            look[s0 + 10] = [ramp[0], ramp[1], 0.0, 0.0];
            let sun_way = from_light(light.parallel[0].elevation, light.parallel[0].azimuth);
            look[s0 + 7] = [sun_way.x, sun_way.y, sun_way.z, 0.0];
            sky_plane = true;
        }
        // The sky's large clouds (docs/shading.md, "Ryza DX: the sky, the clouds"): the constants
        // the game's strip draws take from the set's `cloud_` block and its light for them, in
        // render space (x mirrored) and times the exposure; the maps' constants beside.
        {
            let (sky, _, _, set) = *sky_fog;
            let s = set.shader(&sky, [-eye.x, eye.y, eye.z], seconds);
            let c = CLOUD_SLOT;
            let lit = |v: [f32; 3]| [v[0] * k, v[1] * k, v[2] * k];
            let wrap = |v: f32| v - v.floor();
            look[c] = [-s.lit_dir[0], s.lit_dir[1], s.lit_dir[2], 0.0];
            let l = lit(s.lit_col);
            look[c + 1] = [l[0], l[1], l[2], 0.0];
            // STAND-IN: The clouds' ambient light is none; the real thing is the lighting set's word for it (zero in the capture at Rasenboden's noon).
            look[c + 2] = [0.0, 0.0, 0.0, 0.0];
            look[c + 3] = [s.c_dif[0], s.c_dif[1], s.c_dif[2], s.range[1]];
            look[c + 4] = [s.pos_of[0], 0.0, s.range[2], s.range[3]];
            look[c + 5] = [s.tex_of[0], s.tex_of[1], wrap(s.tex_of[4]), wrap(s.tex_of[5])];
            look[c + 6] = s.trace_params;
            look[c + 7] = [s.trace_delta_scale[0], s.trace_delta_scale[1], s.fog[0], s.fog[1]];
            look[c + 8] = s.scat[0];
            for (row, scat) in s.scat.iter().enumerate().skip(1) {
                let v = lit([scat[0], scat[1], scat[2]]);
                look[c + 8 + row] = [v[0], v[1], v[2], scat[3]];
            }
            let (range, persistence, scale) = set.noise();
            clouds = Some(CloudFrame {
                density: CloudDensity { range, persistence, scale, octaves: barrel_ryza::environment::CLOUD_OCTAVES },
                shape: {
                    let threshold = phase_override("BARREL_CLOUD_THRESHOLD").unwrap_or(CLOUD_THRESHOLD);
                    CloudShape { threshold: [threshold; 4], sharpness: [cloud_sharpness(threshold); 4], thinness: [1.0; 4], height: [1.0; 4] }
                },
            });
        }
        // STAND-IN: The sun's shadow is one square over the camera's target; the real thing is the game's cascades.
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
            // 13000, are the near square here), scaled by the general preset's shadow distance
            // (`shadow_reach`: 1 on High, 0.7 on Middle; how the game applies it to its
            // cascades is not known, docs/ryza/open.md).
            let (fade_start, fade_end) = (20000.0 * shadow_reach, 30000.0 * shadow_reach);
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
    View { look, vp, shadow_vp, shadow_far, sky_plane, clouds, near, far, exposure: exposure_k }
}
