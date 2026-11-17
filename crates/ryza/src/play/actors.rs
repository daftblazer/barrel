//! Characters an event stages (the title's Ryza, Lent and Tao): each one's model, skeleton and
//! motions read from the archives, and posed from the stage's body (where it stands, how it is
//! turned, the motion it plays and since when). Also the shared step of putting a posed character
//! into the scene with her toon-ramp rows, which the walking Ryza uses too.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use barrel_engine::stage::{Body, Playback};
use barrel_formats::elixir::Elixir;
use barrel_formats::g1a::Animation;
use barrel_formats::g1m::{G1m, Geometry, Skeleton, Submesh};
use barrel_formats::g1t::G1t;
use barrel_render::{render_space, Draw, Instance, Scene};
use barrel_ryza::install::Install;
use glam::{Mat4, Quat, Vec3};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The title's event: its characters, their turns and motions (docs/renderer.md, "The backdrop").
pub const TITLE_EVENT: &str = "tt01_010";

// STAND-IN: The event's start (EVENT_START) is fitted to a capture's frames; the real thing is the game's own start of the event on its clock.
/// Seconds of the title that pass before the event's clock starts. Fitted to the `title-cycle`
/// capture's frames 1400, 1500 and 1700 (title time 4.33, 6.0 and 9.3 s; the clock starts 1140
/// frames in): the game reads the event at frame 1152 and shows the scene at 1155.
pub const EVENT_START: f32 = 35.0 / 60.0;

/// A character's files, read: its model container and texture pack as stored, and the motions
/// the event plays on it.
pub struct Files {
    slot: u32,
    tag: String,
    size: [f32; 3],
    model: Vec<u8>,
    pack: Vec<u8>,
    anims: BTreeMap<u32, Animation>,
    /// The hip height each motion of a shared set is made for.
    bases: BTreeMap<u32, f32>,
    face: Option<Animation>,
}

/// A character of the stage, parsed from its [`Files`].
pub struct Actor<'a> {
    pub slot: u32,
    pub size: [f32; 3],
    pub skeleton: Skeleton,
    pub geometry: Geometry<'a>,
    pub pack: G1t<'a>,
    anims: &'a BTreeMap<u32, Animation>,
    bases: &'a BTreeMap<u32, f32>,
    // STAND-IN: The hair and skirt joints stay where their parents carry them; the real thing is the game's cloth and swing simulation.
    /// The joints no motion or expression moves off their bind pose (the hair and skirt, which
    /// the game's cloth and swing simulation moves): the draw trace marks them, as Barrel
    /// leaves them where their parents carry them.
    pub swing: Vec<u32>,
    /// The default expression laid over the body's pose.
    face: Option<barrel_ryza::face::Face>,
    /// Its instance in the scene, once added.
    pub instance: u32,
}

impl Files {
    // STAND-IN: Only the playable characters are drawn: the event's other characters and every weapon are not; the real thing is drawing them all.
    /// Reads the character of event slot `slot` (a row of `chara.xml`) with `motions`, or None
    /// when its tag is not a playable character's (`PC..`: the others are not drawn yet).
    pub fn read(install: &Install, slot: u32, motions: &BTreeSet<u32>) -> Result<Option<Files>> {
        let (chara, base) = (install.read("\\saves\\chara\\chara.xml")?, install.read("\\saves\\chara\\charabase.xml")?);
        let tag = barrel_ryza::chara::row_tag(&chara, slot as usize)?;
        let set = tag.to_ascii_lowercase();
        if !(tag.starts_with("PC") && tag.len() == 5) {
            return Ok(None);
        }
        let size = barrel_ryza::chara::Chara::read(&chara, &base, &tag)?.size;
        let dir = format!("\\data\\x64\\character\\{set}\\{set}_model");
        let (model, pack) = (install.read(&format!("{dir}.elixir.gz"))?, install.read(&format!("{dir}.g1t"))?);
        let folders = barrel_ryza::chara::motion_sets(&base, &install.read("\\saves\\chara\\charamotionset.xml")?, &tag)?;
        let first = |bytes: &[u8]| -> Result<Animation> {
            let c = Elixir::parse(bytes)?;
            let entry = c.entries.iter().find(|e| !e.is_dummy()).ok_or("empty motion")?;
            Ok(barrel_formats::g1a::parse(c.data(entry))?)
        };
        let (mut anims, mut bases) = (BTreeMap::new(), BTreeMap::new());
        for &n in motions {
            match folders.iter().find_map(|f| install.read(&barrel_ryza::motion::path(&f.dir, n)).ok().map(|b| (b, f))) {
                Some((bytes, set)) => {
                    anims.insert(n, first(&bytes)?);
                    bases.extend(set.skeleton_base.map(|b| (n, b)));
                }
                None => eprintln!("{tag}: motion {n} is in none of {folders:?}"),
            }
        }
        let face = install.read(&barrel_ryza::face::path(&tag, "expression_default")).ok().and_then(|b| first(&b).ok());
        eprintln!("{tag} (slot {slot}): motions {:?} from {folders:?}", anims.keys().collect::<Vec<_>>());
        Ok(Some(Files { slot, tag, size, model, pack, anims, bases, face }))
    }

