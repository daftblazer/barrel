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

// STAND-IN: The event's start (EVENT_START) is fitted to a capture's frames; the real thing is the game's loading between the event's first commands and its placements (40 frames in the capture's run), which Barrel does not time.
/// Seconds of the title that pass before the event's clock starts. In the game's run the
/// event's first command is at capture frame 874 (a frame before the title's first), its
/// characters load until 917 and their motions are set then; Barrel's player sets them 3
/// frames into the event (the first fade's hold), so 38 frames puts them a frame from there.
/// Fitted: at 38 the bones are within 0.012 of the capture's at frames 1400 and 1500 (0.1 and
/// more a frame either side).
pub const EVENT_START: f32 = 38.0 / 60.0;

/// A character's files, read: its model container and texture pack as stored, and the motions
/// the event plays on it.
pub struct Files {
    slot: u32,
    tag: String,
    /// What the model container's entries are named after (`<entry>_MODEL.g1m`): the tag, for a
    /// townsperson its character's (`NPC30B`).
    entry: String,
    size: [f32; 3],
    model: Vec<u8>,
    pack: Vec<u8>,
    anims: BTreeMap<u32, Animation>,
    /// The hip height each motion of a shared set is made for.
    bases: BTreeMap<u32, f32>,
    face: Option<Animation>,
    /// For a prop (a weapon lying in the scene as a character of its own, never moved): its model
    /// matrix, from the position and three angles its `q5` adds it with.
    prop: Option<Mat4>,
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
    /// A prop's model matrix ([`Files::read`]).
    prop: Option<Mat4>,
    /// The event's turns of its head ([`head_turns`]): when, and the command's three angles.
    pub head: Vec<(f32, [f32; 3])>,
}

/// `s88` `{character, a, b, c}`: turns the character's head by whole degrees. The title's
/// `s88 0 -7 0 0` holds Ryza's head 7 degrees about the model's x axis off her motion's (the
/// capture's head matrix against the motion's: the same x axis, y and z turned 6.97 degrees).
pub const HEAD_TURN: u32 = 0x7300_0058;
/// The head's bone, as the motions number it (joint 6 of the playable characters' skeletons).
const HEAD: u32 = 6;

/// Each character's head turns among an event's commands, in time order.
pub fn head_turns(taken: &[barrel_ryza::event::Taken]) -> BTreeMap<u32, Vec<(f32, [f32; 3])>> {
    let mut out: BTreeMap<u32, Vec<(f32, [f32; 3])>> = BTreeMap::new();
    for (time, _, words) in taken {
        if let [HEAD_TURN, character, a, b, c, ..] = words[..] {
            out.entry(character).or_default().push((*time, [a, b, c].map(|v| v as i32 as f32)));
        }
    }
    out
}

impl Files {
    // STAND-IN: Only the playable characters and the weapons lying in the scene are drawn: the event's other characters are not (the title's event has none); the real thing is drawing them all, with held weapons.
    /// Reads the character of event slot `slot` (a row of `chara.xml`) with `motions`, or None
    /// when its tag is neither a playable character's (`PC00A`) nor a weapon's (`PC00_WEP_01`:
    /// a prop with no motion, placed as the event's `q5` adds it: `added`, [`barrel_ryza::event::added`]).
    pub fn read(install: &Install, slot: u32, motions: &BTreeSet<u32>, added: Option<&barrel_ryza::event::Added>) -> Result<Option<Files>> {
        let (chara, base) = (install.read("\\saves\\chara\\chara.xml")?, install.read("\\saves\\chara\\charabase.xml")?);
        let tag = barrel_ryza::chara::row_tag(&chara, slot as usize)?;
        let set = tag.to_ascii_lowercase();
        let root = format!("\\data\\x64\\character\\{set}\\{set}");
        if let (Some(a), true) = (added, tag.starts_with("PC") && tag.get(4..).is_some_and(|rest| rest.starts_with("_WEP_"))) {
            let (model, pack) = (install.read(&format!("{root}.elixir.gz"))?, install.read(&format!("{root}.g1t"))?);
            let size = barrel_ryza::chara::size(&chara, &tag)?;
            eprintln!("{tag} (slot {slot}): a prop at {:?} turned {:?}", a.position, a.degrees);
            let prop = Some(Mat4::from_scale_rotation_translation(Vec3::from(size), barrel_ryza::event::rotation(a.degrees), Vec3::from(a.position)));
            return Ok(Some(Files { slot, entry: tag.clone(), tag, size, model, pack, anims: BTreeMap::new(), bases: BTreeMap::new(), face: None, prop }));
        }
        if !(tag.starts_with("PC") && tag.len() == 5) {
            return Ok(None);
        }
        let size = barrel_ryza::chara::Chara::read(&chara, &base, &tag)?.size;
        let dir = format!("{root}_model");
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
        Ok(Some(Files { slot, entry: tag.clone(), tag, size, model, pack, anims, bases, face, prop: None }))
    }

