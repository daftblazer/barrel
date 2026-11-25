//! `barrel-ryza`: Atelier Ryza DX on Barrel. For now it draws one field offscreen:
//!
//! ```text
//!     barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--yaw DEG] [--pitch DEG]
//!                 [--look-at X,Y,Z] [--distance D] [--time morning|noon|evening|night]
//!                 [--weather sunny|cloudy|foggy] [--done EVENT[,EVENT...]] [--hide NAME]...
//!                 [--ryza X,Y,Z [--turn DEG] [--motion N] [--motion-time S]] [--no-shadows]
//!                 [--window [--fullscreen]] [--preset NAME] [--setting-ini FILE] [--fov DEG] [--draw-ranges auto|pc|nx|off]
//! ```
//!
//! FIELD is a field's folder name in the archives (`s01_i_attic`). The camera frames the field's
//! ground (its terrain's square; the backdrop's mountains far outside it are drawn, not framed),
//! or looks at X,Y,Z (field units, as the placement list gives them) from D away. The light is
//! the field environment's set for the time and weather (noon, sunny by default). `--hide`
//! leaves out the objects whose placement names contain NAME (to find which object is which);
//! `--unseen NAME` leaves them out of the view alone (they still cast their shadows: what the
//! game does with an object right in front of the camera).
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
use barrel_ryza::load as flow;
use glam::{Mat4, Vec3};

mod actors;
mod gathering;
mod cloth;
mod constants;

/// The renderer's material bit for a draw shown with its texture alone, unlit (`model.frag`).
/// The alpha test's reference (`vATest`) on every field and character model's draw of the
/// game's frame, and on its plants' (docs/shading.md): 50.1 and 63.1 of 255.
const MODEL_ALPHA_REF: f32 = 50.1 / 255.0;
const PLANT_ALPHA_REF: f32 = 63.1 / 255.0;

const UNLIT: u32 = 32;
/// STAND-IN: How far above its path a walker's feet may find the ground (the game's own rule for its walkers is not read).
const WALKER_STEP: f32 = 200.0;
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
// STAND-IN: The emission strengths of morning, evening and night are chosen by the stills; the real thing is the game's run-time emScl.
/// The emission maps' strength by time of day (morning, noon, evening, night), in the light's
/// units. Noon's is the game's: its `emScl` on a captured noon frame is 1 on the houses' window
/// glass (and 0.8 to 0.85 on five draws of one other material, not told apart here). The other
/// times' are a stand-in: the game sets `emScl` at run time (not read).
const EMISSION_BY_TIME: [f32; 4] = [0.0, 1.0, 2.0, 3.0];

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
    /// `--unseen NAME`: objects left out of the view that still cast their shadows.
    unseen: Vec<String>,
    /// The story's progress (`--done`, or the save's with `--save`).
    story: barrel_ryza::story::Story,
    /// A save file to take the story's progress from (`--save`).
    save: Option<String>,
    /// `--hud`: the field's HUD over a field she stands in (a `--save` shows it too).
    hud: bool,
    /// The basket's count the HUD shows (`--basket`; the field_hud's default is a save's).
    basket: Option<u32>,
    /// No area-name banner on the HUD's arrival (a new game's: the opening's hand-over shows the game's popups instead).
    no_banner: bool,
    /// `--menu-keys F:y,F:down,F:up,F:a,F:b`: the buttons of the field's menu at the capture's game frames (offscreen).
    menu_keys: Vec<(f32, barrel_ryza::field_menu::Key)>,
    /// The hour the save's clock word gives (`--save`).
    save_hour: Option<u32>,
    /// The frame of the game's capture the HUD's pulse starts at (`--hud-at`), to line a frame up with it.
    hud_at: f32,
    /// The capture frame the field's arrival falls on (`--hud-arrival`, 0 in a run of its own): the area name's banner and the help bar run from it.
    hud_arrival: f32,
    /// The capture's frames the move stick is out of its dead zone in (`--hud-stick FROM-TO,...`, the pad script's own frames), to replay the help bar by.
    hud_stick: Vec<(f32, f32)>,
    /// `--hud-slips F,..`: the capture frames at which the game's animation clock lost a frame (its looping glows run 1 frame behind from each).
    hud_slips: Vec<f32>,
    /// `--gather NEAR:PRESS`: the capture's frames a gathering point came in reach (its prompt began to slide in) and A was pressed at it; the HUD shows the gathering's popups by them (`BARREL_RANDOM_STATE` is the generator they draw from).
    gather: Option<(f32, f32)>,
    /// `--swing NEAR:PRESS`: the same for the staff's swing (X) at a point of its own kind.
    swing: Option<(f32, f32)>,
    /// `--spent ID,...`: the points already gathered from (they give nothing).
    spent: Vec<u32>,
    /// `--learned ITEM,...`: items the library knows in full from before the capture's start (an earlier gathering's).
    learned: Vec<i32>,
    /// The camera's yaw in the game's terms, when a run is given it (`--camera-yaw`), instead of behind her.
    camera_yaw: Option<f32>,
    /// `--camera-length L`: how far the eye is from her look point, as a capture has it (no pull-in against the ground behind her).
    camera_length: Option<f32>,
    /// `--camera-target X,Y,Z`: where the follow camera looks (game units), as a capture has it.
    camera_target: Option<[f32; 3]>,
    /// `--camera-pitch P`: the follow camera's pitch in degrees, as a capture has it.
    camera_pitch: Option<f32>,
    /// `--no-skyline`: objects behind the ground are drawn too.
    no_skyline: bool,
    no_occluders: bool,
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
    /// The original's `Setting.ini` (`--setting-ini`): its `[Graphics]` items over the config's.
    setting_ini: Option<String>,
    fullscreen: bool,
    /// Frames to draw: in a window, quit after this many; offscreen, the same frame this many
    /// times, the last written (for `--timings`). And a lean held all along in a window (tests).
    frames: Option<u32>,
    /// `--walk-from F`: offscreen, her walk goes on for `--frames` frames from the game's frame F,
    /// the sticks read from `BARREL_PAD_SCRIPT` at 1/60 s a frame, the last frame written.
    walk_from: Option<u32>,
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
    /// `--into-field`: offscreen too, the title's load goes on into the field from the chosen
    /// save (a window always does), the frame count and the sound's frames running on.
    into_field: bool,
    /// `--opening`: the game's opening event (`event_mm00_010`) played live from New Game's
    /// end, `--opening-at F` the capture frame the first frame shows (default
    /// `barrel_ryza::opening::START`, the event's first frame).
    opening: bool,
    opening_at: i32,
    /// `--settings-as-game`: the Settings screen lists only the game's own rows and choices (for the captures).
    settings_as_game: bool,
    /// What the player does on the title when no one is at the keys: `--title-press F` the
    /// title frame of the button at the prompt, `--title-keys F:down:N,F:up:N,...` a direction
    /// going down on frame F for N frames, `--title-saves` as if a save existed.
    title_input: barrel_ryza::title_menu::Input,
    /// `--saves DIR`: the folder holding `<user id>/GameData<NN>` and `AutoSaveData`, which the
    /// title's Load Game lists (read only). In a window without it the original's own folder in a
    /// Steam library's Proton prefix is searched.
    saves: Option<String>,
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
    /// The gimmicks' bodies among the walls, by `fixID`.
    bodies: Vec<(u32, Vec<barrel_engine::walk::Wall>)>,
    player: barrel_ryza::player::Player,
    chara: barrel_ryza::chara::Chara,
    /// Her face's expression and her shut eyes (`barrel_ryza::face`).
    face: Option<barrel_ryza::face::Face>,
    shut: Option<barrel_ryza::face::Face>,
    /// Her cloth's solvers, and her skinning matrices as last posed (game space).
    cloth: Option<cloth::ClothDriver>,
    joints: Vec<Mat4>,
}

fn args(args: Vec<String>) -> Result<Args> {
    let mut it = args.into_iter();
    let usage = "usage: barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--telop N] [--event NAME --at T] [--yaw DEG] [--pitch DEG] [--look-at X,Y,Z] [--distance D] [--time T] [--weather W]";
    let (install, field, out) = (it.next().ok_or(usage)?, it.next().ok_or(usage)?, it.next().ok_or(usage)?);
    let mut a = Args { install, field, out, size: (1280, 720), yaw: 0.0, pitch: 30.0, look_at: None, distance: None, time: Time::Noon, weather: Weather::Sunny, hide: Vec::new(), unseen: Vec::new(), story: Default::default(), save: None, hud: false, basket: None, no_banner: false, menu_keys: Vec::new(), save_hour: None, hud_at: 0.0, hud_arrival: 0.0, hud_stick: Vec::new(), hud_slips: Vec::new(), gather: None, swing: None, spent: Vec::new(), learned: Vec::new(), camera_yaw: None, camera_length: None, camera_target: None, camera_pitch: None, no_skyline: false, no_occluders: false, ryza: None, no_shadows: false, fov: None, draw_ranges: "auto".into(), window: false, preset: None, setting_ini: None, fullscreen: false, timings: false, frames: None, walk_from: None, lean: [0.0; 2], telop: None, event: None, opening: false, opening_at: barrel_ryza::opening::START, title: false, title_input: Default::default(), saves: None, title_at: std::env::var("BARREL_TITLE_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0), into_field: false, settings_as_game: false, boot: false, boot_at: std::env::var("BARREL_BOOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0) };
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
            "--unseen" => a.unseen.push(value()?),
            "--no-shadows" => a.no_shadows = true,
            "--fov" => a.fov = Some(value()?.parse()?),
            "--draw-ranges" => a.draw_ranges = value()?,
            "--window" => a.window = true,
            "--fullscreen" => a.fullscreen = true,
            "--preset" => {
                let name = value()?;
                a.preset = Some(RenderSettings::preset(&name).ok_or(format!("--preset {name}: no such preset"))?);
            }
            "--setting-ini" => a.setting_ini = Some(value()?),
            "--timings" => a.timings = true,
            "--frames" => a.frames = Some(value()?.parse()?),
            "--walk-from" => a.walk_from = Some(value()?.parse()?),
            "--title" => a.title = true,
            "--into-field" => a.into_field = true,
            "--opening" => a.opening = true,
            "--opening-at" => a.opening_at = value()?.parse()?,
            "--settings-as-game" => a.settings_as_game = true,
            "--title-at" => a.title_at = value()?.parse()?,
            "--title-press" => a.title_input.press = Some(value()?.parse()?),
            "--title-saves" => a.title_input.saves = true,
            "--saves" => a.saves = Some(value()?),
            "--title-keys" => {
                // F:down:N / F:up:N hold a direction on the menu for N frames; F:choose the button
                // on the menu's item; F:ldown:N / F:lup:N a direction on the load list, the Settings
                // list or its dialog's box; F:ok the button on a save, or A on the dialog; F:lb /
                // F:rb a tab button on Settings; F:left / F:right change the row under its cursor;
                // F:back is B (title frames).
                use barrel_ryza::title_menu::Hold;
                for key in value()?.split(',') {
                    let parts = key.split(':').collect::<Vec<_>>();
                    let from = parts.first().ok_or("--title-keys F:down:N,...")?.parse::<i32>()?;
                    let held = || -> Result<i32> { Ok(from + parts.get(2).ok_or("--title-keys F:down:N,...")?.parse::<i32>()?) };
                    match parts.get(1).copied() {
                        Some("down") => a.title_input.holds.push(Hold { from, until: Some(held()?), way: 1 }),
                        Some("up") => a.title_input.holds.push(Hold { from, until: Some(held()?), way: -1 }),
                        Some("ldown") => a.title_input.list_holds.push(Hold { from, until: Some(held()?), way: 1 }),
                        Some("lup") => a.title_input.list_holds.push(Hold { from, until: Some(held()?), way: -1 }),
                        Some("choose") => a.title_input.choose = Some(from),
                        Some("ok") => {
                            a.title_input.confirm = Some(from);
                            a.title_input.answers.push(from);
                        }
                        Some("right") => a.title_input.changes.push((from, 1)),
                        Some("left") => a.title_input.changes.push((from, -1)),
                        Some("back") => a.title_input.backs.push(from),
                        Some("lb") => a.title_input.tabs.push((from, -1)),
                        Some("rb") => a.title_input.tabs.push((from, 1)),
                        _ => return Err("--title-keys F:down:N,...".into()),
                    }
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
            "--hud" => a.hud = true,
            "--menu-keys" => {
                use barrel_ryza::field_menu::Key;
                for key in value()?.split(',') {
                    let (frame, name) = key.split_once(':').ok_or("--menu-keys F:y,F:down,...")?;
                    let key = match name {
                        "y" => Key::Open,
                        "up" => Key::Up,
                        "down" => Key::Down,
                        "a" => Key::Confirm,
                        "b" => Key::Back,
                        "left" => Key::Screen(barrel_ryza::options_run::Key::Left),
                        "right" => Key::Screen(barrel_ryza::options_run::Key::Right),
                        "lb" | "lt" => Key::Screen(barrel_ryza::options_run::Key::TabPrev),
                        "rb" | "rt" => Key::Screen(barrel_ryza::options_run::Key::TabNext),
                        _ => return Err("--menu-keys F:y,F:down,F:up,F:a,F:b,F:left,F:right,F:lb,F:rb".into()),
                    };
                    a.menu_keys.push((frame.parse()?, key));
                }
            }
            "--basket" => a.basket = Some(value()?.parse()?),
            "--save-hour" => a.save_hour = Some(value()?.parse()?),
            "--hud-at" => a.hud_at = value()?.parse()?,
            "--hud-arrival" => a.hud_arrival = value()?.parse()?,
            "--gather" => {
                let v = value()?;
                let (near, press) = v.split_once(':').ok_or("--gather NEAR:PRESS")?;
                a.gather = Some((near.parse()?, press.parse()?));
            }
            "--swing" => {
                let v = value()?;
                let (near, press) = v.split_once(':').ok_or("--swing NEAR:PRESS")?;
                a.swing = Some((near.parse()?, press.parse()?));
            }
            "--spent" => a.spent = value()?.split(',').map(str::parse).collect::<std::result::Result<_, _>>()?,
            "--learned" => a.learned = value()?.split(',').map(str::parse).collect::<std::result::Result<_, _>>()?,
            "--hud-slips" => a.hud_slips = value()?.split(',').map(str::parse).collect::<std::result::Result<_, _>>()?,
            "--hud-stick" => {
                for span in value()?.split(',') {
                    let (from, to) = span.split_once('-').ok_or("--hud-stick FROM-TO,...")?;
                    a.hud_stick.push((from.parse()?, to.parse()?));
                }
            }
            "--camera-yaw" => a.camera_yaw = Some(value()?.parse()?),
            "--camera-length" => a.camera_length = Some(value()?.parse()?),
            "--camera-target" => {
                let v = value()?;
                let n = v.split(',').map(str::parse).collect::<std::result::Result<Vec<f32>, _>>()?;
                a.camera_target = Some(*n.first_chunk::<3>().ok_or("--camera-target X,Y,Z")?);
            }
            "--camera-pitch" => a.camera_pitch = Some(value()?.parse()?),
            "--no-skyline" => a.no_skyline = true,
            "--no-occluders" => a.no_occluders = true,
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

/// The opening's 2D layer (a telop, then the event's fade) with, for each telop number the event
/// shows, its strip's rectangle, texture coordinates and atlas in the layer (a name card is not
/// drawn yet). The atlases are the hd ones above 1080 lines, as `telop_layer` has them.
#[allow(clippy::type_complexity)]
fn opening_telops(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer, cards: &std::collections::BTreeSet<u32>, pictures: &[u32], screen: (u32, u32)) -> Result<(text::Layer, std::collections::HashMap<u32, Vec<([f32; 4], [f32; 4], usize)>>, barrel_ryza::talk::Talk)> {
    use barrel_ryza::telop;
    let hd = screen.1 > 1080;
    let layout = install.read("\\saves\\ui_en\\a21_telop\\uil_a21_telop01.xml")?;
    let strips = telop::strips(&install.read("\\saves\\ui_en\\etc\\uis_a21_telop.xml")?)?;
    let (mut paths, mut bytes): (Vec<String>, Vec<Vec<u8>>) = (Vec::new(), Vec::new());
    let mut wanted = Vec::new();
    for &n in cards {
        // STAND-IN: Name-card telops are not drawn; the real thing is drawing them.
        let Some(telop::Node::Caption(k)) = telop::Node::of(n) else { continue };
        let node = format!("telop{k:02}");
        let (at, image) = telop::place(&layout, &node)?.ok_or(format!("no {node} in the telop layout"))?;
        let strip = strips.get(image as usize).ok_or(format!("telop {n}: no strip {image}"))?;
        let file = strip.texture.trim_start_matches("ui/");
        let path = if hd { format!("\\data\\x64\\res_en\\ui\\neo\\neo_{file}") } else { format!("\\data\\x64\\res_en\\ui\\{file}") };
        let k = match paths.iter().position(|p| *p == path) {
            Some(k) => k,
            None => {
                bytes.push(install.read(&path)?);
                paths.push(path);
                paths.len() - 1
            }
        };
        wanted.push((n, k, at, strip));
    }
    let files: Vec<G1t> = bytes.iter().zip(&paths).map(|(b, p)| G1t::parse(b).map_err(|e| format!("{p}: {e}"))).collect::<std::result::Result<_, _>>()?;
    let mut shown = Vec::new();
    for (n, k, at, strip) in wanted {
        let t = files[k].textures.first().ok_or("a telop atlas without a texture")?;
        shown.push((n, k, telop::quad(strip, at, screen, (t.width, t.height), if hd { 1.0 } else { 0.5 })));
    }
    let mut textures: Vec<&barrel_formats::g1t::Texture> = files.iter().filter_map(|f| f.textures.first()).collect();
    // The name cards' pictures come after the talk's (`a21_charatelop`: the plate and the names, its two textures).
    let card_file = install.read(&if hd { "\\data\\x64\\res_en\\ui\\neo\\neo_a21_charatelop.g1t" } else { "\\data\\x64\\res_en\\ui\\a21_charatelop.g1t" }.to_string())?;
    let card_g1t = G1t::parse(&card_file).map_err(|e| format!("a21_charatelop: {e}"))?;
    // The talk's textures follow the telops': the font, the arrow's pictures, the hints' plate, the letterbox's white.
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = barrel_ryza::font::glyphs(&exe).ok_or("the executable's font table")?;
    let talk_files = [
        install.read("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t")?,
        install.read("\\data\\x64\\res_en\\ui\\a21_balloon_frame.g1t")?,
        install.read("\\data\\x64\\res_cmn\\ui\\a21_common.g1t")?,
        install.read("\\data\\x64\\res_cmn\\ui\\sys_white.g1t")?,
    ];
    let talk_g1t: Vec<G1t> = talk_files.iter().map(|b| G1t::parse(b).map_err(|e| format!("talk textures: {e}"))).collect::<std::result::Result<_, _>>()?;
    let base = textures.len();
    // (the hints' plate is the fourth texture of a21_common)
    let (font, balloon, common, white) = (&talk_g1t[0].textures[0], &talk_g1t[1].textures[0], &talk_g1t[2].textures[3], &talk_g1t[3].textures[0]);
    textures.extend([font, balloon, common, white, &card_g1t.textures[0], &card_g1t.textures[1]]);
    // The tutorial's pictures: a tiled texture, two backgrounds, the arrow's shadow, then each picture (`t<84 + number>`).
    let mut set_files = vec![
        install.read("\\data\\x64\\character\\fm_d02_barrel_02\\fm_d02_barrel_02.g1t")?,
        install.read("\\data\\x64\\res_cmn\\ui\\a21_bg05.g1t")?,
        install.read("\\data\\x64\\res_cmn\\ui\\a21_bg12.g1t")?,
        install.read("\\data\\x64\\res_cmn\\ui\\sys_alpha_circle.g1t")?,
    ];
    for id in pictures {
        set_files.push(barrel_ryza::talk::picture_file(install, *id)?);
    }
    let set_g1t: Vec<G1t> = set_files.iter().map(|b| G1t::parse(b).map_err(|e| format!("tutorial textures: {e}"))).collect::<std::result::Result<_, _>>()?;
    textures.push(&set_g1t[0].textures[1]);
    textures.extend(set_g1t[1..].iter().map(|g| &g.textures[0]));
    let names = barrel_ryza::talk::names(&install.read("\\saves\\text_en\\str_event_chara_name.xml")?);
    let talk = barrel_ryza::talk::Talk::new(
        glyphs,
        (font.width, font.height),
        names,
        barrel_ryza::talk::Textures {
            font: base,
            balloon: base + 1,
            common: base + 2,
            white: base + 3,
            tiled: base + 6,
            background: base + 7,
            frame: base + 8,
            circle: base + 9,
            pictures: pictures.iter().enumerate().map(|(i, &id)| (id, base + 10 + i)).collect(),
        },
    );
    // The cards the event shows (`u65`'s numbers 1 to 14 are `name00` to `name13`).
    let mut cards_shown: std::collections::HashMap<u32, Vec<([f32; 4], [f32; 4], usize)>> = shown.into_iter().map(|(n, k, (rect, uv))| (n, vec![(rect, uv, k)])).collect();
    let (card_layout, card_styles) = (install.read("\\saves\\ui_en\\a21_telop\\uil_a21_telop01.xml")?, install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_charatelop.xml")?);
    let (k, left) = (screen.1 as f32 / barrel_ryza::telop::LAYOUT_HEIGHT, (screen.0 as f32 - 1280.0 * screen.1 as f32 / barrel_ryza::telop::LAYOUT_HEIGHT) * 0.5);
    for &n in cards {
        let Some(barrel_ryza::telop::Node::Name(card)) = barrel_ryza::telop::Node::of(n) else { continue };
        if card > 13 {
            continue;
        }
        let mut quads = Vec::new();
        for image in barrel_ryza::telop::name_card(&card_layout, &card_styles, card)? {
            let t = card_g1t.textures.get(image.texture).ok_or("a21_charatelop: a picture's texture")?;
            let scale = if hd { 1.0 } else { 0.5 };
            let [x, y, w, h] = image.rect;
            let (aw, ah) = (t.width as f32, t.height as f32);
            let rect = [left + image.at[0] * k, image.at[1] * k, left + (image.at[0] + image.size[0]) * k, (image.at[1] + image.size[1]) * k];
            quads.push((rect, [x * scale / aw, y * scale / ah, (x + w) * scale / aw, (y + h) * scale / ah], base + 4 + image.texture));
        }
        cards_shown.insert(n, quads);
    }
    // (the fade's plain quad reads no texture, but a layer needs one)
    let layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &textures)?;
    Ok((layer, cards_shown, talk))
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

/// Frames between a pad script's frame and the game frame its stick reaches the field's idle
/// count (`hud-walk`: the stick at script frame 1900 starts the help bar's fade-out at 1902, 2780
/// at 2782, and the last stick frame 3299 leaves it its 121 frames of rest before 3422).
const PAD_LAG: f32 = 2.0;

/// Frames a pressed button stays down in the captures' pad scripts (the third column of `pad.txt`).
const PAD_HOLD: f32 = 6.0;

/// The help bar at capture frame `tick` for a field whose arrival fell on `arrival`, the move
/// stick out of its dead zone at the script frames of `spans` (`--hud-stick`): stepped from the
/// arrival frame by frame.
fn replay_help(arrival: f32, tick: f32, spans: &[(f32, f32)], mut bar: barrel_ryza::field_hud::HelpBar) -> barrel_ryza::field_hud::HelpBar {
    let mut f = arrival;
    while f <= tick {
        bar.step(spans.iter().any(|&(from, to)| f >= from + PAD_LAG && f < to + PAD_LAG), 1.0);
        f += 1.0;
    }
    bar
}

/// The field HUD's layer (the textures of `barrel_ryza::field_hud`) and its view.
fn hud_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer, items: &[usize]) -> Result<(text::Layer, barrel_ryza::field_hud::Hud)> {
    use barrel_ryza::field_hud as fh;
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = |va| barrel_ryza::font::glyphs_at(&exe, va).ok_or("the executable's font table");
    // (file, texture number) in the layer's order (`field_hud`'s constants).
    let wanted = [
        ("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\font\\mainfont00_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_field.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21_minimap_image_006.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_common.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_icon.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_various.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_field.g1t", 1),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 3),
        ("\\data\\x64\\res_en\\ui\\a21_battle.g1t", 4),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_saveload.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_field.g1t", 0),
    ];
    // The gathered items' pictures, two each: the small one is the item's own texture of `a21_item_s.g1t`, the large one `a21_item_l_<n>.g1t`'s only.
    let item_files = items.iter().flat_map(|&n| [("\\data\\x64\\res_cmn\\ui\\a21_item_s.g1t".to_string(), n), (format!("\\data\\x64\\res_cmn\\ui\\a21_item_l_{n:03}.g1t"), 0)]);
    let wanted: Vec<(String, usize)> = wanted.iter().map(|(n, i)| (n.to_string(), *i)).chain(item_files).collect();
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let size = |i: usize| (tex[i].width, tex[i].height);
    let hud = fh::Hud::read(fh::Sources {
        places: &install.read("\\saves\\ui_en\\a21_field\\uil_a21_fm01.xml")?,
        anims: &install.read("\\saves\\ui_en\\uia_common.xml")?,
        help: &install.read("\\saves\\ui_en\\a21_button_help\\uil_a21_shade.xml")?,
        guide_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_common.xml")?,
        battle_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_battle.xml")?,
        autosave: &install.read("\\saves\\ui_cmn\\a21_saveload\\uil_a21_autosave.xml")?,
        spinner_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_saveload.xml")?,
        saveload_anims: &install.read("\\saves\\ui_en\\a21_saveload\\uia_a21_saveload.xml")?,
        minimap: &install.read("\\saves\\ui_en\\a21_field\\uil_a21_fm_minimap.xml")?,
        time: &install.read("\\saves\\ui_en\\a21_field\\uil_a21_time_display.xml")?,
        collect: &install.read("\\saves\\ui_en\\a21_field\\uil_a21_collect_item.xml")?,
        field_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_field.xml")?,
        icon_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_icon.xml")?,
        common_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_common.xml")?,
        various_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_various.xml")?,
        strings: &install.read("\\saves\\text_en\\strcombineall.xml")?,
        glyphs: [glyphs(barrel_ryza::font::MAIN_FONT_01_HD1080)?, glyphs(barrel_ryza::font::MAIN_FONT_00_HD1080)?],
        sizes: [size(2), size(3), size(4), size(5), size(6), size(7), size(8), size(9), size(10), size(11)],
        advertise: &install.read("\\saves\\ui_en\\a21_field\\uil_a21_fm_advertise01.xml")?,
        field_anims: &install.read("\\saves\\ui_en\\a21_field\\uia_a21_fm01.xml")?,
        item_sizes: (!items.is_empty()).then(|| [size(fh::item_texture(0, false)), size(fh::item_texture(0, true))]),
    })?;
    let layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &tex[..])?;
    Ok((layer, hud))
}

/// The field menu's layer (the textures of `barrel_ryza::field_menu`) and its view.
fn menu_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer) -> Result<(text::Layer, barrel_ryza::field_menu::View)> {
    use barrel_ryza::field_menu as fm;
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = |va| barrel_ryza::font::glyphs_at(&exe, va).ok_or("the executable's font table");
    // (file, texture number) in the layer's order (`field_menu`'s constants).
    let wanted = [
        ("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\font\\mainfont00_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_bg04.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_main_menu_image_00.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_main_menu.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_main_menu.g1t", 1),
        ("\\data\\x64\\res_en\\ui\\a21_main_menu.g1t", 2),
        ("\\data\\x64\\res_cmn\\ui\\sys_white.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_mix.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21_icon.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_main_menu_image.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 5),
    ];
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let size = |i: usize| (tex[i].width, tex[i].height);
    let view = fm::View::read(fm::Sources {
        layout: &install.read("\\saves\\ui_en\\a21_main_menu\\uil_a21_main_menu.xml")?,
        chara: &install.read("\\saves\\ui_en\\a21_main_menu\\uil_a21_main_menu_chara.xml")?,
        styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_main_menu.xml")?,
        photo_styles: &install.read("\\saves\\ui_en\\etc\\uis_a21_main_menu_image.xml")?,
        mix_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_mix.xml")?,
        icon_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_icon.xml")?,
        anims: &install.read("\\saves\\ui_en\\a21_main_menu\\uia_a21_main_menu.xml")?,
        common_anims: &install.read("\\saves\\ui_en\\uia_common.xml")?,
        help: &install.read("\\saves\\ui_en\\a21_button_help\\uil_a21_shade.xml")?,
        help_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_common.xml")?,
        strings: &install.read("\\saves\\text_en\\strcombineall.xml")?,
        glyphs: [glyphs(barrel_ryza::font::MAIN_FONT_01_HD1080)?, glyphs(barrel_ryza::font::MAIN_FONT_00_HD1080)?],
        figure_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_main_menu_image.xml")?,
        sizes: [size(fm::PHOTO), size(fm::MENU0), size(fm::MENU1), size(fm::MENU2), size(fm::MIX), size(fm::ICON), size(fm::GUIDE), size(fm::FIGURES), size(fm::RING)],
    })?;
    let layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &tex[..fm::TEXTURES])?;
    Ok((layer, view))
}

