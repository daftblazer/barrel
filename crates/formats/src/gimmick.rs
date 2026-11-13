//! Each map's gimmicks (English Escha executable): a table of 122 records of 0x40 bytes at
//! [`MAPS_EN`], one per map number: the 12 town maps first ([`TOWN_MAPS`], region 0: the
//! atelier to the hallway), then the 96 field areas from [`AREAS_EN`] (map 12 is `AREA_01_01`,
//! the Twilight Forest's entrance), ten placeholders, the world map and three test maps. Each
//! points to a -1-ended list of 0x60-byte gimmicks. The field's runtime object holds them by map
//! number (+0x10e8 + 0x40 × map). See docs/tables.md, "Gimmicks".

use crate::pe::Pe;

pub const MAPS_EN: u64 = 0x1_4076_ff30;
pub const MAP_COUNT: usize = 122;
/// The town maps (0 to 11), whose region (the record's first word) is 0.
pub const TOWN_MAPS: usize = 12;
/// The first field area's record (map [`TOWN_MAPS`]).
pub const AREAS_EN: u64 = MAPS_EN + (TOWN_MAPS * AREA) as u64;
pub const AREA_COUNT: usize = 96;
const AREA: usize = 0x40;
const GIMMICK: usize = 0x60;

/// The `FIELDMAP_GIMMICK_TYPE_*` names, in enum order (the executable's strings).
pub const TYPES: [&str; 40] = [
    "ITEM_BOX", "ITEM_D", "ITEM_D_MULTIPLE", "FIELDMAP_CHANGE", "WORLDMAP_CHANGE", "MESSAGE_BOX", "MESSAGE", "EVENT", "EXEC_EVENT", "MIX",
    "SHOP", "SAVE_AND_LOAD", "IN_CHANGE", "EFF_CIRCLE_A", "EFF_CIRCLE_B", "SAVE_BASE_H", "SAVE_EFF_OFF_H", "SAVE_EFF_ON_H", "WARP",
    "MAP_CHANGE_INFO", "CALENDAR", "TARU", "ITEM_TRADE", "ROCK", "WELL_WATER", "FIELDMAP_CHANGE_ANIM", "FIELDMAP_CHANGE_EX",
    "WORLDMAP_CHANGE_EX", "ENEMY_FORCE_ATTACK", "MESSAGE_ITEM_GET", "CALL", "UNI", "MESSAGE_ICON", "CONTAINER", "BOX_KABE", "POSTER",
    "BREAKABLE_OBJECT", "TRAIN", "BUDGET", "MENU_EQUIP",
];

/// A field area's record.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// Its place name and its area name ("Forest Entrance", "Twilight Forest 1").
    pub place: String,
    pub name: String,
    /// The six words before the list pointer, not read yet (the first is the region:
    /// 1 for the Twilight Forest, 2 the Old Highway, 3 the Southern Ruins, ...).
    pub words: [i32; 6],
    pub gimmicks: Vec<Gimmick>,
}

/// A gimmick.
#[derive(Clone, Debug, PartialEq)]
pub struct Gimmick {
    /// Its `FIELDMAP_GIMMICK_TYPE_*` number ([`TYPES`]).
    pub kind: i32,
    /// Word 1 (-1 in most).
    pub id: i32,
    /// The float at +8 (−10, 0, 50 ...; a turn in degrees, by the values).
    pub angle: f32,
    /// Word 3 (100 in most).
    pub word3: i32,
    /// Where it stands (+0x10, metres).
    pub position: [f32; 3],
    /// Three floats at +0x1c (3, 10, 3 for the map exits; 1.5, 2, 1.5 for the breakables).
    pub size: [f32; 3],
    /// The events it appears after and until (+0x28, +0x2c, `0x14035f2d0`; -1 for none).
    pub events: (i32, i32),
    /// The rest, raw (+0x30 to +0x60).
    pub rest: [u8; 0x30],
}

