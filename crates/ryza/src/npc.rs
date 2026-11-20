//! The townsfolk a field places, from the four tables of `saves\fieldmap\npc\`: `npcbasedata.xml`
//! (one row a person, naming a row in each of the others by tag), `npcpeople.xml` (who: the
//! character and the motion it plays), `npcmarker.xml` (where) and `npccondition.xml` (when).
//! Docs: docs/tables.md, "Ryza DX: field NPCs".

use barrel_formats::table::{self, Element};

use crate::environment::{Time, Weather};
use crate::story::{Span, Story};

/// A person a field places.
#[derive(Clone, Debug, PartialEq)]
pub struct Npc {
    /// The character's tag in `chara.xml` (`NPC30B_00`).
    pub chara: String,
    /// The motion it plays looped, and the motion-set choice beside it (`mot_idx`: 0 the
    /// character's first set, -1 none named).
    pub motion: u32,
    pub set: i32,
    /// Where it stands, in world units, and its facing in degrees.
    pub position: [f32; 3],
    pub yaw: f32,
    /// It stays at its marker's height rather than dropping to the ground.
    pub no_gravity: bool,
    /// She can talk to it (`talk` of its people row): the minimap marks these.
    pub talk: bool,
    /// The times of day and the weathers it is there for, as bit masks.
    pub timezone: u32,
    pub weather: u32,
    /// The story spans it is there within (`ev_range_*` of its condition: any one of them,
    /// when there are any) and the ones it is away for (`ev_ex_*`).
    pub ranges: Vec<Span>,
    pub away: Vec<Span>,
    /// The path it walks instead of standing (`move_data` of its base row: a file of
    /// `saves\fieldmap\npc\move_data\`, without the `.xml`).
    pub path: Option<String>,
}

/// A walker's path (`move_data\<name>.xml`): it starts at `start`, walks to each of `points`
/// in turn at `speed`, and goes round them again from the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    /// World units a second (`move_speed`, metres a second: 1 a walk, 3 a run, as the
    /// capture's walkers cover the ground).
    pub speed: f32,
    pub start: [f32; 3],
    /// Each point and the pause there in seconds.
    pub points: Vec<([f32; 3], f32)>,
}

/// A walker's path from its file's bytes.
pub fn path(bytes: &[u8]) -> Result<Path, String> {
    let root = table::parse(bytes).map_err(|e| e.to_string())?;
    let child = |name: &str| root.children.iter().find(|e| e.name == name);
    let num = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<f32>().ok());
    let base = child("base_setting");
    let point = |e: &Element| -> Result<([f32; 3], f32), String> {
        let v: Vec<f32> = e.attr("to_point").ok_or("a move has no to_point")?.split(',').map(|v| v.trim().parse::<f32>().map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
        let [x, y, z] = v[..] else { return Err("a to_point is not x,y,z".into()) };
        // STAND-IN: A pause is the shortest the row allows (`interval_min`); the real thing is the game's draw between it and `interval_max`.
        let pause = num(e, "interval_min").or(base.and_then(|b| num(b, "interval_min"))).unwrap_or(0.0);
        Ok(([x * 100.0, y * 100.0, z * 100.0], pause))
    };
    let points_of = |name: &str| child(name).map(|e| e.children.iter().filter(|m| m.name == "move_data").map(point).collect::<Result<Vec<_>, _>>()).transpose().map(Option::unwrap_or_default);
    let start = points_of("start")?.first().ok_or("a path has no start")?.0;
    Ok(Path { speed: base.and_then(|b| num(b, "move_speed")).unwrap_or(1.0) * 100.0, start, points: points_of("loop")? })
}