/// The Equipment page's layer (the fonts, the white and the page's own textures) and its view.
fn page_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer, party: &[barrel_ryza::field_menu::Member], tables: &barrel_ryza::party_stats::Tables, shelf: &[barrel_ryza::field_page::ShelfItem]) -> Result<(text::Layer, barrel_ryza::field_page::Page)> {
    use barrel_ryza::field_page as fp;
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = |va| barrel_ryza::font::glyphs_at(&exe, va).ok_or("the executable's font table");
    let read = |p: &str| install.read(p).map_err(|e| e.to_string());
    let page = fp::Page::read(fp::Files { read: &read, glyphs: [glyphs(barrel_ryza::font::MAIN_FONT_01_HD1080)?, glyphs(barrel_ryza::font::MAIN_FONT_00_HD1080)?] }, party, &|i| tables.image(i), shelf)?;
    let mut wanted: Vec<(String, usize)> = vec![
        ("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t".into(), 0),
        ("\\data\\x64\\res_en\\font\\mainfont00_hd1080_0.g1t".into(), 0),
        ("\\data\\x64\\res_cmn\\ui\\sys_white.g1t".into(), 0),
    ];
    wanted.extend(page.wanted().iter().cloned());
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &tex)?;
    Ok((layer, page))
}

/// The layer of the blurred menu picture and the vignettes over it (`barrel_ryza::field_menu::backdrop`).
fn backdrop_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer) -> Result<text::Layer> {
    let wanted = [("\\data\\x64\\res_cmn\\ui\\sys_white.g1t", 0), ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 1)];
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let mut layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &tex[..barrel_ryza::field_menu::BACKDROP_TEXTURES])?;
    // The game's first quad reads the blurred menu; the plain white texture stands in for it in the trace.
    layer.set_screen_texture(Some(barrel_ryza::field_menu::BACKDROP_WHITE));
    Ok(layer)
}

/// The load screen's layer (the textures of `barrel_ryza::load_view`), its view and the saving
/// notice drawn over the black while saves are read and one loads.
fn load_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer) -> Result<(text::Layer, barrel_ryza::load_view::View, barrel_ryza::boot::Boot)> {
    use barrel_ryza::load_view as lv;
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = |va| barrel_ryza::font::glyphs_at(&exe, va).ok_or("the executable's font table");
    // (file, texture number) in the layer's order (`load_view`'s constants).
    let wanted = [
        ("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\font\\mainfont00_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_saveload.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_saveload.g1t", 2),
        ("\\data\\x64\\res_en\\ui\\a21_saveload.g1t", 1),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 2),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21_bg07.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_caption.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_loading.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_loading.g1t", 1),
        // The notice's window and spinner (only their sizes are read).
        ("\\data\\x64\\res_en\\ui\\a21_memorymap.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_saveload.g1t", 0),
    ];
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let size = |t: &&barrel_formats::g1t::Texture| (t.width, t.height);
    // The styles count 2160-line pixels: twice the texture's.
    let style_size = |i: usize| (tex[i].width * 2, tex[i].height * 2);
    let boot = barrel_ryza::boot::Boot::read(barrel_ryza::boot::Sources {
        progress: &install.read("\\saves\\ui_en\\a21_saveload\\uil_a21_saveload_progress.xml")?,
        saveload_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_saveload.xml")?,
        saveload_anims: &install.read("\\saves\\ui_en\\a21_saveload\\uia_a21_saveload.xml")?,
        common_anims: &install.read("\\saves\\ui_en\\uia_common.xml")?,
        notification: &install.read("\\saves\\ui_cmn\\a21_saveload\\uil_a21_notification_display.xml")?,
        window_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_memorymap.xml")?,
        spinner_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_saveload.xml")?,
        strings: &install.read("\\saves\\text_en\\str_ui.xml")?,
        glyphs: glyphs(barrel_ryza::font::MAIN_FONT_01_HD1080)?,
        sizes: [size(&tex[2]), size(&tex[3]), size(&tex[11]), size(&tex[12]), size(&tex[0])],
    })?;
    let view = lv::View::read(lv::Sources {
        layout: &install.read("\\saves\\ui_en\\a21_saveload\\uil_a21_saveload.xml")?,
        styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_saveload.xml")?,
        common_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_common.xml")?,
        caption_styles: &install.read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_caption.xml")?,
        help: &install.read("\\saves\\ui_en\\a21_button_help\\uil_a21_shade.xml")?,
        anims: &install.read("\\saves\\ui_en\\uia_common.xml")?,
        strings: &install.read("\\saves\\text_en\\strcombineall.xml")?,
        glyphs: [glyphs(barrel_ryza::font::MAIN_FONT_01_HD1080)?, glyphs(barrel_ryza::font::MAIN_FONT_00_HD1080)?],
        atlases: [style_size(4), style_size(5), style_size(6), style_size(8)],
        loading_layout: &install.read("\\saves\\ui_cmn\\a21_loading\\uil_a21_loading.xml")?,
        loading_styles: &install.read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_loading.xml")?,
        loading_anims: &install.read("\\saves\\ui_cmn\\a21_loading\\uia_a21_loading.xml")?,
        loading_atlases: [style_size(9), style_size(10)],
    })?;
    let layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &tex[..lv::TEXTURES])?;
    Ok((layer, view, boot))
}

