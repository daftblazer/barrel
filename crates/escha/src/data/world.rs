//! The world map's areas (English Escha executable): 85 records of 0x58 bytes at
//! [`AREAS_EN`], the routes between places (`R0` to `R12`) and the paths inside fields
//! (`A0r0` ...). See docs/tables.md, "World map".

use barrel_formats::pe::Pe;

pub const AREAS_EN: u64 = 0x1_40d6_7fb0;
pub const AREA_COUNT: usize = 85;
const AREA_SIZE: usize = 0x58;

/// One area record.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// Its short id (`R1`, `A2r3`).
    pub id: String,
    /// The float at +0x24 (1.0 to 3.5; its reader is not found).
    pub float: f32,
    /// The trip's days (+0x28), then two shorter ones, each once an event has been seen: the
    /// days at +0x30 after event +0x2c, at +0x38 after event +0x34 (-1 for none).
    pub days: [(i32, i32); 3],
}

impl Area {
    /// The days the trip takes now (`0x140171d80`): the last of its shorter times whose event
    /// has been seen, else its first.
    pub fn trip_days(&self, seen: impl Fn(u32) -> bool) -> i32 {
        let [(base, _), (fewer, after), (fewest, last)] = self.days;
        let has = |e: i32| e >= 0 && seen(e as u32);
        if has(last) {
            fewest
        } else if has(after) {
            fewer
        } else {
            base
        }
    }
}

/// The area records, in order (their numbers are the save's route numbers).
pub fn areas(pe: &Pe, table: u64) -> Option<Vec<Area>> {
    let at = pe.offset(table)?;
    let data = pe.data.get(at..at + AREA_COUNT * AREA_SIZE)?;
    let i32_at = |r: &[u8], o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    Some(
        data.chunks_exact(AREA_SIZE)
            .map(|r| Area {
                id: pe.name_at(u64::from_le_bytes(r[8..16].try_into().unwrap())).unwrap_or("").to_string(),
                float: f32::from_le_bytes(r[0x24..0x28].try_into().unwrap()),
                // The event comes before each shorter time: (days, -1), (days, event), (days, event).
                days: [(i32_at(r, 0x28), -1), (i32_at(r, 0x30), i32_at(r, 0x2c)), (i32_at(r, 0x38), i32_at(r, 0x34))],
            })
            .collect(),
    )
}

/// The world map's places (English Escha executable): 96 records of 0x60 bytes at
/// [`PLACES_EN`], right before the routes (`AREAS_EN`), one per world map point and in the
/// same count: `{name, name (Japanese), u64 kind, tag, i32 links[6], description, description
/// (Japanese), i32 a, i32 b, u64, u64}`. The kind is 0 for the town and the regions' big
/// points, 1 for a region's own entry and its sectors (`A2` "Southern Ruins", `A2p0` "Ruins
/// Entrance" ...). The tag is the place's mark on the world map (`point_tag`). The links are
/// route numbers (`areas`: 24 `A2r0`, 25 `A2r1` ..., the tags of the path dots between the
/// points), each shared by the two places it joins, one per direction the
/// cursor can go: right (+x), left (-x), down (+z), up (-z), then two others (-1: none): the Ruins Entrance `[-1, -1, 25, 24, -1, -1]`, Sector One
/// `[27, 25, 26, -1, -1, -1]`, the region's entry `[-1, -1, 24, -1, -1, -1]`. The description
/// is what the area map's panel shows ("A path leading into the ruins."); none for the town
/// and the regions' entries.
pub const PLACES_EN: u64 = 0x1_40d6_5bb0;
pub const PLACE_COUNT: usize = 96;
const PLACE_SIZE: usize = 0x60;

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub tag: String,
    pub kind: u32,
    pub links: [i32; 6],
    pub description: String,
    /// The two words at +0x48 (-1, 0 in nearly all; `A2p2`: 1, 30).
    pub extra: [i32; 2],
}

