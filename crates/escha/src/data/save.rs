//! Escha & Logy DX saves (`Documents/KoeiTecmo/Atelier Escha and Logy DX/SAVEDATA/
//! GAMEDATA<NN>.pcsave`, the game's `ROOT:savedata/GAMEDATA%02d`).
//!
//! ```text
//! 0x000  0x100 plain bytes, the summary the load screen shows:
//!        +0x28.. i32s (what each is is not read yet), +0x74.. -1 padding,
//!        +0xa4 the time it was saved, a Windows SYSTEMTIME (8 × u16: year, month,
//!        day of the week, day, hour, minute, second, milliseconds)
//! 0x100  the game state, coded ([`decode`])
//! ```
//!
//! `SYSDATA.pcsave` (the settings) is coded from its first byte. The coding
//! (`0x1400219a0`, decoding with its last argument 0) is undone by [`decode`]; see
//! `docs/formats.md`, "Saves".

use barrel_formats::save_code::{self, gamma, substitute, ungamma, unsubstitute, Keys};
use barrel_formats::Error;

pub const HEADER: usize = 0x100;

/// When a save was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedAt {
    pub year: u16,
    pub month: u16,
    pub weekday: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
}

/// A save's plain header and its encrypted body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save<'a> {
    /// The header's i32 words from +0x28 (summary values).
    pub summary: Vec<i32>,
    pub saved_at: SavedAt,
    pub body: &'a [u8],
}

pub fn parse(d: &[u8]) -> Result<Save<'_>, Error> {
    if d.len() < HEADER {
        return Err(Error::Corrupt("save shorter than its header"));
    }
    let u16_at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let w: Vec<u16> = (0..8).map(|k| u16_at(0xa4 + 2 * k)).collect();
    let saved_at = SavedAt { year: w[0], month: w[1], weekday: w[2], day: w[3], hour: w[4], minute: w[5], second: w[6], millisecond: w[7] };
    if !(1..=12).contains(&saved_at.month) || !(1..=31).contains(&saved_at.day) {
        return Err(Error::NotThisFormat);
    }
    let summary = (0x28..0x74).step_by(4).map(|o| i32::from_le_bytes(d[o..o + 4].try_into().unwrap())).collect();
    Ok(Save { summary, saved_at, body: &d[HEADER..] })
}

/// Escha & Logy DX's keys for the saves' coding.
pub const KEYS: Keys = Keys { words: 0xd727, modulus: 0xec2, threshold: 0x761, runs: [0xa22f, 0xa129, 0x394d] };

/// Decodes a coded save body (a `GAMEDATA` file after its header, or all of `SYSDATA`): the
/// shared coding opened with this game's keys ([`barrel_formats::save_code::open`]), then
/// [`ungamma`] and [`unsubstitute`].
pub fn decode(coded: &[u8]) -> Result<Vec<u8>, Error> {
    let mut state = unsubstitute(&ungamma(&save_code::open(coded, &KEYS)?)?)?;
    // Encoding drops the state's trailing zero bytes (`0x140021a1f`): pad them back so the last
    // top record is whole (each top record's size read in turn after the 0x20-byte header).
    let mut end = 0x20;
    while let Some(size) = state.get(end..end + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize) {
        if size == 0 || size > 1 << 24 {
            break;
        }
        end += size;
    }
    if end > state.len() {
        state.resize(end, 0);
    }
    Ok(state)
}

/// Codes a state as the game does when saving (`0x1400219a0` with encode 1), the inverse of
/// [`decode`]: trailing zero bytes dropped, [`substitute`] and [`gamma`], then the shared
/// coding ([`barrel_formats::save_code::seal`]). `seed` stands for the game's `0x1403b7390`
/// value (any value reads back). The output differs from the game's for one state (its table
/// order), but [`decode`] gives the state back.
pub fn encode(state: &[u8], seed: u32) -> Vec<u8> {
    let len = state.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    save_code::seal(&gamma(&substitute(&state[..len])), seed, &KEYS)
}

