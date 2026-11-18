//! A KTGL field into a renderer scene: its terrain (`<AREA>.ktf`), the objects its placement list
//! (`<AREA>.opl2`) puts down with their `_hi` models from the field's object container, and its
//! texture pack (`<AREA>.g1t`), as Escha & Logy DX and the Ryza games keep them. What a
//! placement's tags mean (which objects cast shadows, sway, or show only in some versions) is
//! the game's, given as `place`.

use barrel_formats::elixir::Elixir;
use barrel_formats::g1a::Animation;
use barrel_formats::g1m::{G1m, Skeleton};
use barrel_formats::g1t::G1t;
use barrel_formats::opl2::{self, Instance, PlacedType};
use barrel_formats::ktf;
use barrel_render::{Scene, TerrainDraw};
use std::rc::Rc;
use glam::{EulerRot, Mat4, Quat, Vec3};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// What the game makes of one placed object.
#[derive(Clone, Copy, Debug, Default)]
pub struct Placement {
    /// Its own instance, for the game to move (a tree's spring); else baked into the scene.
    pub own_instance: bool,
    /// It casts the sun's shadow.
    pub casts: bool,
    /// It shows only in this version of the field (`Draw::version`).
    pub version: Option<u32>,
    /// Its draw class (`Draw::class`).
    pub class: u8,
    /// Also its `_mid` and `_low` models where the container has them, each draw marked with
    /// its level (`Draw::lod`) for the game to pick by distance.
    pub lods: bool,
    /// It plays the motion the object container, or the game's container of motions shared
    /// between fields, has under its name (`<model>.g1a`), looped.
    pub moves: bool,
}

/// A placed object that plays its motion (a windmill's sails, a moored boat, a tree's leaves).
pub struct Moving {
    pub instance: u32,
    pub skeleton: Rc<Skeleton>,
    pub motion: Rc<Animation>,
}

impl Moving {
    /// Its joints `time` seconds into the field (the motion looped), to the scene's palette.
    pub fn pose(&self, time: f32, instances: &[barrel_render::Instance], palette: &mut [Mat4]) {
        let at = if self.motion.duration > 0.0 { time % self.motion.duration } else { 0.0 };
        let base = instances[self.instance as usize].palette as usize;
        for (slot, j) in palette[base..].iter_mut().zip(crate::skin::pose(&self.skeleton, &self.motion, at)) {
            *slot = barrel_render::render_space(j);
        }
    }
}

/// What `load` gives back of a field.
pub struct Loaded {
    /// Its texture pack.
    pub textures: Vec<u8>,
    /// Its light texture (`<AREA>_pat.g1t`; indoor fields carry a 64-byte stub, taken as none).
    pub pattern: Option<Vec<u8>>,
    /// Its objects that play a motion.
    pub moving: Vec<Moving>,
}

/// Where a placement puts its model: scale, then rotate about x, y and z in that order, then
/// move.
pub fn placement_transform(i: &opl2::Instance) -> Mat4 {
    let rotation = Quat::from_euler(EulerRot::ZYX, i.rotation[2], i.rotation[1], i.rotation[0]);
    Mat4::from_scale_rotation_translation(Vec3::from(i.scale), rotation, Vec3::from(i.position))
}

/// Where a collision file's shape sits within its model: turned by its rotation (the same
/// angle rule as a placement's), then moved by its offset.
pub fn collision_within(c: &opl2::CollisionFile) -> Mat4 {
    let r = c.rotation;
    Mat4::from_rotation_translation(Quat::from_euler(EulerRot::ZYX, r[2], r[1], r[0]), Vec3::from(c.offset))
}

/// A container's entry by name (dummies passed over).
pub fn named<'a>(container: &'a Elixir, name: &str) -> Option<&'a [u8]> {
    container.entries.iter().find(|e| e.name == name && !e.is_dummy()).map(|e| container.data(e))
}

