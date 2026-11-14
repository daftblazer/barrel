//! `barrel-ryza`: Atelier Ryza DX on Barrel. For now it draws one field offscreen:
//!
//!     barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--yaw DEG] [--pitch DEG]
//!                 [--look-at X,Y,Z] [--distance D] [--time morning|noon|evening|night]
//!                 [--weather sunny|cloudy|foggy] [--hide NAME]...
//!
//! FIELD is a field's folder name in the archives (`s01_i_attic`). The camera frames the field's
//! ground (its terrain's square; the backdrop's mountains far outside it are drawn, not framed),
//! or looks at X,Y,Z (field units, as the placement list gives them) from D away. The light is
//! the field environment's set for the time and weather (noon, sunny by default). `--hide`
//! leaves out the objects whose placement names contain NAME (to find which object is which). Every draw is shown unlit
//! (its texture as it is) until Ryza's own lighting is read (`docs/ryza/open.md`).

use std::path::Path;

use ash::vk;
use barrel_formats::elixir::Elixir;
use barrel_formats::g1t::G1t;
use barrel_render::renderer::{Camera, Frame, Renderer};
use barrel_render::settings::RenderSettings;
use barrel_render::*;
use barrel_ryza::environment::{Light, Time, Weather};
use barrel_ryza::install::Install;
use glam::{Mat4, Vec3};

/// The renderer's material bit for a draw shown with its texture alone, unlit (`model.frag`).
const UNLIT: u32 = 32;

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
}

fn args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let usage = "usage: barrel-ryza INSTALL FIELD OUT.png [--size WxH] [--yaw DEG] [--pitch DEG] [--look-at X,Y,Z] [--distance D] [--time T] [--weather W]";
    let (install, field, out) = (it.next().ok_or(usage)?, it.next().ok_or(usage)?, it.next().ok_or(usage)?);
    let mut a = Args { install, field, out, size: (1280, 720), yaw: 0.0, pitch: 30.0, look_at: None, distance: None, time: Time::Noon, weather: Weather::Sunny, hide: Vec::new() };
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

/// One exposure for the frame, standing in for the game's adaptation (`trueHDR_`, not read):
/// what maps the light on an upward-facing surface of average albedo (0.25) to the set's
/// middle gray.
fn exposure(c: &[[f32; 3]; 9], light: &Light) -> f32 {
    let luma = |v: [f32; 3]| 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
    let up = barrel_ryza::environment::basis([0.0, 1.0, 0.0]);
    let sky: [f32; 3] = std::array::from_fn(|ch| (0..9).map(|i| c[i][ch] * up[i]).sum::<f32>() * light.ambient[ch]);
    let sun = light.parallel.iter().map(|p| luma(p.color) * p.elevation.to_radians().sin().max(0.0)).sum::<f32>();
    let lit = (luma(sky) + sun) * 0.25;
    if lit > 0.0 { light.middle_gray / lit } else { 1.0 }
}