    /// Its model container, unpacked (the actor's mesh borrows from it).
    pub fn container(&self) -> Result<Elixir<'_>> {
        Ok(Elixir::parse(&self.model)?)
    }

    /// The actor, its mesh read from `model` (this character's [`Files::container`]).
    pub fn open<'a>(&'a self, model: &'a Elixir<'a>) -> Result<Actor<'a>> {
        let named = |name: String| barrel_engine::field::named(model, &name).ok_or(format!("no {name}"));
        let skeleton = G1m::parse(named(format!("{}_MODEL.g1m", self.tag))?)?.skeleton.ok_or("the model has no skeleton")?;
        let geometry = G1m::parse(named(format!("{}_MODEL_default.g1m", self.tag))?)?.geometry.ok_or("the mesh has no geometry")?;
        let face = self.face.as_ref().map(|a| barrel_ryza::face::Face::new(&skeleton, a));
        let swing = unmoved_joints(&skeleton, self.anims.values(), face.as_ref());
        Ok(Actor { slot: self.slot, size: self.size, skeleton, geometry, pack: G1t::parse(&self.pack)?, anims: &self.anims, bases: &self.bases, swing, face, instance: 0 })
    }
}

impl Actor<'_> {
    // STAND-IN: Ryza's head look-at is not applied, so her head follows only her motion; the real thing is the game's look-at turning it toward its target.
    /// Its joints' skinning matrices for `body` at stage time `now`.
    pub fn joints(&self, body: &Body, now: f32) -> Vec<Mat4> {
        let playing = body.motion.as_ref();
        let number = playing.map(|p| p.motion).filter(|n| self.anims.contains_key(n)).or_else(|| self.anims.keys().next().copied());
        let still = Animation { duration: 0.0, tracks: Vec::new() };
        let anim = number.and_then(|n| self.anims.get(&n)).unwrap_or(&still);
        let t = playing.map_or(0.0, |p| p.elapsed(now));
        let mut locals = barrel_engine::skin::local_pose(&self.skeleton, anim, if anim.duration > 0.0 { t % anim.duration } else { 0.0 });
        // A motion of a set shared by a body type is made for a hip `base` high: it moves this
        // model's hip in proportion to the model's own (measured: the title's Tao, hip 80.1,
        // plays a motion for 98 and stands 17.9 lower than the motion's own hip).
        if let (Some(&base), Some(hip)) = (number.and_then(|n| self.bases.get(&n)), self.skeleton.joint(barrel_ryza::motion::HIP)) {
            locals[hip].2.y *= self.skeleton.joints[hip].position[1] / base;
        }
        if let Some(f) = &self.face {
            f.apply(&mut locals, None, 0.0);
        }
        barrel_engine::skin::skin(&self.skeleton, &locals)
    }

    /// Its model matrix for `body` (game space: scaled, turned about y, moved).
    pub fn model(&self, body: &Body) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::from(self.size), Quat::from_rotation_y(body.facing), Vec3::from(body.position))
    }

    /// Poses its instance for `body` at `now` in the scene's instances and palette (render space).
    pub fn pose(&self, body: &Body, now: f32, instances: &mut [Instance], palette: &mut [Mat4]) {
        let base = instances[self.instance as usize].palette as usize;
        for (slot, j) in palette[base..].iter_mut().zip(self.joints(body, now)) {
            *slot = render_space(j);
        }
        let i = &mut instances[self.instance as usize];
        i.prev_model = i.model;
        i.model = render_space(self.model(body));
    }
}

/// The motions each slot plays anywhere in the event's steps (the ones to load).
pub fn motions_of(steps: &[barrel_engine::event::Timed]) -> BTreeMap<u32, BTreeSet<u32>> {
    let mut out: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for t in steps {
        if let barrel_engine::event::Step::Motion { character, motion, .. } = &t.step {
            out.entry(*character).or_default().insert(*motion);
        }
    }
    out
}

/// Runs the event's stage on to the tick nearest `seconds` (it never runs back; a time on a tick's
/// edge, like the capture frame 1400's 3.7333 s, must not flip with float noise).
pub fn advance(play: &mut Playback, seconds: f32) {
    while play.stage.time + barrel_engine::stage::FRAME * 0.5 < seconds {
        play.tick();
    }
}

