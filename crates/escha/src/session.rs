//! A game session: what the game is doing from a new game on, one scene at a time. A scene is
//! an event playing or free play in a field; an event's end picks what follows (the story's
//! next event by `event_start::Chain`, else play goes on in the field the event left the
//! player in).
//!
//! A session saves to Barrel's own small text format ([`Session::save`], [`Session::load`]),
//! taken in free play: the lead, the field and where the player stands, the money, each party
//! member's level, experience and equipment, and the story (the date, each seen event with the
//! day it was seen, the flags that are not 0). One fact a line:
//!
//! ```text
//! barrel-save 1
//! lead escha
//! field 3 10000 10950
//! today 90
//! tenths 5
//! money 100
//! member 0 1 0 364,417,-1,-1
//! carry 0 270 55 2 12,-1 3
//! item 55 60 12,40
//! seen 8 90
//! flag 36 1
//! ```

use crate::event_start::{new_game_event, Chain};

/// The money a new game starts with (`0x14036a8c0`).
pub const NEW_GAME_MONEY: i32 = 100;
/// The most money held (`0x14036a8e0`).
pub const MAX_MONEY: i32 = 999_999;

/// A request taken from the board (`quest/player`'s slots, `save::BoardSlot`): its kind (the
/// Requests screen's order, `requests::Kind`: 0 synthesis, 1 gathering, 2 subjugation; the board's
/// own slots count 0 synthesis, 1 monster, 2 gathering), target (an item row, or a monster
/// number), how many are wanted and done so far, and the
/// days it was posted and is due, the rank points it pays and its two prizes (item row, count;
/// row -1 for none), as the slot holds them rolled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Taken {
    pub kind: i32,
    pub target: i32,
    pub wanted: i32,
    pub progress: i32,
    pub posted: i32,
    pub until: i32,
    pub points: i32,
    pub prizes: [(i32, i32); 2],
}

impl Taken {
    pub fn done(&self) -> bool {
        self.progress >= self.wanted
    }
}

/// The requests a player can hold at once (the board's ten slots).
pub const TAKEN_SLOTS: usize = 10;

/// One party member's lasting state (its record at `0x1410bcb60` + 0x2b0 × member).
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub level: i32,
    pub experience: i32,
    /// The items equipped, by item number, -1 for none: weapon, armour, two accessories.
    pub equipment: [i32; 4],
    /// The equipped items as the save holds them (quality, effects, traits), by slot; `None`
    /// where only the item number is known (a new game's kit, a text save before `wear` lines).
    pub worn: [Option<WornItem>; 4],
    /// The battle items it carries (the alchemists' `explore_equip` lists), in order.
    pub battle_items: Vec<BattleItem>,
    /// Its HP, its most HP, its MP and its most MP (with equipment) now, when the save holds
    /// them: a battle starts from them.
    pub now: Option<[i32; 4]>,
}

impl Member {
    /// A member with no battle items, no worn units and no saved vitals.
    pub fn fresh(level: i32, experience: i32, equipment: [i32; 4]) -> Member {
        Member { level, experience, equipment, worn: Default::default(), battle_items: Vec::new(), now: None }
    }

    /// The equipped items for a battle's stats: the worn units when every equipped slot has
    /// one, else `None` (the item numbers alone make fresh items, `Tables::equip`).
    pub fn worn_units(&self) -> Option<Vec<crate::data::save::ItemUnit>> {
        let mut units = Vec::new();
        for (&item, w) in self.equipment.iter().zip(&self.worn) {
            match (item >= 0, w) {
                (true, Some(w)) if w.item == item => units.push(w.unit()),
                (true, _) => return None,
                (false, _) => {}
            }
        }
        (!units.is_empty()).then_some(units)
    }
}

/// An equipped item with what its stats depend on: the item, its quality, its effect rows
/// (`item_effect.xml`) and trait rows (`item_potential.xml`).
#[derive(Clone, Debug, PartialEq)]
pub struct WornItem {
    pub item: i32,
    pub quality: f32,
    pub effects: Vec<i16>,
    pub traits: Vec<usize>,
    /// Whether the traits add to its stats (the game's item unit gates them on a flag word,
    /// which is 0 for items as made).
    pub traits_count: bool,
}

impl WornItem {
    /// From the game's item unit.
    fn from_unit(u: &crate::data::save::ItemUnit) -> WornItem {
        WornItem { item: i32::from(u.item), quality: u.quality, effects: u.effects.iter().copied().filter(|&e| e >= 0).collect(), traits: held_traits(u), traits_count: u.word_e == 0 }
    }

    /// An item from the basket, as it was made.
    fn from_got(g: &crate::gather::Got) -> WornItem {
        WornItem { item: g.item, quality: g.quality, effects: Vec::new(), traits: g.traits.clone(), traits_count: true }
    }

    /// Back into the basket.
    fn got(&self) -> crate::gather::Got {
        crate::gather::Got { item: self.item, quality: self.quality, traits: self.traits.clone() }
    }

    /// The item unit the equipment rules read (`equipment::item_stat`): unused rows are -1.
    pub fn unit(&self) -> crate::data::save::ItemUnit {
        let mut effects = [-1i16; 4];
        for (o, &e) in effects.iter_mut().zip(&self.effects) {
            *o = e;
        }
        let mut traits = [-1i16; 3];
        for (o, &t) in traits.iter_mut().zip(&self.traits) {
            *o = t as i16;
        }
        crate::data::save::ItemUnit { serial: 1, item: self.item as i16, quality: self.quality, traits, word_e: u16::from(!self.traits_count), effects, equip_cost: 0, word_1a: 0, counts: [0; 4] }
    }

    /// Its `wear` line for member `c`'s slot `k`:
    /// `wear C SLOT ITEM QUALITY EFFECTS|- TRAITS|- COUNTS(1/0)`.
    fn line(&self, c: usize, k: usize) -> String {
        let list = |v: Vec<String>| if v.is_empty() { "-".to_string() } else { v.join(",") };
        let effects = list(self.effects.iter().map(i16::to_string).collect());
        let traits = list(self.traits.iter().map(usize::to_string).collect());
        format!("wear {c} {k} {} {} {effects} {traits} {}\n", self.item, self.quality, u8::from(self.traits_count))
    }

    /// From a `wear` line's words after the member and slot.
    fn parse(w: &[&str]) -> Option<WornItem> {
        let list = |v: &str| -> Option<Vec<i64>> { if v == "-" { Some(Vec::new()) } else { v.split(',').map(|x| x.parse().ok()).collect() } };
        Some(WornItem {
            item: w.first()?.parse().ok()?,
            quality: w.get(1)?.parse().ok()?,
            effects: list(w.get(2)?)?.into_iter().map(|e| i16::try_from(e).ok()).collect::<Option<_>>()?,
            traits: list(w.get(3)?)?.into_iter().map(|t| usize::try_from(t).ok()).collect::<Option<_>>()?,
            traits_count: *w.get(4)? != "0",
        })
    }
}

/// A carried battle item: the item, its quality, the uses left, its effect rows
/// (`item_effect.xml`) and trait rows (`item_potential.xml`), as the game's item unit holds them.
#[derive(Clone, Debug, PartialEq)]
pub struct BattleItem {
    pub item: i32,
    pub quality: f32,
    pub uses: i32,
    pub effects: Vec<i16>,
    pub traits: Vec<usize>,
}

/// The library's level of a place visited (`worldmap/knowledgePointer`: 15 all, 10 only seen).
const KNOWLEDGE_ALL: i32 = 15;

/// An item unit's trait rows, those it has.
fn held_traits(u: &crate::data::save::ItemUnit) -> Vec<usize> {
    u.traits.iter().filter_map(|&t| usize::try_from(t).ok()).collect()
}

impl BattleItem {
    /// From the game's item unit: the uses left are its first count.
    fn from_unit(u: &crate::data::save::ItemUnit) -> BattleItem {
        BattleItem { item: i32::from(u.item), quality: u.quality, uses: i32::from(u.counts[0]), effects: u.effects.to_vec(), traits: held_traits(u) }
    }

