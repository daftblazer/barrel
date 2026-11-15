//! `barrel-escha`: draw one model offscreen with Vulkan and write the frame as PNG.
//!
//!     barrel-escha MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png [--size N] [--yaw DEG]
//!     barrel-escha --field FIELD_DIR OUT.png [--size N] [--yaw DEG]
//!
//! The second form draws every object a field's placement list puts down, each with its
//! model from the field's `_Obj` container and the field's shared textures.
//!
//! It renders a single frame and exits: no window, no loop. Set BARREL_VALIDATION=1 to
//! enable the Khronos validation layer.

use std::path::{Path, PathBuf};

use ash::vk;
use barrel_formats::elixir::Elixir;
use barrel_formats::g1m::G1m;
use barrel_formats::g1t::{Format, G1t};
use glam::{Mat4, Vec3};

use barrel_render::*;
use barrel_engine::field::named;
use barrel_engine::skin::{bind_world, local_pose, mix_pose, pose, skin, skin_world, Trs};

mod hud;
mod hero;
mod loadlist;
mod title;
mod splash;
mod screen;
mod menu;
mod message;
mod status;
mod basket;
mod assignment;
mod battlehud;
mod result;
mod shopping;
mod equipping;
mod area_panel;
mod question;
mod options;
mod drops;
mod mix;

/// `FILE#N` is entry N of a container; a plain path is a loose file (Sophie 2 keeps its
/// texture packs beside the model containers).
fn entry_bytes(spec: &str) -> Result<Vec<u8>> {
    let Some((file, n)) = spec.rsplit_once('#') else {
        return Ok(std::fs::read(spec)?);
    };
    let bytes = std::fs::read(file)?;
    let e = Elixir::parse(&bytes)?;
    let entry = e.entries.get(n.parse::<usize>()?).ok_or("no such entry")?;
    Ok(e.data(entry).to_vec())
}

/// Each joint's skinning matrix for an animation at `time`: its posed world transform times
/// the inverse of its bind world transform. Joints without a track keep their bind pose.
/// How long the text window stays up, empty, after a line goes down, for the next line to come
/// (seconds of event time: long enough to bridge the one-tick step between lines and the short
/// `p1 50` pauses between some). Not yet the game's: a stand-in for however it keeps its window
/// between lines (`docs/engine.md`, "Deliberate differences").
const WINDOW_GRACE: f32 = 0.6;

/// The music tracks compiled into the English executable (none if it cannot be read).
fn music_tracks(root: &Path) -> Vec<barrel_escha::data::bgm::Track> {
    std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))
        .ok()
        .and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().and_then(|pe| barrel_escha::data::bgm::tracks(&pe, barrel_escha::data::bgm::TRACKS_EN)))
        .unwrap_or_default()
}

/// The cauldron (`mix`), in the story's free play in the Atelier (map 0): the recipe made, if
/// one is, goes through `Session::synthesize` (its materials out of the basket, the item in,
/// its days passed).
#[allow(clippy::too_many_arguments)]
fn cauldron(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &screen::Target,
    window: Option<(&present::Presenter, &mut window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    session: Option<&mut barrel_escha::session::Session>,
    keys: &[title::Button],
    out: Option<&Path>,
    frame: u32,
) -> Result<()> {
    let Some(s) = session else { return Ok(()) };
    if *s.scene() != barrel_escha::session::Scene::Field(ATELIER_MAP) {
        eprintln!("the cauldron is in the Atelier");
        return Ok(());
    }
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let book = barrel_escha::cauldron::Book::load(&exe, root)?;
    let Some((r, picks)) = mix::run(gpu, pool, target, window, offscreen, root, &book, &s.basket, keys, out)? else {
        eprintln!("frame {frame}: synthesis: nothing made");
        return Ok(());
    };
    let mut rng = barrel_engine::rng::Rng::seeded(frame + 1);
    match s.synthesize(&book, r, &picks, &mut |lo, hi| rng.range(lo, hi)) {
        Ok(m) => eprintln!("frame {frame}: synthesis: made {} at quality {:.0} in {} days; the basket holds {}", book.name(m.item), m.quality, m.days, s.basket.len()),
        Err(e) => eprintln!("frame {frame}: synthesis: {e}"),
    }
    Ok(())
}

/// A town shop (B in free play in town; `BARREL_SHOP_AT` offscreen): the shop with the most
/// wares on sale (`Saves_EN/shop/ShopData.xml`; which shop stands in which part of town is not
/// read yet), one deal a visit through `Session::buy` / `Session::sell`.
#[allow(clippy::too_many_arguments)]
fn shop_visit(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &screen::Target,
    window: Option<(&present::Presenter, &mut window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    session: Option<&mut barrel_escha::session::Session>,
    keys: &[title::Button],
    out: Option<&Path>,
    frame: u32,
) -> Result<()> {
    let Some(s) = session else { return Ok(()) };
    if !matches!(*s.scene(), barrel_escha::session::Scene::Field(m) if m < FIRST_AREA_MAP) {
        eprintln!("the shops are in town");
        return Ok(());
    }
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let book = barrel_escha::cauldron::Book::load(&exe, root)?;
    let (tags, _) = pe.enum_names("ITEM_MIX_CRAFT").ok_or("no ITEM_* enum")?;
    let item_number = |tag: &str| tags.iter().position(|t| *t == tag).map(|n| n as i32);
    let table = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/shop/ShopData.xml"))?)?;
    let wares = barrel_escha::shop::wares(&table);
    let on_sale: Vec<&barrel_escha::shop::Ware> = wares.iter().filter(|w| barrel_escha::shop::on_sale(w, |e| s.event_seen(e)) && item_number(&w.item).is_some()).collect();
    let mut shops: Vec<&str> = on_sale.iter().map(|w| w.shop.as_str()).collect();
    shops.dedup();
    let Some(shop) = shops.iter().copied().max_by_key(|t| on_sale.iter().filter(|w| w.shop == *t).count()) else {
        eprintln!("frame {frame}: no shop has anything on sale");
        return Ok(());
    };
    let shown: Vec<&barrel_escha::shop::Ware> = on_sale.into_iter().filter(|w| w.shop == shop).collect();
    let item_rows = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/item/itemData.xml"))?)?;
    let cost = |item: i32| usize::try_from(item).ok().and_then(|i| item_rows.children.get(i)).and_then(|r| r.attr("cost")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let basket: Vec<(String, f32, i32)> = s.basket.iter().map(|g| (book.name(g.item).to_string(), g.quality, barrel_escha::synthesis::worth(cost(g.item), g.quality, &[]))).collect();
    let name = shown.first().map_or(shop.to_string(), |w| w.shop_name.clone());
    let held = |w: &barrel_escha::shop::Ware| item_number(&w.item).map_or(0, |i| s.basket.iter().filter(|g| g.item == i).count() as i32);
    let grid_items = basket::items(&s.basket.iter().map(|g| (g.item, g.quality)).collect::<Vec<_>>(), &item_rows.children);
    let lists = shopping::lists(&name, &shown, s.money, &basket, held, grid_items);
    match shopping::run(gpu, pool, target, window, offscreen, root, &lists, keys, out)? {
        Some(shopping::Deal::Buy(k)) => {
            let w = shown[k];
            let item = item_number(&w.item).unwrap_or(-1);
            if s.buy(item, w.quality as f32, barrel_escha::shop::list_price(w), 1) {
                eprintln!("frame {frame}: bought {} for {}; {} cole left", book.name(item), barrel_escha::shop::list_price(w), s.money);
            }
        }
        Some(shopping::Deal::Sell(k)) => {
            let item = s.basket[k].item;
            let paid = s.sell(&[k], cost, 0);
            eprintln!("frame {frame}: sold {} for {paid}; {} cole now", book.name(item), s.money);
        }
        None => eprintln!("frame {frame}: left the shop"),
    }
    Ok(())
}

/// The Equip page (the menu's Battle Equip; `BARREL_EQUIP_AT` offscreen with
/// `BARREL_EQUIP_KEYS`): one change through `Session::equip`.
#[allow(clippy::too_many_arguments)]
fn equip_visit(
    gpu: &Gpu,
    pool: vk::CommandPool,
    target: &screen::Target,
    window: Option<(&present::Presenter, &mut window::Pump)>,
    offscreen: Option<vk::Image>,
    root: &Path,
    session: Option<&mut barrel_escha::session::Session>,
    keys: &[title::Button],
    out: Option<&Path>,
    frame: u32,
) -> Result<()> {
    let Some(s) = session else { return Ok(()) };
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let book = barrel_escha::cauldron::Book::load(&exe, root)?;
    let table = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/item/itemData.xml"))?)?;
    let items: Vec<barrel_escha::equipment::ItemRow> = table.children.iter().map(barrel_escha::equipment::ItemRow::from_row).collect();
    let name = |item: i32| if item < 0 { "(none)".to_string() } else { book.name(item).to_string() };
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let members: Vec<equipping::Member> = s.party.iter().enumerate().map(|(c, m)| {
        let fitting: [Vec<(usize, String)>; 4] = std::array::from_fn(|slot| s.basket.iter().enumerate()
            .filter(|(_, g)| usize::try_from(g.item).ok().and_then(|i| items.get(i)).is_some_and(|r| r.fits(c, slot)))
            .map(|(k, g)| (k, format!("{}   quality {:.0}", book.name(g.item), g.quality)))
            .collect());
        // The Status page's values (Escha's and Logy's tables only), worn and with each change.
        let values = |m: &barrel_escha::session::Member| (c < 2).then(|| status::Values::lead(&pe, c == 1, Some(m), &table.children)).flatten();
        let previews = std::array::from_fn(|slot| fitting[slot].iter().map(|&(k, _)| {
            let mut changed = m.clone();
            changed.equipment[slot] = s.basket[k].item;
            values(&changed)
        }).collect());
        equipping::Member {
            name: ["Escha", "Logy"].get(c).map_or(format!("Member {c}"), |n| n.to_string()),
            worn: m.equipment.map(name),
            status: values(m),
            previews,
            fitting,
        }
    }).collect();
    match equipping::run(gpu, pool, target, window, offscreen, root, &members, keys, out)? {
        Some((member, slot, pick)) => {
            let item = s.basket[pick].item;
            match s.equip(member, slot, pick, &items) {
                Ok(()) => eprintln!("frame {frame}: {} now wears {} ({:?})", members[member].name, book.name(item), s.party[member].equipment),
                Err(e) => eprintln!("frame {frame}: equip: {e}"),
            }
        }
        None => eprintln!("frame {frame}: equipment unchanged"),
    }
    Ok(())
}

/// The town's entrance (`TOWN_05`), where a trip to the town arrives.
const TOWN_ENTRANCE: u32 = 4;

/// What the area map's cursor picked.
#[derive(Clone, Debug)]
enum AreaPick {
    /// Enter this field (an area map's sector).
    Field(u32),
    /// Up to the world map from region `region`'s area map.
    World { region: String },
    /// A trip on the world map from mark `from` to mark `to`.
    Travel { from: String, to: String },
}

/// The field a trip to world map mark `to` arrives at, for the story's arrival checks: the
/// town's entrance, or the first sector of region `to` (across its entry's route).
fn trip_field(pe: &barrel_formats::pe::Pe, to: &str) -> Option<u32> {
    if to == "T0" {
        return Some(TOWN_ENTRANCE);
    }
    let places = barrel_escha::data::world::places(pe, barrel_escha::data::world::PLACES_EN)?;
    let entry = places.iter().position(|p| p.kind == 1 && p.tag == to)?;
    let first = barrel_escha::area_map::neighbours(&places, entry).into_iter().map(|(_, k)| k).find(|&k| places[k].kind == 1)?;
    let names = barrel_escha::data::world::point_names(pe)?;
    let point = names.iter().position(|n| barrel_escha::data::world::point_tag(n).as_deref() == Some(places[first].tag.as_str()))?;
    barrel_escha::data::world::point_maps(pe)?.into_iter().find(|(p, _, _)| *p as usize == point).map(|(_, m, _)| m)
}

/// The area map on the world map (`game::area_map`): the places, their marks, the routes'
/// tags, the field each opens, where the player stands, and its panels (`area_panel`).
struct AreaMap {
    places: Vec<barrel_escha::data::world::Place>,
    marks: Vec<Option<[f32; 3]>>,
    routes: Vec<String>,
    fields: Vec<Option<u32>>,
    placements: barrel_formats::opl2::Placements,
    at: usize,
    layer: text::Layer,
    font: text::Font,
    sheets: hud::Sheets,
    shown: Option<(usize, u32, u32, bool, Option<(i32, u32, u32)>, Option<bool>, bool)>,
    /// The panel lists the sector's monsters (X), not its gatherables.
    monsters_shown: bool,
    /// The field enemies and encounter groups (a sector's monsters) and the monsters' names.
    field_enemies: Vec<barrel_escha::data::battle_data::FieldEnemy>,
    groups: Vec<barrel_escha::data::battle_data::EncounterGroup>,
    monster_names: Vec<String>,
    pe_bytes: Vec<u8>,
    /// The world map (the town and the regions, kind 0) rather than a region's area map, and
    /// the place the cursor started on (a trip's start).
    world: bool,
    from: usize,
    /// The world map's routes (a trip's days), on the world map.
    trips: Option<barrel_escha::travel::WorldMap>,
    /// The gathering tables (a sector's gatherables) and the items' names.
    gathering: Option<barrel_escha::gather::Gathering>,
    item_names: Vec<String>,
    /// The area routes' tags not known yet (`Session::known_routes`): their dots are not drawn.
    hidden_routes: Vec<String>,
    /// The routes seen but not walked: their dots green (`SEEN_ROUTE`).
    seen_routes: Vec<String>,
    /// "Return to World Map?" asked (Back on an area map): Yes lit when true.
    asking: Option<bool>,
    /// The places closed to the player (`Session::closed`), and the sectors not known yet
    /// (`Session::known`): no way there, no label, not listed.
    closed: Vec<bool>,
    /// The places visited (`Session::visited`): a sector's mark lit (`point_sm_on`, red) once
    /// visited, else unlit (`point_sm_off`, yellow).
    visited: Vec<bool>,
}

/// A mark's model's state: `Some(true)` for the visited one (`point_sm_on`, `point_big_on`,
/// `point_mat_on`: red), `Some(false)` for the one only seen (`_off`: yellow for a sector,
/// green for a region, as the colseit-arrival recording at 1:25 has the Old Highway).
fn mark_state(name: &str) -> Option<bool> {
    ["point_sm_", "point_big_", "point_mat_"].iter().find_map(|k| {
        let rest = &name[name.find(k)? + k.len()..];
        if rest.starts_with("on") { Some(true) } else if rest.starts_with("off") { Some(false) } else { None }
    })
}

impl AreaMap {
    fn new(wmap: &Path, pos: [f32; 3], world: bool, gpu: &Gpu, pool: vk::CommandPool, pass: vk::RenderPass) -> Result<AreaMap> {
        let root = wmap.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("not inside a game install")?;
        let pe_bytes = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&pe_bytes)?;
        let places = barrel_escha::data::world::places(&pe, barrel_escha::data::world::PLACES_EN).ok_or("no place table")?;
        let routes = barrel_escha::data::world::areas(&pe, barrel_escha::data::world::AREAS_EN).ok_or("no route table")?.into_iter().map(|a| a.id).collect();
        let bytes = std::fs::read(wmap.join("A15_WMAP.elixir.gz"))?;
        let container = Elixir::parse(&bytes)?;
        let entry = container.entries.iter().find(|e| e.name.ends_with(".opl2")).ok_or("the world map has no placements")?;
        let placements = barrel_formats::opl2::parse(container.data(entry))?;
        let names = barrel_escha::data::world::point_names(&pe).unwrap_or_default();
        let points = barrel_escha::data::world::point_maps(&pe).unwrap_or_default();
        // A place's point name (its tag as a world map point), its mark and its field.
        let point_of = |tag: &str| names.iter().position(|n| barrel_escha::data::world::point_tag(n).as_deref() == Some(tag));
        let marks = places.iter().map(|p| point_of(&p.tag).and_then(|k| barrel_escha::data::world::point_position(&placements, &names[k]))).collect::<Vec<_>>();
        let fields = places.iter().map(|p| point_of(&p.tag).and_then(|k| points.iter().find(|(q, _, _)| *q as usize == k).map(|(_, m, _)| *m))).collect();
        // Where she stands: the nearest mark of the level's places.
        let level = u32::from(!world);
        let at = (0..places.len())
            .filter(|&k| places[k].kind == level && marks[k].is_some())
            .min_by(|&a, &b| {
                let d = |k: usize| marks[k].map_or(f32::MAX, |m| (m[0] - pos[0]).powi(2) + (m[2] - pos[2]).powi(2));
                d(a).total_cmp(&d(b))
            })
            .ok_or("no place on the world map")?;
        let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("no glyph table")?;
        let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
        let atlas = G1t::parse(&atlas_bytes)?;
        let image = atlas.textures.first().ok_or("an empty font atlas")?;
        let font = text::Font { glyphs, size: image.width as f32 };
        // The panels' sheets, as any place's panels use them.
        let mut all: Vec<hud::Element> = (0..5).flat_map(|tier| area_panel::elements(&pe, "", "", "", &[], Some(tier), |_, _| 0.0)).collect();
        all.extend(area_panel::world_elements(&pe, "", "-", &[], |_, _| 0.0));
        all.extend(area_panel::travel(&pe, [0, 0], 0, 1, 1));
        all.extend(area_panel::frame(&pe));
        for side in [area_panel::Side::Left, area_panel::Side::Right, area_panel::Side::Above, area_panel::Side::Below] {
            all.extend(area_panel::label(&pe, [960, 540], "", side));
        }
        all.extend(question::elements(&pe, "", true));
        let keys = screen::sheet_keys(&pe, all);
        let packs = screen::read_packs(root, &keys)?;
        let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), G1t::parse(b)?))).collect::<Result<Vec<_>>>()?;
        let mut images = vec![image];
        let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
        for (name, index) in keys {
            let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
            sheets.sizes.push([t.width as f32, t.height as f32]);
            sheets.keys.push((name, index));
            images.push(t);
        }
        let layer = text::Layer::new(gpu, pool, pass, &images)?;
        drop(images);
        let trips = if world { barrel_escha::travel::WorldMap::load(&pe, &placements) } else { None };
        let gathering = barrel_escha::gather::Gathering::load(root).map_err(|e| eprintln!("area map: no gathering tables: {e}")).ok();
        let item_names = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/item/itemData.xml"))?)?.children.iter().map(|r| r.attr("ItemName").unwrap_or("?").to_string()).collect();
        use barrel_escha::data::battle_data as bd;
        let field_enemies = bd::field_enemies(&pe, bd::FIELD_ENEMIES_BUILDER_EN).unwrap_or_default();
        let groups = bd::encounter_groups(&pe, bd::ENCOUNTER_GROUPS_EN);
        let monster_names = bd::monsters(&pe, bd::MONSTERS_EN).unwrap_or_default().into_iter().map(|m| m.name).collect();
        Ok(AreaMap { places, marks, routes, fields, placements, at, layer, font, sheets, shown: None, pe_bytes, world, from: at, trips, gathering, item_names, closed: Vec::new(), visited: Vec::new(), hidden_routes: Vec::new(), seen_routes: Vec::new(), asking: None, monsters_shown: false, field_enemies, groups, monster_names })
    }

    /// Whether the cursor stands on the town (`T0`, Colseit): no area to show info for.
    fn on_town(&self) -> bool {
        self.places[self.at].tag == "T0"
    }

    /// The name of the region place `at` is in (`A2p1`'s is `A2`'s: "Southern Ruins").
    fn region_name(&self, at: usize) -> &str {
        let tag = &self.places[at].tag;
        let region = tag.split_once('p').map_or(tag.as_str(), |(r, _)| r);
        self.places.iter().find(|p| p.tag == region).map_or("", |p| p.name.as_str())
    }

    /// Whether a draw (by its name: object and placement tags) is on the way chosen: the
    /// mark of the place she started from, the cursor's place's mark, and the dots of the
    /// route between them (the colseit-arrival recording at 1:17: those blue, the dots past
    /// the cursor's place white). The ring under her in the recordings is her mark so tinted.
    fn chosen(&self, name: &str) -> bool {
        let tags = || name.split_whitespace();
        if mark_state(name).is_some() || name.contains("point_mat_") {
            let (from, at) = (&self.places[self.from].tag, &self.places[self.at].tag);
            return tags().any(|w| w == from || w == at);
        }
        if self.at == self.from || !name.contains("point_r") {
            return false;
        }
        let place = &self.places[self.from];
        let route = [barrel_escha::area_map::Way::Right, barrel_escha::area_map::Way::Left, barrel_escha::area_map::Way::Down, barrel_escha::area_map::Way::Up]
            .into_iter()
            .find(|&way| barrel_escha::area_map::step(&self.places, self.from, way) == Some(self.at))
            .and_then(|way| self.routes.get(place.links[way as usize] as usize));
        route.is_some_and(|r| tags().any(|w| w == r))
    }

    /// A draw's run-time tint: green for a dot of a route seen but not walked.
    fn tint(&self, name: &str) -> u32 {
        if !self.seen_routes.is_empty() && name.contains("point_r") && name.split_whitespace().any(|w| self.seen_routes.iter().any(|r| r == w)) { SEEN_ROUTE } else { 0 }
    }

    /// Whether a draw (by its name: object and placement tags) is a sector's mark or dot on
    /// the world map, a dot of a route not known,
    /// or the mark of a sector closed or not known yet (the recording at 0:45 has no marks
    /// for Sector One's unknown neighbours south of her, A2p2 and A2p4).
    fn hides(&self, name: &str) -> bool {
        // The world map shows the regions' marks and the routes between them, not the sectors'
        // marks and dots (the colseit-arrival recording at 1:12: none south of her).
        if self.world && (name.contains("point_sm_") || name.contains("ns_a15_point_r_off")) {
            return true;
        }
        name.split_whitespace().any(|w| {
            self.hidden_routes.iter().any(|r| r == w)
                || self.places.iter().zip(&self.closed).any(|(p, &shut)| shut && p.kind == 1 && p.tag == w)
                || (!self.visited.is_empty() && mark_state(name).is_some_and(|on| self.places.iter().enumerate().any(|(k, p)| {
                    p.tag == w && self.visited.get(k).copied().unwrap_or(false) != on
                })))
        })
    }

    /// The open place across the route in way `way` from where she stands.
    fn step(&self, way: barrel_escha::area_map::Way) -> Option<usize> {
        barrel_escha::area_map::step(&self.places, self.at, way).filter(|&k| !self.closed.get(k).copied().unwrap_or(false))
    }

    /// A button: a direction goes along that way's route (the dots queued as the walker's
    /// waypoints, then the next place's mark), confirm gives the field to enter.
    fn press(&mut self, b: title::Button, w: &mut Walker) -> Option<AreaPick> {
        use barrel_escha::area_map::Way;
        // The question open: up and down choose, confirm on Yes leaves for the world map.
        if let Some(yes) = self.asking {
            match b {
                title::Button::Up | title::Button::Down => self.asking = Some(!yes),
                title::Button::Confirm if yes => {
                    self.asking = None;
                    let tag = &self.places[self.at].tag;
                    return Some(AreaPick::World { region: tag.split_once('p').map_or(tag.as_str(), |(r, _)| r).to_string() });
                }
                title::Button::Confirm | title::Button::Back => self.asking = None,
                _ => {}
            }
            return None;
        }
        let way = match b {
            title::Button::Up => Way::Up,
            title::Button::Down => Way::Down,
            title::Button::Left => Way::Left,
            title::Button::Right => Way::Right,
            // The world map: a trip to the place the cursor is on (none where it started).
            title::Button::Confirm if self.world => {
                return (self.at != self.from).then(|| AreaPick::Travel { from: self.places[self.from].tag.clone(), to: self.places[self.at].tag.clone() });
            }
            title::Button::Confirm => return self.fields[self.at].map(AreaPick::Field),
            // Back asks first, Yes lit (the recording).
            title::Button::Back if !self.world => {
                self.asking = Some(true);
                return None;
            }
            _ => return None,
        };
        let next = self.step(way)?;
        let route = self.places[self.at].links[way as usize];
        let tag = self.routes.get(route as usize)?;
        let mark = self.marks[next]?;
        let mut path = barrel_escha::data::world::route_path(&self.placements, tag, w.pos);
        path.push(mark);
        eprintln!("area map: {} to {} along {tag} ({} dots)", self.places[self.at].name, self.places[next].name, path.len() - 1);
        w.waypoints.extend(path);
        self.at = next;
        None
    }

    /// Where each neighbour's label stands (world, game space): its route's third dot out (as
    /// the recording has "Ruins Entrance" and "Sector Two" either side of Sector One).
    fn label_places(&self) -> Vec<(usize, [f32; 3], barrel_escha::area_map::Way)> {
        use barrel_escha::area_map::Way;
        let Some(from) = self.marks[self.at] else { return Vec::new() };
        [Way::Right, Way::Left, Way::Down, Way::Up]
            .into_iter()
            .filter_map(|way| {
                let next = self.step(way)?;
                let tag = self.routes.get(self.places[self.at].links[way as usize] as usize)?;
                // No label across a route not shown (Upland Forest west of Colseit at 8/20).
                if self.hidden_routes.contains(tag) {
                    return None;
                }
                let path = barrel_escha::data::world::route_path(&self.placements, tag, from);
                // On the world map the route's middle dot, of two the one nearer the town
                // (colseit-arrival 1:25: R0's 3 dots → the 2nd, R1's 4 → the 2nd, R2's 6 → the
                // 3rd from Colseit; 1:12: R1's 3rd from the Southern Ruins); on an area map the
                // third dot out.
                let k = match (self.world, self.places[self.at].tag == "T0") {
                    (true, true) => path.len().saturating_sub(1) / 2,
                    (true, false) => path.len() / 2,
                    (false, _) => 2,
                };
                let mid = *path.get(k).or(path.last()).or(self.marks[next].as_ref())?;
                Some((next, mid, way))
            })
            .collect()
    }

    /// On the world map with the cursor away from where it started, the trip's days there
    /// (`seen`: the events seen, which shorten some routes).
    fn trip_days(&self, seen: impl Fn(u32) -> bool) -> Option<i32> {
        let map = self.trips.as_ref().filter(|_| self.at != self.from)?;
        map.trip(&self.places[self.from].tag, &self.places[self.at].tag, seen).map(|(d, _)| d)
    }

    /// The panels for the place she stands on, redrawn when it changes; with the camera's
    /// `mvp` (she stands still) the neighbours' labels too; on the world map, `trip` (days,
    /// arrival month and day) shows the travel panel.
    fn panel(&mut self, gpu: &Gpu, width: u32, height: u32, mvp: Option<Mat4>, trip: Option<(i32, u32, u32)>, knows: impl Fn(i32) -> bool, levels: barrel_escha::gather::Levels, seen: &[bool]) -> Result<()> {
        if self.shown == Some((self.at, width, height, mvp.is_some(), trip, self.asking, self.monsters_shown)) {
            return Ok(());
        }
        let pe = barrel_formats::pe::Pe::parse(&self.pe_bytes)?;
        let p = &self.places[self.at];
        let font = &self.font;
        let measure = |s: &str, size: f32| message::width_of(font, s) * size / message::LINE * 1.47;
        let mut e = if self.world {
            let sector = format!("{}p", p.tag);
            let sectors: Vec<String> = self.places.iter().enumerate().filter(|(k, q)| q.kind == 1 && q.tag.starts_with(&sector) && !self.closed.get(*k).copied().unwrap_or(false)).map(|(_, q)| q.name.clone()).collect();
            let mut e = area_panel::world_elements(&pe, &p.name, &p.description, &sectors, measure);
            // Under the cursor's mark, once she stands on it.
            if let (Some((days, month, day)), Some(mvp), Some(mark)) = (trip, mvp, self.marks[self.at]) {
                let k = height as f32 / 1080.0;
                let dx = (width as f32 - 1920.0 * k) / 2.0;
                let clip = mvp * glam::Vec4::new(-mark[0], mark[1], mark[2], 1.0);
                if clip.w > 0.0 {
                    let (x, y) = ((clip.x / clip.w + 1.0) * 0.5 * width as f32, (clip.y / clip.w + 1.0) * 0.5 * height as f32);
                    e.extend(area_panel::travel(&pe, [((x - dx) / k) as i32, (y / k) as i32], days, month, day));
                }
            }
            e
        } else {
            // The sector's gatherables, "????" for those not known yet.
            let items: Vec<String> = match (&self.gathering, self.fields[self.at]) {
                (Some(g), Some(map)) => g.field_items(map as i32, levels).into_iter().map(|i| if knows(i) { self.item_names.get(i as usize).cloned().unwrap_or_default() } else { "????".into() }).collect(),
                _ => Vec::new(),
            };
            if self.monsters_shown {
                let seen = |k: i32| usize::try_from(k).ok().and_then(|k| seen.get(k)).copied().unwrap_or(false);
                let monsters: Vec<String> = self.fields[self.at]
                    .map(|map| barrel_escha::field_ai::sector_monsters(&self.field_enemies, &self.groups, map as i32, seen))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| self.monster_names.get(m as usize).cloned().unwrap_or_default())
                    .collect();
                area_panel::elements(&pe, self.region_name(self.at), &p.name, &p.description, &monsters, None, measure)
            } else {
                area_panel::elements(&pe, self.region_name(self.at), &p.name, &p.description, &items, Some(levels.tier(false)), measure)
            }
        };
        if let Some(mvp) = mvp.filter(|_| self.asking.is_none()) {
            // On the 1920 × 1080 layout screen (scaled to the height, centred).
            let k = height as f32 / 1080.0;
            let dx = (width as f32 - 1920.0 * k) / 2.0;
            for (next, at, way) in self.label_places() {
                let clip = mvp * glam::Vec4::new(-at[0], at[1], at[2], 1.0);
                if clip.w <= 0.0 {
                    continue;
                }
                let (x, y) = ((clip.x / clip.w + 1.0) * 0.5 * width as f32, (clip.y / clip.w + 1.0) * 0.5 * height as f32);
                let name = &self.places[next].name;
                let at = [((x - dx) / k) as i32, (y / k) as i32];
                // A route going up or down (her own way) gets its label over or under it; one
                // going sideways beside it, toward the screen's middle.
                let side = match way {
                    barrel_escha::area_map::Way::Up => area_panel::Side::Above,
                    barrel_escha::area_map::Way::Down => area_panel::Side::Below,
                    _ if at[0] < 960 => area_panel::Side::Left,
                    _ => area_panel::Side::Right,
                };
                // The world map's labels stand farther off their dots than the area map's
                // (colseit-arrival 1:25, measured at 720p, the Left one mirrored, unseen).
                let at = match (self.world, side) {
                    (true, area_panel::Side::Below) => [at[0], at[1] - WORLD_LABEL_LIFT],
                    (true, area_panel::Side::Right) => [at[0] - WORLD_LABEL_SIDE[0], at[1] - WORLD_LABEL_SIDE[1]],
                    (true, area_panel::Side::Left) => [at[0] + WORLD_LABEL_SIDE[0], at[1] - WORLD_LABEL_SIDE[1]],
                    _ => at,
                };
                e.extend(area_panel::label(&pe, at, name, side));
            }
        }
        if let Some(yes) = self.asking {
            e.extend(question::elements(&pe, RETURN_TO_WORLD_MAP, yes));
        }
        let mut e = [area_panel::frame(&pe), e].concat();
        self.layer.set(gpu, &hud::quads(&pe, Some(&self.font), &self.sheets, &e, width as f32, height as f32))?;
        self.shown = Some((self.at, width, height, mvp.is_some(), trip, self.asking, self.monsters_shown));
        Ok(())
    }
}

/// A seen route's dots, as unorm8 RGBA: the recording's green (0:45: (0.77, 0.97, 0.75) where
/// a walked route's are (0.95, 0.95, 0.91)), matched by eye on Barrel's lighting.
/// The maps' blue for the way chosen, on the display colour: `c * mul + add` (the
/// colseit-arrival recording at 1:12 and 1:17: a white dot (221, 212, 196) turns (156, 218,
/// 250), the town mark's cream rim (236, 227, 207) (160, 224, 251), its red middle (180, 94,
/// 77) (126, 99, 142); not a plain multiply, the blue rises on dark texels). Applied to the
/// texel, before Barrel's (brighter) map light: green 0.9 rather than the fitted 1.03, so the
/// lit result matches (1:25: dot (131, 217, 251), mark middle (108, 92, 155)).
const CHOSEN_TINT: ([f32; 3], [f32; 3]) = ([0.8, 0.97, 0.91], [0.0, 0.0, 0.28]);
const SEEN_ROUTE: u32 = u32::from_le_bytes([170, 255, 170, 255]);

/// The area map's Back question (the executable's string at `0x140949850`).
const RETURN_TO_WORLD_MAP: &str = "Return to World Map?";

/// A lost battle's line for the lead (`Saves_EN/systemMessage/SysMess.xml`, row 240 for Escha,
/// 241 for Logy: the one the game showed after a wipe; row 61, Escha's other, presumably
/// after a wipe in the Atelier's own fights), as (name, text).
fn wipe_line(root: &Path, logy: bool) -> Option<(String, String)> {
    let table = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/systemMessage/SysMess.xml")).ok()?).ok()?;
    let text = table.children.get(if logy { 241 } else { 240 })?.attr("Text")?.to_string();
    Some((if logy { "Logy" } else { "Escha" }.to_string(), text))
}

/// How long an offscreen run shows a carried notice.
const NOTICE_SECONDS: f32 = 3.0;

/// The first field area's map number (the town's maps come before it).
const FIRST_AREA_MAP: u32 = 12;

/// The Atelier's field map number.
const ATELIER_MAP: u32 = 0;

/// The period's assignment card for the menu's Assignment page (empty without a story or
/// when the table cannot be read).
fn card_of(root: &Path, session: Option<&barrel_escha::session::Session>) -> Vec<Option<assignment::Task>> {
    let Some(s) = session else { return Vec::new() };
    assignment::card(root, s.today().max(0) as u32, |e| s.event_seen(e)).unwrap_or_else(|e| {
        eprintln!("no assignment card: {e}");
        Vec::new()
    })
}

/// The basket's (item row, quality) pairs, for the menu's Basket page.
fn basket_of(session: Option<&barrel_escha::session::Session>) -> Vec<(i32, f32)> {
    session.map_or_else(Vec::new, |s| s.basket.iter().map(|g| (g.item, g.quality)).collect())
}

/// Place the characters an event adds, at their first position, in the scene. Returns the
/// texture packs it used, appended after the `base` textures already in the scene.
/// An event's camera: where it looks, from how far, at which angles (degrees) and field of
/// view. From `t20/0x10` (0x74100014), all in hundredths: {yaw°, angle°, pitch°, angle°,
/// distance, target x, y, z, offset x, y, z}; the camera looks at target + offset.
#[derive(Clone, Copy, Debug)]
struct EventCamera {
    /// Where the eye is, when known outright (the stage's camera, `--at`); otherwise it is
    /// worked out from distance, yaw and pitch.
    eye: Option<Vec3>,
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
    fov: f32,
}

/// A character the frame loop moves: its instance, its skeleton, where its motions come
/// from, and the motion it plays (its `CharaBase` column) and how far into it it is at the
/// start (`--at`).
struct Actor {
    who: u32,
    instance: u32,
    skeleton: barrel_formats::g1m::Skeleton,
    motions: Motions,
    column: String,
    anim: barrel_formats::g1a::Animation,
    start: f32,
    looping: bool,
    /// What is being blended out.
    previous: Option<BlendFrom>,
    /// What it plays: the motion's column, its division and the stage time it started; a
    /// change of any starts a blend.
    playing: (String, Option<u32>, f32),
    /// The local pose shown last frame (to blend out of when a switch comes mid-blend).
    last_locals: Vec<Trs>,
    face: Option<FaceRig>,
    /// The time shown at the last frame, in the current motion.
    last_t: f32,
    /// The head gestures' clip and the joints it turns (`s116`).
    head: Option<(barrel_formats::g1a::Animation, Vec<usize>)>,
    /// The division of the motion it plays (`s5`, `s11`), when one is set and found.
    division: Option<barrel_formats::chara_div::Division>,
    /// The motion whose division was last logged.
    last_division_column: String,
}

/// What a character's new motion blends in over.
enum BlendFrom {
    /// The old motion running on: it, its time at the switch, whether it loops.
    Running(barrel_formats::g1a::Animation, f32, bool),
    /// A held pose: the one shown when the switch came during a blend, or between two
    /// divisions of one motion (so the pose never jumps).
    Held(Vec<Trs>),
}

/// A character's body as loaded: its texture pack, the pack's texture count, its instance, and
/// what playback moves it with (skeleton, motions, the motion and its start) when found.
struct Body {
    /// Its texture packs (the body's, then an accessory's), and their textures in all.
    packs: Vec<Vec<u8>>,
    count: usize,
    instance: u32,
    rig: Option<(barrel_formats::g1m::Skeleton, Motions, barrel_formats::g1a::Animation, f32)>,
}

/// An outfit's parts on a body (`barrel_escha::data::costume::resolve`): the body's own nodes to
/// leave off (`P_M_Acc_Hat1` ...), and the accessories hung from nodes: each its file stem in
/// `Data/x64/Accessory` (`HEAD01`, `FACE00`), the node's name and its placement there
/// (`AccessoryAdjust`).
#[derive(Clone, Debug, Default)]
struct Wear {
    hidden: Vec<String>,
    accessories: Vec<(String, String, Mat4)>,
}

/// Loads `row`'s model (a `CharaBase` row) into the scene at `pos` turned `angle` degrees,
/// posed in motion `column` at `motion_time` (seconds, looping or not; the middle when none),
/// its textures from `tex_base`, with the accessories always worn; `face(scene, container)`
/// runs before the body is added (for morph targets) and gives what it returns back.
#[allow(clippy::too_many_arguments)]
fn add_body<F>(
    root: &Path,
    scene: &mut Scene,
    tex_base: usize,
    row: &barrel_formats::table::Element,
    who: u32,
    pos: [f32; 3],
    angle: f32,
    column: &str,
    motion_time: Option<(f32, bool)>,
    wear: Option<&Wear>,
    face: impl FnOnce(&mut Scene, &Elixir, &str, &str) -> Result<Option<F>>,
) -> Result<Option<(Body, Option<F>)>> {
    let Some(model) = row.attr("Model") else { return Ok(None) };
    // Rows with Directory "Obj" (the map cursors) keep their models in `Data/x64/Obj`.
    let folder = if row.attr("Directory") == Some("Obj") { "Data/x64/Obj" } else { "Data/x64/Character" };
    let path = root.join(folder).join(format!("{model}.elixir.gz"));
    let Ok(bytes) = std::fs::read(&path) else { return Ok(None) };
    let container = Elixir::parse(&bytes)?;
    let stem = model.rsplit('/').next().unwrap_or(model);
    let (Some(mesh), Some(tex)) = (named(&container, &format!("{stem}_default.g1m")), named(&container, &format!("{stem}.g1t"))) else {
        return Ok(None);
    };
    let count = G1t::parse(tex)?.textures.len();
    let g = G1m::parse(mesh)?;
    let Some(geometry) = g.geometry.as_ref() else { return Ok(None) };
    let transform = Mat4::from_rotation_translation(glam::Quat::from_rotation_y(angle.to_radians()), Vec3::from(pos));
    let looping = motion_time.is_none_or(|(_, l)| l);
    let rig = (|| -> Option<(Vec<Mat4>, barrel_formats::g1m::Skeleton, Motions, barrel_formats::g1a::Animation, f32)> {
        let mut motions = Motions::new(root, model, row)?;
        let anim = motions.get(column)?.clone();
        let skeleton = G1m::parse(named(&container, &format!("{stem}.g1m"))?).ok()?.skeleton?;
        eprintln!("character {who}: motion {column} = {}_{}", motions.head, motions.columns.get(column).map_or("(its own)", String::as_str));
        let t0 = motion_time.map_or(0.5, |(t, _)| t);
        let joints = pose(&skeleton, &anim, motion_clock(&anim, t0, looping));
        Some((joints, skeleton, motions, anim, t0))
    })();
    let (joints, rig) = match rig {
        Some((j, sk, m, an, t0)) => (Some(j), Some((sk, m, an, t0))),
        None => (None, None),
    };
    let face = face(scene, &container, model, stem)?;
    let instance = scene.add_with_pack(geometry, transform, tex_base, count, joints.as_deref())?;
    scene.morph.clear();
    // The accessories always worn (<stem>_P_M_Acc_*: Escha's hat), on the body's skeleton
    // and instance; the weapons and options (CharaPartsChange groups) are left off.
    let accessory = format!("{stem}_P_M_Acc_");
    let first = scene.draws.len();
    let hidden = |name: &str| wear.is_some_and(|w| w.hidden.iter().any(|h| name.contains(h.as_str())));
    for e in container.entries.iter().filter(|e| e.name.starts_with(&accessory) && e.name.ends_with(".g1m") && !hidden(&e.name)) {
        let Some(bytes) = named(&container, &e.name) else { continue };
        let part = G1m::parse(bytes)?;
        if let Some(pg) = part.geometry.as_ref() {
            scene.add_posed(pg, Mat4::IDENTITY, count, joints.as_deref(), instance)?;
        }
    }
    for d in &mut scene.draws[first..] {
        d.texture += tex_base;
        d.overlay = d.overlay.map(|o| o + tex_base);
        d.mask = d.mask.map(|m| m + tex_base);
    }
    let mut packs = vec![tex.to_vec()];
    let mut count = count;
    // The outfit's accessories, each hung from its node: its vertices are moved into the node's
    // bind frame (and by the table's placement) and bound wholly to that joint, so the pose
    // carries them as the body moves.
    let names = named(&container, &format!("{stem}Oid.bin")).and_then(|b| barrel_formats::oid::parse(b).ok()).unwrap_or_default();
    for (stem_a, node, adjust) in wear.map_or(&[][..], |w| &w.accessories[..]) {
        let (Some((skeleton, ..)), Some(j)) = (rig.as_ref(), joints.as_ref()) else { break };
        let joint = names.iter().position(|n| n == node);
        let acc_bytes = std::fs::read(root.join("Data/x64/Accessory").join(format!("{stem_a}.elixir.gz"))).ok();
        let acc = acc_bytes.as_deref().and_then(|b| Elixir::parse(b).ok());
        let (Some(joint), Some(acc)) = (joint, acc) else { continue };
        let (Some(mesh), Some(atex)) = (named(&acc, &format!("{stem_a}_MODEL.g1m")), named(&acc, &format!("{stem_a}_MODEL.g1t"))) else { continue };
        let g = G1m::parse(mesh)?;
        let Some(geometry) = g.geometry.as_ref() else { continue };
        let bind = bind_world(skeleton);
        let textures = G1t::parse(atex)?.textures.len();
        let first = scene.draws.len();
        scene.rigid_joint = Some(joint as u16);
        scene.add_posed(geometry, bind[joint] * *adjust, textures, Some(j), instance)?;
        scene.rigid_joint = None;
        for d in &mut scene.draws[first..] {
            d.texture += tex_base + count;
            d.overlay = d.overlay.map(|o| o + tex_base + count);
            d.mask = d.mask.map(|m| m + tex_base + count);
        }
        count += textures;
        packs.push(atex.to_vec());
        eprintln!("character {who}: {stem_a} at {node} (joint {joint})");
    }
    Ok(Some((Body { packs, count, instance, rig }, face)))
}

/// Outfit `body` (a `CostumeBody` row) with head accessory `head` (a `CostumeHead` row): the
/// body's model tag and what it wears (`costume::resolve`; the accessory placed by its
/// `AccessoryAdjust` row: offset, then the rotation in degrees about x, y, z, then the scale).
fn outfit(root: &Path, body: usize, head: usize, face: usize) -> Result<Option<(String, Wear)>> {
    use barrel_escha::data::costume as c;
    let table = |n: &str| -> Result<barrel_formats::table::Element> { Ok(barrel_formats::table::parse(&std::fs::read(root.join("Saves/costume").join(n))?)?) };
    let bodies = c::bodies(&table("CostumeBody.xml")?);
    let Some(b) = bodies.get(body) else { return Ok(None) };
    let adjusts = c::adjusts(&table("AccessoryAdjust.xml")?);
    let (changes, hides) = (c::changes(&table("AccessoryChange.xml")?), c::hides(&table("PartsInvisible.xml")?));
    // The body's tag: the `COSTUME_BODY_*` name of its row, in the executable's enum order.
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let body_tag = pe
        .enum_names("COSTUME_BODY_ESCHA_DEFAULT")
        .and_then(|(names, first)| names[first..].iter().take_while(|n| n.starts_with("COSTUME_BODY_")).nth(body).map(|n| n.to_string()))
        .unwrap_or_default();
    let mut wear = Wear::default();
    // Each part: its accessory row's tag (`COSTUME_HEAD_` or `COSTUME_FACE_` and the model stem,
    // `_NONE` for none), resolved on the body and placed by its `AccessoryAdjust` row (offset,
    // then the rotation in degrees about x, y, z, then the scale).
    for (part, prefix, file, row, node) in [("COSTUME_PARTS_HEAD", "COSTUME_HEAD_", "CostumeHead.xml", head, &b.head_attach), ("COSTUME_PARTS_FACE", "COSTUME_FACE_", "CostumeFace.xml", face, &b.face_attach)] {
        let rows = c::accessories(&table(file)?);
        let Some(a) = rows.get(row) else { continue };
        let tag = if a.file.is_empty() { format!("{prefix}NONE") } else { format!("{prefix}{}", a.file) };
        let (attached, hidden) = c::resolve(&body_tag, part, &tag, &changes, &hides);
        wear.hidden.extend(hidden);
        if let Some(tag) = attached
            && let Some(adj) = adjusts.iter().find(|x| x.body == body_tag && x.parts == part && x.accessory == tag) {
                let r = glam::Quat::from_euler(glam::EulerRot::XYZ, adj.rotation[0].to_radians(), adj.rotation[1].to_radians(), adj.rotation[2].to_radians());
                let m = Mat4::from_scale_rotation_translation(Vec3::from(adj.scale), r, Vec3::from(adj.offset));
                wear.accessories.push((tag.trim_start_matches(prefix).to_string(), node.clone(), m));
            }
    }
    eprintln!("outfit: body {body} {} ({body_tag}), head {head}, face {face}: hidden {:?}, accessories {:?}", b.base, wear.hidden, wear.accessories.iter().map(|a| &a.0).collect::<Vec<_>>());
    Ok(Some((b.base.clone(), wear)))
}

/// The outfits the settings save (`SYSDATA`) has chosen, as `(Chara.xml row, body, head, face)`:
/// its `costumeData` entry k is `CostumeChara.xml` row k, whose `target_chara` (`CHARA_*`, the
/// party order) is the character: its `Chara.xml` row is the same number, but for Nio (10), who
/// is row 11 (`battle_data::party_size`).
fn saved_costumes(root: &Path, sys: &Path) -> Result<Vec<(u32, usize, usize, usize)>> {
    use barrel_escha::data::costume as c;
    let state = barrel_escha::data::save::decode(&std::fs::read(sys)?)?;
    let records = barrel_escha::data::save::state_records(&state)?;
    fn find<'a>(rs: &'a [barrel_escha::data::save::Record<'a>], name: &str) -> Option<&'a [u8]> {
        use barrel_escha::data::save::Body;
        rs.iter().find_map(|r| match &r.body {
            Body::Raw(d) | Body::Data(d) if r.name == name => Some(*d),
            Body::Group(g) => find(g, name),
            _ => None,
        })
    }
    let choices = find(&records, "costumeData").map(c::choices).unwrap_or_default();
    let charas = c::charas(&barrel_formats::table::parse(&std::fs::read(root.join("Saves/costume/CostumeChara.xml"))?)?);
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let names: Vec<&str> = pe.enum_names("CHARA_ESCHA").map(|(n, first)| n[first..].iter().copied().take_while(|x| x.starts_with("CHARA_")).collect()).unwrap_or_default();
    let mut out = Vec::new();
    for (k, [b, h, f]) in choices.into_iter().enumerate() {
        let Some(member) = charas.get(k).and_then(|ch| names.iter().position(|n| *n == ch.target)) else { continue };
        let row = if member >= 10 { member + 1 } else { member };
        out.push((row as u32, b.max(0) as usize, h.max(0) as usize, f.max(0) as usize));
    }
    eprintln!("costumes from the settings save: {out:?}");
    Ok(out)
}

/// A field enemy the frame loop moves: its place in the field's enemies, its instance, and its
/// rig and motion (a `CharaBase` column) with when that started.
struct FieldBody {
    mob: usize,
    instance: u32,
    skeleton: barrel_formats::g1m::Skeleton,
    motions: Motions,
    column: String,
    anim: barrel_formats::g1a::Animation,
    since: f32,
}

/// Field area `area`'s enemies (`barrel_escha::field_ai::build`), each drawn as its group's first
/// monster in its field wait, and Escha as a stand-in player walking west through them (`mob`
/// `usize::MAX`), added to the scene; returns the field, the bodies, their texture packs, the
/// motion names and where the player starts.
/// The town people the field load (`0x140071d00`) would place on this field: its map number from
/// the directory's name, each placement whose conditions pass on the save's seen events (or,
/// without a save, each character's first placement), as a still body at its spot (metres to
/// field units). Random-companion placements (rows 318 to 322) are left out.
/// A town person's body, posed in its wait motion each frame (it does not move).
struct TownBody {
    instance: u32,
    skeleton: barrel_formats::g1m::Skeleton,
    anim: barrel_formats::g1a::Animation,
    /// Its own motion clock (seconds into `anim`), if not the viewer's.
    clock: Option<f32>,
    /// The motion it is leaving, its clock and how far into the blend it is (seconds): the new
    /// one is blended in over [`MOTION_BLEND`] (`0x140235410` asks 0.15 s).
    from: Option<(barrel_formats::g1a::Animation, f32, f32)>,
    /// When a switch came during a blend: the pose then shown, blended out of instead of
    /// `from`'s motion (so the pose never jumps).
    held: Option<Vec<Trs>>,
    /// The local pose shown last frame.
    last_locals: Vec<Trs>,
    /// Gone (a fallen monster): drawn collapsed.
    hidden: bool,
}

impl TownBody {
    /// Starts `next` from its beginning, blended in over [`MOTION_BLEND`]: out of the old motion
    /// running on, or, mid-blend, out of the pose last shown.
    fn switch(&mut self, next: barrel_formats::g1a::Animation) {
        self.held = (self.from.is_some() && !self.last_locals.is_empty()).then(|| self.last_locals.clone());
        let old = std::mem::replace(&mut self.anim, next);
        self.from = Some((old, self.clock.unwrap_or(0.0), 0.0));
        self.clock = Some(0.0);
    }
}

/// How long a gait change blends the old motion into the new one (seconds).
const MOTION_BLEND: f32 = 0.15;

fn town_people(root: &Path, field_dir: &Path, seen: Option<Vec<bool>>, scene: &mut Scene, base: usize) -> Result<(Vec<Vec<u8>>, Option<[f32; 3]>, Vec<TownBody>)> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let dir_name = field_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let map = barrel_escha::data::maps::field_maps(&pe).iter().position(|n| n == dir_name).ok_or("--town-people: the field's map number is not known")? as i32;
    let is_seen = |e: &i32| seen.as_ref().and_then(|s| usize::try_from(*e).ok().and_then(|e| s.get(e))).copied().unwrap_or(false);
    let holds = |c: &i32| barrel_escha::data::npc::condition(&pe, *c).is_some_and(|(want, not)| want.iter().all(is_seen) && !not.iter().any(is_seen));
    let table = |name: &str| std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let (charas, bases) = (table("Chara.xml").ok_or("no Chara.xml")?, table("CharaBase.xml").ok_or("no CharaBase.xml")?);
    let mut placed: Vec<i32> = Vec::new();
    let (mut packs, mut tex_base, mut first, mut bodies) = (Vec::new(), base, None, Vec::new());
    for p in barrel_escha::data::npc::places(&pe).ok_or("no placement table")?.iter().filter(|p| p.map == map) {
        if (318..=322).contains(&p.chara) || placed.contains(&p.chara) {
            continue;
        }
        if seen.is_some() && !((p.conditions[0].is_empty() || p.conditions[0].iter().any(holds)) && !p.conditions[1..].iter().flatten().any(holds)) {
            continue;
        }
        let Some(row) = usize::try_from(p.chara)
            .ok()
            .and_then(|r| charas.children.get(r)?.attr("BaseTag"))
            .and_then(|tag| bases.children.iter().find(|b| b.attr("BaseTag") == Some(tag)))
        else {
            continue;
        };
        let pos = p.position.map(|v| v * 100.0);
        let who = 20_000 + placed.len() as u32;
        let Some((body, _)) = add_body(root, scene, tex_base, row, who, pos, p.angle, "F_WAIT", None, None, |_, _, _, _| Ok(None::<()>))? else { continue };
        eprintln!("town person chara {} as {:?} at {pos:?} facing {}", p.chara, row.attr("BaseTag"), p.angle);
        tex_base += body.count;
        packs.extend(body.packs);
        if let Some((skeleton, _, anim, _)) = body.rig {
            bodies.push(TownBody { instance: body.instance, skeleton, anim, clock: None, from: None, held: None, last_locals: Vec::new(), hidden: false });
        }
        placed.push(p.chara);
        first = first.or(Some(pos));
    }
    Ok((packs, first, bodies))
}


/// A small xorshift for the trees' push (cosmetic: the game uses its C library's rand()).
struct SwayRng(u32);

impl SwayRng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

/// The player walking: her instance, where she stands (field units), her velocity (x, z, units
/// a second) and the field's walls and ground she walks against (`game::walk`).
struct Walker {
    instance: u32,
    /// Her entry in the town bodies (posed there in `wait` or `run`).
    body: usize,
    wait: barrel_formats::g1a::Animation,
    walk: barrel_formats::g1a::Animation,
    run: barrel_formats::g1a::Animation,
    pos: [f32; 3],
    velocity: [f32; 2],
    /// The way she faces (radians about y, from +z), kept when she stops.
    facing: f32,
    /// The motion her body is in.
    gait: barrel_escha::player::Gait,
    /// The field's gimmicks (its doors among them).
    gimmicks: Vec<barrel_escha::data::gimmick::Gimmick>,
    /// The run toggle (T in a window; the game's pad button 6), off at the start.
    run_on: bool,
    /// The field she walks (walls, ground, gimmicks: its doors lead on).
    field: barrel_engine::walk::Field,
    /// The gimmicks she stands in now (a door takes her only as she steps in).
    inside: Vec<usize>,
    /// The field names by map number, for the doors.
    maps: Vec<String>,
    /// A world map trip's waypoints still ahead (`--world-trip`): she runs to each in turn.
    waypoints: std::collections::VecDeque<[f32; 3]>,
    /// Her `JUMP` motion, and the seconds since a jump left the ground (J in a window;
    /// `BARREL_JUMP_AT=FRAME` jumps at that frame).
    jump_anim: Option<barrel_formats::g1a::Animation>,
    jump: Option<f32>,
    /// Her field strike (K in a window; `BARREL_STRIKE_AT=FRAME`): the `wandAttack` macro's
    /// motion, `LAD` (her `ACT_KNOCK`), whose divisions 0, 1 and 2 the macro plays in turn, back
    /// to back; and whether one is under way.
    strike_anim: Option<barrel_formats::g1a::Animation>,
    striking: bool,
    /// Whether a jump calls out (`game::walk::JumpVoice`), on its own generator.
    jump_voice: barrel_escha::player::JumpVoice,
    rng: barrel_engine::rng::Rng,
    /// The install, for her sounds.
    root: std::path::PathBuf,
    /// The play camera's follow (`play_camera::Follow`).
    follow: barrel_engine::play_camera::Follow,
    /// On the world map, the cursor's own model (`E3D_WMAP_CUR00`'s instance) and where she
    /// stays meanwhile: the walker is the cursor there, she waits on her start mark.
    cursor: Option<(u32, [f32; 3], f32)>,
    /// Logy leads instead of Escha (`BARREL_LEAD=logy`): his swing, its sound and reach, his
    /// jump voices.
    logy: bool,
    /// The map's field music (`Area::music`), played from the start in a window.
    music: Option<u32>,
    /// The map's landing sound (`walk::landing_sound`).
    landing: Option<u32>,
    /// The map's ambience and its volume (`Area::ambience`; none where the volume is 0).
    ambience: Option<(u32, f32)>,
}

/// The player (Escha, `Chara.xml` row 0) standing at (x, z) on the field in FIELD_DIR: on its
/// ground there (`game::walk::Ground`: the heightmap and the collision floors), in her wait
/// motion. Her texture packs, where she stands (field units) and her body.
fn player(root: &Path, field_dir: &Path, at: [f32; 2], logy: bool, scene: &mut Scene, base: usize) -> Result<(Vec<Vec<u8>>, [f32; 3], TownBody, Walker)> {
    let name = field_dir.file_name().and_then(|n| n.to_str()).ok_or("no field name")?;
    // The map's gimmicks from the executable's table (its doors lead to other maps).
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let maps = barrel_escha::data::maps::field_maps(&pe);
    let map = maps.iter().position(|n| n == name);
    let area = map.and_then(|m| barrel_escha::data::gimmick::areas(&pe, barrel_escha::data::gimmick::MAPS_EN, barrel_escha::data::gimmick::MAP_COUNT)?.get(m).cloned());
    let gimmicks = area.as_ref().map(|a| a.gimmicks.clone()).unwrap_or_default();
    let logy = logy || std::env::var("BARREL_LEAD").is_ok_and(|v| v.eq_ignore_ascii_case("logy"));
    // The viewer has no story flags: flag 303 is taken as unset.
    let music = map.zip(area.as_ref()).and_then(|(m, a)| u32::try_from(barrel_escha::player::field_music(m, a.music(), logy, false)).ok());
    let landing = area.as_ref().and_then(|a| barrel_escha::player::landing_sound(a.landing()));
    let ambience = area.as_ref().map(|a| a.ambience()).and_then(|(n, v)| Some((u32::try_from(n).ok()?, v))).filter(|&(_, v)| v > 0.0);
    let field = barrel_engine::walk::Field::load(field_dir.parent().ok_or("no fields folder")?, name)?;
    let ground = &field.ground;
    // Dropped from high above onto whatever is under her.
    let y = ground.height(at[0], at[1], 1.0e6, 0.0).ok_or("--player: no ground there")?;
    let table = |name: &str| std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let (charas, bases) = (table("Chara.xml").ok_or("no Chara.xml")?, table("CharaBase.xml").ok_or("no CharaBase.xml")?);
    // The lead: Escha (`Chara.xml` row 0), or Logy (row 1) with `BARREL_LEAD=logy`.
    let row = charas
        .children
        .get(usize::from(logy))
        .and_then(|r| r.attr("BaseTag"))
        .and_then(|tag| bases.children.iter().find(|b| b.attr("BaseTag") == Some(tag)))
        .ok_or("no row for the player")?;
    let pos = [at[0], y, at[1]];
    let (body, _) = add_body(root, scene, base, row, 21_000, pos, 0.0, "F_WAIT", None, None, |_, _, _, _| Ok(None::<()>))?.ok_or("the player's model did not load")?;
    let (skeleton, mut motions, anim, _) = body.rig.ok_or("the player has no skeleton")?;
    // Her run (the `RUN` column, `ACT_RUN`; `WALK` is the slow one).
    let run = motions.get("RUN").cloned().ok_or("the player has no RUN motion")?;
    let walk = motions.get("WALK").cloned().ok_or("the player has no WALK motion")?;
    let jump_anim = motions.get("JUMP").cloned();
    let strike_anim = motions.get("LAD").cloned();
    eprintln!("player as {:?} at {pos:?}", row.attr("BaseTag"));
    let inside = barrel_escha::data::gimmick::inside(&gimmicks, pos);
    let walker = Walker { instance: body.instance, body: 0, wait: anim.clone(), walk, run, pos, velocity: [0.0; 2], facing: 0.0, gait: barrel_escha::player::Gait::Wait, run_on: false, field, gimmicks, inside, maps, waypoints: Default::default(), jump_anim, jump: None, strike_anim, striking: false, jump_voice: Default::default(), rng: barrel_engine::rng::Rng::from_state([0x1357_9bdf, 0x2468_ace0, 0x0f1e_2d3c, 0x4b5a_6978]), root: root.to_path_buf(), logy, music, landing, ambience, follow: Default::default(), cursor: None };
    Ok((body.packs, pos, TownBody { instance: body.instance, skeleton, anim, clock: Some(0.0), from: None, held: None, last_locals: Vec::new(), hidden: false }, walker))
}

/// A world map trip's way (`game::travel::WorldMap::path`), from the executable beside the
/// install and the world map field in `field`.
fn world_trip(field: &Path, from: &str, to: &str) -> Result<Vec<[f32; 3]>> {
    let root = field.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("the field is not inside a game install")?;
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let stem = field.file_name().and_then(|n| n.to_str()).ok_or("no field name")?;
    let bytes = std::fs::read(field.join(format!("{stem}.elixir.gz")))?;
    let container = Elixir::parse(&bytes)?;
    let entry = container.entries.iter().find(|e| e.name.ends_with(".opl2")).ok_or("the field has no placements")?;
    let placements = barrel_formats::opl2::parse(container.data(entry))?;
    let map = barrel_escha::travel::WorldMap::load(&pe, &placements).ok_or("no routes")?;
    Ok(map.path(&placements, from, to, |_| false).ok_or(format!("no way from {from} to {to}"))?)
}

/// Where world map point `name` stands on the world map field in `field` (field units), from
/// its tagged placement (`barrel_escha::data::world::point_position`).
fn world_point(field: &Path, name: &str) -> Result<[f32; 3]> {
    let stem = field.file_name().and_then(|n| n.to_str()).ok_or("no field name")?;
    let bytes = std::fs::read(field.join(format!("{stem}.elixir.gz")))?;
    let container = Elixir::parse(&bytes)?;
    let entry = container.entries.iter().find(|e| e.name.ends_with(".opl2")).ok_or("the field has no placements")?;
    let placements = barrel_formats::opl2::parse(container.data(entry))?;
    Ok(barrel_escha::data::world::point_position(&placements, name).ok_or(format!("no point {name} on {stem}"))?)
}

/// The area map's camera: its pitch (degrees), distance (units) and field of view (degrees),
/// looking at a point [`AREA_CAMERA_AHEAD`] units north of the player. Fitted to the recordings
/// (`colseit-arrival-60hz.mkv` and `ruins-entrance-lost-battle-60hz.mkv` at 0:45, the same
/// view): Sector One's row of dots (z 31300) and the column toward Ruins Entrance (x 26250, z
/// 30000 to 31000), their centres found in the frame: 0.85 pixels RMS over 14 dots at 720p
/// (the earlier 43 degrees, 5400 and 15 degrees: 28). The field of view and distance trade off
/// (10 degrees at 8150 fits within 1.1 pixels).
const AREA_CAMERA_PITCH: f32 = 35.0;
const AREA_CAMERA_DISTANCE: f32 = 7600.0;
const AREA_CAMERA_FOV: f32 = 10.7;
const AREA_CAMERA_AHEAD: f32 = 178.0;
/// The world map's camera, fitted the same way to `colseit-arrival-60hz.mkv` at 1:25 (the
/// cursor on Colseit: its mark, the dots of R0 east to the Twilight Forest's mark, R1 south and
/// R2 north to the Old Highway's mark, one of them under its label): 5.6 pixels RMS over 13
/// points at 720p, best at a 21-degree view (15 degrees: 9; the earlier 15 degrees, 12000 and
/// 400 ahead, fitted by eye on the 1:12 frame: 16).
const WORLD_CAMERA_PITCH: f32 = 35.0;
/// How much higher a label under its route stands on the world map than on an area map, and
/// how much farther in and higher one beside it (1080p; the recording at 1:25: the Southern
/// Ruins' plate 59 and the Twilight Forest's 35 left and 13 higher at 720p).
const WORLD_LABEL_LIFT: i32 = 80;
const WORLD_LABEL_SIDE: [i32; 2] = [56, 11];
const WORLD_CAMERA_FOV: f32 = 21.0;
const WORLD_CAMERA_DISTANCE: f32 = 8016.0;
const WORLD_CAMERA_AHEAD: f32 = 357.0;

/// How much larger the player is drawn on the maps (the recording at 0:45, at the fitted
/// camera: 170 pixels from her head to her feet against Barrel's 97 at life size).
const AREA_MAP_SCALE: f32 = 1.75;
/// And on the world map, whose camera stands farther off (the recording at 1:12: about as tall
/// on screen as on the area map).
const WORLD_MAP_SCALE: f32 = 3.7;
/// How far (units) the world map's walker goes from her start before the cursor takes over.
const CURSOR_LEAVES: f32 = 60.0;

/// The world map point field area `map` stands for (`SP_A0P0` ...) and where it is (x, z), none
/// for the town's maps and fields on no point.
fn area_point(root: &Path, map: u32) -> Result<Option<(String, [f32; 2])>> {
    if (map as usize) < barrel_escha::data::gimmick::TOWN_MAPS {
        return Ok(None);
    }
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let names = barrel_escha::data::world::point_names(&pe).unwrap_or_default();
    let Some(point) = barrel_escha::data::world::point_maps(&pe).unwrap_or_default().into_iter().find(|(_, m, _)| *m == map).and_then(|(p, _, _)| names.get(p as usize).cloned()) else {
        return Ok(None);
    };
    let at = world_point(&root.join("Data/x64/Field/A15_WMAP"), &point)?;
    Ok(Some((point, [at[0], at[2]])))
}

/// A trip off a field's world-map exit to mark `to`: the world map (its routes from the world
/// map field's placements), the mark the field `from_map` stands for, and the field map `to`
/// opens with the place to arrive at there (field units): the field's record in the arrival
/// table (`world::arrivals`), else its own world-map exit (`WORLDMAP_CHANGE_EX`).
fn world_exit_trip(root: &Path, from_map: u32, to: &str) -> Result<(barrel_escha::travel::WorldMap, String, u32, [f32; 2], Option<f32>)> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let wmap = root.join("Data/x64/Field/A15_WMAP");
    let bytes = std::fs::read(wmap.join("A15_WMAP.elixir.gz"))?;
    let container = Elixir::parse(&bytes)?;
    let entry = container.entries.iter().find(|e| e.name.ends_with(".opl2")).ok_or("the world map has no placements")?;
    let placements = barrel_formats::opl2::parse(container.data(entry))?;
    let map = barrel_escha::travel::WorldMap::load(&pe, &placements).ok_or("no routes")?;
    let names = barrel_escha::data::world::point_names(&pe).unwrap_or_default();
    let tag = |p: u32| names.get(p as usize).and_then(|n| barrel_escha::data::world::point_tag(n));
    let points = barrel_escha::data::world::point_maps(&pe).unwrap_or_default();
    // The town's maps all stand for its mark, T0.
    let from = if (from_map as usize) < barrel_escha::data::gimmick::TOWN_MAPS {
        "T0".to_string()
    } else {
        points.iter().find(|(_, m, _)| *m == from_map).and_then(|(p, _, _)| tag(*p)).ok_or(format!("field {from_map} is on no world map mark"))?
    };
    let field = points.iter().find(|(p, _, _)| tag(*p).as_deref() == Some(to)).map(|(_, m, _)| *m).ok_or(format!("{to} opens no field"))?;
    if let Some(a) = barrel_escha::data::world::arrival(&pe, field) {
        // The record's facing in degrees, the walker's way (atan2(x, z)): Sector One's 180 has
        // Escha's back to its camera, which looks along -z.
        return Ok((map, from, field, [a.position[0] * 100.0, a.position[2] * 100.0], Some(a.facing.to_radians())));
    }
    let areas = barrel_escha::data::gimmick::areas(&pe, barrel_escha::data::gimmick::MAPS_EN, barrel_escha::data::gimmick::MAP_COUNT).ok_or("no map table")?;
    let exit = areas.get(field as usize).and_then(|a| a.gimmicks.iter().find(|g| g.kind == barrel_escha::data::gimmick::WORLDMAP_CHANGE_EX)).ok_or(format!("field {field} has no world map exit"))?;
    Ok((map, from, field, [exit.position[0] * 100.0, exit.position[2] * 100.0], None))
}

/// The party members' new-game defaults (`battle_data::party_defaults`), none when the
/// executable does not read.
fn party_defaults(exe: &[u8]) -> Vec<barrel_escha::data::battle_data::PartyDefault> {
    barrel_formats::pe::Pe::parse(exe).ok().and_then(|pe| barrel_escha::data::battle_data::party_defaults(&pe, barrel_escha::data::battle_data::PARTY_DEFAULTS_EN)).unwrap_or_default()
}

/// The battle tables for field encounters (`barrel_escha::auto_battle`).
fn battle_tables(root: &Path) -> Result<barrel_escha::auto_battle::Tables> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let effects = barrel_formats::table::parse(&std::fs::read(root.join("Saves_EN/item/item_effect.xml"))?)?;
    let chara = std::fs::read(root.join("Saves/chara/Chara.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let items = std::fs::read(root.join("Saves_EN/item/itemData.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let tables = barrel_escha::auto_battle::Tables::load(&pe, &effects, chara.as_ref(), items.as_ref())?;
    let potential = std::fs::read(root.join("Saves_EN/item/item_potential.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    Ok(match potential {
        Some(p) => tables.with_traits(&p),
        None => tables,
    })
}

/// The battle HUD's layer (`battlehud`, layout 886) and what redrawing it needs: the font, the
/// sheets, the placed nodes and the executable's bytes (for its texts).
struct BattleHud {
    layer: text::Layer,
    font: text::Font,
    sheets: hud::Sheets,
    nodes: Vec<barrel_escha::data::layout::Placed>,
    /// The target plate's nodes (layout 920).
    plate_nodes: Vec<barrel_escha::data::layout::Placed>,
    /// The Result screen's nodes (layout 904), shown instead once a won battle is over.
    result_nodes: Vec<barrel_escha::data::layout::Placed>,
    /// The field HUD's nodes (layout 625), for its date panel over a lost battle's white.
    field_nodes: Vec<barrel_escha::data::layout::Placed>,
    exe: Vec<u8>,
    /// What it shows now, and at what size (the wipe: its day and fade step).
    shown: Option<((Vec<battlehud::Member>, Vec<Option<usize>>), Option<battlehud::Commands>, Option<result::Values>, Option<(u32, u32)>, u32, u32, usize, Vec<hud::Element>, Option<String>)>,
}

fn battle_hud(gpu: &Gpu, pool: vk::CommandPool, pass: vk::RenderPass, root: &Path) -> Result<BattleHud> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN).ok_or("battle HUD: no glyph table")?;
    let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t"))?;
    let atlas = G1t::parse(&atlas_bytes)?;
    let nodes = barrel_escha::data::layout::placed(&pe, battlehud::LAYOUT);
    // Every picture it may show: each member's portrait, the bars.
    let members: Vec<battlehud::Member> = (0..3).map(|k| battlehud::Member { chara: k, hp: (1, 1), mp: (1, 1) }).collect();
    let every_face: Vec<Option<usize>> = (0..=15).map(Some).collect();
    let mut every: Vec<hud::Element> = (0..3).flat_map(|f| battlehud::elements(&nodes, |p| barrel_escha::data::layout::text(&pe, p), &members, &every_face, f, Some(1), Some(3))).collect();
    every.extend((0..11).map(|cut| hud::Element::Sprite { part: 0xf2, cut, at: [0, 0] }));
    let all_commands: Vec<i32> = (0..9).collect();
    every.extend((0..2).flat_map(|chosen| battlehud::command_row(&all_commands, chosen)));
    let plate_nodes = barrel_escha::data::layout::placed(&pe, battlehud::TARGET_PLATE);
    every.extend(battlehud::target_plate(&plate_nodes, |p| barrel_escha::data::layout::text(&pe, p), &battlehud::Plate { name: String::new(), hp: (1, 1), at: [0, 0] }));
    // And the Result screen's: its pieces, each member's portrait, both sizes of digits.
    // The battle numbers' digits and the action banner (their sheets).
    every.extend(battlehud::number(-1, [0, 0], 0.5));
    every.extend(battlehud::banner(&pe, "", 0.0));
    let result_nodes = barrel_escha::data::layout::placed(&pe, result::LAYOUT);
    let sample = result::Values { rows: (0..6).map(|chara| result::Row { chara, level: 1, next: 1, level_up: true }).collect(), exp: 1, money: 1 };
    every.extend(result::elements(&result_nodes, |p| barrel_escha::data::layout::text(&pe, p), &sample));
    every.extend((0..11).map(|cut| hud::Element::Sprite { part: 0x4, cut, at: [0, 0] }));
    every.extend((0..10).flat_map(|cut| [0x4b, 0x4c].map(|part| hud::Element::Sprite { part, cut, at: [0, 0] })));
    // And the field HUD's date panel (a lost battle's wipe), every digit and year.
    let field_nodes = barrel_escha::data::layout::placed(&pe, hud::FIELD_HUD);
    every.extend(hud::date_panel(&field_nodes, |p| barrel_escha::data::layout::text(&pe, p), hud::Values { left: 0, month: 10, day: 23, year: 1, tenths_spent: 0, cards: Default::default(), gauge: None }));
    every.extend((0..10).map(|cut| hud::Element::Sprite { part: 0x57, cut, at: [0, 0] }));
    every.extend((3..8).map(|cut| hud::Element::Sprite { part: 0x8a, cut, at: [0, 0] }));
    let keys = screen::sheet_keys(&pe, every);
    let packs = screen::read_packs(root, &keys)?;
    let parsed = packs.iter().map(|(n, b)| Ok((n.clone(), G1t::parse(b)?))).collect::<Result<Vec<_>>>()?;
    let mut images = vec![atlas.textures.first().ok_or("battle HUD: an empty font atlas")?];
    let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
    for (name, index) in keys {
        let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
        sheets.sizes.push([t.width as f32, t.height as f32]);
        sheets.keys.push((name, index));
        images.push(t);
    }
    let size = images[0].width as f32;
    let layer = text::Layer::new(gpu, pool, pass, &images)?;
    Ok(BattleHud { layer, font: text::Font { glyphs, size }, sheets, nodes, plate_nodes, result_nodes, field_nodes, exe, shown: None })
}

/// How long an offscreen run shows a won battle's Result screen before going back.
const RESULT_SECONDS: f32 = 3.0;

/// A lost battle's wipe (`ruins-entrance-lost-battle-60hz.mkv`): the fade to white, then the
/// date panel alone on the white, in seconds (about 2 and 2 in the recording).
/// The most frames an offscreen run writes (its length is this times `--save-every`).
const MAX_WRITTEN_FRAMES: u32 = 600;

/// How far above a fighter's feet its battle numbers stand (world units; the game's anchor
/// not read: a stand-in near a monster's middle, as the recording shows them).
const NUMBER_RAISE: f32 = 100.0;
const WIPE_FADE: f32 = 2.0;
const WIPE_SECONDS: f32 = 4.0;

/// The centre of a battle stage (`BF_POINT*`, 16 × 16 cells of 400: taken as the middle of its
/// ground; not yet read from the game).
const BATTLE_CENTRE: [f32; 3] = [3200.0, 0.0, 3200.0];

/// A battle laid out on a stage (`--battle GROUP`): encounter group GROUP's monsters, drawn as
/// the game draws them, scattered about the centre (`battle::scatter`), and the party (the lead,
/// then the other alchemist) at their places around it (`battle::party_spot`, member k in
/// sector k mod 3), everyone in their `WAIT` motion (the battle idle, `B_WAIT`), the party facing
/// the centre and the monsters the lead.
///
/// `members` are the session's party (its save's `member` lines): each fights at its level in
/// its equipment; without them, at level 1 in the new game's kit.
fn battle_people(root: &Path, group: i32, logy: bool, sound_quest: bool, members: &[barrel_escha::session::Member], scene: &mut Scene, base: usize, first_body: usize) -> Result<(Vec<Vec<u8>>, Vec<TownBody>, BattlePlay)> {
    let mut tables = battle_tables(root)?;
    for (c, m) in members.iter().enumerate() {
        tables.equip(c, &m.equipment);
        tables.carry(c, &m.battle_items);
    }
    let level = |c: usize| members.get(c).map_or(1, |m| m.level.max(1));
    let mut rng = barrel_engine::rng::Rng::seeded(1);
    let drawn = barrel_escha::auto_battle::draw(&tables, group, &mut rng)?;
    let sizes: Vec<f32> = drawn.iter().map(|d| d.1).collect();
    // The battle scatters the monsters first: a copy of its generator gives the same spots.
    let spots = barrel_escha::battle::scatter(&sizes, &mut rng.clone());
    let party = [(usize::from(logy), level(usize::from(logy))), (usize::from(!logy), level(usize::from(!logy)))];
    let monsters: Vec<usize> = drawn.iter().map(|d| d.0).collect();
    // Fought a member's command at a time (`fight_commanded`): up to the first member turn.
    let (outcome, turn) = barrel_escha::auto_battle::fight_commanded(&tables, &party, &monsters, rng, &[])?;
    let table = |name: &str| std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let (charas, bases) = (table("Chara.xml").ok_or("no Chara.xml")?, table("CharaBase.xml").ok_or("no CharaBase.xml")?);
    let base_row = |chara: usize| charas.children.get(chara)?.attr("BaseTag").and_then(|tag| bases.children.iter().find(|b| b.attr("BaseTag") == Some(tag)));
    let at = |p: [f32; 3]| [BATTLE_CENTRE[0] + p[0], BATTLE_CENTRE[1] + p[1], BATTLE_CENTRE[2] + p[2]];
    // Facing a point: the turn (degrees) whose forward, (sin, cos), points from `from` to `to`.
    let facing = |from: [f32; 3], to: [f32; 3]| (to[0] - from[0]).atan2(to[2] - from[2]).to_degrees();
    let mut people: Vec<(usize, [f32; 3], [f32; 3])> = Vec::new();
    let lead = party_spot_of(0);
    for (k, c) in [usize::from(logy), usize::from(!logy)].into_iter().enumerate() {
        let p = party_spot_of(k);
        people.push((c, p, [0.0; 3]));
    }
    for (&(m, _), (spot, _)) in drawn.iter().zip(&spots) {
        people.push((barrel_escha::data::battle_data::MONSTER_CHARA_FIRST + m, *spot, lead));
    }
    let (mut packs, mut tex_base, mut bodies) = (Vec::new(), base, Vec::new());
    let mut fighters = Vec::new();
    for (k, (chara, spot, look)) in people.into_iter().enumerate() {
        fighters.push(None);
        let Some(row) = base_row(chara) else { continue };
        let pos = at(spot);
        let angle = facing(spot, look);
        let Some((body, _)) = add_body(root, scene, tex_base, row, 30_000 + k as u32, pos, angle, "WAIT", None, None, |_, _, _, _| Ok(None::<()>))? else { continue };
        eprintln!("battle: chara {chara} as {:?} at {pos:?} facing {angle:.0}", row.attr("BaseTag"));
        tex_base += body.count;
        packs.extend(body.packs);
        if let Some((skeleton, motions, anim, _)) = body.rig {
            fighters[k] = Some(BattleFighter { body: first_body + bodies.len(), place: pos, motions, back_at: None, down: false, party: k < party.len() });
            bodies.push(TownBody { instance: body.instance, skeleton, anim, clock: Some(0.0), from: None, held: None, last_locals: Vec::new(), hidden: false });
        }
    }
    let music = barrel_escha::auto_battle::group_kind(&tables, group).and_then(|k| barrel_escha::battle::music(k, sound_quest, logy));
    Ok((packs, bodies, BattlePlay {
        fighters,
        now: outcome.start.clone(),
        charas: party.iter().map(|p| p.0).collect(),
        outcome,
        next: 0,
        time: 0.0,
        until: BATTLE_FIRST_STEP,
        fight: BattleFight { tables, party: party.to_vec(), monsters, rng, commands: Vec::new() },
        result: None,
        result_closed: false,
        lost_noted: false,
        items_noted: false,
        numbers: Vec::new(),
        banner: None,
        music,
        victory_played: false,
        wiped: None,
        drops_shown: false,
        turn,
        chosen: 0,
        row: 0,
        choosing: Choosing::Row,
        target: 0,
    }))
}

/// The battle's Fast Forward (0 off, 1 ×1.5, 2 ×2: `battlehud::FAST_FORWARD_SPEEDS`), kept from
/// battle to battle as the game's global is.
static FAST_FORWARD: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Seconds before the battle's first step, and between steps.
const BATTLE_FIRST_STEP: f32 = 1.0;
const BATTLE_STEP: f32 = 1.6;

/// A battle fought out (`auto_battle`) played in the scene: one event a step, the actor in its
/// attack motion and the target in its damage motion, each back to its wait when that ends; a
/// fallen monster gone, a fallen member down. Not yet the game's: the steps' timing, every
/// action as the normal attack's motion (`ATTACK00`), no one moving to their target, no effects,
/// numbers or sounds.
struct BattlePlay {
    /// Per fighter (the party, then the monsters, as the battle numbers them).
    fighters: Vec<Option<BattleFighter>>,
    outcome: barrel_escha::auto_battle::Outcome,
    /// Each fighter's HP, max HP, MP and max MP now.
    now: Vec<[i32; 4]>,
    /// The party's character numbers (the first fighters).
    charas: Vec<usize>,
    /// What fights the battle again with each new command.
    fight: BattleFight,
    /// The member turn waiting for a command (once the events before it have played).
    turn: Option<barrel_escha::auto_battle::Turn>,
    /// The command list's choice (0 the attack, then the skills) and the target's place among
    /// the standing monsters.
    chosen: usize,
    target: usize,
    /// Where on the command row, and what the turn is choosing.
    row: usize,
    choosing: Choosing,
    next: usize,
    time: f32,
    until: f32,
    /// A won battle's Result screen once it is over, with the time it came up, and whether a
    /// confirm press has closed it.
    result: Option<(result::Values, f32)>,
    result_closed: bool,
    /// A lost battle's day has gone into the field's save.
    lost_noted: bool,
    /// The battle items' spent uses are in the field's save.
    items_noted: bool,
    /// The numbers up over fighters (`battlehud::number`): fighter, value, when it came up.
    numbers: Vec<(usize, i32, f32)>,
    /// The action playing (its name in the banner, layout 885) and when it began.
    banner: Option<(String, f32)>,
    /// The battle's music track (`battle::music`), played in a window.
    music: Option<u32>,
    /// The victory music has started (with the Result screen).
    victory_played: bool,
    /// A lost battle's wipe: when it began and the day it turns to.
    wiped: Option<(f32, u32)>,
    /// A won battle's drops screen has been shown.
    drops_shown: bool,
}

/// What a member's turn is choosing, as the game's battle goes: a command on the row (Left and
/// Right), a skill from the list (Up and Down), then the target (Left and Right) for the attack
/// or the skill picked; Back goes a step back.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Choosing {
    Row,
    Skill,
    Target { skill: Option<usize> },
    /// The item list (the Item command).
    Item,
    /// A monster for item `0` (one thrown at the other side).
    ItemTarget(usize),
}

/// The command row an alchemist's turn shows (the battle-stats recording: Katla's Attack,
/// Skill, Move, Wait, Change and Run; the alchemists' Item taken to go in its command order,
/// after Attack, `docs/escha/open.md`). Change is not given yet.
const COMMAND_ROW: [i32; 7] = {
    use battlehud::command::*;
    [ATTACK, ITEM, SKILL, MOVE, WAIT, CHANGE, RUN]
};

/// A battle's tables, sides, generator and the commands given so far.
struct BattleFight {
    tables: barrel_escha::auto_battle::Tables,
    party: Vec<(usize, i32)>,
    monsters: Vec<usize>,
    rng: barrel_engine::rng::Rng,
    commands: Vec<barrel_escha::auto_battle::Command>,
}

struct BattleFighter {
    /// Its body among the town bodies.
    body: usize,
    /// Where it stands (field units).
    place: [f32; 3],
    motions: Motions,
    /// When its one-off motion ends (back to its wait).
    back_at: Option<f32>,
    down: bool,
    party: bool,
}

impl BattlePlay {
    fn set(&mut self, k: usize, field: usize, value: i32) {
        if let Some(v) = self.now.get_mut(k) {
            v[field] = value;
        }
    }

    /// Whether the battle is won and played out (a moment after its last event).
    fn over(&self) -> bool {
        self.turn.is_none() && self.next > self.outcome.events.len() && self.time >= self.until
    }

    /// Whether a member's turn waits for a command (its events before it all played).
    fn waiting(&self) -> bool {
        self.turn.is_some() && self.next >= self.outcome.events.len() && self.time >= self.until
    }

    /// The command list: the attack, then the member's skills (with their MP costs; those it
    /// cannot pay for marked), and the chosen target's name.
    /// `screen`: a fighter's place on the 1920 × 1080 screen (for the target plate).
    fn commands(&self, screen: impl Fn(usize) -> Option<[i32; 2]>) -> Option<battlehud::Commands> {
        let turn = self.turn.as_ref().filter(|_| self.waiting())?;
        let target = turn.foes.get(self.target).copied();
        let plate = target.filter(|_| matches!(self.choosing, Choosing::Target { .. } | Choosing::ItemTarget(_))).and_then(|k| {
            let v = self.now.get(k)?;
            Some(battlehud::Plate { name: self.outcome.names.get(k).cloned().unwrap_or_default(), hp: (v[0], v[1]), at: screen(k)? })
        });
        let list: Vec<(String, bool)> = match self.choosing {
            Choosing::Item => turn.items.iter().map(|(name, left, _)| (format!("{name}  ×{left}"), *left > 0)).collect(),
            _ => turn.skills.iter().map(|(name, cost, can)| (format!("{name}  {cost} MP"), *can)).collect(),
        };
        Some(battlehud::Commands {
            row: COMMAND_ROW.to_vec(),
            at: self.row,
            skills: matches!(self.choosing, Choosing::Skill | Choosing::Item).then(|| (list, self.chosen)),
            title: String::new(),
            marked: target.filter(|_| plate.is_some()).and_then(|k| turn.slots.iter().skip(1).position(|&o| o == Some(k))).map(|p| p + 1),
            // The mark follows the command under the cursor: the skill's in the skill list.
            next: turn.next_places.get(match self.choosing {
                Choosing::Skill => 1 + self.chosen,
                Choosing::Row if COMMAND_ROW[self.row] == battlehud::command::WAIT => turn.skills.len() + 1,
                _ => 0,
            }).or(turn.next_places.first()).copied().unwrap_or(0),
            plate,
        })
    }

    /// Fighter `k`'s place on the stage, if it has a body.
    fn place_of(&self, k: usize) -> Option<[f32; 3]> {
        self.fighters.get(k)?.as_ref().map(|f| f.place)
    }

    /// Fighter `k`'s body, if it has one.
    fn body_of(&self, k: usize) -> Option<usize> {
        self.fighters.get(k)?.as_ref().map(|f| f.body)
    }

    /// A button on a member's turn: up and down choose the command, left and right the target,
    /// confirm gives it (the battle is fought again with it, the events after it to play).
    fn press(&mut self, b: title::Button) {
        if self.result.is_some() {
            self.result_closed |= b == title::Button::Confirm;
            return;
        }
        let Some(turn) = self.turn.clone().filter(|_| self.waiting()) else { return };
        let skills = turn.skills.len().max(1);
        let items = turn.items.len().max(1);
        let foes = turn.foes.len().max(1);
        let row = COMMAND_ROW.len();
        use title::Button::*;
        match (self.choosing, b) {
            (Choosing::Row, Left) => self.row = (self.row + row - 1) % row,
            (Choosing::Row, Right) => self.row = (self.row + 1) % row,
            (Choosing::Row, Confirm) => match COMMAND_ROW[self.row] {
                battlehud::command::ATTACK => self.choosing = Choosing::Target { skill: None },
                battlehud::command::SKILL if !turn.skills.is_empty() => self.choosing = Choosing::Skill,
                battlehud::command::ITEM if !turn.items.is_empty() => (self.choosing, self.chosen) = (Choosing::Item, 0),
                battlehud::command::WAIT => self.give(barrel_escha::auto_battle::Command::Wait),
                battlehud::command::RUN => self.give(barrel_escha::auto_battle::Command::Run),
                // the next sector round (the game's choice of where, and which way, is not read)
                battlehud::command::MOVE => self.give(barrel_escha::auto_battle::Command::Move { sector: turn.sector + 1 }),
                c => eprintln!("battle: command {c} is not given yet"),
            },
            (Choosing::Skill, Up) => self.chosen = (self.chosen + skills - 1) % skills,
            (Choosing::Skill, Down) => self.chosen = (self.chosen + 1) % skills,
            (Choosing::Skill, Confirm) if turn.skills.get(self.chosen).is_some_and(|s| s.2) => self.choosing = Choosing::Target { skill: Some(self.chosen) },
            (Choosing::Skill, Back) => self.choosing = Choosing::Row,
            (Choosing::Item, Up) => self.chosen = (self.chosen + items - 1) % items,
            (Choosing::Item, Down) => self.chosen = (self.chosen + 1) % items,
            (Choosing::Item, Back) => self.choosing = Choosing::Row,
            (Choosing::Item, Confirm) => match turn.items.get(self.chosen) {
                Some((_, left, true)) if *left > 0 => self.choosing = Choosing::ItemTarget(self.chosen),
                // on the party: the member itself (the battle takes the first of its side when
                // the item cannot be aimed there; choosing a member is not given yet)
                Some((_, left, false)) if *left > 0 => self.give(barrel_escha::auto_battle::Command::Item { item: self.chosen, target: turn.who }),
                _ => {}
            },
            (Choosing::ItemTarget(_), Left) => self.target = (self.target + foes - 1) % foes,
            (Choosing::ItemTarget(_), Right) => self.target = (self.target + 1) % foes,
            (Choosing::ItemTarget(item), Back) => (self.choosing, self.chosen) = (Choosing::Item, item),
            (Choosing::ItemTarget(item), Confirm) => {
                let target = turn.foes.get(self.target).copied().unwrap_or(0);
                self.give(barrel_escha::auto_battle::Command::Item { item, target });
            }
            (Choosing::Target { .. }, Left) => self.target = (self.target + foes - 1) % foes,
            (Choosing::Target { .. }, Right) => self.target = (self.target + 1) % foes,
            (Choosing::Target { skill }, Back) => self.choosing = if skill.is_some() { Choosing::Skill } else { Choosing::Row },
            (Choosing::Target { skill }, Confirm) => {
                use barrel_escha::auto_battle::Command;
                let target = turn.foes.get(self.target).copied().unwrap_or(0);
                self.give(match skill {
                    None => Command::Attack { target },
                    Some(skill) => Command::Skill { skill, target },
                });
            }
            _ => {}
        }
    }

    /// Gives the waiting member `command` and plays the battle on to the next member's turn.
    fn give(&mut self, command: barrel_escha::auto_battle::Command) {
        {
            {
                self.fight.commands.push(command);
                let f = &self.fight;
                match barrel_escha::auto_battle::fight_commanded(&f.tables, &f.party, &f.monsters, f.rng, &f.commands) {
                    Ok((outcome, turn)) => {
                        self.outcome = outcome;
                        self.turn = turn;
                    }
                    Err(e) => {
                        eprintln!("battle: {e}");
                        self.turn = None;
                    }
                }
                (self.chosen, self.target, self.row, self.choosing) = (0, 0, 0, Choosing::Row);
                self.until = self.time;
            }
        }
    }

    /// The turn order bar's portraits, the one acting first: a party member's portrait cut (its
    /// number), a monster's [`battlehud::MONSTER_FACES`] on from its monster number.
    fn order(&self) -> Vec<Option<usize>> {
        let party = self.charas.len();
        let face = |k: usize| match self.charas.get(k) {
            Some(&c) => Some(c),
            None => self.outcome.monsters.get(k - party).map(|&m| battlehud::MONSTER_FACES + m),
        };
        // One slot a timeline entry, free ones empty (`Turn::slots`).
        self.turn.as_ref().map_or(Vec::new(), |t| t.slots.iter().map(|k| k.and_then(face)).collect())
    }

    /// The party as the HUD shows it.
    fn members(&self) -> Vec<battlehud::Member> {
        self.charas
            .iter()
            .enumerate()
            .filter_map(|(k, &chara)| {
                let v = self.now.get(k)?;
                Some(battlehud::Member { chara, hp: (v[0], v[1]), mp: (v[2], v[3]) })
            })
            .collect()
    }

    /// Plays `column` on fighter `k` once, from the start.
    fn play(&mut self, bodies: &mut [TownBody], k: usize, column: &str) {
        let time = self.time;
        let Some(f) = self.fighters.get_mut(k).and_then(Option::as_mut) else { return };
        let Some(anim) = f.motions.get(column).cloned() else { return };
        f.back_at = Some(time + anim.duration);
        if let Some(b) = bodies.get_mut(f.body) {
            b.switch(anim);
        }
    }

    /// One frame: the bodies' clocks, motions back to their wait, and the next event when due.
    fn step(&mut self, bodies: &mut [TownBody], dt: f32) {
        self.time += dt;
        let now = self.time;
        self.numbers.retain(|&(_, _, at)| now - at < battlehud::NUMBER_SECONDS);
        for f in self.fighters.iter_mut().flatten() {
            let Some(b) = bodies.get_mut(f.body) else { continue };
            b.clock = Some(b.clock.unwrap_or(0.0) + dt);
            if let Some((_, clock, blend)) = &mut b.from {
                *clock += dt;
                *blend += dt;
            }
            if b.from.as_ref().is_some_and(|x| x.2 >= MOTION_BLEND) {
                b.from = None;
            }
            if f.back_at.is_some_and(|t| self.time >= t) {
                f.back_at = None;
                if f.down && !f.party {
                    b.hidden = true;
                } else if let Some(anim) = f.motions.get(if f.down { "DOWN_LOOP" } else { "WAIT" }).cloned() {
                    b.switch(anim);
                }
            }
        }
        if self.time < self.until {
            return;
        }
        // Nothing to play: a member's turn waits (the step's time kept for its command), or the
        // battle is played out.
        if (self.next >= self.outcome.events.len() && self.turn.is_some()) || self.next > self.outcome.events.len() {
            return;
        }
        self.until = self.time + BATTLE_STEP;
        let Some(event) = self.outcome.events.get(self.next).cloned() else {
            if self.next == self.outcome.events.len() && self.turn.is_none() {
                self.next += 1;
                if let Some(end) = self.outcome.lines().last() {
                    eprintln!("battle: {end}");
                }
            }
            return;
        };
        if let Some(line) = self.outcome.lines().get(self.next) {
            eprintln!("battle: {line}");
        }
        self.next += 1;
        use barrel_escha::fight::Event;
        match event {
            Event::Acted { who, target, hp_left, mp_left, .. } | Event::Healed { who, target, hp_left, mp_left, .. } => {
                self.set(target, 0, hp_left);
                self.set(who, 2, mp_left);
            }
            Event::Felled { target, .. } => self.set(target, 0, 0),
            Event::Poisoned { who, hp_left, .. } => self.set(who, 0, hp_left),
            // the run's and the move's motions are not played yet
            Event::Ran { .. } | Event::Moved { .. } => {}
        }
        match &event {
            Event::Acted { skill, .. } | Event::Healed { skill, .. } | Event::Felled { skill, .. } => self.banner = Some((skill.clone(), self.time)),
            _ => {}
        }
        match event {
            Event::Acted { target, damage, .. } if damage > 0 => self.numbers.push((target, -damage, self.time)),
            Event::Healed { target, amount, .. } if amount > 0 => self.numbers.push((target, amount, self.time)),
            Event::Poisoned { who, damage, .. } if damage > 0 => self.numbers.push((who, -damage, self.time)),
            _ => {}
        }
        let (who, target, fell) = match event {
            Event::Acted { who, target, hp_left, .. } => (Some(who), Some(target), hp_left <= 0),
            Event::Healed { who, target, .. } => (Some(who), Some(target).filter(|&t| t != who), false),
            Event::Felled { who, target, .. } => (Some(who), Some(target), true),
            Event::Poisoned { who, hp_left, .. } => (None, Some(who), hp_left <= 0),
            Event::Ran { .. } | Event::Moved { .. } => (None, None, false),
        };
        if let Some(w) = who {
            self.play(bodies, w, "ATTACK00");
        }
        if let Some(t) = target {
            if fell {
                if let Some(f) = self.fighters.get_mut(t).and_then(Option::as_mut) {
                    f.down = true;
                }
                let party = self.fighters.get(t).and_then(Option::as_ref).is_some_and(|f| f.party);
                self.play(bodies, t, if party { "DOWN" } else { "DAMAGE" });
            } else if matches!(event, Event::Acted { damage, .. } if damage > 0) || matches!(event, Event::Poisoned { .. }) {
                self.play(bodies, t, "DAMAGE");
            }
        }
    }
}

/// Party member k's battle place (sector k mod 3).
fn party_spot_of(k: usize) -> [f32; 3] {
    barrel_escha::battle::party_spot(k as i32, (k % 3) as i32)
}

fn field_creatures(root: &Path, area: i32, scene: &mut Scene, base: usize) -> Result<(barrel_escha::field_ai::Field, Vec<FieldBody>, Vec<Vec<u8>>, Vec<String>, [f64; 3])> {
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let script = |name: &str| -> std::result::Result<Vec<barrel_engine::squirrel::Stmt>, String> {
        let bytes = std::fs::read(root.join("Script").join(format!("{name}.nut"))).map_err(|e| format!("{name}: {e}"))?;
        let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
        barrel_engine::squirrel::parse(&text).map_err(|e| format!("{name}: {e}"))
    };
    let table = |name: &str| std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let (charas, bases) = (table("Chara.xml").ok_or("no Chara.xml")?, table("CharaBase.xml").ok_or("no CharaBase.xml")?);
    let (field, enemies) = barrel_escha::field_ai::build(&pe, area, script, Some(&charas), Some(&bases))?;
    let groups = barrel_escha::data::battle_data::encounter_groups(&pe, barrel_escha::data::battle_data::ENCOUNTER_GROUPS_EN);
    let (mut bodies, mut packs, mut tex_base) = (Vec::new(), Vec::new(), base);
    for (k, e) in enemies.iter().enumerate() {
        // The model the game shows it as (`FieldEnemy::model`, a CharaBase row); its group's
        // first monster's when that row is missing.
        let Some(row) = usize::try_from(e.model).ok().and_then(|m| bases.children.get(m)).or_else(|| {
            barrel_escha::field_ai::body_row(&groups, e)
                .and_then(|r| charas.children.get(r)?.attr("BaseTag"))
                .and_then(|tag| bases.children.iter().find(|b| b.attr("BaseTag") == Some(tag)))
        }) else {
            continue;
        };
        let c = &field.mobs[k].creature;
        let pos = c.position.map(|v| v as f32);
        let Some((body, _)) = add_body(root, scene, tex_base, row, 10_000 + k as u32, pos, c.facing.to_degrees() as f32, "F_WAIT", None, None, |_, _, _, _| Ok(None::<()>))? else { continue };
        eprintln!("enemy {k} ({}) as {:?} at {pos:?}", e.ai.1, row.attr("BaseTag"));
        tex_base += body.count;
        packs.extend(body.packs);
        if let Some((skeleton, motions, anim, _)) = body.rig {
            bodies.push(FieldBody { mob: k, instance: body.instance, skeleton, motions, column: "F_WAIT".into(), anim, since: 0.0 });
        }
    }
    // The stand-in player: Escha (Chara row 0) walking west from 15 m east of the first enemy.
    let start = field.mobs.first().map_or([0.0; 3], |m| m.creature.position);
    let at = [start[0] + 1500.0, start[1], start[2]];
    let escha = charas.children.first().and_then(|c| c.attr("BaseTag")).and_then(|tag| bases.children.iter().find(|b| b.attr("BaseTag") == Some(tag)));
    if let Some(row) = escha
        && let Some((body, _)) = add_body(root, scene, tex_base, row, 0, at.map(|v| v as f32), -90.0, "WALK", None, None, |_, _, _, _| Ok(None::<()>))? {
            packs.extend(body.packs);
            if let Some((skeleton, motions, anim, _)) = body.rig {
                bodies.push(FieldBody { mob: usize::MAX, instance: body.instance, skeleton, motions, column: "WALK".into(), anim, since: 0.0 });
            }
        }
    Ok((field, bodies, packs, barrel_escha::gesture::motions(&pe), at))
}

/// What a character's face needs to change during playback: its rows, its facial
/// animations (loaded by name, kept), and its morph sets' places in the weight buffer.
struct FaceRig {
    row: u32,
    head: String,
    facials: Vec<u8>,
    /// Per morph set (sorted by layout, as the weights are laid out): its target count and
    /// where its weights begin in the instance's block.
    sets: Vec<(usize, u32)>,
    loaded: std::collections::HashMap<String, Option<barrel_formats::g1a::Animation>>,
}

impl FaceRig {
    fn anim(&mut self, name: &str) -> Option<&barrel_formats::g1a::Animation> {
        if !self.loaded.contains_key(name) {
            let anim = Elixir::parse(&self.facials)
                .ok()
                .and_then(|c| named(&c, &format!("{name}.g1a")).map(|b| b.to_vec()))
                .and_then(|b| barrel_formats::g1a::parse(&b).ok());
            self.loaded.insert(name.to_string(), anim);
        }
        self.loaded.get(name).and_then(Option::as_ref)
    }

    /// The weights for a face (brow and eyes held at 0.5 s; the mouth at its end, or moving
    /// over `time` while the character talks), into `block` (the instance's weights).
    fn weights(&mut self, face: &barrel_escha::face::Face, talking: bool, time: f32, block: &mut [f32]) {
        block.fill(0.0);
        for (name, t) in [(&face.brow, Some(0.5)), (&face.eye, Some(0.5)), (&face.mouth, None)] {
            let Some(name) = name.clone() else { continue };
            let sets = self.sets.clone();
            let Some(anim) = self.anim(&name) else { continue };
            let Some(&(n, offset)) = sets.iter().find(|(n, _)| n + 1 == anim.tracks.len()) else { continue };
            let at = t.unwrap_or(if talking && anim.duration > 0.0 { time.rem_euclid(anim.duration) } else { 1.0 });
            let w = face_weights(anim, n, at);
            let o = offset as usize;
            if let Some(slot) = block.get_mut(o..o + n) {
                slot.copy_from_slice(&w);
            }
        }
    }
}

/// A character's motions: `<FileHead>_<value>.g1a` for a `CharaBase` column's value, in the
/// `<Motion1>` container or in a container of its own beside it. Loaded on demand, kept once
/// loaded.
struct Motions {
    char_dir: PathBuf,
    head: String,
    container: String,
    /// The `CharaBase` row's columns (`F_WAIT` → `ACT_DEF`, ...).
    columns: std::collections::HashMap<String, String>,
    loaded: std::collections::HashMap<String, Option<barrel_formats::g1a::Animation>>,
    /// The `Neck` container (`<FileHead>_NECK`), with the head gestures' clip.
    neck: Option<String>,
}

impl Motions {
    /// The head gestures' clip (`<FileHead>_M_HEAD.g1a` in the `Neck` container) and the joints
    /// it turns (more than half a degree anywhere in it): `s116` lays only those over the body.
    fn head_gestures(&self) -> Option<(barrel_formats::g1a::Animation, Vec<usize>)> {
        let bytes = std::fs::read(self.char_dir.join(format!("{}.elixir.gz", self.neck.as_ref()?))).ok()?;
        let c = Elixir::parse(&bytes).ok()?;
        let anim = barrel_formats::g1a::parse(named(&c, &format!("{}_M_HEAD.g1a", self.head))?).ok()?;
        let frames = (anim.duration * 30.0).round() as usize;
        let moving = anim
            .tracks
            .iter()
            .filter(|t| {
                let Some((first, ..)) = t.pose(0.0) else { return false };
                let first = glam::Quat::from_array(first).normalize();
                (0..=frames).filter_map(|f| t.pose(f as f32 / 30.0)).any(|(q, ..)| glam::Quat::from_array(q).normalize().angle_between(first) > 0.5f32.to_radians())
            })
            .map(|t| t.bone as usize)
            .collect();
        Some((anim, moving))
    }

    fn new(root: &Path, model: &str, row: &barrel_formats::table::Element) -> Option<Motions> {
        let dir = model.rsplit_once('/').map_or("", |(d, _)| d);
        Some(Motions {
            char_dir: root.join(if row.attr("Directory") == Some("Obj") { "Data/x64/Obj" } else { "Data/x64/Character" }).join(dir),
            head: row.attr("FileHead")?.to_string(),
            container: row.attr("Motion1")?.to_string(),
            columns: row.attributes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            loaded: Default::default(),
            neck: row.attr("Neck").map(str::to_string),
        })
    }

    /// The motion in `column`, if the row has one and its file is there.
    fn get(&mut self, column: &str) -> Option<&barrel_formats::g1a::Animation> {
        if !self.loaded.contains_key(column) {
            // An object's row (the map cursors) names no motions: its one motion is
            // `<FileHead>.g1a` in its `Motion1` container, asked for as `BASE_MOTION1`.
            let base = (column == BASE_MOTION && !self.columns.contains_key(column)).then(String::new);
            let anim = self.columns.get(column).cloned().or(base).and_then(|value| {
                let name = if value.is_empty() { format!("{}.g1a", self.head) } else { format!("{}_{value}.g1a", self.head) };
                let from = |file: String| -> Option<Vec<u8>> {
                    let bytes = std::fs::read(self.char_dir.join(file)).ok()?;
                    let c = Elixir::parse(&bytes).ok()?;
                    named(&c, &name).map(|b| b.to_vec())
                };
                let bytes = from(format!("{}.elixir.gz", self.container)).or_else(|| from(format!("{}_{value}.elixir.gz", self.head)))?;
                barrel_formats::g1a::parse(&bytes).ok()
            });
            self.loaded.insert(column.to_string(), anim);
        }
        self.loaded.get(column).and_then(Option::as_ref)
    }
}

/// The motion name an object's own motion is asked for by.
const BASE_MOTION: &str = "BASE_MOTION1";

/// An event character's placement: number, position and facing angle in degrees.
type Placement = (u32, [f32; 3], f32);

fn event_characters(
    event_path: &Path,
    scene: &mut Scene,
    base: usize,
    line: Option<usize>,
    at: Option<f32>,
    actors: &mut Vec<Actor>,
    absent_out: &mut std::collections::HashSet<u32>,
    costumes: &[(u32, usize, usize, usize)],
) -> Result<(Vec<Vec<u8>>, Option<Vec3>, Option<EventCamera>, Vec<Placement>, Option<f32>, Option<u32>)> {
    use barrel_formats::ebd::{self, Ebd, OperandTable};
    let event = Ebd::parse(&std::fs::read(event_path)?)?;
    let root = event_path.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
    let table = |name: &str| -> Result<barrel_formats::table::Element> {
        Ok(barrel_formats::table::parse(&std::fs::read(root.join("Saves/chara").join(name))?)?)
    };
    let (charas, bases) = (table("Chara.xml")?, table("CharaBase.xml")?);
    // Faces at message line `line`: each speaker's latest expression up to it (the line's
    // fourth field), with the line's own speaker talking.
    let mut faces: std::collections::HashMap<i32, (i32, bool)> = Default::default();
    // And each speaker's latest gesture (the line's third field).
    let mut gestures: std::collections::HashMap<i32, i32> = Default::default();
    let mut face_tables = None;
    let mut gesture_table: Option<(Vec<[Option<barrel_escha::gesture::Form>; 2]>, Vec<String>)> = None;
    if let Some(n) = line {
        let stem = event_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ebm = event_path.with_file_name(format!("{}.ebm", stem.replacen("EVENT_", "EVENT_MESSAGE_", 1)));
        let messages = barrel_formats::ebm::parse(&std::fs::read(&ebm)?)?;
        for (i, m) in messages.iter().enumerate().take(n + 1) {
            if m.fields[3] >= 0 {
                faces.insert(m.speaker, (m.fields[3], false));
            }
            if m.fields[2] >= 0 {
                gestures.insert(m.speaker, m.fields[2]);
            }
            if i == n {
                if let Some(text) = Some(m.repaired_text()) {
                    eprintln!("line {n}, speaker {}: {}", m.speaker, text.replace("<CR>", " "));
                }
                faces.entry(m.speaker).or_insert((0, false)).1 = true;
            }
        }
    }
    // The face and gesture tables (always: a played event changes faces as it goes).
    for entry in std::fs::read_dir(root)?.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|x| x == "exe") {
            let bytes = std::fs::read(&p)?;
            if let Ok(pe) = barrel_formats::pe::Pe::parse(&bytes) {
                let t = barrel_escha::face::Tables::read(&pe);
                if !t.brow.is_empty() {
                    face_tables = Some(t);
                    if let Some(g) = barrel_escha::gesture::read(&bytes) {
                        gesture_table = Some((g, barrel_escha::gesture::motions(&pe)));
                    }
                    break;
                }
            }
        }
    }
    // First placement of each character: q4 (ADDITION_EX: number, ..., x, y, z, ?, angle) or
    // s4 (POS_SET: number, 0, 0, 0, x, y, z).
    let mut first: Vec<(u32, [f32; 3], f32)> = Vec::new();
    let mut camera = None;
    let ops = OperandTable::escha();
    for a in &event.actions {
        for c in ebd::decode(&a.words, &ops)? {
            let o = c.operands;
            if c.word == 0x7410_0014 && o.len() >= 11 && camera.is_none() {
                let i = |k: usize| o[k] as i32 as f32;
                camera = Some(EventCamera {
                    eye: None,
                    target: Vec3::new(i(5) + i(8), i(6) + i(9), i(7) + i(10)),
                    distance: i(4),
                    yaw: i(0) / 100.0,
                    pitch: i(3) / 100.0,
                    fov: i(2) / 100.0,
                });
            }
            let place = match c.word {
                0x7100_0004 if o.len() >= 9 => Some((o[0], [o[4], o[5], o[6]], o[8] as i32 as f32)),
                0x7300_0004 if o.len() >= 7 => Some((o[0], [o[4], o[5], o[6]], 0.0)),
                _ => None,
            };
            if let Some((who, p, angle)) = place
                && !first.iter().any(|f| f.0 == who) {
                    first.push((who, p.map(|v| v as i32 as f32), angle));
                }
        }
    }
    let mut stage_motions: std::collections::HashMap<u32, (String, f32, bool)> = Default::default();
    // The field the stage is in at `at` (none: where the event starts).
    let mut start_field = None;
    let absent = absent_out;
    // --at SECONDS: where the stage has everyone then (barrel_engine::stage), and its camera
    // when it has set both the eye and the target.
    // The same schedule playback uses: the event's lines, each held for a reading time. The
    // English lines when there are (as playback reads them: the reading times follow the text,
    // so other lines would put the stage out of step with what is shown).
    let lines = if at.is_some() || line.is_some() {
        let stem = event_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ebm = event_path.with_file_name(format!("{}.ebm", stem.replacen("EVENT_", "EVENT_MESSAGE_", 1)));
        let english = event_path.strip_prefix(root.join("Event")).ok().map(|rel| root.join("Event_EN").join(rel).with_file_name(ebm.file_name().unwrap_or_default()));
        let read_lines = |p: &Path| std::fs::read(p).ok().and_then(|b| barrel_formats::ebm::parse(&b).ok()).filter(|m| !m.is_empty());
        english.as_deref().and_then(read_lines).or_else(|| read_lines(&ebm)).unwrap_or_default()
    } else {
        Vec::new()
    };
    let schedule = if at.is_some() || line.is_some() {
        Some(barrel_engine::event::Player::new(&event, &lines, &ops)?.reading_time(barrel_engine::event::reading_time).schedule())
    } else {
        None
    };
    // --line N without --at: from the time line N starts (its camera and stage then).
    let at = at.or_else(|| {
        let n = line?;
        let t = schedule.as_ref()?.iter().find(|t| matches!(t.step, barrel_engine::event::Step::Message { index, .. } if index == n))?;
        eprintln!("line {n} starts at {:.2} s", t.time);
        Some(t.time + barrel_engine::stage::FRAME)
    });
    if let Some(at) = at {
        use barrel_engine::stage::{Measures, Stage, FRAME};
        let curves = std::fs::read(root.join("Saves/curve/curve_data.xml"))
            .ok()
            .and_then(|b| barrel_formats::table::parse(&b).ok())
            .map(|t| barrel_engine::curve::Curve::table(&t))
            .unwrap_or_default();
        let steps = schedule.unwrap_or_default();
        let upto: Vec<_> = steps.into_iter().filter(|t| t.time <= at).collect();
        let mut stage = Stage { measures: Measures::table(&charas), curves, ..Stage::default() }.play(&upto, |_, _| {});
        stage.run_until(at + FRAME * 0.5);
        start_field = stage.field;
        // Everyone the event ever places is loaded (so playing on can show characters who come
        // later); those not on the stage at `at` start hidden.
        let later: Vec<Placement> = first.iter().filter(|f| !stage.characters.contains_key(&f.0)).copied().collect();
        first = stage.characters.iter().map(|(&n, b)| (n, b.position, b.facing.to_degrees())).collect();
        absent.extend(later.iter().map(|f| f.0));
        first.extend(later);
        // Each character's motion then, by its MOTION_* name (a CharaBase column), and how far in.
        let names = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_escha::gesture::motions(&pe)).filter(|m| !m.is_empty()))
            .unwrap_or_default();
        for (&n, b) in &stage.characters {
            if let Some(m) = b.motion
                && let Some(name) = names.get(m.motion as usize) {
                    stage_motions.insert(n, (name.clone(), at - m.since, m.looping));
                }
        }
        // The camera in use at that time: the field camera unless a manual one was set after
        // it (the stage's snapshot decides, as during playback).
        let snap = stage.snapshot();
        if let (Some(e), Some(t)) = (snap.eye, snap.target) {
            let (e, t) = (Vec3::from(e), Vec3::from(t));
            camera = Some(EventCamera { eye: Some(e), target: t, distance: (e - t).length().max(1.0), yaw: 0.0, pitch: 0.0, fov: snap.fov.unwrap_or(40.0) });
        }
        eprintln!("at {at} s: {} characters on the stage, {} positions not placed", first.len(), stage.unresolved);
    }
    let placements: Vec<Placement> = first.clone();
    let mut packs = Vec::new();
    let mut tex_base = base;
    let mut centre = Vec3::ZERO;
    let mut placed = 0.0f32;
    for (who, pos, angle) in first {
        let Some(tag) = charas.children.get(who as usize).and_then(|c| c.attr("BaseTag")) else { continue };
        // A chosen outfit (`--costume`): its body's model, and its head accessory and the
        // body's nodes hidden for it, by the costume tables.
        let (tag, wear) = match costumes.iter().find(|c| c.0 == who) {
            Some(&(_, b, h, f)) => match outfit(root, b, h, f)? {
                Some((body_tag, wear)) => (body_tag, Some(wear)),
                None => (tag.to_string(), None),
            },
            None => (tag.to_string(), None),
        };
        let tag = tag.as_str();
        let Some(row_index) = bases.children.iter().position(|b| b.attr("BaseTag") == Some(tag)) else { continue };
        let row = &bases.children[row_index];
        // The body motion: the speaker's gesture at this line if any (its main motion's
        // CharaBase column), otherwise the field wait (F_WAIT). The animation is
        // <FileHead>_<value>.g1a, in the <Motion1> container or in a container of its own
        // beside it, on the skeleton in the base model (<stem>.g1m).
        let staged = stage_motions.get(&who).filter(|(c, ..)| row.attr(c).is_some());
        let column = staged.map(|(c, ..)| c.clone()).or_else(|| gestures
            .get(&(who as i32))
            .and_then(|&g| {
                let (table, names) = gesture_table.as_ref()?;
                let form = table.get(g as usize)?[0].as_ref()?;
                names.get(form.main_motion() as usize).cloned()
            })
            .filter(|c| row.attr(c).is_some()))
            .unwrap_or_else(|| "F_WAIT".to_string());
        // How far into it: the stage's time in the motion (looping ones wrap), else the middle.
        let motion_time = staged.map(|&(_, t, looping)| (t, looping));
        let looping = motion_time.is_none_or(|(_, l)| l);
        // The face: every morph set goes in (so playback can change it), weighted by the line's
        // expression when `--line` gives one.
        let face = |scene: &mut Scene, container: &Elixir, model: &str, stem: &str| -> Result<Option<FaceRig>> {
            let (Some(tables), Some(head), Some(facial)) = (&face_tables, row.attr("FileHead"), row.attr("Facial")) else { return Ok(None) };
            let dir = model.rsplit_once('/').map_or("", |(d, _)| d);
            let fbytes = std::fs::read(root.join("Data/x64/Character").join(dir).join(format!("{facial}.elixir.gz")));
            let (Ok(fbytes), Some(g1h)) = (fbytes, named(container, &format!("{stem}_default.g1h"))) else { return Ok(None) };
            let facials = Elixir::parse(&fbytes)?;
            let mut anims = Vec::new();
            if let Some(&(expr, talking)) = faces.get(&(who as i32)) {
                let face = tables.expression(row_index as u32, head, expr, talking);
                for (name, t) in [(&face.brow, 0.5), (&face.eye, 0.5), (&face.mouth, 1.0)] {
                    if let Some(bytes) = name.as_ref().and_then(|n| named(&facials, &format!("{n}.g1a"))) {
                        anims.push((bytes, t));
                    }
                }
                eprintln!("character {who}: expression {expr}{} {face:?}", if talking { ", talking" } else { "" });
            }
            set_face(scene, g1h, &anims)?;
            let mut layouts: Vec<(usize, usize)> = scene.morph.iter().map(|(&l, (t, _))| (l, t.len())).collect();
            layouts.sort();
            let mut offset = 0u32;
            let sets = layouts
                .iter()
                .map(|&(_, n)| {
                    let o = offset;
                    offset += n as u32;
                    (n, o)
                })
                .collect();
            drop(facials);
            Ok(Some(FaceRig { row: row_index as u32, head: head.to_string(), facials: fbytes, sets, loaded: Default::default() }))
        };
        let Some((body, face_rig)) = add_body(root, scene, tex_base, row, who, pos, angle, &column, motion_time, wear.as_ref(), face)? else { continue };
        if let Some((skeleton, motions, anim, start)) = body.rig {
            actors.push(Actor { who, instance: body.instance, skeleton, motions, column: column.clone(), anim, start, looping, previous: None, playing: (column.clone(), None, 0.0), last_locals: Vec::new(), last_t: start, face: face_rig, head: None, division: None, last_division_column: String::new() });
        }
        eprintln!("character {who} ({tag}) at {pos:?}, {angle} degrees");
        centre += Vec3::from(pos) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0);
        placed += 1.0;
        tex_base += body.count;
        packs.extend(body.packs);
    }
    Ok((packs, (placed > 0.0).then(|| centre / placed), camera, placements, at, start_field))
}

/// The fields an event plays in, in the order it first goes to each (`docs/events.md`, "Which
/// field"): each field map number and its directory.
fn event_fields(event_path: &Path) -> Result<Vec<(u32, PathBuf)>> {
    let root = event_path.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
    let event = barrel_formats::ebd::Ebd::parse(&std::fs::read(event_path)?)?;
    let steps = barrel_engine::event::Player::new(&event, &[], &barrel_formats::ebd::OperandTable::escha())?.schedule();
    let mut maps: Vec<u32> = Vec::new();
    for t in &steps {
        if let barrel_engine::event::Step::FieldChange { map, .. } = t.step
            && !maps.contains(&map) {
                maps.push(map);
            }
    }
    if maps.is_empty() {
        return Err("the event changes to no field; give --field DIR".into());
    }
    let names = std::fs::read_dir(root)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "exe"))
        .filter_map(|p| std::fs::read(p).ok())
        .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_escha::data::maps::field_maps(&pe)).filter(|m| !m.is_empty()))
        .unwrap_or_default();
    maps.into_iter()
        .map(|map| {
            let name = names.get(map as usize).ok_or("field map number past the executable's list")?;
            eprintln!("field {map}: {name}");
            Ok((map, root.join("Data/x64/Field").join(name)))
        })
        .collect()
}

/// Where in `anim` to sample at motion time `t`: looping motions wrap, others hold their end.
fn motion_clock(anim: &barrel_formats::g1a::Animation, t: f32, looping: bool) -> f32 {
    if looping && anim.duration > 0.0 { t.rem_euclid(anim.duration) } else { t.clamp(0.0, anim.duration) }
}

/// Blend facial animations (at `time`) into `scene.morph` for the next model added: each
/// animation goes to the morph set with one target fewer than it has tracks.
fn set_face(scene: &mut Scene, g1h: &[u8], anims: &[(&[u8], f32)]) -> Result<()> {
    let sets = barrel_formats::g1h::parse(g1h)?;
    // Every set goes to the GPU (so a face can change later), at rest unless an animation
    // weights it.
    for set in &sets {
        scene.morph.insert(set.layout as usize, (set.targets.clone(), vec![0.0; set.targets.len()]));
    }
    for (bytes, time) in anims {
        let anim = barrel_formats::g1a::parse(bytes)?;
        let Some(set) = sets.iter().find(|s| s.targets.len() + 1 == anim.tracks.len()) else { continue };
        if let Some((_, w)) = scene.morph.get_mut(&(set.layout as usize)) {
            *w = face_weights(&anim, set.targets.len(), *time);
        }
    }
    Ok(())
}

/// A facial animation's target weights at `time`: track k + 1 weights target k.
fn face_weights(anim: &barrel_formats::g1a::Animation, targets: usize, time: f32) -> Vec<f32> {
    let t = time.min(anim.duration);
    (0..targets).map(|k| anim.tracks.iter().find(|tr| tr.bone as usize == k + 1).and_then(|tr| tr.scalar(t)).unwrap_or(0.0)).collect()
}

/// The entry of `container` named `name`, if any.
/// A field's models and terrain into the scene (`barrel_engine::field::load`), from its folder's
/// two containers, with Escha's tags: objects that sway by node (named or tagged `xN<n>_`, the
/// `<n_shaker>` keywords) get their own instance so the tree's spring can turn them; only those
/// marked `xDS_` cast the sun's shadow (TOWN_11's workshop: its furniture and a second, `xDS_`
/// ceiling; the ceiling drawn does not cast); `xAM_..._ver<N>` shows in version N only.
fn field_scene(dir: &Path, scene: &mut Scene, base: usize) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    let area = dir.file_name().ok_or("field directory has no name")?.to_string_lossy().into_owned();
    let main_bytes = std::fs::read(dir.join(format!("{area}.elixir.gz")))?;
    let main = Elixir::parse(&main_bytes)?;
    let obj_bytes = std::fs::read(dir.join(format!("{area}_Obj.elixir.gz")))?;
    let objects = Elixir::parse(&obj_bytes)?;
    let node_sway = |s: &str| s.match_indices("xN").any(|(k, _)| s[k + 2..].chars().next().is_some_and(|c| c.is_ascii_digit()) && s[k + 3..].starts_with('_'));
    let place = |t: &barrel_formats::opl2::PlacedType, i: &barrel_formats::opl2::Instance| barrel_engine::field::Placement {
        // The maps' marks and dots take their own instances, to be tinted (`AreaMap::chosen`).
        own_instance: node_sway(&t.name) || node_sway(&i.tags) || (area == "A15_WMAP" && t.name.contains("point_")),
        casts: t.name.contains("xDS_") || i.tags.contains("xDS_"),
        version: object_version(&i.tags),
        ..Default::default()
    };
    barrel_engine::field::load(&area, &main, &objects, scene, base, &place)
}

/// The version a placement's tags give it: `<N>` from `xAM_balloon_ver<N>` (TOWN_07's dock
/// parts: `xDS_xAM_balloon_ver2`, `_balloon_ver3_xAM_`).
fn object_version(tags: &str) -> Option<u32> {
    if !tags.contains("xAM_") {
        return None;
    }
    let at = tags.find("_ver")? + 4;
    let digits: String = tags[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// What a scene is loaded from: the viewer's positional arguments (a model and its textures,
/// or `--field DIR|auto`) and the options that add to it.
struct SceneSpec {
    field: bool,
    args: Vec<String>,
    event_spec: Option<String>,
    skeleton_spec: Option<String>,
    anim_spec: Option<String>,
    time: f32,
    morph_spec: Option<String>,
    face_specs: Vec<String>,
    costumes: Vec<(u32, usize, usize, usize)>,
    costumes_from: Option<PathBuf>,
    event_line: Option<usize>,
    event_at: Option<f32>,
    look_at: Option<u32>,
    creatures_area: Option<i32>,
    town_people_from: Option<String>,
    /// The events seen, for placing town people in the story's free play.
    town_seen: Option<Vec<bool>>,
    /// `--player X,Z`: the player standing there.
    player_at: Option<[f32; 2]>,
    /// `--walk DX,DZ`: and walking that way (units a second).
    player_walk: Option<[f32; 2]>,
    /// Which way the player faces at the start (radians; 0 towards +z): a field's arrival.
    player_facing: Option<f32>,
    /// `--world-trip`: the waypoints she runs through.
    trip: Vec<[f32; 3]>,
    /// Logy leads (`--hero logy`): the player walking is Logy.
    logy: bool,
    /// `--battle GROUP`: a battle laid out on the stage (`battle_people`).
    battle_group: Option<i32>,
    /// The saved session a field encounter's battle came from (its `member` lines give the party).
    battle_party: Option<String>,
    /// `--look-from FIELD`: the look table (`Saves/field/FIELD.xml`) for a field without its
    /// own, as a battle stage (`BF_POINT*`) takes the field area's.
    look_from: Option<String>,
}

/// A scene loaded on the CPU side: its geometry, draws and instances, the texture packs, the
/// fields it spans, and what plays on it (the event's characters, field enemies, town people).
struct Loaded {
    scene: Scene,
    pack_bytes: Vec<Vec<u8>>,
    /// The field map numbers loaded, in scene order (empty for `--field DIR`).
    field_maps: Vec<u32>,
    field_looks: Vec<Option<barrel_formats::field_look::FieldLook>>,
    /// Each field's distant view's instance (it follows the camera), if it has one.
    sky_instances: Vec<Option<u32>>,
    focus: Option<Vec3>,
    event_camera: Option<EventCamera>,
    actors: Vec<Actor>,
    /// Characters loaded but not on the stage (yet, or any more): not drawn.
    absent: std::collections::HashSet<u32>,
    creatures: Option<(barrel_escha::field_ai::Field, Vec<FieldBody>, Vec<String>, f32, [f64; 3], bool)>,
    /// `--battle`'s battle, played in the scene.
    battle_play: Option<BattlePlay>,
    /// The battle tables and generator for the field's encounters.
    battles: Option<(barrel_escha::auto_battle::Tables, barrel_engine::rng::Rng)>,
    town_bodies: Vec<TownBody>,
    /// Where playback starts (`--at`, or the start of `--line`'s line).
    start_at: Option<f32>,
    /// The field map the event is in there (`--at`), if it has moved.
    start_field: Option<u32>,
    /// Each field's light texture pack (`<AREA>_pat.g1t`), in `field_maps` order.
    patterns: Vec<Option<Vec<u8>>>,
    /// `--player` with `--walk`: the player walking.
    walker: Option<Walker>,
}

/// The story from a new game, over the install at ROOT (`event_start::Chain`).
fn story_chain(root: &Path) -> Result<barrel_escha::event_start::Chain> {
    let checks = barrel_escha::data::event_check::parse(&std::fs::read(root.join("Event/eventCheck/EventCheckDataAll.ecd"))?).map_err(|e| e.to_string())?;
    let process = barrel_escha::data::event_process::parse(&std::fs::read(root.join("Event/eventprocess/EventProcessDataAll.epd"))?).map_err(|e| e.to_string())?;
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
    let (names, _) = pe.enum_names("EVENT_MM01_OP1_111").ok_or("no EVENT_* enum")?;
    Ok(barrel_escha::event_start::Chain::new(checks, process, names.iter().map(|n| n.to_string()).collect()))
}

/// Loads a scene (`docs/renderer.md`, "Chained events": the renderer outlives its scenes).
fn load_scene(spec: SceneSpec) -> Result<Loaded> {
    let SceneSpec {
        field,
        look_from,
        battle_group,
        battle_party,
        args,
        event_spec,
        skeleton_spec,
        anim_spec,
        time,
        morph_spec,
        face_specs,
        mut costumes,
        costumes_from,
        event_line,
        event_at,
        look_at,
        creatures_area,
        town_people_from,
        town_seen,
        player_at,
        player_walk,
        player_facing,
        trip,
        logy,
    } = spec;
    let mut town_bodies = Vec::new();
    let mut battle_play: Option<BattlePlay> = None;
    // Where playback starts: --at, or --line's start.
    let mut start_at = event_at;
    let mut walker: Option<Walker> = None;
    // The field the event is in where playback starts.
    let mut start_field = None;
    // CPU side: the vertices and draws, and the texture pack.
    let mut scene = Scene::new();
    // The field map numbers loaded, in scene order (empty for `--field DIR`).
    let mut field_maps: Vec<u32> = Vec::new();
    let mut field_looks: Vec<Option<barrel_formats::field_look::FieldLook>> = Vec::new();
    let mut patterns: Vec<Option<Vec<u8>>> = Vec::new();
    // Each field's distant view's instance (it follows the camera), if it has one.
    let mut sky_instances: Vec<Option<u32>> = Vec::new();
    let mut pack_bytes = Vec::new();
    if field {
        // `--field auto` with an event: every field the event goes to.
        let dirs = if args[1] == "auto" {
            let e = event_spec.as_ref().ok_or("--field auto needs --event")?;
            event_fields(Path::new(e))?
        } else {
            vec![(u32::MAX, PathBuf::from(&args[1]))]
        };
        let mut base = 0;
        for (k, (map, dir)) in dirs.iter().enumerate() {
            // Its look, from the table of the same name (Saves/field/<FIELD>.xml), else
            // `--look-from`'s.
            let root = dir.ancestors().find(|p| p.join("Saves").is_dir());
            let look = |name: &std::ffi::OsStr| root.and_then(|r| std::fs::read(r.join("Saves/field").join(name).with_extension("xml")).ok());
            let table = dir
                .file_name()
                .and_then(look)
                .or_else(|| look_from.as_deref().and_then(|n| look(std::ffi::OsStr::new(n))))
                .and_then(|b| barrel_formats::table::parse(&b).ok());
            field_looks.push(table.map(|t| barrel_formats::field_look::read(&t)));
            scene.field = Some(k as u16);
            let (bytes, pattern) = field_scene(dir, &mut scene, base)?;
            patterns.push(pattern);
            base += G1t::parse(&bytes)?.textures.len();
            pack_bytes.push(bytes);
            field_maps.push(*map);
            // Its distant view (C_SPHERE/<set>, by the executable's table), all in one instance.
            let root = dir.ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf);
            let set = root.as_ref().and_then(|r| {
                let exe = std::fs::read(r.join("Atelier_Escha_and_Logy_EN.exe")).ok()?;
                let pe = barrel_formats::pe::Pe::parse(&exe).ok()?;
                let name = dir.file_name()?.to_str()?;
                barrel_formats::field_look::sky_sets(&pe, barrel_formats::field_look::SKY_SETS_EN).into_iter().find(|(f, _)| f == name).map(|(_, s)| s)
            });
            let mut sky = None;
            if let (Some(root), Some(set)) = (&root, set.filter(|s| !s.starts_with("set-black"))) {
                let bytes = std::fs::read(root.join("Data/x64/Field/C_SPHERE").join(&set))?;
                let container = Elixir::parse(&bytes)?;
                let instance = scene.instance(Mat4::IDENTITY);
                // Drawn without depth, so back to front by kind: the dome, then the clouds
                // (`xEF_`), then the ground, stones, trees and cliffs in front of them (in file
                // order the clouds came after the trees and veiled them: TOWN_07, event 212).
                let rank = |name: &str| if name.contains("Sphere_sky") { 0 } else if name.contains("xEF_") { 1 } else { 2 };
                let mut entries: Vec<_> = container.entries.iter().collect();
                entries.sort_by_key(|e| rank(&e.name));
                for e in entries {
                    let Some(stem) = e.name.strip_suffix(".g1m") else { continue };
                    let (Some(mesh), Some(tex)) = (named(&container, &format!("{stem}.g1m")), named(&container, &format!("{stem}.g1t"))) else { continue };
                    let count = G1t::parse(tex)?.textures.len();
                    let model = G1m::parse(mesh)?;
                    let Some(geometry) = model.geometry.as_ref() else { continue };
                    let first = scene.draws.len();
                    scene.add_posed(geometry, Mat4::IDENTITY, count, None, instance)?;
                    for d in &mut scene.draws[first..] {
                        d.texture += base;
                        d.overlay = d.overlay.map(|o| o + base);
                        d.mask = d.mask.map(|m| m + base);
                        d.sky = true;
                        d.name = format!("sky:{stem}");
                    }
                    base += count;
                    pack_bytes.push(tex.to_vec());
                }
                eprintln!("{}: distant view {set}", dir.display());
                sky = Some(instance);
            }
            sky_instances.push(sky);
        }
        scene.field = None;
    } else {
        let model_bytes = entry_bytes(&args[0])?;
        let model = G1m::parse(&model_bytes)?;
        let geometry = model.geometry.as_ref().ok_or("model has no geometry")?;
        let tex_bytes = entry_bytes(&args[1])?;
        let count = G1t::parse(&tex_bytes)?.textures.len();
        let joints = match (&skeleton_spec, &anim_spec) {
            (Some(sk), Some(an)) => {
                let sk_bytes = entry_bytes(sk)?;
                let skel = G1m::parse(&sk_bytes)?.skeleton.ok_or("no skeleton in that model")?;
                let anim = barrel_formats::g1a::parse(&entry_bytes(an)?)?;
                eprintln!("posing {} joints with {} tracks at {time}s of {}s", skel.joints.len(), anim.tracks.len(), anim.duration);
                Some(pose(&skel, &anim, time))
            }
            (None, None) => None,
            _ => return Err("--skeleton and --anim go together".into()),
        };
        if let Some(m) = &morph_spec {
            let anims: Vec<Vec<u8>> = face_specs.iter().map(|f| entry_bytes(f)).collect::<Result<_>>()?;
            let with_time: Vec<(&[u8], f32)> = anims.iter().map(|a| (&a[..], time)).collect();
            set_face(&mut scene, &entry_bytes(m)?, &with_time)?;
        } else if !face_specs.is_empty() {
            return Err("--face needs --morph".into());
        }
        let instance = if joints.is_some() || !scene.morph.is_empty() { scene.instance(Mat4::IDENTITY) } else { 0 };
        scene.add_posed(geometry, Mat4::IDENTITY, count, joints.as_deref(), instance)?;
        scene.morph.clear();
        pack_bytes.push(tex_bytes);
    }
    if let (Some(sys), Some(e)) = (&costumes_from, &event_spec) {
        let root = Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--costumes: the event is not inside a game install")?;
        costumes.extend(saved_costumes(root, sys)?);
    }
    let mut focus = None;
    let mut event_camera: Option<EventCamera> = None;
    let mut actors: Vec<Actor> = Vec::new();
    // Characters loaded but not on the stage (yet, or any more): not drawn.
    let mut absent: std::collections::HashSet<u32> = Default::default();
    if let Some(e) = &event_spec {
        let base = pack_bytes.iter().map(|b| G1t::parse(b).map(|t| t.textures.len())).sum::<std::result::Result<usize, _>>()?;
        let (packs, centre, cam, placements, at, field_at) = event_characters(Path::new(e), &mut scene, base, event_line, event_at, &mut actors, &mut absent, &costumes)?;
        start_at = at;
        start_field = field_at;
        pack_bytes.extend(packs);
        focus = centre;
        event_camera = cam;
        // --look-at N: a close-up of character N's head, from in front of them.
        if let Some(who) = look_at {
            let &(_, pos, angle) = placements.iter().find(|p| p.0 == who).ok_or("--look-at: that character is not placed")?;
            event_camera = Some(EventCamera {
                eye: None,
                target: Vec3::from(pos) + Vec3::new(0.0, 140.0, 0.0),
                distance: 160.0,
                yaw: 180.0 - angle,
                pitch: 5.0,
                fov: 30.0,
            });
        }
    }
    let mut creatures = None;
    // The battle tables and generator, with the field's enemies.
    let mut battles: Option<(barrel_escha::auto_battle::Tables, barrel_engine::rng::Rng)> = None;
    if let Some(area) = creatures_area {
        let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--creatures: the field is not inside a game install")?.to_path_buf();
        let base = pack_bytes.iter().map(|b| G1t::parse(b).map(|t| t.textures.len())).sum::<std::result::Result<usize, _>>()?;
        let (field, bodies, packs, names, player_at) = field_creatures(&root, area, &mut scene, base)?;
        pack_bytes.extend(packs);
        if focus.is_none() {
            focus = field.mobs.first().map(|m| Vec3::from(m.creature.position.map(|v| v as f32)) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0));
        }
        creatures = Some((field, bodies, names, 0.0f32, player_at, false));
        battles = match battle_tables(&root) {
            Ok(t) => Some((t, barrel_engine::rng::Rng::seeded(1))),
            Err(e) => {
                eprintln!("no battles: {e}");
                None
            }
        };
    }
    if town_people_from.is_some() || town_seen.is_some() {
        let field_dir = Path::new(&args[1]);
        let root = field_dir.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--town-people: the field is not inside a game install")?.to_path_buf();
        let base = pack_bytes.iter().map(|b| G1t::parse(b).map(|t| t.textures.len())).sum::<std::result::Result<usize, _>>()?;
        // The events seen: the story's (free play), a save's (`--town-people GAMEDATA`), or
        // none (`all`: everyone placed whatever their conditions).
        let seen = match (&town_seen, town_people_from.as_deref()) {
            (Some(s), _) => Some(s.clone()),
            (None, Some(p)) if p != "all" => {
                let data = std::fs::read(p)?;
                let body = barrel_escha::data::save::parse(&data).map_err(|e| format!("{p}: {e}"))?.body;
                let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{p}: {e}"))?;
                let sum = barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| e.to_string())?);
                Some(sum.events.iter().map(|e| e.seen).collect())
            }
            _ => None,
        };
        let (packs, first, bodies) = town_people(&root, field_dir, seen, &mut scene, base)?;
        pack_bytes.extend(packs);
        // After the player's body, if there is one (the walker keeps its index).
        town_bodies.extend(bodies);
        if focus.is_none() {
            focus = first.map(|p| Vec3::from(p) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0));
        }
    }
    if let Some(group) = battle_group {
        let field_dir = Path::new(&args[1]);
        let root = field_dir.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--battle: the stage is not inside a game install")?.to_path_buf();
        let base = pack_bytes.iter().map(|b| G1t::parse(b).map(|t| t.textures.len())).sum::<std::result::Result<usize, _>>()?;
        let members = battle_party.as_deref().map(barrel_escha::session::Session::party_in).unwrap_or_default();
        // The vocal song's flag (`EVENT_FLAG_SOUND_QUEST_FLAG`) in the field's save.
        let sound_quest = battle_party.as_deref().is_some_and(|t| t.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == ["flag", "149", "1"]));
        let (packs, bodies, play) = battle_people(&root, group, logy, sound_quest, &members, &mut scene, base, town_bodies.len())?;
        pack_bytes.extend(packs);
        town_bodies.extend(bodies);
        battle_play = Some(play);
        focus = focus.or(Some(Vec3::from(BATTLE_CENTRE) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0)));
        // A look over the whole battle from behind the lead (not the game's battle camera,
        // which is not read yet).
        let lead = party_spot_of(0);
        event_camera = event_camera.or(Some(EventCamera {
            eye: None,
            target: Vec3::from(BATTLE_CENTRE) + Vec3::new(0.0, 80.0, 0.0),
            distance: 1700.0,
            yaw: lead[0].atan2(lead[2]).to_degrees(),
            pitch: 22.0,
            fov: 45.0,
        }));
    }
    if let Some(at) = player_at {
        let field_dir = Path::new(&args[1]);
        let root = field_dir.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--player: the field is not inside a game install")?.to_path_buf();
        let base = pack_bytes.iter().map(|b| G1t::parse(b).map(|t| t.textures.len())).sum::<std::result::Result<usize, _>>()?;
        let (packs, pos, body, mut w) = player(&root, field_dir, at, logy, &mut scene, base)?;
        let packs_len = packs.len();
        pack_bytes.extend(packs);
        w.body = town_bodies.len();
        town_bodies.push(body);
        w.velocity = player_walk.unwrap_or_default();
        w.facing = player_facing.unwrap_or(w.facing);
        w.waypoints = trip.iter().copied().skip(1).collect();
        // The world map's cursor (CHARA 705, `E3D_WMAP_CUR00`; the map's setup creates it,
        // `0x140383c42`).
        if field_dir.file_name().is_some_and(|n| n == "A15_WMAP") {
            let table = |name: &str| std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
            let row = table("CharaBase.xml").and_then(|t| t.children.into_iter().find(|r| r.attr("Model").is_some_and(|m| m.ends_with("E3D_WMAP_CUR00"))));
            let base = base + pack_bytes.iter().skip(pack_bytes.len() - packs_len).map(|b| G1t::parse(b).map_or(0, |t| t.textures.len())).sum::<usize>();
            match row.map(|row| add_body(&root, &mut scene, base, &row, 21_001, pos, 0.0, "BASE_MOTION1", None, None, |_, _, _, _| Ok(None::<()>))) {
                Some(Ok(Some((body, _)))) => {
                    pack_bytes.extend(body.packs);
                    w.cursor = Some((body.instance, pos, w.facing));
                    // Its motion loops on the town clock (3.37 s).
                    if let Some((skeleton, _, anim, _)) = body.rig {
                        town_bodies.push(TownBody { instance: body.instance, skeleton, anim, clock: None, from: None, held: None, last_locals: Vec::new(), hidden: false });
                    }
                }
                other => eprintln!("the world map's cursor did not load: {:?}", other.map(|r| r.map(|b| b.is_some()))),
            }
        }
        walker = Some(w);
        focus = Some(Vec3::from(pos) * Vec3::new(-1.0, 1.0, 1.0) + Vec3::new(0.0, 80.0, 0.0));
    }
    Ok(Loaded { scene, pack_bytes, field_maps, field_looks, sky_instances, focus, event_camera, actors, absent, creatures, battles, battle_play, town_bodies, start_at, start_field, patterns, walker })
}

#[allow(deprecated)] // glam 0.33 renamed the camera helpers; the old ones are kept
fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut pump = None;
    // A run ends, or starts again from the title (the menu's Return to Title and Load), with the
    // window kept: everything else is built anew.
    let mut carry = Carry::default();
    loop {
        match view(args, pump, carry)? {
            Ended::Done => return Ok(()),
            Ended::Restart(a, p, c) => {
                eprintln!("restarting: {}", a.join(" "));
                (args, pump, carry) = (a, p, c);
            }
        }
    }
}

/// How a run of [`view`] ended.
enum Ended {
    Done,
    /// Start again with these arguments, the window and what the next run carries over.
    Restart(Vec<String>, Option<window::Pump>, Carry),
}

/// What a run hands the next one: the story's save text (loaded as `--load` would), the field
/// enemies beaten in the field it returns to, and, in a battle, the way back to the field when
/// the battle ends (its arguments and carry).
#[derive(Default)]
struct Carry {
    save: Option<String>,
    defeated: Vec<usize>,
    /// A line to show in the message window on arrival (speaker, text): a lost battle's.
    notice: Option<(String, String)>,
    /// Arrive on the area map at the save's field's point, not in the field (a lost battle).
    to_area_map: bool,
    back: Option<Box<(Vec<String>, Carry)>>,
}

/// The command line's render setting options (`--preset`, `--msaa N` ...), with whether each
/// takes a value.
const SETTINGS_FLAGS: [(&str, bool); 14] = [
    ("--preset", true), ("--handheld", false), ("--fsr", false), ("--gsr", false), ("--motion", false), ("--jitter", false), ("--taa", false),
    ("--gsr2", false), ("--fsr3", false), ("--sharpen", false), ("--msaa", true), ("--scale", true), ("--shadows", true), ("--anisotropy", true),
];

fn has_settings_flags(args: &[String]) -> bool {
    args.iter().any(|a| SETTINGS_FLAGS.iter().any(|(f, _)| a == f))
}

/// `args` less the render setting options, so that a run takes Barrel's config.
fn without_settings_flags(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match SETTINGS_FLAGS.iter().find(|(f, _)| a == f) {
            Some((_, true)) => {
                it.next();
            }
            Some(_) => {}
            None => out.push(a.clone()),
        }
    }
    out
}

/// The window as it is now: its size, or fullscreen.
/// The settings the Options page's "default" puts back: the launcher's default preset and
/// fullscreen (`BARREL_DEFAULT_PRESET`, `BARREL_DEFAULT_FULLSCREEN`: a handheld's), else the
/// desktop preset in a window.
fn default_graphics() -> options::Graphics {
    let render = std::env::var("BARREL_DEFAULT_PRESET").ok().and_then(|n| settings::RenderSettings::preset(&n)).unwrap_or(settings::RenderSettings::DESKTOP);
    let fullscreen = std::env::var_os("BARREL_DEFAULT_FULLSCREEN").is_some_and(|v| v == "1");
    options::Graphics { render, display: settings::Display { fullscreen, ..settings::Display::DEFAULT } }
}

fn current_display(p: &window::Pump) -> settings::Display {
    let w = p.window();
    let size = w.inner_size();
    settings::Display { width: size.width, height: size.height, fullscreen: w.fullscreen().is_some() }
}

/// Writes Barrel's graphics config.
fn write_config(s: &options::Graphics) -> Result<std::path::PathBuf> {
    let path = settings::RenderSettings::config_path().ok_or("no place for the config (no HOME)")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, s.to_config())?;
    Ok(path)
}

/// `args` less what picks a scene (as [`title_args`], and the field options `--battle`,
/// `--look-from`, `--player`, `--walk`, `--creatures`, `--hero`), for a run on another one.
fn plain_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--load" | "--event" | "--at" | "--title" | "--battle" | "--look-from" | "--player" | "--walk" | "--creatures" | "--hero" => {
                it.next();
            }
            "--title-load" | "--chain" | "--free-play" => {}
            _ => out.push(a.clone()),
        }
    }
    out
}

/// `args` with `--field auto` (a run started at the title) made a field of the install at
/// `root`, as a carried save needs to find the install (the save says which field it loads).
fn with_install_field(mut args: Vec<String>, root: &Path) -> Vec<String> {
    if args.len() > 1 && args[0] == "--field" && args[1] == "auto" {
        args[1] = root.join("Data/x64/Field/TOWN_04").to_string_lossy().into_owned();
    }
    args
}

/// The arguments to start again at the title (`load_list`: on its Load Game list): `args` less
/// what picks a start (`--load`, `--event`, `--at`, `--title`, `--title-load`, `--chain`,
/// `--free-play`), with `--field auto` and `--title ROOT`.
/// Whether this process has shown the logo splashes (they show once a launch).
static SPLASHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn title_args(args: &[String], root: &Path, load_list: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--load" | "--event" | "--at" | "--title" => {
                it.next();
            }
            "--title-load" | "--chain" | "--free-play" => {}
            _ => out.push(a.clone()),
        }
    }
    if out.len() > 1 && out[0] == "--field" {
        out[1] = "auto".into();
    }
    out.extend(["--title".to_string(), root.to_string_lossy().into_owned()]);
    if load_list {
        out.push("--title-load".into());
    }
    out
}

/// Where the mixed sound goes: the device (a window), or a WAV file (`--audio`, offscreen: mixed
/// exactly up to the event's clock, so it lines up with the frames written).
enum Sink {
    Device(barrel_audio::Output),
    File { capture: barrel_audio::Capture, start: f64, path: PathBuf },
}

impl Sink {
    /// Keeps the device fed, or the file mixed up to `now` (the event's clock, seconds).
    fn pump(&mut self, mixer: &mut barrel_audio::Mixer, now: f64) {
        match self {
            Sink::Device(out) => {
                out.pump(mixer);
            }
            Sink::File { capture, start, .. } => capture.until(mixer, now - *start),
        }
    }
}

impl Drop for Sink {
    /// The file is written when the run lets go of it, however it ends.
    fn drop(&mut self) {
        if let Sink::File { capture, path, .. } = self {
            match std::fs::write(&*path, capture.wav()) {
                Ok(()) => eprintln!("sound: {} ({:.2} s)", path.display(), capture.frames() as f64 / f64::from(barrel_audio::RATE)),
                Err(e) => eprintln!("sound: {}: {e}", path.display()),
            }
        }
    }
}

fn view(mut args: Vec<String>, kept_pump: Option<window::Pump>, mut carry: Carry) -> Result<Ended> {
    let original_args = args.clone();
    let mut restart: Option<Vec<String>> = None;
    let mut carry_out = Carry::default();
    let field = args.first().is_some_and(|a| a == "--field");
    let positional = if field { 3 } else { 3 };
    if args.len() < positional {
        return Err("usage: barrel-escha MODEL.elixir.gz#N TEXTURES.elixir.gz#N OUT.png | --field DIR OUT.png  [--size N] [--yaw DEG] [--pitch DEG]".into());
    }
    let out_path = if field { args[2].clone() } else { args[2].clone() };
    let mut size = 768u32;
    let mut size_h = 768u32;
    let mut yaw = 0f32;
    let mut pitch = if field { 45f32 } else { 0f32 };
    let mut alpha_cutoff = 0.3f32;
    // The render settings (`settings.rs`): `--preset desktop|desktop-high|handheld` (`--handheld`
    // for the last), then single fields over it, in the order given: `--scale S` (the scene at
    // S times the output's size, 0.25 to 2), `--fsr` (FSR 1 instead of the bilinear scale),
    // `--msaa N` (samples a pixel, resolved in the scene pass), `--shadows desktop|handheld`.
    let mut settings = settings::RenderSettings::DESKTOP;
    // `--show-motion`: the motion vectors shown in place of the frame (red across, green down).
    let mut show_motion = false;
    // Print the GPU's time per stage at the end (--timings).
    let mut timings = false;
    let mut hero = 0u32;
    let mut free_play = false;
    // `--title ROOT`: the title screen first (`title`), its New Game going on as `--new-game`.
    let mut title_root: Option<PathBuf> = None;
    // `--title-load`: the title opens on its Load Game list (the menu's Option → Load).
    let mut title_load = false;
    // --save FILE: where F5 (or `BARREL_SAVE_AT=FRAME`) saves in free play; --load FILE: start
    // in free play from such a save.
    let mut save_file = None::<PathBuf>;
    let mut new_game = None::<PathBuf>;
    let mut travel_to = None::<String>;
    let mut area_mode = false;
    // The area map's top level, the world map (the town and the regions).
    let mut world_mode = false;
    let mut load_file = None::<PathBuf>;
    // `--saves DIR`: Barrel's save slots (`barrel_escha::saves`; by default its own data folder).
    let mut saves_dir = None::<PathBuf>;
    let (mut skeleton_spec, mut anim_spec, mut time) = (None::<String>, None::<String>, 0f32);
    let mut event_spec = None::<String>;
    let mut creatures_area = None::<i32>;
    let mut town_seen: Option<Vec<bool>>;
    let mut town_people_from = None::<String>;
    let mut player_at = None::<[f32; 2]>;
    let mut player_walk = None::<[f32; 2]>;
    // A trip's arrival facing, for the next scene only.
    let mut player_facing = None::<f32>;
    let mut trip_path: Vec<[f32; 3]> = Vec::new();
    let mut town_time = 0.0f32;
    // --costume WHO:BODY:HEAD[:FACE] (repeatable): an event character in outfit BODY with head
    // HEAD and face FACE (rows of CostumeBody.xml, CostumeHead.xml and CostumeFace.xml).
    let mut costumes: Vec<(u32, usize, usize, usize)> = Vec::new();
    let mut costumes_from = None::<PathBuf>;
    // --save-every N: of several frames, write only every Nth and the last.
    let mut save_every = 1u32;
    let mut event_line = None::<usize>;
    let mut event_at = None::<f32>;
    // `--audio OUT.wav`: an offscreen event's sound to a file.
    let mut audio_out = None::<PathBuf>;
    let (mut frames, mut fps) = (1u32, 60f64);
    // --window FRAMES: show that many frames in a window, at the display's rate, instead of
    // writing them to files.
    let mut window_frames = None::<u32>;
    let mut look_at = None::<u32>;
    let (mut morph_spec, mut face_specs) = (None::<String>, Vec::<String>::new());
    let mut use_event_camera = true;
    let mut no_terrain = false;
    let mut look_from: Option<String> = None;
    let mut battle_group: Option<i32> = None;
    let mut chain_on = false;
    let mut it = args[positional..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // N (square) or WxH, up to 8192 either way (4K, ultrawide 3440×1440 and 5120×1440, 8K wide).
            "--size" => {
                let v = it.next().ok_or("--size N|WxH")?;
                let (w, h) = v.split_once('x').unwrap_or((v, v));
                size = w.parse::<u32>()?.clamp(64, 8192);
                size_h = h.parse::<u32>()?.clamp(64, 8192);
            }
            "--yaw" => yaw = it.next().ok_or("--yaw DEG")?.parse()?,
            "--no-alpha" => alpha_cutoff = -1.0,
            "--preset" => {
                let name = it.next().ok_or("--preset desktop|desktop-high|handheld")?;
                settings = settings::RenderSettings::preset(name).ok_or_else(|| format!("{name}: no such preset"))?;
            }
            "--handheld" => settings = settings::RenderSettings::HANDHELD,
            "--fsr" => settings.upscaler = settings::Upscaler::Fsr1,
            "--gsr" => settings.upscaler = settings::Upscaler::Gsr1,
            "--motion" => settings.motion = true,
            "--jitter" => settings.jitter = true,
            "--taa" => settings.taa = true,
            "--gsr2" => settings.gsr2 = true,
            "--fsr3" => settings.fsr3 = true,
            "--timings" => timings = true,
            // The protagonist the player chose, whom "Master Character" (row 749) stands for.
            "--hero" => {
                hero = match it.next().ok_or("--hero escha|logy")?.as_str() {
                    "escha" => 0,
                    "logy" => 1,
                    other => return Err(format!("--hero {other}: escha or logy").into()),
                }
            }
            "--sharpen" => settings.sharpen = true,
            "--show-motion" => {
                settings.motion = true;
                show_motion = true;
            }
            "--msaa" => settings.msaa = it.next().ok_or("--msaa N")?.parse()?,
            "--scale" => settings.scale = it.next().ok_or("--scale S")?.parse::<f32>()?.clamp(0.25, 2.0),
            "--shadows" => {
                settings.shadows = match it.next().map(String::as_str) {
                    Some("desktop") => shadow::Shadows::DESKTOP,
                    Some("handheld") => shadow::Shadows::HANDHELD,
                    _ => return Err("--shadows desktop|handheld".into()),
                }
            }
            "--skeleton" => skeleton_spec = Some(it.next().ok_or("--skeleton FILE#N")?.clone()),
            "--anim" => anim_spec = Some(it.next().ok_or("--anim FILE#N")?.clone()),
            "--time" => time = it.next().ok_or("--time SECONDS")?.parse()?,
            "--morph" => morph_spec = Some(it.next().ok_or("--morph FILE#N")?.clone()),
            "--face" => face_specs.push(it.next().ok_or("--face FILE#N")?.clone()),
            "--look-at" => look_at = Some(it.next().ok_or("--look-at CHARA")?.parse()?),
            "--line" => event_line = Some(it.next().ok_or("--line N")?.parse()?),
            "--at" => event_at = Some(it.next().ok_or("--at SECONDS")?.parse()?),
            "--audio" => audio_out = Some(PathBuf::from(it.next().ok_or("--audio OUT.wav")?)),
            "--frames" => frames = it.next().ok_or("--frames N")?.parse::<u32>()?.max(1),
            // --window 0: until the player quits (a game launch, Kintsugi's `run`).
            "--window" => window_frames = Some(match it.next().ok_or("--window FRAMES")?.parse::<u32>()? {
                0 => u32::MAX,
                n => n.min(100_000),
            }),
            "--fps" => fps = it.next().ok_or("--fps HZ")?.parse::<f64>()?.clamp(1.0, 1000.0),
            "--event" => event_spec = Some(it.next().ok_or("--event FILE.ebd")?.clone()),
            // --creatures AREA: field area AREA's enemies (12 is AREA_01_01) under their scripts.
            "--save-every" => save_every = it.next().ok_or("--save-every N")?.parse::<u32>()?.max(1),
            // --costumes SYSDATA: every costumed character dressed as the settings save has them.
            "--costumes" => costumes_from = Some(PathBuf::from(it.next().ok_or("--costumes SYSDATA")?)),
            "--costume" => {
                let v = it.next().ok_or("--costume WHO:BODY:HEAD[:FACE]")?;
                let p: Vec<usize> = v.split(':').map(str::parse).collect::<std::result::Result<_, _>>()?;
                match p[..] {
                    [w, b, h] => costumes.push((w as u32, b, h, 0)),
                    [w, b, h, f] => costumes.push((w as u32, b, h, f)),
                    _ => return Err("--costume WHO:BODY:HEAD[:FACE]".into()),
                }
            }
            "--creatures" => creatures_area = Some(it.next().ok_or("--creatures AREA")?.parse()?),
            // --town-people all|GAMEDATA: the town people the field load would place here
            // (all: each character's first placement; a save: those its seen events allow).
            "--town-people" => town_people_from = Some(it.next().ok_or("--town-people all|GAMEDATA")?.clone()),
            // --player X,Z: the player (Escha) standing there, on the field's ground.
            "--player" => {
                let v = it.next().ok_or("--player X,Z")?;
                let (x, z) = v.split_once(',').ok_or("--player X,Z")?;
                player_at = Some([x.parse()?, z.parse()?]);
            }
            // --world-point NAME: the player at a world map point (`LP_T0`, `SP_A0P0` ...: the
            // field must be A15_WMAP), where its tagged placement marks it.
            "--world-point" => {
                let name = it.next().ok_or("--world-point NAME")?;
                let field = Path::new(args.get(1).ok_or("--world-point needs --field .../A15_WMAP")?);
                let at = world_point(field, name)?;
                eprintln!("world map point {name} at {at:?}");
                player_at = Some([at[0], at[2]]);
            }
            // --world-trip FROM TO: on the world map (the field A15_WMAP), the player runs a
            // trip's way from mark FROM to mark TO (`T0`, `A0p0` ...: game::travel).
            "--world-trip" => {
                let (from, to) = (it.next().ok_or("--world-trip FROM TO")?, it.next().ok_or("--world-trip FROM TO")?);
                let field = Path::new(args.get(1).ok_or("--world-trip needs --field .../A15_WMAP")?);
                let path = world_trip(field, from, to)?;
                eprintln!("world map trip {from} to {to}: {} waypoints", path.len());
                player_at = path.first().map(|p| [p[0], p[2]]);
                player_walk = Some([0.0, 0.0]);
                trip_path = path;
            }
            // --walk DX,DZ: the player walks that way, DX and DZ units a second.
            "--walk" => {
                let v = it.next().ok_or("--walk DX,DZ")?;
                let (x, z) = v.split_once(',').ok_or("--walk DX,DZ")?;
                player_walk = Some([x.parse()?, z.parse()?]);
            }
            "--free-camera" => use_event_camera = false,
            "--no-terrain" => no_terrain = true,
            // --look-from FIELD: a field without a look table of its own takes FIELD's.
            // --battle GROUP: on a battle stage, encounter group GROUP against the party.
            "--battle" => battle_group = Some(it.next().ok_or("--battle GROUP")?.parse()?),
            "--look-from" => look_from = Some(it.next().ok_or("--look-from FIELD")?.clone()),
            // --chain: when the event ends, the one the story starts after it, and so on.
            "--chain" => chain_on = true,
            // --free-play (with --chain): when no event starts any more, the hero walks freely
            // in the field the last event left them in, from where they stood at its end.
            "--free-play" => free_play = true,
            // --travel-to MARK: where a world map exit takes the player (there is no world
            // map screen yet): `A0`, `A0p0` ... (`barrel world-routes`).
            "--travel-to" => travel_to = Some(it.next().ok_or("--travel-to MARK")?.clone()),
            // --area-map: on the world map (A15_WMAP, from a --world-point), the area map: W A S D
            // (offscreen `BARREL_AREA_KEYS`: up, down, left, right, confirm) go along the routes
            // of the point's region, Enter enters the field of the point she stands on.
            "--area-map" => area_mode = true,
            // --new-game ROOT: a new game over the install at ROOT: the opening the hero's
            // choice starts (`--hero`), the story after it, then free play.
            "--pad-test" => {
                // Prints the pad's name and its presses and sticks for ten seconds, no window.
                let Some(mut g) = pad::Gamepad::open() else { return Err("no gamepad layer".into()) };
                let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let mut last = ([0.0f32; 2], [0.0f32; 2]);
                while std::time::Instant::now() < end {
                    for b in g.poll() {
                        eprintln!("pressed {b:?}");
                    }
                    let now = (g.left.map(|v| (v * 4.0).round() / 4.0), g.right.map(|v| (v * 4.0).round() / 4.0));
                    if now != last {
                        eprintln!("sticks left {:?} right {:?}", now.0, now.1);
                        last = now;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(16));
                }
                return Ok(Ended::Done);
            }
            "--new-game" => new_game = Some(PathBuf::from(it.next().ok_or("--new-game ROOT")?)),
            "--title-load" => title_load = true,
            "--title" => {
                let r = PathBuf::from(it.next().ok_or("--title ROOT")?);
                new_game = Some(r.clone());
                title_root = Some(r);
            }
            "--save" => save_file = Some(PathBuf::from(it.next().ok_or("--save FILE")?)),
            "--load" => load_file = Some(PathBuf::from(it.next().ok_or("--load FILE")?)),
            "--saves" => saves_dir = Some(PathBuf::from(it.next().ok_or("--saves DIR")?)),
            "--pitch" => pitch = it.next().ok_or("--pitch DEG")?.parse::<f32>()?.clamp(-89.0, 89.0),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    // Barrel's graphics config, when there is one (`settings::RenderSettings::config_path`).
    let config_text = settings::RenderSettings::config_path().and_then(|p| std::fs::read_to_string(p).ok());
    // With no config yet, a launcher's defaults (Kintsugi's `run` on a handheld):
    // `BARREL_DEFAULT_FULLSCREEN=1` and `BARREL_DEFAULT_PRESET=NAME` (see `--preset`).
    let display_settings = config_text.as_deref().map_or_else(
        || settings::Display { fullscreen: std::env::var_os("BARREL_DEFAULT_FULLSCREEN").is_some_and(|v| v == "1"), ..settings::Display::DEFAULT },
        settings::Display::from_config,
    );
    // A window (with --window) or an offscreen image to draw into.
    let mut pump = match kept_pump {
        // A new size or fullscreen chosen in the last scene: the window opens again with it.
        Some(mut p) => {
            if let Some(d) = p.pending.take() {
                p.reopen(d)?;
            }
            Some(p)
        }
        None => window_frames.map(|_| window::Pump::open(display_settings)).transpose()?,
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
    let (mut width, mut height) = presenter.as_ref().map_or((size, size_h), |p| (p.extent.width, p.extent.height));
    if let Some(p) = &presenter {
        eprintln!("window: {width}x{height}, {:?}, {} swapchain images", p.format, p.images.len());
    }
    if let Some(root) = &new_game {
        let first = barrel_escha::event_start::new_game_event(hero == 1, false);
        event_spec = Some(root.join("Event/event").join(barrel_escha::event_start::event_file(first)).to_string_lossy().into_owned());
        args[1] = "auto".into();
        chain_on = true;
        free_play = true;
        eprintln!("new game: {first}");
    }
    // Offscreen, at most 600 frames written: `--save-every N` lets a run go N times as long.
    let frames = window_frames.unwrap_or(frames.min(MAX_WRITTEN_FRAMES * save_every));
    let d = &gpu.device;
    // Barrel's graphics config (the Options screen's graphics page writes it) when no setting
    // was given on the command line: in a window, or offscreen when `BARREL_CONFIG` names one.
    // Its window lines are read whatever the command line says.
    if !has_settings_flags(&original_args)
        && (window_frames.is_some() || std::env::var_os("BARREL_CONFIG").is_some())
    {
        match settings::RenderSettings::config_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => {
                settings = settings::RenderSettings::from_config(&text);
                eprintln!("graphics from the config: {settings:?}");
            }
            None => {
                if let Some(preset) = std::env::var("BARREL_DEFAULT_PRESET").ok().and_then(|n| settings::RenderSettings::preset(&n)) {
                    settings = preset;
                    eprintln!("graphics: no config yet, the default preset {settings:?}");
                }
            }
        }
    }
    // TAA brings the motion vectors and the jitter, and drops MSAA.
    let settings = settings.resolved();
    let mut renderer = renderer::Renderer::new(&gpu, settings, presenter.as_ref(), width, height, timings, show_motion)?;
    let mut cpu = timings.then(timing::CpuTimer::default);

    // --chain: the events the story starts after this one, played on in turn under the same
    // renderer (`event_start::Chain`; `docs/renderer.md`, "Chained events").
    // The title screen: New Game goes on into the new game; anything else ends here. Offscreen,
    // `BARREL_TITLE_KEYS` (up, down, confirm, back, comma-separated) picks, and the screen as
    // it then stands is written to OUT.
    if let Some(r) = &title_root {
        let keys = title::keys(&std::env::var("BARREL_TITLE_KEYS").unwrap_or_default());
        let target = title::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
        let window = presenter.as_ref().zip(pump.as_mut());
        let out = presenter.is_none().then(|| PathBuf::from(&out_path));
        let saves = saves_dir.clone().or_else(barrel_escha::saves::default_dir);
        let mut window = window;
        // The logo splashes, once a launch (not on a return to the title, nor after the
        // graphics options start Barrel again): in a window before the title; offscreen only
        // with `BARREL_SPLASH_AT` (seconds in), whose frame is written to OUT and ends the run.
        let splash_at = std::env::var("BARREL_SPLASH_AT").ok().and_then(|v| v.parse::<f32>().ok());
        if !title_load && std::env::var_os("BARREL_SPLASHED").is_none() && !SPLASHED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            if let Some((pr, p)) = window.as_mut() {
                splash::run(&gpu, renderer.pool, &target, Some((*pr, &mut **p)), None, r, 0.0, None)?;
            } else if let Some(t) = splash_at {
                splash::run(&gpu, renderer.pool, &target, None, renderer.offscreen.as_ref().map(|(c, _)| *c), r, t, out.as_deref())?;
                return Ok(Ended::Done);
            }
        }
        // The scripted keys (`BARREL_TITLE_KEYS`) press on the first showing only.
        let mut keys = keys;
        let picked = loop {
            let picked = title::run(&gpu, renderer.pool, &target, window.as_mut().map(|(pr, p)| (*pr, &mut **p)), renderer.offscreen.as_ref().map(|(c, _)| *c), r, title_load, &std::mem::take(&mut keys), out.as_deref(), saves.as_deref())?;
            // Options: the graphics page, then the title again. A change is saved to the config,
            // and Barrel starts again in place (the same process, a fresh start at the title):
            // the renderer's renderer.targets and pipelines are built for the settings at its start, and
            // no scene is up yet to carry over.
            let (Some(title::Start::Options), Some((pr, p))) = (&picked, window.as_mut()) else { break picked };
            let now = options::Graphics { render: settings, display: current_display(p) };
            let kept = options::run(&gpu, renderer.pool, &target, Some((*pr, &mut **p)), None, r, now, default_graphics(), &[], None)?;
            eprintln!("title: options: {kept:?}");
            if let Some(new) = kept.filter(|k| *k != now) {
                let path = write_config(&new)?;
                eprintln!("title: graphics saved to {}, starting again", path.display());
                unsafe { d.device_wait_idle()? };
                use std::os::unix::process::CommandExt;
                let exe = std::env::current_exe()?;
                return Err(std::process::Command::new(exe).args(std::env::args_os().skip(1)).env("BARREL_SPLASHED", "1").exec().into());
            }
        };
        eprintln!("title: {picked:?}");
        match picked {
            Some(title::Start::NewGame { logy }) => {
                // The new game with the lead chosen.
                hero = u32::from(logy);
                let first = barrel_escha::event_start::new_game_event(logy, false);
                event_spec = Some(r.join("Event/event").join(barrel_escha::event_start::event_file(first)).to_string_lossy().into_owned());
                eprintln!("new game: {first}");
            }
            Some(title::Start::Load(file)) => {
                // A saved game: as `--load FILE` with any field of the install.
                event_spec = None;
                chain_on = false;
                free_play = false;
                args[1] = r.join("Data/x64/Field/TOWN_04").to_string_lossy().into_owned();
                load_file = Some(file);
            }
            _ => return Ok(Ended::Done),
        }
    }
    let mut chain = match (&event_spec, chain_on) {
        (Some(e), true) => {
            if args[1] != "auto" {
                return Err("--chain needs --field auto".into());
            }
            let root = Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--chain: the event is not inside a game install")?;
            Some((story_chain(root)?, root.to_path_buf()))
        }
        _ => None,
    };
    // The story in free play (after `--free-play`, or from `--load`), for saving.
    let mut session: Option<barrel_escha::session::Session> = None;
    // The gathering tables, loaded at the first gathering.
    let mut gathering: Option<barrel_escha::gather::Gathering> = None;
    let carried_save = carry.save.take();
    if load_file.is_some() || carried_save.is_some() {
        let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("--load: give --field with a field of the game install")?.to_path_buf();
        let file = load_file.clone().unwrap_or_else(|| PathBuf::from("(carried)"));
        let text = match carried_save {
            Some(t) => t,
            None => std::fs::read_to_string(&file)?,
        };
        let (s, at) = barrel_escha::session::Session::load(story_chain(&root)?, &text)?;
        let s = s.with_calendar(barrel_escha::days::Data::load(&root)?, barrel_engine::rng::Rng::seeded(1));
        let barrel_escha::session::Scene::Field(map) = *s.scene() else { return Err("--load: the save is not in free play".into()) };
        let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
        let s = s.with_party(&party_defaults(&exe));
        let names = barrel_escha::data::maps::field_maps(&barrel_formats::pe::Pe::parse(&exe)?);
        let name = names.get(map as usize).ok_or("--load: no such field map")?;
        args[1] = root.join("Data/x64/Field").join(name).to_string_lossy().into_owned();
        player_at = Some(at);
        hero = u32::from(s.logy);
        eprintln!("loaded {}: field {map} ({name}) at ({:.0}, {:.0}), {}", file.display(), at[0], at[1], if s.logy { "Logy" } else { "Escha" });
        // A lost battle (or `--area-map` with `--load`): on the area map at the field's point.
        if (carry.to_area_map || area_mode)
            && let Some((point, at)) = area_point(&root, map)? {
                eprintln!("onto the area map at {point}");
                args[1] = root.join("Data/x64/Field/A15_WMAP").to_string_lossy().into_owned();
                player_at = Some(at);
                player_walk = player_walk.map(|_| [0.0, 0.0]);
                area_mode = true;
            }
        session = Some(s);
    }
    let mut current_event = event_spec.clone();
    let mut frame_base = 0u32;
    let mut first_event = true;
    // The sound output, its mixer, the sound library and the music playing: one for the whole
    // run, so a voice or effect still sounding when a scene hands over plays to its end.
    let mut sound: Option<(Sink, barrel_audio::Mixer, barrel_audio::Library, Option<barrel_audio::Playing>)> = None;
    // The field's ambience loop, while one plays.
    let mut ambience: Option<barrel_audio::Playing> = None;
    'events: loop {
    // A new scene stops the last one's music and ambience; its voices and effects play on.
    if let Some((_, mixer, _, music)) = sound.as_mut() {
        for p in [music.take(), ambience.take()].into_iter().flatten() {
            mixer.stop(p);
        }
    }
    let event_spec = current_event.clone();
    // A chained event plays from its start.
    let (event_at, event_line) = if first_event { (event_at, event_line) } else { (None, None) };
    // Render each frame: read it back to a file, or present it in the window.
    let bytes = (width * height * 4) as u64;
    let (readback, rmem) = gpu.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST, true)?;
    // In a window, frames in flight (`ring::Frames`): the CPU prepares a frame while the GPU
    // draws the one before; with `--timings` one at a time (the timestamps are read each frame).
    // Offscreen the same, but for the frames written to files (waited for, then read back).
    let mut sync = Some(ring::Frames::new(&gpu)?);
    let overlap = renderer.timer.is_none();
    town_seen = None;
    // In the story's free play a field area (map 12 on) has its enemies out
    // (`--creatures`'s field AI), a town none.
    if session.is_some() && current_event.is_none()
        && let Some(root) = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()) {
            let maps = barrel_escha::data::maps::field_maps(&barrel_formats::pe::Pe::parse(&std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?)?);
            let here = Path::new(&args[1]).file_name().and_then(|n| n.to_str()).and_then(|n| maps.iter().position(|m| m == n));
            creatures_area = here.filter(|&m| m >= barrel_escha::data::gimmick::TOWN_MAPS).map(|m| m as i32);
            // A town map has its people, placed by what the story has seen.
            town_seen = here.filter(|&m| m < barrel_escha::data::gimmick::TOWN_MAPS).and(session.as_ref().map(|s| s.seen()));
        }
    // The area map (the world map field) has no enemies.
    if area_mode {
        creatures_area = None;
    }
    // The scene: loaded, then uploaded under the renderer set up above.
    let Loaded { mut scene, pack_bytes, field_maps, field_looks, sky_instances, focus, event_camera, mut actors, absent, mut creatures, mut battles, mut battle_play, mut town_bodies, start_at, start_field, patterns, mut walker } =
        load_scene(SceneSpec {
            field,
            look_from: look_from.clone(),
            battle_group,
            battle_party: carry.back.as_ref().and_then(|b| b.1.save.clone()),
            args: args.clone(),
            event_spec: event_spec.clone(),
            skeleton_spec: skeleton_spec.clone(),
            anim_spec: anim_spec.clone(),
            time,
            morph_spec: morph_spec.clone(),
            face_specs: face_specs.clone(),
            costumes: costumes.clone(),
            costumes_from: costumes_from.clone(),
            event_line,
            event_at,
            look_at,
            creatures_area,
            town_people_from: town_people_from.clone(),
            town_seen: town_seen.clone(),
            player_at,
            player_walk,
            player_facing: player_facing.take(),
            trip: trip_path.clone(),
            logy: hero == 1,
        })?;
    // The maps' screen filter (`PostEffect/texture/overlay.g1t`), for a field whose look asks
    // for one (`A15_WMAP`'s tex_filter_world / tex_filter_area).
    if field_looks.iter().flatten().any(|l| l.filter_world.is_some() || l.filter_area.is_some()) {
        if let Some(root) = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()) {
            let bytes = std::fs::read(root.join("Data/x64/PostEffect/texture/overlay.g1t"))?;
            let pack = G1t::parse(&bytes)?;
            if let Some(t) = pack.textures.first() {
                renderer.set_filter(&gpu, t)?;
            }
        }
    }
    let event_at = start_at;
    let packs: Vec<G1t> = pack_bytes.iter().map(|b| G1t::parse(b)).collect::<std::result::Result<_, _>>()?;
    let all_textures: Vec<&barrel_formats::g1t::Texture> = packs.iter().flat_map(|p| p.textures.iter()).collect();
    if no_terrain {
        scene.terrains.clear();
    }
    let Scene { vertices, indices, mut draws, terrains, lo, hi, mut instances, mut palette, deltas, mut morph_weights, .. } = scene;
    // The field objects' bounds, for view culling (characters and the sky are always drawn;
    // `BARREL_NO_CULL=1` draws everything).
    let cull = std::env::var_os("BARREL_NO_CULL").is_none_or(|v| v != "1");
    for d in draws.iter_mut().filter(|d| cull && d.field.is_some() && !d.sky) {
        let base = instances.get(d.instance as usize).map_or(0, |i| i.palette);
        d.bounds = Bounds::of(&vertices, &indices, d.first_index, d.index_count, &palette, base);
    }
    // Lay each overlay over its base texture into a new RGBA texture.
    let composites = composite_overlays(&mut draws, &all_textures);
    let composite_textures: Vec<barrel_formats::g1t::Texture> = composites
        .iter()
        .map(|(w, h, px)| barrel_formats::g1t::Texture {
            width: *w,
            height: *h,
            format: Format::Rgba8,
            format_code: 1,
            layers: 1,
            volume: false,
            levels: vec![&px[..]],
        })
        .collect();
    let all_textures: Vec<&barrel_formats::g1t::Texture> = all_textures.into_iter().chain(composite_textures.iter()).collect();
    if draws.is_empty() || all_textures.is_empty() {
        return Err("nothing to draw".into());
    }
    eprintln!("{} draws, {} vertices, {} triangles, {} textures", draws.len(), vertices.len(), indices.len() / 3, all_textures.len());
    eprintln!("bounds {lo:?} .. {hi:?}");
    if std::env::var("BARREL_DRY").is_ok() {
        return Ok(Ended::Done);
    }

    // Geometry buffers (host-visible: one frame does not need a staging copy).
    let vbytes = unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * size_of::<Vertex>()) };
    let ibytes = unsafe { std::slice::from_raw_parts(indices.as_ptr() as *const u8, indices.len() * 4) };

    // Textures, uploaded as their BC blocks with all mip levels, then the outline and white.
    let root = event_spec.as_ref().and_then(|e| Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf));
    let (textures, outline_tex, white) = upload_textures(&gpu, renderer.pool, &all_textures, root.as_deref(), &[])?;
    // Instances: one model matrix each, in a storage buffer the vertex shader indexes by the
    // draw's instance number (set 1, bound once for every renderer.pipeline).
    for i in &mut instances {
        i.prev_palette = i.palette + palette.len() as u32;
        i.prev_model = i.model;
    }
    let mut prev_palette = palette.clone();
    if deltas.len() > 1 {
        eprintln!(
            "morphs: {} vertex offsets, {} weights, {} not zero",
            deltas.len() - 1,
            morph_weights.len() - 1,
            morph_weights.iter().filter(|&&w| w != 0.0).count()
        );
    }
    let grounds = upload_grounds(&gpu, renderer.pool, &terrains, false)?;
    // Each field's light texture (none: white).
    let pattern_textures = patterns
        .iter()
        .map(|p| -> Result<_> {
            let Some(bytes) = p else { return Ok(None) };
            let g = G1t::parse(bytes)?;
            g.textures.first().map(|t| upload_texture(&gpu, renderer.pool, t, false)).transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    let scene_data = scene_gpu::SceneData {
        vertices: vbytes,
        indices: ibytes,
        textures,
        instances: &instances,
        palette: &palette,
        deltas: &deltas,
        weights: &morph_weights,
        grounds,
        patterns: pattern_textures,
        white,
    };
    let scene = scene_gpu::SceneGpu::new(&gpu, scene_data, &renderer.shared)?;
    let sets = scene.sets.clone();

    // Camera: look at the model's bounds from its front, turned by --yaw. Models face +z.
    // With an event, look at its characters from a few metres away; otherwise frame it all.
    let (center, radius) = match focus {
        Some(c) => (c, 250.0),
        None => ((lo + hi) * 0.5, (hi - lo).length() * 0.5),
    };
    let fov = 30f32.to_radians();
    let dist = radius / (fov * 0.5).sin() * 1.05;
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    let dir = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
    let (mut eye, mut target, mut fov, mut dist) = (center + dir * dist, center, fov, dist);
    // The event's opening camera (see docs/events.md); --free-camera frames the characters.
    if let (Some(c), true) = (event_camera, use_event_camera) {
        // Game space: yaw about y from -z, pitch up; then mirror x like the vertices.
        let (yw, pt) = (c.yaw.to_radians(), c.pitch.to_radians());
        // As the field camera builds it (barrel_engine::field_camera::FieldCamera::eye_target).
        let back = Vec3::new(yw.sin() * pt.cos(), pt.sin(), yw.cos() * pt.cos());
        let mirror = Vec3::new(-1.0, 1.0, 1.0);
        target = c.target * mirror;
        eye = c.eye.unwrap_or(c.target + back * c.distance) * mirror;
        fov = c.fov.clamp(10.0, 120.0).to_radians();
        dist = c.distance;
        eprintln!("event camera: target {:?}, distance {}, yaw {}, pitch {}, fov {}", c.target, c.distance, c.yaw, c.pitch, c.fov);
    }
    // The area map looks down on the player from the south (screen up is -z), as fitted to
    // the recording (`AREA_CAMERA_*`).
    if area_mode && let Some(c) = focus {
        let (pitch, distance, ahead) = if world_mode { (WORLD_CAMERA_PITCH, WORLD_CAMERA_DISTANCE, WORLD_CAMERA_AHEAD) } else { (AREA_CAMERA_PITCH, AREA_CAMERA_DISTANCE, AREA_CAMERA_AHEAD) };
        let p = pitch.to_radians();
        // On the ground (the focus stands 80 units up, at her chest), that far north.
        target = c - Vec3::new(0.0, 80.0, ahead);
        dist = distance;
        fov = if world_mode { WORLD_CAMERA_FOV } else { AREA_CAMERA_FOV }.to_radians();
        eye = target + Vec3::new(0.0, p.sin(), p.cos()) * dist;
    }
    // The camera can be turned about its target and moved in or out from the window
    // (arrows, + and -): `orbit` holds the turn (yaw, pitch in radians) and the zoom factor.
    let mut orbit = (0f32, 0f32, 1f32);
    // The projection follows the target's current aspect (the window can change size).
    // The eye after the window's own turning and zooming about the target.
    let turned_eye = |eye: Vec3, target: Vec3, (yaw, pitch, zoom): (f32, f32, f32)| {
        let offset = eye - target;
        let flat = offset.length();
        let turned = glam::Quat::from_rotation_y(yaw) * offset;
        let right = turned.cross(Vec3::Y).normalize_or_zero();
        let tilted = glam::Quat::from_axis_angle(right, pitch) * turned;
        // Do not tip over the top or under the bottom.
        let offset = if tilted.normalize_or_zero().y.abs() < 0.98 { tilted } else { turned };
        target + offset.normalize_or_zero() * flat * zoom
    };
    // BARREL_HIDE=a,b: leave out the draws whose name holds any of these (finding which object
    // is which against an oracle frame).
    let hide_names: Vec<String> = std::env::var("BARREL_HIDE").map(|v| v.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect()).unwrap_or_default();
    let hide_by_name = |name: &str| !name.is_empty() && hide_names.iter().any(|h| name.contains(h.as_str()));
    let view_proj = |eye: Vec3, target: Vec3, up: Vec3, dist: f32, fov: f32, width: u32, height: u32, orbit: (f32, f32, f32)| {
        let zoom = orbit.2;
        let view = Mat4::look_at_rh(turned_eye(eye, target, orbit), target, up);
        // Far enough for the distant view (its dome is about 35,000 units across).
        let mut proj = Mat4::perspective_rh(fov, width as f32 / height.max(1) as f32, dist * 0.01 * zoom, (dist * 20.0 * zoom).max(80_000.0));
        proj.y_axis.y *= -1.0; // Vulkan's y points down.
        // The game's x is mirrored into render space (right-handed); mirroring clip-space x back
        // shows the scene as the game does (left to right as in its frames; oracle shot op111).
        proj.x_axis.x *= -1.0;
        proj * view
    };

    // With --frames N, play the event on: the clock at --fps HZ decides the logic ticks
    // (docs/engine.md, "Timing"), each frame draws the last two ticks blended, and only the
    // characters' instance matrices and joint palettes change between frames.
    let mut player = None;
    // The text layer, the font, and the line it was last given (rebuilt when that changes).
    let mut text_layer: Option<(text::Layer, text::Font, Option<(Option<barrel_engine::stage::Line>, u32, u32, Option<u32>, u16, Option<(u32, Option<u32>, u16)>)>)> = None;
    // The narration cards the event shows (u55), and each one's pieces once the layer has
    // their textures: (texture slot, uv, rectangle on a 1920 × 1080 screen).
    let telop_cards: Vec<u32>;
    // The still pictures the event shows (u1/0x30), and each one's texture slot and size on a
    // 1920 × 1080 screen once the layer has them.
    let still_pictures: Vec<u32>;
    let mut still_slots: std::collections::HashMap<u32, (usize, [f32; 2])> = std::collections::HashMap::new();
    let mut telop_pieces: std::collections::HashMap<u32, Vec<(usize, [f32; 4], [f32; 4])>> = std::collections::HashMap::new();
    // The event's music and voices, in a window only (frames written to files are silent):
    // the output, the mixer, the library, and the music now playing.
    // Walking a field in a window, its music plays from the start, looping as the track does.
    if let Some(w) = walker.as_ref()
        && presenter.is_some() {
            if sound.is_none() {
                match barrel_audio::Output::open(0.1) {
                    Ok(out) => sound = Some((Sink::Device(out), barrel_audio::Mixer::game(), barrel_audio::Library::new(&w.root, "Voice_EN", music_tracks(&w.root)), None)),
                    Err(e) => eprintln!("no sound: {e}"),
                }
            }
            if let Some((_, mixer, library, music)) = sound.as_mut() {
                *music = w.music.and_then(|t| library.music(t)).map(|m| mixer.play_stream(m.stream, m.volume, m.loop_from));
                // And its ambience, a looping environment sound under the music.
                if let Some((n, volume)) = w.ambience
                    && let Some(barrel_audio::Effect::Stream(stream, from)) = library.effect(n) {
                        ambience = Some(mixer.play_stream(stream, volume, from));
                    }
                eprintln!("field music {:?}, ambience {:?}", w.music, w.ambience);
            }
        }
    // A battle in a window: its music (`battle::music`, the group's kind and the lead) from the
    // start, in place of the field's.
    if let Some(play) = battle_play.as_ref()
        && presenter.is_some()
        && let Some(root) = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()) {
            if sound.is_none() {
                match barrel_audio::Output::open(0.1) {
                    Ok(out) => sound = Some((Sink::Device(out), barrel_audio::Mixer::game(), barrel_audio::Library::new(root, "Voice_EN", music_tracks(root)), None)),
                    Err(e) => eprintln!("no sound: {e}"),
                }
            }
            if let Some((_, mixer, library, music)) = sound.as_mut() {
                if let Some(old) = music.take() {
                    mixer.stop(old);
                }
                *music = play.music.and_then(|t| library.music(t)).map(|m| mixer.play_stream(m.stream, m.volume, m.loop_from));
                eprintln!("battle music {:?}, playing: {}", play.music, music.is_some());
            }
        }
    let mut motion_names: Vec<String> = Vec::new();
    let messages: Vec<barrel_formats::ebm::Message>;
    let mut speaker_names: Vec<String> = Vec::new();
    // The message window's names (the executable's speaker-name table).
    let mut window_names: Vec<String> = Vec::new();
    let mut face_tables_play: Option<barrel_escha::face::Tables> = None;
    let mut gestures_play: Vec<[Option<barrel_escha::gesture::Form>; 2]> = Vec::new();
    // The motions' divisions (s5, s11), from Saves/chara.
    // The camera quakes (t14), and the generator their shake draws from: the game's own kind,
    // seeded here (the game's renderer.shared generator state is not followed: docs/events.md).
    let quakes: Vec<barrel_formats::quake::Quake> = event_spec
        .as_ref()
        .and_then(|e| Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf))
        .and_then(|root| std::fs::read(root.join("Saves/cameraQuake/camera_quake_data.xml")).ok())
        .and_then(|b| barrel_formats::table::parse(&b).ok())
        .map(|t| barrel_formats::quake::read(&t))
        .unwrap_or_default();
    let mut quake_rng = barrel_engine::rng::Rng::from_state([0x1234_5678, 0x9abc_def0, 0x0fed_cba9, 0x8765_4321]);
    let mut divisions = barrel_formats::chara_div::Divisions::new();
    // The event is played (up to `--at` and on) even for one frame, so a still shows the state its
    // commands have set by then (objects shown, versions, the camera).
    let event_file = event_spec.as_ref().map(std::fs::read).transpose()?;
    let event_data = event_file.as_ref().map(|b| barrel_formats::ebd::Ebd::parse(b)).transpose()?;
    if let (Some(event), Some(e)) = (&event_data, &event_spec) {
        use barrel_engine::stage::{Measures, Playback, Stage};
        let root = Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("event is not inside a game install")?;
        let xml = |rel: &str| std::fs::read(root.join(rel)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
        let measures = xml("Saves/chara/Chara.xml").map(|t| Measures::table(&t)).unwrap_or_default();
        let curves = xml("Saves/curve/curve_data.xml").map(|t| barrel_engine::curve::Curve::table(&t)).unwrap_or_default();
        let stem = Path::new(e).file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ebm = Path::new(e).with_file_name(format!("{}.ebm", stem.replacen("EVENT_", "EVENT_MESSAGE_", 1)));
        // The English lines (Event_EN mirrors Event), else the original's.
        let english = Path::new(e).strip_prefix(root.join("Event")).ok().map(|rel| root.join("Event_EN").join(rel).with_file_name(ebm.file_name().unwrap_or_default()));
        let read_lines = |p: &Path| std::fs::read(p).ok().and_then(|b| barrel_formats::ebm::parse(&b).ok()).filter(|m| !m.is_empty());
        messages = english.as_deref().and_then(read_lines).or_else(|| read_lines(&ebm)).unwrap_or_default();
        window_names = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))
            .ok()
            .and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_escha::data::layout::speaker_names(&pe)))
            .unwrap_or_default();
        speaker_names = xml("Saves_EN/chara/Chara.xml")
            .or_else(|| xml("Saves/chara/Chara.xml"))
            .map(|t| t.children.iter().map(|c| c.attr("Name").unwrap_or("").to_string()).collect())
            .unwrap_or_default();
        motion_names = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_escha::gesture::motions(&pe)).filter(|m| !m.is_empty()))
            .unwrap_or_default();
        for name in ["CharaDiv.xml", "SubCharaDiv.xml"] {
            if let Some(t) = std::fs::read(root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok()) {
                barrel_formats::chara_div::read(&t, &mut divisions);
            }
        }
        // The motions' lengths for s6 (wait for a motion): every (character, motion, division)
        // the event plays, measured from the clips and the divisions before scheduling.
        let mut lengths: std::collections::HashMap<(u32, u32, Option<u32>), (f32, bool)> = Default::default();
        {
            use barrel_engine::event::Step;
            let mut current: std::collections::HashMap<u32, u32> = Default::default();
            let mut wanted = Vec::new();
            for step in barrel_engine::event::Player::new(event, &messages, &barrel_formats::ebd::OperandTable::escha())?.hero(hero) {
                match step {
                    Step::Motion { character, motion, division, .. } => {
                        current.insert(character, motion);
                        wanted.push((character, motion, division));
                    }
                    Step::MotionDivision { character, division, .. } => {
                        if let Some(&motion) = current.get(&character) {
                            wanted.push((character, motion, Some(division)));
                        }
                    }
                    _ => {}
                }
            }
            for (character, motion, division) in wanted {
                let Some(a) = actors.iter_mut().find(|a| a.who == character) else { continue };
                let Some(column) = motion_names.get(motion as usize) else { continue };
                let Some(duration) = a.motions.get(column).map(|c| c.duration) else { continue };
                let found = division.and_then(|k| *divisions.get(&(a.motions.head.clone(), column.clone()))?.get(k as usize)?);
                let length = match found {
                    Some(d) => (d.end.unwrap_or(duration).min(duration) - d.start).max(0.0),
                    None => duration,
                };
                lengths.insert((character, motion, division), (length, found.is_some_and(|d| d.loops)));
            }
        }
        // Lines held for a reading time, as nobody presses a button here.
        let steps = barrel_engine::event::Player::new(event, &messages, &barrel_formats::ebd::OperandTable::escha())?
            .hero(hero)
            .reading_time(barrel_engine::event::reading_time)
            .motion_length(Box::new(move |c, m, d| lengths.get(&(c, m, d)).copied()))
            .schedule();
        // BARREL_SCHEDULE_LOG: the times this playback shows each narration card, line and field
        // (they differ from `barrel play-event --timed`, which has no motion lengths).
        if std::env::var("BARREL_SCHEDULE_LOG").is_ok() {
            use barrel_engine::event::Step;
            for t in &steps {
                match &t.step {
                    Step::Telop { card, .. } => eprintln!("{:8.2} card {card}", t.time),
                    Step::Message { index, .. } => eprintln!("{:8.2} line {index}", t.time),
                    Step::FieldChange { map, .. } => eprintln!("{:8.2} field {map}", t.time),
                    _ => {}
                }
            }
        }
        still_pictures = steps
            .iter()
            .filter_map(|t| match t.step {
                barrel_engine::event::Step::Still { picture, .. } => Some(picture),
                _ => None,
            })
            .collect();
        telop_cards = steps
            .iter()
            .filter_map(|t| match t.step {
                barrel_engine::event::Step::Telop { card, .. } => Some(card),
                _ => None,
            })
            .collect();
        // The lines' voices (English): the event's first clip from the executable's table
        // (`voice::first_clips`, by the event's EVENT_* number); line k plays that plus k.
        let story_voice = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).ok().and_then(|b| {
            let pe = barrel_formats::pe::Pe::parse(&b).ok()?;
            let (names, _) = pe.enum_names("EVENT_MM01_OP1_111")?;
            let event = names.iter().position(|n| *n == stem)?;
            let clip = (*barrel_escha::data::voice::first_clips(&pe, barrel_escha::data::voice::FIRST_CLIPS_EN)?.get(event)?)?;
            eprintln!("story voices from clip {clip}");
            Some(clip)
        });
        let mut play = Playback::new(Stage { measures, curves, reading: Some(barrel_engine::event::reading_time), story_voice, ..Stage::default() }, steps);
        face_tables_play = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_escha::face::Tables::read(&pe)).filter(|t| !t.brow.is_empty()));
        gestures_play = std::fs::read_dir(root)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "exe"))
            .filter_map(|p| std::fs::read(p).ok())
            .find_map(|b| barrel_escha::gesture::read(&b))
            .unwrap_or_default();
        // Frames written to files are not real time: no stall cap (a window keeps it).
        let mut clock = barrel_engine::clock::Clock::default();
        if presenter.is_none() {
            clock.max_frame = f64::INFINITY;
        }
        while clock.time() + 1e-9 < event_at.unwrap_or(0.0) as f64 {
            clock.ticks += 1;
            play.tick();
        }
        let snap = play.stage.snapshot();
        eprintln!("playing {frames} frames at {fps} Hz from {:.2} s, {} actors", clock.time(), actors.len());
        player = Some((play, clock, snap.clone(), snap));
        if presenter.is_some() && sound.is_none() {
            match barrel_audio::Output::open(0.1) {
                Ok(out) => sound = Some((Sink::Device(out), barrel_audio::Mixer::game(), barrel_audio::Library::new(root, "Voice_EN", music_tracks(root)), None)),
                Err(e) => eprintln!("no sound: {e}"),
            }
        } else if presenter.is_none() && sound.is_none()
            && let Some(path) = audio_out.clone() {
                let start = player.as_ref().map_or(0.0, |p| p.1.time());
                sound = Some((Sink::File { capture: barrel_audio::Capture::new(), start, path }, barrel_audio::Mixer::game(), barrel_audio::Library::new(root, "Voice_EN", music_tracks(root)), None));
            }
        // The event's lines in the game's English font.
        let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).ok();
        let glyphs = exe.as_ref().and_then(|b| barrel_formats::pe::Pe::parse(b).ok().and_then(|pe| barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN)));
        let atlas = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t")).ok();
        let common = std::fs::read(root.join("Data/x64/WinXls_EN/a15_common.g1t")).ok();
        let ui = common.as_ref().and_then(|c| barrel_formats::g1t::G1t::parse(c).ok());
        // The narration cards' pieces and their WinXls_EN textures (slots after the font and
        // the window).
        let mut sheet_names: Vec<(String, u32)> = Vec::new();
        let mut card_pieces: Vec<(u32, Vec<barrel_escha::data::layout::Piece>)> = Vec::new();
        if let Some(pe) = exe.as_ref().and_then(|b| barrel_formats::pe::Pe::parse(b).ok()) {
            for &card in &telop_cards {
                let Some(id) = barrel_escha::data::layout::telop_layout(&pe, card) else { continue };
                let pieces = barrel_escha::data::layout::pieces(&pe, id);
                for p in &pieces {
                    let key = (p.texture.clone(), p.index);
                    if !sheet_names.contains(&key) {
                        sheet_names.push(key);
                    }
                }
                card_pieces.push((card, pieces));
            }
        }
        // The still pictures: Data/x64/Ipu_EN first (the English ones), else Ipu.
        let mut still_list: Vec<(u32, barrel_escha::data::layout::Still)> = Vec::new();
        if let Some(pe) = exe.as_ref().and_then(|b| barrel_formats::pe::Pe::parse(b).ok()) {
            for &n in &still_pictures {
                if let Some(st) = barrel_escha::data::layout::still(&pe, n)
                    && !still_list.iter().any(|(k, _)| *k == n) {
                        still_list.push((n, st));
                    }
            }
        }
        let still_bytes: Vec<Option<Vec<u8>>> = still_list
            .iter()
            .map(|(_, st)| {
                let x64 = root.join("Data/x64");
                std::fs::read(x64.join(format!("{}_EN", st.folder)).join(&st.file)).or_else(|_| std::fs::read(x64.join(&st.folder).join(&st.file))).ok()
            })
            .collect();
        let still_g1t: Vec<Option<barrel_formats::g1t::G1t>> = still_bytes.iter().map(|b| b.as_ref().and_then(|b| barrel_formats::g1t::G1t::parse(b).ok())).collect();
        let sheet_bytes: Vec<Option<Vec<u8>>> = sheet_names.iter().map(|(n, _)| std::fs::read(root.join("Data/x64/WinXls_EN").join(n)).ok()).collect();
        let sheets: Vec<Option<barrel_formats::g1t::G1t>> = sheet_bytes.iter().map(|b| b.as_ref().and_then(|b| barrel_formats::g1t::G1t::parse(b).ok())).collect();
        match (glyphs, atlas.as_ref().and_then(|a| barrel_formats::g1t::G1t::parse(a).ok()), ui.as_ref().and_then(|u| u.textures.get(4))) {
            (Some(glyphs), Some(g), Some(window)) if !g.textures.is_empty() => {
                let size = g.textures[0].width as f32;
                let mut images = vec![&g.textures[0], window];
                // Each sheet's slot in the layer (None: missing).
                let mut slots = Vec::new();
                for (sheet, (_, index)) in sheets.iter().zip(&sheet_names) {
                    match sheet.as_ref().and_then(|s| s.textures.get(*index as usize)) {
                        Some(t) => {
                            slots.push(Some((images.len(), t.width as f32, t.height as f32)));
                            images.push(t);
                        }
                        None => slots.push(None),
                    }
                }
                for ((n, st), g) in still_list.iter().zip(&still_g1t) {
                    if let Some(t) = g.as_ref().and_then(|g| g.textures.first()) {
                        still_slots.insert(*n, (images.len(), st.size));
                        images.push(t);
                    }
                }
                for (card, pieces) in &card_pieces {
                    let list = pieces
                        .iter()
                        .filter_map(|p| {
                            let (slot, w, h) = slots[sheet_names.iter().position(|(n, i)| *n == p.texture && *i == p.index)?]?;
                            let [l, t, r, b] = p.src.map(f32::from);
                            let [x, y] = p.at.map(|v| v as f32);
                            Some((slot, [l / w, t / h, r / w, b / h], [x, y, x + r - l, y + b - t]))
                        })
                        .collect();
                    telop_pieces.insert(*card, list);
                }
                text_layer = Some((text::Layer::new(&gpu, renderer.pool, renderer.post_pass, &images)?, text::Font { glyphs, size }, None));
            }
            _ => eprintln!("no text: the English executable's glyph table, the Font_EN atlas or WinXls_EN/a15_common.g1t is missing"),
        }
    }
    // The field HUD in free play (`hud`): its layer (the font atlas, then the HUD's pictures),
    // the font, the pictures' slots, layout 625's nodes, and what it last showed.
    // The install, from the field drawn (free play has no event to find it by).
    let hud_root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf);
    let hud_exe = match &hud_root {
        Some(r) if session.is_some() && player.is_none() => std::fs::read(r.join("Atelier_Escha_and_Logy_EN.exe")).ok(),
        _ => None,
    };
    let mut field_hud: Option<(text::Layer, text::Font, hud::Sheets, Vec<barrel_escha::data::layout::Placed>, Option<(hud::Values, u32, u32, Option<(bool, bool)>)>)> = None;
    // The field's HUD (layout 625); on the maps its date panel and the maps' button lists (636).
    // Not in a battle: the game shows only the battle's own HUD there (the battle recordings).
    if session.is_some() && player.is_none() && battle_play.is_none() && std::env::var("BARREL_NO_HUD").is_err() {
        let made = (|| -> Option<(text::Layer, text::Font, hud::Sheets, Vec<barrel_escha::data::layout::Placed>)> {
            let Some(exe) = hud_exe.as_ref() else {
                return None;
            };
            let root = hud_root.as_ref()?;
            let pe = barrel_formats::pe::Pe::parse(exe).ok()?;
            let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN)?;
            let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t")).ok()?;
            let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes).ok()?;
            let mut nodes = barrel_escha::data::layout::placed(&pe, hud::FIELD_HUD);
            if area_mode {
                nodes.extend(barrel_escha::data::layout::placed(&pe, hud::MAP_LIST));
            }
            let keys = hud::sheet_keys(&pe, &nodes);
            let mut packs: Vec<(String, Vec<u8>)> = Vec::new();
            for (name, _) in &keys {
                if !packs.iter().any(|(n, _)| n == name) {
                    let x64 = root.join("Data/x64");
                    match std::fs::read(x64.join("WinXls_EN").join(name)).or_else(|_| std::fs::read(x64.join("WinXls").join(name))) {
                        Ok(bytes) => packs.push((name.clone(), bytes)),
                        Err(e) => eprintln!("HUD: {name}: {e}"),
                    }
                }
            }
            let parsed: Vec<(String, barrel_formats::g1t::G1t)> = packs.iter().filter_map(|(n, b)| Some((n.clone(), barrel_formats::g1t::G1t::parse(b).ok()?))).collect();
            let mut images = vec![atlas.textures.first()?];
            let mut sheets = hud::Sheets { keys: Vec::new(), sizes: Vec::new(), first: 1 };
            for (name, index) in keys {
                let Some(t) = parsed.iter().find(|(n, _)| *n == name).and_then(|(_, g)| g.textures.get(index as usize)) else { continue };
                sheets.sizes.push([t.width as f32, t.height as f32]);
                sheets.keys.push((name, index));
                images.push(t);
            }
            let size = images[0].width as f32;
            let layer = text::Layer::new(&gpu, renderer.pool, renderer.post_pass, &images).ok()?;
            Some((layer, text::Font { glyphs, size }, sheets, nodes))
        })();
        match made {
            Some((layer, font, sheets, nodes)) => field_hud = Some((layer, font, sheets, nodes, None)),
            None => eprintln!("no HUD: the English executable, the font or a WinXls pack is missing"),
        }
    }
    // A carried notice (a lost battle's line) in the message window until confirmed (Enter;
    // renderer.offscreen `NOTICE_SECONDS`): its layer (the font, the window) and the time it went up.
    let mut notice: Option<(text::Layer, f32)> = None;
    if let (Some((speaker, line)), Some(root), true) = (carry.notice.take(), hud_root.as_ref(), session.is_some() && player.is_none()) {
        let made = (|| -> Option<text::Layer> {
            let exe = hud_exe.as_ref()?;
            let pe = barrel_formats::pe::Pe::parse(exe).ok()?;
            let glyphs = barrel_escha::data::font::glyphs(&pe, barrel_escha::data::font::FONT_EN)?;
            let atlas_bytes = std::fs::read(root.join("Data/x64/Font_EN/MainFont_x64_0.g1t")).ok()?;
            let common_bytes = std::fs::read(root.join("Data/x64/WinXls_EN/a15_common.g1t")).ok()?;
            let atlas = barrel_formats::g1t::G1t::parse(&atlas_bytes).ok()?;
            let common = barrel_formats::g1t::G1t::parse(&common_bytes).ok()?;
            let images = [atlas.textures.first()?, common.textures.get(4)?];
            let font = text::Font { glyphs, size: images[0].width as f32 };
            let mut layer = text::Layer::new(&gpu, renderer.pool, renderer.post_pass, &images).ok()?;
            layer.set(&gpu, &message::dialogue(&font, Some(&speaker), &line, width as f32, height as f32)).ok()?;
            Some(layer)
        })();
        match made {
            Some(layer) => {
                eprintln!("{speaker}: {line}");
                notice = Some((layer, 0.0));
            }
            None => eprintln!("no message window for the notice"),
        }
    }
    // The field enemies already beaten (carried back from their battles).
    if let Some((field, ..)) = creatures.as_mut() {
        for &k in &carry.defeated {
            if let Some(d) = field.defeated.get_mut(k) {
                *d = true;
            }
        }
    }
    let mut battle_keys: std::collections::VecDeque<title::Button> = title::keys(&std::env::var("BARREL_BATTLE_KEYS").unwrap_or_default()).into();
    let mut battle_layer = match battle_play.as_ref() {
        Some(_) if std::env::var("BARREL_NO_HUD").is_err() => {
            let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf);
            match root.map(|r| battle_hud(&gpu, renderer.pool, renderer.post_pass, &r)) {
                Some(Ok(h)) => Some(h),
                Some(Err(e)) => {
                    eprintln!("no battle HUD: {e}");
                    None
                }
                None => None,
            }
        }
        _ => None,
    };
    // The field drawn (its place in `field_maps`): where the event is when playback starts,
    // else the first.
    let mut current_field: u16 = player
        .as_ref()
        .and_then(|p| p.0.stage.field)
        .or(start_field)
        .and_then(|m| field_maps.iter().position(|&f| f == m))
        .map_or(0, |k| k as u16);
    let start_ticks = player.as_ref().map_or(0, |p| p.1.ticks);
    let mut hidden = vec![false; instances.len()];
    // The field object versions shown (q15): none until the event shows one.
    let mut versions_shown = player.as_ref().map(|p| p.0.stage.versions_shown.clone()).unwrap_or_default();
    // Whether the field's xEV_ objects show (q10): only while an event shows them.
    let mut event_objects = player.as_ref().is_some_and(|p| p.0.stage.event_objects);
    for a in &actors {
        hidden[a.instance as usize] = match &player {
            Some((_, _, _, now)) => !now.characters.contains_key(&a.who),
            None => absent.contains(&a.who),
        };
    }

    let mut last_frame = std::time::Instant::now();
    let mut stale = false;
    let mut paused = false;
    // In a window a line waits at its end for A / Enter (as the game's message window does);
    // pressed earlier, it ends the line there and then. The line end last let go, the press,
    // and the line's voice (stopped when the line is ended early).
    let mut line_released: Option<f32> = None;
    let mut next_line = false;
    let mut line_voice: Option<barrel_audio::Playing> = None;
    // Auto mode (T, or Y on a pad, during an event): lines go on by themselves once their
    // reading time is up and their voice has finished.
    let mut auto_lines = false;
    // The fades' presets (u22), from the executable.
    let fade_presets = event_spec
        .as_ref()
        .and_then(|e| Path::new(e).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf))
        .and_then(|r| std::fs::read(r.join("Atelier_Escha_and_Logy_EN.exe")).ok())
        .and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_formats::field_look::fade_presets(&pe, barrel_formats::field_look::FADE_PRESETS_EN)))
        .unwrap_or_default();
    // The start time of the last screen effect seen (a new one starts a cross-fade).
    let mut last_effect: Option<f32> = None;
    let mut prev_mvp: Option<Mat4> = None;
    let mut jitter_index = 0u32;
    let mut frame_jitter = (0f32, 0f32);
    let mut last_shot: Option<taa::Shot> = None;
    let mut shot_field = u16::MAX;
    // The light texture's clock: from the scene's first frame (the game's starts at the field's load).
    let mut pattern_time = 0.0f32;
    // The grass sway groups' clocks (game::wind::Shaker), and the time stepped this frame.
    let mut shakers: Vec<barrel_engine::wind::Shaker> = Vec::new();
    let mut sway_dt = 0.0f32;
    // The trees' node springs, for the field they were made for: (instance, its rest
    // transform, its shaker in render space, its spring).
    type TreeSpring = (u32, Mat4, barrel_formats::field_look::NShaker, barrel_engine::wind::NodeSpring);
    let mut tree_springs: Option<(u16, Vec<TreeSpring>)> = None;
    // The trees' random push (a small xorshift: the game's rand() for a look, not for play).
    let mut sway_rng = SwayRng(0x9e37_79b9);
    let mut brushed = false;
    let mut ran = 0u32;
    let mut stop = false;
    // A door the player stepped into: the map it leads to, where she stands there, her velocity.
    let mut door: Option<(String, [f32; 2], [f32; 2])> = None;
    // The player stepped onto the world map.
    let mut world_exit = false;
    // The area map (`--area-map`), built once the player stands on the world map, and the field
    // map chosen with Enter.
    let mut area_map: Option<AreaMap> = None;
    let mut area_enter: Option<AreaPick> = None;
    // Offscreen keys for the world map (`BARREL_WORLD_KEYS`) and the area map
    // (`BARREL_AREA_KEYS`): `|` parts them by visit, the first part for the first scene at
    // that level, the next for the next.
    static WORLD_VISITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static AREA_VISITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut area_keys: std::collections::VecDeque<title::Button> = std::collections::VecDeque::new();
    if area_mode {
        let (name, visits) = if world_mode { ("BARREL_WORLD_KEYS", &WORLD_VISITS) } else { ("BARREL_AREA_KEYS", &AREA_VISITS) };
        let visit = visits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let list = std::env::var(name).unwrap_or_default();
        area_keys = title::keys(list.split('|').nth(visit).unwrap_or("")).into_iter().collect();
    }
    for frame in 0..frames - frame_base {
    ran = frame + 1;
    if let Some(c) = cpu.as_mut() {
        c.begin();
    }
    // The slot this frame's data goes into is free once the frame two back has been drawn.
    if let Some(f) = sync.as_mut() {
        f.wait_slot(&gpu, renderer.pool)?;
        if let Some(c) = cpu.as_mut() {
            c.mark("wait for the frame two back");
        }
    }
    // Chained: the event over (every step applied), the next one takes the window.
    if chain.is_some() && frame > 0 && player.as_ref().is_some_and(|p| p.0.finished()) {
        ran = frame;
        break;
    }
    // Last frame's transforms and joints, before this frame moves anything (motion vectors).
    for i in &mut instances {
        i.prev_model = i.model;
    }
    prev_palette.copy_from_slice(&palette);
    // The real time since the last frame (a window's frames are real time).
    let now = std::time::Instant::now();
    let frame_dt = (now - last_frame).as_secs_f64().min(0.25) as f32;
    last_frame = now;
    // A jump asked for this frame (J, or `BARREL_JUMP_AT`).
    let mut jump_now = std::env::var("BARREL_JUMP_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64);
    let mut strike_now = std::env::var("BARREL_STRIKE_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64);
    let mut open_menu = false;
    // The page the menu opens on (the field's LB: Assignment).
    let mut menu_page: Option<menu::Start> = None;
    // The Atelier's cauldron (C in free play there; `BARREL_MIX_AT` renderer.offscreen).
    let mut open_mix = presenter.is_none() && std::env::var("BARREL_MIX_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64);
    // The Equip page renderer.offscreen at a frame (`BARREL_EQUIP_AT`, `BARREL_EQUIP_KEYS`); the run goes on.
    if presenter.is_none() && std::env::var("BARREL_EQUIP_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64)
        && let (Some(root), Some((color, _))) = (hud_root.clone(), renderer.offscreen.as_ref()) {
            let keys = title::keys(&std::env::var("BARREL_EQUIP_KEYS").unwrap_or_default());
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            let color = *color;
            equip_visit(&gpu, renderer.pool, &target, None, Some(color), &root, session.as_mut(), &keys, Some(Path::new(&out_path)), frame)?;
        }
    // The Options screen's graphics page renderer.offscreen (`BARREL_OPTIONS_AT`, `BARREL_OPTIONS_KEYS`),
    // drawn to OUT-options.png; the run goes on.
    if presenter.is_none() && std::env::var("BARREL_OPTIONS_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64)
        && let (Some(root), Some((color, _))) = (hud_root.clone(), renderer.offscreen.as_ref()) {
            let keys = title::keys(&std::env::var("BARREL_OPTIONS_KEYS").unwrap_or_default());
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            let shot = Path::new(&out_path).with_extension("").to_string_lossy().into_owned() + "-options.png";
            let now = options::Graphics { render: settings, display: display_settings };
            let first = options::Graphics { render: settings::RenderSettings::DESKTOP, display: settings::Display::DEFAULT };
            let kept = options::run(&gpu, renderer.pool, &target, None, Some(*color), &root, now, first, &keys, Some(Path::new(&shot)))?;
            eprintln!("frame {frame}: options: {kept:?}");
            // Kept: into the config `BARREL_CONFIG` names (renderer.offscreen runs never write the user's).
            if let Some(new) = kept.filter(|_| std::env::var_os("BARREL_CONFIG").is_some()) {
                eprintln!("frame {frame}: graphics saved to {}", write_config(&new)?.display());
            }
        }
    // A town shop (B in free play in town; `BARREL_SHOP_AT` renderer.offscreen, `BARREL_SHOP_KEYS`).
    let mut open_shop = false;
    if presenter.is_none() && std::env::var("BARREL_SHOP_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64)
        && let (Some(root), Some((color, _))) = (hud_root.clone(), renderer.offscreen.as_ref()) {
            let keys = title::keys(&std::env::var("BARREL_SHOP_KEYS").unwrap_or_default());
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            let color = *color;
            shop_visit(&gpu, renderer.pool, &target, None, Some(color), &root, session.as_mut(), &keys, Some(Path::new(&out_path)), frame)?;
        }
    let pose_jumps = std::env::var("BARREL_POSE_JUMPS").ok().and_then(|v| v.parse::<f32>().ok());
    // Offscreen: the cauldron at a frame (`BARREL_MIX_AT`) with the buttons in `BARREL_MIX_KEYS`.
    if open_mix && presenter.is_none()
        && let (Some(root), Some((color, _))) = (hud_root.clone(), renderer.offscreen.as_ref()) {
            let keys = title::keys(&std::env::var("BARREL_MIX_KEYS").unwrap_or_default());
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            let color = *color;
            cauldron(&gpu, renderer.pool, &target, None, Some(color), &root, session.as_mut(), &keys, Some(Path::new(&out_path)), frame)?;
        }
    // In a window `BARREL_MENU_AT` opens the menu at that frame, `BARREL_MENU_KEYS` and
    // `BARREL_OPTIONS_KEYS` pressed in it first (a scripted check of the screens in a window).
    // Once a run: a scene started again (as the page's confirm does) does not run it again.
    static SCRIPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let scripted = presenter.is_some()
        && std::env::var("BARREL_MENU_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64)
        && !SCRIPTED.swap(true, std::sync::atomic::Ordering::Relaxed);
    if scripted {
        open_menu = true;
    }
    let script = |name: &str| if scripted { title::keys(&std::env::var(name).unwrap_or_default()) } else { Vec::new() };
    // Offscreen: the main menu drawn at a frame (`BARREL_MENU_AT`) after the buttons in
    // `BARREL_MENU_KEYS` (as `BARREL_TITLE_KEYS`), written to the output picture; the run ends.
    if presenter.is_none() && std::env::var("BARREL_MENU_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64)
        && let (Some(root), Some((color, _))) = (hud_root.as_ref(), renderer.offscreen.as_ref()) {
            let keys = title::keys(&std::env::var("BARREL_MENU_KEYS").unwrap_or_default());
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            let picked = menu::run(&gpu, renderer.pool, &target, None, Some(*color), root, session.as_ref().is_some_and(|s| s.logy), session.as_ref().and_then(|s| s.party.get(usize::from(s.logy))), &basket_of(session.as_ref()), &card_of(root, session.as_ref()), &keys, None, Some(Path::new(&out_path)))?;
            eprintln!("menu: {picked:?}");
            if let Some(menu::Outcome::Option(o @ (menu::OptionRow::ReturnToTitle | menu::OptionRow::Load))) = picked {
                restart = Some(title_args(&original_args, root, o == menu::OptionRow::Load));
            }
            stop = true;
            break;
        }
    // Offscreen, a carried notice goes down after `NOTICE_SECONDS`.
    if presenter.is_none() && let Some((_, up)) = notice.as_mut() {
        *up += (1.0 / fps) as f32;
        if *up >= NOTICE_SECONDS {
            unsafe { gpu.device.device_wait_idle()? };
            if let Some((layer, _)) = notice.take() {
                layer.destroy(&gpu);
            }
        }
    }
    let mut save_now = std::env::var("BARREL_SAVE_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64);
    let mut gather_now = std::env::var("BARREL_GATHER_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frame as u64);
    if let Some(p) = pump.as_mut() {
        if !p.pump() {
            eprintln!("window closed after {frame} frames");
            stop = true;
            break;
        }
        // Space pauses the event, Escape quits; arrows turn the camera about its target
        // and + / - move it in and out (R puts it back).
        use winit::keyboard::KeyCode;
        let mut quit = false;
        let battle_waits = battle_play.as_ref().is_some_and(BattlePlay::waiting);
        let (keys, pads) = p.take_input();
        // The pad as the game reads it here (`docs/engine.md`, "Controller"; A, B and the D-pad
        // came in as Enter, Backspace and the arrows): in free play X strikes, Y opens the main
        // menu, B jumps, A gathers, the left stick's click turns running on or off and the right
        // one's puts the camera back; in events X turns "Fast Forward" on or off (Barrel's lines
        // going on by themselves stand in for it; the escha-event-buttons recording: the event's
        // legend shows X Fast Forward and View Message Log, Start and Y do nothing).
        let mut keys = keys;
        for b in pads {
            use pad::Pad as P;
            let free_play = walker.is_some() && area_map.is_none() && battle_play.is_none();
            let on_area_map = walker.is_some() && area_map.is_some() && battle_play.is_none();
            let key = match b {
                P::West if free_play => Some(KeyCode::KeyK),
                P::North if free_play && field_hud.is_some() => Some(KeyCode::Space),
                P::East if free_play => Some(KeyCode::KeyJ),
                P::South if free_play => Some(KeyCode::KeyG),
                P::LeftStick if free_play => Some(KeyCode::KeyT),
                // RB: "Leave Map", as walking out of the field's exit (onto its area map; the
                // town's onto the world map).
                P::RightShoulder if free_play && field_hud.is_some() && !area_mode => {
                    eprintln!("frame {frame}: leave map");
                    world_exit = true;
                    None
                }
                // LB: "Assignment", the main menu's page on its own.
                P::LeftShoulder if free_play && field_hud.is_some() => {
                    open_menu = true;
                    menu_page = Some(menu::Start::Page(menu::Page::Assignment(assignment::Assignment::default())));
                    None
                }
                // X on an area map (not the world's): the panel's list, gatherables or monsters.
                P::West if on_area_map => {
                    if let Some(a) = area_map.as_mut().filter(|a| !a.world) {
                        a.monsters_shown = !a.monsters_shown;
                    }
                    None
                }
                // The area map (the escha-areamap-buttons recording): Y the main menu, LB the
                // Assignment page, RB "System" (the Option list alone).
                P::North | P::LeftShoulder | P::RightShoulder if on_area_map => {
                    open_menu = true;
                    menu_page = match b {
                        P::LeftShoulder => Some(menu::Start::Page(menu::Page::Assignment(assignment::Assignment::default()))),
                        P::RightShoulder => Some(menu::Start::System),
                        _ => None,
                    };
                    None
                }
                P::RightStick if free_play => Some(KeyCode::KeyR),
                P::West if player.is_some() => Some(KeyCode::KeyT),
                // L3 in battle: "Fast Forward" (F on the keyboard).
                P::LeftStick if battle_play.is_some() => Some(KeyCode::KeyF),
                P::East if player.is_some() => Some(KeyCode::Enter),
                _ => None,
            };
            if let Some(k) = key {
                keys.push(k);
            } else if !matches!(b, P::South | P::East | P::Up | P::Down | P::Left | P::Right) && !open_menu && !world_exit && !(b == P::West && area_map.is_some()) {
                eprintln!("frame {frame}: pad {b:?} does nothing here yet");
            }
        }
        for key in keys {
            // A member's turn: the arrows choose the command and target, Enter gives it.
            if battle_waits {
                let b = match key {
                    KeyCode::ArrowUp => Some(title::Button::Up),
                    KeyCode::ArrowDown => Some(title::Button::Down),
                    KeyCode::ArrowLeft => Some(title::Button::Left),
                    KeyCode::ArrowRight => Some(title::Button::Right),
                    KeyCode::Enter | KeyCode::NumpadEnter => Some(title::Button::Confirm),
                    _ => None,
                };
                if let (Some(b), Some(play)) = (b, battle_play.as_mut()) {
                    play.press(b);
                    continue;
                }
            }
            if let Some(a) = area_map.as_mut() {
                let b = match key {
                    KeyCode::KeyW => Some(title::Button::Up),
                    KeyCode::KeyS => Some(title::Button::Down),
                    KeyCode::KeyA => Some(title::Button::Left),
                    KeyCode::KeyD => Some(title::Button::Right),
                    KeyCode::Enter | KeyCode::NumpadEnter => Some(title::Button::Confirm),
                    KeyCode::Backspace | KeyCode::KeyX => Some(title::Button::Back),
                    _ => None,
                };
                if let (Some(b), Some(w)) = (b, walker.as_mut()) {
                    if let Some(field) = a.press(b, w) {
                        area_enter = Some(field);
                    }
                    continue;
                }
            }
            match key {
                KeyCode::Enter | KeyCode::NumpadEnter => next_line = true,
                // In free play Space (the pad's Start) opens the main menu; in events it pauses.
                KeyCode::Space if walker.is_some() && field_hud.is_some() => open_menu = true,
                KeyCode::Space => {
                    paused = !paused;
                    eprintln!("frame {frame}: {}", if paused { "paused" } else { "playing" });
                }
                KeyCode::Escape => quit = true,
                KeyCode::KeyR => orbit = (0.0, 0.0, 1.0),
                KeyCode::KeyF if battle_play.is_some() => {
                    let n = (FAST_FORWARD.load(std::sync::atomic::Ordering::Relaxed) + 1) % 3;
                    FAST_FORWARD.store(n, std::sync::atomic::Ordering::Relaxed);
                    eprintln!("frame {frame}: {}", battlehud::FAST_FORWARD_LABELS[usize::from(n)]);
                }
                KeyCode::KeyJ => jump_now = true,
                KeyCode::KeyK => strike_now = true,
                KeyCode::F5 => save_now = true,
                KeyCode::KeyG => gather_now = true,
                KeyCode::KeyC if walker.is_some() => open_mix = true,
                KeyCode::KeyB if walker.is_some() => open_shop = true,
                KeyCode::Enter | KeyCode::NumpadEnter if notice.is_some() => {
                    unsafe { gpu.device.device_wait_idle()? };
                    if let Some((layer, _)) = notice.take() {
                        layer.destroy(&gpu);
                    }
                }
                KeyCode::KeyT => {
                    if let Some(w) = walker.as_mut() {
                        w.run_on = !w.run_on;
                        eprintln!("run {}", if w.run_on { "on" } else { "off" });
                    } else if player.is_some() {
                        auto_lines = !auto_lines;
                        eprintln!("frame {frame}: auto {}", if auto_lines { "on" } else { "off" });
                    }
                }
                _ => {}
            }
        }
        if quit {
            eprintln!("quit after {frame} frames");
            stop = true;
            break;
        }
        if open_mix && let (Some(root), Some(pr)) = (hud_root.clone(), presenter.as_ref()) {
            unsafe { gpu.device.device_wait_idle()? };
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            cauldron(&gpu, renderer.pool, &target, Some((pr, &mut *p)), None, &root, session.as_mut(), &[], None, frame)?;
        }
        if open_shop && let (Some(root), Some(pr)) = (hud_root.clone(), presenter.as_ref()) {
            unsafe { gpu.device.device_wait_idle()? };
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            shop_visit(&gpu, renderer.pool, &target, Some((pr, &mut *p)), None, &root, session.as_mut(), &[], None, frame)?;
        }
        if open_menu && let (Some(root), Some(pr)) = (hud_root.as_ref(), presenter.as_ref()) {
            unsafe { gpu.device.device_wait_idle()? };
            let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
            let picked = menu::run(&gpu, renderer.pool, &target, Some((pr, &mut *p)), None, root, session.as_ref().is_some_and(|s| s.logy), session.as_ref().and_then(|s| s.party.get(usize::from(s.logy))), &basket_of(session.as_ref()), &card_of(root, session.as_ref()), &script("BARREL_MENU_KEYS"), menu_page.take(), None)?;
            eprintln!("frame {frame}: menu: {picked:?}");
            match picked {
                None => {
                    eprintln!("window closed in the menu after {frame} frames");
                    stop = true;
                    break;
                }
                Some(menu::Outcome::Option(menu::OptionRow::Save)) => save_now = true,
                // Back to the title, or to its Load Game list: the run starts again.
                Some(menu::Outcome::Option(o @ (menu::OptionRow::ReturnToTitle | menu::OptionRow::Load))) => {
                    restart = Some(title_args(&original_args, root, o == menu::OptionRow::Load));
                    stop = true;
                    break;
                }
                // Option → Options: the graphics page (not applied live yet: the run reports it).
                Some(menu::Outcome::Option(menu::OptionRow::Options)) => {
                    unsafe { gpu.device.device_wait_idle()? };
                    let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
                    let now = options::Graphics { render: settings, display: current_display(p) };
                    let kept = options::run(&gpu, renderer.pool, &target, Some((pr, &mut *p)), None, root, now, default_graphics(), &script("BARREL_OPTIONS_KEYS"), None)?;
                    eprintln!("frame {frame}: options: {kept:?}");
                    // Kept and changed: into the config. The window takes its new size or
                    // fullscreen at once; new render settings start the scene again where she
                    // stands (the renderer's renderer.targets and pipelines are built at a scene's start).
                    if let Some(new) = kept.filter(|k| *k != now) {
                        let path = write_config(&new)?;
                        eprintln!("frame {frame}: graphics saved to {}", path.display());
                        // A new window size or fullscreen: the window opens again with it at
                        // the scene's start (the compositor keeps a live window's size).
                        if new.display != now.display {
                            p.pending = Some(new.display);
                        }
                        if let (Some(s), Some(w)) = (session.as_ref(), walker.as_ref())
                            && let Some(text) = s.save([w.pos[0], w.pos[2]]) {
                                restart = Some(with_install_field(without_settings_flags(&plain_args(&original_args)), root));
                                carry_out = Carry { save: Some(text), ..Carry::default() };
                                stop = true;
                                break;
                            }
                    }
                }
                Some(menu::Outcome::Command(menu::Command::AdventureEquip | menu::Command::BattleEquip)) => {
                    unsafe { gpu.device.device_wait_idle()? };
                    let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
                    equip_visit(&gpu, renderer.pool, &target, Some((pr, &mut *p)), None, &root.clone(), session.as_mut(), &[], None, frame)?;
                }
                Some(_) => {}
            }
            // The menu's time is not play time.
            last_frame = std::time::Instant::now();
        }
        let axis = |neg: KeyCode, pos: KeyCode| (p.held(pos) as i32 - p.held(neg) as i32) as f32;
        let right_stick = pad::stick(p.sticks().1[0], p.sticks().1[1]);
        orbit.0 += (axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + right_stick[0]) * 1.5 * frame_dt;
        orbit.1 = (orbit.1 + (axis(KeyCode::ArrowDown, KeyCode::ArrowUp) + right_stick[1]) * 1.0 * frame_dt).clamp(-1.2, 1.2);
        let zoom_in = axis(KeyCode::Minus, KeyCode::Equal) + axis(KeyCode::NumpadSubtract, KeyCode::NumpadAdd);
        orbit.2 = (orbit.2 * (1.0 - zoom_in * 1.2 * frame_dt)).clamp(0.1, 10.0);
    }
    // The field's enemies: a step of their scripts while the stand-in player walks west at 2 m/s
    // (until one touches it: then all stop, as the field turns to the encounter), then each
    // body placed, turned and posed in its current motion.
    if let Some((field, bodies, names, time, player_at, stopped)) = creatures.as_mut() {
        let dt = if presenter.is_some() { if paused { 0.0 } else { frame_dt } } else { (1.0 / fps) as f32 };
        // With the player (`--player`) the enemies watch her, else a stand-in walking west.
        let (at, facing, strike_time) = match walker.as_ref() {
            Some(w) => (w.pos.map(|v| v as f64), w.facing as f64, w.striking.then(|| town_bodies.get(w.body).and_then(|b| b.clock)).flatten()),
            None => (*player_at, -std::f64::consts::FRAC_PI_2, None),
        };
        if frame > 0 && dt > 0.0 && !*stopped {
            if walker.is_none() {
                player_at[0] -= 200.0 * dt as f64;
            } else {
                *player_at = at;
            }
            // Her strike hits first, in its window (0.35 to 0.6 s into the swing,
            // `0x14026e2c0`): the nearest enemy within reach of a point ahead of her
            // (`encounter::struck`, stage 0, Escha's radius 130).
            use barrel_escha::encounter::{STRIKE_RADIUS_ESCHA, STRIKE_RADIUS_LOGY, STRIKE_WINDOW};
            let radius = if walker.as_ref().is_some_and(|w| w.logy) { STRIKE_RADIUS_LOGY[0] } else { STRIKE_RADIUS_ESCHA[0] };
            let struck = strike_time.filter(|&t| (STRIKE_WINDOW.0..=STRIKE_WINDOW.1).contains(&(t as f64))).and_then(|_| barrel_escha::encounter::struck(at, facing, 0, radius, &field.placed()));
            let met = if let Some(k) = struck {
                eprintln!("frame {frame}: her strike hits enemy {k}: encounter group {} (begun by a strike)", field.groups[k]);
                Some(k)
            } else if let Some(t) = field.step(at, facing, dt as f64).map_err(|e| format!("creatures: {e}"))? {
                eprintln!("frame {frame}: enemy {} touches the player: encounter group {}", t.enemy, t.group);
                Some(t.enemy)
            } else {
                None
            };
            // The encounter is fought out (no battle screen yet: `auto_battle`) and the field
            // goes on, the enemy gone; without the battle tables the field stops there.
            // In the story's free play the encounter opens its battle: the run starts again on
            // the area's battle stage, and comes back here when it ends.
            let to_battle = met.and_then(|k| {
                let s = session.as_ref()?;
                let w = walker.as_ref()?;
                let area = creatures_area?;
                let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir())?;
                let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).ok()?;
                let pe = barrel_formats::pe::Pe::parse(&exe).ok()?;
                let areas = barrel_escha::data::gimmick::areas(&pe, barrel_escha::data::gimmick::MAPS_EN, barrel_escha::data::gimmick::MAP_COUNT)?;
                let stage = areas.get(area as usize)?.words[5];
                let stage_dir = root.join("Data/x64/Field").join(format!("BF_POINT{stage:02}"));
                if stage < 0 || !stage_dir.is_dir() {
                    return None;
                }
                let save = s.save([w.pos[0], w.pos[2]])?;
                let look = Path::new(&args[1]).file_name()?.to_string_lossy().into_owned();
                let mut battle = plain_args(&original_args);
                if battle.len() > 1 {
                    battle[1] = stage_dir.to_string_lossy().into_owned();
                }
                battle.extend(["--look-from".into(), look, "--battle".into(), field.groups[k].to_string(), "--hero".into(), if w.logy { "logy" } else { "escha" }.into()]);
                let mut defeated: Vec<usize> = (0..field.defeated.len()).filter(|&j| field.defeated[j]).collect();
                defeated.push(k);
                eprintln!("frame {frame}: to battle stage {stage} with encounter group {}", field.groups[k]);
                // The way back: this run's arguments less what picked its start (a title, a save
                // file: the carried save says where), in a field of the install.
                let back = with_install_field(plain_args(&original_args), root);
                Some((battle, Carry { back: Some(Box::new((back, Carry { save: Some(save), defeated, back: None, notice: None, to_area_map: false }))), ..Carry::default() }))
            });
            if let Some((battle, c)) = to_battle {
                restart = Some(battle);
                carry_out = c;
                stop = true;
                break;
            }
            if let Some(k) = met {
                match battles.as_mut() {
                    Some((tables, rng)) => {
                        let logy = walker.as_ref().is_some_and(|w| w.logy);
                        // The session's party, at its levels and in its equipment (level 1 and
                        // the new game's kit without a session).
                        let members = session.as_ref().map(|s| s.party.clone()).unwrap_or_default();
                        for (c, m) in members.iter().enumerate() {
                            tables.equip(c, &m.equipment);
        tables.carry(c, &m.battle_items);
                        }
                        let level = |c: usize| members.get(c).map_or(1, |m| m.level.max(1));
                        let party = [(usize::from(logy), level(usize::from(logy))), (usize::from(!logy), level(usize::from(!logy)))];
                        match barrel_escha::auto_battle::fight(tables, &party, field.groups[k], rng) {
                            Ok(o) => {
                                for line in o.lines() {
                                    eprintln!("battle: {line}");
                                }
                                // The spoils: money in, experience for the members who fought.
                                let (exp, money) = tables.spoils(&o);
                                if let Some(s) = session.as_mut() {
                                    for (slot, &(c, _)) in party.iter().enumerate() {
                                        s.use_items(c, &tables.uses_left(c, o.items_left.get(slot).map_or(&[][..], Vec::as_slice)));
                                    }
                                }
                                if let Some(s) = session.as_mut().filter(|_| o.winner == Some(0)) {
                                    s.won(&[party[0].0, party[1].0], exp, money, &tables.exp);
                                    s.found(&o.drops);
                                    eprintln!("battle: {exp} experience, {money} money (now {}), drops {:?}", s.money, o.drops);
                                }
                            }
                            Err(e) => eprintln!("battle: {e}"),
                        }
                        field.defeated[k] = true;
                    }
                    None => *stopped = true,
                }
            }
            *time += dt;
        }
        for b in bodies.iter_mut() {
            // A beaten enemy is gone: its body collapsed, as the stand-in's below.
            if field.defeated.get(b.mob).copied().unwrap_or(false) {
                instances[b.instance as usize].model = Mat4::from_scale(Vec3::ZERO);
                let base = instances[b.instance as usize].palette as usize;
                for k in 0..b.skeleton.joints.len() {
                    if let Some(slot) = palette.get_mut(base + k) {
                        *slot = Mat4::from_scale(Vec3::ZERO);
                    }
                }
                continue;
            }
            let (position, facing, motion) = match field.mobs.get(b.mob) {
                Some(m) => (m.creature.position, m.creature.facing, m.creature.motion.and_then(|m| names.get(m as usize)).cloned()),
                None if walker.is_some() => {
                    // The stand-in is not needed with the player there: hidden (its model and its
                    // joints both collapsed, as a skinned body draws through its joints).
                    instances[b.instance as usize].model = Mat4::from_scale(Vec3::ZERO);
                    let base = instances[b.instance as usize].palette as usize;
                    for k in 0..b.skeleton.joints.len() {
                        if let Some(slot) = palette.get_mut(base + k) {
                            *slot = Mat4::from_scale(Vec3::ZERO);
                        }
                    }
                    continue;
                }
                None => (*player_at, -std::f64::consts::FRAC_PI_2, Some(if *stopped { "F_WAIT" } else { "WALK" }.to_string())),
            };
            if let Some(column) = motion
                && column != b.column {
                    if let Some(anim) = b.motions.get(&column).cloned() {
                        b.anim = anim;
                        b.since = *time;
                    }
                    b.column = column;
                }
            let turn = glam::Quat::from_rotation_y(facing as f32);
            instances[b.instance as usize].model = render_space(Mat4::from_rotation_translation(turn, Vec3::from(position.map(|v| v as f32))));
            let joints = pose(&b.skeleton, &b.anim, motion_clock(&b.anim, *time - b.since, true));
            let base = instances[b.instance as usize].palette as usize;
            for (k, m) in joints.iter().enumerate() {
                if let Some(slot) = palette.get_mut(base + k) {
                    *slot = render_space(*m);
                }
            }
        }
        if player.is_none() {
            scene.write_instances(&gpu, &instances)?;
            scene.write_palette(&gpu, &palette, &prev_palette)?;
            // The camera follows the player with the field's play-camera setting for where the
            // player is (`barrel_engine::play_camera`): its distance, yaw, pitch, field of view and
            // look offset, the eye built as the field camera builds it (in game space, then x
            // mirrored like the vertices).
            let look_table = field_looks.first().and_then(|l| l.as_ref());
            let point = look_table
                .and_then(|l| barrel_engine::play_camera::setting(&l.camera_areas, &l.camera_points, player_at.map(|v| v as f32 / 100.0)))
                .or_else(|| look_table.and_then(|l| l.camera_points.first().cloned()))
                .unwrap_or_default();
            let mirror = Vec3::new(-1.0, 1.0, 1.0);
            let look = Vec3::from(player_at.map(|v| v as f32)) + Vec3::from(point.diff) * 100.0;
            let (y, p) = (point.yaw.to_radians(), point.pitch.to_radians());
            dist = point.length.max(1.0) * 100.0;
            target = look * mirror;
            eye = (look + Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist) * mirror;
            fov = point.fov.clamp(10.0, 120.0).to_radians();
        }
    }
    if frame > 0 {
        sway_dt = if presenter.is_some() { if paused { 0.0 } else { frame_dt } } else { (1.0 / fps) as f32 };
        pattern_time += sway_dt;
    }
    // The player walking: a step against the field's walls and onto its ground, turned the way
    // she goes, the camera carried along. In a window W A S D steer her (relative to the
    // camera) at the game's speed (`game::walk::player_move`); renderer.offscreen `--walk` does.
    if area_mode && area_map.is_none()
        && let Some(w) = walker.as_ref() {
            match AreaMap::new(Path::new(&args[1]), w.pos, world_mode, &gpu, renderer.pool, renderer.post_pass) {
                Ok(mut a) => {
                    eprintln!("area map: {} ({})", a.places[a.at].name, a.places[a.at].tag);
                    // Where she stands is visited; the sectors closed or not known yet are hidden.
                    if let Some(s) = session.as_mut() {
                        s.visit(a.at);
                    }
                    a.closed = match session.as_ref() {
                        Some(s) => {
                            let known = s.known(&a.places);
                            (0..a.places.len()).map(|k| s.closed(&a.places, k) || (a.places[k].kind == 1 && !known[k])).collect()
                        }
                        None => Vec::new(),
                    };
                    if let Some(s) = session.as_ref() {
                        a.visited = (0..a.places.len()).map(|k| s.visited(k)).collect();
                        let known = s.known_routes(&a.places);
                        let area_routes: std::collections::BTreeSet<i32> = a.places.iter().filter(|p| p.kind == 1).flat_map(|p| p.links).filter(|&r| r >= 0).collect();
                        a.hidden_routes = area_routes.difference(&known).filter_map(|&r| a.routes.get(r as usize).cloned()).collect();
                        let walked = s.walked_routes(&a.places);
                        // The routes between the regions (the world level's places' links) show
                        // once walked, both ends visited: the world's places open with the story,
                        // not by lying next to one visited (the user's save at 8/20 knows only
                        // R0, R1 and R2, all walked, and none of Colseit's other routes).
                        // A route's ends are the route table's (`T0` - `A1p0` for R2), not the
                        // places whose links hold it (R2 is in the Old Highway region's links).
                        let world_routes: std::collections::BTreeSet<i32> = a.places.iter().filter(|p| p.kind == 0).flat_map(|p| p.links).filter(|&r| r >= 0).collect();
                        let visited_tag = |tag: &str| a.places.iter().position(|p| p.tag == tag).is_some_and(|k| s.visited(k));
                        let world_walked: std::collections::BTreeSet<String> = a.trips.as_ref().map_or_else(Default::default, |m| {
                            m.routes.iter().filter(|r| visited_tag(&r.ends.0) && visited_tag(&r.ends.1)).map(|r| r.area.id.clone()).collect()
                        });
                        a.hidden_routes.extend(world_routes.iter().filter_map(|&r| a.routes.get(r as usize)).filter(|tag| !world_walked.contains(*tag)).cloned());
                        a.seen_routes = known.difference(&walked).filter(|r| !world_routes.contains(r)).filter_map(|&r| a.routes.get(r as usize).cloned()).collect();
                    }
                    area_map = Some(a);
                }
                Err(e) => {
                    eprintln!("no area map: {e}");
                    area_mode = false;
                }
            }
        }
    // Offscreen, a key a frame once she stands still.
    if presenter.is_none()
        && let (Some(a), Some(w)) = (area_map.as_mut(), walker.as_mut())
        && w.waypoints.is_empty()
        && let Some(b) = area_keys.pop_front()
        && let Some(field) = a.press(b, w) {
            area_enter = Some(field);
        }
    if let Some(a) = area_map.as_mut() {
        // The neighbours' labels stand while she stands on a point.
        let still = walker.as_ref().is_some_and(|w| w.waypoints.is_empty());
        let mvp = still.then(|| view_proj(eye, target, Vec3::Y, dist, fov, width, height, orbit));
        let seen = session.as_ref().map(|s| s.seen()).unwrap_or_default();
        let today = session.as_ref().map_or(0, |s| s.today().max(0) as u32);
        let trip = a.trip_days(|e| seen.get(e as usize).copied().unwrap_or(false)).map(|d| {
            let v = hud::Values::on(today + d.max(0) as u32, 0);
            (d, v.month, v.day)
        });
        a.panel(&gpu, width, height, mvp, trip, |i| session.as_ref().is_some_and(|s| s.knows(i)), session.as_ref().map(|s| s.levels()).unwrap_or_default(), &session.as_ref().map(|s| s.seen()).unwrap_or_default())?;
    }
    if area_enter.is_some() {
        stop = true;
        break;
    }
    if let Some(w) = walker.as_mut() {
        // A trip's waypoints: run straight to the next (within a step of it, on to the one after).
        while let Some(next) = w.waypoints.front().copied() {
            let (dx, dz) = (next[0] - w.pos[0], next[2] - w.pos[2]);
            let d = (dx * dx + dz * dz).sqrt();
            if d < 30.0 {
                w.waypoints.pop_front();
                if w.waypoints.is_empty() {
                    w.velocity = [0.0, 0.0];
                }
                continue;
            }
            // No faster than reaches it this frame (at a low frame rate a full step would
            // overshoot it and back, never within reach).
            let step_dt = (1.0 / fps) as f32;
            let speed = barrel_escha::player::player_run_speed(false).min(d / step_dt.max(1e-3));
            w.velocity = [dx / d * speed, dz / d * speed];
            break;
        }
        if let Some(p) = pump.as_ref().filter(|_| w.waypoints.is_empty() && area_map.is_none()) {
            use winit::keyboard::KeyCode;
            let axis = |neg: KeyCode, pos: KeyCode| (p.held(pos) as i32 - p.held(neg) as i32) as f32;
            let (mut ahead, mut right) = (axis(KeyCode::KeyS, KeyCode::KeyW), axis(KeyCode::KeyA, KeyCode::KeyD));
            // The pad's left stick, leaned partway, walks partway (the game's own dead zone in
            // `player_move`); keys lean it all the way.
            let (stick, _) = p.sticks();
            let tilt = (stick[0] * stick[0] + stick[1] * stick[1]).sqrt().min(1.0);
            let lean = if tilt > 0.05 {
                (right, ahead) = (stick[0], stick[1]);
                tilt
            } else {
                1.0
            };
            let e = turned_eye(eye, target, orbit);
            let f = Vec3::new(target.x - e.x, 0.0, target.z - e.z).normalize_or_zero();
            // Right on screen: the projection mirrors clip-space x (`view_proj`), so it is the
            // right-handed view's left, (f.z, 0, -f.x); game x is render's mirrored.
            // The speed a second is the move over one second.
            let v = (f * ahead + Vec3::new(f.z, 0.0, -f.x) * right).normalize_or_zero();
            w.velocity = barrel_escha::player::player_move([-v.x * lean, v.z * lean], 1.0, false, w.run_on);
        }
        let moving = w.velocity != [0.0, 0.0];
        let dt = if presenter.is_some() { if paused { 0.0 } else { frame_dt } } else { (1.0 / fps) as f32 };
        let mut speed = 0.0;
        if moving && frame > 0 && dt > 0.0 {
            let delta = [w.velocity[0] * dt, w.velocity[1] * dt];
            // On the area and world maps she runs the route's dots freely (the marks' own
            // collision would hold her short of them).
            let next = if area_map.is_some() { [w.pos[0] + delta[0], w.pos[2] + delta[1]] } else { w.field.walls.step([w.pos[0], w.pos[2]], delta, w.pos[1], 150.0, 48.0) };
            if let Some(y) = w.field.ground.height(next[0], next[1], w.pos[1], 50.0) {
                let moved = Vec3::new(next[0] - w.pos[0], y - w.pos[1], next[1] - w.pos[2]) * Vec3::new(-1.0, 1.0, 1.0);
                speed = Vec3::new(moved.x, 0.0, moved.z).length() / dt;
                w.pos = [next[0], y, next[1]];
                eye += moved;
                target += moved;
                // A door she steps into: the scene is rebuilt in the map it leads to.
                let now = barrel_escha::data::gimmick::inside(&w.gimmicks, w.pos);
                let entered = now.iter().find(|i| !w.inside.contains(i)).and_then(|&i| w.gimmicks[i].destination);
                // A world map exit: the trip is taken after the frame (`--travel-to`).
                if now.iter().any(|&i| !w.inside.contains(&i) && w.gimmicks[i].kind == barrel_escha::data::gimmick::WORLDMAP_CHANGE_EX) {
                    eprintln!("frame {frame}: onto the world map");
                    world_exit = true;
                }
                w.inside = now;
                if let Some(to) = entered
                    && let Some(name) = w.maps.get(to.map as usize) {
                        eprintln!("frame {frame}: through the door to {name} at {:?}", to.position);
                        door = Some((name.clone(), [to.position[0] * 100.0, to.position[2] * 100.0], w.velocity));
                    }
            }
        }
        // A jump (`docs/engine.md`, "Walking"): her `JUMP` motion at 1.5, blended in, through its
        // divisions (`CharaDiv.xml`: 0 the take-off to 0.1 s, 1 and 2 the rise and the fall, 3,
        // 1.1 to 1.5 s, looping while she is still up, 4 from 1.5 s the landing). The launch (5 m/s,
        // `game::walk::jump_height`) comes as the take-off ends; touching the ground again starts
        // the landing, and her gait takes over when the motion ends.
        if jump_now && w.jump.is_none()
            && let (Some(anim), Some(b)) = (w.jump_anim.clone(), town_bodies.get_mut(w.body)) {
                b.switch(anim);
                w.jump = Some(-1.0);
                // The jump's sound (0x23 at 0.6) and, now and then, a voice (Escha's).
                let voice = w.jump_voice.jump(u32::from(w.logy), &mut w.rng);
                eprintln!("frame {frame}: jump{}", voice.map_or(String::new(), |c| format!(", voice {c}")));
                if sound.is_none() && presenter.is_some() {
                    match barrel_audio::Output::open(0.1) {
                        Ok(out) => sound = Some((Sink::Device(out), barrel_audio::Mixer::game(), barrel_audio::Library::new(&w.root, "Voice_EN", Vec::new()), None)),
                        Err(e) => eprintln!("no sound: {e}"),
                    }
                }
                if let Some((_, mixer, library, _)) = sound.as_mut() {
                    if let Some(barrel_audio::Effect::Clip(e)) = library.effect(barrel_escha::player::JUMP_SOUND) {
                        mixer.play(e, 0.6, None);
                    }
                    if let Some(v) = voice.and_then(|c| library.voice(c)) {
                        mixer.play(v, barrel_audio::VOICE_GAIN, None);
                    }
                }
            }
        // G (or `BARREL_GATHER_AT`): a gathering at the nearest point of this map within its
        // reach (its size, 2 m in most, from its centre on the ground), into the session's
        // basket with its field time (`gather::Gathering`, `Session::gathered`).
        if gather_now {
            if gathering.is_none() {
                gathering = Some(barrel_escha::gather::Gathering::load(&w.root).map_err(|e| format!("gathering: {e}"))?);
            }
            let g = gathering.as_ref().unwrap();
            let map = w.maps.iter().position(|m| *m == w.field.name).map_or(-1, |m| m as i32);
            let near = g
                .points_in(map)
                .map(|(k, p)| (k, ((p.position[0] * 100.0 - w.pos[0]).powi(2) + (p.position[2] * 100.0 - w.pos[2]).powi(2)).sqrt(), p.size[0].max(p.size[2]) * 100.0))
                .filter(|&(_, d, reach)| d <= reach)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match near {
                Some((k, d, _)) => {
                    let got = g.gather(k, session.as_ref().map(|s| s.levels()).unwrap_or_default(), |_| 0, 1, &mut w.rng);
                    eprintln!("frame {frame}: gathering at point {k} ({d:.0} away): items {:?}", got.iter().map(|g| (g.item, g.quality)).collect::<Vec<_>>());
                    if let Some(s) = session.as_mut() {
                        s.gathered(got);
                        eprintln!("  basket {} items, day {}", s.basket.len(), s.today());
                    }
                }
                None => eprintln!("frame {frame}: no gathering point within reach"),
            }
        }
        // F5 (or `BARREL_SAVE_AT`): the session saved where she stands (`--save FILE`, else the
        // next free slot of the saves folder).
        if save_now {
            let next_slot = || {
                let dir = saves_dir.clone().or_else(barrel_escha::saves::default_dir)?;
                std::fs::create_dir_all(&dir).ok()?;
                let n = barrel_escha::saves::list(&dir).iter().map(|s| s.number).max().unwrap_or(0) + 1;
                Some(barrel_escha::saves::slot_path(&dir, n))
            };
            let file = save_file.clone().or_else(next_slot);
            match (session.as_ref().and_then(|s| s.save([w.pos[0], w.pos[2]])), &file) {
                (Some(text), Some(file)) => match std::fs::write(file, text) {
                    Ok(()) => eprintln!("frame {frame}: saved to {}", file.display()),
                    Err(e) => eprintln!("frame {frame}: saving to {} failed: {e}", file.display()),
                },
                (None, _) => eprintln!("frame {frame}: nothing to save (no story in free play)"),
                (_, None) => eprintln!("frame {frame}: no --save FILE or saves folder to save to"),
            }
        }
        // A strike: the swing's motion once through, blended in, then her gait again.
        if strike_now && !w.striking && w.jump.is_none()
            && let (Some(anim), Some(b)) = (w.strike_anim.clone(), town_bodies.get_mut(w.body)) {
                b.switch(anim);
                w.striking = true;
                eprintln!("frame {frame}: strike");
            }
        if w.striking
            && let Some(b) = town_bodies.get_mut(w.body) {
                let before = b.clock.unwrap_or(0.0);
                let clock = before + dt;
                b.clock = Some(clock);
                // The macro's sound at full volume: Escha's `SE_WAND01` (98) after division 0
                // (to 0.1 s) and waits of 0.05 and 0.30 s, so 0.45 s in; Logy's `SE_WAND02` (99)
                // 0.10 s into division 1 (from 0.21 s), so 0.31 s in.
                let swing_sound: (f32, u32) = if w.logy { (0.31, 99) } else { (0.45, 98) };
                if before < swing_sound.0 && clock >= swing_sound.0 {
                    if sound.is_none() && presenter.is_some()
                        && let Ok(out) = barrel_audio::Output::open(0.1) {
                            sound = Some((Sink::Device(out), barrel_audio::Mixer::game(), barrel_audio::Library::new(&w.root, "Voice_EN", Vec::new()), None));
                        }
                    if let Some((_, mixer, library, _)) = sound.as_mut()
                        && let Some(barrel_audio::Effect::Clip(e)) = library.effect(swing_sound.1) {
                            mixer.play(e, 1.0, None);
                        }
                }
                if let Some((_, c, blend)) = &mut b.from {
                    *c += dt;
                    *blend += dt;
                }
                if b.from.as_ref().is_some_and(|f| f.2 >= MOTION_BLEND) {
                    b.from = None;
                }
                // Escha's swing runs to the motion's end; Logy's to division 3's end, 0.9 s.
                let end = if w.logy { 0.9 } else { w.strike_anim.as_ref().map_or(0.0, |a| a.duration) };
                if clock >= end {
                    w.striking = false;
                    w.gait = barrel_escha::player::Gait::Jump;
                }
            }
        let mut lift = 0.0;
        if let Some(t) = w.jump {
            const TAKE_OFF: f32 = 0.1;
            const LOOP: (f32, f32) = (1.1, 1.5);
            let duration = w.jump_anim.as_ref().map_or(0.0, |a| a.duration);
            if let Some(b) = town_bodies.get_mut(w.body) {
                let mut clock = b.clock.unwrap_or(0.0) + dt * 1.5;
                let mut t = t;
                if t < 0.0 {
                    // The take-off: launched once its division has played.
                    if clock >= TAKE_OFF {
                        t = 0.0;
                    }
                } else {
                    t += dt;
                    let h = barrel_escha::player::jump_height(barrel_escha::player::JUMP_SPEED, t);
                    if h > 0.0 {
                        lift = h;
                        // Still up past the fall: division 3 loops.
                        if clock >= LOOP.1 {
                            clock = LOOP.0 + (clock - LOOP.0) % (LOOP.1 - LOOP.0);
                        }
                    } else if t > 0.0 && clock < LOOP.1 {
                        // Down: the landing division, and the ground's landing sound.
                        clock = LOOP.1;
                        t = f32::INFINITY;
                        eprintln!("frame {frame}: lands{}", w.landing.map_or(String::new(), |s| format!(", sound {s}")));
                        if let Some((_, mixer, library, _)) = sound.as_mut()
                            && let Some(barrel_audio::Effect::Clip(e)) = w.landing.and_then(|s| library.effect(s)) {
                                mixer.play(e, barrel_escha::player::LANDING_VOLUME, None);
                            }
                    }
                }
                b.clock = Some(clock);
                if let Some((_, c, blend)) = &mut b.from {
                    *c += dt;
                    *blend += dt;
                }
                if b.from.as_ref().is_some_and(|f| f.2 >= MOTION_BLEND) {
                    b.from = None;
                }
                w.jump = Some(t);
                if t == f32::INFINITY && clock >= duration {
                    w.jump = None;
                    // The gait picks her motion again from the next frame (blended from here).
                    w.gait = barrel_escha::player::Gait::Jump;
                }
            }
        }
        // Her motion from how fast she went (`game::walk::player_gait`): a new one starts from
        // its beginning and is blended in over 0.15 s, the old one playing on meanwhile.
        // On the world map, while the cursor is away, she waits where she started.
        let waiting = w.cursor.is_some_and(|(_, home, _)| world_mode && (w.pos[0] - home[0]).hypot(w.pos[2] - home[2]) > CURSOR_LEAVES);
        if frame > 0 && dt > 0.0 && w.jump.is_none() && !w.striking {
            let (gait, rate) = if waiting { (barrel_escha::player::Gait::Wait, 1.0) } else { barrel_escha::player::player_gait(speed, false) };
            if let Some(b) = town_bodies.get_mut(w.body) {
                if gait != w.gait {
                    w.gait = gait;
                    let next = match gait {
                        barrel_escha::player::Gait::Wait => w.wait.clone(),
                        barrel_escha::player::Gait::Walk => w.walk.clone(),
                        barrel_escha::player::Gait::Run | barrel_escha::player::Gait::Jump => w.run.clone(),
                    };
                    b.switch(next);
                }
                b.clock = Some(b.clock.unwrap_or(0.0) + dt * rate);
                if let Some((_, clock, blend)) = &mut b.from {
                    *clock += dt * rate;
                    *blend += dt;
                }
                if b.from.as_ref().is_some_and(|f| f.2 >= MOTION_BLEND) {
                    b.from = None;
                }
            }
        }
        if moving {
            w.facing = w.velocity[0].atan2(w.velocity[1]);
        }
        // The field's play camera on her (as the creatures' mode uses it, `play_camera`): the
        // setting for where she stands, unless `--free-camera` (then it is carried along).
        if use_event_camera {
            let look_table = field_looks.first().and_then(|l| l.as_ref());
            let point = look_table
                .and_then(|l| barrel_engine::play_camera::setting(&l.camera_areas, &l.camera_points, w.pos.map(|v| v / 100.0)))
                .or_else(|| look_table.and_then(|l| l.camera_points.first().cloned()));
            if let Some(point) = point {
                let mirror = Vec3::new(-1.0, 1.0, 1.0);
                // It looks at her and pulls back while she moves (`play_camera::Follow`).
                let dt = if presenter.is_some() { if paused { 0.0 } else { frame_dt } } else { (1.0 / fps) as f32 };
                let (at, extra) = w.follow.step(w.pos, moving, &point, dt);
                let look = Vec3::from(at) + Vec3::from(point.diff) * 100.0;
                let (y, p) = (point.yaw.to_radians(), point.pitch.to_radians());
                dist = point.length.max(1.0) * 100.0 + extra;
                target = look * mirror;
                eye = (look + Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos()) * dist) * mirror;
                fov = point.fov.clamp(10.0, 120.0).to_radians();
            }
        }
        let turn = glam::Quat::from_rotation_y(w.facing);
        let size = match (area_mode, world_mode) {
            (true, true) => WORLD_MAP_SCALE,
            (true, false) => AREA_MAP_SCALE,
            _ => 1.0,
        };
        // On the world map, away from her start, the cursor moves and she waits there.
        let away = w.cursor.filter(|&(_, home, _)| world_mode && (w.pos[0] - home[0]).hypot(w.pos[2] - home[2]) > CURSOR_LEAVES);
        let (at, her_turn) = away.map_or((w.pos, turn), |(_, home, facing)| (home, glam::Quat::from_rotation_y(facing)));
        instances[w.instance as usize].model = render_space(Mat4::from_scale_rotation_translation(Vec3::splat(size), her_turn, Vec3::from(at) + Vec3::Y * lift));
        if let Some((cursor, ..)) = w.cursor {
            instances[cursor as usize].model = match away {
                Some(_) => render_space(Mat4::from_scale_rotation_translation(Vec3::ONE, turn, Vec3::from(w.pos) + Vec3::Y * lift)),
                None => Mat4::from_scale(Vec3::ZERO),
            };
        }
        if player.is_none()
            && let Some((out, mixer, ..)) = sound.as_mut() {
                out.pump(mixer, 0.0);
            }
    }
    if door.is_some() || world_exit {
        break;
    }
    if let Some(c) = cpu.as_mut() {
        c.mark("input + simulation");
    }
    // Town people: each posed in its wait motion on the viewer's clock.
    if !town_bodies.is_empty() {
        let dt = if presenter.is_some() { if paused { 0.0 } else { frame_dt } } else { (1.0 / fps) as f32 };
        if frame > 0 {
            town_time += dt;
            if let Some(play) = battle_play.as_mut() {
                let speed = battlehud::FAST_FORWARD_SPEEDS[usize::from(FAST_FORWARD.load(std::sync::atomic::Ordering::Relaxed)).min(2)];
                play.step(&mut town_bodies, dt * speed);
                // Offscreen: `BARREL_BATTLE_KEYS` (as `BARREL_MENU_KEYS`), a key a frame while a
                // member's turn waits.
                if presenter.is_none() && play.waiting()
                    && let Some(b) = battle_keys.pop_front() {
                        play.press(b);
                    }
                // Over, however it ended: the battle items' uses left go into the field's save.
                if play.over() && !play.items_noted
                    && let Some(save) = carry.back.as_mut().and_then(|b| b.1.save.as_mut()) {
                        for (slot, &c) in play.charas.iter().enumerate() {
                            let uses = play.fight.tables.uses_left(c, play.outcome.items_left.get(slot).map_or(&[][..], Vec::as_slice));
                            *save = barrel_escha::session::Session::spent_in_save(save, c, &uses);
                        }
                        play.items_noted = true;
                    }
                // Over and won: the spoils go into the field's save (money, the fighters'
                // experience, the drops) and the Result screen comes up with the party's
                // levels before and after.
                let won = play.outcome.winner == Some(0);
                if play.over() && won && play.result.is_none()
                    && let Some(save) = carry.back.as_mut().and_then(|b| b.1.save.as_mut()) {
                        let (exp, money) = play.fight.tables.spoils(&play.outcome);
                        let before = barrel_escha::session::Session::party_in(save);
                        *save = barrel_escha::session::Session::won_in_save(save, &play.charas, exp, money, &play.outcome.drops, &play.fight.tables.exp);
                        let after = barrel_escha::session::Session::party_in(save);
                        eprintln!("battle: {exp} experience, {money} money, drops {:?}", play.outcome.drops);
                        let rows = play.charas.iter().filter_map(|&c| {
                            let m = after.get(c)?;
                            let next = barrel_escha::growth::to_next(&play.fight.tables.exp, m.level, m.experience, 0).unwrap_or(0);
                            Some(result::Row { chara: c, level: m.level, next, level_up: before.get(c).is_some_and(|b| m.level > b.level) })
                        }).collect();
                        play.result = Some((result::Values { rows, exp, money }, play.time));
                    }
                // The Result screen's victory music, in place of the battle's (`battle::victory_music`).
                if play.result.is_some() && !play.victory_played {
                    play.victory_played = true;
                    let track = barrel_escha::battle::victory_music(play.charas.first() == Some(&1));
                    eprintln!("victory music {track}");
                    if let Some((_, mixer, library, music)) = sound.as_mut() {
                        if let Some(old) = music.take() {
                            mixer.stop(old);
                        }
                        *music = library.music(track).map(|m| mixer.play_stream(m.stream, m.volume, m.loop_from));
                    }
                }
                // Over and lost: a day passes in the field's save, and the lead's line from the
                // game's wipe (not yet shown on screen: the console).
                // Run away: no spoils and no defeat, straight back to the field.
                if play.over() && !won && !play.lost_noted && !play.outcome.escaped()
                    && let Some(save) = carry.back.as_mut().and_then(|b| b.1.save.as_mut()) {
                        *save = barrel_escha::session::Session::lost_in_save(save);
                        play.lost_noted = true;
                        let logy = save.lines().any(|l| l == "lead logy");
                        let today = save.lines().find_map(|l| l.strip_prefix("today ")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                        play.wiped = Some((play.time, today));
                        if let Some(back) = carry.back.as_mut() {
                            back.1.notice = hud_root.as_ref().and_then(|root| wipe_line(root, logy));
                            // The game brings the party round on the area map of the battle's area.
                            back.1.to_area_map = true;
                        }
                        eprintln!("battle lost: a day passes; \"Oww... that was bad. Let's be more careful next time.\"");
                    }
                // Then back to the field it came from: a lost battle at once, a won one once its
                // Result is closed (confirm; renderer.offscreen after `RESULT_SECONDS`).
                let closed = play.result.as_ref().is_some_and(|(_, at)| play.result_closed || (presenter.is_none() && play.time - at >= RESULT_SECONDS));
                // A won battle's drops on their own screen once the Result is closed
                // (offscreen: `BARREL_DROPS_KEYS`, drawn to OUT-drops.png).
                if closed && !play.outcome.drops.is_empty() && !play.drops_shown
                    && let (Some(root), Some(save)) = (hud_root.clone(), carry.back.as_ref().and_then(|b| b.1.save.clone())) {
                        play.drops_shown = true;
                        let drops: Vec<(i32, f32)> = play.outcome.drops.iter().flat_map(|&(item, count, quality)| std::iter::repeat_n((item, quality), count.max(0) as usize)).collect();
                        let held = |item: i32| save.lines().filter(|l| l.split_whitespace().nth(1).and_then(|v| v.parse::<i32>().ok()) == Some(item) && l.starts_with("item ")).count() as i32;
                        let possessed: Vec<i32> = drops.iter().map(|&(item, _)| held(item)).collect();
                        unsafe { gpu.device.device_wait_idle()? };
                        let target = screen::Target { pass: renderer.post_pass, framebuffers: &renderer.targets.framebuffers, width, height };
                        match (presenter.as_ref(), pump.as_mut(), renderer.offscreen.as_ref()) {
                            (Some(pr), Some(p), _) => drops::run(&gpu, renderer.pool, &target, Some((pr, &mut *p)), None, &root, &drops, &possessed, &[], None)?,
                            (None, _, Some((color, _))) => {
                                let keys = title::keys(&std::env::var("BARREL_DROPS_KEYS").unwrap_or_default());
                                let shot = Path::new(&out_path).with_extension("").to_string_lossy().into_owned() + "-drops.png";
                                drops::run(&gpu, renderer.pool, &target, None, Some(*color), &root, &drops, &possessed, &keys, Some(Path::new(&shot)))?;
                            }
                            _ => {}
                        }
                    }
                let wiped = play.wiped.is_none_or(|(at, _)| play.time - at >= WIPE_SECONDS);
                if play.over() && (!won && wiped || closed || won && play.result.is_none() && carry.back.as_ref().and_then(|b| b.1.save.as_ref()).is_none())
                    && let Some(back) = carry.back.take() {
                        eprintln!("frame {frame}: the battle is over, back to the field");
                        (restart, carry_out) = (Some(back.0), back.1);
                        stop = true;
                        break;
                    }
            }
        }
        for b in &mut town_bodies {
            let now = local_pose(&b.skeleton, &b.anim, motion_clock(&b.anim, b.clock.unwrap_or(town_time), true));
            let locals = match (&b.from, &b.held) {
                (Some((_, _, blend)), Some(held)) if held.len() == now.len() => mix_pose(held, &now, (blend / MOTION_BLEND).clamp(0.0, 1.0)),
                (Some((old, clock, blend)), _) => mix_pose(&local_pose(&b.skeleton, old, motion_clock(old, *clock, true)), &now, (blend / MOTION_BLEND).clamp(0.0, 1.0)),
                (None, _) => {
                    b.held = None;
                    now
                }
            };
            b.last_locals.clone_from(&locals);
            let joints = if b.hidden { vec![Mat4::from_scale(Vec3::ZERO); b.skeleton.joints.len()] } else { skin(&b.skeleton, &locals) };
            let base = instances[b.instance as usize].palette as usize;
            for (k, m) in joints.iter().enumerate() {
                if let Some(slot) = palette.get_mut(base + k) {
                    *slot = render_space(*m);
                }
            }
        }
        if player.is_none() {
            scene.write_palette(&gpu, &palette, &prev_palette)?;
        }
    }
    // The view's up: the event camera's roll turns it (t10).
    let mut up = Vec3::Y;
    if let Some((play, clock, before, after)) = player.as_mut()
        && frame > 0 {
            // A window takes the real time since the last frame; files take 1/fps.
            let mut real = if presenter.is_some() {
                if paused { 0.0 } else { frame_dt as f64 }
            } else {
                1.0 / fps
            };
            // A window holds a line at its end until A / Enter; a press before then skips to it.
            if presenter.is_some()
                && let Some(end) = play.stage.line.as_ref().and_then(|l| l.until)
                && line_released != Some(end) {
                    let tick = 1.0 / f64::from(clock.rate());
                    let left = (f64::from(end) - f64::from(play.stage.time) - tick).max(0.0);
                    // Auto: the line's reading time is up and its voice done.
                    let voice_done = match (line_voice, sound.as_ref()) {
                        (Some(v), Some((_, mixer, ..))) => !mixer.is_playing(v),
                        _ => true,
                    };
                    if auto_lines && left <= 0.0 && voice_done {
                        next_line = true;
                    }
                    if std::mem::take(&mut next_line) {
                        line_released = Some(end);
                        real = real.max(left + tick);
                        if let (Some(v), Some((_, mixer, ..))) = (line_voice.take(), sound.as_mut()) {
                            mixer.stop(v);
                        }
                    } else {
                        real = real.min(left);
                    }
                }
            next_line = false;
            let f = clock.advance(real);
            for _ in 0..f.ticks {
                *before = std::mem::replace(after, {
                    play.tick();
                    play.stage.snapshot()
                });
                let cues = std::mem::take(&mut play.stage.cues);
                if let Some((_, mixer, library, music)) = sound.as_mut() {
                    for cue in cues {
                        match cue {
                            barrel_engine::stage::Cue::Music { track: Some(t), volume, .. } => {
                                if let Some(m) = music.take() {
                                    mixer.stop(m);
                                }
                                if let Some(m) = library.music(t) {
                                    eprintln!("music {t}");
                                    *music = Some(mixer.play_stream(m.stream, m.volume * volume, m.loop_from));
                                }
                            }
                            barrel_engine::stage::Cue::Music { track: None, fade, .. } => {
                                if let Some(m) = music.take() {
                                    mixer.fade_out(m, fade);
                                }
                            }
                            barrel_engine::stage::Cue::Effect { effect, volume, pan } => {
                                if let Some(e) = library.effect(effect) {
                                    let p = match e {
                                        barrel_audio::Effect::Clip(s) => mixer.play(s, volume, None),
                                        barrel_audio::Effect::Stream(s, from) => mixer.play_stream(s, volume, from),
                                    };
                                    mixer.set_pan(p, pan);
                                }
                            }
                            barrel_engine::stage::Cue::Voice { clip } => {
                                eprintln!("frame {frame}: voice {clip}");
                                if let Some(v) = library.voice(clip) {
                                    line_voice = Some(mixer.play(v, barrel_audio::VOICE_GAIN, None));
                                }
                            }
                        }
                    }
                }
            }
            if let Some((out, mixer, ..)) = sound.as_mut() {
                out.pump(mixer, clock.time());
            }
            versions_shown.clone_from(&play.stage.versions_shown);
            if event_objects != play.stage.event_objects {
                eprintln!("frame {frame}: event objects {}", if play.stage.event_objects { "shown" } else { "hidden" });
            }
            event_objects = play.stage.event_objects;
            // The field the event is in now.
            if let Some(k) = play.stage.field.and_then(|m| field_maps.iter().position(|&f| f == m))
                && k as u16 != current_field {
                    eprintln!("frame {frame}: field {}", field_maps[k]);
                    current_field = k as u16;
                }
            if let Some((layer, font, last)) = text_layer.as_mut() {
                // Between the lines of a conversation the window stays up (`WINDOW_GRACE`).
                let line = play.stage.window(WINDOW_GRACE);
                // The narration card and its opacity (in 1/1000ths, so a fade redraws each step).
                let card = play.stage.telop.filter(|_| play.stage.telop_alpha.now() > 0.0);
                let card_alpha = (play.stage.telop_alpha.now().clamp(0.0, 1.0) * 1000.0) as u16;
                // The still (and the one it fades in over), with its opacity in 1/1000ths.
                let still = play.stage.still.map(|(n, before, since, seconds)| {
                    let a = if seconds > 0.0 { ((play.stage.time - since) / seconds).clamp(0.0, 1.0) } else { 1.0 };
                    (n, before, (a * 1000.0) as u16)
                });
                let now = Some((line.clone(), width, height, card, card_alpha, still));
                if now != *last {
                    // The still picture first (under the window and the text): its size on a
                    // 1920 × 1080 screen, scaled to the height and centred.
                    let mut quads = Vec::new();
                    if let Some((n, before, a)) = still {
                        let k = height as f32 / 1080.0;
                        let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
                        let a = a as f32 / 1000.0;
                        let layers = [(before.filter(|_| a < 1.0), 1.0), (Some(n), a)];
                        for (picture, alpha) in layers {
                            if let Some(&(texture, [w, h])) = picture.and_then(|p| still_slots.get(&p)) {
                                let rect = [cx - w * k / 2.0, cy - h * k / 2.0, cx + w * k / 2.0, cy + h * k / 2.0];
                                quads.push(text::Quad { turn: false, rect, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, alpha], texture });
                            }
                        }
                    }
                    quads.extend(match &line {
                        Some(l) => {
                            // The name the line gives (the speaker-name table: "???" before a
                            // character is introduced), else the speaker's own.
                            let name = l
                                .name
                                .and_then(|k| usize::try_from(k).ok())
                                .and_then(|k| window_names.get(k))
                                .or_else(|| usize::try_from(l.speaker).ok().and_then(|k| speaker_names.get(k)))
                                .map(String::as_str);
                            if std::env::var("BARREL_TEXT_LOG").is_ok() {
                                eprintln!("frame {frame}: line {:?}: {}", name, l.text);
                            }
                            message::dialogue(&font, name, &l.text, width as f32, height as f32)
                        }
                        None => Vec::new(),
                    });
                    // The card: the 1920 × 1080 layout scaled to the height and centred.
                    if let Some(pieces) = card.and_then(|c| telop_pieces.get(&c)) {
                        let k = height as f32 / 1080.0;
                        let dx = (width as f32 - 1920.0 * k) / 2.0;
                        let a = card_alpha as f32 / 1000.0;
                        if std::env::var("BARREL_TEXT_LOG").is_ok() {
                            eprintln!("frame {frame}: narration card {:?} at {a:.2}", card);
                        }
                        for &(texture, uv, [l, t, r, b]) in pieces {
                            quads.push(text::Quad { turn: false, rect: [dx + l * k, t * k, dx + r * k, b * k], uv, color: [1.0, 1.0, 1.0, a], texture });
                        }
                    }
                    layer.set(&gpu, &quads)?;
                    *last = now;
                }
            }
            let shown = barrel_engine::stage::Snapshot::blend(before, after, f.alpha);
            // The event's camera as the stage has it (game space, x mirrored like the
            // vertices); the opening one until the event sets one.
            if let (Some(e), Some(t), true) = (shown.eye, shown.target, use_event_camera) {
                let mirror = Vec3::new(-1.0, 1.0, 1.0);
                (eye, target) = (Vec3::from(e) * mirror, Vec3::from(t) * mirror);
                // The camera's roll (t10): up turned about the line from the target to the eye
                // (game space, then mirrored like the rest).
                if shown.roll != 0.0 {
                    let axis = (Vec3::from(e) - Vec3::from(t)).normalize_or_zero();
                    if axis != Vec3::ZERO {
                        up = glam::Quat::from_axis_angle(axis, shown.roll) * Vec3::Y * mirror;
                    }
                }
                // A camera quake: each axis a random whole number of field units between its two
                // tracks' values (metres) at the quake's frame, added to the eye and the target
                // (0x140035920; which camera values the game moves is taken, not read).
                if let Some((number, since, seconds)) = play.stage.quake {
                    let elapsed = play.stage.time - since;
                    if let (Some(q), true) = (quakes.iter().find(|q| q.number() == Some(number)), elapsed <= seconds) {
                        let frame = elapsed * 60.0;
                        let mut shake = [0.0f32; 3];
                        for (k, axis) in shake.iter_mut().enumerate() {
                            if let (Some(a), Some(b)) = (q.tracks.get(2 * k), q.tracks.get(2 * k + 1)) {
                                let (lo, hi) = ((a.at(frame) * 100.0) as i32, (b.at(frame) * 100.0) as i32);
                                *axis = quake_rng.range(lo, hi) as f32;
                            }
                        }
                        let offset = Vec3::from(shake) * mirror;
                        (eye, target) = (eye + offset, target + offset);
                    }
                }
                dist = (eye - target).length().max(1.0);
                if let Some(f) = shown.fov {
                    fov = f.clamp(5.0, 120.0).to_radians();
                }
            }
            let since = ((clock.ticks - start_ticks) as f32 + f.alpha) / clock.rate() as f32;
            // The time on screen: the logic time less the part of the last tick not yet shown.
            let shown_time = play.stage.time - (1.0 - f.alpha) / clock.rate() as f32;
            for a in actors.iter_mut() {
                let gone = !shown.characters.contains_key(&a.who);
                if gone != hidden[a.instance as usize] {
                    eprintln!("frame {frame}: character {} {}", a.who, if gone { "leaves" } else { "appears" });
                }
                hidden[a.instance as usize] = gone;
                // The stage's motion for the character, by its MOTION_* name; a new one is
                // loaded (once) and played from the time the stage started it.
                let staged = play.stage.characters.get(&a.who).and_then(|b| b.motion);
                // A line's gesture, when newer than the motion: its chain of motion divisions
                // plays from then on (until the next motion command or gesture), the last (or a
                // looping one) held.
                let gestured = play.stage.characters.get(&a.who).and_then(|b| b.gesture).and_then(|(g, since)| {
                    let form = gestures_play.get(usize::try_from(g).ok()?)?[0].as_ref()?;
                    let ((motion, division), began) = form.at(shown_time - since, |m, d| {
                        let column = motion_names.get(m as usize)?;
                        let clip = a.motions.get(column)?.duration;
                        match d.and_then(|k| *divisions.get(&(a.motions.head.clone(), column.clone()))?.get(k as usize)?) {
                            Some(div) => Some((div.end.unwrap_or(clip) - div.start, div.loops)),
                            None => Some((clip, false)),
                        }
                    });
                    Some(barrel_engine::stage::Playing { motion, since: since + began, looping: true, blend: 0.2, division })
                });
                let staged = match (staged, gestured) {
                    (Some(m), Some(g)) if g.since > m.since => Some(g),
                    (None, Some(g)) => Some(g),
                    (m, _) => m,
                };
                let mut t = a.start + since;
                // Blend weight of the current motion against the one before it.
                let mut weight = 1.0;
                if let Some(m) = staged
                    && let Some(column) = motion_names.get(m.motion as usize) {
                        let now_playing = (column.clone(), m.division, m.since);
                        if now_playing != a.playing {
                            // Mid-blend, or a new division of the same motion: blend out of
                            // the pose last shown; else the old motion runs on under the new.
                            let mid_blend = a.previous.is_some();
                            let same_motion = *column == a.column;
                            if same_motion {
                                if m.blend > 0.0 && !a.last_locals.is_empty() {
                                    a.previous = Some(BlendFrom::Held(a.last_locals.clone()));
                                }
                            } else if let Some(anim) = a.motions.get(column).cloned() {
                                eprintln!("frame {frame}: character {} plays {column} (blend {:.2} s)", a.who, m.blend);
                                let old = std::mem::replace(&mut a.anim, anim);
                                a.previous = match (m.blend > 0.0, mid_blend && !a.last_locals.is_empty()) {
                                    (false, _) => None,
                                    (true, true) => Some(BlendFrom::Held(a.last_locals.clone())),
                                    (true, false) => Some(BlendFrom::Running(old, a.last_t, a.looping)),
                                };
                            }
                            a.column = column.clone();
                            a.looping = m.looping;
                            a.playing = now_playing;
                        }
                        t = shown_time - m.since;
                        // Its division, if the command names one the tables have.
                        a.division = m.division.and_then(|k| {
                            *divisions.get(&(a.motions.head.clone(), column.clone()))?.get(k as usize)?
                        });
                        if let (Some(k), true) = (m.division, *column != a.last_division_column) {
                            eprintln!("frame {frame}: character {} {column} division {k}: {:?}", a.who, a.division);
                            a.last_division_column = column.clone();
                        }
                        if m.blend > 0.0 {
                            weight = (t / m.blend).clamp(0.0, 1.0);
                        }
                    }
                a.last_t = t;
                if let Some(&(pos, facing)) = shown.characters.get(&a.who) {
                    // The facing, then the tilt about the character's own x axis (s18 mode 2).
                    let tilt = shown.tilts.get(&a.who).copied().unwrap_or(0.0);
                    let turn = glam::Quat::from_rotation_y(facing) * glam::Quat::from_rotation_x(tilt);
                    instances[a.instance as usize].model = render_space(Mat4::from_rotation_translation(turn, Vec3::from(pos)));
                }
                let base = instances[a.instance as usize].palette as usize;
                let clock = match a.division {
                    Some(d) => d.clock(t, a.anim.duration),
                    None => motion_clock(&a.anim, t, a.looping),
                };
                let mut locals = local_pose(&a.skeleton, &a.anim, clock);
                match (&a.previous, weight < 1.0) {
                    // The old motion runs on from where it was while the new one fades in.
                    (Some(BlendFrom::Running(old, at_switch, old_loops)), true) => {
                        let before = local_pose(&a.skeleton, old, motion_clock(old, at_switch + t.max(0.0), *old_loops));
                        locals = mix_pose(&before, &locals, weight);
                    }
                    (Some(BlendFrom::Held(before)), true) if before.len() == locals.len() => locals = mix_pose(before, &locals, weight),
                    (Some(_), _) => a.previous = None,
                    _ => {}
                }
                // `BARREL_POSE_JUMPS=CM`: report a frame where a joint moved more than CM
                // (game units) in the model's own space, to find motion pops.
                if let Some(limit) = pose_jumps
                    && a.last_locals.len() == locals.len() {
                        let world = |l: &[Trs]| skin_world(&a.skeleton, l);
                        let (was, now) = (world(&a.last_locals), world(&locals));
                        let (k, d) = was.iter().zip(&now).map(|(p, q)| p.distance(*q)).enumerate().fold((0, 0.0f32), |m, (k, d)| if d > m.1 { (k, d) } else { m });
                        if d > limit {
                            eprintln!("frame {frame}: character {} joint {k} jumped {d:.1} ({})", a.who, a.column);
                        }
                    }
                a.last_locals.clone_from(&locals);
                // A head gesture (s116): its segment's pose laid over the joints it turns, at its
                // weight (the game lerps the joints' transforms, 0x1404177a0).
                if let Some((segment, rate, w, since)) = play.stage.characters.get(&a.who).and_then(|b| b.head) {
                    if a.head.is_none() {
                        a.head = a.motions.head_gestures();
                    }
                    if let Some((clip, joints)) = &a.head {
                        let (from, to) = barrel_engine::event::HEAD_SEGMENTS[segment];
                        let at = (from / 30.0 + ((shown_time - since).max(0.0) * rate)).min(to / 30.0);
                        let over = local_pose(&a.skeleton, clip, at);
                        for &j in joints {
                            if let (Some(base), Some(top)) = (locals.get(j).copied(), over.get(j)) {
                                locals[j] = (base.0.lerp(top.0, w), base.1.slerp(top.1, w), base.2.lerp(top.2, w));
                            }
                        }
                    }
                }
                let joints = skin(&a.skeleton, &locals);
                // The face the stage gives the character now (a talking mouth moves).
                if let (Some(rig), Some(tables), Some(b)) = (a.face.as_mut(), &face_tables_play, play.stage.characters.get(&a.who)) {
                    let face = tables.expression(rig.row, &rig.head, b.expression.unwrap_or(0), b.talking);
                    let base = instances[a.instance as usize].weights as usize;
                    let total: usize = rig.sets.iter().map(|(n, _)| n).sum();
                    if let Some(block) = morph_weights.get_mut(base..base + total) {
                        rig.weights(&face, b.talking, shown_time, block);
                        if std::env::var("BARREL_FACE_LOG").is_ok() {
                            let on: f32 = block.iter().sum();
                            eprintln!("frame {frame}: character {} face {:?}{} weight sum {on:.3}", a.who, b.expression, if b.talking { " talking" } else { "" });
                        }
                    }
                }
                for (k, m) in joints.iter().enumerate() {
                    if let Some(slot) = palette.get_mut(base + k) {
                        *slot = render_space(*m);
                    }
                }
            }
            scene.write_instances(&gpu, &instances)?;
            scene.write_weights(&gpu, &morph_weights)?;
            scene.write_palette(&gpu, &palette, &prev_palette)?;
        }
    if let Some(c) = cpu.as_mut() {
        c.mark("people + events + poses");
    }
    // A resized window, or a swapchain gone stale: remake it and the framebuffers.
    if let (Some(p), Some(pp)) = (presenter.as_mut(), pump.as_mut())
        && (pp.take_resized() || stale) {
            let size = pp.window().inner_size();
            if size.width == 0 || size.height == 0 {
                continue; // minimised
            }
            p.rebuild(&gpu, size.width, size.height)?;
            (width, height) = (p.extent.width, p.extent.height);
            renderer.rebuild_targets(&gpu, Some(&*p), width, height)?;
            stale = false;
            eprintln!("frame {frame}: window now {width}x{height}");
        }
    // The image to draw into: the window's next, or the renderer.offscreen one.
    let image = match (&presenter, &sync) {
        (Some(p), Some(f)) => match p.acquire(f.acquired(&gpu))? {
            Some(i) => i as usize,
            None => {
                stale = true;
                continue;
            }
        },
        _ => 0,
    };
    let mvp = view_proj(eye, target, up, dist, fov, width, height, orbit);
    if let Some(c) = cpu.as_mut() {
        c.mark("acquire");
    }
    // A camera cut (or a new field): the last frame's view says nothing about this one, so TAA
    // starts its history afresh and the motion vectors take this frame's view for the last.
    let shot = taa::Shot { eye, target, fov };
    if last_shot.is_some_and(|before| taa::is_cut(before, shot)) || shot_field != current_field {
        if last_shot.is_some() {
            eprintln!("frame {frame}: camera cut");
            prev_mvp = None;
            renderer.targets.history_fresh.set(true);
        }
        shot_field = current_field;
    }
    last_shot = Some(shot);
    // A camera that has not moved since the last frame (GSR 2 keeps more of its history then).
    let same_camera = prev_mvp.is_some_and(|p| p.abs_diff_eq(mvp, 1e-5));
    if std::env::var_os("BARREL_SHOTS").is_some() {
        eprintln!("frame {frame}: shot {shot:?}");
    }
    // The view-projection drawn with: shifted by this frame's sub-pixel jitter when it is on
    // (`settings::jitter`, a clip-space translation by the offset in pixels × 2 / the scene's
    // size), the plain one otherwise.
    let draw_vp = if settings.jitter {
        // FSR 3.1 expects its own cycle length (8 × the upscale ratio squared).
        let (sw, _) = renderer.targets.scene_size;
        let (jx, jy) = if settings.fsr3 { settings::jitter_cycle(jitter_index, fsr3::jitter_phase_count(sw as i32, width as i32) as u32) } else { settings::jitter(jitter_index) };
        frame_jitter = (jx, jy);
        let (sw, sh) = renderer.targets.scene_size;
        let mut j = Mat4::IDENTITY;
        j.w_axis.x = 2.0 * jx / sw as f32;
        j.w_axis.y = 2.0 * jy / sh as f32;
        jitter_index += 1;
        j * mvp
    } else {
        mvp
    };
    // The view's planes, for leaving out the field objects wholly outside it
    // (`BARREL_NO_CULL=1`: draw them all, to compare).
    let view_planes_now = view_planes(&draw_vp);
    // With motion vectors, this frame's and last frame's transforms and joints every frame (a
    // frame that moves nothing still has last frame's movement to forget).
    if settings.motion {
        scene.write_instances(&gpu, &instances)?;
        scene.write_palette(&gpu, &palette, &prev_palette)?;
    }
    // The sun's shadow map (deliberately one square around the camera's target: the game's
    // cascades are not read yet, docs/renderer.md "Shadows").
    // The event's changes to the field's lights (q42 to q57), over the field's own.
    let event_lights = player.as_ref().map(|p| p.0.stage.lights).unwrap_or_default();
    let parallels: Vec<([f32; 3], [f32; 4])> = {
        let mut list: Vec<([f32; 3], [f32; 4])> = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).map_or(Vec::new(), |l| {
            l.lights
                .iter()
                .filter_map(|p| match *p {
                    barrel_formats::field_look::Light::Parallel { rot, color } => Some((rot, color)),
                    _ => None,
                })
                .collect()
        });
        for (k, set) in event_lights.parallel.iter().enumerate() {
            if let Some((rgb, pitch, yaw)) = *set {
                let light = ([pitch, yaw, 0.0], [rgb[0], rgb[1], rgb[2], 1.0]);
                if k < list.len() {
                    list[k] = light;
                } else {
                    list.resize(k, ([90.0, 0.0, 0.0], [0.0; 4]));
                    list.push(light);
                }
            }
        }
        list
    };
    let sun_dir = parallels
        .first()
        .map(|(rot, _)| {
            let (p, y) = (rot[0].to_radians(), rot[1].to_radians());
            Vec3::new(-(p.cos() * y.sin()), -p.sin(), p.cos() * y.cos())
        })
        .unwrap_or(Vec3::NEG_Y);
    let shadow_vp = shadow::sun_view_proj(target, sun_dir, SHADOW_HALF, 6000.0);
    // A cross-fade (u13 kind 3): the frame it starts on keeps the last frame as it was, then
    // the live scene fades in over it.
    let (freeze_now, frozen_weight) = match player.as_ref().and_then(|p| p.0.stage.screen_effect.map(|e| (e, p.0.stage.time))) {
        Some(((3, _, seconds, since), now)) => {
            let fresh = last_effect != Some(since);
            if fresh {
                eprintln!("frame {frame}: cross-fade over {seconds} s");
            }
            last_effect = Some(since);
            let t = if seconds > 0.0 { ((now - since) / seconds).clamp(0.0, 1.0) } else { 1.0 };
            (fresh && frame > 0, 1.0 - t)
        }
        _ => (false, 0.0),
    };
    // The fade's colour over the screen now (u22): from the preset's first colour to its second.
    let overlay: [f32; 4] = match player.as_ref().and_then(|p| p.0.stage.fade.map(|f| (f, p.0.stage.time))) {
        Some(((preset, seconds, since), now)) => match fade_presets.get(preset as usize) {
            Some((from, to)) => {
                let t = if seconds > 0.0 { ((now - since) / seconds).clamp(0.0, 1.0) } else { 1.0 };
                std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t)
            }
            None => [0.0; 4],
        },
        None => [0.0; 4],
    };
    // The trees' node sway (game::wind::NodeSpring): each placement whose name holds an
    // `<n_shaker>` keyword springs about its rest axis in the field's wind, turned from it to
    // the spring's tip (render space: x mirrored, as the vertices are). The player's push is
    // not done here.
    if tree_springs.as_ref().is_none_or(|(f, _)| *f != current_field) {
        let mut springs = Vec::new();
        if let Some(l) = field_looks.get(current_field as usize).and_then(|l| l.as_ref()) {
            let mut seen = std::collections::HashSet::new();
            // Only instances all of whose draws are the tree's (a field's own instance holds
            // many objects; turning it would turn them all).
            let keyword_of = |d: &Draw| l.n_shakers.iter().position(|n| !n.keyword.is_empty() && d.name.contains(n.keyword.as_str()));
            let mixed: std::collections::HashSet<u32> = draws.iter().filter(|d| keyword_of(d).is_none()).map(|d| d.instance).collect();
            for draw in draws.iter().filter(|d| d.field.is_none_or(|f| f == current_field) && !d.sky && !mixed.contains(&d.instance)) {
                let Some(n) = l.n_shakers.iter().find(|n| !n.keyword.is_empty() && draw.name.contains(n.keyword.as_str())) else { continue };
                if seen.insert(draw.instance) {
                    let n = barrel_formats::field_look::NShaker { axis: [-n.axis[0], n.axis[1], n.axis[2]], ..n.clone() };
                    let spring = barrel_engine::wind::NodeSpring::new(&n);
                    springs.push((draw.instance, instances[draw.instance as usize].model, n, spring));
                }
            }
        }
        if !springs.is_empty() {
            eprintln!("{} trees sway by node", springs.len());
        }
        if let Some(l) = field_looks.get(current_field as usize).and_then(|l| l.as_ref()) {
            let grass = draws.iter().filter(|d| d.field.is_none_or(|f| f == current_field) && l.v_shakers.iter().take(8).any(|v| !v.keyword.is_empty() && d.name.contains(v.keyword.as_str()))).count();
            if grass > 0 {
                eprintln!("{grass} draws sway by vertex");
            }
        }
        tree_springs = Some((current_field, springs));
    }
    if let Some((_, springs)) = tree_springs.as_mut().filter(|(_, s)| !s.is_empty()) {
        let w = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).map_or([0.0; 3], |l| barrel_engine::wind::wind(&l.winds, pattern_time));
        let w = Vec3::new(-w[0], w[1], w[2]);
        let strength = w.length();
        // The player, in render space, for the trees' push.
        let player = walker.as_ref().map(|w| Vec3::new(-w.pos[0], w.pos[1], w.pos[2]));
        for (inst, base, n, spring) in springs.iter_mut() {
            let toward = (glam::Mat3::from_mat4(*base).inverse() * w).normalize_or_zero();
            // Within its char_range of the player (metres; here from the tree's origin, the game
            // measures from its tip in its own space), a push along a random direction, each
            // part (rand() % 100 - 50) / 50 as the game draws it (`0x140285886`).
            let push = player.filter(|p| p.distance(base.w_axis.truncate()) < n.char_range * 100.0).map(|_| {
                let mut part = || ((sway_rng.next() % 100) as f32 - 50.0) / 50.0;
                Vec3::new(part(), part(), part()).normalize_or_zero().to_array()
            });
            if push.is_some() && !brushed {
                eprintln!("frame {frame}: the player brushes a tree");
                brushed = true;
            }
            spring.step(n, toward.to_array(), strength, push, sway_dt);
            let turn = glam::Quat::from_rotation_arc(Vec3::from(n.axis), Vec3::from(spring.d));
            instances[*inst as usize].model = *base * Mat4::from_quat(turn);
        }
        scene.write_instances(&gpu, &instances)?;
    }
    // The maps' chosen way in their blue (`AreaMap::chosen`): the start's mark, the cursor's,
    // and the route's dots between them.
    if let Some(a) = area_map.as_ref() {
        let mut changed = false;
        for d in draws.iter().filter(|d| d.instance != 0) {
            let want = a.chosen(&d.name).then_some(CHOSEN_TINT);
            let i = &mut instances[d.instance as usize];
            if (i.tint_add[3] > 0.5) != want.is_some() {
                i.set_tint(want);
                changed = true;
            }
        }
        if changed {
            scene.write_instances(&gpu, &instances)?;
        }
    }
    // The current field's distant view follows the eye (plus the field's sphere offset).
    if let Some(Some(inst)) = sky_instances.get(current_field as usize) {
        let offset = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).and_then(|l| l.sphere_offset).unwrap_or([0.0; 3]);
        let at = turned_eye(eye, target, orbit) + Vec3::new(-offset[0], offset[1], offset[2]) * 100.0;
        instances[*inst as usize].model = Mat4::from_translation(at);
        scene.write_instances(&gpu, &instances)?;
    }
    // The field's character lighting, mirrored into render space, and the eye.
    {
        let field_look = field_looks.get(current_field as usize).and_then(|l| l.as_ref());
        let chara = field_look.and_then(|l| l.chara);
        // A field with none: a light from above and in front, no colour or rim.
        let c = chara.unwrap_or(barrel_formats::field_look::CharaShader {
            light_dir: [0.0, -1.0, -1.0],
            light_color: [1.0, 1.0, 1.0, 0.0],
            rim_light_dir: [0.0, -1.0, 1.0],
            rim_light_color: [1.0, 1.0, 1.0, 0.0],
            point_light_strength: 0.0,
        });
        let c = barrel_formats::field_look::CharaShader {
            light_color: event_lights.chara_color.unwrap_or(c.light_color),
            rim_light_color: event_lights.rim_color.unwrap_or(c.rim_light_color),
            point_light_strength: event_lights.point_strength.unwrap_or(c.point_light_strength),
            ..c
        };
        let m = |d: [f32; 3]| [-d[0], d[1], d[2], 0.0];
        let e = turned_eye(eye, target, orbit);
        let mut look = [[0f32; 4]; LOOK_SLOTS];
        // Last frame's view-projection (this frame's on the first), for the motion vectors.
        for (k, col) in prev_mvp.unwrap_or(mvp).to_cols_array_2d().into_iter().enumerate() {
            look[40 + k] = col;
        }
        // This frame's view-projection without the jitter, for the motion vectors.
        for (k, col) in mvp.to_cols_array_2d().into_iter().enumerate() {
            look[44 + k] = col;
        }
        // The billboards' axes (docs/shading.md, "Billboards"): the camera's right as the screen
        // shows it (render space is mirrored, so the view's -x), up, and back toward the eye.
        let cam = Mat4::look_at_rh(e, target, up);
        let row = |k: usize, sign: f32| [sign * cam.row(k).x, sign * cam.row(k).y, sign * cam.row(k).z, 0.0];
        look[37] = row(0, -1.0);
        look[38] = row(1, 1.0);
        look[39] = row(2, 1.0);
        look[..5].copy_from_slice(&[m(c.light_dir), c.light_color, m(c.rim_light_dir), c.rim_light_color, [e.x, e.y, e.z, 1.0]]);
        // The field's first four point lights (metres to field units, x mirrored).
        let points = field_look.map_or(Vec::new(), |l| {
            l.lights
                .iter()
                .filter_map(|p| match *p {
                    barrel_formats::field_look::Light::Point { pos, color, ambient, att, range } => Some((pos, color, ambient, att, range)),
                    _ => None,
                })
                .take(4)
                .collect::<Vec<_>>()
        });
        // The event's point lights take the field's slots (q54).
        let mut points = points;
        for (k, set) in event_lights.points.iter().enumerate() {
            if let Some(barrel_formats::field_look::Light::Point { pos, color, ambient, att, range }) = *set {
                let light = (pos, color, ambient, att, range);
                if k < points.len() {
                    points[k] = light;
                } else {
                    points.resize(k, ([0.0; 3], [0.0; 4], [0.0; 4], [1.0, 0.0, 0.0], 0.0));
                    points.push(light);
                }
            }
        }
        look[5] = [c.point_light_strength, points.len() as f32, 0.0, 0.0];
        // The field's two directional lights: (0, 0, 1) turned by rot.x degrees about x, then
        // rot.y about y, the light travelling along it (the field loader's own, 0x140068e6c).
        for (k, slot) in [22usize, 24].into_iter().enumerate() {
            if let Some((rot, color)) = parallels.get(k) {
                let (p, y) = (rot[0].to_radians(), rot[1].to_radians());
                look[slot] = m([p.cos() * y.sin(), -p.sin(), p.cos() * y.cos()]);
                look[slot + 1] = *color;
            }
        }
        // The light texture's mapping (docs/shading.md, "The light texture"): {1 / width,
        // 1 / height (field units), u + 0.5, v + 0.5}, and whether there is one.
        if let (Some(pat), true) = (field_look.and_then(|l| l.pattern), patterns.get(current_field as usize).is_some_and(Option::is_some)) {
            let drift = |speed: f32| (pattern_time * speed).rem_euclid(1.0);
            look[48] = [1.0 / (pat.size[0] * 100.0), 1.0 / (pat.size[1] * 100.0), drift(pat.speed[0]) + 0.5, drift(pat.speed[1]) + 0.5];
            look[49] = [1.0, 0.0, 0.0, 0.0];
        }
        // The grass sway groups (game::wind): the field's winds at the wind clock, each group's
        // own clock stepped by the frame's time.
        if let Some(l) = field_look {
            let wind = barrel_engine::wind::wind(&l.winds, pattern_time);
            shakers.resize(l.v_shakers.len().min(8), barrel_engine::wind::Shaker::default());
            for (k, (v, sh)) in l.v_shakers.iter().zip(shakers.iter_mut()).enumerate() {
                let sway = sh.step(v, wind, sway_dt);
                look[50 + 2 * k] = [sway.wind[0], sway.wind[1], sway.wind[2], sway.time];
                look[51 + 2 * k] = [sway.inv_height, sway.root_height, sway.shift[0], sway.shift[2]];
            }
        }
        look[26] = match event_lights.ambient {
            Some(rgb) => [rgb[0], rgb[1], rgb[2], 1.0],
            None => field_look.and_then(|l| l.ambient).unwrap_or([0.6, 0.6, 0.6, 1.0]),
        };
        // The fog, as the game turns the field's <fog> into its shader constants (read from
        // its memory: docs/shading.md, "Fog"); none: nothing taken off or added.
        if let Some(f) = field_look.and_then(|l| l.fog) {
            let (start, end) = (f.start * 100.0, f.end * 100.0);
            let span = (end - start).max(1.0);
            look[32] = [1.0 / span, -start / span, 0.0, 0.0];
            let sun = parallels.first().map_or([1.0; 4], |p| p.1);
            let (r, m, g) = (f.rayleigh_rgb, f.mie_rgb, f.mie_g);
            let ext = |i: usize| r[i] + m[i];
            let part = |x: f32, i: usize| if ext(i) > 0.0 { x / ext(i) } else { 0.0 };
            let pi = std::f32::consts::PI;
            look[33] = [-ext(0) * std::f32::consts::LOG2_E, -ext(1) * std::f32::consts::LOG2_E, -ext(2) * std::f32::consts::LOG2_E, 0.0];
            look[34] = [look[26][0] * f.ambient, look[26][1] * f.ambient, look[26][2] * f.ambient, 1.0];
            let k = 3.0 / (16.0 * pi) * f.rayleigh_pow;
            look[35] = [k * sun[0] * part(r[0], 0), k * sun[1] * part(r[1], 1), k * sun[2] * part(r[2], 2), -2.0 * g];
            let h = f.rayleigh_pow * (1.0 - g) * (1.0 - g) / (4.0 * pi);
            look[36] = [h * sun[0] * part(m[0], 0), h * sun[1] * part(m[1], 1), h * sun[2] * part(m[2], 2), 1.0 + g * g];
        }
        // The sun's shadow map: from the first directional light, over the camera's target.
        // On unless BARREL_NO_SHADOWS is set (for comparing), and off on the maps: the
        // recordings show no shadow under her there (the ruins' dark undersides are their own).
        if !parallels.is_empty() && !area_mode && std::env::var("BARREL_NO_SHADOWS").is_err() {
            for (k, col) in shadow_vp.to_cols_array_2d().into_iter().enumerate() {
                look[27 + k] = col;
            }
            look[31] = [1.0 / renderer.shadow_target.size as f32, 1.0, 0.0, 0.0];
        }
        for (k, (pos, color, ambient, att, range)) in points.iter().enumerate() {
            // The range in field units too (squared); the attenuation counts in field units.
            look[6 + k] = [-pos[0] * 100.0, pos[1] * 100.0, pos[2] * 100.0, (range * 100.0) * (range * 100.0)];
            look[10 + k] = *color;
            look[14 + k] = [att[0], att[1], att[2], 0.0];
            look[18 + k] = *ambient;
        }
        renderer.look_ring.write(&gpu, 0, unsafe { std::slice::from_raw_parts(look.as_ptr() as *const u8, LOOK_SIZE as usize) });
    }
    prev_mvp = Some(mvp);
    if let Some(c) = cpu.as_mut() {
        c.mark("camera, lights, sway");
    }
    // Set 1 for the field drawn (its light texture at binding 6).
    let instance_set = scene.instance_set(&gpu, current_field as usize);
    // The field HUD with the session's date and field time, rebuilt when they change.
    if let (Some((layer, font, sheets, nodes, last)), Some(s), Some(exe)) = (field_hud.as_mut(), session.as_ref(), hud_exe.as_ref()) {
        let mut v = hud::Values::on(s.today().max(0) as u32, s.field_tenths());
        v.cards = hud::cards(&s.taken);
        // The field effect's gauge: on the world map at 0 without a notice (the colseit-arrival
        // recording, after leaving the Southern Ruins at 45); in a field area and on its area
        // map, the session's; none in town (not checked against the game).
        v.gauge = match area_map.as_ref() {
            Some(a) if a.world => Some(hud::Gauge { percent: 0, mode: -1 }),
            Some(_) => Some(hud::Gauge { percent: s.field_gauge, mode: s.field_state.mode }),
            None => s.field().filter(|&m| m as usize >= barrel_escha::data::gimmick::TOWN_MAPS).map(|_| hud::Gauge { percent: s.field_gauge, mode: s.field_state.mode }),
        };
        let map = area_map.as_ref().map(|a| (a.world, a.on_town()));
        if *last != Some((v, width, height, map))
            && let Ok(pe) = barrel_formats::pe::Pe::parse(exe) {
                let texts = |p: &barrel_escha::data::layout::Placed| barrel_escha::data::layout::text(&pe, p);
                let elements = match area_map.as_ref() {
                    Some(a) => [hud::date_panel(nodes, texts, v), hud::map_list(nodes, texts, match (a.world, a.on_town()) {
                        (true, true) => None,
                        (true, false) => Some(hud::AREA_INFO),
                        (false, _) => Some(hud::RETURN),
                    })].concat(),
                    None => hud::elements(nodes, texts, v),
                };
                layer.set(&gpu, &hud::quads(&pe, Some(font), sheets, &elements, width as f32, height as f32))?;
                *last = Some((v, width, height, map));
            }
    }
    // The battle HUD with the party's HP and MP, rebuilt when they change.
    if let (Some(h), Some(play)) = (battle_layer.as_mut(), battle_play.as_ref()) {
        let members = (play.members(), play.order());
        let fast = usize::from(FAST_FORWARD.load(std::sync::atomic::Ordering::Relaxed));
        // A fighter's place on the 1920 × 1080 layout screen (scaled to the height, centred).
        let screen = |k: usize| -> Option<[i32; 2]> {
            let b = town_bodies.get(play.body_of(k)?)?;
            let p = instances.get(b.instance as usize)?.model.w_axis;
            let clip = mvp * glam::Vec4::new(p.x, p.y, p.z, 1.0);
            if clip.w <= 0.0 {
                return None;
            }
            let k = height as f32 / 1080.0;
            let dx = (width as f32 - 1920.0 * k) / 2.0;
            let (x, y) = ((clip.x / clip.w + 1.0) * 0.5 * width as f32, (clip.y / clip.w + 1.0) * 0.5 * height as f32);
            Some([((x - dx) / k) as i32, (y / k) as i32])
        };
        let commands = play.commands(screen);
        // The numbers up, each over its fighter (raised from its feet: the game's anchor not read).
        let numbers: Vec<hud::Element> = play
            .numbers
            .iter()
            .filter_map(|&(k, value, at)| {
                // Its place as the scene draws it (x mirrored).
                let p = play.place_of(k)?;
                let clip = mvp * glam::Vec4::new(-p[0], p[1] + NUMBER_RAISE, p[2], 1.0);
                if clip.w <= 0.0 {
                    return None;
                }
                let kk = height as f32 / 1080.0;
                let dx = (width as f32 - 1920.0 * kk) / 2.0;
                let (x, y) = ((clip.x / clip.w + 1.0) * 0.5 * width as f32, (clip.y / clip.w + 1.0) * 0.5 * height as f32);
                Some(battlehud::number(value, [((x - dx) / kk) as i32, (y / kk) as i32], play.time - at))
            })
            .flatten()
            .collect();
        let banner = play.banner.as_ref().filter(|(_, at)| play.time - at < battlehud::BANNER_SECONDS).map(|(name, _)| name.clone());
        let shown_result = play.result.as_ref().map(|(v, _)| v.clone());
        // The wipe's day and its fade in twentieths (20: white, the panel on it).
        let wipe = play.wiped.map(|(at, day)| (day, (((play.time - at) / WIPE_FADE).clamp(0.0, 1.0) * 20.0) as u32));
        if h.shown.as_ref() != Some(&(members.clone(), commands.clone(), shown_result.clone(), wipe, width, height, fast, numbers.clone(), banner.clone()))
            && let Ok(pe) = barrel_formats::pe::Pe::parse(&h.exe) {
                let mut elements = match &shown_result {
                    // The Result screen, in place of the battle's HUD.
                    Some(v) => result::elements(&h.result_nodes, |p| barrel_escha::data::layout::text(&pe, p), v),
                    None => {
                        let mut e = battlehud::elements(&h.nodes, |p| barrel_escha::data::layout::text(&pe, p), &members.0, &members.1, fast, commands.as_ref().and_then(|c| c.marked), commands.as_ref().map(|c| c.next));
                        if let Some(c) = &commands {
                            // The game's row; the skills' list and the target's name still
                            // Barrel's own text.
                            e.extend(battlehud::command_row(&c.row, c.at));
                            if let Some(plate) = &c.plate {
                                e.extend(battlehud::target_plate(&h.plate_nodes, |p| barrel_escha::data::layout::text(&pe, p), plate));
                            }
                            match &c.skills {
                                Some((list, chosen)) => e.extend(battlehud::command_list(list, *chosen, &c.title)),
                                None if !c.title.is_empty() => e.extend(battlehud::command_list(&[], 0, &c.title)),
                                None => {}
                            }
                        }
                        e
                    }
                };
                if shown_result.is_none() {
                    elements.extend(numbers.iter().cloned());
                    if let Some(name) = &banner {
                        let width = message::width_of(&h.font, name) * battlehud::BANNER_TEXT_SIZE / message::LINE * 1.47;
                        elements.extend(battlehud::banner(&pe, name, width));
                    }
                }
                if let Some((day, step)) = wipe {
                    elements.push(hud::Element::Fill { rect: [-4000, 0, 6000, 1080], colour: [1.0, 1.0, 1.0, step as f32 / 20.0] });
                    if step == 20 {
                        elements.extend(hud::date_panel(&h.field_nodes, |p| barrel_escha::data::layout::text(&pe, p), hud::Values::on(day, 0)));
                    }
                }
                h.layer.set(&gpu, &hud::quads(&pe, Some(&h.font), &h.sheets, &elements, width as f32, height as f32))?;
                h.shown = Some((members, commands, shown_result, wipe, width, height, fast, numbers, banner));
            }
    }
    // Offscreen, the frames written to files (with --save-every, only some): only those are
    // read back (8 MB a frame at 1080p).
    let shot_number = frame_base + frame;
    let saved_shot = frames == 1 || (shot_number + 1).is_multiple_of(save_every) || shot_number + 1 == frames;
    // The draws recorded this frame (scene, shadow), reported with `--timings`.
    let drawn;
    let layers: Vec<&text::Layer> = [
        text_layer.as_ref().map(|(l, ..)| l),
        // The maps' layer (their frame: the vignette and border, then their panels) under the
        // HUD's date panel and button lists, as the game draws them.
        area_map.as_ref().map(|a| &a.layer),
        field_hud.as_ref().map(|(l, ..)| l),
        battle_layer.as_ref().map(|h| &h.layer),
        notice.as_ref().map(|(l, _)| l),
    ]
    .into_iter()
    .flatten()
    .collect();
    let sway_groups: Vec<String> = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).map(|l| l.v_shakers.iter().take(8).map(|v| v.keyword.clone()).collect()).unwrap_or_default();
    let shown = |d: &Draw| d.version.is_none_or(|v| versions_shown.contains(&v)) && (event_objects || !d.name.contains("xEV_")) && !hide_by_name(&d.name) && !area_map.as_ref().is_some_and(|a| a.hides(&d.name));
    let material = |draw: &Draw| {
        // The distant view's solid pieces (all but the dome and the clouds) are lit and
        // alpha-tested like the field's models (docs/renderer.md, "Distant view"); xEF_ field
        // models (window light) are not lit; the grass sway group (bits 8-11, number + 1) by
        // the mesh name's keyword.
        let solid = draw.sky && !draw.name.contains("Sphere_sky") && !draw.name.contains("xEF_");
        // Not lit either: models drawn with `MapSTDNA` (the maps' marks and dots: their texels
        // as they are on screen, colseit-arrival 1:05).
        let unlit = !draw.sky && (draw.name.contains("xEF_") || draw.shader == "MapSTDNA");
        let bits = u32::from(solid) << 4 | u32::from(unlit) << 5
            | sway_groups.iter().position(|k| !k.is_empty() && draw.name.contains(k.as_str())).map_or(0, |k| (k as u32 + 1).min(15)) << 8;
        let tint = if draw.tint != 0 { draw.tint } else { area_map.as_ref().map_or(0, |a| a.tint(&draw.name)) };
        (bits, tint)
    };
    // The glow's rate is the field's (<hdr>; none: no glow); the bright pass's threshold is the
    // glow program's own default (`GLOW_THRESHOLD`): the game reads `<hdr threshold>` but never
    // passes it on.
    let glow = field_looks.get(current_field as usize).and_then(|l| l.as_ref()).and_then(|l| l.hdr).map_or((1.0, 0.0), |(_, rate)| (GLOW_THRESHOLD, rate));
    let zoom = orbit.2;
    let frame_desc = renderer::Frame {
        image,
        width,
        height,
        scene: &scene,
        instance_set,
        draws: &draws,
        instances: &instances,
        sets: &sets,
        white,
        outline_tex,
        current_field,
        hidden: &hidden,
        shown: &shown,
        material: &material,
        draw_vp,
        view_planes: view_planes_now,
        shadow_vp,
        alpha_cutoff,
        glow,
        saturation: [0.0; 3],
        tone_white: 0.0,
        tone_gamma: 0.0,
        filter: area_map.as_ref().zip(field_looks.get(current_field as usize).and_then(|l| l.as_ref())).and_then(|(a, l)| if a.world { l.filter_world } else { l.filter_area }).unwrap_or(0.0),
        camera: renderer::Camera { jitter: frame_jitter, fov, near: dist * 0.01 * zoom, far: (dist * 20.0 * zoom).max(80_000.0), dt: frame_dt, same: same_camera },
        freeze_now,
        frozen_weight,
        overlay,
        layers: &layers,
        readback: saved_shot.then_some(readback),
        sky_plane: false,
    };
    if let Some(c) = cpu.as_mut() {
        c.mark("HUD layers");
    }
    if let (Some(p), Some(f)) = (&presenter, sync.as_mut()) {
        // Draw into the acquired image, signal, present; the next frame is prepared while the
        // GPU draws this one (its slot's fence is waited for two frames on).
        let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(renderer.pool).command_buffer_count(1))?[0] };
        unsafe {
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            drawn = renderer.record(&gpu, cb, &frame_desc);
            d.end_command_buffer(cb)?;
        }
        if let Some(c) = cpu.as_mut() {
            c.mark("record");
        }
        if timings && frame % 100 == 10 {
            let [scene_draws, shadow_draws] = drawn;
            eprintln!("frame {frame}: {scene_draws} draws in the scene, {shadow_draws} in the shadow map");
        }
        let drawn = f.submit(&gpu, cb, image, p.images.len())?;
        stale |= p.present(&gpu, image as u32, drawn)?;
        if let Some(c) = cpu.as_mut() {
            c.mark("submit + present");
        }
        if !overlap {
            f.wait_submitted(&gpu)?;
            if let Some(c) = cpu.as_mut() {
                c.mark("wait for the GPU");
            }
        }
        if let Some(c) = cpu.as_mut() {
            c.end("");
        }
        if let Some(t) = renderer.timer.as_mut() {
            t.collect(&gpu)?;
        }
        gpu.frame.set(gpu.frame.get() + 1);
        continue;
    }
    {
        let f = sync.as_mut().ok_or("no frame sync")?;
        let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(renderer.pool).command_buffer_count(1))?[0] };
        unsafe {
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            drawn = renderer.record(&gpu, cb, &frame_desc);
            d.end_command_buffer(cb)?;
        }
        if let Some(c) = cpu.as_mut() {
            c.mark("record");
        }
        if timings && frame % 100 == 10 {
            let [scene_draws, shadow_draws] = drawn;
            eprintln!("frame {frame}: {scene_draws} draws in the scene, {shadow_draws} in the shadow map");
        }
        f.submit_offscreen(&gpu, cb)?;
        if saved_shot || !overlap {
            f.wait_submitted(&gpu)?;
            if let Some(c) = cpu.as_mut() {
                c.mark("wait for the GPU");
            }
        }
        if let Some(c) = cpu.as_mut() {
            c.end("");
        }
        gpu.frame.set(gpu.frame.get() + 1);
    }
    if let Some(t) = renderer.timer.as_mut() {
        t.collect(&gpu)?;
    }
    if !saved_shot {
        continue;
    }
    let mut pixels = vec![0u8; bytes as usize];
    unsafe {
        let p = d.map_memory(rmem, 0, bytes, vk::MemoryMapFlags::empty())? as *const u8;
        std::ptr::copy_nonoverlapping(p, pixels.as_mut_ptr(), pixels.len());
        d.unmap_memory(rmem);
    }
    // One frame: OUT.png; several: OUT_000.png, OUT_001.png, ... (with --save-every, only some).
    let path = if frames > 1 {
        let p = Path::new(&out_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
        p.with_file_name(format!("{stem}_{shot_number:03}.png"))
    } else {
        Path::new(&out_path).to_path_buf()
    };
    std::fs::write(&path, barrel_formats::png::encode_rgba(width, height, &pixels))?;
    eprintln!("wrote {}", path.display());
    }

    // The scene's own resources go; the renderer stays for the next.
    unsafe { d.device_wait_idle()? };
    if let Some((layer, ..)) = text_layer.take() {
        layer.destroy(&gpu);
    }
    if let Some((layer, _)) = notice.take() {
        layer.destroy(&gpu);
    }
    if let Some((layer, ..)) = field_hud.take() {
        layer.destroy(&gpu);
    }
    if let Some(h) = battle_layer.take() {
        h.layer.destroy(&gpu);
    }
    unsafe {
        d.destroy_buffer(readback, None);
        d.free_memory(rmem, None);
        if let Some(f) = sync.take() {
            f.destroy(&gpu, renderer.pool)?;
        }
    }
    scene.destroy(&gpu);
    frame_base += ran;
    if let Some(a) = area_map.take() {
        unsafe { gpu.device.device_wait_idle()? };
        a.layer.destroy(&gpu);
    }
    // The area map's Back: up to the world map, on the region's mark.
    if let Some(AreaPick::World { region }) = area_enter.as_ref() {
        let wmap = Path::new(&args[1]).to_path_buf();
        let at = world_point(&wmap, &format!("LP_{region}"))?;
        eprintln!("onto the world map at {region}");
        player_at = Some([at[0], at[2]]);
        area_mode = true;
        world_mode = true;
        area_enter = None;
        first_event = false;
        continue 'events;
    }
    // The world map's confirm: the trip (`Session::travelled`: its days, an event on the road
    // or on arriving), then the region's area map on its mark, or the town at its entrance.
    if let Some(AreaPick::Travel { from, to }) = area_enter.clone() {
        area_enter = None;
        let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("the world map is not inside a game install")?.to_path_buf();
        let pe_bytes = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&pe_bytes)?;
        let maps = barrel_escha::data::maps::field_maps(&pe);
        let wmap = root.join("Data/x64/Field/A15_WMAP");
        let bytes = std::fs::read(wmap.join("A15_WMAP.elixir.gz"))?;
        let container = Elixir::parse(&bytes)?;
        let entry = container.entries.iter().find(|e| e.name.ends_with(".opl2")).ok_or("the world map has no placements")?;
        let placements = barrel_formats::opl2::parse(container.data(entry))?;
        let map = barrel_escha::travel::WorldMap::load(&pe, &placements).ok_or("no routes")?;
        let field = trip_field(&pe, &to).ok_or(format!("{to} opens no field"))?;
        let scene = match session.as_mut() {
            Some(s) => {
                let left = s.field().unwrap_or(TOWN_ENTRANCE);
                s.travelled(&map, left, &from, &to, field).cloned().ok_or(format!("no way from {from} to {to}"))?
            }
            None => barrel_escha::session::Scene::Field(field),
        };
        eprintln!("trip {from} to {to}{}: {scene:?}", session.as_ref().map_or(String::new(), |s| format!(", day {}", s.today())));
        area_mode = false;
        world_mode = false;
        match scene {
            barrel_escha::session::Scene::Event(e) => {
                let s = session.take().ok_or("an event without a session")?;
                chain = Some((s.into_chain(), root.clone()));
                current_event = Some(root.join("Event/event").join(barrel_escha::event_start::event_file(&e)).to_string_lossy().into_owned());
                args[1] = "auto".into();
                player_at = None;
            }
            barrel_escha::session::Scene::Field(m) if to == "T0" => {
                // The town at its entrance, on its world map exit.
                let areas = barrel_escha::data::gimmick::areas(&pe, barrel_escha::data::gimmick::MAPS_EN, barrel_escha::data::gimmick::MAP_COUNT).ok_or("no map table")?;
                let exit = areas.get(m as usize).and_then(|a| a.gimmicks.iter().find(|g| g.kind == barrel_escha::data::gimmick::WORLDMAP_CHANGE_EX)).ok_or("the town entrance has no world map exit")?;
                args[1] = root.join("Data/x64/Field").join(maps.get(m as usize).ok_or("no such field")?).to_string_lossy().into_owned();
                player_at = Some([exit.position[0] * 100.0, exit.position[2] * 100.0]);
            }
            barrel_escha::session::Scene::Field(_) => {
                let at = world_point(&wmap, &format!("LP_{to}"))?;
                args[1] = wmap.to_string_lossy().into_owned();
                player_at = Some([at[0], at[2]]);
                area_mode = true;
            }
        }
        first_event = false;
        continue 'events;
    }
    // The area map's Enter: the field of the point she stands on, at its arrival place.
    if let Some(AreaPick::Field(field)) = area_enter.take() {
        // The sector entered is visited (its open neighbours seen: `area_map::known`).
        if let (Some(s), Some(a)) = (session.as_mut(), area_map.as_ref()) {
            s.visit(a.at);
        }
        let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("the world map is not inside a game install")?.to_path_buf();
        let pe_bytes = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
        let pe = barrel_formats::pe::Pe::parse(&pe_bytes)?;
        let maps = barrel_escha::data::maps::field_maps(&pe);
        // Entering a sector is a walk into its field (an event may start there, as a door's).
        let scene = match session.as_mut() {
            Some(s) => s.walked(field, field).clone(),
            None => barrel_escha::session::Scene::Field(field),
        };
        eprintln!("area map: into field {field} ({}): {scene:?}", maps.get(field as usize).map_or("?", |m| m.as_str()));
        match scene {
            barrel_escha::session::Scene::Event(e) => {
                let s = session.take().ok_or("an event without a session")?;
                chain = Some((s.into_chain(), root.clone()));
                current_event = Some(root.join("Event/event").join(barrel_escha::event_start::event_file(&e)).to_string_lossy().into_owned());
                args[1] = "auto".into();
                player_at = None;
            }
            barrel_escha::session::Scene::Field(m) => {
                args[1] = root.join("Data/x64/Field").join(maps.get(m as usize).ok_or("no such field")?).to_string_lossy().into_owned();
                match barrel_escha::data::world::arrival(&pe, m) {
                    Some(a) => {
                        player_at = Some([a.position[0] * 100.0, a.position[2] * 100.0]);
                        player_facing = Some(a.facing.to_radians());
                    }
                    None => player_at = Some([0.0, 0.0]),
                }
            }
        }
        area_mode = false;
        first_event = false;
        continue 'events;
    }
    // Onto the world map: the trip to `--travel-to`, through the session (its days, an event on
    // the road or on arriving), then the field it opens, the player at its own world map exit.
    if world_exit {
        // Without `--travel-to`, a field area's exit leads to its area map, the player on the
        // field's point (the town's leads to the world map itself: not drawn yet).
        if travel_to.is_none()
            && let Some(root) = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf) {
                let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
                let maps = barrel_escha::data::maps::field_maps(&barrel_formats::pe::Pe::parse(&exe)?);
                let here = Path::new(&args[1]).file_name().and_then(|n| n.to_str()).and_then(|n| maps.iter().position(|m| m == n)).map(|h| h as u32);
                if let Some(h) = here
                    && let Some((point, at)) = area_point(&root, h)? {
                    if let Some(s) = session.as_mut() {
                        s.left_for_area_map(h);
                    }
                    eprintln!("onto the area map at {point}");
                    args[1] = root.join("Data/x64/Field/A15_WMAP").to_string_lossy().into_owned();
                    player_at = Some(at);
                    player_walk = player_walk.map(|_| [0.0, 0.0]);
                    area_mode = true;
                    first_event = false;
                    continue 'events;
                }
            }
        // The town's exit: onto the world map at the town's mark.
        if travel_to.is_none()
            && let Some(root) = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf) {
                let wmap = root.join("Data/x64/Field/A15_WMAP");
                let at = world_point(&wmap, "LP_T0")?;
                eprintln!("onto the world map at T0");
                args[1] = wmap.to_string_lossy().into_owned();
                player_at = Some([at[0], at[2]]);
                player_walk = player_walk.map(|_| [0.0, 0.0]);
                area_mode = true;
                world_mode = true;
                first_event = false;
                continue 'events;
            }
        let Some(to) = travel_to.clone() else {
            eprintln!("on the world map: no --travel-to MARK, so the run ends here");
            break 'events;
        };
        let root = Path::new(&args[1]).ancestors().find(|p| p.join("Saves").is_dir()).ok_or("the field is not inside a game install")?.to_path_buf();
        let maps = barrel_escha::data::maps::field_maps(&barrel_formats::pe::Pe::parse(&std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?)?);
        let here = Path::new(&args[1]).file_name().and_then(|n| n.to_str()).and_then(|n| maps.iter().position(|m| m == n)).ok_or("this field is not in the map table")? as u32;
        let (map, from, field, arrive, facing) = world_exit_trip(&root, here, &to)?;
        let scene = match session.as_mut() {
            Some(s) => s.travelled(&map, here, &from, &to, field).cloned().ok_or(format!("no way from {from} to {to}"))?,
            None => barrel_escha::session::Scene::Field(field),
        };
        eprintln!("trip {from} to {to}{}: {scene:?}", session.as_ref().map_or(String::new(), |s| format!(", day {}", s.today())));
        match scene {
            barrel_escha::session::Scene::Event(e) => {
                let s = session.take().ok_or("an event without a session")?;
                chain = Some((s.into_chain(), root.clone()));
                current_event = Some(root.join("Event/event").join(barrel_escha::event_start::event_file(&e)).to_string_lossy().into_owned());
                args[1] = "auto".into();
                player_at = None;
            }
            // A trip ends on the area map of the place it reached (as the game's does: the
            // region's map with its sectors), the player on that place's point; a field on no
            // point (or the town) is entered at once.
            barrel_escha::session::Scene::Field(m) => match area_point(&root, m)? {
                Some((point, at)) => {
                    eprintln!("onto the area map at {point}");
                    args[1] = root.join("Data/x64/Field/A15_WMAP").to_string_lossy().into_owned();
                    player_at = Some(at);
                    player_walk = player_walk.map(|_| [0.0, 0.0]);
                    area_mode = true;
                    travel_to = None;
                }
                None => {
                    args[1] = root.join("Data/x64/Field").join(maps.get(m as usize).ok_or("no such field")?).to_string_lossy().into_owned();
                    player_at = Some(arrive);
                    player_facing = facing;
                    player_walk = player_walk.map(|_| [0.0, 0.0]);
                }
            },
        }
        first_event = false;
        continue 'events;
    }
    // Through a door: the same again in the next map, the player at the door's spot.
    if let Some((name, at, velocity)) = door.take()
        && let Some(next) = Path::new(&args[1]).parent().map(|p| p.join(&name)) {
            // In a story's free play the walk may start an event (`Session::walked`): it plays,
            // and free play goes on after it where it leaves the hero.
            let root = next.ancestors().find(|p| p.join("Saves").is_dir()).map(Path::to_path_buf);
            let maps = root.as_ref().and_then(|r| std::fs::read(r.join("Atelier_Escha_and_Logy_EN.exe")).ok()).and_then(|b| barrel_formats::pe::Pe::parse(&b).ok().map(|pe| barrel_escha::data::maps::field_maps(&pe))).unwrap_or_default();
            let number = |p: &Path| p.file_name().and_then(|n| n.to_str()).and_then(|n| maps.iter().position(|m| m == n)).map(|m| m as u32);
            if let (Some(s), Some(from), Some(to), Some(root)) = (session.as_mut(), number(Path::new(&args[1])), number(&next), root.clone())
                && let barrel_escha::session::Scene::Event(e) = s.walked(from, to).clone() {
                    eprintln!("field {from} to {to}, day {}: {e} starts", s.today());
                    let s = session.take().unwrap();
                    chain = Some((s.into_chain(), root.clone()));
                    current_event = Some(root.join("Event/event").join(barrel_escha::event_start::event_file(&e)).to_string_lossy().into_owned());
                    args[1] = "auto".into();
                    player_at = None;
                    first_event = false;
                    continue 'events;
                }
            args[1] = next.to_string_lossy().into_owned();
            player_at = Some(at);
            if player_walk.is_some() {
                player_walk = Some(velocity);
            }
            first_event = false;
            continue 'events;
        }
    let ended = event_spec.as_deref().and_then(|e| Path::new(e).file_stem()).and_then(|s| s.to_str()).unwrap_or("").to_string();
    let next = match chain.as_mut() {
        Some((c, root)) if !stop && frame_base < frames => c.after(&ended).map(|n| root.join("Event/event").join(barrel_escha::event_start::event_file(n))),
        _ => None,
    };
    match next {
        Some(path) if path.is_file() => {
            eprintln!("{ended} ends: {}", path.display());
            current_event = Some(path.to_string_lossy().into_owned());
            first_event = false;
        }
        Some(path) => return Err(format!("{}: no such event file", path.display()).into()),
        None if free_play && chain.is_some() && !stop && frame_base < frames && current_event.is_some() => {
            // Free play: the last event's last field, the hero where the event left them.
            let path = Path::new(current_event.as_deref().unwrap_or_default()).to_path_buf();
            let fields = event_fields(&path)?;
            let event = barrel_formats::ebd::Ebd::parse(&std::fs::read(&path)?)?;
            let steps = barrel_engine::event::Player::new(&event, &[], &barrel_formats::ebd::OperandTable::escha())?.hero(hero).schedule();
            let last = barrel_engine::event::last_field(&steps).and_then(|m| fields.iter().find(|(n, _)| *n == m)).map(|(_, p)| p.clone());
            let at = player.as_ref().and_then(|(play, ..)| play.stage.snapshot().characters.get(&hero).map(|(p, _)| [p[0], p[2]]));
            let (Some(dir), Some(at)) = (last, at) else {
                eprintln!("{ended} ends: no event starts, and no field or hero to play freely");
                break 'events;
            };
            eprintln!("{ended} ends: free play in {} at ({:.0}, {:.0})", dir.display(), at[0], at[1]);
            args[1] = dir.to_string_lossy().into_owned();
            current_event = None;
            player_at = Some(at);
            // The story goes on as the session, in free play there.
            if let (Some((c, root)), Some(map)) = (chain.take(), barrel_engine::event::last_field(&steps)) {
                // With the calendar (the main generator not known: a fixed seed stands in).
                let data = barrel_escha::days::Data::load(&root)?;
                let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
                session = Some(barrel_escha::session::Session::in_field(c, map, hero == 1).with_calendar(data, barrel_engine::rng::Rng::seeded(1)).with_party(&party_defaults(&exe)));
            }
            first_event = false;
        }
        None => {
            if chain.is_some() && !stop {
                if frame_base >= frames {
                    eprintln!("{ended}: the frames are used up");
                } else {
                    eprintln!("{ended} ends: no event starts");
                }
            }
            break 'events;
        }
    }
    }

    // Tear down in reverse order.
    renderer.destroy(&gpu)?;
    if let Some(c) = cpu.take() {
        c.report();
    }
    unsafe {
        if let Some(p) = presenter {
            p.destroy(&gpu);
        }
        d.destroy_device(None);
        gpu.instance.destroy_instance(None);
    }
    let _ = gpu.physical;
    Ok(match restart {
        Some(a) => Ended::Restart(a, pump, carry_out),
        None => Ended::Done,
    })
}

#[cfg(test)]
mod restart_tests {
    use super::*;

    #[test]
    fn a_title_launch_restarts_in_a_field_of_the_install() {
        // Kintsugi's launch, then Options kept in play: the carried save needs a real field.
        let a: Vec<String> = ["--field", "auto", "/dev/null", "--title", "/g", "--window", "0"].map(String::from).to_vec();
        let again = with_install_field(without_settings_flags(&plain_args(&a)), Path::new("/g"));
        assert_eq!(again, ["--field", "/g/Data/x64/Field/TOWN_04", "/dev/null", "--window", "0"].map(String::from).to_vec());
        // A run started in a field keeps its own.
        let b: Vec<String> = ["--field", "/g/Data/x64/Field/AREA_03_02", "out.png"].map(String::from).to_vec();
        assert_eq!(with_install_field(b.clone(), Path::new("/g")), b);
    }

    #[test]
    fn title_args_drop_the_start() {
        let a: Vec<String> = ["--field", "/g/Data/x64/Field/TOWN_04", "out.png", "--load", "s.save", "--size", "1280x720", "--free-play"].map(String::from).to_vec();
        assert_eq!(title_args(&a, Path::new("/g"), false), ["--field", "auto", "out.png", "--size", "1280x720", "--title", "/g"].map(String::from).to_vec());
        assert_eq!(title_args(&a, Path::new("/g"), true).last().map(String::as_str), Some("--title-load"));
    }
}