impl Path {
    /// Where the walker is `time` seconds after the field appears, the way it faces (degrees
    /// about y, as a marker's) and whether it is walking (not pausing at a point).
    pub fn at(&self, time: f32) -> ([f32; 3], f32, bool) {
        let v = |p: [f32; 3]| glam::Vec3::from(p);
        let facing = |from: glam::Vec3, to: glam::Vec3| (to.x - from.x).atan2(to.z - from.z).to_degrees();
        let Some(&(first, _)) = self.points.first() else { return (self.start, 0.0, false) };
        let (mut left, mut from) = (time.max(0.0), v(self.start));
        // The way to the first point, once; then the round, as often as the time allows.
        let round: f32 = (0..self.points.len()).map(|i| v(self.points[i].0).distance(v(self.points[(i + 1) % self.points.len()].0)) / self.speed + self.points[i].1).sum();
        let lead = from.distance(v(first)) / self.speed;
        let mut next = 0;
        if left >= lead {
            left = if round > 0.0 { (left - lead) % round } else { 0.0 };
            // At the first point: its pause, then on.
            loop {
                let (here, pause) = self.points[next];
                let to = v(self.points[(next + 1) % self.points.len()].0);
                if left < pause || round <= 0.0 {
                    return (here, facing(from, v(here)), false);
                }
                left -= pause;
                let leg = v(here).distance(to) / self.speed;
                if left < leg {
                    return (v(here).lerp(to, left / leg.max(1e-6)).to_array(), facing(v(here), to), true);
                }
                left -= leg;
                from = v(here);
                next = (next + 1) % self.points.len();
            }
        }
        (from.lerp(v(first), left / lead.max(1e-6)).to_array(), facing(from, v(first)), true)
    }
}

impl Npc {
    /// The character's `chara.xml` tag: its `chara_tag` without the number after the underscore
    /// (`NPC30B_00` is `NPC30B`).
    pub fn character(&self) -> &str {
        self.chara.split('_').next().unwrap_or(&self.chara)
    }

    // STAND-IN: The bits of `timezone` and `weather` are taken in the order of the game's times and weathers (bit 0 morning, noon, evening, night, midnight; bit 0 sunny, cloudy, foggy); the real thing is the executable's own numbering. The data fits: the day rows carry 7, their night twins 24.
    /// Whether it is there at `time` in `weather` at this point of the `story` (the story
    /// part is the game's `0x14040f9e0`: within one of its spans when it has any, and within
    /// none of the spans it is away for, which count only once their start has played).
    pub fn present(&self, time: Time, weather: Weather, story: &Story) -> bool {
        if !self.ranges.is_empty() && !self.ranges.iter().any(|s| story.within(s, false)) || self.away.iter().any(|s| story.within(s, true)) {
            return false;
        }
        let t = match time {
            Time::Morning => 0,
            Time::Noon => 1,
            Time::Evening => 2,
            Time::Night => 3,
        };
        let w = match weather {
            Weather::Sunny => 0,
            Weather::Cloudy => 1,
            Weather::Foggy => 2,
        };
        self.timezone >> t & 1 == 1 && self.weather >> w & 1 == 1
    }
}