/// A record of a save's state (after its 0x20-byte header): a little-endian u32 size counting
/// itself, a NUL-terminated name, then one of
/// - `Data`: a u32 length and that many bytes (most records; little-endian in Escha & Logy,
///   big-endian in Shallie);
/// - `Group`: records that fill the rest exactly (`system_data`, `option`, ...), or a u32
///   count and that many records (`party`);
/// - `Raw`: anything else, kept as it is (some records are written by their own code:
///   `FLAG` holds two bytes and a `CLEAR` record of two bytes, `m_recipeData` a byte and a
///   count of 265 entries, `costumeData` a count and length-value pairs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record<'a> {
    pub name: &'a str,
    pub body: Body<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body<'a> {
    Data(&'a [u8]),
    Group(Vec<Record<'a>>),
    Raw(&'a [u8]),
}

impl<'a> Record<'a> {
    /// The child record of that name, in a group.
    pub fn child(&self, name: &str) -> Option<&Record<'a>> {
        match &self.body {
            Body::Group(c) => c.iter().find(|r| r.name == name),
            _ => None,
        }
    }
}

/// The records that fill `d` exactly, or `None`.
fn records(d: &[u8]) -> Option<Vec<Record<'_>>> {
    let mut out = Vec::new();
    let mut o = 0;
    while o < d.len() {
        let size = u32::from_le_bytes(d.get(o..o + 4)?.try_into().unwrap()) as usize;
        let rec = d.get(o..o.checked_add(size)?)?;
        let z = rec.get(4..)?.iter().position(|&b| b == 0)? + 4;
        let name = std::str::from_utf8(&rec[4..z]).ok().filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))?;
        let rest = &rec[z + 1..];
        // The data length is little-endian in Escha's saves, big-endian in Shallie's.
        let fits = |l: &[u8]| {
            let l: [u8; 4] = l.try_into().unwrap();
            [u32::from_le_bytes(l), u32::from_be_bytes(l)].iter().any(|&n| n as usize == rest.len() - 4)
        };
        let body = match rest.get(..4) {
            Some(l) if fits(l) => Body::Data(&rest[4..]),
            _ => match records(rest) {
                Some(children) if !rest.is_empty() => Body::Group(children),
                // A u32 count, then that many records and maybe more (`party`: 11 `Party` records
                // and `m_squad`).
                _ => match rest.get(..4).map(|c| u32::from_le_bytes(c.try_into().unwrap()) as usize).zip(rest.get(4..).and_then(records)) {
                    Some((n, children)) if n > 0 && n <= children.len() => Body::Group(children),
                    _ => Body::Raw(rest),
                },
            },
        };
        out.push(Record { name, body });
        o += size;
    }
    Some(out)
}

/// The places' states, the `pointerStatus` record (96 × 12 bytes, `0x14035e200` loads it):
/// per place the obstacle's durability left (+0, set from the place table's `[1, N]`), whether
/// it is still shut (+4, a byte: set for the table's `[0, N]` places until their gimmick is
/// used) and a word (+8, not read yet). See `area_map::closed`.
pub fn place_states(d: &[u8]) -> Vec<(i32, bool, i32)> {
    d.chunks_exact(12).map(|c| (i32::from_le_bytes(c[0..4].try_into().unwrap()), c[4] != 0, i32::from_le_bytes(c[8..12].try_into().unwrap()))).collect()
}

/// The field effect, the `fieldmap/m_userDataFieldMapData` record (9 words): a word (0 in the
/// user's save), the gauge (the HUD's percentage, 45), the effect's mode (4: the gathering
/// tier raised by the first value) and values (2, then -1s). Returns (gauge, mode, values).
pub fn field_effect(d: &[u8]) -> Option<(i32, i32, [i32; 3])> {
    let w = |k: usize| d.get(4 * k..4 * k + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    Some((w(1)?, w(2)?, [w(3)?, w(4)?, w(5)?]))
}

/// One held item (36 bytes; the basket, container and material lists are runs of these):
/// a serial number (+0; the `item/ID` record holds the next), the `itemData.xml` row (+2),
/// the quality as a float (+4), three trait rows (+8, -1 none), a word the battle code checks
/// (+0xe), four effect rows (+0x10), the equip cost (+0x18: the squares it takes on the battle
/// item grid, `0x14029f030`), a word (+0x1a, taken off the cost) and the use counts (+0x1c the
/// uses left, which the battle code reads, = +0x20 + +0x1e; +0x1e the uses from its recipe and
/// traits, `0x14029ef10`; +0x20, and +0x22 the stack size, 1 to 34 in a save, which the item counts sum). A serial of 0xffff is an empty slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemUnit {
    pub serial: u16,
    pub item: i16,
    pub quality: f32,
    pub traits: [i16; 3],
    pub word_e: u16,
    pub effects: [i16; 4],
    pub equip_cost: i16,
    pub word_1a: i16,
    pub counts: [i16; 4],
}

pub const ITEM_UNIT: usize = 36;

/// The items of a list record (empty slots left out).
pub fn item_units(d: &[u8]) -> Vec<ItemUnit> {
    let i16_at = |c: &[u8], o: usize| i16::from_le_bytes([c[o], c[o + 1]]);
    d.chunks_exact(ITEM_UNIT)
        .filter(|c| u16::from_le_bytes([c[0], c[1]]) != 0xffff)
        .map(|c| ItemUnit {
            serial: u16::from_le_bytes([c[0], c[1]]),
            item: i16_at(c, 2),
            quality: f32::from_le_bytes(c[4..8].try_into().unwrap()),
            traits: [i16_at(c, 8), i16_at(c, 10), i16_at(c, 12)],
            word_e: u16::from_le_bytes([c[14], c[15]]),
            effects: [i16_at(c, 16), i16_at(c, 18), i16_at(c, 20), i16_at(c, 22)],
            equip_cost: i16_at(c, 24),
            word_1a: i16_at(c, 26),
            counts: [i16_at(c, 28), i16_at(c, 30), i16_at(c, 32), i16_at(c, 34)],
        })
        .collect()
}

