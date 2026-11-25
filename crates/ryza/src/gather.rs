//! Gathering: what a gathering place gives. The places and what each offers are tables in the
//! executable (`FIELDMAP_COLLECT_PLACE_*`, a place's lists of items with weights, quality and
//! potential ranges); the rest is the game's rule, which draws from the game's random numbers
//! ([`crate::random`]) in a fixed order, so a run from the same generator state gives the
//! same item. Docs: docs/ryza/open.md, "Gathering (P6)".
//!
//! One gathering at a place draws: the number of items (`irand(min, max)` of the place),
//! then for each item the pick (`irand(0, total weight - 1)`, among the items the library
//! does not know yet while there are any), its quality (`frand`), the number of random
//! potentials, the level threshold of the potentials it may take, and for each potential which
//! one and at what level.

use barrel_formats::pe::Pe;
use barrel_formats::table;

use crate::random::Random;

/// The place table in the executable: 1,834 rows of twelve i32 (`PLACE_ROW` bytes), in the
/// order of the `FIELDMAP_COLLECT_PLACE_*` names.
const PLACES_VA: u64 = 0x1_4116_4260;
const PLACE_ROW: usize = 0x30;
/// The entry table: 3,419 rows of 27 i32 (`ENTRY_ROW` bytes), each an item of a list a place
/// offers (the list is the row's first word).
const ENTRIES_VA: u64 = 0x1_4117_9a40;
const ENTRY_ROW: usize = 0x6c;
const ENTRY_COUNT: usize = 0xd5b;
/// The lists the places and entries name (`0x730`).
const LISTS: i32 = 0x730;
/// The first of the place names.
const PLACE_PREFIX: &str = "FIELDMAP_COLLECT_PLACE_";
/// Items above this number are not in the library's table (the game treats them as always new).
const LIBRARY_ITEMS: i32 = 0x2fc;
/// A library level above this is known: the game picks the items at or below it first.
const KNOWN_ABOVE: i32 = 2;
/// The kinds' (`ITEM_KIND_*`, in the executable's order) group in the potentials' table of which
/// kinds may take a potential: mix, attack, heal, debuff, buff, weapon, armour, accessory,
/// material, core, other (`-1` none). The jump table of `0x140520d10`.
const KIND_GROUP: [i8; 14] = [8, 0, 1, 2, 3, 4, 9, -1, -1, 5, 6, 7, -1, 10];
/// The groups' attribute names in `item_potential.xml`.
const GROUPS: [&str; 11] = ["mix", "atk", "heal", "supEne", "supPar", "wep", "arm", "acc", "mat", "core", "etc"];

/// A gathering place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// The list of entries by story rank (five ranks).
    pub lists: [i32; 5],
    /// How many items a gathering gives, from and to.
    pub count: (i32, i32),
    /// The event flags that open and close it (`-1` for none).
    pub open: i32,
    pub close: i32,
    /// The field it is in (`FIELDMAP_*`'s number).
    pub field: i32,
}

/// One item a place can give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub list: i32,
    pub item: i32,
    /// The quality range before the difficulty's bonus.
    pub quality: (i32, i32),
    /// How many random potentials it may get.
    pub potentials: (i32, i32),
    /// The levels of potential it may take.
    pub levels: (i32, i32),
    /// The level a random potential gets.
    pub potential_level: (i32, i32),
    pub weight: i32,
    /// The event flags that make it available and end it (`-1` for none).
    pub open: i32,
    pub close: i32,
    /// Fixed potentials, by number (`-1` for none), with their level ranges.
    pub fixed: [(i32, (i32, i32)); 3],
    /// Fixed effects (`-1` for none).
    pub effects: [i32; 4],
}

/// A row of `item_potential.xml`, as gathering reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Potential {
    pub grade: i32,
    /// Whether gathering may give it (`give_range`).
    pub gives: bool,
    /// For each kind group ([`GROUPS`]), whether the potential fits.
    pub groups: [bool; 11],
}

/// The tables of gathering.
#[derive(Clone, Debug)]
pub struct Tables {
    pub places: Vec<Place>,
    entries: Vec<Entry>,
    /// The place names with their prefix cut (`S06_T_RASEN_COMB_HAND`).
    names: Vec<String>,
    potentials: Vec<Potential>,
    /// Each item's kind group (`-1` for none).
    item_groups: Vec<i8>,
    /// Each item's picture (`imgNo`).
    images: Vec<usize>,
}

