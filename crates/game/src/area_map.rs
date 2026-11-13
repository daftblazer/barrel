//! The area map: a region of the world map (its own terrain zoomed in, `docs/tables.md`, "The
//! places") whose points are the region's entry and its sectors. The cursor stands on one place
//! and goes along the routes it shares with its neighbours (`world::places`' links).
//!
//! The link slots are the directions on the map: right (+x), left (-x), down (+z), up (-z),
//! then two not read (one route in all the table uses slot 4). Read from the Southern Ruins'
//! marks (`world::point_position`): Sector One's slot 0 leads to Sector Two 700 units east,
//! slot 1 to the Ruins Entrance west (and north), slot 2 to Sector Three 700 south; Sector
//! Three's slot 3 back north, Sector Two's slot 1 back west.

use barrel_formats::world::Place;

/// The places of region `region` (its tag: `A2` the Southern Ruins): its entry (the place with
/// that tag) and its sectors (`A2p0` ...), by their indices in `places`.
pub fn region(places: &[Place], region: &str) -> Vec<usize> {
    let sector = format!("{region}p");
    (0..places.len()).filter(|&k| places[k].tag == region || places[k].tag.starts_with(&sector)).collect()
}

/// The places sharing a route with place `at`: (route, place).
pub fn neighbours(places: &[Place], at: usize) -> Vec<(i32, usize)> {
    let Some(here) = places.get(at) else { return Vec::new() };
    let mut out = Vec::new();
    for &route in here.links.iter().filter(|&&r| r >= 0) {
        if let Some(k) = (0..places.len()).find(|&k| k != at && places[k].links.contains(&route)) {
            out.push((route, k));
        }
    }
    out
}

/// Whether place `place` is closed (`0x14035e6f0`, `0x14035e740`), by its opening condition
/// (`extra`, the table's +0x48) and what the player has done to it: `[0, _]` places start shut
/// and open when their field gimmick is used (`opened`: `0x140176619` clears the save's flag);
/// `[1, N]` places stand behind an obstacle of durability N ("Durability: %d"), which hits wear
/// down (`damage` in all: `0x140177ad2`) and which opens at 0. Other places are always open.
/// The game keeps this per place in the save's `pointerStatus` (`save::place_states`).
pub fn closed(place: &Place, opened: bool, damage: i32) -> bool {
    match place.extra[0] {
        0 => !opened,
        1 => place.extra[1] - damage > 0,
        _ => false,
    }
}

/// A direction on the map, as the link slots order them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Right = 0,
    Left = 1,
    Down = 2,
    Up = 3,
}

/// Where a press goes from `at`: the place across the route in that way's slot.
pub fn step(places: &[Place], at: usize, way: Way) -> Option<usize> {
    let route = *places.get(at)?.links.get(way as usize)?;
    (route >= 0).then(|| (0..places.len()).find(|&k| k != at && places[k].links.contains(&route))).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(tag: &str, links: [i32; 6]) -> Place {
        Place { name: tag.into(), tag: tag.into(), kind: 1, links, description: String::new(), extra: [-1, 0] }
    }

    #[test]
    fn obstacles_and_gimmicks_close_places() {
        let mut sector_three = place("A2p2", [29, -1, -1, 26, -1, -1]);
        sector_three.extra = [1, 30];
        assert!(closed(&sector_three, false, 0) && closed(&sector_three, false, 29) && !closed(&sector_three, false, 30));
        let mut promenade = place("B1p1", [33, 34, -1, -1, -1, -1]);
        promenade.extra = [0, 31];
        assert!(closed(&promenade, false, 0) && !closed(&promenade, true, 0));
        assert!(!closed(&place("A2p1", [-1; 6]), false, 0));
    }

    #[test]
    fn presses_follow_the_routes_the_way_they_lie() {
        // The Southern Ruins' first places as the table has them.
        let places = vec![
            place("A1p5", [-1, 23, -1, -1, -1, -1]),
            place("A2", [-1, -1, 24, -1, -1, -1]),
            place("A2p0", [-1, -1, 25, 24, -1, -1]),
            place("A2p1", [27, 25, 26, -1, -1, -1]),
            place("A2p2", [29, -1, -1, 26, -1, -1]),
            place("A2p3", [-1, 27, 28, -1, -1, -1]),
        ];
        assert_eq!(region(&places, "A2"), vec![1, 2, 3, 4, 5]);
        assert_eq!(neighbours(&places, 3), vec![(27, 5), (25, 2), (26, 4)]);
        // Sector One: Sector Two right, the Ruins Entrance left, Sector Three down, nothing up.
        assert_eq!(step(&places, 3, Way::Right), Some(5));
        assert_eq!(step(&places, 3, Way::Left), Some(2));
        assert_eq!(step(&places, 3, Way::Down), Some(4));
        assert_eq!(step(&places, 3, Way::Up), None);
        // The Ruins Entrance: down to Sector One, up to the region's entry.
        assert_eq!(step(&places, 2, Way::Down), Some(3));
        assert_eq!(step(&places, 2, Way::Up), Some(1));
    }
}