/// A character's record in `party` (a `Party` record, 698 bytes after its name): the
/// character (+0), level (+4), experience (+8), a word (+0xc, 392 in every one seen), HP and
/// MP now (+0x10, +0x14), then the base stats: max HP, max MP, attack, defense, speed
/// (+0x18 to +0x28), five small values (+0x2c, resistances presumably) and two constants (5,
/// 25); the same block with equipment applied at +0x114 (max HP from +0x118); the skills in
/// command order from +0x230 (ten 12-byte entries: skill row, 0, 1), the Move skill
/// (+0x2a8), the level again (+0x2ac) and the friendship float (+0x2b0, the calendar's daily
/// gain; the record is the runtime one, whose +0x2a4 it is, with 12 bytes in front). The
/// equipment is four item units (weapon, armour, two accessories) at +0x19c, after a 144 (their
/// size in bytes); a unit with no item has `itemData` row -1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub character: i32,
    pub level: i32,
    pub exp: i32,
    pub hp: i32,
    pub mp: i32,
    pub max_hp: i32,
    pub max_mp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    /// Max HP, max MP, attack, defense, speed with equipment.
    pub equipped: [i32; 5],
    /// The whole in-use stat block (`equipment::STATS` words from +0x118, `0x14032de50`): the
    /// five above, then the resistances, critical stats, support gauge and the rest, with
    /// equipment and passives counted.
    pub in_use: [i32; 32],
    /// The skill rows of commands 0 to 9.
    pub skills: [i32; 10],
    pub friendship: f32,
    /// The `itemData` rows worn: weapon, armour, two accessories (-1 none).
    pub equipment: [i32; 4],
    /// The same four item units whole (quality, effects, traits), `None` for an empty slot.
    pub units: [Option<ItemUnit>; 4],
}

/// Where a `Party` record's four equipped item units start.
const EQUIPMENT: usize = 0x19c;

pub fn member(d: &[u8]) -> Option<Member> {
    let w = |k: usize| d.get(4 * k..4 * k + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    let f = d.get(0x2b0..0x2b4).map(|b| f32::from_le_bytes(b.try_into().unwrap()))?;
    Some(Member {
        character: w(0)?,
        level: w(1)?,
        exp: w(2)?,
        hp: w(4)?,
        mp: w(5)?,
        max_hp: w(6)?,
        max_mp: w(7)?,
        attack: w(8)?,
        defense: w(9)?,
        speed: w(10)?,
        equipped: [w(0x46)?, w(0x47)?, w(0x48)?, w(0x49)?, w(0x4a)?],
        in_use: std::array::from_fn(|k| w(0x46 + k).unwrap_or(0)),
        skills: std::array::from_fn(|k| w(0x8c + 3 * k).unwrap_or(-1)),
        friendship: f,
        units: std::array::from_fn(|k| d.get(EQUIPMENT + k * ITEM_UNIT..EQUIPMENT + (k + 1) * ITEM_UNIT).and_then(|u| item_units(u).into_iter().next())),
        equipment: std::array::from_fn(|k| d.get(EQUIPMENT + k * ITEM_UNIT + 2..EQUIPMENT + k * ITEM_UNIT + 4).map_or(-1, |b| i32::from(i16::from_le_bytes([b[0], b[1]])))),
    })
}

/// A request board slot (`quest/now`, `player`, `next`: ten 52-byte slots each, laid out as the
/// game's at `0x1410985d8`, filled by `0x14033ab30`): the kind (+0: 0 synthesis, 1 monster,
/// 2 gathering), the target (+4: an `itemData` row, or a monster number counted from the first
/// `MONSTER_` name), how many are wanted (+0xc, at least 1; 0 an empty slot) and done so far
/// (+0x10), the day posted and the assignment's deadline (+0x14, +0x18), a flag the posting
/// sets to 1 (+0x1c), the points (+0x20) and two prizes as an item row and a count (+0x24 ..
/// +0x30), all rolled (`quest::rewards`). The request is done when the progress reaches the
/// wanted count (`0x14033b1c0`) and pays the points truncated and each prize counted 1 or more
/// (`0x14033b220`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardSlot {
    pub kind: i32,
    pub target: i32,
    pub wanted: i32,
    pub progress: i32,
    pub posted: i32,
    pub until: i32,
    pub flag: i32,
    pub value: f32,
    pub prizes: [(i32, f32); 2],
}

