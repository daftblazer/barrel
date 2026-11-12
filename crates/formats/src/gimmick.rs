//! Each field area's gimmicks (English Escha executable): a table of 96 records of 0x40 bytes
//! at [`AREAS_EN`], one per field area (record 0 is `AREA_01_01`, the Twilight Forest's
//! entrance), each pointing to a -1-ended list of 0x60-byte gimmicks. The field copies them
//! into its runtime object (+0x10e8 + 0x40 × area). See docs/tables.md, "Gimmicks".

use crate::pe::Pe;

pub const AREAS_EN: u64 = 0x1_4077_0230;
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
}

pub fn areas(pe: &Pe, table: u64) -> Option<Vec<Area>> {
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
    for k in 0..AREA_COUNT {
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
