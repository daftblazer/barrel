//! The townsfolk a field places, from the four tables of `saves\fieldmap\npc\`: `npcbasedata.xml`
//! (one row a person, naming a row in each of the others by tag), `npcpeople.xml` (who: the
//! character and the motion it plays), `npcmarker.xml` (where) and `npccondition.xml` (when).
//! Docs: docs/tables.md, "Ryza DX: field NPCs".

use barrel_formats::table::{self, Element};

use crate::environment::{Time, Weather};

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
    /// The times of day and the weathers it is there for, as bit masks.
    pub timezone: u32,
    pub weather: u32,
    /// It depends on the story's progress (`ev_range_*`, `ev_ex_*` of its condition).
    pub story: bool,
    /// Its marker is all zero: it walks a path from `move_data` instead of standing.
    pub walks: bool,
}

impl Npc {
    /// The character's `chara.xml` tag: its `chara_tag` without the number after the underscore
    /// (`NPC30B_00` is `NPC30B`).
    pub fn character(&self) -> &str {
        self.chara.split('_').next().unwrap_or(&self.chara)
    }

    // STAND-IN: The bits of `timezone` and `weather` are taken in the order of the game's times and weathers (bit 0 morning, noon, evening, night, midnight; bit 0 sunny, cloudy, foggy); the real thing is the executable's own numbering. The data fits: the day rows carry 7, their night twins 24.
    /// Whether it is there at `time` in `weather`.
    pub fn present(&self, time: Time, weather: Weather) -> bool {
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
/// of a kind (in order of first appearance) is the Nth row of that field in that table.
pub fn read(basedata: &[u8], people: &[u8], marker: &[u8], condition: &[u8], field: &str) -> Result<Vec<Npc>, String> {
    let parse = |b: &[u8], name: &str| table::parse(b).map_err(|e| format!("{name}: {e}"));
    let base = parse(basedata, "npcbasedata")?;
    let Some(rows) = base.children.iter().find(|e| e.name == field) else { return Ok(Vec::new()) };
    let rows_of = |root: &Element, suffix: &str| -> Vec<Element> { root.children.iter().filter(|e| e.name == format!("{field}_{suffix}")).cloned().collect() };
    let (people, marker, condition) = (rows_of(&parse(people, "npcpeople")?, "people"), rows_of(&parse(marker, "npcmarker")?, "marker"), rows_of(&parse(condition, "npccondition")?, "Condition"));
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
            timezone: int(c, "timezone") as u32,
            weather: int(c, "weather") as u32,
            story: c.attributes.iter().any(|(k, _)| k.starts_with("ev_range") || k.starts_with("ev_ex")),
            walks: [x, y, z] == [0.0; 3],
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
        <S06_T_RASEN_BaseData people="C" marker="M3" condition="C1"/>
    </S06_T_RASEN><S07_X><S07_X_BaseData people="Z" marker="Z" condition="Z"/></S07_X></root>"#;
    const PEOPLE: &[u8] = br#"<root><S06_T_RASEN_people chara_tag="MIO" init_mot_no="0"/>
        <S06_T_RASEN_people chara_tag="NPC30A_00" init_mot_no="2101" mot_idx="-1" no_gravity="1"/>
        <S06_T_RASEN_people chara_tag="NPC24B_00" init_mot_no="12531" mot_idx="0"/><S07_X_people chara_tag="OTHER"/></root>"#;
    const MARKER: &[u8] = br#"<root><S06_T_RASEN_marker marker="1,2,3,90"/><S06_T_RASEN_marker marker="294.52,57.77,257.06,-58.46"/><S06_T_RASEN_marker marker="0,0,0,0"/></root>"#;
    const CONDITION: &[u8] = br#"<root><S06_T_RASEN_Condition ev_ex_00="X" timezone="3" weather="63"/><S06_T_RASEN_Condition timezone="24" weather="1"/></root>"#;

    #[test]
    fn rows_are_the_nth_distinct_tag() {
        let npcs = read(BASE, PEOPLE, MARKER, CONDITION, "S06_T_RASEN").unwrap();
        assert_eq!(npcs.len(), 3);
        assert_eq!((npcs[0].chara.as_str(), npcs[0].story, npcs[0].position), ("MIO", true, [100.0, 200.0, 300.0]));
        let b = &npcs[1];
        assert_eq!((b.chara.as_str(), b.motion, b.set, b.no_gravity, b.timezone, b.weather), ("NPC30A_00", 2101, -1, true, 24, 1));
        assert!((b.position[0] - 29452.0).abs() < 1e-2 && (b.yaw + 58.46).abs() < 1e-4);
        // the third person's marker is all zero and its condition is the first's
        assert!(npcs[2].walks && npcs[2].story && !npcs[0].walks);
    }

    #[test]
    fn present_by_bits() {
        let npcs = read(BASE, PEOPLE, MARKER, CONDITION, "S06_T_RASEN").unwrap();
        assert!(npcs[0].present(Time::Noon, Weather::Foggy) && !npcs[0].present(Time::Night, Weather::Sunny));
        assert!(npcs[1].present(Time::Night, Weather::Sunny) && !npcs[1].present(Time::Night, Weather::Cloudy));
    }

    #[test]
    fn other_fields_and_short_tables() {
        assert!(read(BASE, PEOPLE, MARKER, CONDITION, "S99").unwrap().is_empty());
        assert!(read(BASE, PEOPLE, b"<root/>", CONDITION, "S06_T_RASEN").is_err());
    }
}
