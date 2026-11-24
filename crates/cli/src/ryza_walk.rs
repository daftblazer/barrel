//! `barrel-tool walk`: Ryza walked through a field without a window or a GPU, to check the
//! walk's rules (`barrel_ryza::walk`) against the game's own state a frame at a time
//! (`tools/walkdiff.py`). The pad's presses are read from a script, one a line:
//! `<frame> <buttons> [<frames held>]`, the buttons joined by `+`, a stick as
//! `LX`, `LY`, `RX` or `RY` `=<-32768..32767>` (up is positive); a press at frame F held N
//! frames acts on the frames F to F+N-1. The state is written a frame a line,
//! `frame x y z yaw | ex ey ez camyaw pitch`: her feet and facing (degrees), the camera's eye,
//! its yaw and pitch. Frames run at exactly 1/60 s.

use std::fmt::Write as _;
use std::path::Path;

use barrel_formats::elixir::Elixir;
use barrel_ryza::walk::{Camera, Sticks};
use glam::Vec3;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// A stick's full throw in the pad's units.
const STICK_MAX: f32 = 32767.0;

/// The sticks held on each frame from a pad script.
pub struct Pad {
    presses: Vec<(u32, u32, Sticks)>,
}

impl Pad {
    pub fn parse(text: &str) -> Result<Pad> {
        let mut presses = Vec::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let mut words = line.split_whitespace();
            let frame: u32 = words.next().ok_or("pad: no frame")?.parse().map_err(|_| format!("pad: not a frame: {line}"))?;
            let buttons = words.next().ok_or(format!("pad: no buttons: {line}"))?;
            let held: u32 = words.next().map(|w| w.parse()).transpose().map_err(|_| format!("pad: not a count: {line}"))?.unwrap_or(1);
            let mut sticks = Sticks::default();
            for b in buttons.split('+') {
                let Some((name, v)) = b.split_once('=') else { continue };
                let v = v.parse::<f32>().map_err(|_| format!("pad: not a number: {b}"))?.clamp(-32768.0, STICK_MAX) / STICK_MAX;
                match name {
                    "LX" => sticks.left[0] = v,
                    "LY" => sticks.left[1] = v,
                    "RX" => sticks.right[0] = v,
                    "RY" => sticks.right[1] = v,
                    _ => {}
                }
            }
            presses.push((frame, held.max(1), sticks));
        }
        Ok(Pad { presses })
    }

    /// The sticks held on `frame`: the presses active then, each stick's value added.
    pub fn sticks(&self, frame: u32) -> Sticks {
        let mut s = Sticks::default();
        for (_, _, p) in self.presses.iter().filter(|(f, n, _)| (*f..*f + *n).contains(&frame)) {
            s.left = [s.left[0] + p.left[0], s.left[1] + p.left[1]];
            s.right = [s.right[0] + p.right[0], s.right[1] + p.right[1]];
        }
        s
    }

    /// The frame after the last press ends.
    pub fn end(&self) -> u32 {
        self.presses.iter().map(|(f, n, _)| f + n).max().unwrap_or(0)
    }
}

/// `walk INSTALL FIELD --pad FILE --from N --start X,Y,Z,YAW --camera YAW,PITCH,DIST --state OUT [--to N]`.
pub fn run(args: &[&str]) -> Result<()> {
    let [root, field, rest @ ..] = args else { return Err("walk INSTALL FIELD --pad FILE --from N --start X,Y,Z,YAW --camera YAW,PITCH,DIST --state OUT [--to N]".into()) };
    let (mut pad, mut from, mut to, mut start, mut camera, mut out, mut bare) = (None, 0u32, None, None, None, None, false);
    let numbers = |v: &str, n: usize| -> Result<Vec<f32>> {
        let v: Vec<f32> = v.split(',').map(|x| x.trim().parse::<f32>()).collect::<std::result::Result<_, _>>()?;
        if v.len() == n { Ok(v) } else { Err(format!("{v:?}: {n} numbers wanted").into()) }
    };
    let mut it = rest.iter();
    while let Some(&flag) = it.next() {
        let value = it.next().copied().ok_or(format!("{flag}: no value"))?;
        match flag {
            "--pad" => pad = Some(value),
            "--from" => from = value.parse()?,
            "--to" => to = Some(value.parse::<u32>()?),
            "--start" => start = Some(numbers(value, 4)?),
            "--camera" => camera = Some(numbers(value, 3)?),
            "--state" => out = Some(value),
            "--bare" => bare = value == "1",
            _ => return Err(format!("walk: unknown {flag}").into()),
        }
    }
    let pad = Pad::parse(&std::fs::read_to_string(pad.ok_or("walk: --pad")?)?)?;
    let start = start.ok_or("walk: --start")?;
    let camera = camera.ok_or("walk: --camera")?;
    let out = out.ok_or("walk: --state")?;
    let to = to.unwrap_or(pad.end() + 60);

    // The field, as `--ryza` play loads it.
    let install = barrel_ryza::install::Install::open(Path::new(root))?;
    let files = install.field(field)?;
    let (main, objects) = (Elixir::parse(&files.main)?, Elixir::parse(&files.objects)?);
    let barrel_ryza::walk::Terrain { ground, mut walls, start: at } = barrel_ryza::walk::load_terrain(&install, field, &files.area, &main, &objects, Vec3::new(start[0], start[1], start[2]))?;
    if bare {
        walls = None;
    }
    let chara = barrel_ryza::chara::Chara::read(&install.read("\\saves\\chara\\chara.xml")?, &install.read("\\saves\\chara\\charabase.xml")?, "PC00A")?;
    let free = install.read(&barrel_ryza::camera::FreeCamera::path(field)).ok().and_then(|b| barrel_ryza::camera::FreeCamera::parse(&b).ok()).unwrap_or(barrel_ryza::camera::FreeCamera::DEFAULT);
    let mut player = barrel_ryza::player::Player::new(at, start[3].to_radians(), &[]);
    (player.radius, player.height) = (chara.radius, chara.height);
    let mut cam = Camera::new(camera[0], camera[1], camera[2], free.pitch_range).following();

    let wrap = |degrees: f32| (degrees + 180.0).rem_euclid(360.0) - 180.0;
    let mut text = String::new();
    for frame in from..=to {
        barrel_ryza::walk::step(&mut player, &mut cam, &pad.sticks(frame), 1.0 / 60.0, ground.as_ref(), walls.as_ref());
        let eye = cam.eye(player.at, cam.distance());
        let p = player.at;
        writeln!(text, "{frame} {:.2} {:.2} {:.2} {:.2} | {:.2} {:.2} {:.2} {:.2} {:.2}", p.x, p.y, p.z, wrap(player.turn.to_degrees()), eye.x, eye.y, eye.z, wrap(cam.yaw), cam.pitch)?;
    }
    std::fs::write(out, text)?;
    Ok(())
}