impl Gimmick {
    pub fn kind_name(&self) -> &'static str {
        TYPES.get(self.kind as usize).copied().unwrap_or("?")
    }

    /// Whether a point (metres) stands inside the gimmick's area, as the field tests it
    /// (`0x140281770`): turned by minus its angle about y, its offset from the gimmick within
    /// half its size along x and along z (the height is not tested). A gimmick whose size has a
    /// negative part takes its type's default size (`0x140dfc1c8` + 0x20, 48 bytes a type,
    /// floats +0xc to +0x14), not read here: such a gimmick contains nothing. (The turn's
    /// direction is `0x1400c2930`'s, taken as the usual right-handed one.)
    pub fn contains(&self, p: [f32; 3]) -> bool {
        if self.size.iter().any(|&v| v < 0.0) {
            return false;
        }
        let (dx, dz) = (p[0] - self.position[0], p[2] - self.position[2]);
        let (s, c) = self.angle.to_radians().sin_cos();
        let (x, z) = (c * dx - s * dz, s * dx + c * dz);
        x.abs() <= self.size[0] * 0.5 && z.abs() <= self.size[2] * 0.5
    }
}

/// `count` records from `table` ([`MAPS_EN`] with [`MAP_COUNT`] for every map, [`AREAS_EN`] with
/// [`AREA_COUNT`] for the field areas).
pub fn areas(pe: &Pe, table: u64, count: usize) -> Option<Vec<Area>> {
    let at = pe.offset(table)?;
    let text = |va: u64| -> String {
        if va == 0 {
            return String::new();
        }
        let Some(o) = pe.offset(va) else { return String::new() };
        let end = pe.data[o..].iter().position(|&b| b == 0).unwrap_or(0);
        String::from_utf8_lossy(&pe.data[o..o + end]).into_owned()
    };
    let mut out = Vec::new();
    for k in 0..count {
        let r = pe.data.get(at + AREA * k..at + AREA * (k + 1))?;
        let q = |o: usize| u64::from_le_bytes(r[o..o + 8].try_into().unwrap());
        let words: [i32; 6] = std::array::from_fn(|i| i32::from_le_bytes(r[4 * i..4 * i + 4].try_into().unwrap()));
        let mut gimmicks = Vec::new();
        if let Some(mut g) = pe.offset(q(0x20)) {
            while let Some(e) = pe.data.get(g..g + GIMMICK) {
                let w = |o: usize| i32::from_le_bytes(e[o..o + 4].try_into().unwrap());
                let f = |o: usize| f32::from_le_bytes(e[o..o + 4].try_into().unwrap());
                if w(0) == -1 || gimmicks.len() > 256 {
                    break;
                }
                gimmicks.push(Gimmick {
                    kind: w(0),
                    id: w(4),
                    angle: f(8),
                    word3: w(0xc),
                    position: [f(0x10), f(0x14), f(0x18)],
                    size: [f(0x1c), f(0x20), f(0x24)],
                    events: (w(0x28), w(0x2c)),
                    rest: e[0x30..0x60].try_into().unwrap(),
                });
                g += GIMMICK;
            }
        }
        out.push(Area { place: text(q(0x30)), name: text(q(0x38)), words, gimmicks });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_box_contains_along_its_turn() {
        let g = Gimmick { kind: 19, id: -1, angle: 90.0, word3: 100, position: [10.0, 0.0, 10.0], size: [6.0, 10.0, 2.0], events: (-1, -1), rest: [0; 0x30] };
        // Turned a quarter, its long side (x, 6) lies along z.
        assert!(g.contains([10.0, 50.0, 12.5]));
        assert!(!g.contains([12.5, 0.0, 10.0]));
        let square = Gimmick { angle: 0.0, size: [3.0, 10.0, 3.0], ..g.clone() };
        assert!(square.contains([11.4, 0.0, 8.6]) && !square.contains([11.6, 0.0, 10.0]));
    }
}