fn int(data: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn number(row: &table::Element, key: &str) -> i32 {
    row.attr(key).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

impl Tables {
    /// The tables from the executable (`exe`), `itemdata.xml` and `item_potential.xml`.
    pub fn read(exe: &[u8], item_data: &[u8], item_potential: &[u8]) -> Result<Tables, String> {
        let pe = Pe::parse(exe).map_err(|e| format!("gather: {e}"))?;
        let (all, _) = pe.enum_names(&format!("{PLACE_PREFIX}S06_T_RASEN_COMB_HAND")).ok_or("gather: no place names")?;
        let names: Vec<String> = all.iter().skip_while(|n| !n.starts_with(PLACE_PREFIX)).take_while(|n| n.starts_with(PLACE_PREFIX)).map(|n| n[PLACE_PREFIX.len()..].to_string()).collect();
        let rows = |va: u64, row: usize, count: usize| -> Result<&[u8], String> {
            let at = pe.offset(va).ok_or("gather: table outside the executable")?;
            exe.get(at..at + row * count).ok_or_else(|| "gather: table cut short".to_string())
        };
        let raw = rows(PLACES_VA, PLACE_ROW, names.len())?;
        let places: Vec<Place> = raw
            .chunks_exact(PLACE_ROW)
            .map(|r| Place { lists: std::array::from_fn(|k| int(r, 4 * k)), count: (int(r, 20), int(r, 24)), open: int(r, 28), close: int(r, 32), field: int(r, 36) })
            .collect();
        if places.iter().any(|p| p.lists.iter().any(|&l| l >= LISTS)) {
            return Err("gather: the place table is not where it was".into());
        }
        let raw = rows(ENTRIES_VA, ENTRY_ROW, ENTRY_COUNT)?;
        let entries: Vec<Entry> = raw
            .chunks_exact(ENTRY_ROW)
            .map(|r| {
                let at = |k: usize| int(r, 4 * k);
                Entry {
                    list: at(0),
                    item: at(1),
                    quality: (at(2), at(3)),
                    potentials: (at(5), at(6)),
                    levels: (at(7), at(8)),
                    potential_level: (at(9), at(10)),
                    weight: at(11),
                    open: at(12),
                    close: at(13),
                    fixed: std::array::from_fn(|k| (at(14 + k), (at(17 + k), at(20 + k)))),
                    effects: std::array::from_fn(|k| at(23 + k)),
                }
            })
            .collect();
        if entries.iter().any(|e| e.list < 0 || e.list >= LISTS) {
            return Err("gather: the entry table is not where it was".into());
        }

        let potentials = table::parse(item_potential)
            .map_err(|e| format!("item_potential: {e}"))?
            .children
            .iter()
            .map(|r| Potential { grade: number(r, "grade"), gives: r.attr("give_range").is_some_and(|v| v.trim() == "-1"), groups: std::array::from_fn(|k| r.attr(GROUPS[k]).is_some_and(|v| v.trim() == "-1")) })
            .collect();
        let (kinds, first) = pe.enum_names("ITEM_KIND_MATERIAL").ok_or("gather: no item kinds")?;
        let material = kinds.iter().position(|n| *n == "ITEM_KIND_MATERIAL").unwrap_or(first);
        let items = table::parse(item_data).map_err(|e| format!("itemdata: {e}"))?;
        let item_groups = items
            .children
            .iter()
            .map(|r| {
                let kind = r.attr("kindTag").and_then(|t| kinds.iter().position(|n| *n == t)).map(|k| k - material);
                kind.and_then(|k| KIND_GROUP.get(k)).copied().unwrap_or(-1)
            })
            .collect();
        let images = items.children.iter().map(|r| number(r, "imgNo") as usize).collect();
        Ok(Tables { places, entries, names, potentials, item_groups, images })
    }

    /// The number of a place by its name (`FIELDMAP_COLLECT_PLACE_S06_T_RASEN_COMB_HAND` or
    /// without the prefix).
    pub fn place(&self, name: &str) -> Option<usize> {
        let name = name.strip_prefix(PLACE_PREFIX).unwrap_or(name);
        self.names.iter().position(|n| n == name)
    }

    /// An item's picture number (`imgNo`: the small and large pictures' places in their files).
    pub fn image(&self, item: i32) -> Option<usize> {
        self.images.get(usize::try_from(item).ok()?).copied()
    }

    pub fn place_name(&self, place: usize) -> Option<&str> {
        self.names.get(place).map(String::as_str)
    }

    /// The entries of a list, in the table's order.
    pub fn list(&self, list: i32) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |e| e.list == list)
    }

    /// Whether `item` may take a potential at all, and the potentials it may take: the ones
    /// gathering gives that fit its kind.
    fn fits(&self, potential: &Potential, item: i32) -> bool {
        if item > LIBRARY_ITEMS {
            return true;
        }
        match self.item_groups.get(item as usize).copied().unwrap_or(-1) {
            g if g >= 0 => potential.groups[g as usize],
            _ => false,
        }
    }
}

