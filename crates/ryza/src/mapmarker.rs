//! Where the story goes next, from `saves\mapmarker\mapmarker.xml`: one `<MapMarker>` a stretch
//! of the story (`startEv` played, `endEv0` not), naming the field it points to (`fieldMapTag`)
//! and, when that is the field the player stands in, the place (`targetX`, `targetZ`, metres).
//! The minimap marks it. Docs: docs/tables.md, "Ryza DX: map markers".

use barrel_formats::table;

use crate::story::Story;

/// A row of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct MapMarker {
    /// The field the story goes to (`S08_T_BODEN`).
    pub field: String,
    /// The events it holds between: the first has played, none of the others.
    pub start: String,
    pub ends: Vec<String>,
    /// The place in that field, in world units (the row's metres times 100), if it names one.
    pub target: Option<[f32; 2]>,
}

/// The rows of the table, from its bytes.
pub fn read(bytes: &[u8]) -> Result<Vec<MapMarker>, String> {
    let root = table::parse(bytes).map_err(|e| format!("mapmarker: {e}"))?;
    let metres = |row: &table::Element, k: &str| row.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).map(|m| m * 100.0);
    Ok(root
        .children
        .iter()
        .filter(|e| e.name == "MapMarker")
        .filter_map(|row| {
            Some(MapMarker {
                field: row.attr("fieldMapTag")?.to_ascii_uppercase(),
                start: row.attr("startEv")?.to_string(),
                ends: (0..8).filter_map(|i| row.attr(&format!("endEv{i}"))).map(str::to_string).collect(),
                target: metres(row, "targetX").zip(metres(row, "targetZ")).map(|(x, z)| [x, z]),
            })
        })
        .collect())
}

/// The row the story is at: its start has played and none of its ends.
pub fn current<'a>(rows: &'a [MapMarker], story: &Story) -> Option<&'a MapMarker> {
    rows.iter().find(|r| story.done(&r.start) && !r.ends.iter().any(|e| story.done(e)))
}

// STAND-IN: Which exit leads to which field is not in the tables (a `FIELDMAP_CHANGE_EX` row has no target; the game keeps it in the executable); this is the one read off the capture: the destination `S08_T_BODEN`'s marker points at Rasenboden's exit 6 (the five frames' pointing rays of `hud-walk` cross within 15 units of it).
/// (field, destination field, the field's exit `fixID` toward it).
const EXITS_TOWARD: [(&str, &str, u32); 1] = [("S06_T_RASEN", "S08_T_BODEN", 6)];

/// The `fixID` of the exit of `field` that leads toward the field `to`, where known.
pub fn exit_toward(field: &str, to: &str) -> Option<u32> {
    EXITS_TOWARD.iter().find(|(f, t, _)| f.eq_ignore_ascii_case(field) && t.eq_ignore_ascii_case(to)).map(|e| e.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<Root>
      <MapMarker fieldMapTag="S08_T_BODEN" startEv="MM00_010" endEv0="MM00_020"/>
      <MapMarker fieldMapTag="S06_T_RASEN" startEv="MM00_020" endEv0="MM00_030" targetX="451" targetZ="249.22"/>
    </Root>"#;

    #[test]
    fn the_row_the_story_is_at() {
        let rows = read(XML.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(current(&rows, &Story::after("MM00_010")).map(|r| r.field.as_str()), Some("S08_T_BODEN"));
        let later = Story::after("MM00_010,MM00_020");
        let r = current(&rows, &later).unwrap();
        assert_eq!((r.field.as_str(), r.target), ("S06_T_RASEN", Some([45100.0, 24922.0])));
        assert!(current(&rows, &Story::after("MM00_010,MM00_020,MM00_030")).is_none());
    }
}
