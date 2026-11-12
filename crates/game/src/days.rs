//! Days passing: the state the calendar touches and the work of each new day, in the game's
//! order (`docs/tables.md`, "The calendar"; English Escha executable `0x14035bf10` and
//! `0x14035bb50`). A new game starts on April 1st of school year 1 and draws the request
//! board's seed offset from the main generator.
//!
//! After the days, the shops restock (`0x14035b930` → `0x14035c560`, with the number of days).
//!
//! Left out: the request expiry pass of each day, the manager's monthly work (`0x140267f50`),
//! the queued messages, the characters' recovery and the field objects of `0x14035b930`, and
//! the parts of the board's tests not read yet (see [`crate::quest::draw_board`]).

use crate::calendar;
use crate::quest::{self, Request};
use crate::rank::RankState;
use crate::shop::{self, Stock, Ware};
use crate::rng::Rng;

/// Why days pass (the second argument of `0x14035bf10`; names from the callers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// 1: `0x140052b5a` and two more callers.
    One,
    /// 2: a synthesis's days.
    Synthesis,
    /// 3: one day, `0x14003412b`.
    Three,
    /// 4: world-map travel, presumably.
    Travel,
    /// 7: one day, from field code.
    Field,
    /// 8: one day, `0x14039b3d2`.
    Eight,
}

/// One month's report (`0x141093570` + 0x144 × the month, April first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonthReport {
    /// Syntheses of each of the 265 recipes this month.
    pub recipes: Vec<u8>,
    /// Battles, gatherings, bou, syntheses, tra, rel, requests (+0x10c to +0x124).
    pub counts: [i32; 7],
    /// Points made this month (+0x138).
    pub points: i32,
    /// The month's total, set when it closes (+0x134).
    pub total: i32,
    /// The rank when it closed (+0x13c).
    pub rank: i32,
    /// 0 open, 1 closed and new, 2 paid (+0x140).
    pub state: u8,
}

impl Default for MonthReport {
    fn default() -> Self {
        MonthReport { recipes: vec![0; RECIPES], counts: [0; 7], points: 0, total: 0, rank: 0, state: 0 }
    }
}

pub const RECIPES: usize = 265;
/// Month reports kept (`0x141093570`, 0x144 bytes each; 19,440 bytes in a save): five years.
pub const MONTHS: usize = 60;
pub const MEMBERS: usize = 11;

/// What the calendar keeps.
#[derive(Clone, Debug)]
pub struct World {
    /// The date (`0x1410a42cc`).
    pub date: u32,
    /// The main random generator, drawn from by each day's rolls.
    pub rng: Rng,
    /// The request board's seed offset (`0x1410989f0`).
    pub board_offset: i32,
    /// Each character's +0x2a4 (friendship, presumably), and its place in the 11-slot list
    /// (`0x141093530`; `None` when not in it).
    pub friendship: [f32; MEMBERS],
    pub slot: [Option<usize>; MEMBERS],
    /// The characters the daily gain goes over (the list `0x140330470` builds), and the one it
    /// skips.
    pub joined: Vec<usize>,
    pub leader: usize,
    pub rank: RankState,
    /// The month reports, one per month since the game began (`MONTHS`).
    pub months: Vec<MonthReport>,
    /// The board's ten slots, as indices into the request list.
    pub board: [Option<usize>; 10],
    /// The last day's rolls (`0x1410a42e0`).
    pub rolls: Vec<i32>,
    /// Each ware's stock, in the wares' order.
    pub stock: Vec<Stock>,
}

/// What the days need to look up.
pub struct Tables<'a> {
    pub requests: &'a [Request],
    pub wares: &'a [Ware],
    pub ranks: &'a [crate::rank::Rank],
    /// The price of the item each recipe makes.
    pub recipe_cost: &'a dyn Fn(usize) -> i32,
    pub seen: &'a dyn Fn(&str) -> bool,
    pub cleared: &'a dyn Fn(&str) -> bool,
    pub known: &'a dyn Fn(&Request) -> bool,
    pub level: &'a dyn Fn(&Request) -> i32,
}