    /// Its `carry` line for member `c`: `carry C ITEM QUALITY USES EFFECTS|- [TRAITS]`.
    fn line(&self, c: usize) -> String {
        let list = |v: Vec<String>| if v.is_empty() { "-".to_string() } else { v.join(",") };
        let effects = list(self.effects.iter().map(i16::to_string).collect());
        let traits: Vec<String> = self.traits.iter().map(usize::to_string).collect();
        format!("carry {c} {} {} {} {effects}{}\n", self.item, self.quality, self.uses, if traits.is_empty() { String::new() } else { format!(" {}", traits.join(",")) })
    }

    /// From a `carry` line's words after the member: `None` when it does not read.
    fn parse(w: &[&str]) -> Option<BattleItem> {
        let list = |v: &str| -> Option<Vec<String>> { if v == "-" { Some(Vec::new()) } else { Some(v.split(',').map(str::to_string).collect()) } };
        Some(BattleItem {
            item: w.first()?.parse().ok()?,
            quality: w.get(1)?.parse().ok()?,
            uses: w.get(2)?.parse().ok()?,
            effects: list(w.get(3)?)?.iter().map(|v| v.parse().ok()).collect::<Option<_>>()?,
            traits: w.get(4).map_or(Some(Vec::new()), |t| t.split(',').map(|v| v.parse().ok()).collect())?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scene {
    /// An event playing, by its `EVENT_*` name.
    Event(String),
    /// Free play in a field, by its map number (the town's 0 to 11, the areas from 12).
    Field(u32),
}

pub struct Session {
    chain: Chain,
    /// The calendar, once given its tables ([`Session::with_calendar`]): the date, the shops,
    /// the board, field time carried.
    calendar: Option<(crate::days::World, crate::days::Data)>,
    /// Field time carried from a loaded save, for the calendar once it is given.
    carried: i32,
    /// What the player holds (gathered so far): the basket.
    pub basket: Vec<crate::gather::Got>,
    scene: Scene,
    /// The field the player is in (the last an event changed to), if any yet.
    field: Option<u32>,
    /// Logy leads (the new game's choice).
    pub logy: bool,
    pub money: i32,
    /// The 11 party members by member number, once given their defaults ([`Session::with_party`]).
    pub party: Vec<Member>,
    /// The members who fight, in the order of the battle's plates (the lead first); empty when
    /// the save does not say (the lead and the other alchemist fight).
    pub squad: Vec<usize>,
    /// The items the player has come to know (held at some time: gathered, found, bought), as
    /// the area map's gatherable list shows them (the others are "????").
    known: std::collections::BTreeSet<i32>,
    /// The places opened by their field gimmicks, and the damage dealt to the obstacles in
    /// front of others (`area_map::closed`), by place number.
    opened: std::collections::BTreeSet<usize>,
    worn: std::collections::BTreeMap<usize, i32>,
    /// The places visited, by place number (`area_map::known`).
    visited: std::collections::BTreeSet<usize>,
    /// The field effect: its gauge (the HUD's percentage) and its state (a mode and values;
    /// the save's `fieldmap/m_userDataFieldMapData`: the user's at 8/20 reads 45, mode 4, 2).
    pub field_gauge: i32,
    pub field_state: crate::gather::FieldState,
    /// The requests taken, by slot.
    pub taken: [Option<Taken>; TAKEN_SLOTS],
    /// The adventurer rank (a `RankData` row) and the rank points held.
    pub rank: (i32, i32),
    /// Each party member's friendship value by character (the `Party` record's float, from a
    /// game save only: the menu shows it truncated), and the alchemy level and experience
    /// (`mix_lv`).
    pub friendship: Vec<f32>,
    pub alchemy: (i32, i32),
    /// The library has entries not looked at yet (the Journal's "NEW" badge in the menu).
    pub library_new: bool,
}

impl Session {
    /// A session with nothing in it yet: a new game's money, no party, no calendar, standing
    /// in field 0. The constructors set what they know on top of it, so a new field of the
    /// session needs its default here only.
    fn bare(chain: Chain) -> Session {
        Session {
            chain,
            calendar: None,
            carried: 0,
            basket: Vec::new(),
            scene: Scene::Field(0),
            field: None,
            logy: false,
            money: NEW_GAME_MONEY,
            party: Vec::new(),
            squad: Vec::new(),
            known: Default::default(),
            opened: Default::default(),
            worn: Default::default(),
            visited: Default::default(),
            field_gauge: 0,
            field_state: Default::default(),
            taken: [None; TAKEN_SLOTS],
            rank: (0, 0),
            friendship: Vec::new(),
            alchemy: (0, 0),
            library_new: false,
        }
    }

    /// A new game with the lead chosen (`logy`), opening with its first event.
    pub fn new_game(chain: Chain, logy: bool) -> Session {
        Session { scene: Scene::Event(new_game_event(logy, false).to_string()), logy, ..Session::bare(chain) }
    }

    /// A session already in free play in field `map` with the story where `chain` has it.
    pub fn in_field(chain: Chain, map: u32, logy: bool) -> Session {
        Session { scene: Scene::Field(map), field: Some(map), logy, ..Session::bare(chain) }
    }

    /// The player walks from field `from` to field `to` in free play: the event that starts as
    /// they leave or once they are there (`Chain::walk`, both fields of kind 0), which then
    /// plays; else play goes on in `to`.
    pub fn walked(&mut self, from: u32, to: u32) -> &Scene {
        // Leaving a field area (not the town) with field time carried passes a day.
        if from as usize >= crate::data::gimmick::TOWN_MAPS && from != to {
            self.time(|w, t| w.leave_field(t));
        }
        self.field = Some(to);
        self.scene = match self.chain.walk([0, from as i32], [0, to as i32]) {
            Some(e) => Scene::Event(e.to_string()),
            None => Scene::Field(to),
        };
        &self.scene
    }

    /// The field the player is in (or last left for the world map).
    pub fn field(&self) -> Option<u32> {
        self.field
    }

    /// The player leaves field area `from` for its area map: leaving a field area with field
    /// time carried passes a day (as [`Session::walked`] does between fields).
    pub fn left_for_area_map(&mut self, from: u32) {
        if from as usize >= crate::data::gimmick::TOWN_MAPS {
            self.time(|w, t| w.leave_field(t));
        }
    }

    /// The calendar from `data`, starting as a new game's (`days::World::new_game`, the main
    /// generator as `rng`) on the story's date.
    /// The party as a new game sets it up (`0x1403619d0`): every member at level 1 with no
    /// experience, in its default kit (`battle_data::party_defaults`), carrying no battle items
    /// (`0x140364ff0` empties the alchemists' lists). Members a loaded save
    /// already holds keep their own.
    pub fn with_party(mut self, defaults: &[crate::data::battle_data::PartyDefault]) -> Session {
        for d in defaults.iter().skip(self.party.len()) {
            self.party.push(Member::fresh(1, 0, d.equipment));
        }
        self
    }

    /// Money in (or out, when negative), kept within 0 and [`MAX_MONEY`]; false (and nothing
    /// paid) when there is not enough.
    pub fn pay(&mut self, amount: i32) -> bool {
        if self.money + amount < 0 {
            return false;
        }
        self.money = (self.money + amount).min(MAX_MONEY);
        true
    }

    /// A battle won by `fighters` (member numbers): the money comes in and each of them gains
    /// the experience (`growth::Growth::gain`, no level-limit bonus), going up the levels it
    /// reaches in `exp_table`.
    pub fn won(&mut self, fighters: &[usize], exp: i32, money: i32, exp_table: &[(i32, i32)]) {
        self.pay(money);
        for &c in fighters {
            if let Some(m) = self.party.get_mut(c) {
                let mut g = crate::growth::Growth { level: m.level, exp: m.experience, gained: 0 };
                g.gain(exp_table, exp, 0, false);
                (m.level, m.experience) = (g.level, g.exp);
            }
        }
    }

    /// Sells the basket's items at `picks` (indices into the basket) to a shop
    /// (`shop::sale_total`): each one's worth (`synthesis::worth` of its `itemData` cost, from
    /// `cost`, at its quality; its traits' price changes not counted, all 0 as shipped) with the
    /// sell-price bonus `sell_bonus` percent. The items leave the basket and the money comes in;
    /// what it paid.
    pub fn sell(&mut self, picks: &[usize], cost: impl Fn(i32) -> i32, sell_bonus: i32) -> i32 {
        let sold: Vec<i32> = picks.iter().filter_map(|&k| self.basket.get(k).map(|g| g.item)).collect();
        self.known.extend(sold);
        let mut picks: Vec<usize> = picks.iter().copied().filter(|&k| k < self.basket.len()).collect();
        picks.sort_unstable();
        picks.dedup();
        let sold: Vec<(i32, i32)> = picks.iter().map(|&k| (crate::synthesis::worth(cost(self.basket[k].item), self.basket[k].quality, &[]), 1)).collect();
        let paid = crate::shop::sale_total(&sold, sell_bonus);
        for &k in picks.iter().rev() {
            self.basket.remove(k);
        }
        self.pay(paid);
        paid
    }

    /// Buys `count` of a shop's ware (`shop::purchase_cost` of its list price): false and
    /// nothing bought when the money is short; else the money goes and the items come into the
    /// basket at the ware's quality (its effects and traits not carried yet).
    pub fn buy(&mut self, item: i32, quality: f32, price: i32, count: i32) -> bool {
        if count <= 0 || !self.pay(-crate::shop::purchase_cost(price, count)) {
            return false;
        }
        self.known.insert(item);
        for _ in 0..count {
            self.basket.push(crate::gather::Got { item, quality, traits: Vec::new() });
        }
        true
    }

    /// Puts basket item `pick` on member `member` (character number) in slot `slot` (0 weapon,
    /// 1 armour, 2 and 3 accessories) when it fits there (`ItemRow::fits`); what the slot held
    /// goes back into the basket with its quality and traits (quality 50 and none when only its
    /// number was known). Not yet the game's: a worn item's effects are not kept in the basket.
    pub fn equip(&mut self, member: usize, slot: usize, pick: usize, items: &[crate::equipment::ItemRow]) -> Result<(), String> {
        let got = self.basket.get(pick).ok_or("no such basket item")?;
        let row = usize::try_from(got.item).ok().and_then(|i| items.get(i)).ok_or("no such item")?;
        if !row.fits(member, slot) {
            return Err("it does not fit there".into());
        }
        let m = self.party.get_mut(member).ok_or("no such member")?;
        let off = std::mem::replace(&mut m.equipment[slot], got.item);
        let off_unit = std::mem::replace(&mut m.worn[slot], Some(WornItem::from_got(got)));
        self.basket.remove(pick);
        if off >= 0 {
            // What was worn comes back as it was held (as made when only its number is known).
            self.basket.push(off_unit.filter(|w| w.item == off).map_or(crate::gather::Got { item: off, quality: 50.0, traits: Vec::new() }, |w| w.got()));
        }
        Ok(())
    }

    /// Whether place `k` (of the table `places`) is closed to the player.
    pub fn closed(&self, places: &[crate::data::world::Place], k: usize) -> bool {
        places.get(k).is_some_and(|p| crate::area_map::closed(p, self.opened.contains(&k), self.worn.get(&k).copied().unwrap_or(0)))
    }

    /// What chooses the gathering points' lists (no table level-up items yet).
    pub fn levels(&self) -> crate::gather::Levels {
        crate::gather::Levels { field: self.field_state, ..Default::default() }
    }

    /// Place `k` visited.
    pub fn visit(&mut self, k: usize) {
        self.visited.insert(k);
    }

    /// Whether place `k` has been visited.
    pub fn visited(&self, k: usize) -> bool {
        self.visited.contains(&k)
    }

    /// Which places the player knows, by place number.
    pub fn known(&self, places: &[crate::data::world::Place]) -> Vec<bool> {
        crate::area_map::known(places, &self.visited, |k| self.closed(places, k))
    }

    /// Which routes the player knows, by route number.
    pub fn known_routes(&self, places: &[crate::data::world::Place]) -> std::collections::BTreeSet<i32> {
        crate::area_map::known_routes(places, &self.visited, |k| self.closed(places, k))
    }

    /// Which routes the player has walked, by route number.
    pub fn walked_routes(&self, places: &[crate::data::world::Place]) -> std::collections::BTreeSet<i32> {
        crate::area_map::walked_routes(places, &self.visited)
    }

    /// Place `k`'s field gimmick used: it opens.
    pub fn open_place(&mut self, k: usize) {
        self.opened.insert(k);
    }

    /// A hit of `damage` on the obstacle before place `k`.
    pub fn wear(&mut self, k: usize, damage: i32) {
        *self.worn.entry(k).or_default() += damage.max(0);
    }

    /// Whether the player knows item `item` (has held it).
    pub fn knows(&self, item: i32) -> bool {
        self.known.contains(&item) || self.basket.iter().any(|g| g.item == item)
    }

    /// A won battle's drops (item, count, quality) into the basket, with no traits.
    pub fn found(&mut self, drops: &[(i32, i32, f32)]) {
        self.known.extend(drops.iter().map(|d| d.0));
        for &(item, count, quality) in drops {
            for _ in 0..count.max(0) {
                self.basket.push(crate::gather::Got { item, quality, traits: Vec::new() });
            }
        }
    }

    pub fn with_calendar(mut self, data: crate::days::Data, rng: barrel_engine::rng::Rng) -> Session {
        let leader = usize::from(self.logy);
        let mut world = crate::days::World::new_game(rng, leader, &data.wares);
        world.date = self.chain.story().today.max(0) as u32;
        world.tenths = self.carried;
        self.calendar = Some((world, data));
        self
    }

    /// Field time spent, in tenths of a day (a gathering: `days::gather_tenths`; a battle's
    /// time): whole days pass for `reason`.
    pub fn spend(&mut self, tenths: i32, reason: crate::days::Reason) {
        self.time(|w, t| w.spend_time(tenths, reason, t));
    }

    /// A gathering gave `got`: into the basket, and its field time passes (`days::gather_tenths`
    /// with no reduction: half a day).
    pub fn gathered(&mut self, got: Vec<crate::gather::Got>) {
        self.known.extend(got.iter().map(|g| g.item));
        self.basket.extend(got);
        self.spend(crate::days::gather_tenths(0), crate::days::Reason::Gather);
    }

    /// Which events have been seen, by event number.
    pub fn seen(&self) -> Vec<bool> {
        self.chain.story().seen.iter().map(Option::is_some).collect()
    }

    /// Recipe `r` made from the basket's entries `picks` (`cauldron::Book::make`): they leave
    /// the basket, the item made goes in, and the recipe's days pass (reason 2, synthesis).
    pub fn synthesize(&mut self, book: &crate::cauldron::Book, r: usize, picks: &[usize], roll: &mut dyn FnMut(i32, i32) -> i32) -> Result<crate::cauldron::Made, String> {
        let mut sorted = picks.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != picks.len() {
            return Err("a material is picked twice".into());
        }
        let made = book.make(r, picks, &self.basket, roll)?;
        for &k in sorted.iter().rev() {
            self.basket.remove(k);
        }
        self.basket.push(crate::gather::Got { item: made.item, quality: made.quality, traits: Vec::new() });
        self.spend((made.days * 10.0).round() as i32, crate::days::Reason::Synthesis);
        Ok(made)
    }

    /// Whether the event named `name` (`EVENT_*`) has been seen.
    pub fn event_seen(&self, name: &str) -> bool {
        let story = self.chain.story();
        self.chain.names().iter().position(|n| n == name).is_some_and(|i| story.seen.get(i).is_some_and(Option::is_some))
    }

    /// The tenths of today already spent in the field (0 without a calendar).
    pub fn field_tenths(&self) -> i32 {
        self.calendar.as_ref().map_or(0, |(w, _)| w.tenths)
    }

    /// The date (the story's day number; `calendar::Date` reads it).
    pub fn today(&self) -> i32 {
        self.chain.story().today
    }

    /// Runs a calendar step with its tables (what the story has seen; no assignments cleared,
    /// every request known, held counts 0: the session keeps no items yet), then the story's
    /// date follows the calendar's.
    fn time(&mut self, step: impl FnOnce(&mut crate::days::World, &crate::days::Tables) -> Vec<usize>) {
        let Some((world, data)) = self.calendar.as_mut() else { return };
        let story = self.chain.story();
        let names = self.chain.names();
        let seen = |e: &str| names.iter().position(|n| n == e).is_some_and(|i| story.seen.get(i).is_some_and(Option::is_some));
        let cost = |k: usize| data.recipe_costs.get(k).copied().unwrap_or(0);
        let t = crate::days::Tables {
            requests: &data.requests,
            wares: &data.wares,
            ranks: &data.ranks,
            recipe_cost: &cost,
            seen: &seen,
            cleared: &|_| false,
            known: &|_| true,
            level: &|_| 0,
        };
        step(world, &t);
        let date = world.date as i32;
        self.chain.story_mut().today = date;
    }

    /// A world-map trip from mark `from` to mark `to` (`travel::WorldMap`), leaving field `left`
    /// and arriving in field `field`: leaving a field area with time carried passes a day, the
    /// trip's days pass (on the calendar when there is one), then an event on the road or on
    /// arriving plays, else play goes on in `field`. `None` when there is no way there.
    pub fn travelled(&mut self, map: &crate::travel::WorldMap, left: u32, from: &str, to: &str, field: u32) -> Option<&Scene> {
        if left as usize >= crate::data::gimmick::TOWN_MAPS {
            self.time(|w, t| w.leave_field(t));
        }
        let before = self.today();
        let (event, days) = self.chain.travel(map, from, to, field as i32);
        let event = event.map(str::to_string);
        let days = days?;
        if self.calendar.is_some() {
            self.chain.story_mut().today = before;
            self.time(|w, t| w.advance(days.max(0) as u32, crate::days::Reason::Travel, t));
        }
        self.field = Some(field);
        self.scene = match event {
            Some(e) => Scene::Event(e),
            None => Scene::Field(field),
        };
        Some(&self.scene)
    }

    /// The story, to play events on ([`Session::in_field`] takes it back).
    pub fn into_chain(self) -> Chain {
        self.chain
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// The event playing has ended, having last changed to field `last_field` (if it changed
    /// field at all): the story's next event if one starts, else free play in the field.
    pub fn event_ended(&mut self, last_field: Option<u32>) -> &Scene {
        if let Some(f) = last_field {
            self.field = Some(f);
        }
        if let Scene::Event(name) = &self.scene {
            let name = name.clone();
            self.scene = match self.chain.after(&name) {
                Some(next) => Scene::Event(next.to_string()),
                // With no field yet the game would be nowhere; the Atelier (map 0) stands in.
                None => Scene::Field(self.field.unwrap_or(0)),
            };
        }
        &self.scene
    }
}

/// A session's saved state could not be read.
#[derive(Debug, PartialEq, Eq)]
pub struct BadSave(pub String);

impl std::fmt::Display for BadSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "save: {}", self.0)
    }
}

impl std::error::Error for BadSave {}

impl Session {
    /// The party members of saved text (its `member` lines, in order), for a run that has the
    /// text but no session: the battle a field encounter opens.
    pub fn party_in(text: &str) -> Vec<Member> {
        let mut party = text
            .lines()
            .filter_map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                if w.first() != Some(&"member") || w.len() != 5 {
                    return None;
                }
                let kit: Vec<i32> = w[4].split(',').filter_map(|v| v.parse().ok()).collect();
                Some(Member::fresh(w[2].parse().ok()?, w[3].parse().ok()?, kit.try_into().ok()?))
            })
            .collect::<Vec<_>>();
        for l in text.lines() {
            let w: Vec<&str> = l.split_whitespace().collect();
            if w.first() == Some(&"wear") {
                if let (Some(m), Some(k), Some(item)) = (w.get(1).and_then(|c| c.parse::<usize>().ok()).and_then(|c| party.get_mut(c)), w.get(2).and_then(|k| k.parse::<usize>().ok()).filter(|&k| k < 4), WornItem::parse(&w[3.min(w.len())..])) {
                    m.worn[k] = Some(item);
                }
                continue;
            }
            if w.first() != Some(&"carry") {
                continue;
            }
            if let (Some(m), Some(item)) = (w.get(1).and_then(|c| c.parse::<usize>().ok()).and_then(|c| party.get_mut(c)), BattleItem::parse(&w[2.min(w.len())..])) {
                m.battle_items.push(item);
            }
        }
        party
    }