fn main() -> Result<()> {
    let a = args()?;
    let install = Install::open(Path::new(&a.install))?;
    let files = install.field(&a.field)?;
    let (main, objects) = (Elixir::parse(&files.main)?, Elixir::parse(&files.objects)?);
    let mut scene = Scene::new();
    scene.field = Some(0);
    // No tag is read yet: nothing casts a shadow or sways (`docs/ryza/open.md`).
    let place = |_: &barrel_formats::opl2::PlacedType, _: &barrel_formats::opl2::Instance| barrel_engine::field::Placement::default();
    let (pack, _) = barrel_engine::field::load(&files.area, &main, &objects, &mut scene, 0, &place)?;
    let pack = G1t::parse(&pack)?;
    let textures: Vec<&barrel_formats::g1t::Texture> = pack.textures.iter().collect();
    let Scene { vertices, indices, mut draws, terrains, lo, hi, instances, palette, deltas, morph_weights, .. } = scene;
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
            let light = Light::read(bin, a.time, a.weather)?;
            eprintln!("light ({:?}, {:?}): {light:?}", a.time, a.weather);
            Some((c, light))
        }
        None => None,
    };
    eprintln!("{} draws, {} vertices, {} triangles, {} textures", draws.len(), vertices.len(), indices.len() / 3, textures.len());

    let (width, height) = a.size;
    let gpu = Gpu::new(None)?;
    let mut renderer = Renderer::new(&gpu, RenderSettings::DESKTOP.resolved(), None, width, height, false, false)?;
    let d = &gpu.device;
    let vbytes = unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * size_of::<Vertex>()) };
    let ibytes = unsafe { std::slice::from_raw_parts(indices.as_ptr() as *const u8, indices.len() * 4) };
    let (uploaded, outline_tex, white) = upload_textures(&gpu, renderer.pool, &textures, None)?;
    let grounds = upload_grounds(&gpu, renderer.pool, &terrains)?;
    let data = scene_gpu::SceneData { vertices: vbytes, indices: ibytes, textures: uploaded, instances: &instances, palette: &palette, deltas: &deltas, weights: &morph_weights, grounds, patterns: vec![None], white };
    let scene = scene_gpu::SceneGpu::new(&gpu, data, &renderer.shared)?;

    // The camera: on the given point, or framing the ground (render space mirrors x: the
    // terrain spans x -extent..0, z 0..extent), or everything when there is no terrain.
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
    let fov = 30f32.to_radians();
    let dist = a.distance.unwrap_or(radius / (fov * 0.5).sin() * 1.05).max(1.0);
    let (y, p) = (a.yaw.to_radians(), a.pitch.to_radians());
    let eye = center + Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist;
    let view = Mat4::look_at_rh(eye, center, Vec3::Y);
    let mut proj = Mat4::perspective_rh(fov, width as f32 / height as f32, dist * 0.01, (dist * 20.0).max(80_000.0));
    proj.y_axis.y *= -1.0;
    proj.x_axis.x *= -1.0;
    let vp = proj * view;
    // The look block (`barrel_render`'s `Look`): the eye, and a white ambient alone (the
    // terrain, which has no unlit flag, shows its colours as they are); no sun, no fog.
    let mut look = [[0f32; 4]; LOOK_SLOTS];
    look[4] = [eye.x, eye.y, eye.z, 1.0];
    look[26] = [1.0, 1.0, 1.0, 1.0];
    // The environment's light instead, when the field has one: the cube map's light times the
    // ambient colour (model.vert's `sh`), the sun and the second light as the field's two
    // directional lights, all scaled by one exposure (`exposure`).
    if let Some((c, light)) = environment {
        let k = exposure(&c, &light);
        look[66] = [1.0, 0.0, 0.0, 0.0];
        for (i, [r, g, b]) in c.into_iter().enumerate() {
            look[67 + i] = [r * light.ambient[0] * k, g * light.ambient[1] * k, b * light.ambient[2] * k, 0.0];
        }
        for (slot, p) in [22usize, 24].into_iter().zip(light.parallel) {
            let d = from_light(p.elevation, p.azimuth);
            look[slot] = [d.x, d.y, d.z, 0.0];
            look[slot + 1] = [p.color[0] * k, p.color[1] * k, p.color[2] * k, 1.0];
        }
        eprintln!("exposure {k:.4}");
    }
    renderer.look_ring.write(&gpu, 0, unsafe { std::slice::from_raw_parts(look.as_ptr() as *const u8, LOOK_SIZE as usize) });

    let bytes = u64::from(width) * u64::from(height) * 4;
    let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
    let hidden = vec![false; instances.len()];
    let shown = |d: &Draw| !a.hide.iter().any(|h| d.name.contains(h.as_str()));
    // Lit as the field's models: the environment's light on their vertex colour (no sun yet).
    let material = |_: &Draw| (if environment.is_some() { 0 } else { UNLIT }, 0);
    let layers: Vec<&text::Layer> = Vec::new();
    let frame = Frame {
        image: 0,
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
        draw_vp: vp,
        view_planes: view_planes(&vp),
        shadow_vp: vp,
        alpha_cutoff: 0.3,
        glow: (1.0, 0.0),
        camera: Camera { fov, near: dist * 0.01, far: (dist * 20.0).max(80_000.0), ..Camera::default() },
        freeze_now: false,
        frozen_weight: 0.0,
        overlay: [0.0; 4],
        layers: &layers,
        readback: Some(readback),
    };
    let mut sync = ring::Frames::new(&gpu)?;
    let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(renderer.pool).command_buffer_count(1))?[0] };
    unsafe { d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))? };
    let drawn = renderer.record(&gpu, cb, &frame);
    unsafe { d.end_command_buffer(cb)? };
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

    unsafe {
        d.device_wait_idle()?;
        d.destroy_buffer(readback, None);
        d.free_memory(rmem, None);
    }
    sync.destroy(&gpu, renderer.pool);
    scene.destroy(&gpu);
    renderer.destroy(&gpu)?;
    unsafe {
        d.destroy_device(None);
        gpu.instance.destroy_instance(None);
    }
    Ok(())
}