    /// The townsperson `tag` (`NPC30B`, [`barrel_ryza::npc::Npc::character`]) with the motion `motion` it plays looped: its model,
    /// pack and that motion, found in the character's motion sets (`motset0`, then `motset1`).
    /// One set of files serves every placement of the same character.
    pub fn read_npc(install: &Install, tag: &str, motions: &BTreeSet<u32>) -> Result<Files> {
        let (chara, base) = (install.read("\\saves\\chara\\chara.xml")?, install.read("\\saves\\chara\\charabase.xml")?);
        // A character (`NPC24B`) is a base's model (`NPC24A`, whose `chara.xml` rows give the size
        // and whose `charabase.xml` row the motion sets) in the texture pack of its own folder
        // (the base's rows say `LoadTexture="NPC24B/NPC24B_MODEL"`).
        let base_tag = format!("{}A", &tag[..tag.len() - 1]);
        let size = barrel_ryza::chara::size(&chara, &base_tag)?;
        let (own, shared) = (tag.to_ascii_lowercase(), base_tag.to_ascii_lowercase());
        let model = install.read(&format!("\\data\\x64\\character\\{shared}\\{shared}_model.elixir.gz"))?;
        let pack = install.read(&format!("\\data\\x64\\character\\{own}\\{own}_model.g1t"))?;
        let folders = barrel_ryza::chara::motion_sets(&base, &install.read("\\saves\\chara\\charamotionset.xml")?, &base_tag)?;
        let (mut anims, mut bases) = (BTreeMap::new(), BTreeMap::new());
        for &motion in motions {
            let found = folders.iter().find_map(|f| install.read(&barrel_ryza::motion::path(&f.dir, motion)).ok().map(|b| (b, f)));
            match found {
                Some((bytes, set)) => {
                    let c = Elixir::parse(&bytes)?;
                    let entry = c.entries.iter().find(|e| !e.is_dummy()).ok_or("empty motion")?;
                    anims.insert(motion, barrel_formats::g1a::parse(c.data(entry))?);
                    bases.extend(set.skeleton_base.map(|b| (motion, b)));
                }
                None => eprintln!("{tag}: motion {motion} is in none of {folders:?}"),
            }
        }
        Ok(Files { slot: 0, entry: base_tag, tag: tag.to_string(), size, model, pack, anims, bases, face: None, prop: None })
    }

    /// The gimmick model `tag` (`FM_BOARD_01`, [`barrel_ryza::gimmick::Gimmick::tag`]): a static
    /// object in `charabase.xml`'s files for it (a folder of its own, entries named after the
    /// file stem), placed by its callers: its matrix is the identity.
    pub fn read_gimmick(install: &Install, tag: &str) -> Result<Files> {
        let (chara, base) = (install.read("\\saves\\chara\\chara.xml")?, install.read("\\saves\\chara\\charabase.xml")?);
        // STAND-IN: The notice boards (`FM_BOARD_01`, `_02`) have no row of their own and are both drawn as the town's `A21_H_BOARD_02`; the real thing is the game's own name for them. The capture draws both Rasenboden boards with that mesh (draw sizes 60, 2583, 7119, 450) and its textures: `_01`'s mesh and `_03`'s (one draw smaller) do not match.
        let alias = tag.starts_with("FM_BOARD_").then(|| "A21_H_BOARD_02".to_string());
        let mut found = None;
        for t in [Some(tag.to_string()), alias].into_iter().flatten() {
            if let Some(files) = barrel_ryza::chara::model_files(&base, &t)? {
                found = Some((t, files));
                break;
            }
        }
        let (row, (model, texture)) = found.ok_or_else(|| format!("{tag}: no charabase row"))?;
        let size = barrel_ryza::chara::size(&chara, &row)?;
        let path = |name: &str| format!("\\data\\x64\\character\\{}", name.to_ascii_lowercase().replace('/', "\\"));
        let (m, pack) = (install.read(&format!("{}.elixir.gz", path(&model)))?, install.read(&format!("{}.g1t", path(&texture)))?);
        // The entries are named after the file stem, but a copy of a model keeps its original's
        // (`A21_H_BOARD_03`'s are `-02`): read the name off the mesh entry.
        let stem = Elixir::parse(&m)?.entries.iter().find_map(|e| e.name.strip_suffix("_default.g1m").map(str::to_string)).ok_or_else(|| format!("{model}: no mesh entry"))?;
        Ok(Files { slot: 0, entry: stem, tag: tag.to_string(), size, model: m, pack, anims: BTreeMap::new(), bases: BTreeMap::new(), face: None, prop: Some(Mat4::IDENTITY) })
    }

