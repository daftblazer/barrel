//! The world map's areas (English Escha executable): 85 records of 0x58 bytes at
//! [`AREAS_EN`], the routes between places (`R0` to `R12`) and the paths inside fields
//! (`A0r0` ...). See docs/tables.md, "World map".

use crate::pe::Pe;

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
pub fn point_position(placements: &crate::opl2::Placements, name: &str) -> Option<[f32; 3]> {
    let tag = point_tag(name)?;
    placements.objects.iter().flat_map(|t| t.instances.iter()).find(|i| i.tags.split_whitespace().any(|t| t == tag) || i.tags == tag).map(|i| i.position)
}

/// A route's dots on the world map (placements tagged `tag`: `R0` ... for the routes between
/// places, `A0r0` ... for the paths inside a region), and the two marks nearest the ends of
/// their chain (the two dots farthest apart): which points the route joins, as its dots show it
/// (the game's own table for this is not read). Marks are the points' tags (`T0`, `A0`,
/// `A0p0` ...).
pub fn route_ends(placements: &crate::opl2::Placements, tag: &str) -> Option<(String, String, usize)> {
    let instances: Vec<&crate::opl2::Instance> = placements.objects.iter().flat_map(|t| t.instances.iter()).collect();
    let has = |i: &crate::opl2::Instance, t: &str| i.tags.split_whitespace().any(|x| x == t);
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
    // The marks: tags of the form T0, A0 (region), A0p0 (small point).
    let is_mark = |t: &str| t == "T0" || (t.len() == 2 && t.as_bytes()[0].is_ascii_uppercase() && t.as_bytes()[0] != b'R' && t.as_bytes()[1].is_ascii_digit()) || (t.len() >= 4 && t.as_bytes()[2] == b'p');
    let marks: Vec<(&str, [f32; 3])> = instances.iter().flat_map(|i| i.tags.split_whitespace().filter(|t| is_mark(t)).map(move |t| (t, i.position))).collect();
    let nearest = |p: [f32; 3]| marks.iter().min_by(|x, y| d2(x.1, p).total_cmp(&d2(y.1, p))).map(|m| m.0.to_string());
    Some((nearest(a)?, nearest(b)?, dots.len()))
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
}
