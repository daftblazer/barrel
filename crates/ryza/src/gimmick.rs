//! The gimmicks a field places, from `saves\fieldmap\gimmick\gimmick_fieldmap_<field>.xml`: one
//! `<Property>` a gathering spot, notice board, crate, warp and so on. The ones with a `model`
//! are drawn in the field. Docs: docs/tables.md, "Ryza DX: field gimmicks".

use barrel_formats::table;

use crate::environment::Time;

/// A gimmick with a model.
#[derive(Clone, Debug, PartialEq)]
pub struct Gimmick {
    /// Its row's `fixID`.
    pub id: u32,
    /// The model's character tag (`model` without its `CHARA_`: `FM_BOARD_01`).
    pub tag: String,
    /// Where it stands, in world units (the row's metres times 100), and its turn about the
    /// vertical in degrees.
    pub position: [f32; 3],
    pub yaw: f32,
    /// A turn of the model about x, y and z in degrees (`objRot`), and its scale (`objScale`).
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    /// Whether it depends on the story's progress (`startEvent` or `endEvent`).
    pub story: bool,
    /// The times of day it is there between (`startTZ`, `endTZ`), wrapping, both included.
    pub span: Option<(Time, Time)>,
}

/// The time named by a `TIME_ZONE_*` value. Only `MORNING`, `EVENING` and `NIGHT` occur in the
/// tables; the order is the one of [`Time`].
fn zone(name: &str) -> Option<Time> {
    Some(match name.strip_prefix("TIME_ZONE_")? {
        "MORNING" => Time::Morning,
        "NOON" => Time::Noon,
        "EVENING" => Time::Evening,
        "NIGHT" => Time::Night,
        _ => return None,
    })
}

impl Gimmick {
    /// Whether it is there at `time` of day (a row without a span is there all day).
    pub fn present(&self, time: Time) -> bool {
        let Some((from, to)) = self.span else { return true };
        let at = |t: Time| t as u8;
        match at(from) <= at(to) {
            true => (at(from)..=at(to)).contains(&at(time)),
            false => at(time) >= at(from) || at(time) <= at(to),
        }
    }
}

fn numbers<const N: usize>(text: Option<&str>, or: [f32; N]) -> Result<[f32; N], String> {
    let Some(text) = text else { return Ok(or) };
    let v = text.split(',').map(|v| v.trim().parse::<f32>().map_err(|e| format!("{text}: {e}"))).collect::<Result<Vec<_>, _>>()?;
    v.try_into().map_err(|_| format!("{text}: not {N} numbers"))
}

/// The gimmicks with a model, from the table's bytes.
pub fn read(bytes: &[u8]) -> Result<Vec<Gimmick>, String> {
    let root = table::parse(bytes).map_err(|e| format!("gimmick: {e}"))?;
    let mut out = Vec::new();
    for row in root.children.iter().filter(|e| e.name == "Property") {
        let Some(model) = row.attr("model") else { continue };
        let zone_of = |k: &str| row.attr(k).map(|n| zone(n).ok_or(format!("gimmick: {n}"))).transpose();
        let [x, y, z] = numbers(row.attr("position"), [0.0; 3])?;
        out.push(Gimmick {
            id: row.attr("fixID").and_then(|v| v.trim().parse().ok()).ok_or("gimmick: no fixID")?,
            tag: model.strip_prefix("CHARA_").unwrap_or(model).to_string(),
            position: [x * 100.0, y * 100.0, z * 100.0],
            yaw: row.attr("rotationY").and_then(|v| v.trim().parse().ok()).unwrap_or(0.0),
            rotation: numbers(row.attr("objRot"), [0.0; 3])?,
            scale: numbers(row.attr("objScale"), [1.0; 3])?,
            story: row.attr("startEvent").is_some() || row.attr("endEvent").is_some(),
            span: match (zone_of("startTZ")?, zone_of("endTZ")?) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<Root>
      <Property fixID="1" gimmickTypeTag="X" rotationY="40.5" position="4.5,0.5,2"/>
      <Property fixID="107" rotationY="40.5" position="4.5,0.5,2" model="CHARA_FM_BOARD_01"/>
      <Property fixID="62" position="1,2,3" model="CHARA_FM_UNI" objScale="1.5,1.5,1.5" objRot="-6.5,0,1" startEvent="EVENT_X" startTZ="TIME_ZONE_EVENING" endTZ="TIME_ZONE_MORNING"/>
    </Root>"#;

    #[test]
    fn rows_with_a_model() {
        let g = read(XML.as_bytes()).unwrap();
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].id, g[0].tag.as_str(), g[0].position, g[0].yaw, g[0].scale, g[0].story), (107, "FM_BOARD_01", [450.0, 50.0, 200.0], 40.5, [1.0; 3], false));
        assert_eq!((g[1].scale, g[1].rotation, g[1].story), ([1.5; 3], [-6.5, 0.0, 1.0], true));
    }

    #[test]
    fn spans_wrap() {
        let g = read(XML.as_bytes()).unwrap();
        assert!(g[0].present(Time::Night));
        assert!(!g[1].present(Time::Noon) && g[1].present(Time::Evening) && g[1].present(Time::Night) && g[1].present(Time::Morning));
    }
}