    /// The character's tag.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Its model container, unpacked (the actor's mesh borrows from it).
    pub fn container(&self) -> Result<Elixir<'_>> {
        Ok(Elixir::parse(&self.model)?)
    }

    /// The actor, its mesh read from `model` (this character's [`Files::container`]).
    pub fn open<'a>(&'a self, model: &'a Elixir<'a>) -> Result<Actor<'a>> {
        let named = |name: String| barrel_engine::field::named(model, &name).ok_or_else(|| format!("no {name} among {:?}", model.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>()));
        // A person's entries are `<TAG>_MODEL`, a prop's the bare `<TAG>`.
        let suffix = if self.prop.is_some() { "" } else { "_MODEL" };
        let skeleton = G1m::parse(named(format!("{}{suffix}.g1m", self.entry))?)?.skeleton.ok_or("the model has no skeleton")?;
        let mut geometry = G1m::parse(named(format!("{}{suffix}_default.g1m", self.entry))?)?.geometry.ok_or("the mesh has no geometry")?;
        place_cloths(&mut geometry, &skeleton);
        let face = self.face.as_ref().map(|a| barrel_ryza::face::Face::new(&skeleton, a));
        let swing = unmoved_joints(&skeleton, self.anims.values(), face.as_ref());
        Ok(Actor { slot: self.slot, size: self.size, skeleton, geometry, pack: G1t::parse(&self.pack)?, anims: &self.anims, bases: &self.bases, swing, face, instance: 0, prop: self.prop, head: Vec::new() })
    }
}