pub fn places(pe: &Pe, table: u64) -> Option<Vec<Place>> {
    let at = pe.offset(table)?;
    let data = pe.data.get(at..at + PLACE_COUNT * PLACE_SIZE)?;
    // A text: up to 200 bytes of UTF-8 before a zero (spaces and punctuation in it).
    let text = |r: &[u8], o: usize| {
        pe.offset(u64::from_le_bytes(r[o..o + 8].try_into().unwrap()))
            .and_then(|at| pe.data.get(at..))
            .and_then(|rest| rest.iter().take(200).position(|&b| b == 0).map(|end| &rest[..end]))
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
            .to_string()
    };
    let i32_at = |r: &[u8], o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    Some(
        data.chunks_exact(PLACE_SIZE)
            .map(|r| Place {
                name: text(r, 0),
                tag: text(r, 0x18),
                kind: i32_at(r, 0x10) as u32,
                links: std::array::from_fn(|k| i32_at(r, 0x20 + 4 * k)),
                description: text(r, 0x38),
                extra: [i32_at(r, 0x48), i32_at(r, 0x4c)],
            })
            .collect(),
    )
}

/// The arrival places (English Escha executable): where the player stands on entering a field
/// from the area map (and, presumably, the world map): 107 records of 24 bytes at
/// [`ARRIVALS_EN`], `{u32 map, f32 facing (degrees), f32 x, y, z (metres), u32 0}`, one per
/// field map (towns 0-10, fields from 12; some numbers have none). Read live (2026-11-13): the
/// Southern Ruins' Sector One put Escha at its record's (53.32, 0, 102.80), the Ruins Entrance at
/// (129.08, 0, 200) against its record's (129, 0, 200).
pub const ARRIVALS_EN: u64 = 0x1_407b_3ee0;
pub const ARRIVAL_COUNT: usize = 107;

/// One field's arrival place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrival {
    pub map: u32,
    /// Which way the player faces, degrees.
    pub facing: f32,
    /// Metres.
    pub position: [f32; 3],
}

/// The arrival places, in table order.
pub fn arrivals(pe: &Pe, table: u64) -> Option<Vec<Arrival>> {
    let at = pe.offset(table)?;
    let data = pe.data.get(at..at + ARRIVAL_COUNT * 24)?;
    let f = |r: &[u8], o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    Some(
        data.chunks_exact(24)
            .map(|r| Arrival { map: u32::from_le_bytes(r[0..4].try_into().unwrap()), facing: f(r, 4), position: [f(r, 8), f(r, 12), f(r, 16)] })
            .collect(),
    )
}

/// The arrival place of field map `map`, if it has one.
pub fn arrival(pe: &Pe, map: u32) -> Option<Arrival> {
    arrivals(pe, ARRIVALS_EN)?.into_iter().find(|a| a.map == map)
}

#[cfg(test)]
mod tests {
    use super::Area;

    #[test]
    fn trip_days_follow_the_events() {
        // R1, Colseit to the Southern Ruins: 4 days, 3 after EVENT_XG03_130, 2 after _140.
        let r1 = Area { id: "R1".into(), float: 1.5, days: [(4, -1), (3, 688), (2, 689)] };
        assert_eq!(r1.trip_days(|_| false), 4);
        assert_eq!(r1.trip_days(|e| e == 688), 3);
        assert_eq!(r1.trip_days(|e| e == 689 || e == 688), 2);
    }
}

/// The world map's points and the fields they open (English Escha): 107 records of 32 bytes at
/// [`POINT_MAPS_EN`], `{1, point, 0, 0, map, 0, n, event}`: world map point `point` (a
/// `WM_POINTER_*` number, see [`point_names`]) leads to field map `map`, once `event` has been
/// seen (-1, always, in all of them). `0x1400f18c0` looks a point (or a map) up in it. The town
/// point (`LP_T0`) leads to the town's maps 0 to 10; `SP_A0P0` (15) to map 12 (`AREA_01_01`).
pub const POINT_MAPS_EN: u64 = 0x1_407b_3180;
pub const POINT_MAP_COUNT: usize = 107;