/// The people of `field` (`S06_T_RASEN`: its name in upper case) from the four tables' bytes.
/// The tags of the base data are not attributes of the other tables: a field's Nth distinct tag
/// of a kind (in order of first appearance) is the Nth row of that field in that table. A
/// condition's story spans are rows of `evrange` by the number their tag ends in
/// (`S06_EVR_003` is the field's third).
pub fn read(basedata: &[u8], people: &[u8], marker: &[u8], condition: &[u8], evrange: &[u8], field: &str) -> Result<Vec<Npc>, String> {
    let parse = |b: &[u8], name: &str| table::parse(b).map_err(|e| format!("{name}: {e}"));
    let base = parse(basedata, "npcbasedata")?;
    let Some(rows) = base.children.iter().find(|e| e.name == field) else { return Ok(Vec::new()) };
    let rows_of = |root: &Element, suffix: &str| -> Vec<Element> { root.children.iter().filter(|e| e.name == format!("{field}_{suffix}")).cloned().collect() };
    let (people, marker, condition) = (rows_of(&parse(people, "npcpeople")?, "people"), rows_of(&parse(marker, "npcmarker")?, "marker"), rows_of(&parse(condition, "npccondition")?, "Condition"));
    let spans: Vec<Span> = rows_of(&parse(evrange, "npcevrange")?, "EvRange").iter().map(|e| Span::new(e.attr("start"), e.attr("end"))).collect();
    let spans_of = |c: &Element, kind: &str| -> Result<Vec<Span>, String> {
        c.attributes.iter().filter(|(k, _)| k.starts_with(kind)).map(|(_, tag)| {
            let n = tag.rsplit('_').next().and_then(|n| n.parse::<usize>().ok()).filter(|n| *n > 0).ok_or(format!("{field}: {tag} has no number"))?;
            spans.get(n - 1).cloned().ok_or(format!("{field}: no story span {tag}"))
        }).collect()
    };
    let (mut tags, mut out): ([Vec<&str>; 3], Vec<Npc>) = (Default::default(), Vec::new());
    let mut seen: Vec<[usize; 3]> = Vec::new();
    for row in &rows.children {
        let mut at = [0; 3];
        for (k, key) in ["people", "marker", "condition"].into_iter().enumerate() {
            let tag = row.attr(key).ok_or(format!("{field}: a row has no {key}"))?;
            at[k] = tags[k].iter().position(|t| *t == tag).unwrap_or_else(|| {
                tags[k].push(tag);
                tags[k].len() - 1
            });
        }
        if seen.contains(&at) {
            continue;
        }
        seen.push(at);
        let (Some(p), Some(m), Some(c)) = (people.get(at[0]), marker.get(at[1]), condition.get(at[2])) else {
            return Err(format!("{field}: the tables have fewer rows than the base data has tags ({at:?})"));
        };
        let num = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<f32>().ok());
        let int = |e: &Element, k: &str| num(e, k).unwrap_or(0.0) as i32;
        let place: Vec<f32> = m.attr("marker").ok_or("a marker has no position")?.split(',').map(|v| v.trim().parse::<f32>().map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
        let [x, y, z, yaw] = place[..] else { return Err(format!("{field}: a marker is not x,y,z,yaw")) };
        out.push(Npc {
            chara: p.attr("chara_tag").ok_or("a person has no chara_tag")?.to_string(),
            motion: int(p, "init_mot_no").max(0) as u32,
            set: p.attr("mot_idx").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            position: [x * 100.0, y * 100.0, z * 100.0],
            yaw,
            no_gravity: int(p, "no_gravity") == 1,
            talk: int(p, "talk") == 1,
            timezone: int(c, "timezone") as u32,
            weather: int(c, "weather") as u32,
            ranges: spans_of(c, "ev_range")?,
            away: spans_of(c, "ev_ex")?,
            path: row.attr("move_data").map(str::to_ascii_lowercase),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &[u8] = br#"<root><S06_T_RASEN>
        <S06_T_RASEN_BaseData people="A" marker="M1" condition="C1"/>
        <S06_T_RASEN_BaseData people="B" marker="M2" condition="C2"/>
        <S06_T_RASEN_BaseData people="B" marker="M2" condition="C2"/>
        <S06_T_RASEN_BaseData people="C" marker="M3" move_data="s06_move_data_002_N" condition="C1"/>
    </S06_T_RASEN><S07_X><S07_X_BaseData people="Z" marker="Z" condition="Z"/></S07_X></root>"#;
    const PEOPLE: &[u8] = br#"<root><S06_T_RASEN_people chara_tag="MIO" init_mot_no="0"/>
        <S06_T_RASEN_people chara_tag="NPC30A_00" init_mot_no="2101" mot_idx="-1" no_gravity="1"/>
        <S06_T_RASEN_people chara_tag="NPC24B_00" init_mot_no="12531" mot_idx="0"/><S07_X_people chara_tag="OTHER"/></root>"#;
    const MARKER: &[u8] = br#"<root><S06_T_RASEN_marker marker="1,2,3,90"/><S06_T_RASEN_marker marker="294.52,57.77,257.06,-58.46"/><S06_T_RASEN_marker marker="0,0,0,0"/></root>"#;
    const CONDITION: &[u8] = br#"<root><S06_T_RASEN_Condition ev_ex_00="S06_EVR_002" timezone="3" weather="63"/><S06_T_RASEN_Condition ev_range_00="S06_EVR_001" ev_range_01="S06_EVR_002" timezone="24" weather="1"/></root>"#;
    const RANGES: &[u8] = br#"<root><S02_X_EvRange start="A"/><S06_T_RASEN_EvRange end="MM01_111"/><S06_T_RASEN_EvRange start="MM00_010" end="MM00_080"/></root>"#;

    #[test]
    fn rows_are_the_nth_distinct_tag() {
        let npcs = read(BASE, PEOPLE, MARKER, CONDITION, RANGES, "S06_T_RASEN").unwrap();
        assert_eq!(npcs.len(), 3);
        assert_eq!((npcs[0].chara.as_str(), npcs[0].away.len(), npcs[0].position), ("MIO", 1, [100.0, 200.0, 300.0]));
        let b = &npcs[1];
        assert_eq!((b.chara.as_str(), b.motion, b.set, b.no_gravity, b.timezone, b.weather), ("NPC30A_00", 2101, -1, true, 24, 1));
        assert!((b.position[0] - 29452.0).abs() < 1e-2 && (b.yaw + 58.46).abs() < 1e-4);
        // the third person walks a path and its condition is the first's
        assert_eq!((npcs[2].path.as_deref(), npcs[2].away == npcs[0].away, &npcs[0].path), (Some("s06_move_data_002_n"), true, &None));
    }

    #[test]
    fn a_path_goes_to_its_points_and_round() {
        let p = path(b"<Root>\n<!-- note -->\n<base_setting move_speed = \"2\" interval_min = \"0.0\" interval_max = \"0.0\" />\n<start><move_data to_point = \"0,\t1, 0\" /></start>\n<loop><move_data to_point = \"4, 1, 0\" /><move_data to_point = \"4, 1, 4\" interval_min = \"1.0\" interval_max = \"3.0\"/></loop></Root>").unwrap();
        assert_eq!((p.speed, p.start, p.points.len(), p.points[1]), (200.0, [0.0, 100.0, 0.0], 2, ([400.0, 100.0, 400.0], 1.0)));
        // on the way to the first point, facing +x
        assert_eq!(p.at(1.0), ([200.0, 100.0, 0.0], 90.0, true));
        // 2 s there, 1 s on towards the second, facing +z
        assert_eq!(p.at(3.0), ([400.0, 100.0, 200.0], 0.0, true));
        // 4 s: at the second, pausing a second; then back to the first, and round again
        assert_eq!(p.at(4.5).0, [400.0, 100.0, 400.0]);
        assert!(!p.at(4.5).2);
        assert_eq!(p.at(6.0), ([400.0, 100.0, 200.0], 180.0, true));
        assert_eq!(p.at(8.0).0, p.at(3.0).0);
    }

    #[test]
    fn present_by_bits() {
        let npcs = read(BASE, PEOPLE, MARKER, CONDITION, RANGES, "S06_T_RASEN").unwrap();
        let none = Story::default();
        assert!(npcs[0].present(Time::Noon, Weather::Foggy, &none) && !npcs[0].present(Time::Night, Weather::Sunny, &none));
        assert!(npcs[1].present(Time::Night, Weather::Sunny, &none) && !npcs[1].present(Time::Night, Weather::Cloudy, &none));
    }

    #[test]
    fn present_by_the_story() {
        let npcs = read(BASE, PEOPLE, MARKER, CONDITION, RANGES, "S06_T_RASEN").unwrap();
        let at = |n: usize, done: &str| npcs[n].present(if n == 0 { Time::Noon } else { Time::Night }, Weather::Sunny, &Story::after(done));
        // away for a span only once its start has played, until its end has
        assert!(at(0, "") && !at(0, "MM00_010") && at(0, "MM00_010,MM00_080"));
        // there within any one of its spans: the first is open until MM01_111
        assert!(at(1, "") && at(1, "MM01_111,MM00_010") && !at(1, "MM01_111") && !at(1, "MM01_111,MM00_010,MM00_080"));
    }

    #[test]
    fn other_fields_and_short_tables() {
        assert!(read(BASE, PEOPLE, MARKER, CONDITION, RANGES, "S99").unwrap().is_empty());
        assert!(read(BASE, PEOPLE, b"<root/>", CONDITION, RANGES, "S06_T_RASEN").is_err());
    }
}