impl Actor<'_> {
    // STAND-IN: Only the first angle of a head turn (`s88`) is applied, about the model's x axis, and at once; the real thing is all three angles as the game applies them, and `s90`'s timed turn.
    /// Its joints' skinning matrices for `body` at stage time `now`.
    pub fn joints(&self, body: Option<&Body>, now: f32) -> Vec<Mat4> {
        let playing = body.and_then(|b| b.motion.as_ref());
        self.joints_of(playing.map(|p| p.motion), playing.map_or(0.0, |p| p.elapsed(now)), now)
    }

    /// Its joints' skinning matrices playing motion `motion` (or its first, when it has not that
    /// one) `t` seconds in; `now` is the stage time the head's turns are looked up at.
    pub fn joints_of(&self, motion: Option<u32>, t: f32, now: f32) -> Vec<Mat4> {
        let number = motion.filter(|n| self.anims.contains_key(n)).or_else(|| self.anims.keys().next().copied());
        let still = Animation { duration: 0.0, tracks: Vec::new() };
        let anim = number.and_then(|n| self.anims.get(&n)).unwrap_or(&still);
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
        // The event's latest turn of the head, laid over the motion's.
        if let (Some(&(_, [a, ..])), Some(head)) = (self.head.iter().rev().find(|(at, _)| *at <= now), self.skeleton.joint(HEAD)) {
            // about the model's x axis as the head has it in the bind pose (the capture's head
            // matrix and the motion's share their x column)
            let mut bind = Quat::IDENTITY;
            let mut at = Some(head);
            while let Some(j) = at {
                bind = Quat::from_array(self.skeleton.joints[j].rotation).normalize() * bind;
                at = self.skeleton.joints[j].parent;
            }
            locals[head].1 *= bind.inverse() * Quat::from_rotation_x(a.to_radians()) * bind;
        }
        barrel_engine::skin::skin(&self.skeleton, &locals)
    }

    /// Whether it is a prop (a weapon lying in the scene), which has no body on the stage.
    pub fn is_prop(&self) -> bool {
        self.prop.is_some()
    }

    /// Its model matrix for `body` (game space: scaled, turned about y, moved; a prop's is its
    /// own, fixed by where its `q5` adds it, and takes no body).
    pub fn model(&self, body: Option<&Body>) -> Mat4 {
        match (self.prop, body) {
            (Some(prop), _) => prop,
            (None, Some(b)) => Mat4::from_scale_rotation_translation(Vec3::from(self.size), Quat::from_rotation_y(b.facing), Vec3::from(b.position)),
            (None, None) => Mat4::IDENTITY,
        }
    }

    /// Its model matrix (a gimmick's size) at `position`, turned `yaw` degrees about the
    /// vertical, then by `rotation` (degrees about x, y, z) and scaled by `scale`.
    pub fn gimmick_model(&self, position: Vec3, yaw: f32, rotation: [f32; 3], scale: [f32; 3]) -> Mat4 {
        let turn = Quat::from_rotation_y(yaw.to_radians()) * barrel_ryza::event::rotation(rotation);
        Mat4::from_scale_rotation_translation(Vec3::from(self.size) * Vec3::from(scale), turn, position)
    }

    /// Its model matrix standing at `position`, turned `yaw` radians about the vertical.
    pub fn placed(&self, position: Vec3, yaw: f32) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::from(self.size), Quat::from_rotation_y(yaw), position)
    }

    /// Poses its instance (a townsperson standing where it was placed) `t` seconds into its
    /// looped motion `motion`.
    pub fn pose_standing(&self, instance: u32, motion: u32, t: f32, instances: &mut [Instance], palette: &mut [Mat4]) {
        let base = instances[instance as usize].palette as usize;
        for (slot, j) in palette[base..].iter_mut().zip(self.joints_of(Some(motion), t, 0.0)) {
            *slot = render_space(j);
        }
        let i = &mut instances[instance as usize];
        i.prev_model = i.model;
    }

    /// Poses its instance for `body` at `now` in the scene's instances and palette (render space).
    pub fn pose(&self, body: &Body, now: f32, instances: &mut [Instance], palette: &mut [Mat4]) {
        let base = instances[self.instance as usize].palette as usize;
        for (slot, j) in palette[base..].iter_mut().zip(self.joints(Some(body), now)) {
            *slot = render_space(j);
        }
        let i = &mut instances[self.instance as usize];
        i.prev_model = i.model;
        i.model = render_space(self.model(Some(body)));
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
// STAND-IN: The fade's two presets are the colours the capture shows (2 clear to black, 3 black to clear); the real thing is the game's own table of contrast presets.
/// How much of the screen the event's fade covers in black (`u49`, [`Stage::fade`]): 1 before
/// the event's first fade (the title comes up black), then from one of the preset's ends to
/// the other, a step a frame (the game's quad: `rate` k/90 on the k-th frame of a 1.5 s fade,
/// the first being the command's own; the stage has the command a frame before).
pub fn cover(stage: &barrel_engine::stage::Stage) -> f32 {
    let Some((preset, seconds, since)) = stage.fade else { return 1.0 };
    let (from, to) = match preset {
        2 => (0.0, 1.0),
        3 => (1.0, 0.0),
        _ => return 0.0,
    };
    let (frames, length) = (((stage.time - since) * 60.0).round(), (seconds * 60.0).round());
    let rate = if length <= 0.0 { 1.0 } else { (frames / length).clamp(0.0, 1.0) };
    from + (to - from) * rate
}

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
    // STAND-IN: The ramp's and the occlusion's constants (model.frag) are one capture's, Rasenboden at noon; the real thing is wherever the game keeps them a field, time and weather.
    // The occlusion maps (`sOcc`, texture kind 5), in the emissive slot (a character's draws
    // have no emission map of their own drawn): for the submeshes the toon ramp shades (those
    // with a `rampIndex`; a plain piece's program samples no occlusion though its material
    // names one).
    let occlusions: Vec<Option<usize>> = drawn.iter().map(|s| geometry.materials.get(s.material as usize).filter(|_| geometry.param(s, "rampIndex").is_some()).and_then(|m| m.textures.iter().find(|t| t.kind == 5)).map(|t| t.index as usize)).collect();
    if scene.draws.len() - first == rows.len() {
        for ((d, &row), occlusion) in scene.draws[first..].iter_mut().zip(&rows).zip(&occlusions) {
            ramp_rows.insert((d.first_index, d.instance), row);
            d.emission = *occlusion;
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
        d.specular = None;
        d.field = Some(0);
        d.name = name.into();
        d.casts = true;
        for b in d.bones.iter_mut().filter(|b| swing.contains(b)) {
            *b |= Draw::SIMULATED;
        }
    }
    Ok(scene.draws.len() - first)
}

// STAND-IN: The cloth pieces hang at rest, carried by their parent joints; the real thing is the game's cloth simulation moving their control points.
/// Move a model's cloth pieces' points from their parent joints' space into the model's, so
/// its cloth submeshes are built where they hang at rest (`Geometry::cloth_of`).
pub fn place_cloths(geometry: &mut Geometry, skeleton: &Skeleton) {
    let bind = barrel_engine::skin::bind_world(skeleton);
    for cloth in &mut geometry.cloths {
        if let Some((j, m)) = skeleton.joint(cloth.parent_bone).and_then(|j| Some((j, bind.get(j)?))) {
            cloth.place(j as u32, [m.row(0).into(), m.row(1).into(), m.row(2).into()]);
        }
    }
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