    /// Saved text with a won battle's spoils paid in (as [`Session::won`]): for the battle a
    /// field encounter opens, which carries its field's save and hands it back.
    pub fn won_in_save(text: &str, fighters: &[usize], exp: i32, money: i32, drops: &[(i32, i32, f32)], exp_table: &[(i32, i32)]) -> String {
        let mut s = Session { party: Self::party_in(text), ..Session::bare(Chain::new(Vec::new(), Vec::new(), Vec::new())) };
        if let Some(m) = text.lines().find_map(|l| l.strip_prefix("money ")).and_then(|v| v.trim().parse().ok()) {
            s.money = m;
        }
        s.won(fighters, exp, money, exp_table);
        let mut out = String::new();
        let mut members = s.party.iter().enumerate();
        for line in text.lines() {
            if line.starts_with("money ") {
                out += &format!("money {}\n", s.money);
            } else if line.starts_with("member ") {
                if let Some((c, m)) = members.next() {
                    let kit: Vec<String> = m.equipment.iter().map(i32::to_string).collect();
                    out += &format!("member {c} {} {} {}\n", m.level, m.experience, kit.join(","));
                }
            } else {
                out += line;
                out += "\n";
            }
        }
        // The drops join the basket: its `item` lines (in any order: `load` reads them all).
        let mut found = Session { money: 0, ..Session::bare(Chain::new(Vec::new(), Vec::new(), Vec::new())) };
        found.found(drops);
        for g in &found.basket {
            out += &format!("item {} {}\n", g.item, g.quality);
        }
        out
    }

