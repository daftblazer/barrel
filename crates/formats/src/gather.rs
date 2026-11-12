//! What gathering gives (English Escha executable): a table of 64-byte records at
//! [`GATHER_EN`], one per item a gathering list can give, the lists numbered 0 to 695. The
//! area map shows a list's items: Southern Ruins Sector One's is list 71 (Moss Grass, an
//! unknown item 12, Mountain Feather, Eagle Feather). See docs/tables.md, "Gathering".

use crate::pe::Pe;

pub const GATHER_EN: u64 = 0x1_4088_a060;
const RECORD: usize = 0x40;

/// One item a gathering list can give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gather {
    /// The list it belongs to.
    pub list: u32,
    /// Its `itemData` row.
    pub item: i32,
    /// Its quality range, when the record gives one (0, 0 otherwise).
    pub quality: (i32, i32),
    /// Words 5 to 7, not read yet (0 to 3, then 1 to 10, then 4 to 20 where set).
    pub unread: [i32; 3],
    /// Its weight in the list's draw (a list's weights add up to 100 in the lists checked).
    pub weight: i32,
    /// The traits it comes with (`item_potential.xml` rows), up to four.
    pub traits: Vec<i32>,
    /// Word 13: -1 in all but two; random traits are drawn only when it is -1 (`0x14023d8c0`).
    pub word13: i32,
}

/// The records, from the table's start while they look like records (list number not
/// going down, item a row or -1, weight 0 to 1,000).
pub fn gathering(pe: &Pe, table: u64) -> Option<Vec<Gather>> {
    let mut at = pe.offset(table)?;
    let mut out: Vec<Gather> = Vec::new();
    while let Some(r) = pe.data.get(at..at + RECORD) {
        let w = |k: usize| i32::from_le_bytes(r[4 * k..4 * k + 4].try_into().unwrap());
        let list = w(0);
        let ok = list >= 0 && out.last().is_none_or(|g| list as u32 >= g.list) && (-1..1000).contains(&w(1)) && (0..=1000).contains(&w(8));
        if !ok {
            break;
        }
        out.push(Gather {
            list: list as u32,
            item: w(1),
            quality: (w(3), w(4)),
            unread: [w(5), w(6), w(7)],
            weight: w(8),
            traits: (9..13).map(w).filter(|&t| t >= 0).collect(),
            word13: w(13),
        });
        at += RECORD;
    }
    Some(out)
}

/// The gathering points (English Escha): 356 records of 88 bytes at [`POINTS_EN`], copied with
/// the lists into one object by `0x14023b8f0`.
pub const POINTS_EN: u64 = 0x1_4088_25a0;
pub const POINT_COUNT: usize = 356;
const POINT: usize = 88;

/// A gathering point.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// Up to seven lists it draws from (five in most, the same one five times in some): which
    /// one applies when is not read yet (story stages, by the items: list 13, then 17, 21, 25
    /// give the same plants a little better each time).
    pub lists: Vec<u32>,
    /// Words 7 and 8 (1 and 1 in most), not read yet.
    pub flags: (i32, i32),
    /// The field map it stands in (12 is `AREA_01_01`; -1 for some).
    pub map: i32,
    /// Its byte +0x30 (word 12): set, its list goes by the alchemy table level, else by the
    /// gathering one (`0x1400d3160`).
    pub alchemy: bool,
    /// Where it stands (metres).
    pub position: [f32; 3],
    /// The two float triples after it (0.01 three times, then 2.0 three times, in most).
    pub scale: [f32; 3],
    pub size: [f32; 3],
}

pub fn points(pe: &Pe, table: u64) -> Option<Vec<Point>> {
    let at = pe.offset(table)?;
    let data = pe.data.get(at..at + POINT_COUNT * POINT)?;
    Some(
        data.chunks_exact(POINT)
            .map(|r| {
                let w = |k: usize| i32::from_le_bytes(r[4 * k..4 * k + 4].try_into().unwrap());
                let f = |k: usize| f32::from_le_bytes(r[4 * k..4 * k + 4].try_into().unwrap());
                Point {
                    lists: (0..7).map(w).filter(|&l| l >= 0).map(|l| l as u32).collect(),
                    flags: (w(7), w(8)),
                    map: w(9),
                    alchemy: r[0x30] != 0,
                    position: [f(13), f(14), f(15)],
                    scale: [f(16), f(17), f(18)],
                    size: [f(19), f(20), f(21)],
                }
            })
            .collect(),
    )
}
