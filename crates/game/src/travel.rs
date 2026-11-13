//! Travelling on the world map: its routes joining the points (`barrel_formats::world`, the
//! ends as the routes' dots lie, `route_ends`), each taking its days (fewer after some events,
//! `Area::trip_days`), and the fields a point opens (`point_maps`).

use barrel_formats::world::Area;

/// A route between two marks (`T0`, `A0`, `A0p0` ...): its record (id, days) and its ends.
#[derive(Clone, Debug)]
pub struct Route {
    pub area: Area,
    pub ends: (String, String),
}

/// The world map as travelled: its routes.
#[derive(Clone, Debug, Default)]
pub struct WorldMap {
    pub routes: Vec<Route>,
}

impl WorldMap {
    /// The routes leaving `mark`, each with the mark at its other end.
    pub fn from<'a>(&'a self, mark: &'a str) -> impl Iterator<Item = (&'a Route, &'a str)> + 'a {
        self.routes.iter().filter_map(move |r| match (&r.ends.0, &r.ends.1) {
            (a, b) if a == mark => Some((r, b.as_str())),
            (a, b) if b == mark => Some((r, a.as_str())),
            _ => None,
        })
    }

    /// The fewest days from `from` to `to` (paths inside a region take none), and the routes
    /// taken, with `seen` saying which events have been seen (the shorter trips after them).
    pub fn trip(&self, from: &str, to: &str, seen: impl Fn(u32) -> bool) -> Option<(i32, Vec<String>)> {
        // Dijkstra over the marks (few: a simple search will do).
        let mut best: std::collections::HashMap<String, (i32, Vec<String>)> = std::collections::HashMap::new();
        best.insert(from.to_string(), (0, Vec::new()));
        let mut open = vec![from.to_string()];
        while let Some(k) = (0..open.len()).min_by_key(|&k| best[&open[k]].0) {
            let mark = open.swap_remove(k);
            if mark == to {
                break;
            }
            let (days, path) = best[&mark].clone();
            for (route, next) in self.from(&mark) {
                let d = days + route.area.trip_days(&seen).max(0);
                if best.get(next).is_none_or(|b| d < b.0) {
                    let mut p = path.clone();
                    p.push(route.area.id.clone());
                    best.insert(next.to_string(), (d, p));
                    if !open.iter().any(|o| o == next) {
                        open.push(next.to_string());
                    }
                }
            }
        }
        best.remove(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(id: &str, a: &str, b: &str, days: i32) -> Route {
        Route { area: Area { id: id.into(), float: 1.0, days: [(days, -1), (days, -1), (days, -1)] }, ends: (a.into(), b.into()) }
    }

    #[test]
    fn the_shortest_trip_is_taken() {
        let w = WorldMap { routes: vec![route("R0", "T0", "A0", 4), route("A0r0", "A0", "A0p0", 0), route("R9", "T0", "A0p0", 6)] };
        assert_eq!(w.trip("T0", "A0p0", |_| false), Some((4, vec!["R0".into(), "A0r0".into()])));
        assert_eq!(w.trip("A0p0", "T0", |_| false).map(|t| t.0), Some(4));
        assert_eq!(w.trip("T0", "B1", |_| false), None);
    }
}
