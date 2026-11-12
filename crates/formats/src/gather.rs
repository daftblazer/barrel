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
        });
        at += RECORD;
    }
    Some(out)
}