/// A world map point's link to a field: (point, map, event; -1 none).
pub fn point_maps(pe: &Pe) -> Option<Vec<(u32, u32, i32)>> {
    let at = pe.offset(POINT_MAPS_EN)?;
    (0..POINT_MAP_COUNT)
        .map(|k| {
            let r = pe.data.get(at + 32 * k..at + 32 * (k + 1))?;
            let w = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            (w(0) == 1 && w(12) == 0).then(|| (w(4) as u32, w(16) as u32, w(28)))
        })
        .collect()
}

/// The world map points' names (`WM_POINTER_` dropped: `LP_T0`, `LP_A0`, ..., `SP_A0P0`, ...),
/// by number: the executable's names from `LP_T0` on.
pub fn point_names(pe: &Pe) -> Option<Vec<String>> {
    let (names, first) = pe.enum_names("WM_POINTER_LP_T0")?;
    Some(names[first..].iter().take_while(|n| n.starts_with("WM_POINTER_")).map(|n| n["WM_POINTER_".len()..].to_string()).collect())
}

/// The placement tag that marks a world map point on the world map's field (`A15_WMAP.opl2`):
/// `LP_T0` the town's mark (`T0`), `LP_A0` ... a region's big mark (`A0`), `SP_A0P0` ... a
/// small point (`A0p0`). The region entries `SP_A0` ... have no mark of their own (none).
pub fn point_tag(name: &str) -> Option<String> {
    if let Some(region) = name.strip_prefix("LP_") {
        return Some(region.to_string());
    }
    let rest = name.strip_prefix("SP_")?;
    let (region, n) = rest.split_once('P')?;
    Some(format!("{region}p{n}"))
}

/// Where a world map point stands (field units), from the world map's placements: the first
/// placement tagged with the point's tag.
pub fn point_position(placements: &barrel_formats::opl2::Placements, name: &str) -> Option<[f32; 3]> {
    let tag = point_tag(name)?;
    placements.objects.iter().flat_map(|t| t.instances.iter()).find(|i| i.tags.split_whitespace().any(|t| t == tag) || i.tags == tag).map(|i| i.position)
}

/// A route's dots on the world map (placements tagged `tag`: `R0` ... for the routes between
/// places, `A0r0` ... for the paths inside a region), and the two marks nearest the ends of
/// their chain (the two dots farthest apart): which points the route joins, as its dots show it
/// (the game's own table for this is not read). Marks are the points' tags (`T0`, `A0`,
/// `A0p0` ...).
pub fn route_ends(placements: &barrel_formats::opl2::Placements, tag: &str) -> Option<(String, String, usize)> {
    let instances: Vec<&barrel_formats::opl2::Instance> = placements.objects.iter().flat_map(|t| t.instances.iter()).collect();
    let has = |i: &barrel_formats::opl2::Instance, t: &str| i.tags.split_whitespace().any(|x| x == t);
    let dots: Vec<[f32; 3]> = instances.iter().filter(|i| has(i, tag)).map(|i| i.position).collect();
    let d2 = |a: [f32; 3], b: [f32; 3]| (a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2);
    let (mut a, mut b, mut best) = (*dots.first()?, *dots.first()?, -1.0f32);
    for &p in &dots {
        for &q in &dots {
            if d2(p, q) > best {
                (a, b, best) = (p, q, d2(p, q));
            }
        }
    }
    let marks = marks(placements);
    let nearest = |p: [f32; 3]| marks.iter().min_by(|x, y| d2(x.1, p).total_cmp(&d2(y.1, p))).map(|m| m.0.clone());
    Some((nearest(a)?, nearest(b)?, dots.len()))
}