impl World {
    /// A new game (`0x14035be60` sets the date; the board reset at `0x1403644f9` draws the
    /// offset): `rng` is the main generator as it stands then.
    pub fn new_game(mut rng: Rng, leader: usize, wares: &[Ware]) -> World {
        let board_offset = rng.next_31();
        World {
            date: calendar::Date { year: 1, month: 4, day: 1 }.number(),
            rng,
            board_offset,
            friendship: [0.0; MEMBERS],
            slot: [None; MEMBERS],
            joined: vec![leader],
            leader,
            rank: RankState::default(),
            months: vec![MonthReport::default(); MONTHS],
            board: [None; 10],
            rolls: Vec::new(),
            stock: wares.iter().map(Stock::new).collect(),
        }
    }

    /// The calendar as a save left it (`barrel_formats::save::summary`): the date, the board's
    /// seed offset, each character's friendship (its `Party` record), the squad (`m_squad`) as
    /// the 11-slot list, its characters as those the daily gain goes over and its first as the
    /// leader, the rank, the month reports and the shops' stock (each ware found by its shop,
    /// in the order the shops first appear in the ware table, and its place among that shop's
    /// wares), and the board as `quest::board_from_save` found it. `rng` is the main
    /// generator, which the save does not hold. Not read yet: the last day's rolls.
    pub fn from_save(s: &barrel_formats::save::Summary, rng: Rng, wares: &[Ware], board: [Option<usize>; 10]) -> World {
        let mut friendship = [0.0; MEMBERS];
        for m in &s.party {
            if let Some(f) = friendship.get_mut(m.character as usize) {
                *f = m.friendship;
            }
        }
        let mut slot = [None; MEMBERS];
        for (k, &c) in s.squad_slots.iter().enumerate() {
            if let Some(sl) = usize::try_from(c).ok().and_then(|c| slot.get_mut(c)) {
                *sl = Some(k);
            }
        }
        let joined: Vec<usize> = s.squad.iter().filter_map(|&c| usize::try_from(c).ok()).filter(|&c| c < MEMBERS).collect();
        let leader = joined.first().copied().unwrap_or(0);
        let (rank, points) = s.rank.unwrap_or((0, 0));
        let mut by_period = [0; 9];
        for (b, &v) in by_period.iter_mut().zip(&s.points_by_period) {
            *b = v;
        }
        let i32_at = |d: &[u8], o: usize| i32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let mut months: Vec<MonthReport> = s
            .month_reports
            .iter()
            .map(|d| MonthReport {
                recipes: d[..RECIPES].to_vec(),
                counts: std::array::from_fn(|k| i32_at(d, 0x10c + 4 * k)),
                total: i32_at(d, 0x134),
                points: i32_at(d, 0x138),
                rank: i32_at(d, 0x13c),
                state: d[0x140],
            })
            .collect();
        months.resize(MONTHS, MonthReport::default());
        // Each ware's (shop, place) in the save's 4 × 50 stock.
        let mut shops: Vec<&str> = Vec::new();
        let mut placed = vec![0usize; 0];
        let stock = wares
            .iter()
            .map(|w| {
                let k = shops.iter().position(|&n| n == w.shop).unwrap_or_else(|| {
                    shops.push(&w.shop);
                    placed.push(0);
                    shops.len() - 1
                });
                let j = placed[k];
                placed[k] += 1;
                s.shops.get(k).and_then(|shop| shop.get(j)).map_or(Stock::new(w), |&[count, days, state]| Stock { count, days, state })
            })
            .collect();
        World {
            date: s.day.unwrap_or_else(calendar::first_day),
            rng,
            board_offset: s.board_seed.unwrap_or(0),
            friendship,
            slot,
            joined,
            leader,
            rank: RankState { points, rank, by_period },
            months,
            board,
            rolls: Vec::new(),
            stock,
        }
    }

    /// The month report now being filled.
    pub fn month(&mut self) -> &mut MonthReport {
        let k = calendar::month_index(self.date) as usize;
        &mut self.months[k.min(MONTHS - 1)]
    }

    /// Lets `days` days pass (`0x14035bf10`), working each new day. The reason only labels
    /// the message the game queues.
    pub fn advance(&mut self, days: u32, _reason: Reason, t: &Tables) -> Vec<usize> {
        for _ in 0..days {
            self.date += 1;
            self.new_day(t);
        }
        self.restock(days as i32, t)
    }