/// A posed character's submeshes added to the scene as `name`, with their toon-ramp rows
/// (`ramp_rows`, by first index and instance), her hair highlights, and her textures shifted by
/// `texture_base` (where her pack starts among the scene's textures; `ramp` is the toon ramp's
/// texture number). Returns the number of draws added.
#[allow(clippy::too_many_arguments)]
pub fn add_character(
    scene: &mut Scene,
    geometry: &Geometry,
    texture_count: usize,
    joints: &[Mat4],
    instance: u32,
    texture_base: usize,
    ramp: usize,
    name: &str,
    swing: &[u32],
    ramp_rows: &mut HashMap<(u32, u32), (u32, bool)>,
) -> Result<usize> {
    let first = scene.draws.len();
    scene.add_posed(geometry, Mat4::IDENTITY, texture_count, Some(joints), instance)?;
    // Her submeshes' ramp rows (`rampIndex`), one draw each in order for those with UVs and
    // normals (the others draw nothing, `Scene::add_posed`).
    let drawn: Vec<&Submesh> = geometry.submeshes.iter().filter(|s| geometry.simple_mesh(s).is_ok_and(|m| !m.uvs.is_empty() && !m.normals.is_empty())).collect();
    let rows: Vec<(u32, bool)> = drawn.iter().map(|s| (geometry.param_record(s, "rampIndex").and_then(|p| p.integer()).unwrap_or(1).min(255), geometry.param_record(s, "rmIndex").is_some())).collect();
    // The hair's highlight (`sAlbedoRetouch2HighlightMap`, texture kind 66): laid over the hair's
    // colour by its alpha. The game shifts it along v by `dot(V, alr2HDir) x alr2HAdj` (-0.11 at
    // most, about 0.015 at the field camera's pitch): laid on unshifted (docs/shading.md).
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
        eprintln!("{name}: {} draws for {} submeshes: no toon ramp", scene.draws.len() - first, rows.len());
    }
    for d in &mut scene.draws[first..] {
        let ramped = ramp_rows.contains_key(&(d.first_index, d.instance));
        d.texture += texture_base;
        d.mask = if ramped { Some(ramp) } else { d.mask.map(|m| m + texture_base) };
        d.overlay = d.overlay.map(|o| o + texture_base);
        d.normal_map = d.normal_map.map(|n| n + texture_base);
        d.emission = d.emission.map(|e| e + texture_base);
        d.field = Some(0);
        d.name = name.into();
        d.casts = true;
        for b in d.bones.iter_mut().filter(|b| swing.contains(b)) {
            *b |= Draw::SIMULATED;
        }
    }
    Ok(scene.draws.len() - first)
}

/// The joints that stay at their bind pose in every one of `anims` (sampled eight times over each)
/// with the expression laid on.
fn unmoved_joints<'a>(skeleton: &Skeleton, anims: impl Iterator<Item = &'a Animation>, face: Option<&barrel_ryza::face::Face>) -> Vec<u32> {
    let bind = barrel_engine::skin::local_pose(skeleton, &Animation { duration: 0.0, tracks: Vec::new() }, 0.0);
    let mut moved = vec![false; bind.len()];
    for anim in anims {
        for k in 0..8 {
            let mut pose = barrel_engine::skin::local_pose(skeleton, anim, anim.duration * k as f32 / 8.0);
            if let Some(f) = face {
                f.apply(&mut pose, None, 0.0);
            }
            for (j, (now, was)) in pose.iter().zip(&bind).enumerate() {
                moved[j] |= now.2.distance(was.2) > 1e-3 || now.0.distance(was.0) > 1e-4 || now.1.dot(was.1).abs() < 1.0 - 1e-6;
            }
        }
    }
    (0..moved.len() as u32).filter(|&j| !moved[j as usize]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_engine::event::{Step, Timed};
    use barrel_engine::stage::Stage;

    #[test]
    fn motions_are_collected_by_slot() {
        let motion = |character, motion| Timed { time: 0.0, action: 0, step: Step::Motion { action: 0, character, motion, blend: 0.0, looping: false, division: None } };
        let steps = [motion(0, 120), motion(2, 2190), motion(0, 92511), motion(0, 120)];
        let by = motions_of(&steps);
        assert_eq!(by[&0].iter().copied().collect::<Vec<_>>(), [120, 92511]);
        assert_eq!(by[&2].iter().copied().collect::<Vec<_>>(), [2190]);
        assert!(!by.contains_key(&3));
    }

    /// The stage runs to the tick nearest the time, not past it: 225 ticks for 3.75 s however
    /// the float noise falls (the title's capture frame 1400).
    #[test]
    fn the_stage_runs_to_the_nearest_tick() {
        for seconds in [3.75, 3.7500002, 3.7499998] {
            let mut play = Playback::new(Stage::default(), Vec::new());
            advance(&mut play, seconds);
            assert!((play.stage.time - 3.75).abs() < 1e-3, "{seconds}: {}", play.stage.time);
            advance(&mut play, 1.0);
            assert!((play.stage.time - 3.75).abs() < 1e-3);
        }
    }
}
