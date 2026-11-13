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
//! item 55 60 12,40
//! seen 8 90
//! flag 36 1
//! ```

use crate::event_start::{new_game_event, Chain};

/// The money a new game starts with (`0x14036a8c0`).
pub const NEW_GAME_MONEY: i32 = 100;
/// The most money held (`0x14036a8e0`).
pub const MAX_MONEY: i32 = 999_999;

/// One party member's lasting state (its record at `0x1410bcb60` + 0x2b0 × member).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub level: i32,
    pub experience: i32,
    /// The items equipped, by item number, -1 for none: weapon, armour, two accessories.
    pub equipment: [i32; 4],
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
}

impl Session {
    /// A new game with the lead chosen (`logy`), opening with its first event.
    pub fn new_game(chain: Chain, logy: bool) -> Session {
        Session { chain, calendar: None, carried: 0, basket: Vec::new(), scene: Scene::Event(new_game_event(logy, false).to_string()), field: None, logy, money: NEW_GAME_MONEY, party: Vec::new() }
    }

    /// A session already in free play in field `map` with the story where `chain` has it.
    pub fn in_field(chain: Chain, map: u32, logy: bool) -> Session {
        Session { chain, calendar: None, carried: 0, basket: Vec::new(), scene: Scene::Field(map), field: Some(map), logy, money: NEW_GAME_MONEY, party: Vec::new() }
    }

    /// The player walks from field `from` to field `to` in free play: the event that starts as
    /// they leave or once they are there (`Chain::walk`, both fields of kind 0), which then
    /// plays; else play goes on in `to`.
    pub fn walked(&mut self, from: u32, to: u32) -> &Scene {
        // Leaving a field area (not the town) with field time carried passes a day.
        if from as usize >= barrel_formats::gimmick::TOWN_MAPS && from != to {
            self.time(|w, t| w.leave_field(t));
        }
        self.field = Some(to);
        self.scene = match self.chain.walk([0, from as i32], [0, to as i32]) {
            Some(e) => Scene::Event(e.to_string()),
            None => Scene::Field(to),
        };
        &self.scene
    }

    /// The calendar from `data`, starting as a new game's (`days::World::new_game`, the main
    /// generator as `rng`) on the story's date.
    /// The party as a new game sets it up (`0x1403619d0`): every member at level 1 with no
    /// experience, in its default kit (`battle_data::party_defaults`). Members a loaded save
    /// already holds keep their own.
    pub fn with_party(mut self, defaults: &[barrel_formats::battle_data::PartyDefault]) -> Session {
        for d in defaults.iter().skip(self.party.len()) {
            self.party.push(Member { level: 1, experience: 0, equipment: d.equipment });
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

    pub fn with_calendar(mut self, data: crate::days::Data, rng: crate::rng::Rng) -> Session {
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
        if left as usize >= barrel_formats::gimmick::TOWN_MAPS {
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
        text.lines()
            .filter_map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                if w.first() != Some(&"member") || w.len() != 5 {
                    return None;
                }
                let kit: Vec<i32> = w[4].split(',').filter_map(|v| v.parse().ok()).collect();
                Some(Member { level: w[2].parse().ok()?, experience: w[3].parse().ok()?, equipment: kit.try_into().ok()? })
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
        for g in &self.basket {
            let traits: Vec<String> = g.traits.iter().map(usize::to_string).collect();
            out += &format!("item {} {}{}\n", g.item, g.quality, if traits.is_empty() { String::new() } else { format!(" {}", traits.join(",")) });
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
                    party.push(Member { level: num(2)? as i32, experience: num(3)? as i32, equipment });
                }
                "item" => basket.push(crate::gather::Got {
                    item: num(1)? as i32,
                    quality: w.get(2).and_then(|v| v.parse().ok()).ok_or_else(|| bad(line))?,
                    traits: w.get(3).map_or(Ok(Vec::new()), |t| t.split(',').map(|v| v.parse().map_err(|_| bad(line))).collect())?,
                }),
                "seen" => *story.seen.get_mut(num(1)? as usize).ok_or_else(|| bad(line))? = Some(num(2)? as i32),
                "flag" => *story.flags.get_mut(num(1)? as usize).ok_or_else(|| bad(line))? = num(2)? as i32,
                _ => return Err(bad(line)),
            }
        }
        let field = field.ok_or_else(|| BadSave("no field line".into()))?;
        story.field_map = Some(field as i32);
        Ok((Session { chain, calendar: None, carried, basket, scene: Scene::Field(field), field: Some(field), logy, money, party }, at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> Chain {
        Chain::new(Vec::new(), Vec::new(), vec!["EVENT_A".into()])
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
        let mut s = Session::in_field(chain(), 12, false).with_calendar(data, crate::rng::Rng::seeded(1));
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
        let kit = |e: [i32; 4]| barrel_formats::battle_data::PartyDefault { flag: 0, level: 1, experience: 0, stats: barrel_formats::battle_data::LevelStats { hp: 1, mp: 1, attack: 1, defense: 1, speed: 1 }, more: [0; 5], equipment: e };
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
}