pub fn board_slots(d: &[u8]) -> Vec<Option<BoardSlot>> {
    let i = |c: &[u8], k: usize| i32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap());
    let f = |c: &[u8], k: usize| f32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap());
    d.chunks_exact(52)
        .map(|c| {
            (i(c, 3) != 0).then(|| BoardSlot {
                kind: i(c, 0),
                target: i(c, 1),
                wanted: i(c, 3),
                progress: i(c, 4),
                posted: i(c, 5),
                until: i(c, 6),
                flag: i(c, 7),
                value: f(c, 8),
                prizes: [(i(c, 9), f(c, 10)), (i(c, 11), f(c, 12))],
            })
        })
        .collect()
}

/// An event's record (`event/m_eventListSaveData`: 1,165 of 36 bytes, one per `EVENT_*` name
/// in the executable's enum order; at runtime from `0x1410a42d0`): whether it has been seen
/// (+0), the day it was (+4, -31 never), how many times (+8), the day's roll from 0 to 100
/// (+0x10, `0x1410a42e0` + 0x24 × the event: the calendar's daily rolls) and a value (+0x14,
/// 0 to 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRecord {
    pub seen: bool,
    pub day: i32,
    pub count: i32,
    pub roll: i32,
    pub value: i32,
}

pub fn event_records(d: &[u8]) -> Vec<EventRecord> {
    let i = |c: &[u8], k: usize| i32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap());
    d.chunks_exact(36).map(|c| EventRecord { seen: i(c, 0) != 0, day: i(c, 1), count: i(c, 2), roll: i(c, 4), value: i(c, 5) }).collect()
}

/// What a game save's state says in plain numbers (records read so far).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    /// `money` (the header's fourth summary value too).
    pub money: Option<i32>,
    /// `mix_lv/m_lv`, `m_exp`: the alchemy level (the header's fifth value) and experience.
    pub alchemy: Option<(i32, i32)>,
    /// `date`'s first u32 (229 in one save: a day count, presumably).
    pub day: Option<u32>,
    /// `item/limitSize`: the basket's and the container's sizes (60, 999).
    pub limits: Option<(i32, i32)>,
    /// `item/m_unitKago`, `m_unitContainer`, `m_unitMaterial`.
    pub basket: Vec<ItemUnit>,
    pub container: Vec<ItemUnit>,
    pub materials: Vec<ItemUnit>,
    /// `explore_equip/explore_item_escha` and `explore_item_logy`: the items each alchemist
    /// takes into battle (25 slots each).
    pub battle_items: [Vec<ItemUnit>; 2],
    /// `explore_equip/explore_item_escha` and `_logy` as saved, empty slots kept (25 each):
    /// the item list index is the item's first square on the equipment grid.
    pub battle_item_slots: [Vec<Option<ItemUnit>>; 2],
    /// `explore_equip/explore_slot_escha` and `_logy`: the 25 grid squares, each the index of
    /// the item covering it, -1 free, 25 locked (past the capacity).
    pub battle_grid: [Vec<i32>; 2],
    /// `party`'s `Party` records.
    pub party: Vec<Member>,
    /// `party/m_squad`: the characters in the squad, in order (-1 slots left out).
    pub squad: Vec<i32>,
    /// `party/m_squad` as saved: its 11 slots (-1 empty), the list the daily friendship gain
    /// reads.
    pub squad_slots: Vec<i32>,
    /// `event/m_eventListSaveData`: the events, in `EVENT_*` order.
    pub events: Vec<EventRecord>,
    /// `event/m_eventFlag`: the 151 event flags (`0x1410932d0`, read by `0x14035f460`).
    pub event_flags: Vec<i32>,
    /// `quest/now`: the request board's ten slots.
    pub board: Vec<Option<BoardSlot>>,
    /// `quest/player`: the requests taken.
    pub taken: Vec<Option<BoardSlot>>,
    /// `subject/job_point_interval`: the rank points made in each of the nine periods (they
    /// sum to `job_point`).
    pub points_by_period: Vec<i32>,
    /// `monthrep/data`: the month reports, 0x144 bytes each (60).
    pub month_reports: Vec<Vec<u8>>,
    /// `quest/randseed`: the request board's seed offset (the calendar's `board_offset`).
    pub board_seed: Option<i32>,
    /// `global` +0x38: the protagonist the player chose, 0 Escha or 1 Logy (`Chara.xml` row,
    /// what "Master Character", row 749, stands for).
    pub hero: Option<i32>,
    /// `subject/subject_save`: each assignment's record (a `SubjectData` row each).
    pub subjects: Vec<crate::data::subject::SavedSubject>,
    /// `subject/job_rank` and `job_point`: the rank and its points.
    pub rank: Option<(i32, i32)>,
    /// `shop/shop_data`: four shops, each a u32 and 50 wares' (count, days, state), the
    /// calendar's `Stock`.
    pub shops: Vec<Vec<[i32; 3]>>,
    /// `knowledge`: how well the library knows each entry.
    pub knowledge: Knowledge,
    /// `mix_recipe`: per recipe (265, the recipe table's order), ten flags, one per effect
    /// in [`crate::data::recipe::Recipe::effect_list`]'s order: whether that effect is revealed. Kept
    /// at `0x1410993a0`, 10 bytes a recipe; saved as a count, then a length (10) and the bytes
    /// for each.
    pub recipe_effects: Vec<[bool; 10]>,
    /// `bonus/value`: 128 bonus values (`0x141079de0`, read through `0x14002f2f0`), mostly
    /// percentages added to 100 where they are used (docs/formats.md, "Bonuses").
    pub bonus_values: Vec<i32>,
    /// `bonus/skill`: a flag per synthesis skill record (`battle_data::mix_skills`, 68 used of
    /// 128): learnt.
    pub mix_skills_learnt: Vec<bool>,
    /// `bonus/mixvalue`: the 544 synthesis bonus totals (`bonus::mix_slot`).
    pub mix_bonus: Vec<i32>,
    /// `worldmap/knowledgeRoute` (the 85 area records) and `knowledgePointer` (the 96
    /// places): 0 nothing, 10 new, 15 all (the game's "Knowledge: ..." levels).
    pub route_knowledge: Vec<i32>,
    pub place_knowledge: Vec<i32>,
    /// `library`'s marks: some entry of the item, effect, trait, synthesis skill, book or monster
    /// lists is new (its bit clear, within the list's length; docs/formats.md, "Library marks").
    pub library_new: bool,
    /// `worldmap/chara`: two i32 (29 and 0 in one save: the place the party is at, presumably).
    pub world_position: Option<(i32, i32)>,
    /// `worldmap/pointerStatus`: per place, see [`place_states`].
    pub place_states: Vec<(i32, bool, i32)>,
    /// `fieldmap/m_userDataFieldMapData`: see [`field_effect`].
    pub field_effect: Option<(i32, i32, [i32; 3])>,
    /// `work/work_data_status`: per `WorkData.xml` row (103), -1 not open, 1 open and new, 0
    /// open, 2 done.
    pub work_status: Vec<i32>,
    /// `delivery/delivery_data`: per `DeliveryData.xml` row (71), the state (2 passed) and the
    /// score handed in so far.
    pub deliveries: Vec<(i32, i32)>,
}