/// What the game has around a gathering: the player's standing and the party's bonuses.
pub struct Context<'a> {
    /// The difficulty setting (`0x141aae83c`: 1 is Normal), which raises the quality range.
    pub difficulty: i32,
    /// The story rank that picks a place's list (`0x141ab7988`).
    pub rank: usize,
    /// The library's level of each item (`knowledge/item` of the save).
    pub knowledge: &'a [i32],
    /// Whether an event flag is set (the entries' `open` and `close`).
    pub flag: &'a dyn Fn(i32) -> bool,
    /// Items more from the party's effects (0 without any).
    pub more_items: i32,
    pub boost: Boost,
}

/// The party's bonuses to the quality: a flat addition and two percentages (the maximums of
/// the effects `0xe9`, `0x12b` over the party, and the tool's `0xea`). All 0 without effects.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boost {
    pub flat: f32,
    pub percent: [f32; 2],
    /// Levels added to a potential's (`0x1c5`).
    pub potential_levels: i32,
}

/// An item gathered.
#[derive(Clone, Debug, PartialEq)]
pub struct Gathered {
    pub item: i32,
    pub quality: i32,
    /// The entry's fixed effects.
    pub effects: Vec<i32>,
    /// The potentials it got: the number in `item_potential.xml` and the level.
    pub potentials: Vec<(usize, i32)>,
    /// Whether the library did not know it (it shows as a new discovery).
    pub new: bool,
}

impl Context<'_> {
    fn known(&self, item: i32) -> i32 {
        self.knowledge.get(item as usize).copied().unwrap_or(0)
    }

    /// `0x1405a2c20`: an entry or place is on when its opening flag is set (or none) and its
    /// closing flag is not.
    fn on(&self, open: i32, close: i32) -> bool {
        (open == -1 || (self.flag)(open)) && (close == -1 || !(self.flag)(close))
    }
}

impl Tables {
    /// A gathering at `place` from the generator `rng`: the items it gives, in order, the
    /// draws made in the order the game makes them. Nothing when the place is not a place.
    pub fn gather(&self, ctx: &Context, rng: &mut Random, place: usize) -> Vec<Gathered> {
        let Some(row) = self.places.get(place) else { return Vec::new() };
        let count = rng.irand(row.count.0, row.count.1) + ctx.more_items;
        let offered: Vec<&Entry> = self.list(row.lists[ctx.rank.min(4)]).filter(|e| ctx.on(e.open, e.close)).collect();
        let total: i32 = offered.iter().map(|e| e.weight).sum();
        if offered.is_empty() || total <= 0 || count <= 0 {
            return Vec::new();
        }
        // The items not known yet are taken first, each once.
        let mut fresh: Vec<&Entry> = offered.iter().copied().filter(|e| e.item > LIBRARY_ITEMS || ctx.known(e.item) <= KNOWN_ABOVE).collect();
        let mut fresh_total: i32 = fresh.iter().map(|e| e.weight).sum();
        let mut out = Vec::new();
        for _ in 0..count {
            let picked = if fresh.is_empty() {
                let at = rng.irand(0, total - 1);
                weighted(&offered, at)
            } else {
                let at = rng.irand(0, fresh_total - 1);
                let picked = weighted(&fresh, at);
                if let Some(p) = picked {
                    fresh.retain(|e| e.item != p.item);
                    fresh_total = fresh.iter().map(|e| e.weight).sum();
                }
                picked
            };
            let Some(entry) = picked else { break };
            out.push(self.one(ctx, rng, entry));
        }
        out
    }

    /// One item of an entry (`0x1404b36e0` and `0x1404b3980`).
    fn one(&self, ctx: &Context, rng: &mut Random, entry: &Entry) -> Gathered {
        // The quality: the entry's range raised by the difficulty, drawn as a float, then the party's bonuses.
        let (mut low, mut high) = (entry.quality.0 as f32, entry.quality.1 as f32);
        if low != high {
            let bonus = ctx.difficulty as f32 + 1.0;
            high += bonus;
            low += bonus * 0.5;
        }
        let drawn = rng.frand(low, high);
        let rate = |p: f32| (p + 100.0) / 100.0;
        let quality = (drawn * rate(ctx.boost.percent[0]) * rate(ctx.boost.percent[1]) + ctx.boost.flat) as i32;

        // Random potentials: how many, which levels they may take, then one by one.
        let mut extra = rng.irand(entry.potentials.0, entry.potentials.1);
        let threshold_low = (entry.levels.0.min(0xfe)).max(0);
        let threshold_high = (entry.levels.1.min(0xff)).max(threshold_low);
        let threshold = rng.irand(threshold_low, threshold_high);
        let mut potentials = Vec::new();
        for (id, (low, high)) in entry.fixed {
            if id >= 0 {
                let level = rng.irand(low + ctx.boost.potential_levels, high + ctx.boost.potential_levels);
                potentials.push((id as usize, level));
                extra -= 1;
            }
        }
        let mut candidates = self.candidates(entry, threshold, threshold_low, threshold_high, extra.max(0) as usize);
        for _ in 0..extra.max(0) {
            if candidates.is_empty() {
                break;
            }
            let at = rng.irand(0, candidates.len() as i32 - 1) as usize;
            let level = rng.irand(entry.potential_level.0 + ctx.boost.potential_levels, entry.potential_level.1 + ctx.boost.potential_levels);
            potentials.push((candidates.remove(at), level));
        }
        Gathered { item: entry.item, quality, effects: entry.effects.iter().copied().filter(|&e| e >= 0).collect(), potentials, new: entry.item <= LIBRARY_ITEMS && ctx.known(entry.item) <= KNOWN_ABOVE }
    }