/// A field's ground and objects into the scene, as field `scene.field`, its textures taken to
/// start at `base` in the scene's list. `shared` is the game's container of motions for the
/// objects many fields place (Ryza's trees), looked in after the field's own.
pub fn load(area: &str, main: &Elixir, objects: &Elixir, shared: Option<&Elixir>, scene: &mut Scene, base: usize, place: &dyn Fn(&PlacedType, &Instance) -> Placement) -> Result<Loaded> {
    let first_draw = scene.draws.len();
    let textures = named(&main, &format!("{area}.g1t")).ok_or("field has no texture pack")?.to_vec();
    let pattern = named(&main, &format!("{area}_pat.g1t")).filter(|b| G1t::parse(b).is_ok_and(|g| !g.textures.is_empty())).map(<[u8]>::to_vec);
    let texture_count = G1t::parse(&textures)?.textures.len();
    // A flat placeholder terrain (every height 0, one layer: TOWN_03, TOWN_07) is not drawn: the
    // town's ground is its models, and events stand characters below it (TOWN_07's at y -255).
    let terrain = named(&main, &format!("{area}.ktf")).map(|b| ktf::parse(b).map(|t| (b, t))).transpose()?.filter(|(_, t)| {
        let flat = t.heightmap.heights.iter().all(|&v| v == 0.0) && t.layer_list().len() <= 1;
        if flat {
            eprintln!("{area}: flat placeholder terrain, not drawn");
        }
        // A heightmap alone has nothing to draw (the Ryza games' model-built fields).
        let bare = t.layers.textures.is_empty();
        if bare {
            eprintln!("{area}: heightmap-only terrain, not drawn");
        }
        !flat && !bare
    });
    if let Some((bytes, t)) = terrain {
        let h = &t.heightmap;
        let mesh = ktf::mesh(bytes)?;
        let patches = scene.add_terrain(&mesh, &t.layer_repeats());
        let index_count: u32 = patches.iter().map(|p| p.index_count).sum();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for z in 0..=h.cells_z {
            for x in 0..=h.cells_x {
                let v = h.height(x, z);
                (lo, hi) = (lo.min(v), hi.max(v));
            }
        }
        eprintln!(
            "{area}: terrain {}x{} cells of {}, heights {lo}..{hi}, {} layers, {} maps, {} triangles",
            h.cells_x,
            h.cells_z,
            h.cell_size,
            t.layer_list().len(),
            t.blend.textures.len(),
            index_count / 3
        );
        scene.terrains.push((
            scene.field.unwrap_or(0),
            TerrainDraw { patches, cell: mesh.cell_size, extent: [h.cells_x as f32 * h.cell_size, h.cells_z as f32 * h.cell_size], ktf: bytes.to_vec() },
        ));
    }
    let placements = opl2::parse(named(&main, &format!("{area}.opl2")).ok_or("field has no placement list")?)?;
    let (mut placed, mut missing) = (0usize, 0usize);
    let mut moving = Vec::new();
    for t in &placements.objects {
        let Some(bytes) = named(&objects, &format!("{}_hi.g1m", t.name)) else {
            missing += t.instances.len();
            continue;
        };
        let model = G1m::parse(bytes)?;
        let Some(geometry) = model.geometry.as_ref() else { continue };
        // Its lower levels of detail, for a game that draws them (`Placement::lods`).
        // Its motion, when a container has one for it and the model has joints to move.
        let motion_name = format!("{}.g1a", t.name);
        let motion = named(&objects, &motion_name).or_else(|| shared.and_then(|c| named(c, &motion_name))).and_then(|b| barrel_formats::g1a::parse(b).ok()).zip(model.skeleton.clone()).map(|(a, s)| (Rc::new(s), Rc::new(a)));
        let lower: Vec<G1m> = ["_mid", "_low"].iter().filter_map(|l| named(&objects, &format!("{}{l}.g1m", t.name)).and_then(|b| G1m::parse(b).ok())).filter(|m| m.geometry.is_some()).collect();
        for i in &t.instances {
            let transform = placement_transform(i);
            let how = place(t, i);
            let levels: Vec<&barrel_formats::g1m::Geometry> = std::iter::once(geometry).chain(if how.lods { lower.iter().filter_map(|m| m.geometry.as_ref()).collect() } else { Vec::new() }).collect();
            for (level, g) in levels.iter().enumerate() {
                let before = scene.draws.len();
                // An object the game moves (a tree's spring) gets its own instance; the rest
                // are baked.
                if let Some((skeleton, anim)) = motion.as_ref().filter(|_| how.moves) {
                    // Posed by its joints, the placement on its instance; lower levels of
                    // detail take the palette the first level made.
                    let instance = scene.instance(transform);
                    scene.add_posed(g, Mat4::IDENTITY, texture_count, Some(&crate::skin::pose(skeleton, anim, 0.0)), instance)?;
                    if level == 0 {
                        moving.push(Moving { instance, skeleton: skeleton.clone(), motion: anim.clone() });
                    }
                } else if how.own_instance {
                    let instance = scene.instance(transform);
                    scene.add_posed(g, Mat4::IDENTITY, texture_count, None, instance)?;
                } else {
                    scene.add(g, transform, texture_count)?;
                }
                let lod = if levels.len() < 2 { 0 } else { level as u8 + 1 | if level + 1 == levels.len() { barrel_render::LOD_LAST } else { 0 } };
                for d in &mut scene.draws[before..] {
                    d.casts = how.casts;
                    // The object's name and its placement's tags (the sway keywords, xV1_ /
                    // xN1_, may be in either).
                    d.name = if i.tags.is_empty() { t.name.clone() } else { format!("{} {}", t.name, i.tags) };
                    d.version = how.version;
                    d.class = how.class;
                    d.lod = lod;
                    d.object = placed as u32 + 1;
                }
            }
            placed += 1;
        }
    }
    eprintln!("{area}: {placed} objects placed, {missing} without a model");
    for d in &mut scene.draws[first_draw..] {
        d.texture += base;
        d.overlay = d.overlay.map(|o| o + base);
        d.mask = d.mask.map(|m| m + base);
        d.normal_map = d.normal_map.map(|m| m + base);
        d.emission = d.emission.map(|m| m + base);
        d.specular = d.specular.map(|m| m + base);
    }
    Ok(Loaded { textures, pattern, moving })
}