    /// Member `c`'s battle items get the uses a battle left them (`uses`: index among its items,
    /// uses left; `auto_battle::Tables::uses_left`). Spent ones stay carried at 0.
    pub fn use_items(&mut self, c: usize, uses: &[(usize, i32)]) {
        let Some(m) = self.party.get_mut(c) else { return };
        for &(k, n) in uses {
            if let Some(b) = m.battle_items.get_mut(k) {
                b.uses = n;
            }
        }
    }

    /// [`Session::use_items`] on a field save's text: member `c`'s `carry` lines, in their order.
    pub fn spent_in_save(text: &str, c: usize, uses: &[(usize, i32)]) -> String {
        let mut k = 0;
        let mut out = String::new();
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            if w.first() == Some(&"carry") && w.get(1).and_then(|v| v.parse::<usize>().ok()) == Some(c) {
                match (uses.iter().find(|u| u.0 == k), BattleItem::parse(&w[2.min(w.len())..])) {
                    (Some(&(_, n)), Some(mut b)) => {
                        b.uses = n;
                        out += &b.line(c);
                    }
                    _ => out += &format!("{line}\n"),
                }
                k += 1;
            } else {
                out += &format!("{line}\n");
            }
        }
        out
    }

    /// A lost battle in a field save's text: a day passes (`today` + 1; the game's wipe turns the
    /// date panel 8/20 → 8/21 on white, `docs/events.md` "A lost battle"); the rest is kept.
    /// Not yet the game's: the party comes back where the battle was touched, not on the area
    /// map at that area (Barrel has no area map), and their HP is not kept by Barrel's saves.
    pub fn lost_in_save(text: &str) -> String {
        text.lines()
            .map(|line| match line.strip_prefix("today ").and_then(|v| v.trim().parse::<i32>().ok()) {
                Some(day) => format!("today {}\n", day + 1),
                None => format!("{line}\n"),
            })
            .collect()
    }

    /// The session's state in free play, where the player stands at `at` (x, z field units),
    /// as text; `None` while an event plays (the game saves only in free play).
    pub fn save(&self, at: [f32; 2]) -> Option<String> {
        let Scene::Field(map) = self.scene else { return None };
        let story = self.chain.story();
        let mut out = format!("barrel-save 1\nlead {}\nfield {map} {} {}\ntoday {}\n", if self.logy { "logy" } else { "escha" }, at[0], at[1], story.today);
        let tenths = self.calendar.as_ref().map_or(self.carried, |(w, _)| w.tenths);
        if tenths != 0 {
            out += &format!("tenths {tenths}\n");
        }
        out += &format!("money {}\n", self.money);
        for (c, m) in self.party.iter().enumerate() {
            let kit: Vec<String> = m.equipment.iter().map(i32::to_string).collect();
            out += &format!("member {c} {} {} {}\n", m.level, m.experience, kit.join(","));
        }
        for (c, m) in self.party.iter().enumerate() {
            for (k, w) in m.worn.iter().enumerate() {
                if let Some(w) = w {
                    out += &w.line(c, k);
                }
            }
        }
        for (c, m) in self.party.iter().enumerate() {
            for b in &m.battle_items {
                out += &b.line(c);
            }
        }
        for g in &self.basket {
            let traits: Vec<String> = g.traits.iter().map(usize::to_string).collect();
            out += &format!("item {} {}{}\n", g.item, g.quality, if traits.is_empty() { String::new() } else { format!(" {}", traits.join(",")) });
        }
        let known: Vec<String> = self.known.iter().chain(self.basket.iter().map(|g| &g.item)).collect::<std::collections::BTreeSet<_>>().into_iter().map(i32::to_string).collect();
        if !known.is_empty() {
            out += &format!("known {}\n", known.join(","));
        }
        for k in &self.opened {
            out += &format!("opened {k}\n");
        }
        for k in &self.visited {
            out += &format!("visited {k}\n");
        }
        if self.field_gauge != 0 || self.field_state != crate::gather::FieldState::default() {
            let [a, b, c] = self.field_state.values;
            out += &format!("field-effect {} {} {a} {b} {c}\n", self.field_gauge, self.field_state.mode);
        }
        for (k, t) in self.taken.iter().enumerate() {
            if let Some(t) = t {
                out += &format!("taken {k} {} {} {} {} {} {} {} {} {} {} {}\n", t.kind, t.target, t.progress, t.wanted, t.posted, t.until, t.points, t.prizes[0].0, t.prizes[0].1, t.prizes[1].0, t.prizes[1].1);
            }
        }
        if self.rank != (0, 0) {
            out += &format!("rank {} {}\n", self.rank.0, self.rank.1);
        }
        for (k, d) in &self.worn {
            out += &format!("worn {k} {d}\n");
        }
        for (e, day) in story.seen.iter().enumerate() {
            if let Some(d) = day {
                out += &format!("seen {e} {d}\n");
            }
        }
        for (f, v) in story.flags.iter().enumerate().filter(|(_, v)| **v != 0) {
            out += &format!("flag {f} {v}\n");
        }
        Some(out)
    }

    /// A session from saved text over the story's `chain` (a fresh one, as for a new game):
    /// back in free play where it was saved, with the player's spot.
    pub fn load(mut chain: Chain, text: &str) -> Result<(Session, [f32; 2]), BadSave> {
        let bad = |line: &str| BadSave(format!("bad line: {line}"));
        let mut lines = text.lines();
        if lines.next() != Some("barrel-save 1") {
            return Err(BadSave("not a Barrel save (version 1)".into()));
        }
        let (mut logy, mut field, mut at, mut carried, mut basket) = (false, None, [0.0f32; 2], 0, Vec::new());
        let (mut money, mut party) = (NEW_GAME_MONEY, Vec::<Member>::new());
        let mut known = std::collections::BTreeSet::new();
        let (mut opened, mut worn) = (std::collections::BTreeSet::new(), std::collections::BTreeMap::new());
        let mut visited = std::collections::BTreeSet::new();
        let (mut field_gauge, mut field_state) = (0, crate::gather::FieldState::default());
        let mut taken = [None; TAKEN_SLOTS];
        let mut rank = (0, 0);
        let story = chain.story_mut();
        story.seen.iter_mut().for_each(|s| *s = None);
        story.flags.iter_mut().for_each(|f| *f = 0);
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let w: Vec<&str> = line.split_whitespace().collect();
            let num = |k: usize| w.get(k).and_then(|v| v.parse::<i64>().ok()).ok_or_else(|| bad(line));
            match w[0] {
                "lead" => logy = w.get(1) == Some(&"logy"),
                "field" => {
                    field = Some(u32::try_from(num(1)?).map_err(|_| bad(line))?);
                    at = [w.get(2).and_then(|v| v.parse().ok()).ok_or_else(|| bad(line))?, w.get(3).and_then(|v| v.parse().ok()).ok_or_else(|| bad(line))?];
                }
                "today" => story.today = num(1)? as i32,
                "tenths" => carried = num(1)? as i32,
                "money" => money = (num(1)? as i32).clamp(0, MAX_MONEY),
                "member" => {
                    let c = num(1)? as usize;
                    if c != party.len() {
                        return Err(bad(line));
                    }
                    let kit: Vec<i32> = w.get(4).ok_or_else(|| bad(line))?.split(',').map(|v| v.parse().map_err(|_| bad(line))).collect::<Result<_, _>>()?;
                    let equipment: [i32; 4] = kit.try_into().map_err(|_| bad(line))?;
                    party.push(Member::fresh(num(2)? as i32, num(3)? as i32, equipment));
                }
                "wear" => {
                    let item = WornItem::parse(&w[3.min(w.len())..]).ok_or_else(|| bad(line))?;
                    let slot = party.get_mut(num(1)? as usize).and_then(|m| m.worn.get_mut(num(2).ok()? as usize)).ok_or_else(|| bad(line))?;
                    *slot = Some(item);
                }
                "carry" => {
                    let item = BattleItem::parse(&w[2.min(w.len())..]).ok_or_else(|| bad(line))?;
                    party.get_mut(num(1)? as usize).ok_or_else(|| bad(line))?.battle_items.push(item);
                }
                "item" => basket.push(crate::gather::Got {
                    item: num(1)? as i32,
                    quality: w.get(2).and_then(|v| v.parse().ok()).ok_or_else(|| bad(line))?,
                    traits: w.get(3).map_or(Ok(Vec::new()), |t| t.split(',').map(|v| v.parse().map_err(|_| bad(line))).collect())?,
                }),
                "known" => known.extend(w.get(1).ok_or_else(|| bad(line))?.split(',').map(|v| v.parse::<i32>().map_err(|_| bad(line))).collect::<Result<Vec<_>, _>>()?),
                "field-effect" => {
                    field_gauge = num(1)? as i32;
                    field_state = crate::gather::FieldState { mode: num(2)? as i32, values: [num(3)? as i32, num(4)? as i32, num(5)? as i32] };
                }
                "taken" => {
                    let k = num(1)? as usize;
                    let slot = taken.get_mut(k).ok_or_else(|| bad(line))?;
                    // The rewards came later: a line without them pays nothing.
                    let more = |k: usize, none: i32| num(k).map_or(none, |v| v as i32);
                    *slot = Some(Taken {
                        kind: num(2)? as i32,
                        target: num(3)? as i32,
                        progress: num(4)? as i32,
                        wanted: num(5)? as i32,
                        posted: num(6)? as i32,
                        until: num(7)? as i32,
                        points: more(8, 0),
                        prizes: [(more(9, -1), more(10, 0)), (more(11, -1), more(12, 0))],
                    });
                }
                "rank" => {
                    rank = (num(1)? as i32, num(2)? as i32);
                }
                "visited" => {
                    visited.insert(num(1)? as usize);
                }
                "opened" => {
                    opened.insert(num(1)? as usize);
                }
                "worn" => {
                    worn.insert(num(1)? as usize, num(2)? as i32);
                }
                "seen" => *story.seen.get_mut(num(1)? as usize).ok_or_else(|| bad(line))? = Some(num(2)? as i32),
                "flag" => *story.flags.get_mut(num(1)? as usize).ok_or_else(|| bad(line))? = num(2)? as i32,
                _ => return Err(bad(line)),
            }
        }
        let field = field.ok_or_else(|| BadSave("no field line".into()))?;
        story.field_map = Some(field as i32);
        known.extend(basket.iter().map(|g: &crate::gather::Got| g.item));
        Ok((Session { carried, basket, scene: Scene::Field(field), field: Some(field), logy, money, party, known, opened, worn, visited, field_gauge, field_state, taken, rank, ..Session::bare(chain) }, at))
    }

    /// A session from either save format, by its bytes: Barrel's text (it starts `barrel-save`)
    /// or the game's `GAMEDATA<NN>.pcsave` ([`Session::from_game_save`]).
    pub fn open(chain: Chain, bytes: &[u8], landmarks: &crate::data::world::Landmarks) -> Result<(Session, [f32; 2]), BadSave> {
        if bytes.starts_with(b"barrel-save") {
            return Session::load(chain, std::str::from_utf8(bytes).map_err(|_| BadSave("not text".into()))?);
        }
        Session::from_game_save(chain, bytes, landmarks)
    }

    /// A session from the game's own save (`GAMEDATA<NN>.pcsave`), over the story's `chain` (a
    /// fresh one): in free play where the save was made. The save holds that as a world map
    /// point, not a field and a spot, so the player stands where the field's arrival place is
    /// (the game saves on the area map, and the next field entered starts there). What Barrel's
    /// text format holds is read; the rest of the save is ignored.
    pub fn from_game_save(mut chain: Chain, bytes: &[u8], landmarks: &crate::data::world::Landmarks) -> Result<(Session, [f32; 2]), BadSave> {
        use crate::data::save;
        let bad = |e: barrel_formats::Error| BadSave(format!("game save: {e}"));
        let state = save::decode(save::parse(bytes).map_err(bad)?.body).map_err(bad)?;
        let sum = save::summary(&save::state_records(&state).map_err(bad)?);
        let missing = |what: &str| BadSave(format!("game save: no {what}"));
        let point = sum.world_position.ok_or_else(|| missing("world position"))?.0;
        let (field, at) = landmarks.field_at(point).ok_or_else(|| BadSave(format!("game save: world map point {point} opens no field")))?;
        let story = chain.story_mut();
        story.today = i32::try_from(sum.day.ok_or_else(|| missing("date"))?).map_err(|_| missing("date"))?;
        story.seen = vec![None; story.seen.len()];
        for (slot, e) in story.seen.iter_mut().zip(&sum.events) {
            *slot = e.seen.then_some(e.day);
        }
        story.flags = vec![0; story.flags.len()];
        for (slot, &v) in story.flags.iter_mut().zip(&sum.event_flags) {
            *slot = v;
        }
        story.field_map = Some(field as i32);
        // The party in character order; the alchemists carry their battle items.
        let mut by_character: Vec<_> = sum.party.iter().collect();
        by_character.sort_by_key(|m| m.character);
        if by_character.iter().enumerate().any(|(k, m)| usize::try_from(m.character) != Ok(k)) {
            return Err(BadSave("game save: party records are not characters 0 to n".into()));
        }
        let party = by_character
            .iter()
            .map(|m| Member {
                level: m.level,
                experience: m.exp,
                equipment: m.equipment,
                worn: m.units.each_ref().map(|u| u.as_ref().map(WornItem::from_unit)),
                now: Some([m.hp, m.equipped[0], m.mp, m.equipped[1]]),
                battle_items: sum.battle_items.get(m.character as usize).into_iter().flatten().map(BattleItem::from_unit).collect(),
            })
            .collect();
        let basket: Vec<crate::gather::Got> = sum.basket.iter().map(|u| crate::gather::Got { item: i32::from(u.item), quality: u.quality, traits: held_traits(u) }).collect();
        // The library's items seen or better (2 and up), as the text's `known` holds them.
        let mut known: std::collections::BTreeSet<i32> = sum.knowledge.item.iter().enumerate().filter(|&(_, &l)| l >= 2).map(|(k, _)| k as i32).collect();
        known.extend(basket.iter().map(|g| g.item));
        let visited = sum.place_knowledge.iter().enumerate().filter(|&(_, &l)| l == KNOWLEDGE_ALL).map(|(k, _)| k).collect();
        // The places' states: a place behind a gimmick is open once its flag is clear, one
        // behind an obstacle has its durability worn down.
        let (mut opened, mut worn) = (std::collections::BTreeSet::new(), std::collections::BTreeMap::new());
        for (k, (place, state)) in landmarks.places.iter().zip(&sum.place_states).enumerate() {
            match place.extra[0] {
                0 if !state.1 => {
                    opened.insert(k);
                }
                1 if place.extra[1] - state.0 > 0 => {
                    worn.insert(k, place.extra[1] - state.0);
                }
                _ => {}
            }
        }
        let (field_gauge, field_state) = sum.field_effect.map_or((0, Default::default()), |(gauge, mode, values)| (gauge, crate::gather::FieldState { mode, values }));
        let mut taken = [None; TAKEN_SLOTS];
        for (slot, t) in taken.iter_mut().zip(&sum.taken) {
            // The points are paid truncated and each prize counted 1 or more (`BoardSlot`).
            *slot = t.map(|t| Taken {
                kind: t.kind,
                target: t.target,
                wanted: t.wanted,
                progress: t.progress,
                posted: t.posted,
                until: t.until,
                points: t.value as i32,
                prizes: t.prizes.map(|(item, count)| (item, if item < 0 { 0 } else { (count as i32).max(1) })),
            });
        }
        Ok((
            Session {
                basket,
                scene: Scene::Field(field),
                field: Some(field),
                logy: sum.hero == Some(1),
                money: sum.money.ok_or_else(|| missing("money"))?.clamp(0, MAX_MONEY),
                party,
                squad: sum.squad.iter().filter_map(|&c| usize::try_from(c).ok()).collect(),
                known,
                opened,
                worn,
                visited,
                field_gauge,
                field_state,
                taken,
                rank: sum.rank.unwrap_or((0, 0)),
                friendship: by_character.iter().map(|m| m.friendship).collect(),
                alchemy: sum.alchemy.unwrap_or((0, 0)),
                library_new: sum.library_new,
                ..Session::bare(chain)
            },
            at,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> Chain {
        Chain::new(Vec::new(), Vec::new(), vec!["EVENT_A".into()])
    }

    #[test]
    fn taken_requests_survive_a_save() {
        let mut s = Session::in_field(chain(), 12, false);
        s.taken[0] = Some(Taken { kind: 2, target: 44, wanted: 2, progress: 2, posted: 220, until: 329, points: 6, prizes: [(12, 2), (-1, 0)] });
        s.taken[3] = Some(Taken { kind: 0, target: 4, wanted: 1, progress: 0, posted: 221, until: 329, points: 0, prizes: [(-1, 0); 2] });
        let (back, _) = Session::load(chain(), &s.save([0.0, 0.0]).unwrap()).unwrap();
        assert_eq!(back.taken, s.taken);
        assert!(back.taken[0].unwrap().done() && !back.taken[3].unwrap().done());
    }

    #[test]
    fn known_items_outlast_the_basket_and_a_save() {
        let mut s = Session::in_field(chain(), 12, false);
        s.gathered(vec![crate::gather::Got { item: 75, quality: 20.0, traits: Vec::new() }]);
        s.basket.clear();
        assert!(s.knows(75) && !s.knows(12));
        let text = s.save([0.0, 0.0]).unwrap();
        assert!(text.contains("known 75\n"));
        let (back, _) = Session::load(chain(), &text).unwrap();
        assert!(back.knows(75) && !back.knows(12));
    }

    #[test]
    fn opened_places_and_worn_obstacles_survive_a_save() {
        let place = |extra| crate::data::world::Place { name: String::new(), tag: String::new(), kind: 1, links: [-1; 6], description: String::new(), extra };
        let places = [place([1, 30]), place([0, 31])];
        let mut s = Session::in_field(chain(), 12, false);
        assert!(s.closed(&places, 0) && s.closed(&places, 1));
        s.wear(0, 30);
        s.open_place(1);
        s.visit(1);
        s.field_gauge = 45;
        s.field_state = crate::gather::FieldState { mode: 4, values: [2, -1, -1] };
        let (back, _) = Session::load(chain(), &s.save([0.0, 0.0]).unwrap()).unwrap();
        assert!(!back.closed(&places, 0) && !back.closed(&places, 1));
        assert_eq!(back.known(&places), [false, true]);
        assert_eq!((back.field_gauge, back.levels().tier(false)), (45, 2));
    }

    #[test]
    fn a_session_saves_and_loads() {
        let mut s = Session::new_game(chain(), true);
        assert_eq!(s.save([0.0, 0.0]), None, "no saving while an event plays");
        s.chain.story_mut().seen[8] = Some(90);
        s.chain.story_mut().flags[36] = 1;
        s.scene = Scene::Field(3);
        let text = s.save([10000.0, 10950.5]).unwrap();
        assert_eq!(text, "barrel-save 1\nlead logy\nfield 3 10000 10950.5\ntoday 90\nmoney 100\nseen 8 90\nflag 36 1\n");
        let (back, at) = Session::load(chain(), &text).unwrap();
        assert_eq!(at, [10000.0, 10950.5]);
        assert!(back.logy);
        assert_eq!(back.scene(), &Scene::Field(3));
        assert_eq!(back.save(at).unwrap(), text);
        assert!(Session::load(chain(), "barrel-save 2\n").is_err());
    }

    #[test]
    fn field_time_passes_days() {
        let data = crate::days::Data { requests: Vec::new(), wares: Vec::new(), ranks: Vec::new(), recipe_costs: Vec::new() };
        let mut s = Session::in_field(chain(), 12, false).with_calendar(data, barrel_engine::rng::Rng::seeded(1));
        assert_eq!(s.today(), 90);
        // Two and a half days of gathering: two pass, half a day is carried.
        s.spend(25, crate::days::Reason::Gather);
        assert_eq!(s.today(), 92);
        assert!(s.save([0.0, 0.0]).unwrap().contains("tenths 5\n"));
        // Leaving the area with time carried: a day more, nothing carried.
        s.walked(12, 13);
        assert_eq!(s.today(), 93);
        s.walked(13, 0);
        assert_eq!(s.today(), 93);
        // A gathering: into the basket, half a day; saved and loaded back.
        s.gathered(vec![crate::gather::Got { item: 55, quality: 60.0, traits: vec![12, 40] }]);
        s.gathered(Vec::new());
        assert_eq!(s.today(), 94);
        let text = s.save([0.0, 0.0]).unwrap();
        assert!(text.contains("item 55 60 12,40\n"), "{text}");
        assert_eq!(Session::load(chain(), &text).unwrap().0.basket, s.basket);
        // Town maps carry nothing.
        s.spend(5, crate::days::Reason::Gather);
        s.walked(0, 1);
        assert_eq!(s.today(), 94);
    }

    #[test]
    fn money_and_the_party_save_and_load() {
        let kit = |e: [i32; 4]| crate::data::battle_data::PartyDefault { flag: 0, level: 1, experience: 0, stats: crate::data::battle_data::LevelStats { hp: 1, mp: 1, attack: 1, defense: 1, speed: 1 }, more: [0; 5], equipment: e };
        let mut s = Session::in_field(chain(), 3, false).with_party(&[kit([364, 417, -1, -1]), kit([369, 438, -1, -1])]);
        assert_eq!((s.money, s.party.len(), s.party[1].equipment[1]), (100, 2, 438));
        assert!(s.pay(-60) && !s.pay(-41));
        assert!(s.pay(2_000_000) && s.money == MAX_MONEY);
        s.party[0].level = 4;
        s.party[0].experience = 40;
        let text = s.save([1.0, 2.0]).unwrap();
        assert!(text.contains("money 999999\nmember 0 4 40 364,417,-1,-1\nmember 1 1 0 369,438,-1,-1\n"), "{text}");
        let (back, _) = Session::load(chain(), &text).unwrap();
        assert_eq!((back.money, &back.party), (MAX_MONEY, &s.party));
        assert_eq!(Session::party_in(&text), s.party);
        // A save from before these lines: a new game's money, the party filled in later.
        let (old, _) = Session::load(chain(), "barrel-save 1\nfield 3 0 0\ntoday 90\n").unwrap();
        assert_eq!((old.money, old.party.len()), (NEW_GAME_MONEY, 0));
    }

    #[test]
    fn a_won_battle_pays_and_grows_the_fighters() {
        let kit = |e: [i32; 4]| crate::data::battle_data::PartyDefault { flag: 0, level: 1, experience: 0, stats: crate::data::battle_data::LevelStats { hp: 1, mp: 1, attack: 1, defense: 1, speed: 1 }, more: [0; 5], equipment: e };
        let mut s = Session::in_field(chain(), 3, false).with_party(&[kit([-1; 4]), kit([-1; 4]), kit([-1; 4])]);
        let table = [(0, 0), (10, 0), (20, 0), (40, 0)];
        s.won(&[0, 1], 25, 12, &table);
        assert_eq!(s.money, 112);
        assert_eq!((s.party[0].level, s.party[0].experience), (3, 25));
        assert_eq!((s.party[2].level, s.party[2].experience), (1, 0), "a member not fighting gains nothing");
        // The same through saved text (the battle screen's way back).
        let mut t = Session::in_field(chain(), 3, false).with_party(&[kit([-1; 4]), kit([-1; 4]), kit([-1; 4])]);
        let text = t.save([0.0, 0.0]).unwrap();
        t.won(&[0, 1], 25, 12, &table);
        assert_eq!(Session::won_in_save(&text, &[0, 1], 25, 12, &[], &table), t.save([0.0, 0.0]).unwrap());
        // Drops land in the basket, through the save too.
        let with = Session::won_in_save(&text, &[0, 1], 25, 12, &[(111, 2, 50.0)], &table);
        let (back, _) = Session::load(chain(), &with).unwrap();
        assert_eq!(back.basket.iter().map(|g| (g.item, g.quality)).collect::<Vec<_>>(), vec![(111, 50.0), (111, 50.0)]);
    }

    #[test]
    fn equipping_swaps_with_the_basket_and_survives_a_save() {
        use crate::equipment::ItemRow;
        let row = |slots: &'static [usize], wearers: u16| ItemRow { equip: true, stats: [0; 9], slots, wearers };
        // Item 0 a material, 1 Escha's staff, 2 Logy's sword, 3 an accessory for all.
        let items = [ItemRow { equip: false, stats: [0; 9], slots: &[], wearers: 0 }, row(&[0], 1), row(&[0], 2), row(&[2, 3], 0x7ff)];
        let mut s = Session::in_field(chain(), 3, false);
        s.party = vec![Member { level: 1, experience: 0, equipment: [-1; 4], worn: Default::default(), now: None, battle_items: vec![BattleItem { item: 270, quality: 55.0, uses: 2, effects: vec![12, -1], traits: vec![3] }] }, Member { level: 1, experience: 0, equipment: [2, -1, -1, -1], worn: Default::default(), battle_items: Vec::new(), now: None }];
        s.basket = [0, 1, 3].iter().map(|&item| crate::gather::Got { item, quality: 60.0, traits: Vec::new() }).collect();
        assert!(s.equip(0, 0, 0, &items).is_err(), "a material is no weapon");
        assert!(s.equip(1, 0, 1, &items).is_err(), "Escha's staff is not Logy's");
        assert!(s.equip(0, 1, 1, &items).is_err(), "a weapon is no armour");
        s.equip(1, 3, 2, &items).unwrap();
        assert_eq!(s.party[1].equipment, [2, -1, -1, 3]);
        // Logy's sword is not his to take off for Escha's staff, but Escha takes hers.
        s.equip(0, 0, 1, &items).unwrap();
        assert_eq!(s.party[0].equipment[0], 1);
        assert_eq!(s.basket.iter().map(|g| g.item).collect::<Vec<_>>(), vec![0]);
        let text = s.save([0.0, 0.0]).unwrap();
        let (back, _) = Session::load(chain(), &text).unwrap();
        assert_eq!(back.party, s.party);
        assert_eq!(Session::party_in(&text), s.party);
        // Worn units (equip put the basket's items on) are in the text and come back.
        assert!(text.contains("wear 0 0 1 60 - -"), "{text}");
        assert!(s.party[0].worn[0].is_some() && back.party[0].worn[0] == s.party[0].worn[0]);
        let mut with_effects = s.party.clone();
        with_effects[1].worn[0] = Some(WornItem { item: 2, quality: 69.5, effects: vec![4, 9], traits: vec![7, 8], traits_count: false });
        s.party = with_effects;
        let (again, _) = Session::load(chain(), &s.save([0.0, 0.0]).unwrap()).unwrap();
        assert_eq!(again.party[1].worn[0], s.party[1].worn[0]);
        s.party[1].worn[0] = None;
        // A battle spent one of Escha's two uses.
        let spent = Session::spent_in_save(&text, 0, &[(0, 1)]);
        let mut after = s.party.clone();
        after[0].battle_items[0].uses = 1;
        assert_eq!(Session::party_in(&spent), after);
        let mut t = back;
        t.use_items(0, &[(0, 1)]);
        assert_eq!(t.party, after);
    }

    #[test]
    fn a_new_game_carries_no_battle_items() {
        // `0x140364ff0` empties both alchemists' battle item lists.
        let kit = |e: [i32; 4]| crate::data::battle_data::PartyDefault { flag: 0, level: 1, experience: 0, stats: crate::data::battle_data::LevelStats { hp: 1, mp: 1, attack: 1, defense: 1, speed: 1 }, more: [0; 5], equipment: e };
        let s = Session::in_field(chain(), 3, false).with_party(&[kit([364, 417, -1, -1]), kit([369, 438, -1, -1])]);
        assert!(s.party.iter().all(|m| m.battle_items.is_empty()));
        let text = s.save([0.0, 0.0]).unwrap();
        assert!(!text.lines().any(|l| l.starts_with("carry ")));
    }

    #[test]
    fn a_lost_battle_costs_a_day() {
        let s = Session::in_field(chain(), 3, false);
        let text = s.save([0.0, 0.0]).unwrap();
        let (back, _) = Session::load(chain(), &Session::lost_in_save(&text)).unwrap();
        assert_eq!(back.today(), s.today() + 1);
        assert_eq!((back.money, back.party.clone(), back.basket.len()), (s.money, s.party.clone(), s.basket.len()));
    }

    #[test]
    fn buying_and_selling_move_money_and_survive_a_save() {
        let mut s = Session::in_field(chain(), 3, false);
        // Two Crafts (cost 30) bought at 70 each, quality 50.
        assert!(s.buy(195, 50.0, 70, 1));
        assert!(!s.buy(195, 50.0, 70, 1), "100 - 70 leaves 30: not enough for another");
        assert_eq!((s.money, s.basket.len()), (30, 1));
        s.basket.push(crate::gather::Got { item: 195, quality: 50.0, traits: Vec::new() });
        // Sold back: worth trunc(150 × 30 / 200) = 22 each, a 10% bonus: trunc(44 × 1.1) = 48.
        let paid = s.sell(&[1, 0, 1], |_| 30, 10);
        assert_eq!((paid, s.money, s.basket.len()), (48, 78, 0));
        let (back, _) = Session::load(chain(), &s.save([0.0, 0.0]).unwrap()).unwrap();
        assert_eq!(back.money, 78);
    }

    /// The game's own save of one moment of play against Barrel's text save of the same moment
    /// (made from it earlier, by another route). Skipped without the files:
    /// `BARREL_GAME_SAVE` (a `GAMEDATA<NN>.pcsave`), `BARREL_TEXT_SAVE` (the text) and
    /// `BARREL_EXE` (the game's executable).
    #[test]
    fn a_game_save_loads_as_the_text_save_of_it() {
        let root = "../escha-recomp/work";
        let var = |name: &str, default: String| std::env::var(name).unwrap_or(default);
        let game = var("BARREL_GAME_SAVE", format!("{root}/drive_c/users/player/Documents/KoeiTecmo/Atelier Escha and Logy DX/SAVEDATA/GAMEDATA00.pcsave"));
        let text = var("BARREL_TEXT_SAVE", format!("{root}/saves/barrel-user.save"));
        let exe = var("BARREL_EXE", format!("{root}/game/Atelier_Escha_and_Logy_EN.exe"));
        let (Ok(game), Ok(text), Ok(exe)) = (std::fs::read(&game), std::fs::read_to_string(&text), std::fs::read(&exe)) else {
            eprintln!("skipped: the game save, the text save or the executable is not there");
            return;
        };
        let landmarks = crate::data::world::Landmarks::from_exe(&barrel_formats::pe::Pe::parse(&exe).unwrap()).unwrap();
        let (want, want_at) = Session::load(Chain::new(Vec::new(), Vec::new(), Vec::new()), &text).unwrap();
        let (got, got_at) = Session::open(Chain::new(Vec::new(), Vec::new(), Vec::new()), &game, &landmarks).unwrap();
        assert_eq!((got.logy, got.field(), got_at), (want.logy, want.field(), want_at), "lead, field, position");
        assert_eq!(got.today(), want.today(), "today");
        assert_eq!(got.field_tenths(), want.field_tenths());
        assert_eq!(got.money, want.money, "money");
        assert_eq!(got.party.len(), want.party.len(), "party size");
        for (c, (g, w)) in got.party.iter().zip(&want.party).enumerate() {
            assert_eq!((g.level, g.experience, g.equipment), (w.level, w.experience, w.equipment), "member {c}");
        }
        assert_eq!(got.basket, want.basket, "basket");
        assert_eq!(got.known, want.known, "known items");
        assert_eq!(got.visited, want.visited, "visited places");
        assert_eq!(got.opened, want.opened, "opened places");
        assert_eq!(got.worn, want.worn, "worn obstacles");
        assert_eq!((got.field_gauge, got.field_state), (want.field_gauge, want.field_state), "field effect");
        // The text save predates the rewards (points, prizes) and the rank of a taken request.
        let bare = |s: &Session| s.taken.map(|t| t.map(|t| (t.kind, t.target, t.wanted, t.progress, t.posted, t.until)));
        assert_eq!(bare(&got), bare(&want), "taken requests");
        assert_eq!(got.rank, (3, 1565), "rank and points");
        assert_eq!(got.chain.story().seen, want.chain.story().seen, "events seen");
        assert_eq!(got.chain.story().flags, want.chain.story().flags, "event flags");
        // The text save predates its `carry` lines: the game's save holds the alchemists' battle items.
        assert!(want.party.iter().all(|m| m.battle_items.is_empty()));
        assert!(!got.party[0].battle_items.is_empty());
    }
}