/// The marks' positions on the world map: each point mark's tag (`T0`, `A0`, `A0p0` ...) and
/// where it stands.
pub fn marks(placements: &barrel_formats::opl2::Placements) -> Vec<(String, [f32; 3])> {
    let is_mark = |t: &str| t == "T0" || (t.len() == 2 && t.as_bytes()[0].is_ascii_uppercase() && t.as_bytes()[0] != b'R' && t.as_bytes()[1].is_ascii_digit()) || (t.len() >= 4 && t.as_bytes()[2] == b'p');
    let mut out: Vec<(String, [f32; 3])> = Vec::new();
    for i in placements.objects.iter().flat_map(|t| t.instances.iter()) {
        for t in i.tags.split_whitespace().filter(|t| is_mark(t)) {
            if !out.iter().any(|(k, _)| k == t) {
                out.push((t.to_string(), i.position));
            }
        }
    }
    out
}

/// A route's dots in order from the end nearest `from` (a position): each next dot the nearest
/// of those left, as the dots run along it.
pub fn route_path(placements: &barrel_formats::opl2::Placements, tag: &str, from: [f32; 3]) -> Vec<[f32; 3]> {
    let mut left: Vec<[f32; 3]> =
        placements.objects.iter().flat_map(|t| t.instances.iter()).filter(|i| i.tags.split_whitespace().any(|x| x == tag)).map(|i| i.position).collect();
    let d2 = |a: [f32; 3], b: [f32; 3]| (a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2);
    let mut path = Vec::with_capacity(left.len());
    let mut at = from;
    while let Some(k) = (0..left.len()).min_by(|&a, &b| d2(left[a], at).total_cmp(&d2(left[b], at))) {
        at = left.swap_remove(k);
        path.push(at);
    }
    path
}

#[cfg(test)]
mod point_tests {
    #[test]
    fn point_names_become_tags() {
        assert_eq!(super::point_tag("LP_T0").as_deref(), Some("T0"));
        assert_eq!(super::point_tag("LP_A0").as_deref(), Some("A0"));
        assert_eq!(super::point_tag("SP_A0P3").as_deref(), Some("A0p3"));
        assert_eq!(super::point_tag("SP_A0"), None);
    }

    #[test]
    #[ignore = "needs the game's executable (BARREL_EXE)"]
    fn arrivals_match_the_live_reads() {
        let Ok(path) = std::env::var("BARREL_EXE") else { return };
        let exe = std::fs::read(path).unwrap();
        let pe = barrel_formats::pe::Pe::parse(&exe).unwrap();
        let all = super::arrivals(&pe, super::ARRIVALS_EN).unwrap();
        assert_eq!((all[0].map, all.last().unwrap().map), (0, 107));
        let one = super::arrival(&pe, 26).unwrap();
        assert_eq!((one.facing, one.position), (180.0, [53.32, 0.0, 102.8]));
        assert_eq!(super::arrival(&pe, 25).unwrap().position, [129.0, 0.0, 200.0]);
    }

    #[test]
    #[ignore = "needs the game's executable (BARREL_EXE)"]
    fn the_southern_ruins_places_link_up() {
        let Ok(path) = std::env::var("BARREL_EXE") else { return };
        let exe = std::fs::read(path).unwrap();
        let pe = barrel_formats::pe::Pe::parse(&exe).unwrap();
        let all = super::places(&pe, super::PLACES_EN).unwrap();
        assert_eq!((all[0].name.as_str(), all[0].tag.as_str()), ("Colseit", "T0"));
        let entrance = all.iter().find(|p| p.tag == "A2p0").unwrap();
        assert_eq!((entrance.name.as_str(), entrance.description.as_str()), ("Ruins Entrance", "A path leading into the ruins."));
        let one = all.iter().find(|p| p.tag == "A2p1").unwrap();
        assert_eq!(one.links, [27, 25, 26, -1, -1, -1]);
        // The link between them is shared.
        assert!(entrance.links.contains(&25));
        let routes = super::areas(&pe, super::AREAS_EN).unwrap();
        assert_eq!([24, 25, 26, 27].map(|k| routes[k].id.clone()), ["A2r0", "A2r1", "A2r2", "A2r3"]);
    }
}
