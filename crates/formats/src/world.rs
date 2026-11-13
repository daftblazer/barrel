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