/// `walk-near INSTALL FIELD X Z RADIUS`: the placed collision shapes (hulls, meshes, boxes) whose box lies
/// within RADIUS of the point (x, z): their box and, for a shape of 40 vertices or fewer, its vertices.
pub fn near(args: &[&str]) -> Result<()> {
    let [root, field, x, z, r] = args else { return Err("walk-near INSTALL FIELD X Z RADIUS".into()) };
    let (x, z, r): (f32, f32, f32) = (x.parse()?, z.parse()?, r.parse()?);
    let install = barrel_ryza::install::Install::open(Path::new(root))?;
    let files = install.field(field)?;
    let (main, objects) = (Elixir::parse(&files.main)?, Elixir::parse(&files.objects)?);
    let placements = barrel_engine::field::named(&main, &format!("{}.opl2", files.area)).map(barrel_formats::opl2::parse).transpose()?.ok_or("no placements")?;
    let (solids, _) = barrel_ryza::solids::placed_meshes(&placements, &objects);
    for (k, c) in solids.iter().enumerate() {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &c.vertices {
            for a in 0..3 {
                lo[a] = lo[a].min(v[a]);
                hi[a] = hi[a].max(v[a]);
            }
        }
        if hi[0] < x - r || lo[0] > x + r || hi[2] < z - r || lo[2] > z + r {
            continue;
        }
        println!("shape {k}: {} triangles, {} vertices, x {:.1} to {:.1}, y {:.1} to {:.1}, z {:.1} to {:.1}", c.triangles.len(), c.vertices.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
        if c.vertices.len() <= 40 {
            for v in &c.vertices {
                println!("    {:.1} {:.1} {:.1}", v[0], v[1], v[2]);
            }
        }
    }
    Ok(())
}

/// `walk-ground INSTALL FIELD STATE FROM TO`: the game's feet along a state file against the ground Barrel
/// builds: the highest surface at or under her feet (plus 2) and the highest of all.
pub fn ground(args: &[&str]) -> Result<()> {
    let [root, field, state, from, to] = args else { return Err("walk-ground INSTALL FIELD STATE FROM TO".into()) };
    let (from, to): (u32, u32) = (from.parse()?, to.parse()?);
    let install = barrel_ryza::install::Install::open(Path::new(root))?;
    let files = install.field(field)?;
    let (main, objects) = (Elixir::parse(&files.main)?, Elixir::parse(&files.objects)?);
    let t = barrel_ryza::walk::load_terrain(&install, field, &files.area, &main, &objects, Vec3::ZERO)?;
    let g = t.ground.ok_or("no ground")?;
    for line in std::fs::read_to_string(state)?.lines() {
        let v: Vec<f32> = line.replace('|', " ").split_whitespace().filter_map(|w| w.parse().ok()).collect();
        if v.len() < 5 || (v[0] as u32) < from || (v[0] as u32) > to {
            continue;
        }
        let (x, y, z) = (v[1], v[2], v[3]);
        let round = |h: f32| (h * 100.0).round() / 100.0;
        println!("{} y {:.2} under {:?} highest {:?}", v[0], y, g.height(x, z, y, 2.0).map(round), g.height(x, z, 1e6, 0.0).map(round));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pad_script_holds_presses_over_their_frames() {
        let pad = Pad::parse("10 A 6\n20 LX=-32768+LY=32767 3\n22 RX=16384\n").unwrap();
        assert_eq!(pad.sticks(19).left, [0.0; 2]);
        let s = pad.sticks(20);
        assert_eq!(s.left, [-32768.0 / 32767.0, 1.0]);
        assert_eq!(pad.sticks(22).right[0], 16384.0 / 32767.0);
        assert_eq!(pad.sticks(23).left, [0.0; 2]);
        // A press without a count lasts a frame.
        assert_eq!(pad.sticks(23).right, [0.0; 2]);
        assert_eq!(pad.end(), 23);
    }

    #[test]
    fn a_bad_line_is_an_error() {
        assert!(Pad::parse("x A").is_err());
        assert!(Pad::parse("5 LX=left").is_err());
    }
}
