//! A KTGL field into a renderer scene: its terrain (`<AREA>.ktf`), the objects its placement list
//! (`<AREA>.opl2`) puts down with their `_hi` models from the field's object container, and its
//! texture pack (`<AREA>.g1t`), as Escha & Logy DX and the Ryza games keep them. What a
//! placement's tags mean (which objects cast shadows, sway, or show only in some versions) is
//! the game's, given as `place`.

use barrel_formats::elixir::Elixir;
use barrel_formats::g1m::G1m;
use barrel_formats::g1t::G1t;
use barrel_formats::opl2::{self, Instance, PlacedType};
use barrel_formats::ktf;
use barrel_render::{Scene, TerrainDraw};
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
}

/// A container's entry by name (dummies passed over).
pub fn named<'a>(container: &'a Elixir, name: &str) -> Option<&'a [u8]> {
    container.entries.iter().find(|e| e.name == name && !e.is_dummy()).map(|e| container.data(e))
}

/// A field's ground and objects into the scene, as field `scene.field`, its textures taken to
/// start at `base` in the scene's list. Returns its texture pack and its light texture
/// (`<AREA>_pat.g1t`; indoor fields carry a 64-byte stub, taken as none).
pub fn load(area: &str, main: &Elixir, objects: &Elixir, scene: &mut Scene, base: usize, place: &dyn Fn(&PlacedType, &Instance) -> Placement) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    let first_draw = scene.draws.len();
    let textures = named(&main, &format!("{area}.g1t")).ok_or("field has no texture pack")?.to_vec();
    let pattern = named(&main, &format!("{area}_pat.g1t")).filter(|b| G1t::parse(b).is_ok_and(|g| !g.textures.is_empty())).map(<[u8]>::to_vec);
    let texture_count = G1t::parse(&textures)?.textures.len();
    // A flat placeholder terrain (every height 0, one layer: TOWN_03, TOWN_07) is not drawn: the
    // town's ground is its models, and events stand characters below it (TOWN_07's at y -255).
    let terrain = named(&main, &format!("{area}.ktf")).map(|b| ktf::parse(b).map(|t| (b, t))).transpose()?.filter(|(_, t)| {
        let flat = t.heightmap.heights.iter().all(|&v| v == 0.0) && t.layers.textures.len() <= 1;
        if flat {
            eprintln!("{area}: flat placeholder terrain, not drawn");
        }
        !flat
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
            t.layers.textures.len(),
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
    for t in &placements.objects {
        let Some(bytes) = named(&objects, &format!("{}_hi.g1m", t.name)) else {
            missing += t.instances.len();
            continue;
        };
        let model = G1m::parse(bytes)?;
        let Some(geometry) = model.geometry.as_ref() else { continue };
        for i in &t.instances {
            // Scale, then rotate about x, y and z in that order, then move.
            let rotation = Quat::from_euler(EulerRot::ZYX, i.rotation[2], i.rotation[1], i.rotation[0]);
            let transform = Mat4::from_scale_rotation_translation(Vec3::from(i.scale), rotation, Vec3::from(i.position));
            let before = scene.draws.len();
            let how = place(t, i);
            // An object the game moves (a tree's spring) gets its own instance; the rest are
            // baked.
            if how.own_instance {
                let instance = scene.instance(transform);
                scene.add_posed(geometry, Mat4::IDENTITY, texture_count, None, instance)?;
            } else {
                scene.add(geometry, transform, texture_count)?;
            }
            for d in &mut scene.draws[before..] {
                d.casts = how.casts;
                // The object's name and its placement's tags (the sway keywords, xV1_ / xN1_,
                // may be in either).
                d.name = if i.tags.is_empty() { t.name.clone() } else { format!("{} {}", t.name, i.tags) };
                d.version = how.version;
            }
            placed += 1;
        }
    }
    eprintln!("{area}: {placed} objects placed, {missing} without a model");
    for d in &mut scene.draws[first_draw..] {
        d.texture += base;
        d.overlay = d.overlay.map(|o| o + base);
        d.mask = d.mask.map(|m| m + base);
    }
    Ok((textures, pattern))
}