/// The `knowledge` record: the library's level (0 to 10) for every entry of its eight lists,
/// as one block at `0x141079fe0` (0x24e4 bytes, zeroed by `0x1403692d0`). The levels only
/// rise (`0x1403692f0` and the others keep the higher one unless forced), and an item reaching
/// 2 or more posts a library notice. Item levels: 0 unknown, 2 seen (an item bought or
/// given), 10 known in full (an item made or gathered), 4 also seen in a save; recipes hold 5
/// and 6, books 4 and 10. The request board reads them: a synthesis request needs its item at
/// 6 or more, a battle request an entry at 2.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Knowledge {
    /// `item`: an `itemData` row each (560).
    pub item: Vec<i32>,
    /// `recipe`: the recipes (265).
    pub recipe: Vec<i32>,
    /// `book`: the recipe books (73).
    pub book: Vec<i32>,
    /// `fieldMap`: the field maps (122).
    pub field_map: Vec<i32>,
    /// `monster`: the monsters (153).
    pub monster: Vec<i32>,
    /// `mixEsp`: synthesis skills (90).
    pub mix_esp: Vec<i32>,
    /// `potential`: the traits (400).
    pub potential: Vec<i32>,
    /// `eff`: the effects (698).
    pub eff: Vec<i32>,
}

/// The lists of the library's marks (`library`'s bitsets) and how many entries each has.
const LIBRARY_MARKS: [(&str, usize); 6] = [("new_item", 488), ("new_eff", 698), ("new_potential", 400), ("new_property", 90), ("new_book", 73), ("new_monster", 153)];