/// The Settings screen's layer (the textures of `barrel_ryza::options_view`), its view and the
/// words of its rows.
fn options_layer(install: &barrel_ryza::install::Install, gpu: &Gpu, renderer: &Renderer) -> Result<(text::Layer, barrel_ryza::options_view::View, std::collections::HashMap<u32, String>)> {
    use barrel_ryza::options_view as ov;
    let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
    let glyphs = |va| barrel_ryza::font::glyphs_at(&exe, va).ok_or("the executable's font table");
    // (file, texture number) in the layer's order (`options_view`'s constants).
    let wanted = [
        ("\\data\\x64\\res_en\\font\\mainfont01_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\font\\mainfont00_hd1080_0.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_option.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_common.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21v2_various.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 2),
        ("\\data\\x64\\res_cmn\\ui\\a21_common.g1t", 3),
        ("\\data\\x64\\res_cmn\\ui\\a21_bg08.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_bg05.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_caption.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\sys_no_image.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_mix.g1t", 2),
        ("\\data\\x64\\res_cmn\\ui\\a24_option_steam.g1t", 0),
        ("\\data\\x64\\res_cmn\\ui\\a21_window.g1t", 0),
        ("\\data\\x64\\res_en\\ui\\a21_memorymap.g1t", 3),
    ];
    let files = wanted.iter().map(|(n, _)| install.read(n)).collect::<Result<Vec<_>>>()?;
    let parsed = files.iter().zip(&wanted).map(|(b, (n, _))| barrel_formats::g1t::G1t::parse(b).map_err(|e| format!("{n}: {e}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    let tex = parsed.iter().zip(&wanted).map(|(g, (n, i))| g.textures.get(*i).ok_or(format!("{n}: no texture {i}"))).collect::<std::result::Result<Vec<_>, _>>()?;
    // The styles count 2160-line pixels: twice the texture's.
    let atlases = std::array::from_fn(|i| if i < ov::OPTION { (0, 0) } else { (tex[i].width * 2, tex[i].height * 2) });
    let read = |name: &str| install.read(name);
    let view = ov::View::read(ov::Sources {
        settings: &read("\\saves\\ui_cmn\\quick_menu\\uil_quick_menu_settings.xml")?,
        keys: &read("\\saves\\ui_cmn\\quick_menu\\uil_quick_menu_key_assignment.xml")?,
        tab: &read("\\saves\\ui_cmn\\quick_menu\\uil_quick_menu_tab.xml")?,
        saveload: &read("\\saves\\ui_en\\a21_saveload\\uil_a21_saveload.xml")?,
        help: &read("\\saves\\ui_en\\a21_button_help\\uil_a21_shade.xml")?,
        option_styles: &read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_option.xml")?,
        v2_common_styles: &read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_common.xml")?,
        v2_various_styles: &read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21v2_various.xml")?,
        common_styles: &read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_common.xml")?,
        caption_styles: &read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_caption.xml")?,
        mix_styles: &read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_mix.xml")?,
        pad_styles: &read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a24_option_steam.xml")?,
        anims: &read("\\saves\\ui_en\\uia_common.xml")?,
        notification: &read("\\saves\\ui_cmn\\quick_menu\\uil_quick_menu_notification.xml")?,
        balloon: &read("\\saves\\ui_en\\_menu_templates\\uil_mt_balloon_select.xml")?,
        balloon_line: &read("\\saves\\ui_en\\_line_format\\uil_lf_balloon_select.xml")?,
        window_styles: &read("\\saves\\ui_cmn\\gen_styles\\uis_gen_a21_window.xml")?,
        memorymap_styles: &read("\\saves\\ui_en\\gen_styles\\uis_gen_a21_memorymap.xml")?,
        glyphs: [glyphs(barrel_ryza::font::MAIN_FONT_01_HD1080)?, glyphs(barrel_ryza::font::MAIN_FONT_00_HD1080)?],
        atlases,
    })?;
    let strings = barrel_ryza::options::read_strings(&read("\\saves\\text_en\\strcombineall.xml")?)?;
    let layer = text::Layer::new(gpu, renderer.pool, renderer.post_pass, &tex[..ov::TEXTURES])?;
    Ok((layer, view, strings))
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

/// What the minimap marks in `field` at this point of the `story`: the exits open, the
/// `townsfolk` standing, and where the story goes next.
fn minimap_markers(install: &barrel_ryza::install::Install, field: &str, story: &barrel_ryza::story::Story, townsfolk: &[barrel_ryza::npc::Npc]) -> barrel_ryza::field_hud::Markers {
    use barrel_ryza::{gimmick, mapmarker};
    let exits = install
        .read(&format!("\\saves\\fieldmap\\gimmick\\gimmick_fieldmap_{}.xml", field.to_ascii_lowercase()))
        .map_err(|e| e.to_string())
        .and_then(|b| gimmick::exits(&b))
        .unwrap_or_else(|e| {
            eprintln!("exits of {field}: {e}");
            Vec::new()
        });
    let exits: Vec<_> = exits.into_iter().filter(|x| story.within(&x.story, false)).collect();
    let rows = install.read("\\saves\\mapmarker\\mapmarker.xml").map_err(|e| e.to_string()).and_then(|b| mapmarker::read(&b)).unwrap_or_default();
    let destination = mapmarker::current(&rows, story).and_then(|row| match row.target {
        Some(at) if row.field.eq_ignore_ascii_case(field) => Some(at),
        _ => exits.iter().find(|x| x.to.eq_ignore_ascii_case(&row.field)).map(|x| [x.position[0], x.position[2]]),
    });
    // The fishing places and the waterside, which have icons of their own.
    let spots = install.read(&format!("\\saves\\fieldmap\\gimmick\\gimmick_fieldmap_{}.xml", field.to_ascii_lowercase())).map_err(|e| e.to_string()).and_then(|b| gimmick::spots(&b)).unwrap_or_default();
    let at = |kind: &str| -> Vec<[f32; 2]> { spots.iter().filter(|s| s.kind == kind && story.within(&s.story, false)).map(|s| [s.position[0], s.position[2]]).collect() };
    barrel_ryza::field_hud::Markers {
        exits: exits.iter().map(|x| [x.position[0], x.position[2]]).collect(),
        fishing: at("FISHING"),
        water: at("WATERSIDE"),
        folk: townsfolk.iter().filter(|n| n.talk).map(|n| [n.position[0], n.position[2]]).collect(),
        destination,
        doors: exits.iter().filter(|x| barrel_ryza::gimmick::leads_inside(&x.to)).map(|x| x.position).collect(),
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
// STAND-IN: The cascades' ends along the view are the game's ambient-and-shadow pass's at Rasenboden's gate (`vS2VZ` and `vSMDF`: the second cascade from 1600, the third from 1600 + 2000, the static layer from 10000) and the fade into it its `sdplane`'s 3500; the real thing is how the game sets them each frame, not found (the set's `shadowMap_ShadowCascadeStart` words hold other numbers).
const CASCADE_ENDS: [f32; 3] = [1600.0, 3600.0, 10000.0];
const CASCADE_BLEND: f32 = 3500.0;
/// The game's shadow maps in texels (width, height) on the desktop: the three cascades (its
/// ambient-and-shadow pass's `scSMD` and textures at the gate: 2000 x 2400, 2000 x 2400, 1600 x
/// 1600), the static layer (its 2048-square semi-static map) and the characters' (`scSubSM`,
/// 1024). The shadow image's layers are `SHADOW_LAYER` a side on the desktop; a lower
/// setting's smaller image scales them all.
const SHADOW_MAPS: [[u32; 2]; shadow::CASCADES] = [[2000, 2400], [2000, 2400], [1600, 1600], [2048, 2048], [1024, 1024]];
const SHADOW_LAYER: u32 = 2400;

/// The maps' sizes in a shadow image `size` texels a side.
fn shadow_maps(size: u32) -> [[u32; 2]; shadow::CASCADES] {
    SHADOW_MAPS.map(|m| m.map(|v| (v as u64 * size as u64 / SHADOW_LAYER as u64) as u32))
}
// STAND-IN: The cascades' depth begins this far from the camera toward the sun, one plane for all three (the pass's `mW2S` at the gate: 36755); the real thing is where the game takes that plane from (the field's bounds?), not found.
const CASTER_PLANE: f32 = 36_755.0;
/// How the game writes the cascades' depth (its shadow passes' `sbias` at the gate): slope
/// scale, constant (of the cascade's depth), and the alpha under which a texel casts nothing.
const CASCADE_BIAS: [(f32, f32); 3] = [(1.0, 0.0001), (0.5, 0.0003), (0.05, 0.001)];
const CASCADE_ALPHA_REF: f32 = 100.0 / 255.0;
/// How the game writes the characters' map's depth (the shadow pass's `sbias` for its 25 draws at
/// the gate): slope scale and constant for the two sets of its shaders; the alpha reference is
/// the cascades'.
const SUB_BIAS: [(f32, f32); 2] = [(2.5, 0.01), (5.0, 0.07)];
// STAND-IN: The shadow shaders that take the second bias set are those the gate's capture shows with it (the model's shadow groups `@D0A7806D` and `@456FF036`; the other groups take the first); the real thing is the game's rule for choosing between its `ShadowSlopeBias` and `ShadowSlopeBiasEx`, not found.
const SUB_BIAS_EX_SHADERS: [&str; 2] = ["@D0A7806D", "@456FF036"];
// STAND-IN: The characters' map's box is the box of her draws (their vertices on the scene's starting joints) grown by these much sideways (x and z) and up and down, from the gate's: the game's box in the sun's view is a world box's (its v extent equals its depth, the sun at 45 degrees), 144.5 by 220.8 against her draws' own 76.2 by 163.1 (`mSubW2S`); the real thing is how the game bounds the characters (its box follows her pose: 136.8 by 231.1 in the turn), not found.
const SUB_BOX_PAD: Vec3 = Vec3::new(25.0, 6.7, 25.0);
// The static shadow map's box (`mSemiStaticW2S`, Rasenboden at noon, to 1e-4 of a texel): its
// u and v span the heightmap's 64,000-unit square at heights 0 to 5,000 exactly (the map's
// heights run 1,766 to 7,000, so the top is not the map's), and its depth runs 173,439 from
// the square's far corner toward the sun.
// STAND-IN: the top and the depth are one field's numbers; the rule behind them is not found.
const STATIC_TOP: f32 = 5000.0;
const STATIC_DEPTH: f32 = 173_439.0;

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

/// A standing person's sphere for the skyline test: its centre's height above the feet and
/// its radius (a person is some 170 units tall; the arms and what they carry reach wider).
const PERSON_SPHERE: (f32, f32) = (90.0, 120.0);

/// The field's plants as camera-facing cards on its heightmap (`barrel_ryza::grass`), one draw
/// a grid cell as the game's, the atlas being texture `atlas`.
/// Returns each draw's first index with two boxes of its cell in render space: the cell's
/// square from the lowest foot to the highest top of its plants (for the draw trace), and the
/// box the game tests against the view (`shown`): the square from the heightmap's lowest
/// height to its highest plus the cell's tallest plant (the plants' update, `0x140a78dd0`,
/// with the range the field sets from its heightmap).
fn add_plants(scene: &mut Scene, g: &barrel_ryza::grass::Grass, main: &Elixir, area: &str, atlas: usize, normals: Option<usize>, density: f32) -> Option<(PlantField, std::collections::HashMap<u32, [(Vec3, Vec3); 2]>)> {
    let (vertex_base, index_base) = (scene.vertices.len() as u32, scene.indices.len() as u32);
    let cards = plant_cards(g, main, area, density, vertex_base, index_base)?;
    let instance = scene.instance(Mat4::IDENTITY);
    scene.vertices.extend_from_slice(&cards.vertices);
    scene.indices.extend_from_slice(&cards.indices);
    scene.draws.extend(plant_draws(&cards, atlas, normals, instance));
    Some((PlantField { atlas, normals, instance, vertex_base, index_base }, cards.boxes()))
}

/// What the scene keeps of its plants to build them again for another density
/// (`presets.grass.density`): the cards are the scene's last vertices and indices, from these
/// bases on.
struct PlantField {
    atlas: usize,
    normals: Option<usize>,
    instance: u32,
    vertex_base: u32,
    index_base: u32,
}

/// The plants' cards for one density: their vertices and indices (the first vertex and index
/// being `vertex_base` and `index_base` in the scene) and each cell's range and boxes.
struct PlantCards {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    count: usize,
    cells: Vec<PlantCell>,
}

struct PlantCell {
    first_index: u32,
    index_count: u32,
    /// The cell's square from the lowest foot to the highest top of its plants (for the draw
    /// trace), and the box the game tests against the view (`shown`): the square from the
    /// heightmap's lowest height to its highest plus the cell's tallest plant (the plants'
    /// update, `0x140a78dd0`, with the range the field sets from its heightmap).
    boxes: [(Vec3, Vec3); 2],
}

impl PlantCards {
    /// Each cell's draw's first index with its two boxes.
    fn boxes(&self) -> std::collections::HashMap<u32, [(Vec3, Vec3); 2]> {
        self.cells.iter().map(|c| (c.first_index, c.boxes)).collect()
    }
}

/// The plants for `density`, as `add_plants` lays them out; none when the field has no heightmap.
fn plant_cards(g: &barrel_ryza::grass::Grass, main: &Elixir, area: &str, density: f32, vertex_base: u32, index_base: u32) -> Option<PlantCards> {
    let ([low, high], ground) = barrel_engine::field::named(main, &format!("{}.khm", area.to_ascii_uppercase()))
        .or_else(|| main.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".khm")).map(|e| main.data(e)))
        .and_then(|b| barrel_formats::khm::parse(b).ok())
        .map(|h| (h.range, barrel_engine::walk::Ground::of_heightmap(h)))?;
    let [cx, cz] = g.cell();
    let plants = g.plants(PLANT_LIMIT, density, |x, z| ground.height(x, z, f32::MAX / 2.0, 0.0));
    let mut blocks: std::collections::BTreeMap<(u32, u32), Vec<&barrel_ryza::grass::Plant>> = Default::default();
    for p in &plants {
        blocks.entry((p.cell[0], p.cell[1])).or_default().push(p);
    }
    let mut cards = PlantCards { vertices: Vec::new(), indices: Vec::new(), count: plants.len(), cells: Vec::new() };
    for (&(i, j), list) in &blocks {
        let first_index = index_base + cards.indices.len() as u32;
        let (foot, top) = list.iter().fold((f32::MAX, f32::MIN), |(f, t), p| (f.min(p.at[1]), t.max(p.at[1] + p.size[1])));
        let tallest = list.iter().fold(0.0f32, |t, p| t.max(p.size[1]));
        // (render space: game x mirrored)
        let square = |lo: f32, hi: f32| (Vec3::new(-((i + 1) as f32) * cx, lo, j as f32 * cz), Vec3::new(-(i as f32) * cx, hi, (j + 1) as f32 * cz));
        for p in list {
            let [u, v, cw, ch] = p.card;
            let [w, h] = p.size;
            // Vertices are built in render space: game x mirrored (`render_space`).
            // As the game builds a card: every corner at the plant's foot, its offset half the
            // width to either side and the height up (the wind leans the top by that height).
            let foot = [-p.at[0], p.at[1], p.at[2]];
            let base = vertex_base + cards.vertices.len() as u32;
            for (dx, dy, uv) in [(-0.5, 0.0, [u, v + ch]), (0.5, 0.0, [u + cw, v + ch]), (0.5, 1.0, [u + cw, v]), (-0.5, 1.0, [u, v])] {
                cards.vertices.push(Vertex { pos: foot, normal: PLANT_NORMAL, uv, joints: [0; 4], weights: [0; 4], morph: [0; 2], color: [255; 4], corner: [dx * w, dy * h, 0.0], tangent: 0 });
            }
            cards.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        cards.cells.push(PlantCell { first_index, index_count: index_base + cards.indices.len() as u32 - first_index, boxes: [square(foot, top), square(low, high + tallest)] });
    }
    eprintln!("plants: {} in {} draws", cards.count, cards.cells.len());
    Some(cards)
}

/// One draw a grid cell, as the game's, the atlas being texture `atlas`.
fn plant_draws(cards: &PlantCards, atlas: usize, normals: Option<usize>, instance: u32) -> impl Iterator<Item = Draw> + '_ {
    cards.cells.iter().map(move |c| Draw {
        first_index: c.first_index,
        index_count: c.index_count,
        texture: atlas,
        overlay: None,
        instance,
        mask: normals,
        normal_map: normals,
        emission: None,
        specular: None,
        eye: false,
        sky: false,
        scroll: 0.0,
        outline: None,
        field: Some(0),
        casts: false,
        sub_bias: None,
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
        two_sided: false,
    })
}

/// The most ticks the title's event is run ahead for its people's placing (ten seconds: it
/// places them within its first second).
const PLACING_TICKS: u32 = 600;

/// Plays Ryza with `raw`, the command line after the program's name. `chosen` is the install the
/// `barrel` program found and set up (`locate::start`); without it this finds the game itself:
/// `--game DIR`, the command line's INSTALL, `BARREL_GAME_DIR`, the config's, Steam's, else the
/// "Locate game files" screen; every game file is read through the layers over it
/// (`barrel_files`).
/// The input the path from the menu to the field follows: the button on the menu counting only
/// where it loads.
fn flow_input(input: &barrel_ryza::title_menu::Input, loads: &dyn Fn(&barrel_ryza::title_menu::Input, i32) -> bool) -> barrel_ryza::title_menu::Input {
    barrel_ryza::title_menu::Input { choose: input.choose.filter(|&c| loads(input, c)), ..input.clone() }
}

/// What the settings come to: the render settings the renderer is made with, the game's own
/// graphics settings and the presets its tables give them (`barrel_ryza::presets`). The start
/// and a live change of the settings (`Renderer::apply_settings`) resolve them the same way.
struct Resolved {
    graphics: RenderSettings,
    /// The render settings before the presets' passes, which the Settings screen's own rows show.
    render_before_presets: RenderSettings,
    game_settings: barrel_ryza::graphics::Settings,
    presets: barrel_ryza::presets::Graphics,
}

impl Resolved {
    /// `preset` (`--preset`), else Barrel's graphics `config` text over the desktop preset, else
    /// a launcher's default (`BARREL_DEFAULT_PRESET`), else the desktop's; the game's own
    /// settings from the config's game keys, then `ini` (`--setting-ini`) over them (High with
    /// nothing given); the presets through the game's tables, which `read` gives by file name.
    fn from(preset: Option<RenderSettings>, config: Option<&str>, ini: Option<&str>, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Resolved {
        // The game's scene is HDR (`trueHDR_`): drawn into a float target, so the tone curve
        // (`Frame::tone_white`) sees past the half range's 2.0. Its reflections and depth of field
        // are on with a preset; a config can turn them off.
        let ryza = |s: RenderSettings| RenderSettings { hdr_scene: true, reflections: true, depth_of_field: true, ..s };
        let graphics = preset
            .map(ryza)
            .or(config.map(|c| RenderSettings { hdr_scene: true, ..RenderSettings::from_config_over(ryza(RenderSettings::DESKTOP), c) }))
            .or(std::env::var("BARREL_DEFAULT_PRESET").ok().and_then(|n| RenderSettings::preset(&n)).map(ryza))
            .unwrap_or(ryza(RenderSettings::DESKTOP));
        let mut game_settings = barrel_ryza::graphics::Settings::default();
        if let Some(c) = config {
            game_settings.apply_config(c);
        }
        if let Some(ini) = ini {
            game_settings.apply_ini(ini);
        }
        // The settings through the game's tables (`saves/graphicssetting`). They switch Barrel's
        // passes only when the player gave some: else the render settings stand as they are.
        let presets = barrel_ryza::presets::Graphics::resolve(&game_settings, read).unwrap_or_else(|e| {
            eprintln!("graphics presets: {e}; the High one as shipped");
            barrel_ryza::presets::Graphics::HIGH
        });
        let render_before_presets = graphics.resolved();
        let graphics = if game_settings.given { presets.apply(graphics) } else { graphics }.resolved();
        // The game's shadow maps (`SHADOW_MAPS`) are up to 2400 texels: the image grows to hold
        // them, the handheld setting's in the same share.
        let graphics = RenderSettings { shadows: shadow::Shadows { size: graphics.shadows.size * SHADOW_LAYER / shadow::Shadows::DESKTOP.size, ..graphics.shadows }, ..graphics };
        Resolved { graphics, render_before_presets, game_settings, presets }
    }
}

/// The settings the player changed go into Barrel's own config (the graphics config's file),
/// their lines replacing the ones there; the rest of the file stays.
/// The Settings screen the field's menu opened: its run, the game frame of its first frame and
/// whether its leaving was told to the menu (`Key::Return`).
struct SettingsRun {
    run: barrel_ryza::options_run::Run,
    first: i32,
    told: bool,
    /// The stamp of the key last given to the screen: a key a window stamps on a game frame that was stepped already (it renders
    /// faster than 60 Hz) goes to the next frame stepped, so the keys are taken by their stamps and not by the frame alone.
    seen: f32,
}

/// Frames after the Settings screen's last one its caption and help bar take to go out (`field-menu`: the caption by 2431,
/// 30 frames from 2401), counted on from the frame after the last.
const SETTINGS_AFTER: f32 = 31.0;

impl SettingsRun {
    /// The game frame after the screen's last, once it is over.
    fn over_since(&self) -> Option<i32> {
        self.run.left_at().filter(|_| self.run.is_over()).map(|l| self.first + l)
    }
}

/// The screen's key for a key of the menu's script.
fn screen_key(key: barrel_ryza::field_menu::Key) -> Option<barrel_ryza::options_run::Key> {
    use barrel_ryza::field_menu::Key as M;
    use barrel_ryza::options_run::Key as S;
    match key {
        M::Up => Some(S::Up),
        M::Down => Some(S::Down),
        M::Confirm => Some(S::Confirm),
        M::Back => Some(S::Back),
        M::Screen(k) => Some(k),
        M::Open | M::Return => None,
    }
}

fn save_options(state: &barrel_ryza::options::State) {
    let Some(path) = barrel_files::config::graphics_path() else { return };
    let text = state.store(&std::fs::read_to_string(&path).unwrap_or_default());
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("options: {}: {e}", path.display());
    }
}

/// Where the title's run goes on to.
enum Next {
    /// The field the save the load screen chose names.
    Save(std::path::PathBuf),
    /// The opening event, New Game's.
    Opening,
    /// The free play the opening hands over to (the field from where she stands, the HUD arriving).
    FreePlay,
}

/// A save the title's load screen chose: the run goes on into the field from it, in the same
/// window.
struct Handoff {
    root: std::path::PathBuf,
    next: Next,
    /// The window's side, kept for the field (a window run only).
    app: Option<App>,
    /// Offscreen: the picture's path and size, and the frames left of `--frames`.
    out: String,
    size: (u32, u32),
    frames_left: Option<u32>,
    carry: Carry,
}

/// What lives as long as the window does: the window and its events, the Vulkan device and the
/// swapchain on it. A scene (title, field) is made fresh on top of it and torn down at its end;
/// the app goes on to the next scene.
struct App {
    pump: window::Pump,
    gpu: Gpu,
    presenter: present::Presenter,
}

/// What the field takes on from the title's run: the sound (its device, its cues) and where the
/// frame count stands, so the cue log counts on from the title's first frame.
#[derive(Default)]
struct Carry {
    sound: Option<barrel_ryza::sound::Sound>,
    /// The frame the field's first frame is.
    frame: i64,
    /// The frame the field's music is asked for (after the load's stop).
    music_at: i64,
}

pub fn run(raw: Vec<String>, chosen: Option<locate::Resolved>) -> Result<()> {
    let mut next = run_once(raw, chosen, Carry::default(), None)?;
    while let Some(h) = next {
        // Her place and the story's progress are the save's (`--save`), the field the one it names.
        let out = if h.app.is_some() { "-".to_string() } else { h.out };
        let mut args = vec![h.root.to_string_lossy().into_owned(), "-".into(), out];
        match &h.next {
            Next::Save(save) => args.extend(["--save".into(), save.to_string_lossy().into_owned()]),
            Next::Opening => args.push("--opening".into()),
            Next::FreePlay => args.extend(["--opening".into(), "--opening-at".into(), barrel_ryza::opening::FREE_PLAY.to_string()]),
        }
        args.extend(["--size".into(), format!("{}x{}", h.size.0, h.size.1)]);
        if h.app.is_some() {
            args.push("--window".into());
        }
        if let Some(n) = h.frames_left {
            args.extend(["--frames".into(), n.to_string()]);
        }
        next = run_once(args, Some(locate::Resolved { root: h.root, pump: None }), h.carry, h.app)?;
    }
    Ok(())
}

fn run_once(mut raw: Vec<String>, chosen: Option<locate::Resolved>, carry: Carry, app: Option<App>) -> Result<Option<Handoff>> {
    let game = match chosen {
        Some(game) => game,
        None => {
            let given = locate::take_option(&mut raw, "--game").map(std::path::PathBuf::from);
            let launched = locate::take_game_program(&mut raw);
            let named = raw.first().filter(|a| !a.starts_with("--")).map(std::path::PathBuf::from).or(launched);
            let Some(game) = locate::resolve(barrel_files::find::Title::Ryza, given, named)? else { return Ok(None) };
            game
        }
    };
    let mut kept_pump = game.pump;
    // No scene on the command line: the game from its start, as the original launches (the boot
    // screens, the title over its backdrop field, then the menu), in a window.
    if raw.first().is_none_or(|a| a.starts_with("--")) {
        let start = [&*game.root.to_string_lossy(), barrel_ryza::title::FIELD, "-", "--boot", "--window"].map(String::from);
        raw.splice(0..0, start);
    }
    let mut a = args(raw)?;
    // The saves the title's Load Game lists: `--saves`, else (in a window) the original's own
    // folder; the save the player chose goes on into the field. With saves the boot is the one
    // the game has with them (`boot::SAVES_TITLE_START`), else the one without.
    let save_slots = match (a.title, &a.saves, a.window) {
        (true, Some(dir), _) => barrel_ryza::saves::find(Some(Path::new(dir))),
        (true, None, true) => barrel_ryza::saves::find(None),
        _ => None,
    }
    .map(|d| barrel_ryza::saves::slots(&d))
    .unwrap_or_default();
    let boot_saves = a.boot && !save_slots.is_empty();
    let title_start = barrel_ryza::boot::title_start(boot_saves);
    // Graphics as Escha's: --preset, else Barrel's graphics config (in a window, or offscreen
    // when `BARREL_CONFIG` names one), else a launcher's default (`BARREL_DEFAULT_PRESET`,
    // `BARREL_DEFAULT_FULLSCREEN=1` on a handheld), else the desktop's.
    let read_config = || (a.window || std::env::var_os("BARREL_CONFIG").is_some()).then(|| barrel_files::config::graphics_path().and_then(|p| std::fs::read_to_string(p).ok())).flatten();
    let config = read_config();
    // The Settings screen's Audio, Camera and Game values as saved (applied again on Yes).
    let mut play = barrel_ryza::preferences::Play::from_config(config.as_deref());
    // The game's scene is HDR (`trueHDR_`): drawn into a float target, so the tone curve
    // (`Frame::tone_white`) sees past the half range's 2.0. Its reflections and depth of field
    // are on with a preset; a config can turn them off.
    let ini_text = match &a.setting_ini {
        Some(path) => Some(String::from_utf8_lossy(&barrel_files::read(Path::new(path)).map_err(|e| format!("--setting-ini {path}: {e}"))?).into_owned()),
        None => None,
    };
    let install = Install::open(Path::new(&a.install))?;
    let read_table = |n: &str| install.read(&format!("\\saves\\graphicssetting\\{n}")).ok();
    let Resolved { graphics, render_before_presets, game_settings, presets } = Resolved::from(a.preset, config.as_deref(), ini_text.as_deref(), &read_table);
    eprintln!("graphics: {game_settings:?} {presets:?}");
    let mut field_music: Option<u32> = None;
    let mut menu_shown = barrel_ryza::field_menu::Shown { party: Vec::new(), money: 0 };
    let mut party_tables: Option<barrel_ryza::party_stats::Tables> = None;
    let mut item_info: Option<barrel_ryza::item_info::Items> = None;
    // The container's items (the Container page's grid) and its places.
    let mut container: (Vec<barrel_ryza::save::Worn>, usize) = (Vec::new(), 0);
    let mut tools: Vec<barrel_ryza::save::Worn> = Vec::new();
    // The Guide's lists: the library's tables and the save's levels of its items and effects.
    let mut library = barrel_ryza::library::Library::default();
    let (mut know_items, mut know_effects): (Vec<i32>, Vec<i32>) = (Vec::new(), Vec::new());
    let mut played_events: Vec<usize> = Vec::new();
    let mut party_skills = barrel_ryza::party_skills::Skills::default();
    if let Some(save) = &a.save {
        let file = barrel_files::read(Path::new(save))?;
        if let Some(h) = barrel_ryza::saves::Header::parse(&file) {
            eprintln!("menu party {:?}", h.party);
            menu_shown = barrel_ryza::field_menu::Shown { party: h.party.iter().map(|&(id, level)| barrel_ryza::field_menu::Member { id, level, stats: Default::default(), worn: [None; 4] }).collect(), money: h.money };
        }
        let state = barrel_ryza::save::decode(&file).map_err(|e| format!("{save}: {e}"))?;
        // The party's numbers: the growth table at each member's level with the items they wear.
        if let (Some(saved), Ok(tables)) = (barrel_ryza::save::members(&state), barrel_ryza::party_stats::Tables::read(&install.read("\\saves\\party\\party_param.xml")?, &install.read("\\saves\\item\\itemdata.xml")?)) {
            for m in &mut menu_shown.party {
                if let Some(s) = saved.iter().find(|s| s.id == m.id) {
                    (m.worn, m.stats) = (s.worn, tables.stats(m.id, m.level, &s.worn));
                }
            }
            party_tables = Some(tables);
        }
        container = barrel_ryza::save::container(&state).unwrap_or_default();
        tools = barrel_ryza::save::tools(&state).map(|t| t.0).unwrap_or_default();
        know_items = barrel_ryza::save::knowledge(&state, "item").unwrap_or_default();
        know_effects = barrel_ryza::save::knowledge(&state, "eff").unwrap_or_default();
        played_events = barrel_ryza::save::events_done(&state).unwrap_or_default();
        if let Ok(l) = barrel_ryza::library::Library::read(&install.read("\\saves\\library\\libraryitem.xml")?, &install.read("\\saves\\library\\libraryeffdetail.xml")?) {
            library = l;
        }
        item_info = barrel_ryza::item_info::Items::read(&install.read("\\saves\\item\\itemdata.xml")?, &install.read("\\saves\\fieldmix\\fieldmixcat.xml")?).ok();
        if let Ok(skills) = barrel_ryza::party_skills::Skills::read(&install.read("\\saves\\party\\party_skill.xml")?, &install.read("\\saves\\skill\\skill_data.xml")?) {
            party_skills = skills;
        }
        let exe = barrel_files::read(Path::new(&a.install).join("Atelier_Ryza_DX.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
        a.story = barrel_ryza::save::story(&state, &pe).map_err(|e| format!("{save}: {e}"))?;
        a.save_hour = barrel_ryza::save::clock(&state).map(barrel_ryza::save::hour_of);
        // The field's music: the save's map row of the executable's table, by day or night.
        if let Ok(place) = barrel_ryza::save::place(&state) {
            let night = barrel_ryza::music::is_night(a.save_hour.unwrap_or(10));
            field_music = barrel_ryza::music::field_number(&exe, place.map, night).and_then(|n| barrel_ryza::music::music_hash(&barrel_ryza::music::parse_table(&install.read("\\saves\\sound\\stream.xml").unwrap_or_default()), n));
        }
        eprintln!("{save}: {} story events played", barrel_ryza::save::events_done(&state).map_or(0, |e| e.len()));
        // FIELD `-` without an event: the save's field and her place in it, as the game loads it.
        if a.field == "-" && a.event.is_none() {
            let place = barrel_ryza::save::place(&state).map_err(|e| format!("{save}: {e}"))?;
            a.field = barrel_ryza::maps::field_maps(&pe).get(place.map).ok_or(format!("{save}: field map {} not in the executable", place.map))?.clone();
            if a.ryza.is_none() {
                a.ryza = Some((Vec3::from(place.at), place.turn, "00000".into(), 0.0));
            }
            eprintln!("{save}: {} at {:?}, turn {:.1}°", a.field, place.at, place.turn);
        }
    }
    // `--opening` from the frame the opening is over: the free play it hands over to (a window gets there by `Next::FreePlay`).
    if a.opening && a.opening_at >= barrel_ryza::opening::FREE_PLAY {
        barrel_ryza::opening::free_play(&install, &mut a.story)?;
        let (at, yaw) = barrel_ryza::opening::player_start(&install)?;
        a.opening = false;
        a.hud = true;
        a.basket = Some(barrel_ryza::opening::BASKET);
        a.no_banner = true;
        a.ryza = Some((Vec3::from(at), yaw, "00000".into(), 0.0));
        a.hud_at = a.opening_at as f32;
        a.hud_arrival = barrel_ryza::opening::FREE_PLAY as f32;
        let exe = barrel_files::read(Path::new(&a.install).join("Atelier_Ryza_DX.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
        a.field = barrel_ryza::maps::field_maps(&pe).get(barrel_ryza::opening::FIELD_MAP as usize).cloned().ok_or("the opening's field map: not named")?;
    }
    // `--opening`: the field the opening's first field change names (its picture is the event's live stage).
    if a.opening && a.field == "-" {
        let path = barrel_ryza::event::path(barrel_ryza::opening::EVENT).ok_or("opening event")?;
        let event = barrel_formats::ebd::Ebd::parse(&install.read(&path)?).map_err(|e| format!("{path}: {e}"))?;
        let map = barrel_ryza::event::schedule(&event)?.iter().find_map(|t| match t.step { barrel_engine::event::Step::FieldChange { map, .. } => Some(map), _ => None }).ok_or("the opening changes no field")?;
        let exe = barrel_files::read(Path::new(&a.install).join("Atelier_Ryza_DX.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
        a.field = barrel_ryza::maps::field_maps(&pe).get(map as usize).cloned().ok_or(format!("field map {map}: not named"))?;
        eprintln!("opening: field map {map}: {}", a.field);
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
    // STAND-IN: Every placed model casts the sun's shadow; the real thing is the game's own caster choice.
    // Ryza's placements carry no tags (Escha marks its casters `xDS_`): every placed model
    // casts the sun's shadow, and plays its motion where it has one.
    // Each keeps its placement's draw class (near, middle, far, landmark), for the field's draw
    // distances.
    let place = |_: &barrel_formats::opl2::PlacedType, i: &barrel_formats::opl2::Instance| barrel_engine::field::Placement { casts: true, class: (i.word & 3) as u8, lods: true, moves: true, ..Default::default() };
    // The motions of the objects many fields place (the trees' leaves stirring), in one
    // container of the game's (`docs/formats.md`, "Shared field motions").
    let shared_bytes = install.shared_motions();
    let shared = shared_bytes.as_deref().map(Elixir::parse).transpose()?;
    let barrel_engine::field::Loaded { textures: pack, moving, boxes: object_boxes, .. } = barrel_engine::field::load(&files.area, &main, &objects, shared.as_ref(), &mut scene, 0, &place)?;
    let pack = G1t::parse(&pack)?;
    // The field's plants (`_grass.bin`, `_grass.g1t`: barrel_ryza::grass), when it has them.
    let grass = barrel_engine::field::named(&main, &format!("{}_grass.bin", files.area)).and_then(|b| barrel_ryza::grass::Grass::parse(b).ok());
    let grass_pack = barrel_engine::field::named(&main, &format!("{}_grass.g1t", files.area)).and_then(|b| G1t::parse(b).ok());
    // The field's water (`<FIELD>_river.bin`): the pieces of its rivers and sea, in render space.
    let water = barrel_engine::field::named(&main, &format!("{}_river.bin", files.area)).and_then(|b| barrel_ryza::river::parse(b).ok()).map(|bodies| water_slots(&bodies));
    let plant_preset = presets.grass;
    // The title's backdrop: its event's stage run to the first frame's moment, and the characters
    // it stages (`actors`: Ryza, Lent and Tao), loaded before the textures that borrow them.
    let live_event = if a.title { Some(actors::TITLE_EVENT) } else { a.opening.then_some(barrel_ryza::opening::EVENT) };
    let title_event = match live_event {
        Some(name) => Some(barrel_formats::ebd::Ebd::parse(&install.read(&barrel_ryza::event::path(name).ok_or("event name")?)?)?),
        None => None,
    };
    // The event loops: a random wait, then one of three actions drawn each time. BARREL_EVENT_DRAWS
    // (numbers with commas between) gives the first draws in the order the event asks, for
    // playing what a run of the game drew; after them the clock draws as the game's does. An
    // hour of it is laid out.
    // The opening's lines (the title's event has none).
    let opening_messages = if a.opening { barrel_ryza::event::messages(&install, barrel_ryza::opening::EVENT)? } else { Vec::new() };
    // The event that follows it (the help pages) and its lines.
    let (following_event, following_messages) = if a.opening {
        let e = barrel_formats::ebd::Ebd::parse(&install.read(&barrel_ryza::event::path(barrel_ryza::opening::FOLLOWING).ok_or("event name")?)?)?;
        (Some(e), barrel_ryza::event::messages(&install, barrel_ryza::opening::FOLLOWING)?)
    } else {
        (None, Vec::new())
    };
    // The opening's lines wait for the button: scripted (`BARREL_EVENT_PRESSES`), or in a window the player's own, the schedule laid out
    // again at each press. Offscreen without a script a line takes no time.
    let scripted_presses = if a.opening { barrel_ryza::event::buttons_from_env() } else { None };
    let live_presses = a.opening && a.window && scripted_presses.is_none();
    let given: Vec<f32> = std::env::var("BARREL_EVENT_DRAWS").ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default();
    let event_clock = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
    // The event's schedule for some buttons: its steps, the commands taken and when the help event starts.
    let lay = |buttons: Option<barrel_engine::event::Buttons>| -> Result<Option<(Vec<barrel_engine::event::Timed<'_>>, Vec<barrel_ryza::event::Taken>, f32)>> {
        let Some(e) = &title_event else { return Ok(None) };
        Ok(Some(match &following_event {
            Some(next) => barrel_ryza::opening::schedule(&install, e, next, [&opening_messages, &following_messages], buttons, &given, event_clock, 3600.0)?,
            None => {
                let (steps, taken) = barrel_ryza::event::schedule_drawn_taken_with(e, &given, event_clock, 3600.0, None, barrel_ryza::event::Talk { messages: &opening_messages, buttons, fade_late: 0.0, terrain: None })?;
                (steps, taken, f32::INFINITY)
            }
        }))
    };
    let mut presses: Vec<f32> = Vec::new();
    let first_buttons = if live_presses { Some(barrel_ryza::event::buttons_at(&[], barrel_ryza::opening::START)) } else { scripted_presses };
    let laid = lay(first_buttons.clone())?;
    // What does not depend on the presses (who is staged, the cards, the pictures) comes from the event with no waits, as the player has not pressed yet.
    let whole = if live_presses { lay(None)? } else { None };
    let (title_steps, title_taken, help_from) = laid.map_or((None, Vec::new(), f32::INFINITY), |(s, t, h)| (Some(s), t, h));
    let (whole_steps, whole_taken) = match &whole {
        Some((s, t, _)) => (Some(s), t),
        None => (title_steps.as_ref(), &title_taken),
    };
    let title_motions = whole_steps.map(|s| actors::motions_of(s));
    let title_heads = actors::head_turns(whole_taken);
    // What the opening's commands switch for the talk: the letterbox and the button hints.
    let mut opening_cues = barrel_ryza::talk::Cues::from_taken(&title_taken).help_from(help_from);
    // The opening is over when its schedule is whole (a line still waiting for its button leaves the rest of the commands out) and its last wait has run.
    let whole_len = whole_taken.len();
    let over_of = |taken: &[barrel_ryza::event::Taken]| (taken.len() == whole_len).then(|| barrel_ryza::opening::over_at(taken));
    let mut opening_over: Option<f32> = if a.opening { over_of(&title_taken) } else { None };
    let opening_pictures = barrel_ryza::talk::Cues::from_taken(whole_taken).picture_ids();
    // The telops the opening shows (`u65`'s numbers), for the 2D layer built below.
    let opening_cards: std::collections::BTreeSet<u32> = match (whole_steps, a.opening) {
        (Some(steps), true) => steps.iter().filter_map(|t| match t.step { barrel_engine::event::Step::Telop { card, .. } => Some(card), _ => None }).collect(),
        _ => Default::default(),
    };
    // Everyone the opening ever stages (its stage run through once): their models are loaded up front.
    let opening_slots: std::collections::BTreeSet<u32> = match (whole_steps, a.opening) {
        (Some(steps), true) => {
            let mut scratch = barrel_engine::stage::Playback::new(barrel_engine::stage::Stage::default(), steps.clone());
            let mut seen = std::collections::BTreeSet::new();
            while !scratch.finished() {
                scratch.tick();
                seen.extend(scratch.stage.characters.keys().copied());
            }
            seen
        }
        _ => Default::default(),
    };
    let mut title_play = title_steps.map(|s| barrel_engine::stage::Playback::new(barrel_engine::stage::Stage { tutorial_clock: Some(barrel_ryza::talk::TUTORIAL_CLOCK), ..Default::default() }, s));
    let added = title_event.as_ref().map(barrel_ryza::event::added).transpose()?.unwrap_or_default();
    let mut cast_files: Vec<actors::Files> = Vec::new();
    if let (Some(motions), Some(play)) = (title_motions, title_play.as_mut()) {
        // after the boot the title's own time starts where the boot ends
        let boot_ahead = if a.boot { a.boot_at - title_start } else { 0.0 };
        actors::advance(play, if a.opening { barrel_ryza::opening::stage_seconds(a.opening_at) } else { a.title_at + boot_ahead - actors::EVENT_START });
        // Before the event has placed anyone (the boot's logos, the title's first frames) the
        // stage is run on to where it has, so its people are loaded and in the scene by the
        // time the title shows; the clock then catches the stage up (it never runs back).
        for _ in 0..PLACING_TICKS {
            if a.opening || !play.stage.characters.is_empty() {
                break;
            }
            play.tick();
        }
        // the people the stage has placed, then the props only the event's `q5` adds
        let slots: std::collections::BTreeSet<u32> = play.stage.characters.keys().chain(added.keys()).chain(opening_slots.iter()).copied().collect();
        for slot in slots {
            match actors::Files::read(&install, slot, motions.get(&slot).unwrap_or(&Default::default()), added.get(&slot)) {
                Ok(files) => cast_files.extend(files),
                // the opening's cast is wider than the title's: a character that cannot be read is left out
                Err(e) if a.opening => eprintln!("slot {slot}: {e}"),
                Err(e) => return Err(e),
            }
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
            if a.window || a.walk_from.is_some() {
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
    // The minimap's people are the ones out at the hour of the save's clock (the clock's own periods), not the scene's `--time`.
    let hud_markers = match (a.title, a.save_hour) {
        (true, _) => Default::default(),
        (false, hour) => {
            let time = hour.map_or(a.time, |h| match h {
                5..=10 => Time::Morning,
                11..=16 => Time::Noon,
                17..=19 => Time::Evening,
                _ => Time::Night,
            });
            minimap_markers(&install, &a.field, &a.story, &standing_npcs(&install, &a.field, time, a.weather, &a.story))
        }
    };
    let ramp_bytes = match ryza_files.is_some() || a.title || !townsfolk.is_empty() || !gimmicks.is_empty() {
        true => Some(install.read("\\data\\x64\\shader\\texture\\chara_rmp.g1t")?),
        false => None,
    };
    let ramp_pack = ramp_bytes.as_deref().map(G1t::parse).transpose()?;
    // Her draws' ramp rows by (first index, instance): each submesh's `rampIndex`.
    let mut ramp_rows: std::collections::HashMap<(u32, u32), u32> = Default::default();
    // The saturation after the gamma, from the light sets' table.
    let tone_vivid = install.read(barrel_ryza::environment::SATURATION_PATH).map_err(|e| e.to_string()).and_then(|b| barrel_ryza::environment::saturation_ex(&b, barrel_ryza::environment::SATURATION_SET, a.time, a.weather)).map_err(|e| eprintln!("{e}")).unwrap_or(0.0);
    eprintln!("saturation after the gamma: {tone_vivid}");
    // The sun flare's settings, from the environment template's row for the time and weather.
    let flare_look = install.read(barrel_ryza::flare::TEMPLATE_PATH).map_err(|e| e.to_string()).and_then(|b| barrel_ryza::flare::look(&b, a.time, a.weather)).map_err(|e| eprintln!("{e}")).unwrap_or_default();
    // Her model and mesh outlive the loop: her cloth's vertices are built from the mesh each frame.
    let ryza_model = ryza_files.as_ref().map(|(m, _, _)| Elixir::parse(m)).transpose()?;
    let mut ryza_mesh = match &ryza_model {
        Some(model) => Some(G1m::parse(barrel_engine::field::named(model, "PC00A_MODEL_default.g1m").ok_or("no PC00A_MODEL_default.g1m")?)?),
        None => None,
    };
    let mut rig: Option<Rig> = None;
    if let (Some((at, turn, motion, time)), Some((_, _, motions)), Some(rp), Some(model), Some(mesh)) = (&a.ryza, &ryza_files, &ryza_pack, &ryza_model, ryza_mesh.as_mut()) {
        let skeleton = G1m::parse(barrel_engine::field::named(model, "PC00A_MODEL.g1m").ok_or("no PC00A_MODEL.g1m")?)?.skeleton.ok_or("Ryza's model has no skeleton")?;
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
        let barrel_ryza::walk::Terrain { ground, walls, bodies, start: at } = barrel_ryza::walk::load_terrain(&install, &a.field, &files.area, &main, &objects, *at, &gimmicks)?;
        let at = &at;
        eprintln!("Ryza stands at {at:?}");
        // Her settings from the character tables (`barrel_ryza::chara`): her model's scale, her
        // size against the walls, the camera's look height.
        let chara = barrel_ryza::chara::Chara::read(&install.read("\\saves\\chara\\chara.xml")?, &install.read("\\saves\\chara\\charabase.xml")?, "PC00A")?;
        eprintln!("Ryza: {chara:?}");
        let instance = scene.instance(Mat4::from_scale_rotation_translation(Vec3::from(chara.size), glam::Quat::from_rotation_y(turn.to_radians()), *at));
        // Her pack follows the scene's textures, the toon ramp her pack.
        let drawn = actors::add_character(&mut scene, geometry, rp.textures.len(), &joints, instance, textures.len(), textures.len() + rp.textures.len(), "ryza", &[], Some(&mut ramp_rows))?;
        actors::cast_in_sub_map(&mut scene.draws, drawn, geometry, &SUB_BIAS_EX_SHADERS);
        eprintln!("Ryza: {drawn} draws, {} joints, motion {motion} {:.2}s long", joints.len(), anim.duration);
        let lengths: Vec<(u32, f32)> = anims.iter().map(|(n, a)| (*n, a.duration)).collect();
        let mut player = barrel_ryza::player::Player::new(*at, turn.to_radians(), &lengths);
        (player.radius, player.height) = (chara.radius, chara.height);
        // A walked offscreen run starts mid-idle: the motion time given is how far into it she stands (`--motion-time`).
        if a.walk_from.is_some() {
            player.clock = *time;
        }
        // Her cloth (hair, skirt): the pieces' solvers, fed her pose each frame.
        let cloth = cloth::ClothDriver::new(geometry, &skeleton, &scene.cloth_slots, instance, chara.local_gravity);
        rig = Some(Rig { skeleton, anims, instance, ground, walls, bodies, player, chara, face, shut, cloth, joints: joints.clone() });
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
            // (Their packs' textures are added after the loop, so that the list does not keep
            // the actors borrowed mutably: `extra` is what the loop's own adds would be.)
            let (mut extra, mut placed) = (0, Vec::new());
            for (k, actor) in cast.iter_mut().enumerate() {
                let Some(play) = &title_play else { continue };
                // a prop has no body on the stage: the event's `q5` places it for good
                let body = play.stage.characters.get(&actor.slot);
                if body.is_none() && !actor.is_prop() && !a.opening {
                    continue;
                }
                let joints = actor.joints(body, play.stage.time);
                actor.instance = scene.instance(actor.model(body));
                let name = format!("slot {}", actor.slot);
                let drawn = actors::add_character(&mut scene, &actor.geometry, actor.pack.textures.len(), &joints, actor.instance, textures.len() + extra, ramp, &name, &actor.swing, Some(&mut ramp_rows))?;
                eprintln!("{name}: {drawn} draws, {:?} at stage {:.3} s", body.map(|b| (b.position, b.facing.to_degrees(), &b.motion)), play.stage.time);
                extra += actor.pack.textures.len();
                placed.push(k);
            }
            for k in placed {
                textures.extend(cast[k].pack.textures.iter());
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
                actors::add_character(&mut scene, &actor.geometry, actor.pack.textures.len(), &joints, instance, base, ramp, tag, &actor.swing, Some(&mut ramp_rows))?;
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
                actors::add_character(&mut scene, &actor.geometry, actor.pack.textures.len(), &joints, instance, base, ramp, file.tag(), &actor.swing, None)?;
                gimmick_instances.push(instance);
            }
        }
        eprintln!("gimmicks: {} drawn of {} here ({})", gimmick_instances.len(), gimmicks.len(), gimmick_files.iter().map(|f| f.tag()).collect::<Vec<_>>().join(", "));
    }
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
            d.normal_map = d.normal_map.map(|m| m + textures.len());
            d.overlay = d.overlay.map(|o| o + textures.len());
            d.specular = d.specular.map(|m| m + textures.len());
            d.field = Some(0);
            d.sky = true;
            d.casts = false;
            d.name = "sky dome".into();
        }
        eprintln!("sky dome {}: {} draws", dome_files.as_ref().unwrap().0, scene.draws.len() - first);
        textures.extend(dp.textures.iter());
    }
    let mut plant_boxes = std::collections::HashMap::new();
    let mut plant_field: Option<PlantField> = None;
    if let (Some(g), Some(gp)) = (&grass, &grass_pack) {
        if let Some(atlas) = gp.textures.first() {
            // The cards' normal maps (the pack's third texture), for `NORMAL_MAPPED`.
            let normals = gp.textures.get(2).map(|_| textures.len() + 1);
            (plant_field, plant_boxes) = add_plants(&mut scene, g, &main, &files.area, textures.len(), normals, plant_preset.density).map_or((None, Default::default()), |(f, b)| (Some(f), b));
            textures.push(atlas);
            textures.extend(gp.textures.get(2));
        }
    }
    let Scene { mut vertices, indices, mut draws, terrains, lo, hi, mut instances, mut palette, deltas, morph_weights, .. } = scene;
    // The models' Color0 does not reach their materials: the G-buffer programs (`pbb-*_gb.g1s`,
    // docs/shading.md) take no COLOR0, and some models hold masks there (the attic's
    // cabinet). So white; the terrain's (its baked shade) is kept.
    // (A character's alpha stays: the game's forward pass takes its hull's thickness from it.)
    for d in &draws {
        let alpha = ramp_rows.contains_key(&(d.first_index, d.instance));
        for &i in &indices[d.first_index as usize..(d.first_index + d.index_count) as usize] {
            let v = &mut vertices[i as usize];
            v.color = [255, 255, 255, if alpha { v.color[3] } else { 255 }];
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
            Some(&[(lo, hi), _]) => Some(Bounds { lo, hi, pad: f32::INFINITY, round: false }),
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
    let (start_w, start_h) = (width, height);
    let display_of = |config: Option<&str>| {
        let display = match config {
            Some(text) => settings::Display::from_config(text),
            None => settings::Display { width: start_w, height: start_h, fullscreen: std::env::var_os("BARREL_DEFAULT_FULLSCREEN").is_some_and(|v| v == "1") },
        };
        settings::Display { fullscreen: display.fullscreen || a.fullscreen, ..display }
    };
    // The game's own default (60 frames, vsync on: the rows' defaults) where the config says nothing.
    let pacing_of = |config: Option<&str>| barrel_ryza::options::GAME_PACING.over(config.unwrap_or(""));
    let mut display = display_of(config.as_deref());
    let mut pacing = pacing_of(config.as_deref());
    if a.window {
        eprintln!("graphics: {graphics:?}, {display:?}");
    }
    // The window to play in: the one the previous scene kept (with its device and swapchain),
    // else the one the locate screen opened (a process has one event loop), else a new one.
    let (mut pump, gpu, mut presenter) = match app {
        Some(App { pump, gpu, presenter }) => (Some(pump), gpu, Some(presenter)),
        None => {
            let pump = match kept_pump.take() {
                Some(mut p) if a.window => {
                    p.reopen(display)?;
                    Some(p)
                }
                _ => a.window.then(|| window::Pump::open(display)).transpose()?,
            };
            let raw_display = pump.as_ref().map(|p| raw_window_handle::HasDisplayHandle::display_handle(p.window()).map(|h| h.as_raw())).transpose()?;
            let gpu = Gpu::new(raw_display)?;
            let presenter = match (&pump, raw_display) {
                (Some(p), Some(raw_display)) => {
                    let handle = raw_window_handle::HasWindowHandle::window_handle(p.window())?.as_raw();
                    let size = p.window().inner_size();
                    let refresh = p.window().current_monitor().and_then(|m| m.refresh_rate_millihertz()).map(|mhz| mhz as f64 / 1000.0);
                    Some(present::Presenter::new(&gpu, raw_display, handle, size.width, size.height, refresh)?)
                }
                _ => None,
            };
            (pump, gpu, presenter)
        }
    };
    // Vertical sync and the frame rate limit (a window kept from the last scene gets the config's again).
    if let Some(p) = presenter.as_mut() {
        p.set_pacing(&gpu, pacing)?;
    }
    if let Some(p) = &presenter {
        (width, height) = (p.extent.width, p.extent.height);
        eprintln!("window: {width}x{height}");
    }
    let mut renderer = Renderer::new(&gpu, graphics, presenter.as_ref(), width, height, a.timings, false)?;
    renderer.fixed_cloud_map = cloud_map_override()?;
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
    // (The whole ones are kept to upload them again at another bias: views of the packs'
    // bytes, which stay.)
    let unbiased: Vec<&barrel_formats::g1t::Texture> = textures.iter().copied().chain(composite_textures.iter()).collect();
    let biased: Vec<barrel_formats::g1t::Texture> = unbiased.iter().map(|t| t.without_top_levels(presets.texture_bias)).collect();
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
    let (uploaded, opaque, outline_tex, white) = upload_textures(&gpu, renderer.pool, &textures, None, &linear)?;
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
    // Her cloth settled on her first pose, before the scene goes up.
    if let (Some(r), Some(mesh)) = (rig.as_mut(), ryza_mesh.as_ref().and_then(|m| m.geometry.as_ref()))
        && let Some(cloth) = r.cloth.as_mut()
    {
        let wind = cloth::world_wind(gust, r.chara.cloth_wind_scale, 0.0);
        cloth.update(mesh, &r.joints, cloth::model_matrix(r.chara.size, r.player.turn, r.player.at), wind, 0.0, &mut palette);
    }
    let data = scene_gpu::SceneData { vertices: vbytes, indices: ibytes, textures: uploaded, opaque, instances: &instances, palette: &palette, deltas: &deltas, weights: &morph_weights, grounds, patterns: vec![cloud_texture], specular_cubes: vec![specular_cube], white };
    let mut scene = scene_gpu::SceneGpu::new(&gpu, data, &renderer.shared)?;

    // The camera: on the given point (or on Ryza in a window), framing the ground (render
    // space mirrors x: the terrain spans x -extent..0, z 0..extent), or everything when there
    // is no terrain.
    let ground = terrains.first().map(|(_, t)| t.extent);
    let static_box = match ground {
        Some([ex, ez]) => ((Vec3::new(-ex, 0.0, 0.0), Vec3::new(0.0, STATIC_TOP, ez)), true),
        None => ((lo, hi), false),
    };
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
    let mut fov = a.fov.unwrap_or(if a.title { barrel_ryza::title::CAMERA.fov } else { follow.map_or(30.0, |c| c.fov) }).to_radians();
    // How high above her feet the camera looks (`LOOK_HEIGHT`).
    let mut look_height = barrel_ryza::walk::LOOK_HEIGHT;
    let (mut yaw, mut pitch) = match (follow, &rig) {
        (Some(c), Some(r)) => (a.camera_yaw.map_or(c.yaw - r.player.turn.to_degrees(), |g| 180.0 - g), a.camera_pitch.unwrap_or(c.pitch)),
        _ => (a.yaw, a.pitch),
    };
    let mut dist = a.distance.or(follow.map(|c| c.length)).unwrap_or(radius / (fov * 0.5).sin() * 1.05).max(1.0);
    let (pitch_range, length_range) = follow.map_or(((-10.0, 70.0), (150.0, 3000.0)), |c| (c.pitch_range, c.length_range));
    // The walk's camera (`barrel_ryza::walk`), in the game's yaw (the view's is 180 minus it);
    // the locals above are what it gives the view each frame.
    // (A walked offscreen run with `--look-at` is steered by the capture's camera yaw, whatever angle the picture is taken from.)
    let walk_yaw = if a.walk_from.is_some() && a.look_at.is_some() { WALK_CAMERA_YAW } else { 180.0 - yaw };
    let mut cam = barrel_ryza::walk::Camera::new(walk_yaw, pitch, dist, pitch_range);
    cam.feel = play.feel;
    if follow.is_some() && rig.is_some() {
        cam = cam.following();
    }
    let mut center = match (follow, &rig) {
        (Some(_), Some(_)) if a.camera_target.is_some() => Vec3::from(a.camera_target.unwrap_or_default()) * Vec3::new(-1.0, 1.0, 1.0),
        (Some(_), Some(r)) => (r.player.at + Vec3::new(0.0, look_height, 0.0)) * Vec3::new(-1.0, 1.0, 1.0),
        _ => center,
    };
    // The title's camera is fixed (`title::CAMERA`).
    let title_clip = a.title.then_some((barrel_ryza::title::CAMERA.near, barrel_ryza::title::CAMERA.far));
    if a.title {
        (center, yaw, pitch, dist) = barrel_ryza::title::CAMERA.look();
    }
    let mut shadow_size = (!a.no_shadows).then_some(renderer.shadow_target.size);

    let hidden = vec![false; instances.len()];
    // The field's draw distances (`camera::DrawRanges`): a field object farther from the eye
    // than its class's range is left out. Field objects stay where they were placed, so each
    // draw's box is found once (render space, as the eye).
    // The PC game uses them only below the top object-distance setting, or when the field's
    // file asks (docs/renderer.md, "Ryza DX: object draw ranges"); the setting's near, middle
    // and far ranges then stand in place of the field's.
    let ranges_for = |presets: &barrel_ryza::presets::Graphics| -> Result<Option<barrel_ryza::camera::DrawRanges>> {
        Ok(match (a.draw_ranges.as_str(), install.read(&barrel_ryza::camera::FreeCamera::path(&a.field))) {
            ("off", _) | (_, Err(_)) => None,
            ("auto", Ok(bytes)) if !presets.ranged && !barrel_ryza::camera::DrawRanges::asked_for(&bytes) => None,
            (which, Ok(bytes)) => Some(barrel_ryza::camera::DrawRanges::parse(&bytes, which == "nx")?.preset(presets.objects)),
        })
    };
    // The presets as they stand now: a live change of the settings (below, where the frame
    // starts) replaces them, and the draw tests below read them each frame.
    let presets_now = std::cell::Cell::new(presets);
    let plant_boxes = std::cell::RefCell::new(plant_boxes);
    let ranges = std::cell::Cell::new(ranges_for(&presets)?);
    // STAND-IN: The townsfolk and gimmicks have no draw range; the real thing is the field file's `chara` and `gimmick` ranges, which like the objects' count only when ranges are in use.
    // The plants have their own range (`shown`, below), not an object's.
    let boxes: std::collections::HashMap<(u32, u32), (Vec3, Vec3)> = draws.iter().filter(|d| d.field.is_some() && !d.sky && d.name != "plants" && !npc_drawn.iter().any(|&(_, i, ..)| i == d.instance) && !gimmick_instances.contains(&d.instance)).filter_map(|d| {
        let b = d.bounds?;
        let m = instances.get(d.instance as usize)?.model;
        let corners = (0..8).map(|k| m.transform_point3(Vec3::new(if k & 1 == 0 { b.lo.x } else { b.hi.x }, if k & 2 == 0 { b.lo.y } else { b.hi.y }, if k & 4 == 0 { b.lo.z } else { b.hi.z })));
        let (lo, hi) = corners.fold((Vec3::MAX, Vec3::MIN), |(l, h), c| (l.min(c), h.max(c)));
        Some(((d.first_index, d.instance), (lo, hi)))
    }).collect();
    eprintln!("draw ranges: {:?}", ranges.get());
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
        if a.unseen.iter().any(|h| d.name.contains(h.as_str())) {
            return false;
        }
        let Some(h) = &horizon else { return true };
        // STAND-IN: A townsperson is tested as a sphere of PERSON_SPHERE.1 units around a point PERSON_SPHERE.0 above the feet; the real thing is the game's own bounds for a character.
        if let Some(&feet) = npc_spots.borrow().get(&d.instance) {
            return !h.borrow().hides(feet + Vec3::Y * PERSON_SPHERE.0, PERSON_SPHERE.1);
        }
        // The boxes are in the renderer's space, the skyline in the field's: x is mirrored.
        !horizon.as_ref().zip(boxes.get(&(d.first_index, d.instance))).is_some_and(|(h, &(lo, hi))| h.borrow().hides((lo + hi) * Vec3::new(-0.5, 0.5, 0.5), lo.distance(hi) * 0.5))
    };
    // The field's occluders (`<field>_le.koc`, `barrel_ryza::occluders`) and each placed
    // object's oriented box, both in render space (x mirrored); the kept quads are found
    // each frame from the eye.
    let mirror = Vec3::new(-1.0, 1.0, 1.0);
    let occluder_quads: Vec<barrel_ryza::occluders::Corners> = if a.title || a.no_occluders {
        Vec::new()
    } else {
        install.read(&format!("\\data\\x64\\field\\{0}\\{0}_le.koc", a.field.to_ascii_lowercase())).ok().and_then(|b| barrel_ryza::koc::parse(&b).ok()).unwrap_or_default().iter().map(|q| q.map(|c| Vec3::from(c) * mirror)).collect()
    };
    let occluded_boxes: std::collections::HashMap<u32, barrel_ryza::occluders::OrientedBox> =
        object_boxes.iter().map(|b| (b.object, barrel_ryza::occluders::OrientedBox { center: b.center * mirror, axes: b.axes.map(|v| v * mirror) })).collect();
    let occluders = std::cell::RefCell::new(barrel_ryza::occluders::Occluders::default());
    let eye_now = std::cell::Cell::new(Vec3::ZERO);
    let forward_now = std::cell::Cell::new(Vec3::ZERO);
    let sides_now = std::cell::Cell::new([glam::Vec4::ZERO; 4]);
    // The boot sequence's screens come before the scene exists: it is left out until the title.
    let scene_on = std::cell::Cell::new(true);
    // `shown`, for a view: whether the scene is on, its eye, forward axis and four sides.
    let shown_from = |d: &Draw, on: bool, eye: Vec3, forward: Vec3, sides: [glam::Vec4; 4]| {
        on
            && !a.hide.iter().any(|h| d.name.contains(h.as_str()))
            // A plant cell is drawn unless its test box (`add_plants`) is wholly outside one of
            // the view's four sides or past the grass range's end, as the game's plants' update
            // tests it against its six planes (`0x140a78dd0`; docs/renderer.md, "Ryza DX: which
            // plant cells are drawn").
            && plant_boxes.borrow().get(&d.first_index).filter(|_| d.name == "plants").copied().is_none_or(|[_, (lo, hi)]| {
                let corners = (0..8).map(|k| Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }));
                let depth = |c: Vec3| forward.dot(c - eye);
                corners.clone().any(|c| depth(c) <= presets_now.get().grass.far[1]) && sides.iter().all(|p| corners.clone().any(|c| p.truncate().dot(c) + p.w >= 0.0))
            })
            // A placed object wholly behind one of the field's occluder quads is left out.
            && (d.object == 0 || occluded_boxes.get(&d.object).is_none_or(|b| !occluders.borrow().hides(b, eye, forward)))
            && match (ranges.get(), boxes.get(&(d.first_index, d.instance))) {
                (Some(r), Some(&(lo, hi))) => r.shows(d.class, eye, lo, hi),
                _ => true,
            }
            // An object's level of detail by its distance from the eye (the game's LOD
            // distances, `presets.lod`): its full model nearer than the first, its middle one to
            // the second, its low one past it; a draw without a box, and every draw at the top setting,
            // the full one.
            && (d.lod == 0
                || boxes.get(&(d.first_index, d.instance)).filter(|_| presets_now.get().lods).map_or(d.lod & 3 == LOD_HI, |&(lo, hi)| {
                    let far = ((lo + hi) * 0.5).distance(eye);
                    let level = d.lod & 3;
                    let start = match level {
                        LOD_HI => 0.0,
                        LOD_MID => presets_now.get().lod[0],
                        _ => presets_now.get().lod[1],
                    };
                    let end = if d.lod & LOD_LAST != 0 { f32::INFINITY } else if level == LOD_HI { presets_now.get().lod[0] } else { presets_now.get().lod[1] };
                    far >= start && far < end
                }))
    };
    let shown = |d: &Draw| shown_from(d, scene_on.get(), eye_now.get(), forward_now.get(), sides_now.get());
    // The static shadow layer is drawn once, from the view of the frame it is first asked for
    // (`Renderer::far_shadow`); drawn again for new settings (`BARREL`'s live apply) it takes
    // the casters that view has, as a cold start's does.
    let mut far_replay = false;
    let far_view = std::cell::Cell::new(None::<(bool, Vec3, Vec3, [glam::Vec4; 4])>);
    let far_shown = |d: &Draw| match far_view.get() {
        Some((on, eye, forward, sides)) => shown_from(d, on, eye, forward, sides),
        None => shown(d),
    };
    // Lit as the field's models: the environment's light on their vertex colour (no sun yet).
    // Her draws take the toon ramp (flag 128, the row in bits 16-23).
    // The dome's clock, set each frame: its layers' textures slide by it (`dome_shift`).
    let dome_clock = std::cell::Cell::new(0.0f32);
    let material = |d: &Draw| {
        // The sky dome's clouds lit by the set's light for them (flag 4096, look
        // slots `DOME_LIGHT_SLOT`, `DOME_SUN_SLOT`), as the game's `PBSkyDome` does.
        let base = if environment.is_some() {
            if d.sky {
                DOME_LIT | if d.normal_map.is_some() { NORMAL_MAPPED } else { 0 } | if d.specular.is_some() { SPECULAR_AMBIENT } else { 0 }
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
        let base = if spec_on && d.specular.is_some() && !d.sky && d.name != "plants" { base | SPECULAR_AMBIENT } else { base };
        match ramp_rows.get(&(d.first_index, d.instance)) {
            Some(&row) if ramp_pack.is_some() && environment.is_some() => (base | 128 | row << 16, 0),
            // A dome's layer carries how far its texture has slid in the tint's place.
            _ if base & DOME_LIT != 0 => (base, dome_shift(d.scroll, dome_clock.get()).to_bits()),
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
    // The opening's telops and fade (`--opening`): one layer of the strips' atlases.
    let mut opening_layer = match a.opening {
        true => Some(opening_telops(&install, &gpu, &renderer, &opening_cards, &opening_pictures, (width, height))?),
        false => None,
    };
    // The title screen (`--title`): its glyphs from the executable, its layout from the archives.
    let mut title_layer = match a.title {
        true => Some(title_layer(&install, &gpu, &renderer, a.boot)?),
        false => None,
    };
    let bytes = u64::from(width) * u64::from(height) * 4;
    let readback = match &pump {
        Some(_) => None,
        None => Some(gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?),
    };
    // `BARREL_WINDOW_SHOT=N:FILE.png[,M:FILE...]`: in a window, the picture at frame N written to FILE (what an
    // offscreen run cannot show: the swapchain, a resize, live settings over real frame times).
    let window_shots: Vec<(u32, String)> = std::env::var("BARREL_WINDOW_SHOT")
        .ok()
        .filter(|_| pump.is_some())
        .map(|v| v.split(',').filter_map(|e| e.split_once(':')).filter_map(|(n, f)| Some((n.trim().parse().ok()?, f.to_string()))).collect())
        .unwrap_or_default();
    let mut sync = ring::Frames::new(&gpu)?;
    let mut prev_vp = None;
    // The temporal upscalers' and TAA's sub-pixel jitter: the frames drawn so far with it.
    let mut jitter_index = 0u32;
    let mut prev_drawn_vp: Option<Mat4> = None;
    let mut last = std::time::Instant::now();
    let started = last;
    let mut help_bar = if a.no_banner { barrel_ryza::field_hud::HelpBar::arriving_at_once() } else { barrel_ryza::field_hud::HelpBar::arriving() };
    let (mut stale, mut frame_no) = (false, 0u32);
    // The keys pressed this frame, for the title's menu, and what the player has done there.
    let mut title_keys: Vec<winit::keyboard::KeyCode> = Vec::new();
    let mut pad_any = false;
    // The confirm button went down this frame (a window's keyboard or pad).
    let mut confirm_pressed = false;
    // X (the pad's west button) went down: the staff's swing.
    let mut swing_pressed = false;
    let mut title_input = a.title_input.clone();
    title_input.saves |= !save_slots.is_empty();
    // The field's gathering points, and with `--gather` the gathering the HUD replays.
    let gather_points = match (a.title || a.boot, rig.is_some()) {
        (false, true) => barrel_files::read(install.root.join("Atelier_Ryza_DX.exe")).ok().and_then(|exe| gathering::Points::read(&install, &exe, &a.field, &a.story)),
        _ => None,
    };
    for &item in &a.learned {
        barrel_ryza::gather::know(&mut know_items, item);
    }
    let mut gather_rng = gathering::generator();
    let gather_plan = match (a.gather.map(|f| (f, gathering::Tool::Hand)).or(a.swing.map(|f| (f, gathering::Tool::Staff))), &gather_points, &rig) {
        (Some((frames, tool)), Some(points), Some(r)) => points.plan(r.player.at.into(), r.player.turn.to_degrees(), frames, tool, &a.spent, &mut gather_rng, &mut know_items, &played_events),
        _ => None,
    };
    if let Some(plan) = &gather_plan {
        for g in &plan.gave {
            eprintln!("gathered: item {} quality {} potentials {:?}{}", g.item, g.quality, g.potentials, if g.new { " (new discovery)" } else { "" });
        }
    }
    // The field's HUD, over a field she walks (a save puts her there; `--hud` asks for it).
    let mut hud = match (a.hud || a.save.is_some()) && !a.title && !a.boot && rig.is_some() {
        true => Some(hud_layer(&install, &gpu, &renderer, gather_plan.as_ref().map_or(&[][..], |g| &g.images))?),
        false => None,
    };
    // The field's menu (Y): its layer where there is a field HUD to leave, and the blurred picture of it the Settings
    // screen is drawn over (made right after it and before the screen's: the layers draw, and the draw trace lists them, in
    // the order they were made).
    let mut field_menu = match hud.is_some() {
        true => Some(menu_layer(&install, &gpu, &renderer)?),
        false => None,
    };
    let mut backdrop = match hud.is_some() {
        true => Some(backdrop_layer(&install, &gpu, &renderer)?),
        false => None,
    };
    // The Equipment page, over the blurred picture of the menu (made after the backdrop: layers draw in the order they were made).
    let mut page = match (hud.is_some(), party_tables.as_ref()) {
        (true, Some(t)) if !menu_shown.party.is_empty() => Some(page_layer(&install, &gpu, &renderer, &menu_shown.party, t, &container.0.iter().chain(&tools).map(|u| barrel_ryza::field_page::ShelfItem { image: t.image(u.item), category: item_info.as_ref().and_then(|i| i.row(u.item)).and_then(|r| r.category) }).chain(item_info.as_ref().into_iter().flat_map(|i| (0..barrel_ryza::library::TABS).flat_map(|tab| library.cards(i, &know_items, tab, &|n| t.image(n)))).map(|c| barrel_ryza::field_page::ShelfItem { image: c.image, category: None })).collect::<Vec<_>>())?),
        _ => None,
    };
    let mut showing_page = false;
    // The Settings screen's own layer and what it shows (the config's values over the defaults).
    // The field's menu opens it too (where there is a field HUD to leave).
    let field_has_menu = (a.hud || a.save.is_some()) && !a.title && !a.boot && rig.is_some();
    let mut options_screen = match a.title || field_has_menu {
        true => Some(options_layer(&install, &gpu, &renderer)?),
        false => None,
    };
    // The load screen's own layer, with the views of its screens (made after the Settings screen's: its saving notice is drawn over that).
    let mut load_screen = match a.title || field_has_menu {
        true => Some(load_layer(&install, &gpu, &renderer)?),
        false => None,
    };
    let mut options_state = barrel_ryza::options::State::new(game_settings.clone(), render_before_presets);
    options_state.extended = !a.settings_as_game;
    // The title frame of the last tab change on Settings (the cursor's blink starts again).
    let mut tab_changed_at: Option<i32> = None;
    if let Some(c) = &config {
        options_state.load(c);
    }
    // The Options screen (the values, the dialog, the blink: `options_run::Screen`, as the field menu's run has it).
    let mut scr = barrel_ryza::options_run::Screen::new(options_state, 0);
    let mut apply_settings = false;
    // After a live display change: when the frames were counted from, and how many since.
    let mut display_probe: Option<(std::time::Instant, u32)> = None;
    // `BARREL_CONFIG_AT=N:FILE[,M:FILE...]` (with `BARREL_CONFIG` set): at frame N the config takes FILE's text and is
    // applied as a Yes on the Settings screen applies it: a run of live changes checked against a fresh start.
    let config_at: Vec<(u32, String)> = std::env::var("BARREL_CONFIG_AT")
        .ok()
        .filter(|_| std::env::var_os("BARREL_CONFIG").is_some())
        .map(|v| v.split(',').filter_map(|e| e.split_once(':')).filter_map(|(n, f)| Some((n.trim().parse().ok()?, f.to_string()))).collect())
        .unwrap_or_default();
    let reapply_at: Vec<u32> = std::env::var("BARREL_REAPPLY_AT").map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default();
    let mut scripted_saved = false;
    let mut scripted_tab_change: Option<i32> = None;
    let mut scripted_blink: Option<i32> = None;
    let mut was_in_settings = false;
    let mut scripted_holds: Vec<barrel_ryza::title_menu::Hold> = Vec::new();
    // The cursor's row when the holds were last cleared (a tab change clears them).
    let mut options_base = 0usize;
    let mut menu_input = barrel_ryza::menu_input::MenuInput::default();
    // The Settings tab and row last printed (`settings: tab N row M`).
    let mut shown_row: Option<(usize, usize)> = None;
    let mut showing_options = false;
    let mut showing_backdrop = false;
    // The menu's keys so far (the script's offscreen; a window adds the player's: Tab or Y opens it,
    // the arrows or the D-pad move, Enter or A chooses, Backspace, Escape or B closes) and whether it is up.
    let mut menu_keys = a.menu_keys.clone();
    let mut menu_up = false;
    // The Settings screen the menu opened, while it is up (see `SettingsRun`), and the keys' repeat clocks in a window.
    let mut settings_run: Option<SettingsRun> = None;
    let mut settings_repeat: [Option<f32>; 2] = [None; 2];
    let mut handoff: Option<std::path::PathBuf> = None;
    // New Game's button has led to the opening: the run goes on into it.
    let mut opening_next = false;
    // The opening (and the help event) is over: the run goes on into the free play.
    let mut free_play_next = false;
    let mut showing_load = false;
    // The Settings screen's "Saving..." notice is drawn over it (in the load screen's layer).
    let mut showing_notice = false;
    // The title's music and voice (on the device in a window; offscreen only the cue log).
    let sfx = || barrel_ryza::music::parse_table(&install.read("\\saves\\sound\\sfx.xml").unwrap_or_default());
    let mut sound = match carry.sound {
        Some(s) => Some(s),
        None if a.title => {
            let mut s = barrel_ryza::sound::Sound::new(&install.root, sfx(), pump.is_some());
            for cue in barrel_ryza::sound::title_cues(title_input.saves) {
                s.schedule(cue);
            }
            Some(s)
        }
        None if field_music.is_some() || a.opening || a.no_banner => Some(barrel_ryza::sound::Sound::new(&install.root, sfx(), pump.is_some())),
        None => None,
    };
    // The opening and the free play after it (offscreen: a run from `--opening-at`) count their sound frames as the game's.
    // A gathering replayed against a capture counts the sound's frames as the capture's.
    let cue_origin = if pump.is_none() && (a.opening || a.no_banner) { i64::from(a.opening_at) } else if pump.is_none() && gather_plan.is_some() { a.hud_at as i64 } else { 0 };
    let streams = if a.opening || a.no_banner { barrel_ryza::music::parse_table(&install.read("\\saves\\sound\\stream.xml").unwrap_or_default()) } else { Vec::new() };
    let mut closes_heard = 0u32;
    let effects = sfx();
    // The field's own music (Rasenboden by day), which the help event starts with when it takes over.
    let help_music = if a.opening { barrel_files::read(install.root.join("Atelier_Ryza_DX.exe")).ok().and_then(|exe| barrel_ryza::music::field_number(&exe, barrel_ryza::opening::FIELD_MAP as usize, false)) } else { None };
    let sound_of = |taken: &[barrel_ryza::event::Taken], help: f32| {
        let mut cmds = barrel_ryza::sound::commands(taken);
        if help.is_finite() {
            barrel_ryza::sound::event_change(&mut cmds, help, help_music);
        }
        cmds
    };
    let mut sound_cmds = sound_of(&title_taken, help_from);
    let mut sound_heard = carry.frame + cue_origin - 1;
    // The arrival after the opening: the field's music from its first frame and the marquee's sound as its text comes to rest.
    if let (Some(s), true) = (sound.as_mut(), a.no_banner && !a.title) {
        let exe = barrel_files::read(install.root.join("Atelier_Ryza_DX.exe"))?;
        let number = barrel_ryza::music::field_number(&exe, barrel_ryza::opening::FIELD_MAP as usize, barrel_ryza::music::is_night(a.save_hour.unwrap_or(10)));
        if let Some(hash) = number.and_then(|n| barrel_ryza::music::music_hash(&streams, n)) {
            s.schedule(barrel_ryza::sound::Cue::music(carry.frame + cue_origin, hash));
        }
        s.schedule(barrel_ryza::sound::Cue::effect(carry.frame + cue_origin + barrel_ryza::field_hud::MARQUEE_SOUND_AT, barrel_ryza::sound::MARQUEE));
    }
    // A replayed gathering's sounds: the reach and the pick-up, and the discovery's chime for an item the library did not know.
    if let (Some(s), Some(plan)) = (sound.as_mut(), gather_plan.as_ref()) {
        for cue in plan.cues(plan.press as i64) {
            s.schedule(cue);
        }
    }
    // The field's music is asked for once the load's stop is past (the frames count on from the title's).
    if let (Some(s), Some(hash), false) = (sound.as_mut(), field_music, a.title) {
        s.schedule(barrel_ryza::sound::Cue::music(carry.music_at, hash));
    }
    // The opening's own sound comes from its commands (below); a window carrying the title's music over has it stopped by the event's first one.
    if let Some(s) = sound.as_mut() {
        s.set_volumes(&play.volumes);
    }
    let (sound_base, mut handoff_at, mut music_at) = (carry.frame + cue_origin, 0i64, 0i64);
    if pump.is_some() && !a.title {
        eprintln!("field: entered");
    }
    // The camera's distance as drawn: pulled in by the ground behind her, easing back out.
    let (mut view_dist, mut frame_dt) = (None::<f32>, 0.0f32);
    // A walked offscreen run: the pad script (`BARREL_PAD_SCRIPT`) and where she began.
    let walk_script = a.walk_from.and_then(|_| barrel_render::pad::Script::load_env());
    // The gathering in a window: the prompt where a point is in reach, A gathers (the offscreen replay is `--gather`).
    let mut live_gather = gathering::Live::default();
    let mut gather_hud_items: Option<Vec<usize>> = None;
    // Whether the gathering holds the help bar's rest count (as of the last frame).
    let mut gather_busy = false;
    // The point just gathered from: its body stops being a wall.
    let mut gather_gone: Option<u32> = None;
    let walk_start = rig.as_ref().map_or(Vec3::ZERO, |r| r.player.at);
    loop {
        // Last frame's joints, then the field's moving objects at this frame's moment (the
        // game turns Rasenboden's windmills once in their motion's 12 s and stirs its trees'
        // leaves, all on one clock). That clock runs a frame ahead of the sky's: the motions
        // step before the first frame is drawn (`docs/ryza/open.md`, "What moves in the wind").
        prev_palette.copy_from_slice(&palette);
        let field_time = phase_override("BARREL_SKY_CLOCK").unwrap_or(if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 }) + 1.0 / 60.0;
        for m in &moving {
            m.pose(field_time, &instances, &mut palette);
        }
        for &(k, instance, motion, walk) in &npc_drawn {
            let actor = &npc_actors[k];
            // A walker is where its path has it by now, standing still through its pauses.
            let walking = walk.map(|w| w.at(field_time));
            let motion = if walking.is_some_and(|(.., moving)| !moving) { barrel_ryza::motion::STAND } else { motion };
            actor.pose_standing(instance, motion, field_time, &mut instances, &mut palette);
            if let Some((mut at, yaw, _)) = walking {
                if let Some(g) = rig.as_ref().and_then(|r| r.ground.as_ref()) {
                    at[1] = g.height(at[0], at[2], at[1], WALKER_STEP).unwrap_or(at[1]);
                }
                instances[instance as usize].model = barrel_render::render_space(actor.placed(Vec3::from(at), yaw.to_radians()));
            }
        }
        // The load screen's save is chosen and loaded: the field follows, in the same window.
        if (handoff.is_some() || opening_next || free_play_next) && (pump.is_some() || a.into_field) {
            break;
        }
        // The field's sound: its cues (the music) come due on the frame count the title began.
        if !a.title
            && let Some(sound) = sound.as_mut()
        {
            let frames = if pump.is_some() { (started.elapsed().as_secs_f32() * barrel_ryza::sound::FPS) as i64 } else { i64::from(frame_no) };
            sound.tick(sound_base + frames);
        }
        // The player's input: in a window, her lean against the camera and the camera's turn;
        // offscreen with `--walk-from`, the scripted pad's sticks at the game's frame, 1/60 s a frame.
        let drive = if let Some(p) = pump.as_mut() {
            use winit::keyboard::KeyCode as K;
            // The pad is read as menu input (not as keys); the field reads the sticks.
            p.set_menu_pad(true);

            let pad_pressed;
            (title_keys, pad_pressed) = p.take_input();
            pad_any = !pad_pressed.is_empty();
            confirm_pressed = title_keys.contains(&K::Enter) || pad_pressed.contains(&play.buttons().0);
            // STAND-IN: the keyboard's key for the swing is X; the game's own default is not read.
            swing_pressed = title_keys.contains(&K::KeyX) || pad_pressed.contains(&barrel_render::pad::Pad::West);
            if field_menu.is_some() {
                use barrel_ryza::field_menu::Key as MenuKey;
                use barrel_render::pad::Pad;
                let at = a.hud_at + started.elapsed().as_secs_f32() * 60.0;
                let down = |k: K, b: Pad| title_keys.contains(&k) || pad_pressed.contains(&b);
                let (confirm, back) = play.buttons();
                let mut press = |key| menu_keys.push((at, key));
                if !menu_up {
                    if down(K::Tab, Pad::North) {
                        press(MenuKey::Open);
                    }
                } else if settings_run.is_some() {
                    // On the Settings screen: its own keys (the arrows, Q and E, LB and RB, Enter or A, Backspace, Escape or B),
                    // a held up or down repeating after 30 frames, every 8 (`options::REPEAT`).
                    use barrel_ryza::options_run::Key as S;
                    let (up, down_key) = (p.held(K::ArrowUp) || p.pad_down(Pad::Up), p.held(K::ArrowDown) || p.pad_down(Pad::Down));
                    for (slot, held, tapped, first, repeat) in [(0, up, down(K::ArrowUp, Pad::Up), MenuKey::Up, S::RepeatUp), (1, down_key, down(K::ArrowDown, Pad::Down), MenuKey::Down, S::RepeatDown)] {
                        if tapped {
                            press(first);
                            settings_repeat[slot] = Some(at + barrel_ryza::options::REPEAT.first as f32);
                        } else if !held {
                            settings_repeat[slot] = None;
                        } else if let Some(next) = settings_repeat[slot].filter(|&n| at >= n) {
                            press(MenuKey::Screen(repeat));
                            settings_repeat[slot] = Some(next + barrel_ryza::options::REPEAT.next as f32);
                        }
                    }
                    for (k, b, key) in [(K::ArrowLeft, Pad::Left, S::Left), (K::ArrowRight, Pad::Right, S::Right), (K::KeyQ, Pad::LeftShoulder, S::TabPrev), (K::KeyE, Pad::RightShoulder, S::TabNext)] {
                        if down(k, b) {
                            press(MenuKey::Screen(key));
                        }
                    }
                    if title_keys.contains(&K::Enter) || pad_pressed.contains(&confirm) {
                        press(MenuKey::Confirm);
                    } else if down(K::Backspace, back) || title_keys.contains(&K::Escape) {
                        press(MenuKey::Back);
                    }
                } else if down(K::KeyQ, Pad::LeftShoulder) {
                    // (LT and RT on a page, Q and E in a window: the page's previous and next character)
                    press(MenuKey::Screen(barrel_ryza::options_run::Key::TabPrev));
                } else if down(K::KeyE, Pad::RightShoulder) {
                    press(MenuKey::Screen(barrel_ryza::options_run::Key::TabNext));
                } else if down(K::ArrowLeft, Pad::Left) {
                    press(MenuKey::Screen(barrel_ryza::options_run::Key::Left));
                } else if down(K::ArrowRight, Pad::Right) {
                    press(MenuKey::Screen(barrel_ryza::options_run::Key::Right));
                } else if down(K::ArrowDown, Pad::Down) {
                    press(MenuKey::Down);
                } else if down(K::ArrowUp, Pad::Up) {
                    press(MenuKey::Up);
                } else if title_keys.contains(&K::Enter) || pad_pressed.contains(&confirm) {
                    press(MenuKey::Confirm);
                } else if down(K::Backspace, back) || title_keys.contains(&K::Escape) {
                    press(MenuKey::Back);
                }
            }
            p.set_script_frame(sound_base + (started.elapsed().as_secs_f32() * barrel_ryza::sound::FPS) as i64);
            if !p.pump() || (title_keys.contains(&K::Escape) && !showing_options && !menu_up) || a.frames.is_some_and(|n| sound_base + (started.elapsed().as_secs_f32() * barrel_ryza::sound::FPS) as i64 >= i64::from(n)) {
                break;
            }
            let now = std::time::Instant::now();
            let dt = (now - last).as_secs_f32().min(0.1);
            last = now;
            frame_dt = dt;
            let key = |k: K| if p.held(k) { 1.0 } else { 0.0 };
            // The sticks as the walk takes them (`barrel_ryza::walk`: it takes the dead zones out
            // itself); the keys are the same sticks pushed to their edge. Right (the stick or
            // the arrow key) swings the camera the way the game's does (the other sign was
            // mirrored against it, tried on a handheld). Up pitches it down.
            // (The title's camera is fixed: its keys work the menu, and the play camera's
            // limits are not its own.)
            let (left, right) = p.sticks();
            let sticks = barrel_ryza::walk::Sticks {
                left: [key(K::KeyD) - key(K::KeyA) + left[0] + a.lean[0], key(K::KeyW) - key(K::KeyS) + left[1] + a.lean[1]],
                right: [key(K::ArrowRight) - key(K::ArrowLeft) + right[0], right[1] - (key(K::ArrowUp) - key(K::ArrowDown))],
            };
            let zoom = key(K::Equal) - key(K::Minus);
            Some((dt, sticks, if play.zoom_inverted { -zoom } else { zoom }))
        } else if let (Some(from), Some(script), true) = (a.walk_from, &walk_script, rig.is_some()) {
            let (_, left, right) = script.at(i64::from(from) + i64::from(frame_no));
            Some((1.0 / 60.0, barrel_ryza::walk::Sticks { left, right }, 0.0))
        } else {
            None
        };
        // The field stands still under the menu.
        if let (Some((dt, mut sticks, zoom)), false) = (drive, menu_up) {
            let length = (sticks.left[0] * sticks.left[0] + sticks.left[1] * sticks.left[1]).sqrt();
            help_bar.step(length > barrel_ryza::walk::MOVE_DEAD_ZONE || gather_busy, dt * 60.0);
            let full = if pump.as_ref().is_some_and(|p| p.held(winit::keyboard::KeyCode::ShiftLeft) || p.held(winit::keyboard::KeyCode::ShiftRight)) { SHIFT_LEAN } else { 1.0 };
            if length > full {
                sticks.left = sticks.left.map(|v| v / length * full);
            }
            if !a.title {
                cam.length = (cam.length * (1.0 + zoom * dt)).clamp(length_range.0, length_range.1);
            }
            if a.title {
                sticks.right = [0.0; 2];
            }
            if let (Some(id), Some(r)) = (gather_gone.take(), rig.as_mut()) {
                for (_, body) in r.bodies.iter().filter(|(b, _)| *b == id) {
                    if let Some(w) = r.walls.as_mut() {
                        w.remove(body);
                    }
                }
            }
            match rig.as_mut() {
                Some(r) => barrel_ryza::walk::step(&mut r.player, &mut cam, &sticks, dt, r.ground.as_ref(), r.walls.as_ref()),
                None => cam.steer(sticks.right, dt),
            }
            if let Some(path) = std::env::var_os("BARREL_TRACE_CAMERA") {
                use std::io::Write as _;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{} yaw {:.3} pitch {:.3}", frame_no, cam.yaw, cam.pitch);
                }
            }
            // The title keeps its own fixed camera (`title::CAMERA`, set after `cam` was made).
            // (A walked offscreen run with `--look-at` keeps the view it was given, moved with her.)
            if !a.title && !(a.walk_from.is_some() && a.look_at.is_some() && pump.is_none()) {
                (yaw, pitch, dist, look_height) = (180.0 - cam.yaw, cam.pitch, cam.distance(), cam.look_height());
            }
            if let Some(r) = rig.as_mut() {
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
                r.joints.clone_from(&joints);
                let base = instances[r.instance as usize].palette as usize;
                for (slot, j) in palette[base..].iter_mut().zip(&joints) {
                    *slot = render_space(*j);
                }
                let i = &mut instances[r.instance as usize];
                i.prev_model = i.model;
                i.model = render_space(Mat4::from_scale_rotation_translation(Vec3::from(r.chara.size), glam::Quat::from_rotation_y(r.player.turn), r.player.at));
                center = match (a.look_at, pump.is_none()) {
                    _ if a.camera_target.is_some() => Vec3::from(a.camera_target.unwrap_or_default()) * Vec3::new(-1.0, 1.0, 1.0),
                    (Some(p), true) if a.walk_from.is_some() => (p + (r.player.at - walk_start)) * Vec3::new(-1.0, 1.0, 1.0),
                    _ => (r.player.at + Vec3::new(0.0, look_height, 0.0)) * Vec3::new(-1.0, 1.0, 1.0),
                };
                if frame_no % 60 == 0 {
                    eprintln!("frame {frame_no} ({:.2}s): Ryza at {:?}, {:?}, motion {} at {:.2}s", started.elapsed().as_secs_f32(), r.player.at, r.player.gait, r.player.motion, r.player.clock);
                }
            }
        }
        if let Some(p) = pump.as_mut() {
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
        if let Some(play) = title_play.as_mut().filter(|_| a.opening || !cast.is_empty()) {
            let clock = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
            let boot_ahead = if a.boot { a.boot_at - title_start } else { 0.0 };
            actors::advance(play, if a.opening { barrel_ryza::opening::stage_seconds(a.opening_at) + clock } else { a.title_at + boot_ahead + clock - actors::EVENT_START });
            // The event's sound: its music commands, and the message sound of each button that closes a line.
            if let (true, Some(s)) = (a.opening, sound.as_mut()) {
                let now = sound_base + if pump.is_some() { (started.elapsed().as_secs_f32() * barrel_ryza::sound::FPS) as i64 } else { i64::from(frame_no) };
                let offset = sound_base - i64::from(a.opening_at - barrel_ryza::opening::START);
                barrel_ryza::sound::schedule_commands(s, &sound_cmds, offset, sound_heard, now, &streams, &effects);
                sound_heard = now;
                // STAND-IN: the message sound falls on the frame after the step that closes the line (the schedule stamps the button's step one frame ahead of the frame the game's key logs the sound on: `opening-key4`, 8255 for the press the stage closes at 8254).
                for _ in closes_heard..play.stage.lines_closed {
                    s.schedule(barrel_ryza::sound::Cue::effect(now + 1, barrel_ryza::sound::MESSAGE));
                }
                closes_heard = play.stage.lines_closed;
            }
            // The help event's last fade is over: the field goes on as free play (a window; offscreen the frames past it are a run of their own).
            if a.opening && pump.is_some() && opening_over.is_some_and(|end| play.stage.time >= end) {
                eprintln!("opening: over at {:.2} s (stage {:.2} s): on into free play", opening_over.unwrap_or(0.0), play.stage.time);
                free_play_next = true;
            }
            if live_presses && confirm_pressed {
                // The player's button: a press at the stage's moment, the schedule laid out again from it (the steps already applied do not change).
                presses.push(play.stage.time);
                eprintln!("opening: button {} at stage {:.2} s", presses.len(), play.stage.time);
                let buttons = barrel_engine::event::Buttons { presses: presses.clone(), ..barrel_ryza::event::buttons_at(&[], barrel_ryza::opening::START) };
                if let Some((steps, taken, help)) = lay(Some(buttons))? {
                    play.relay(steps);
                    opening_cues = barrel_ryza::talk::Cues::from_taken(&taken).help_from(help);
                    opening_over = over_of(&taken);
                    sound_cmds = sound_of(&taken, help);
                }
            }
            if a.opening {
                // The event's camera (its eye and target as this view's centre, turn, pitch and distance; render space mirrors x).
                let snap = play.stage.snapshot();
                if let (Some(eye), Some(target)) = (snap.eye, snap.target) {
                    let d = Vec3::new(-(eye[0] - target[0]), eye[1] - target[1], eye[2] - target[2]);
                    dist = d.length().max(1.0);
                    center = Vec3::from(target) * Vec3::new(-1.0, 1.0, 1.0);
                    pitch = (d.y / dist).asin().to_degrees();
                    yaw = d.x.atan2(d.z).to_degrees();
                    fov = snap.fov.map_or(fov, f32::to_radians);
                }
                // Its telop and fade, in their own layer.
                if let Some((layer, telops, talk)) = opening_layer.as_mut() {
                    let mut quads = Vec::new();
                    let seen = play.stage.time - barrel_engine::stage::FRAME * 0.5;
                    if opening_cues.bars(seen) {
                        quads.extend(talk.bars((width, height)));
                    }
                    let pictures = opening_cues.pictures(seen);
                    if let Some(set) = pictures {
                        quads.extend(talk.picture_quads(set, play.stage.time, (width, height)));
                    }
                    if let Some((n, alpha)) = barrel_ryza::event::telop(&play.stage)
                        && let Some(pictures) = telops.get(&n)
                    {
                        for &(rect, uv, texture) in pictures {
                            quads.push(text::Quad { turn: false, spin: None, rect, uv, color: [1.0, 1.0, 1.0, alpha], texture });
                        }
                    }
                    if let Some(line) = &play.stage.line {
                        if pictures.is_some() || line.speaker < 0 {
                            quads.extend(talk.tutorial_quads(line, play.stage.time, opening_cues.box_hint(seen), (width, height)));
                        } else {
                            quads.extend(talk.line_quads(line, play.stage.time, opening_cues.hint(seen), (width, height)));
                        }
                    }
                    let cover = actors::cover(&play.stage);
                    if cover > 0.0 {
                        quads.push(barrel_ryza::load::cover(cover, (width, height), 0));
                    }
                    let white = actors::cover_white(&play.stage);
                    if white > 0.0 {
                        quads.push(text::Quad { color: [1.0, 1.0, 1.0, white], ..barrel_ryza::load::cover(white, (width, height), 0) });
                    }
                    layer.set(&gpu, &quads)?;
                }
            }
            for actor in &cast {
                if let Some(body) = play.stage.characters.get(&actor.slot) {
                    actor.pose(body, play.stage.time, &mut instances, &mut palette);
                } else if a.opening && !actor.is_prop() {
                    // not on the stage (yet, or any more): nothing of it is drawn
                    instances[actor.instance as usize].model = Mat4::ZERO;
                }
            }
        }
        // Her cloth on her pose now, in the wind now.
        let mut cloth_moves = false;
        if let (Some(r), Some(mesh)) = (rig.as_mut(), ryza_mesh.as_ref().and_then(|m| m.geometry.as_ref()))
            && let Some(cloth) = r.cloth.as_mut()
        {
            let clock = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
            let wind = cloth::world_wind(gust, r.chara.cloth_wind_scale, clock);
            let dt = if pump.is_some() { frame_dt } else { 1.0 / 60.0 };
            cloth.update(mesh, &r.joints, cloth::model_matrix(r.chara.size, r.player.turn, r.player.at), wind, dt, &mut palette);
            cloth_moves = true;
            if let Some(path) = std::env::var_os("BARREL_TRACE_CLOTH") {
                use std::io::Write as _;
                let frame = i64::from(a.walk_from.unwrap_or(0)) + i64::from(frame_no);
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{}", cloth.trace_line(frame));
                }
            }
        }
        // The settings were saved with Yes: the render settings and the presets they come to
        // are applied now, in this run (`Renderer::apply_settings`), as a fresh start with the
        // saved config would have them.
        // `BARREL_REAPPLY_AT=N[,M...]`: the same settings applied again at frames N, M..., a check that a
        // renderer made again draws as the one it replaces (tools/live-settings.sh `same`).
        if reapply_at.contains(&(frame_no as u32)) {
            let s = renderer.settings;
            renderer = renderer.apply_settings(&gpu, presenter.as_ref(), s)?;
            scene.rebind(&gpu, &renderer.shared);
            eprintln!("settings: applied again (frame {frame_no})");
        }
        for (_, file) in config_at.iter().filter(|(n, _)| *n == frame_no as u32) {
            if let (Some(path), Ok(text)) = (barrel_files::config::graphics_path(), std::fs::read_to_string(file)) {
                std::fs::write(path, text)?;
                apply_settings = true;
                eprintln!("settings: config from {file} (frame {frame_no})");
            }
        }
        if std::mem::take(&mut apply_settings) {
            let config = read_config();
            let now = Resolved::from(a.preset, config.as_deref(), ini_text.as_deref(), &read_table);
            // The Audio, Camera and Game rows: the volumes on the mixer, the camera's feel, the confirm button.
            let saved = barrel_ryza::preferences::Play::from_config(config.as_deref());
            if saved != play {
                if saved.volumes != play.volumes
                    && let Some(s) = sound.as_mut()
                {
                    s.set_volumes(&saved.volumes);
                }
                cam.feel = saved.feel;
                play = saved;
                eprintln!("settings: audio, camera and game values applied ({:?} {:?} swap {})", play.volumes, play.feel, play.swap_confirm);
            }
            // The display rows: the live window takes the size or fullscreen and the pacing as the
            // config has them (the swapchain follows through the resize path). Offscreen runs have no window.
            let (new_display, new_pacing) = (display_of(config.as_deref()), pacing_of(config.as_deref()));
            if let (Some(p), Some(pr)) = (pump.as_mut(), presenter.as_mut()) {
                if new_display != display {
                    p.set_display(new_display);
                }
                if new_pacing != pacing {
                    pr.set_pacing(&gpu, new_pacing)?;
                    stale = true;
                }
                if new_display != display || new_pacing != pacing {
                    eprintln!("display: changed live ({new_display:?}, {new_pacing:?})");
                    display_probe = Some((std::time::Instant::now(), 0));
                }
                (display, pacing) = (new_display, new_pacing);
            }
            if now.graphics != renderer.settings || now.presets != presets_now.get() {
                eprintln!("settings: applied live (game frame {})", sound_base + i64::from(frame_no as i32));
            }
            if now.graphics != renderer.settings {
                renderer = renderer.apply_settings(&gpu, presenter.as_ref(), now.graphics)?;
                scene.rebind(&gpu, &renderer.shared);
                shadow_size = (!a.no_shadows).then_some(renderer.shadow_target.size);
            }
            if now.presets.grass.density != presets_now.get().grass.density
                && let (Some(f), Some(g)) = (&plant_field, &grass)
                && let Some(cards) = plant_cards(g, &main, &files.area, now.presets.grass.density, f.vertex_base, f.index_base)
            {
                // The plants are the scene's last vertices and indices: laid again as a cold
                // start with this density lays them, and their draws in the old ones' place.
                let as_bytes = |p: *const u8, n: usize| unsafe { std::slice::from_raw_parts(p, n) };
                scene.replace_geometry_tail(
                    &gpu,
                    renderer.pool,
                    u64::from(f.vertex_base) * size_of::<Vertex>() as u64,
                    as_bytes(cards.vertices.as_ptr() as *const u8, cards.vertices.len() * size_of::<Vertex>()),
                    u64::from(f.index_base) * 4,
                    as_bytes(cards.indices.as_ptr() as *const u8, cards.indices.len() * 4),
                )?;
                let at = draws.iter().position(|d| d.name == "plants").unwrap_or(draws.len());
                draws.retain(|d| d.name != "plants");
                let fresh: Vec<Draw> = plant_draws(&cards, f.atlas, f.normals, f.instance)
                    .zip(&cards.cells)
                    .map(|(mut d, c)| {
                        d.light = FULL_LIGHT;
                        d.bounds = Some(Bounds { lo: c.boxes[0].0, hi: c.boxes[0].1, pad: f32::INFINITY, round: false });
                        d
                    })
                    .collect();
                draws.splice(at..at, fresh);
                *plant_boxes.borrow_mut() = cards.boxes();
                eprintln!("settings: plants for density {}", now.presets.grass.density);
            }
            if now.presets.texture_bias != presets_now.get().texture_bias {
                let biased: Vec<barrel_formats::g1t::Texture> = unbiased.iter().map(|t| t.without_top_levels(now.presets.texture_bias)).collect();
                let biased: Vec<&barrel_formats::g1t::Texture> = biased.iter().collect();
                let (uploaded, opaque, ..) = upload_textures(&gpu, renderer.pool, &biased, None, &linear)?;
                scene.replace_textures(&gpu, &renderer.shared, uploaded, opaque)?;
                eprintln!("settings: textures at mip bias {}", now.presets.texture_bias);
            }
            let (was, is) = (presets_now.get(), now.presets);
            if (was.lod, was.lods, was.objects, was.ranged) != (is.lod, is.lods, is.objects, is.ranged) && far_view.get().is_some() {
                // The static shadow layer's casters are the view's ranges and levels of detail
                // at the frame it was drawn: drawn again with the new ones, from that view.
                renderer.forget_far_shadow();
                far_replay = true;
            }
            presets_now.set(now.presets);
            ranges.set(ranges_for(&now.presets)?);
        }
        sync.wait_slot(&gpu, renderer.pool)?;
        if pump.is_some() || cloth_moves || !cast.is_empty() || !moving.is_empty() || !npc_drawn.is_empty() {
            scene.write_instances(&gpu, &instances)?;
            scene.write_palette(&gpu, &palette, &prev_palette)?;
        }
        // Following her: no nearer than the ground behind her allows (`camera::clear_distance`).
        let drawn_dist = match (a.camera_length, follow, rig.as_ref().and_then(|r| r.ground.as_ref().map(|g| (r, g)))) {
            (Some(l), ..) => l,
            (_, Some(_), Some((r, g))) => {
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
        // The dome's layers have slid since the field was entered, before the sky's clock
        // began (`BARREL_DOME_CLOCK`: that time on a captured frame).
        dome_clock.set(phase_override("BARREL_DOME_CLOCK").unwrap_or(sky_clock));
        // Her box (the player's draws, in render space) for the characters' shadow map.
        let sub_box = draws.iter().filter(|d| d.sub_bias.is_some()).filter_map(|d| Some((d.bounds?, instances[d.instance as usize].model))).fold(None, |acc: Option<(Vec3, Vec3)>, (b, m)| {
            let corners = (0..8).map(|k| m.transform_point3(Vec3::new(if k & 1 == 0 { b.lo.x } else { b.hi.x }, if k & 2 == 0 { b.lo.y } else { b.hi.y }, if k & 4 == 0 { b.lo.z } else { b.hi.z })));
            let (lo, hi) = corners.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(l, h), c| (l.min(c), h.max(c)));
            Some(acc.map_or((lo, hi), |(l, h)| (l.min(lo), h.max(hi))))
        });
        // The point the sun's shadow is asked about for the sun flare: where the camera looks
        // behind her (the player and her look height), else where it is aimed.
        let sun_probe = rig.as_ref().map_or(center, |r| (r.player.at + Vec3::new(0.0, look_height, 0.0)) * Vec3::new(-1.0, 1.0, 1.0));
        let mut view = look_for(sub_box, center, sun_probe, yaw, pitch, drawn_dist, fov, (width, height), prev_vp, environment.as_ref(), &flare_look, shadow_size, presets_now.get().shadow, static_box, title_clip, sky_clock);
        eye_now.set(Vec3::from_slice(&view.look[4][..3]));
        // The icons over the doors in view, for the HUD.
        let hud_marks: Vec<barrel_ryza::field_hud::WorldMark> = hud_markers.doors.iter().filter_map(|&d| barrel_ryza::field_hud::world_mark(&view.vp, eye_now.get(), d, (width, height))).collect();
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
        *occluders.borrow_mut() = barrel_ryza::occluders::Occluders::new(&occluder_quads, eye_now.get(), forward_now.get(), &barrel_render::view_planes(&view.vp));
        sides_now.set([(deep + across) / across.truncate().length(), (deep - across) / across.truncate().length(), (deep + up) / up.truncate().length(), (deep - up) / up.truncate().length()]);
        // The plants' wind (sway slots 0 and 1; nothing else sways in Ryza's fields yet).
        // `g_vAttCache0` (a, a, a/2, -a/2) and {`g_fAttCache1` 1, `g_fPhase` 0, `g_fOscPhase`}.
        // From the field's wind (`environment::Gust`): the bend from its power now, the sway's
        // phase what it has turned since the field came up (a window's own clock, 60 Hz offscreen).
        let wind_clock = if pump.is_some() { started.elapsed().as_secs_f32() } else { frame_no as f32 / 60.0 };
        let (wind_power, wind_turned) = gust.map_or((0.0, 0.0), |g| g.at(wind_clock));
        let bend = barrel_ryza::environment::Gust::bend(wind_power);
        // For parity: a captured frame's bend and sway phase (`g_vAttCache0.x`, `g_fOscPhase`).
        let (bend, wind_turned) = std::env::var("BARREL_PLANT_WIND")
            .ok()
            .and_then(|v| v.split_once(',').and_then(|(b, t)| Some((b.trim().parse().ok()?, t.trim().parse().ok()?))))
            .unwrap_or((bend, wind_turned));
        // STAND-IN: The cloud shadows' drift starts with the field; the real thing is the game's own phase.
        if let Some((p, _)) = &cloud {
            let drift = |speed: f32| phase_override("BARREL_PATTERN_PHASE").unwrap_or_else(|| (wind_clock * speed).rem_euclid(1.0));
            // The game's matrix (`mW2Pt`) slides a point along the sunlight onto the plane at
            // height 0 (its translation is the drift alone, the same on both axes); the shader
            // slides onto the plane 1000 up, so that plane's share is taken off here.
            let sun = view.look[22];
            let onto = |way: f32, size: f32| if sun[1].abs() > 1e-4 { 1000.0 * way / (sun[1] * size) } else { 0.0 };
            view.look[48] = [1.0 / p.size[0], 1.0 / p.size[1], drift(p.speed[0]) - onto(-sun[0], p.size[0]), drift(p.speed[1]) - onto(sun[2], p.size[1])];
            view.look[49] = [1.0, p.strength, 0.0, 0.0];
        }
        view.look[50] = [bend, bend, bend * 0.5, -bend * 0.5];
        // The water, where the set has a sky to reflect and the field has water.
        let water_pieces = match &water {
            Some(w) if view.sky_plane => {
                let w = &water_in_view(w, &barrel_render::view_planes(&view.vp));
                view.look[WATER_SLOT..WATER_SLOT + w.len()].copy_from_slice(w);
                view.look[WATER_SLOT][1] = sky_clock;
                view.look[WATER_SLOT][0] as u32
            }
            _ => 0,
        };
        view.look[51] = [1.0, 0.0, wind_turned % std::f32::consts::TAU, 0.0];
        view.look[FADE_SLOT] = [presets_now.get().grass.far[0], presets_now.get().grass.far[1], 0.0, 0.0];
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
            // A layer being drawn takes this frame's view for its casters, unless it is the one
            // drawn again for new settings (the first one's).
            if renderer.far_shadow.borrow().build.is_some() && !std::mem::take(&mut far_replay) {
                far_view.set(Some((scene_on.get(), eye_now.get(), forward_now.get(), sides_now.get())));
            }
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
                Some(_) => a.boot_at + clock - title_start + a.title_at,
                None => a.title_at + clock,
            };
            // The player's keys in a window (the pad's buttons stand for them): Enter at the
            // prompt, the arrows on the menu. A direction is held while its key is down.
            let frame = (title_seconds * 60.0).round() as i32;
            // The button on the menu's item counts only on Load Game, with saves to load.
            let loads = |input: &barrel_ryza::title_menu::Input, at: i32| menu.chosen(input, at) == Some("load_game") && !save_slots.is_empty();
            let settings_at = |input: &barrel_ryza::title_menu::Input| input.choose.filter(|&c| menu.chosen(input, c) == Some("setting") && input.returned.is_none_or(|r| frame <= r));
            let scripted = pump.is_none() && !(title_input.tabs.is_empty() && title_input.changes.is_empty() && title_input.backs.is_empty());
            if let (true, Some(c)) = (scripted, settings_at(&title_input)) {
                // Offscreen: the scripted keys (tabs, changes, B, A, the directions) run again from
                // the screen's first frame to this one (`barrel_ryza::leave::replay`).
                let r = barrel_ryza::leave::replay(&scr.initial, &title_input, c + barrel_ryza::options::COVER_FULL, frame);
                scr.state = r.state;
                scr.leave = r.leave;
                scripted_holds = r.holds;
                scripted_tab_change = r.tab_change;
                scripted_blink = r.blink;
                title_input.returned = scr.leave.left_at();
                title_input.returned_lead = scr.leave.lead();
                title_input.returned_spin = menu.return_spin(&title_input, c);
                // The replayed A on Yes writes the changes where a player's would (once), and the
                // settings they come to are applied.
                if !scripted_saved && scr.leave.saved_at().is_some_and(|at| frame >= at) {
                    scripted_saved = true;
                    save_options(&scr.state);
                    apply_settings = true;
                }
            }
            if let Some(p) = pump.as_ref() {
                use winit::keyboard::KeyCode as K;
                let in_settings = matches!(barrel_ryza::options::Stage::at(settings_at(&title_input), frame), barrel_ryza::options::Stage::Screen { .. });
                let listing = in_settings || matches!(flow::Stage::at(&flow_input(&title_input, &loads), save_slots.len(), frame), flow::Stage::Screen { list: Some((_, None)), .. });
                // The keyboard and the pad feed one menu input (`barrel_ryza::menu_input`).
                use barrel_ryza::menu_input::{Action as Act, Actions};
                let keys = |ks: &dyn Fn(&K) -> bool| {
                    const KEYS: [(K, Act); 9] = [
                        (K::ArrowUp, Act::Up),
                        (K::ArrowDown, Act::Down),
                        (K::ArrowLeft, Act::Left),
                        (K::ArrowRight, Act::Right),
                        (K::Enter, Act::Confirm),
                        (K::Escape, Act::Back),
                        (K::Backspace, Act::Back),
                        (K::KeyQ, Act::TabPrev),
                        (K::KeyE, Act::TabNext),
                    ];
                    KEYS.iter().fold(Actions::default(), |acc, &(k, a)| acc.with(a, acc.has(a) || ks(&k)))
                };
                use barrel_render::pad::Pad;
                let lean = p.menu_lean();
                let pad = [
                    (Act::Up, p.pad_down(Pad::Up) || lean.y > 0),
                    (Act::Down, p.pad_down(Pad::Down) || lean.y < 0),
                    (Act::Left, p.pad_down(Pad::Left) || lean.x < 0),
                    (Act::Right, p.pad_down(Pad::Right) || lean.x > 0),
                    (Act::Confirm, p.pad_down(play.buttons().0)),
                    (Act::Back, p.pad_down(play.buttons().1)),
                    (Act::TabPrev, p.pad_down(Pad::LeftShoulder)),
                    (Act::TabNext, p.pad_down(Pad::RightShoulder)),
                ]
                .into_iter()
                .fold(Actions::default(), |acc, (a, on)| acc.with(a, on));
                menu_input.frame(keys(&|k| title_keys.contains(k)), keys(&|k| p.held(*k)), pad, pad_any);
                let pressed = |a| menu_input.pressed(a);
                // Any button leaves "Press any button" (the keyboard's is Enter); the rest wait for A.
                if menu.pressed(&title_input).is_none() {
                    if pressed(Act::Confirm) || menu_input.any_pad {
                        title_input.press = Some(frame);
                    }
                } else if pressed(Act::Confirm) {
                    if title_input.choose.is_none() && (loads(&title_input, frame) || matches!(menu.chosen(&title_input, frame), Some("setting" | "new_game"))) {
                        title_input.choose = Some(frame);
                        title_input.returned = None;
                    } else if listing && !in_settings && title_input.confirm.is_none() {
                        title_input.confirm = Some(frame);
                    }
                }
                if in_settings && !was_in_settings {
                    scr.enter(settings_at(&title_input).unwrap_or(frame) + barrel_ryza::options::COVER_FULL);
                }
                was_in_settings = in_settings;
                // The screen takes the frame's keys and settles (the list's cursor is the hold record's, below).
                use barrel_ryza::options_run::{Effects, Key as ScreenKey};
                let mut fx = Effects::default();
                if in_settings {
                    let mut keys: Vec<ScreenKey> = [
                        (Act::Up, ScreenKey::Up),
                        (Act::Down, ScreenKey::Down),
                        (Act::Left, ScreenKey::Left),
                        (Act::Right, ScreenKey::Right),
                        (Act::Confirm, ScreenKey::Confirm),
                        (Act::Back, ScreenKey::Back),
                        (Act::TabPrev, ScreenKey::TabPrev),
                        (Act::TabNext, ScreenKey::TabNext),
                    ]
                    .into_iter()
                    .filter(|&(act, _)| pressed(act))
                    .map(|(_, key)| key)
                    .collect();
                    keys.extend(title_input.tabs.iter().filter(|t| t.0 == frame).map(|t| if t.1 < 0 { ScreenKey::TabPrev } else { ScreenKey::TabNext }));
                    if keys.contains(&ScreenKey::Back) && scr.takes_keys(frame) {
                        eprintln!("settings: leaving (game frame {})", sound_base + i64::from(frame));
                    }
                    scr.press(frame, &keys, &mut fx);
                }
                scr.settle(frame, &mut fx);
                if fx.saved {
                    save_options(&scr.state);
                    apply_settings = true;
                }
                if let Some(at) = fx.turned {
                    tab_changed_at = Some(at);
                    title_input.list_holds.clear();
                    options_base = scr.state.cursor[scr.state.tab];
                }
                if let Some(sound) = sound.as_mut() {
                    for cue in fx.cues {
                        let (name, id) = cue.sound();
                        sound.request(name, barrel_ryza::sound::Cue::effect(i64::from(frame), id));
                    }
                }
                // The screen is left once its leaving is over: the changes the player did not keep are dropped and the title's menu is back.
                if let Some(l) = scr.left(frame) {
                    eprintln!("settings: left (game frame {})", sound_base + i64::from(frame));
                    scr.finish_leaving();
                    shown_row = None;
                    title_input.list_holds.clear();
                    title_input.returned_spin = title_input.choose.map_or(0, |c| menu.return_spin(&title_input, c));
                    title_input.choose = None;
                    title_input.returned = Some(l);
                    title_input.returned_lead = scr.leave.lead();
                    options_base = 0;
                    was_in_settings = false;
                    scr.leave = barrel_ryza::leave::Leave::default();
                }
                for (act, way) in [(Act::Down, 1), (Act::Up, -1)] {
                    let held = menu_input.held(act);
                    let holds = if listing { &mut title_input.list_holds } else { &mut title_input.holds };
                    for h in holds.iter_mut().filter(|h| h.way == way && h.until.is_none() && !held) {
                        h.until = Some(frame);
                    }
                    if pressed(act) && !(in_settings && scr.leave.busy(frame)) {
                        // (a tap shorter than a frame is a one-frame hold)
                        holds.push(barrel_ryza::title_menu::Hold { from: frame, until: (!held).then_some(frame + 1), way });
                    }
                }
            }
            let with_menu = Some((&*menu, &title_input));
            let quads = match boot {
                // The boot sequence, then the title from its own start.
                Some(boot) => {
                    let t = a.boot_at + clock;
                    scene_on.set(t >= title_start);
                    if t < title_start { if boot_saves { boot.quads_with_saves(t, (width, height)) } else { boot.quads(t, (width, height)) } } else { title.quads_with(title_seconds, (width, height), with_menu) }
                }
                None => title.quads_with(title_seconds, (width, height), with_menu),
            };
            if let Some(sound) = sound.as_mut() {
                use barrel_ryza::sound::{Cue, CLICK, CURSOR, PRESS_START};
                let now = i64::from(frame);
                // The menus' effects, each at the frame of the button or step that makes it.
                if let Some(f) = menu.pressed(&title_input).filter(|&f| f <= frame) {
                    sound.request("press", Cue::effect(i64::from(f), PRESS_START));
                }
                if let Some(c) = title_input.choose.filter(|&c| c <= frame && (loads(&title_input, c) || matches!(menu.chosen(&title_input, c), Some("setting" | "new_game")))) {
                    sound.request("choose", Cue::effect(i64::from(c), CLICK));
                }
                let flowing = flow_input(&title_input, &loads);
                if let Some(k) = flow::confirmed(&flowing).filter(|&k| k <= frame) {
                    sound.request("confirm", Cue::effect(i64::from(k), CLICK));
                }
                if let Some(k) = flow::stop_frame(&flowing).filter(|&k| k <= frame) {
                    sound.request("stop", Cue::stop(i64::from(k)));
                }
                let pressed = menu.pressed(&title_input);
                let menu_steps = title_input.holds.iter().any(|h| pressed.is_some_and(|p| h.from > p) && h.steps_at(frame));
                let rule = if showing_options { barrel_ryza::options::REPEAT } else { barrel_ryza::title_menu::MENU_REPEAT };
                if menu_steps || title_input.list_holds.iter().any(|h| h.steps_by(frame, rule)) {
                    sound.request("cursor", Cue::effect(now, CURSOR));
                }
                sound.tick(now);
            }
            // The path from the menu's Load Game to the field (`barrel_ryza::load`).
            let mut quads = quads;
            let cover_texture = if boot.is_some() { barrel_ryza::boot::LOGO_GUST } else { 2 };
            let mut own_layer = false;
            // The loading screen the boot with saves opens on, in the load screen's layer.
            if let (true, Some(age), Some((layer, view, _))) = (boot_saves, barrel_ryza::boot::saves_loading(a.boot_at + clock), load_screen.as_mut()) {
                quads = vec![text::Quad { turn: false, spin: None, rect: [0.0, 0.0, width as f32, height as f32], uv: [-1.0; 4], color: [0.0, 0.0, 0.0, 1.0], texture: 0 }];
                quads.extend(view.loading(age, (width, height)));
                layer.set(&gpu, &quads)?;
                own_layer = true;
            }
            let flowing_input = flow_input(&title_input, &loads);
            match flow::Stage::at(&flowing_input, save_slots.len(), frame) {
                flow::Stage::Title => {}
                flow::Stage::Cover(alpha) => quads.push(flow::cover(alpha, (width, height), cover_texture)),
                flow::Stage::Black => quads = vec![flow::cover(1.0, (width, height), cover_texture)],
                flow::Stage::Screen { on, list, notice } => {
                    if let Some((layer, view, boot)) = load_screen.as_mut() {
                        quads = vec![text::Quad { turn: false, spin: None, rect: [0.0, 0.0, width as f32, height as f32], uv: [-1.0; 4], color: [0.0, 0.0, 0.0, 1.0], texture: 0 }];
                        // The notice sits under the help bar in the game's draw order.
                        let notice = notice.filter(|n| n.2 > 0.0).map(|(word, age, alpha)| boot.loading_notice(word, age as f32 / 60.0, alpha, (width, height))).unwrap_or_default();
                        match list {
                            Some((age, leaving)) => quads.extend(view.quads(&save_slots, barrel_ryza::load_view::Moment { age, leaving, on }, notice, (width, height))),
                            None => quads.extend(notice),
                        }
                        layer.set(&gpu, &quads)?;
                        own_layer = true;
                    }
                }
                flow::Stage::Loading { on, since, done } => {
                    if let Some((layer, view, _)) = load_screen.as_mut() {
                        quads = vec![text::Quad { turn: false, spin: None, rect: [0.0, 0.0, width as f32, height as f32], uv: [-1.0; 4], color: [0.0, 0.0, 0.0, 1.0], texture: 0 }];
                        quads.extend(view.loading(since - flow::LOADING_AT, (width, height)));
                        layer.set(&gpu, &quads)?;
                        own_layer = true;
                    } else {
                        quads = vec![flow::cover(1.0, (width, height), cover_texture)];
                    }
                    if done {
                        handoff = save_slots.get(on).map(|s| s.file.clone());
                        handoff_at = i64::from(frame) + 1;
                        music_at = flow::stop_frame(&flowing_input).map_or(handoff_at, |k| i64::from(k) + barrel_ryza::sound::FIELD_MUSIC_AFTER_STOP);
                    }
                }
            }
            // The Settings screen over the menu's black (`barrel_ryza::options`).
            showing_options = false;
            showing_notice = false;
            match barrel_ryza::options::Stage::at(settings_at(&title_input), frame) {
                barrel_ryza::options::Stage::Title => {
                    // Back from the screen: the title comes up out of black.
                    if let Some(r) = title_input.returned.filter(|&r| frame > r) {
                        let alpha = (((r - title_input.returned_lead) as f32 + barrel_ryza::leave::RETURN_FADE - frame as f32) / barrel_ryza::leave::RETURN_FADE).clamp(0.0, 1.0);
                        if alpha > 0.0 {
                            quads.push(flow::cover(alpha, (width, height), cover_texture));
                        }
                    }
                }
                barrel_ryza::options::Stage::Cover(alpha) => quads.push(flow::cover(alpha, (width, height), cover_texture)),
                barrel_ryza::options::Stage::Screen { age } => {
                    if let Some((layer, view, strings)) = options_screen.as_mut() {
                        let tab = scr.state.tab;
                        let n = scr.state.entries(tab).len().max(1);
                        // The cursor's blink starts again a frame before a tab change (ryza-recomp settings-tabs:
                        // 0.067 of its opacity 55 frames after each RB; from the change itself 0.086).
                        let tab_change = if pump.is_some() { tab_changed_at } else { scripted_tab_change };
                        // The directions held before a scripted tab change belonged to the tab it left.
                        let mut held = title_input.clone();
                        if scripted {
                            held.list_holds = scripted_holds.clone();
                        }
                        held.list_holds.retain(|h| tab_change.is_none_or(|f| h.from >= f));
                        scr.state.cursor[tab] = barrel_ryza::options::cursor_on(&held, options_base, n, frame);
                        scr.state.follow();
                        if shown_row != Some((tab, scr.state.cursor[tab])) {
                            shown_row = Some((tab, scr.state.cursor[tab]));
                            eprintln!("settings: tab {} row {} (of {n}, game frame {})", tab as i32 - barrel_ryza::options::FIRST_TAB as i32, scr.state.cursor[tab], sound_base + i64::from(frame));
                        }
                        // (the dialog holds the cursor's blink still while it is up)
                        let blink_age = if pump.is_some() { scr.blink_age(frame, age) } else { scripted_blink.unwrap_or(age - scr.leave.stood(frame)) };
                        let blink = if pump.is_some() { (blink_age * barrel_ryza::options::blink_speed(tab)) as f32 } else { barrel_ryza::options::blink_clock(blink_age, barrel_ryza::options::blink_speed(tab)) };
                        let clips;
                        let glow;
                        let look = scr.leave.look(frame);
                        let mut screen = scr.state.screen(strings);
                        screen.leave = look.clone();
                        (quads, clips, glow) = view.quads(&screen, barrel_ryza::options_view::Moment { age, blink, from_menu: false }, (width, height));
                        // The black over the screen as it is saved, and the notice over that (in the load screen's layer, which holds its pictures).
                        if look.cover > 0.0 {
                            quads.push(flow::cover(look.cover, (width, height), 0));
                        }
                        if let (Some((alpha, age)), Some((notice_layer, _, boot))) = (look.notice, load_screen.as_mut()) {
                            notice_layer.set(&gpu, &boot.saving_notice((age as f32 + std::env::var("NS").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0)) / 60.0, alpha, (width, height)))?;
                            showing_notice = true;
                        }
                        layer.set_blended(&gpu, &quads, &clips, glow)?;
                        own_layer = true;
                        showing_options = true;
                    }
                }
            }
            // New Game: the black rises over the title, then the opening begins (`barrel_ryza::new_game`).
            if let Some(c) = title_input.choose.filter(|&c| c < frame && menu.chosen(&title_input, c) == Some("new_game")) {
                let alpha = barrel_ryza::new_game::cover(frame, c);
                if alpha > 0.0 {
                    quads.push(flow::cover(alpha, (width, height), cover_texture));
                }
                if barrel_ryza::new_game::opening_begun(frame, c) {
                    opening_next = true;
                    handoff_at = i64::from(frame) + 1;
                }
            }
            showing_load = own_layer && !showing_options;
            // The screen's own picture covers the title's scene (the game clears to black behind it).
            if showing_options {
                scene_on.set(false);
            } else if boot.is_none() {
                scene_on.set(true);
            }
            // The event's fade over the title: black from a point of the Gust logo.
            let showing_title = boot.as_ref().is_none_or(|_| a.boot_at + clock >= title_start);
            let alpha = title_play.as_ref().filter(|_| showing_title).map_or(0.0, |p| actors::cover(&p.stage));
            if alpha > 0.0 {
                let texture = if boot.is_some() { barrel_ryza::boot::LOGO_GUST } else { 2 };
                quads.push(text::Quad { turn: false, spin: None, rect: [0.0, 0.0, width as f32, height as f32], uv: [0.5; 4], color: [0.0, 0.0, 0.0, alpha], texture });
            }
            if !own_layer {
                layer.set(&gpu, &quads)?;
            }
        }
        let menu_tick = a.hud_at + if pump.is_some() { started.elapsed().as_secs_f32() * 60.0 } else { frame_no as f32 };
        let mut menu_now = field_menu.as_ref().and_then(|(_, v)| barrel_ryza::field_menu::replay(&menu_keys, menu_tick, &v.card_order(menu_shown.party.len())));
        if menu_now.is_some() != menu_up {
            eprintln!("menu: {} (game frame {})", if menu_now.is_some() { "open" } else { "closed" }, menu_tick.floor());
        }
        menu_up = menu_now.is_some();
        // The Settings screen A on the menu's Settings button opened: it runs on its own clock from its first frame.
        // (Its leaving is told to the menu as a key, which the menu's state is then replayed with.)
        loop {
        match (menu_now.as_ref().filter(|n| n.in_settings).and_then(|n| n.chosen), settings_run.as_mut()) {
            (Some(c), None) => {
                eprintln!("settings: opened from the menu (game frame {})", c.floor());
                settings_run = Some(SettingsRun { run: barrel_ryza::options_run::Run::open(scr.state.clone()), first: c.floor() as i32 + barrel_ryza::field_menu::SETTINGS_AT, told: false, seen: (c.floor() as i32 + barrel_ryza::field_menu::SETTINGS_AT) as f32 - 0.001 });
            }
            // A on Settings again while the last screen's remnants are still going out: a new screen.
            (Some(c), Some(sr)) if sr.over_since().is_some() => {
                if let Some(done) = settings_run.take() {
                    scr.set(done.run.finish());
                }
                settings_run = Some(SettingsRun { run: barrel_ryza::options_run::Run::open(scr.state.clone()), first: c.floor() as i32 + barrel_ryza::field_menu::SETTINGS_AT, told: false, seen: (c.floor() as i32 + barrel_ryza::field_menu::SETTINGS_AT) as f32 - 0.001 });
            }
            (None, Some(sr)) if sr.over_since().is_some_and(|over| menu_tick >= over as f32 + SETTINGS_AFTER) => {
                // Back on the menu: the values the screen left stand.
                if let Some(done) = settings_run.take() {
                    scr.set(done.run.finish());
                }
            }
            _ => {}
        }
        let mut told_now = false;
        if let Some(sr) = settings_run.as_mut() {
            let now = menu_tick.floor() as i32;
            while sr.run.frame() < now - sr.first + 1 {
                let at = sr.first + sr.run.frame();
                let due: Vec<_> = menu_keys.iter().filter(|(t, _)| *t > sr.seen && t.floor() as i32 <= at).collect();
                if let Some(last) = due.last() {
                    sr.seen = last.0;
                }
                let keys: Vec<_> = due.into_iter().filter_map(|&(_, k)| screen_key(k)).collect();
                let fx = sr.run.step(&keys);
                if !keys.is_empty() {
                    eprintln!("settings: {keys:?} on the menu's screen (game frame {at}), row {}, changed {}", sr.run.screen.state.cursor[sr.run.screen.state.tab], sr.run.changed());
                }
                if let Some(sound) = sound.as_mut() {
                    let when = sound_base + i64::from(at - a.hud_at as i32);
                    for cue in fx.cues {
                        let (name, id) = cue.sound();
                        sound.request(name, barrel_ryza::sound::Cue::effect(when, id));
                    }
                }
                if fx.saved {
                    eprintln!("settings: saved from the menu's screen (game frame {})", at);
                    save_options(&sr.run.screen.state);
                    apply_settings = true;
                }
                if let (false, Some(l)) = (sr.told, sr.run.left_at()) {
                    told_now = true;
                    eprintln!("settings: left (game frame {})", sound_base + i64::from(sr.first + l - 1));
                    menu_keys.push(((sr.first + l - 1) as f32, barrel_ryza::field_menu::Key::Return));
                    // (the menu's replay takes the keys in the order of their frames; within the frame the screen's own key, which a
                    // window stamps with a fraction of it, comes before the return)
                    menu_keys.sort_by(|x, y| x.0.floor().total_cmp(&y.0.floor()));
                    sr.told = true;
                }
            }
        }
        if !told_now {
            break;
        }
        menu_now = field_menu.as_ref().and_then(|(_, v)| barrel_ryza::field_menu::replay(&menu_keys, menu_tick, &v.card_order(menu_shown.party.len())));
        menu_up = menu_now.is_some();
        }
        // The field stands still while the menu is up: its clock leaves the menu's frames out, and
        // its help bar comes up again as at an arrival from the menu's last frame.
        let menu_spans = barrel_ryza::field_menu::spans(&menu_keys, &field_menu.as_ref().map_or_else(Vec::new, |(_, v)| v.card_order(menu_shown.party.len())));
        let paused: f32 = menu_spans.iter().map(|&(from, to)| (to.map_or(f32::MAX, |t| t + barrel_ryza::field_menu::FIELD_RESUME_LAG).min(menu_tick) - from).max(0.0)).sum();
        let resumed = menu_spans.iter().filter_map(|&(_, to)| to).filter(|&to| to <= menu_tick).last();
        if title_layer.is_none() {
            (showing_options, showing_notice) = (false, false);
        }
        if let (Some((layer, view)), Some(nav)) = (field_menu.as_mut(), menu_now.as_ref()) {
            let (quads, additive, hidden) = view.quads(nav, &menu_shown, menu_tick, (width, height));
            // (a quad only in the trace is the mask of a clip that draws nothing)
            let clips: Vec<text::Clip> = hidden.iter().map(|&mask| text::Clip { mask, quads: 0..0 }).collect();
            layer.set_blended_by(&gpu, &quads, &clips, &|i| additive.contains(&i))?;
            // The blurred menu and the vignettes over it, from the menu's going out to the screen's last frame (and out after it).
            // The first quad is the blurred picture of what the menu drew (the renderer's `screen_blur`); a window whose swapchain cannot be copied from has only the vignettes.
            showing_backdrop = false;
            if let (Some(sr), Some(layer)) = (settings_run.as_ref(), backdrop.as_mut()) {
                let fade = match sr.over_since() {
                    Some(over) => barrel_ryza::field_menu::backdrop_fade(0.0, Some(menu_tick - over as f32)),
                    None => barrel_ryza::field_menu::backdrop_fade(menu_tick - (sr.first as f32 - barrel_ryza::field_menu::BACKDROP_BEFORE), None),
                };
                let mut quads = barrel_ryza::field_menu::backdrop(fade, (width, height));
                if pump.is_some() && !renderer.can_blur_screen() && !quads.is_empty() {
                    quads.remove(0);
                }
                showing_backdrop = !quads.is_empty();
                layer.set(&gpu, &quads)?;
            } else if let (Some(pick), Some(layer)) = (nav.page(menu_tick).or_else(|| nav.page_leaving(menu_tick).filter(|(p, out)| *out < p.backdrop_hold()).map(|(p, _)| p)), backdrop.as_mut()) {
                // The page's blurred picture of the menu and its vignettes come in from the page's first frame.
                let first = pick.confirmed.unwrap_or(0.0) + pick.page_at();
                let mut quads = barrel_ryza::field_menu::backdrop(barrel_ryza::field_menu::backdrop_fade(menu_tick - first, None), (width, height));
                if pump.is_some() && !renderer.can_blur_screen() && !quads.is_empty() {
                    quads.remove(0);
                }
                showing_backdrop = !quads.is_empty();
                layer.set(&gpu, &quads)?;
            } else if let (Some(back), Some(layer)) = (nav.blur_out.map(|back| nav.left.as_ref().and_then(|p| p.leaving.map(|l| l + p.backdrop_hold())).unwrap_or(back)), backdrop.as_mut()) {
                // The menu coming back from a page: the page's blurred picture and vignettes go out from its first frame.
                let mut quads = barrel_ryza::field_menu::backdrop(barrel_ryza::field_menu::backdrop_fade(0.0, Some(menu_tick - back)), (width, height));
                if pump.is_some() && !renderer.can_blur_screen() && !quads.is_empty() {
                    quads.remove(0);
                }
                showing_backdrop = !quads.is_empty();
                layer.set(&gpu, &quads)?;
            }
            // The Equipment page over the blurred menu.
            showing_page = false;
            let (page_now, leaving) = match (nav.page(menu_tick), nav.page_leaving(menu_tick)) {
                (Some(p), _) => (Some(p), None),
                (None, Some((p, out))) => (Some(p), Some(out)),
                _ => (None, None),
            };
            let menu_help = if let (Some(s), true) = (nav.sub_bar(menu_tick), page_now.is_some_and(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Guide))) {
                view.guide_sub_help((width, height), s.button, menu_tick - s.since, s.back.map(|b| menu_tick - b))
            } else if page_now.is_some_and(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Guide)) {
                // (the Guide's bar comes in after its parts have faded)
                view.guide_help((width, height), page_now.and_then(|p| p.confirmed).map_or(0.0, |c| menu_tick - c), leaving)
            } else if page_now.is_some_and(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Tools)) {
                // (the Gathering Tools page's bar comes in late)
                view.tools_help((width, height), page_now.and_then(|p| p.confirmed).map_or(0.0, |c| menu_tick - c), leaving)
            } else {
                view.page_help((width, height), page_now.is_some_and(|p| p.status), page_now.is_some_and(|p| p.slot_since.is_none()), page_now.is_some_and(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Container)), page_now.and_then(|p| p.confirmed).map_or(0.0, |c| menu_tick - c), leaving)
            };
            if let (Some((layer, view)), Some(pick)) = (page.as_mut(), page_now.filter(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Guide))) {
                // The Guide's top: the paper with its cards, the title and the four buttons.
                let cards = match (item_info.as_ref(), party_tables.as_ref()) {
                    (Some(i), Some(t)) => library.cards(i, &know_items, 0, &|n| t.image(n)),
                    _ => Vec::new(),
                };
                let effects = library.known_effects(&know_effects);
                let sub = nav.sub(menu_tick).map(|s| barrel_ryza::field_page::SubLook {
                    page: if s.button == 0 { barrel_ryza::field_page::SubPage::Items(&cards) } else { barrel_ryza::field_page::SubPage::Effects(&effects) },
                    since: menu_tick - s.since,
                    back: s.back.map(|b| menu_tick - b),
                    clock: menu_tick - s.first,
                    rate: if s.button == 0 { library.item_rate(&know_items) } else { library.effect_rate(&know_effects) },
                    cursor: 0,
                });
                let look = barrel_ryza::field_page::GuideLook { cursor: nav.guide_cursor(menu_tick), age: menu_tick - pick.confirmed.unwrap_or(0.0), leaving, moved: nav.guide_moved(menu_tick).map(|m| menu_tick - m), sub };
                let (mut quads, additive, clips) = view.guide_quads(&look, (width, height));
                quads.extend(menu_help.iter().cloned().map(|q| text::Quad { texture: if q.texture == barrel_ryza::field_menu::GUIDE { view.help_texture() } else { q.texture }, ..q }));
                layer.set_blended_by(&gpu, &quads, &clips, &|i| additive.contains(&i))?;
                showing_page = true;
            } else if let (Some((layer, view)), Some(pick), Some(tables), Some(items)) = (page.as_mut(), page_now.filter(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Container)), party_tables.as_ref(), item_info.as_ref()) {
                // The Container page: its grid, the cursor's item as a card.
                let ids: Vec<u8> = menu_shown.party.iter().map(|m| m.id).collect();
                let item = container.0.get(pick.cell).map(|u| items.card(u, &ids, tables.image(u.item)));
                let images: Vec<usize> = container.0.iter().map(|u| tables.image(u.item)).collect();
                let look = barrel_ryza::field_page::ShelfLook {
                    party: &menu_shown.party,
                    images: &images,
                    cursor: pick.cell,
                    tab: 0,
                    free: container.1 - container.0.len(),
                    item: item.as_ref(),
                    age: menu_tick - pick.confirmed.unwrap_or(0.0),
                    leaving,
                };
                let (mut quads, additive, clips) = view.shelf_quads(&look, (width, height));
                quads.extend(menu_help.iter().cloned().map(|q| text::Quad { texture: if q.texture == barrel_ryza::field_menu::GUIDE { view.help_texture() } else { q.texture }, ..q }));
                layer.set_blended_by(&gpu, &quads, &clips, &|i| additive.contains(&i))?;
                showing_page = true;
            } else if let (Some((layer, view)), Some(pick), Some(tables), Some(items)) = (page.as_mut(), page_now.filter(|p| p.items == Some(barrel_ryza::field_menu::ItemPage::Tools)), party_tables.as_ref(), item_info.as_ref()) {
                // The Gathering Tools page: the row of tools, the cursor's tool as a card.
                let ids: Vec<u8> = menu_shown.party.iter().map(|m| m.id).collect();
                let item = tools.get(pick.cell).map(|u| items.card(u, &ids, tables.image(u.item)));
                let images: Vec<usize> = tools.iter().map(|u| tables.image(u.item)).collect();
                let look = barrel_ryza::field_page::ToolsLook { party: &menu_shown.party, images: &images, cursor: pick.cell, item: item.as_ref(), age: pick.page_age(menu_tick), plate_age: menu_tick - pick.confirmed.unwrap_or(0.0), leaving };
                let (mut quads, additive, clips) = view.tools_quads(&look, (width, height));
                quads.extend(menu_help.iter().cloned().map(|q| text::Quad { texture: if q.texture == barrel_ryza::field_menu::GUIDE { view.help_texture() } else { q.texture }, ..q }));
                layer.set_blended_by(&gpu, &quads, &clips, &|i| additive.contains(&i))?;
                showing_page = true;
            } else if let (Some((layer, view)), Some(pick), Some(tables)) = (page.as_mut(), page_now.filter(|p| p.items.is_none()), party_tables.as_ref()) {
                if let Some(member) = menu_shown.party.get(pick.card) {
                    let skills = if pick.status { party_skills.active(usize::from(member.id), member.level, barrel_ryza::field_menu::TACTICS_LEVEL) } else { Vec::new() };
                    let card = item_info.as_ref().zip(member.worn.get(pick.slot).copied().flatten()).map(|(items, unit)| items.card(&unit, &menu_shown.party.iter().map(|m| m.id).collect::<Vec<_>>(), tables.image(unit.item)));
                    let look = barrel_ryza::field_page::Look {
                        item: card.as_ref(),
                        member,
                        party: &menu_shown.party,
                        card: pick.card,
                        item_images: &|i| tables.image(i),
                        slot: pick.slot,
                        slot_age: barrel_ryza::field_page::animation_clock(menu_tick) - barrel_ryza::field_page::animation_clock(pick.slot_since.unwrap_or(pick.confirmed.unwrap_or(0.0) + barrel_ryza::field_page::BLINK_FROM)) + barrel_ryza::field_page::BLINK_LAG,
                        age: menu_tick - pick.confirmed.unwrap_or(0.0),
                        status: pick.status,
                        skills: &skills,
                        row: pick.row,
                        leaving,
                        swap: pick.swap.and_then(|s| Some((menu_shown.party.get(s.from)?.id, menu_tick - s.at + 1.0, s.way))),
                    };
                    let (mut quads, additive, clips) = view.quads(&look, (width, height));
                    // The page's own help bar: the menu's plate and words over the page's textures.
                    quads.extend(menu_help.iter().cloned().map(|q| text::Quad { texture: if q.texture == barrel_ryza::field_menu::GUIDE { view.help_texture() } else { q.texture }, ..q }));
                    layer.set_blended_by(&gpu, &quads, &clips, &|i| additive.contains(&i))?;
                    showing_page = true;
                }
            }
            // What the screen leaves after its last frame (its caption and help bar going out).
            if let (Some(sr), Some((layer, view, strings))) = (settings_run.as_ref().filter(|s| s.over_since().is_some()), options_screen.as_mut()) {
                let age = menu_tick - sr.over_since().unwrap_or(0) as f32;
                let quads = view.remnants(&sr.run.screen.state.screen(strings), age, (width, height));
                layer.set_blended(&gpu, &quads, &[], 0..0)?;
                showing_options = !quads.is_empty();
            }
            // The Settings screen over it, from its first frame.
            if let (Some(sr), Some((layer, view, strings))) = (settings_run.as_ref().filter(|s| s.run.frame() >= 1 && !s.run.is_over()), options_screen.as_mut()) {
                let look = sr.run.look();
                let mut screen = sr.run.screen.state.screen(strings);
                screen.leave = look.clone();
                let (mut quads, clips, glow) = view.quads(&screen, sr.run.moment(), (width, height));
                if look.cover > 0.0 {
                    quads.push(flow::cover(look.cover, (width, height), 0));
                }
                if let (Some((alpha, age)), Some((notice_layer, _, boot))) = (look.notice, load_screen.as_mut()) {
                    notice_layer.set(&gpu, &boot.saving_notice(age as f32 / 60.0, alpha, (width, height)))?;
                    showing_notice = true;
                }
                layer.set_blended(&gpu, &quads, &clips, glow)?;
                showing_options = true;
            }
        } else if let (Some((layer, view)), Some(r)) = (hud.as_mut(), rig.as_ref()) {
            let tick = menu_tick - paused;
            if let (Some(points), true, None) = (&gather_points, pump.is_some(), &gather_plan) {
                if let Some(image) = live_gather.step(points, r.player.at.into(), r.player.turn.to_degrees(), tick, (confirm_pressed, swing_pressed), &mut know_items, &played_events) {
                    gather_hud_items = Some(image).filter(|i| !i.is_empty());
                    gather_gone = live_gather.plan().and_then(|p| p.spot);
                    if let (Some(s), Some(plan)) = (sound.as_mut(), live_gather.plan()) {
                        let now = sound_base + (started.elapsed().as_secs_f32() * barrel_ryza::sound::FPS) as i64;
                        for cue in plan.cues(now) {
                            s.schedule(cue);
                        }
                    }
                }
            }
            gather_busy = live_gather.busy(tick);
            let state = barrel_ryza::field_hud::State {
                at: r.player.at.into(),
                turn: r.player.turn.to_degrees(),
                hour: a.save_hour.unwrap_or(10),
                basket: a.basket.unwrap_or(barrel_ryza::field_hud::BASKET),
                tool: 0,
                map: a.field == barrel_ryza::field_hud::MAP_FIELD,
                camera_yaw: play.map_rotation.then_some(180.0 - yaw),
                tick,
                anim: tick - a.hud_slips.iter().filter(|&&f| tick >= f).count() as f32,
                age: tick - a.hud_arrival,
                place: barrel_ryza::field_hud::place_names(&a.field).filter(|_| !a.no_banner),
                arrival: a.no_banner,
                marks: hud_marks.clone(),
                gather: match &gather_plan {
                    Some(g) => barrel_ryza::field_hud::Gathering { near: (tick >= g.near).then(|| tick - g.near), pressed: (tick >= g.press).then(|| tick - g.press), found: g.found.clone(), notice_at: g.notice_at(), leave_at: g.leave_at(), staff: g.tool == gathering::Tool::Staff },
                    None => live_gather.show(tick),
                },
                help: if let (Some(back), true) = (resumed, pump.is_none()) {
                    let mut bar = barrel_ryza::field_hud::HelpBar::returning();
                    // A button pressed in the field (the menu's keys after it closed) holds the bar back like the stick.
                    let presses: Vec<f32> = menu_keys.iter().filter(|&&(at, key)| at >= back && key != barrel_ryza::field_menu::Key::Open).map(|&(at, _)| at + PAD_LAG).collect();
                    (0..(menu_tick - back).max(0.0) as u32).for_each(|i| bar.step(presses.iter().any(|&from| (from..from + PAD_HOLD).contains(&(back + i as f32))), 1.0));
                    bar
                } else if pump.is_none() && a.walk_from.is_none() {
                    // A still: the bar only where the arrival frame is given (`--hud-arrival`).
                    if a.hud_arrival > 0.0 { replay_help(a.hud_arrival, tick, &a.hud_stick, if a.no_banner { barrel_ryza::field_hud::HelpBar::arriving_at_once() } else { barrel_ryza::field_hud::HelpBar::arriving() }) } else { barrel_ryza::field_hud::HelpBar::arriving() }
                } else {
                    help_bar
                },
            };
            let (quads, clips, glows) = view.quads(&state, &hud_markers, (width, height));
            layer.set_blended_by(&gpu, &quads, &clips, &|i| glows.iter().any(|r| r.contains(&i)))?;
        }
        // The item gathered brings its pictures: the HUD's layer is made again with them.
        if let Some(images) = gather_hud_items.take() {
            hud = Some(hud_layer(&install, &gpu, &renderer, &images)?);
        }
        let layers: Vec<&text::Layer> = match (showing_load, load_screen.as_ref(), showing_options, options_screen.as_ref()) {
            (true, Some((l, _, _)), _, _) => vec![l],
            (_, _, true, Some((l, _, _))) if showing_notice && menu_now.is_none() => std::iter::once(l).chain(load_screen.as_ref().map(|(n, _, _)| n)).collect(),
            (_, _, true, Some((l, _, _))) if menu_now.is_none() => vec![l],
            _ if menu_now.is_some() => field_menu
                .iter()
                .map(|(l, _)| l)
                .chain(backdrop.as_ref().filter(|_| showing_backdrop))
                .chain(page.as_ref().filter(|_| showing_page).map(|(l, _)| l))
                .chain(options_screen.as_ref().filter(|_| showing_options).map(|(l, _, _)| l))
                .chain(load_screen.as_ref().filter(|_| showing_options && showing_notice).map(|(n, _, _)| n))
                .collect(),
            _ => telop_layer.iter().chain(opening_layer.as_ref().map(|(l, _, _)| l)).chain(title_layer.as_ref().map(|(l, _, _, _)| l)).chain(hud.as_ref().map(|(l, _)| l)).collect(),
        };
        // The view-projection drawn with: shifted by this frame's sub-pixel jitter when TAA or a
        // temporal upscaler is on, in a window (as Escha; `settings::jitter`, a clip-space
        // translation by the offset in pixels x 2 / the scene's size). Offscreen pictures are
        // single frames and keep the plain one. The look slots keep the plain one for motion.
        let (draw_vp, frame_jitter) = if renderer.settings.jitter && pump.is_some() {
            let (sw, sh) = renderer.targets.scene_size;
            let (jx, jy) = if renderer.settings.fsr3 { barrel_render::settings::jitter_cycle(jitter_index, barrel_render::fsr3::jitter_phase_count(sw as i32, width as i32) as u32) } else { barrel_render::settings::jitter(jitter_index) };
            jitter_index = jitter_index.wrapping_add(1);
            let mut j = Mat4::IDENTITY;
            j.w_axis.x = 2.0 * jx / sw as f32;
            j.w_axis.y = 2.0 * jy / sh as f32;
            (j * view.vp, (jx, jy))
        } else {
            (view.vp, (0.0, 0.0))
        };
        let same_camera = prev_drawn_vp.is_some_and(|p: Mat4| p.abs_diff_eq(view.vp, 1e-5));
        prev_drawn_vp = Some(view.vp);
        let window_shot = match &presenter {
            Some(p) if p.readable && window_shots.iter().any(|(n, _)| *n == frame_no) => Some(gpu.buffer(u64::from(width) * u64::from(height) * 4, vk::BufferUsageFlags::TRANSFER_DST, true)?),
            _ => None,
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
            far_shown: Some(&far_shown),
            in_sight: &in_sight,
            material: &material,
            draw_vp,
            view_planes: view_planes(&view.vp),
            shadow_vp: view.shadow_vp,
            shadow_cascades: view.shadow_cascades,
            shadow_sides: Some(shadow_maps(renderer.shadow_target.size)),
            shadow_sub: view.shadow_sub.map(|vp| (vp, SUB_BIAS.map(|(slope, constant)| shadow::Bias { slope, constant, alpha_ref: CASCADE_ALPHA_REF }))),
            shadow_bias: view.shadow_cascades[0].map(|_| CASCADE_BIAS.map(|(slope, constant)| shadow::Bias { slope, constant, alpha_ref: CASCADE_ALPHA_REF })),
            alpha_cutoff: MODEL_ALPHA_REF,
            sway_cutoff: PLANT_ALPHA_REF,
            // The set's bloom: `fLumiThreshold` as the bright pass's threshold (on the scene as
            // stored) and `fBloomWeight` as the rate.
            // Bloom off (the game's `BloomType`): no glow, as its Low draws none.
            glow: environment.as_ref().map_or((1.0, 0.0), |(_, _, l, _, _, _, _)| (l.lumi_threshold, if presets_now.get().bloom { l.bloom_weight * GLOW_RANGE } else { 0.0 })),
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
            camera: Camera { jitter: frame_jitter, fov, near: view.near, far: view.far, dt: frame_dt, same: same_camera },
            freeze_now: false,
            frozen_weight: 0.0,
            // Before the title the game shows its boot screens over black: no scene shows.
            overlay: if scene_on.get() { [0.0; 4] } else { [0.0, 0.0, 0.0, 1.0] },
            layers: &layers,
            readback: readback.or(window_shot).map(|(b, _)| b),
            sky_plane: view.sky_plane,
            clouds: view.clouds,
            water_pieces,
            occlusion: true,
            sun_flare: view.sun_flare,
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
                rlr: renderer.settings.rlr,
                meter: frame.meter.as_ref(),
                tone_white: frame.tone_white,
                tone_gamma: frame.tone_gamma,
                tone_scale: frame.tone_scale,
                tone_vivid: frame.tone_vivid,
                near: view.near,
                far: view.far,
                scene_size: renderer.targets.scene_size,
                sun_flare: frame.sun_flare.as_ref(),
                clouds: frame.clouds.as_ref(),
            }))
        });
        // The stage dumps (`BARREL_DUMP_STAGES`): the frame drawn again up to each stage, its
        // scene written; the last, full frame below adds the rest.
        if renderer.dump.is_some() && readback.is_some() && presenter.is_none() && frame_no + 1 >= a.frames.unwrap_or(1) {
            use barrel_render::dump::Stop;
            for stage in [Stop::Lit, Stop::Shadow, Stop::Chara, Stop::Plane, Stop::Layer1, Stop::Dome, Stop::Sky] {
                renderer.upto.set(stage);
                // The shadow stage's look: the shaders write the sun's share (slot 31's w 1), the
                // characters' stage their coverage (2).
                view.look[31][3] = match stage {
                    Stop::Shadow => 1.0,
                    Stop::Chara => 2.0,
                    _ => 0.0,
                };
                renderer.look_ring.write(&gpu, 0, unsafe { std::slice::from_raw_parts(view.look.as_ptr() as *const u8, LOOK_SIZE as usize) });
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
                if let Some((buf, mem)) = window_shot {
                    unsafe { d.device_wait_idle()? };
                    let mut pixels = vec![0u8; (u64::from(width) * u64::from(height) * 4) as usize];
                    unsafe {
                        let src = d.map_memory(mem, 0, pixels.len() as u64, vk::MemoryMapFlags::empty())? as *const u8;
                        std::ptr::copy_nonoverlapping(src, pixels.as_mut_ptr(), pixels.len());
                        d.unmap_memory(mem);
                        d.destroy_buffer(buf, None);
                        d.free_memory(mem, None);
                    }
                    if p.format == vk::Format::B8G8R8A8_UNORM {
                        pixels.chunks_exact_mut(4).for_each(|px| px.swap(0, 2));
                    }
                    for (_, file) in window_shots.iter().filter(|(n, _)| *n == frame_no) {
                        std::fs::write(file, barrel_formats::png::encode_rgba(width, height, &pixels))?;
                        eprintln!("window shot: {file} ({width}x{height}, frame {frame_no})");
                    }
                }
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
        // The window's state a hundred and twenty frames after a live display change, for the check.
        if let (Some((since, n)), Some(p), Some(pr)) = (display_probe.as_mut(), pump.as_ref(), presenter.as_ref()) {
            *n += 1;
            if *n == 120 {
                let w = p.window();
                let ms = since.elapsed().as_secs_f64() * 1000.0 / 120.0;
                eprintln!("display: inner {}x{}, fullscreen {}, swapchain {}x{}, present mode {:?}, mean frame {ms:.2} ms", w.inner_size().width, w.inner_size().height, w.fullscreen().is_some(), pr.extent.width, pr.extent.height, pr.mode);
                display_probe = None;
            }
        }
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
    scene.destroy(&gpu);
    renderer.destroy(&gpu)?;
    let frames_left = a.frames.map(|n| if pump.is_some() { n } else { n.saturating_sub(frame_no).max(1) });
    let carry = Carry { sound: sound.take(), frame: handoff_at, music_at };
    // Into the field the window, device and swapchain go on (the scene's own resources are gone,
    // the device is idle); otherwise they end with the run.
    let app = match (handoff.is_some() || opening_next || free_play_next, pump.take(), presenter.take()) {
        (true, Some(pump), Some(presenter)) => Some(App { pump, gpu, presenter }),
        (_, _, presenter) => {
            if let Some(p) = presenter {
                p.destroy(&gpu);
            }
            unsafe {
                gpu.device.destroy_device(None);
                gpu.instance.destroy_instance(None);
            }
            None
        }
    };
    let next = handoff.map(Next::Save).or(opening_next.then_some(Next::Opening)).or(free_play_next.then_some(Next::FreePlay));
    Ok(next.map(|next| Handoff { root: std::path::PathBuf::from(&a.install), next, app, out: a.out.clone(), size: a.size, frames_left, carry }))
}

/// A debug aid for comparing a frame with the game's: the sky's clock (seconds; the clouds'
/// wind and the water's flow) from `BARREL_SKY_CLOCK`, the cloud shadows' pattern offset
/// (both axes, in the pattern's own widths) from `BARREL_PATTERN_PHASE`, and the cloud map's
/// threshold from `BARREL_CLOUD_THRESHOLD`, in place of the field's own (the game's differ by
/// run: docs/parity.md, "The pixel board"). The same clock turns the windmills and rocks the boat
/// (the capture's sail joints sit where the clock's seconds put them: 7.05 at frame 2150).
/// How far along u a sky dome's layer has slid: the game's `txTrs01.x`, which grows by half
/// the material's `txC0` each second (0.0568 and 0.0435 on two frames 400 apart at 60 a
/// second, for a `txC0` of 0.004; the second layer's 0.002 gives 0.0284 and 0.0217).
fn dome_shift(rate: f32, seconds: f32) -> f32 {
    (rate * 0.5 * seconds).fract()
}

fn phase_override(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.trim().parse().ok()
}

/// A parity tool: `BARREL_CLOUD_MAP=<png>` fills the cloud map (1024 by 1024, 8 bits a channel)
/// from that picture in place of the passes, for the game's own map (its noise lattice is
/// shuffled each run, so its clouds cannot be made again: docs/parity.md, "The pixel board").
fn cloud_map_override() -> Result<Option<Vec<u8>>> {
    let Ok(path) = std::env::var("BARREL_CLOUD_MAP") else { return Ok(None) };
    let png = barrel_formats::png::decode(&std::fs::read(&path)?)?;
    let side = barrel_render::clouds::MAP_SIZE as usize;
    if (png.width, png.height) != (side, side) {
        return Err(format!("BARREL_CLOUD_MAP: {} by {}, not {side} square", png.width, png.height).into());
    }
    let rgba = png.pixels.chunks_exact(png.channels).flat_map(|p| [p[0], p[1], p[2], if png.channels == 4 { p[3] } else { 255 }]).collect();
    Ok(Some(rgba))
}

// STAND-IN: Shift's lean is chosen: a stick lean of this length walks.
/// The lean Shift holds the keys to (the stick's throw, before its dead zone).
const SHIFT_LEAN: f32 = 0.5;
/// The camera's yaw in the game's terms in ryza-recomp's `walk-rasen` (the walk check's `--camera -1.00,8.00,400`).
const WALK_CAMERA_YAW: f32 = -1.0;

/// The field's environment: the diffuse cube map's light, the set's light and its sky fog.
type Environment = ([[f32; 3]; 9], [[f32; 3]; 9], Light, (barrel_ryza::environment::Fog, barrel_ryza::environment::SkyGlow, barrel_ryza::environment::DomeLight, barrel_ryza::environment::Clouds), barrel_ryza::environment::Fog, barrel_ryza::environment::CharaLight, [f32; 2]);

/// A frame's view: the look block and what goes with it.
struct View {
    look: [[f32; 4]; LOOK_SLOTS],
    vp: Mat4,
    shadow_vp: Mat4,
    /// The cascades past the first.
    shadow_cascades: [Option<Mat4>; 2],
    /// The characters' map's view-projection.
    shadow_sub: Option<Mat4>,
    /// The static shadow layer wanted and its look slot (texel, layer to fill).
    shadow_far: Option<(Mat4, [f32; 4])>,
    /// The sun flare's settings (`barrel_ryza::flare`), with the shadows it reads.
    sun_flare: Option<barrel_render::flare::Settings>,
    sky_plane: bool,
    /// The sky's large clouds, when the set has them: what their maps are made from.
    clouds: Option<CloudFrame>,
    near: f32,
    far: f32,
    exposure: Option<f32>,
}

/// A field's water as look slots from `WATER_SLOT` (`water.vert`): {pieces, 0}, then a piece's five:
/// its curve (x mirrored into render space) with its end widths, the body's refraction (the
/// refractive index less 1, and how far the light goes through it) in the `w` of the curve's
/// last two terms, and its body's colour and reflectance at normal incidence.
fn water_slots(bodies: &[barrel_ryza::river::Body]) -> Vec<[f32; 4]> {
    let mut slots = vec![[0.0; 4]];
    for body in bodies {
        for p in body.pieces() {
            if slots.len() + 5 > 1 + 5 * MAX_WATER_PIECES {
                break;
            }
            let mirror = |v: [f32; 3], w: f32| [-v[0], v[1], v[2], w];
            slots.extend([mirror(p.a, p.widths[0]), mirror(p.b, p.widths[1]), mirror(p.c, body.refractive_index - 1.0), mirror(p.d, body.transparency), [body.albedo[0], body.albedo[1], body.albedo[2], body.reflectance[0]]]);
        }
    }
    slots[0][0] = ((slots.len() - 1) / 5) as f32;
    slots
}

/// The water slots of the pieces that may be in view (the game draws only those): each
/// piece boxed by its two end points padded by their widths, against the view's side and near
/// planes (not the far one: the sea runs to the horizon past it). STAND-IN: the game's own box
/// is not read (it draws one river piece fewer in the turn view).
fn water_in_view(slots: &[[f32; 4]], planes: &[glam::Vec4; 6]) -> Vec<[f32; 4]> {
    let mut kept = vec![[0.0; 4]];
    for piece in slots[1..].chunks_exact(5) {
        let [a, b, c, d] = [0, 1, 2, 3].map(|k| Vec3::from_slice(&piece[k][..3]));
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (p, w) in [(d, piece[0][3]), (a + b + c + d, piece[1][3])] {
            lo = lo.min(p - Vec3::new(w, 0.0, w));
            hi = hi.max(p + Vec3::new(w, 0.0, w));
        }
        let seen = planes[..5].iter().all(|p| {
            let far = Vec3::new(if p.x >= 0.0 { hi.x } else { lo.x }, if p.y >= 0.0 { hi.y } else { lo.y }, if p.z >= 0.0 { hi.z } else { lo.z });
            p.truncate().dot(far) + p.w >= 0.0
        });
        if seen {
            kept.extend_from_slice(piece);
        }
    }
    kept[0] = [((kept.len() - 1) / 5) as f32, slots[0][1], slots[0][2], slots[0][3]];
    kept
}

/// The view from `dist` away from `center` (render space) at `yaw` and `pitch` degrees, and
/// its look block (`barrel_render`'s `Look`): the eye, the view-projection and last frame's
/// (`prev_vp`; this one's when there is none), and the environment's light, sky and the sun's
/// shadow (`shadow_size`: the shadow map's, none without shadows; `shadow_reach`: the preset's
/// scale of the far layer's reach, `clip` the near and far planes where the scene fixes them). With no environment, a
/// white ambient alone (the terrain shows its colours as they are).
#[allow(clippy::too_many_arguments)]
fn look_for(sub_box: Option<(Vec3, Vec3)>, center: Vec3, probe: Vec3, yaw: f32, pitch: f32, dist: f32, fov: f32, (width, height): (u32, u32), prev_vp: Option<Mat4>, environment: Option<&Environment>, flare_look: &barrel_ryza::flare::Look, shadow_size: Option<u32>, shadow_reach: f32, (world, terrain_box): ((Vec3, Vec3), bool), clip: Option<(f32, f32)>, seconds: f32) -> View {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    let eye = center + Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist;
    let view = Mat4::look_at_rh(eye, center, Vec3::Y);
    // The game's projection: near 50 and far 300000 (its depth terms at the gate: the nearest pixel at z 315 has depth 0.8417, the sky at 1 unprojects to 300036), so the backdrop's mountains out to 123000 units are not cut.
    let (near, far) = clip.unwrap_or((50.0, 300_000.0));
    let mut proj = Mat4::perspective_rh(fov, width as f32 / height as f32, near, far);
    proj.y_axis.y *= -1.0;
    proj.x_axis.x *= -1.0;
    let vp = proj * view;
    let mut look = [[0f32; 4]; LOOK_SLOTS];
    let mut shadow_vp = vp;
    let mut shadow_far = None;
    let mut shadow_cascades = [None; 2];
    let mut shadow_sub = None;
    let mut sun_flare = None;
    let eye_point = eye;
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
        // The cube map's light as the characters take it (`indirectChara_`: the map's scale and tint).
        let s = chara.specular_map;
        look[CHARA_SPECULAR_SLOT] = [s * chara.specular_map_color[0], s * chara.specular_map_color[1], s * chara.specular_map_color[2], k];
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
        // The sun's shadow as the game's pass sets it (docs/shading.md, "The ambient-and-shadow
        // pass"): three cascades, each the smallest box in the sun's view around a slice of the
        // view's frustum (along the view: from 0 to the first end, to the second, to the last),
        // then the static layer over the whole field past them; unless --no-shadows.
        if let Some(size) = shadow_size {
            let sun = from_light(light.parallel[0].elevation, light.parallel[0].azimuth);
            let forward = (center - eye).normalize_or(Vec3::NEG_Z);
            let inverse = vp.inverse();
            // The point at `z` along the view on the ray through the screen's (nx, ny).
            let at = |nx: f32, ny: f32, z: f32| {
                let near_point = inverse.project_point3(Vec3::new(nx, ny, 0.0));
                let way = (inverse.project_point3(Vec3::new(nx, ny, 1.0)) - near_point).normalize_or(forward);
                eye_point + way * (z / way.dot(forward))
            };
            let ends = CASCADE_ENDS.map(|e| e * shadow_reach);
            let plane = eye_point.dot(sun) - CASTER_PLANE;
            let slice = |from: f32, to: f32| {
                let mut points = [Vec3::ZERO; 8];
                for (k, point) in points.iter_mut().enumerate() {
                    *point = at((k & 1) as f32 * 2.0 - 1.0, (k >> 1 & 1) as f32 * 2.0 - 1.0, if k < 4 { from } else { to });
                }
                shadow::fit_view_proj(&points, sun, plane)
            };
            // The first cascade starts at the near plane (the game's box round the slice from z 50
            // to 1600 maps onto its map exactly; from the eye it is 0.4% off).
            shadow_vp = slice(near, ends[0]);
            shadow_cascades = [Some(slice(ends[0], ends[1])), Some(slice(ends[1], ends[2]))];
            sun_flare = Some(barrel_ryza::flare::settings(flare_look, probe, shadow_vp, view, sun, width as f32 / height as f32));
            for (k, col) in shadow_vp.to_cols_array_2d().into_iter().enumerate() {
                look[27 + k] = col;
            }
            look[31] = [1.0 / size as f32, 1.0, 1.0, 0.0];
            for (k, [w, h]) in shadow_maps(size).into_iter().take(4).enumerate() {
                look[SHADOW_SIDES_SLOT + k] = [w as f32 / size as f32, h as f32 / size as f32, 1.0 / w as f32, 1.0 / h as f32];
            }
            for (c, m) in shadow_cascades.iter().enumerate() {
                for (k, col) in m.unwrap_or_default().to_cols_array_2d().into_iter().enumerate() {
                    look[SHADOW_CASCADE_SLOT + 4 * c + k] = col;
                }
            }
            look[SHADOW_CASCADE_SLOT + 8] = [ends[0], ends[1], ends[2], CASCADE_BLEND * shadow_reach];
            look[SHADOW_CASCADE_SLOT + 9] = [forward.x, forward.y, forward.z, 0.0];
            // The characters' map: the smallest box in the sun's view around the characters' box.
            if let Some((lo, hi)) = sub_box {
                let (lo, hi) = (lo - SUB_BOX_PAD, hi + SUB_BOX_PAD);
                let corners: [Vec3; 8] = std::array::from_fn(|k| Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }));
                let vp = shadow::fit_view_proj(&corners, sun, f32::MAX);
                let side = shadow_maps(size)[shadow::SUB_LAYER][0] as f32;
                for (k, col) in vp.to_cols_array_2d().into_iter().enumerate() {
                    look[SHADOW_SUB_SLOT + k] = col;
                }
                look[SHADOW_SUB_SLOT + 4] = [1.0 / side, side / size as f32, 1.0, shadow::SUB_LAYER as f32];
                shadow_sub = Some(vp);
            }
            // The static layer, drawn once: the game's box (`mSemiStaticW2S`) is the smallest in
            // the sun's view round the terrain's square at heights 0 to STATIC_TOP, reaching
            // STATIC_DEPTH toward the sun from its far side; with no terrain, the whole scene's.
            let corners: [Vec3; 8] = std::array::from_fn(|k| Vec3::new(if k & 1 == 0 { world.0.x } else { world.1.x }, if k & 2 == 0 { world.0.y } else { world.1.y }, if k & 4 == 0 { world.0.z } else { world.1.z }));
            let plane = if terrain_box { corners.iter().map(|c| c.dot(sun.normalize())).fold(f32::MIN, f32::max) - STATIC_DEPTH } else { f32::MAX };
            shadow_far = Some((shadow::fit_view_proj(&corners, sun, plane), [1.0 / size as f32, 0.0, 0.0, 0.0]));
        }
        exposure_k = Some(k);
    }
    View { look, vp, shadow_vp, shadow_cascades, shadow_sub, shadow_far, sun_flare, sky_plane, clouds, near, far, exposure: exposure_k }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn the_settings_come_to_the_same_at_the_start_and_after_a_change() {
        // A config with Barrel's engine rows: the render settings take them over the desktop's.
        let config = "scale 0.75\nmsaa 4\n";
        let start = Resolved::from(None, Some(config), None, &none);
        assert_eq!((start.graphics.scale, start.graphics.msaa), (0.75, 4));
        assert!(start.graphics.hdr_scene && start.graphics.reflections);
        // Without the game's tables the High presets stand.
        assert_eq!(start.presets, barrel_ryza::presets::Graphics::HIGH);
        // The same text again (what the save wrote) comes to the same settings.
        let again = Resolved::from(None, Some(config), None, &none);
        assert_eq!((start.graphics, start.presets), (again.graphics, again.presets));
        // A changed row shows in the next resolve only.
        let changed = Resolved::from(None, Some("scale 1\nmsaa 1\n"), None, &none);
        assert_ne!(changed.graphics, start.graphics);
        assert_eq!(changed.graphics.msaa, 1);
        // --preset wins over the config's render rows, as at the start.
        let preset = Resolved::from(Some(RenderSettings::DESKTOP), Some(config), None, &none);
        assert_eq!(preset.graphics.scale, RenderSettings::DESKTOP.scale);
    }
}