    /// The shops after `days` days (`0x14035c560`): every ware whose event window is open
    /// counts the days; returns the wares refilled.
    fn restock(&mut self, days: i32, t: &Tables) -> Vec<usize> {
        let mut refilled = Vec::new();
        for (i, (s, w)) in self.stock.iter_mut().zip(t.wares).enumerate() {
            if shop::on_sale(w, t.seen) && shop::pass_days(s, w, days) {
                refilled.push(i);
            }
        }
        refilled
    }

    /// A new day (`0x14035bb50`).
    fn new_day(&mut self, t: &Tables) {
        self.rolls = calendar::daily_rolls(&mut self.rng);
        for &c in &self.joined {
            if c != self.leader && c < MEMBERS {
                self.friendship[c] = calendar::add_gain(self.friendship[c], calendar::daily_gain(self.slot[c]));
            }
        }
        if calendar::month_start(self.date) {
            let k = calendar::ended_month(self.date) as usize;
            let rank = self.rank.rank;
            let m = &mut self.months[k.min(MONTHS - 1)];
            m.total = calendar::month_total(&m.recipes, t.recipe_cost);
            m.rank = rank;
            m.state = 1;
        }
        if calendar::board_day(self.date) {
            self.board = quest::draw_board(t.requests, self.date as i32, self.board_offset, t.seen, t.cleared, t.known, t.level);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_sets_the_calendar() {
        use barrel_formats::save::{Member, Summary};
        let member = |c, f| Member { character: c, level: 1, exp: 0, hp: 1, mp: 1, max_hp: 1, max_mp: 1, attack: 1, defense: 1, speed: 1, equipped: [1; 5], skills: [-1; 10], friendship: f };
        let mut month = vec![0u8; 0x144];
        month[0x13c] = 3;
        month[0x140] = 2;
        let s = Summary {
            day: Some(229),
            board_seed: Some(77),
            rank: Some((3, 1565)),
            points_by_period: vec![1539, 26],
            party: vec![member(0, 0.0), member(2, 14.5)],
            squad: vec![0, 2],
            squad_slots: vec![0, 2, -1],
            month_reports: vec![month],
            ..Summary::default()
        };
        let w = World::from_save(&s, Rng::from_state([1, 2, 3, 4]), &[], [None; 10]);
        assert_eq!((w.date, w.board_offset, w.leader, w.joined.clone()), (229, 77, 0, vec![0, 2]));
        assert_eq!((w.friendship[2], w.slot[2], w.slot[0], w.slot[1]), (14.5, Some(1), Some(0), None));
        assert_eq!((w.rank.rank, w.rank.points, w.rank.by_period[..2].to_vec()), (3, 1565, vec![1539, 26]));
        assert_eq!((w.months.len(), w.months[0].rank, w.months[0].state), (MONTHS, 3, 2));
    }

    #[test]
    fn a_month_goes_by() {
        let ranks = [crate::rank::Rank { points: 0, coal: 300, percent: 10, rates: [0; 7], number: 1 }];
        let t = Tables {
            requests: &[],
            wares: &[],
            ranks: &ranks,
            recipe_cost: &|_| 100,
            seen: &|_| true,
            cleared: &|_| true,
            known: &|_| true,
            level: &|_| 1,
        };
        let mut w = World::new_game(Rng::from_state([1, 2, 3, 4]), 0, &[]);
        w.joined = vec![0, 1, 2];
        w.slot[1] = Some(1);
        w.slot[2] = Some(7);
        assert_eq!(w.date, 90);
        w.month().recipes[0] = 3;
        w.advance(30, Reason::Synthesis, &t);
        // May 1st: April's report is closed; the leader gains nothing.
        assert_eq!(calendar::Date::of(w.date), calendar::Date { year: 1, month: 5, day: 1 });
        assert_eq!((w.months[0].total, w.months[0].state), (300, 1));
        assert_eq!(w.friendship[0], 0.0);
        assert!((w.friendship[1] - 30.0 * 0.11).abs() < 1e-3);
        assert!((w.friendship[2] - 30.0 * 0.03).abs() < 1e-3);
        assert_eq!(w.rolls.len(), calendar::DAILY_ROLLS);
    }
}