pub fn summary(records: &[Record]) -> Summary {
    let top = |n: &str| records.iter().find(|r| r.name == n);
    let bytes = |r: Option<&Record<'_>>| -> Option<Vec<u8>> {
        match &r?.body {
            Body::Data(d) | Body::Raw(d) => Some(d.to_vec()),
            Body::Group(_) => None,
        }
    };
    let i32_at = |d: &[u8], o: usize| d.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    let item = top("item");
    let list = |n: &str| bytes(item.and_then(|r| r.child(n))).map(|d| item_units(&d)).unwrap_or_default();
    let mix = top("mix_lv");
    let library_new = LIBRARY_MARKS.iter().any(|&(name, entries)| {
        let marks = bytes(top("library").and_then(|r| r.child(name))).unwrap_or_default();
        (0..entries).any(|k| marks.get(k / 8).is_some_and(|b| b >> (k % 8) & 1 == 0))
    });
    Summary {
        library_new,
        money: bytes(top("money")).and_then(|d| i32_at(&d, 0)),
        alchemy: (|| Some((i32_at(&bytes(mix?.child("m_lv"))?, 0)?, i32_at(&bytes(mix?.child("m_exp"))?, 0)?)))(),
        day: bytes(top("date")).and_then(|d| i32_at(&d, 0)).map(|v| v as u32),
        limits: bytes(item.and_then(|r| r.child("limitSize"))).and_then(|d| Some((i32_at(&d, 0)?, i32_at(&d, 4)?))),
        basket: list("m_unitKago"),
        container: list("m_unitContainer"),
        materials: list("m_unitMaterial"),
        battle_item_slots: ["explore_item_escha", "explore_item_logy"].map(|n| {
            bytes(top("explore_equip").and_then(|r| r.child(n)))
                .map(|d| d.chunks_exact(ITEM_UNIT).map(|c| item_units(c).into_iter().next()).collect())
                .unwrap_or_default()
        }),
        battle_grid: ["explore_slot_escha", "explore_slot_logy"].map(|n| {
            bytes(top("explore_equip").and_then(|r| r.child(n)))
                .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
                .unwrap_or_default()
        }),
        battle_items: ["explore_item_escha", "explore_item_logy"].map(|n| bytes(top("explore_equip").and_then(|r| r.child(n))).map(|d| item_units(&d)).unwrap_or_default()),
        party: match top("party").map(|r| &r.body) {
            Some(Body::Group(c)) => c.iter().filter(|r| r.name == "Party").filter_map(|r| match &r.body {
                Body::Data(d) | Body::Raw(d) => member(d),
                Body::Group(_) => None,
            }).collect(),
            _ => Vec::new(),
        },
        board_seed: bytes(top("quest").and_then(|r| r.child("randseed"))).and_then(|d| i32_at(&d, 0)),
        // The global block (0x141098a10, 68 bytes) as is; the hero at 0x141098a48.
        hero: bytes(top("global")).and_then(|d| i32_at(&d, 0x38)),
        events: bytes(top("event").and_then(|r| r.child("m_eventListSaveData"))).map(|d| event_records(&d)).unwrap_or_default(),
        event_flags: bytes(top("event").and_then(|r| r.child("m_eventFlag")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        board: bytes(top("quest").and_then(|r| r.child("now"))).map(|d| board_slots(&d)).unwrap_or_default(),
        taken: bytes(top("quest").and_then(|r| r.child("player"))).map(|d| board_slots(&d)).unwrap_or_default(),
        subjects: bytes(top("subject").and_then(|r| r.child("subject_save"))).map(|d| crate::data::subject::saved(&d)).unwrap_or_default(),
        rank: (|| {
            let sub = top("subject")?;
            Some((i32_at(&bytes(sub.child("job_rank"))?, 0)?, i32_at(&bytes(sub.child("job_point"))?, 0)?))
        })(),
        shops: bytes(top("shop").and_then(|r| r.child("shop_data")))
            .map(|d| {
                d.chunks_exact(4 + 50 * 12)
                    .map(|shop| shop[4..].chunks_exact(12).map(|w| std::array::from_fn(|k| i32::from_le_bytes(w[4 * k..4 * k + 4].try_into().unwrap()))).collect())
                    .collect()
            })
            .unwrap_or_default(),
        squad_slots: bytes(top("party").and_then(|r| r.child("m_squad")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        points_by_period: bytes(top("subject").and_then(|r| r.child("job_point_interval")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        month_reports: bytes(top("monthrep").and_then(|r| r.child("data"))).map(|d| d.chunks_exact(0x144).map(<[u8]>::to_vec).collect()).unwrap_or_default(),
        squad: bytes(top("party").and_then(|r| r.child("m_squad")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).filter(|&v| v >= 0).collect())
            .unwrap_or_default(),
        deliveries: bytes(top("delivery").and_then(|r| r.child("delivery_data")))
            .map(|d| d.chunks_exact(8).map(|c| (i32::from_le_bytes(c[..4].try_into().unwrap()), i32::from_le_bytes(c[4..].try_into().unwrap()))).collect())
            .unwrap_or_default(),
        work_status: bytes(top("work").and_then(|r| r.child("work_data_status")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        route_knowledge: bytes(top("worldmap").and_then(|r| r.child("knowledgeRoute")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        place_knowledge: bytes(top("worldmap").and_then(|r| r.child("knowledgePointer")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        world_position: bytes(top("worldmap").and_then(|r| r.child("chara"))).and_then(|d| Some((i32_at(&d, 0)?, i32_at(&d, 4)?))),
        place_states: bytes(top("worldmap").and_then(|r| r.child("pointerStatus"))).map(|d| place_states(&d)).unwrap_or_default(),
        field_effect: bytes(top("fieldmap").and_then(|r| r.child("m_userDataFieldMapData"))).and_then(|d| field_effect(&d)),
        mix_skills_learnt: bytes(top("bonus").and_then(|r| r.child("skill"))).map(|d| d.iter().map(|&b| b != 0).collect()).unwrap_or_default(),
        mix_bonus: bytes(top("bonus").and_then(|r| r.child("mixvalue")))
            .map(|d| d.chunks_exact(2).map(|c| i32::from(u16::from_le_bytes([c[0], c[1]]))).collect())
            .unwrap_or_default(),
        bonus_values: bytes(top("bonus").and_then(|r| r.child("value")))
            .map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
            .unwrap_or_default(),
        recipe_effects: bytes(top("mix_recipe")).map(|d| recipe_flags(&d)).unwrap_or_default(),
        knowledge: {
            let k = top("knowledge");
            let levels = |n: &str| bytes(k.and_then(|r| r.child(n))).map(|d| d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect()).unwrap_or_default();
            Knowledge {
                item: levels("item"),
                recipe: levels("recipe"),
                book: levels("book"),
                field_map: levels("fieldMap"),
                monster: levels("monster"),
                mix_esp: levels("mixEsp"),
                potential: levels("potential"),
                eff: levels("eff"),
            }
        },
    }
}

/// `mix_recipe`'s body: a u32 count, then per recipe a u32 length (10) and that many flag
/// bytes.
fn recipe_flags(d: &[u8]) -> Vec<[bool; 10]> {
    let Some(n) = d.get(..4).map(|c| u32::from_le_bytes(c.try_into().unwrap()) as usize) else { return Vec::new() };
    let mut out = Vec::with_capacity(n.min(1024));
    let mut o = 4;
    while out.len() < n {
        let Some(len) = d.get(o..o + 4).map(|c| u32::from_le_bytes(c.try_into().unwrap()) as usize) else { break };
        let Some(b) = d.get(o + 4..o + 4 + len) else { break };
        out.push(std::array::from_fn(|k| b.get(k).is_some_and(|&v| v != 0)));
        o += 4 + len;
    }
    out
}

/// The top records of a decoded state ([`decode`]'s output): its header skipped, records read
/// until the zero padding.
pub fn state_records(state: &[u8]) -> Result<Vec<Record<'_>>, Error> {
    let d = state.get(0x20..).ok_or(Error::Corrupt("save state shorter than its header"))?;
    let mut end = 0;
    while end + 4 <= d.len() {
        let size = u32::from_le_bytes(d[end..end + 4].try_into().unwrap()) as usize;
        if size == 0 || end + size > d.len() {
            break;
        }
        end += size;
    }
    records(&d[..end]).ok_or(Error::Corrupt("save state records"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header() {
        let mut d = vec![0u8; HEADER + 4];
        d[0x28] = 5;
        for (k, v) in [2026u16, 8, 6, 1, 18, 8, 31, 579].iter().enumerate() {
            d[0xa4 + 2 * k..0xa6 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
        let s = parse(&d).unwrap();
        assert_eq!(s.summary[0], 5);
        assert_eq!((s.saved_at.year, s.saved_at.month, s.saved_at.day, s.saved_at.hour), (2026, 8, 1, 18));
        assert_eq!(s.body.len(), 4);
    }

    #[test]
    fn gamma_and_table() {
        // 3 bytes: 0 ("1"), 2 ("011" → 3 - 1), 0xff (eight zeros); then the table step.
        let bits = "1" .to_string() + "011" + "00000000";
        let mut packed = vec![0, 0, 0, 3];
        let mut byte = 0u8;
        for (i, c) in bits.chars().enumerate() {
            byte |= ((c == '1') as u8) << (7 - i % 8);
            if i % 8 == 7 {
                packed.push(byte);
                byte = 0;
            }
        }
        packed.push(byte);
        assert_eq!(ungamma(&packed).unwrap(), [0, 2, 0xff]);
        assert_eq!(unsubstitute(&[2, b'x', b'y', 0, 0, 0, 3, 1, 0, 1]).unwrap(), b"yxy");
    }

    /// The settings file of a test run against its state read from the game's memory (whose
    /// trailing zeros the file drops; `decode` restores those its records cover)
    /// (`BARREL_SYSDATA` = the file, `BARREL_SYSDATA_PLAIN` = the state; not in the repository).
    #[test]
    #[ignore]
    fn decodes_a_real_settings_file() {
        let (Ok(f), Ok(p)) = (std::env::var("BARREL_SYSDATA"), std::env::var("BARREL_SYSDATA_PLAIN")) else { return };
        let out = decode(&barrel_files::read(f).unwrap()).unwrap();
        assert_eq!(out[..], barrel_files::read(p).unwrap()[..out.len()]);
    }

    #[test]
    fn records_nest() {
        // header, then system_data { ipu [2 bytes], FLAG raw }
        let rec = |name: &str, body: &[u8]| {
            let mut r = ((4 + name.len() + 1 + body.len()) as u32).to_le_bytes().to_vec();
            r.extend_from_slice(name.as_bytes());
            r.push(0);
            r.extend_from_slice(body);
            r
        };
        let ipu = rec("ipu", &[2, 0, 0, 0, 7, 8]);
        let flag = rec("FLAG", &[0, 0, 9]);
        let top = rec("system_data", &[ipu, flag].concat());
        let mut state = vec![0u8; 0x20];
        state.extend_from_slice(&top);
        state.extend_from_slice(&[0; 8]);
        let r = state_records(&state).unwrap();
        assert_eq!(r[0].name, "system_data");
        assert_eq!(r[0].child("ipu").unwrap().body, Body::Data(&[7, 8]));
        assert_eq!(r[0].child("FLAG").unwrap().body, Body::Raw(&[0, 0, 9]));
    }

    #[test]
    fn recipe_flags_read() {
        let mut d = 2u32.to_le_bytes().to_vec();
        for flags in [[0u8; 10], [1, 0, 1, 0, 0, 0, 0, 0, 0, 0]] {
            d.extend(10u32.to_le_bytes());
            d.extend(flags);
        }
        let f = recipe_flags(&d);
        assert_eq!(f.len(), 2);
        assert!(!f[0].iter().any(|&b| b));
        assert_eq!(&f[1][..3], &[true, false, true]);
    }

    #[test]
    fn item_units_skip_empty_slots() {
        let mut d = vec![0xffu8; ITEM_UNIT * 2];
        let unit: [u8; 36] = [0x13, 1, 0x0b, 0, 0, 0, 0x20, 0x42, 0x37, 0, 0xff, 0xff, 0xff, 0xff, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0];
        d[ITEM_UNIT..].copy_from_slice(&unit);
        let u = item_units(&d);
        assert_eq!(u.len(), 1);
        assert_eq!((u[0].serial, u[0].item, u[0].quality, u[0].traits), (0x113, 11, 40.0, [0x37, -1, -1]));
        assert_eq!((u[0].effects, u[0].counts), ([-1; 4], [1, 1, 0, 1]));
    }

    #[test]
    fn event_records_read() {
        let mut d = vec![0u8; 72];
        d[36..40].copy_from_slice(&1i32.to_le_bytes());
        d[40..44].copy_from_slice(&90i32.to_le_bytes());
        d[44..48].copy_from_slice(&1i32.to_le_bytes());
        d[52..56].copy_from_slice(&67i32.to_le_bytes());
        d[4..8].copy_from_slice(&(-31i32).to_le_bytes());
        let e = event_records(&d);
        assert_eq!(e[0], EventRecord { seen: false, day: -31, count: 0, roll: 0, value: 0 });
        assert_eq!(e[1], EventRecord { seen: true, day: 90, count: 1, roll: 67, value: 0 });
    }

    #[test]
    fn big_endian_lengths_read_too() {
        let mut r = 15u32.to_le_bytes().to_vec();
        r.extend_from_slice(b"bgm\0");
        r.extend_from_slice(&[0, 0, 0, 3, 7, 8, 9]);
        let rec = records(&r).unwrap();
        assert_eq!(rec[0].body, Body::Data(&[7, 8, 9]));
    }

    #[test]
    fn encode_decodes_back() {
        // A state: header, a record, trailing zeros (dropped and restored).
        let mut state = vec![0u8; 0x20];
        state[0..4].copy_from_slice(&20130322u32.to_le_bytes());
        let mut rec = 20u32.to_le_bytes().to_vec();
        rec.extend_from_slice(b"ipu\0");
        rec.extend_from_slice(&8u32.to_le_bytes());
        rec.extend_from_slice(&[1, 2, 3, 0xff, 0, 0, 7, 0]);
        state.extend_from_slice(&rec);
        for seed in [0, 1, 0x1234_5678] {
            let coded = encode(&state, seed);
            assert_eq!(coded.len() % 16, 0);
            assert_eq!(decode(&coded).unwrap(), state);
        }
        assert_eq!(ungamma(&gamma(&[0, 1, 2, 254, 255, 0])).unwrap(), [0, 1, 2, 254, 255, 0]);
        assert_eq!(unsubstitute(&substitute(b"abracadabra")).unwrap(), b"abracadabra");
    }

    /// A real settings state coded again and decoded back (`BARREL_SYSDATA`, not in the
    /// repository).
    #[test]
    #[ignore]
    fn a_real_state_codes_back() {
        let Ok(f) = std::env::var("BARREL_SYSDATA") else { return };
        let state = decode(&barrel_files::read(f).unwrap()).unwrap();
        assert_eq!(decode(&encode(&state, 0xdead_beef)).unwrap(), state);
    }
}