    /// The potentials an item may be given (`0x140520840`): the ones gathering gives, of a
    /// grade from the entry's first level to `high` and fitting the item's kind, the ones of
    /// grade `threshold` or more first; the lower ones follow only to make `need`.
    fn candidates(&self, entry: &Entry, threshold: i32, low: i32, high: i32, need: usize) -> Vec<usize> {
        let (mut top, mut middle, mut rest) = (Vec::new(), Vec::new(), Vec::new());
        for (n, p) in self.potentials.iter().enumerate() {
            if !p.gives || p.grade < entry.levels.0 || p.grade > high || !self.fits(p, entry.item) {
                continue;
            }
            match p.grade {
                g if g >= threshold => top.push(n),
                g if g >= low => middle.push(n),
                _ => rest.push(n),
            }
        }
        if top.len() < need {
            top.extend(middle);
        }
        if top.len() < need {
            top.extend(rest);
        }
        top
    }
}

/// The entry whose weights' running total covers `at`.
fn weighted<'a>(entries: &[&'a Entry], at: i32) -> Option<&'a Entry> {
    let mut before = 0;
    entries.iter().copied().find(|e| {
        let after = before + e.weight;
        let hit = before <= at && at < after;
        before = after;
        hit
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(item: i32, weight: i32) -> Entry {
        Entry { list: 0, item, quality: (11, 20), potentials: (0, 1), levels: (1, 10), potential_level: (1, 3), weight, open: -1, close: -1, fixed: [(-1, (1, 1)); 3], effects: [-1; 4] }
    }

    #[test]
    fn the_running_total_picks_an_entry() {
        let (a, b) = (entry(106, 70), entry(128, 30));
        let both = [&a, &b];
        assert_eq!(weighted(&both, 0).map(|e| e.item), Some(106));
        assert_eq!(weighted(&both, 69).map(|e| e.item), Some(106));
        assert_eq!(weighted(&both, 70).map(|e| e.item), Some(128));
        assert_eq!(weighted(&both, 100), None);
    }

    /// The installed game (`BARREL_RYZA_INSTALL`): the gathering at the Rasenboden honeycomb
    /// from the generator state of frame 2040 of the capture draws what the game drew
    /// (`irand 1 2`, `irand 0 63` in hex, `frand 12 22`, ...) and gives a Honey Ant.
    #[test]
    fn the_honeycomb_at_2040() {
        let Some(root) = std::env::var_os("BARREL_RYZA_INSTALL") else { return };
        let root = std::path::Path::new(&root);
        let install = crate::install::Install::open(root).unwrap();
        let exe = std::fs::read(root.join("Atelier_Ryza_DX.exe")).unwrap();
        let tables = Tables::read(&exe, &install.read("\\saves\\item\\itemdata.xml").unwrap(), &install.read("\\saves\\item\\item_potential.xml").unwrap()).unwrap();
        let place = tables.place("S06_T_RASEN_COMB_HAND").unwrap();
        assert_eq!(place, 545);
        let mut rng = Random::after(1_767_225_600, 7306);
        rng.record();
        let knowledge = vec![0; 765];
        let ctx = Context { difficulty: 1, rank: 0, knowledge: &knowledge, flag: &|_| true, more_items: 0, boost: Boost::default() };
        let got = tables.gather(&ctx, &mut rng, place);
        let draws: Vec<String> = rng.draws().iter().map(|d| d.to_string().split(" @").next().unwrap().to_string()).collect();
        assert_eq!(draws, ["irand 1 2", "irand 0 63", "frand 12 22", "irand 0 1", "irand 1 a", "irand 0 1", "irand 1 3"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].item, 128);
        assert_eq!(got[0].quality, 21);
        assert_eq!(got[0].potentials, [(35, 1)]);
        assert!(got[0].new);
        assert_eq!(rng.index(), 0x1ba + 7);
    }
}
