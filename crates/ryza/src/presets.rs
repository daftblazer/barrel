//! The DX release's graphics presets (`saves/graphicssetting/*.xml`, docs/formats.md, "the
//! graphics presets"): the general preset names each table's type, and Barrel takes the
//! values from those tables.

use barrel_formats::table::{self, Element};

use crate::grass;

/// The values Barrel draws with from one general preset of the game's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graphics {
    /// The plants (`GrassQualityType`, `GrassRangeType`).
    pub grass: grass::Preset,
    /// Where an object's full model gives way to its middle level of detail, and that to its
    /// low one, from the eye (`LODDistType`: `Lod_hi_dist`, `Lod_mid_dist`).
    pub lod: [f32; 2],
    /// How far the near, middle and far field objects are drawn (`DispRangeType`: `ObjNear`,
    /// `ObjMiddle`, `ObjFar`); the field's own ranges cap them (`camera::DrawRanges::limit`).
    pub objects: [f32; 3],
    /// A scale of how far the sun's shadows reach (`ShadowDistType`: `Distance`, 1 on High).
    pub shadow: f32,
}

impl Graphics {
    /// The game's `High` general preset, as shipped.
    pub const HIGH: Graphics = Graphics { grass: grass::Preset::MIDDLE, lod: [10000.0, 30000.0], objects: [8000.0, 20000.0, 40000.0], shadow: 1.0 };

    /// The general preset `general` ("High", "Middle", "Low"), its tables read through `read`
    /// (a file name under `saves/graphicssetting`).
    pub fn read(general: &str, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Graphics, String> {
        let file = |n: &str| read(n).ok_or_else(|| format!("no {n}"));
        let g = element(&file("generalpreset.xml")?, "GeneralPreset", general)?;
        let kind = |k: &str| g.attr(k).map(str::to_owned).ok_or_else(|| format!("GeneralPreset {general}: no {k}"));
        let (quality, range, lod, disp) = (kind("GrassQualityType")?, kind("GrassRangeType")?, kind("LODDistType")?, kind("DispRangeType")?);
        let shadow = kind("ShadowDistType")?;
        let q = element(&file("grassqualitypreset.xml")?, "GrassQualityPreset", &quality)?;
        let r = element(&file("grassrangepreset.xml")?, "GrassRangePreset", &range)?;
        let l = element(&file("loddistpreset.xml")?, "LODDistPreset", &lod)?;
        let d = element(&file("disprangepreset.xml")?, "DispRangePreset", &disp)?;
        let s = element(&file("shadowdistpreset.xml")?, "ShadowDistPreset", &shadow)?;
        Ok(Graphics {
            grass: grass::Preset { density: number(&q, "Density")?, far: [number(&r, "FarClipBegin")?, number(&r, "FarClipEnd")?] },
            lod: [number(&l, "Lod_hi_dist")?, number(&l, "Lod_mid_dist")?],
            objects: [number(&d, "ObjNear")?, number(&d, "ObjMiddle")?, number(&d, "ObjFar")?],
            shadow: number(&s, "Distance")?,
        })
    }
}

/// The `tag` element of type `kind` for every field (`Field="Default"`) in a preset table.
pub fn element(bytes: &[u8], tag: &str, kind: &str) -> Result<Element, String> {
    let root = table::parse(bytes).map_err(|e| format!("{tag}: {e}"))?;
    let mut all = vec![&root];
    while let Some(e) = all.pop() {
        if e.name == tag && e.attr("Type") == Some(kind) && e.attr("Field").is_none_or(|f| f == "Default") {
            return Ok(e.clone());
        }
        all.extend(e.children.iter());
    }
    Err(format!("{tag}: no {kind}"))
}

fn number(e: &Element, k: &str) -> Result<f32, String> {
    e.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).ok_or_else(|| format!("{}: no {k}", e.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(n: &str) -> Option<Vec<u8>> {
        Some(
            match n {
                "generalpreset.xml" => r#"<Root><GeneralPreset Field="Default" Type="High" GrassRangeType="Middle" GrassQualityType="Middle" LODDistType="High" DispRangeType="High" ShadowDistType="High"/><GeneralPreset Field="Default" Type="Middle" GrassRangeType="Low" GrassQualityType="Low" LODDistType="Middle" DispRangeType="Middle" ShadowDistType="Middle"/></Root>"#,
                "grassqualitypreset.xml" => r#"<Root><GrassQualityPreset Field="Default" Type="Middle" Density="1"/><GrassQualityPreset Field="Default" Type="Low" Density="0.75"/></Root>"#,
                "grassrangepreset.xml" => r#"<Root><GrassRangePreset Field="Default" Type="Middle" FarClipBegin="8000" FarClipEnd="15000"/><GrassRangePreset Field="Default" Type="Low" FarClipBegin="3000" FarClipEnd="3100"/></Root>"#,
                "loddistpreset.xml" => r#"<Root><LODDistPreset Field="Default" Type="High" Lod_hi_dist="10000" Lod_mid_dist="30000"/><LODDistPreset Field="OTHER" Type="Middle" Lod_hi_dist="1" Lod_mid_dist="2"/><LODDistPreset Field="Default" Type="Middle" Lod_hi_dist="8000" Lod_mid_dist="25000"/></Root>"#,
                "disprangepreset.xml" => r#"<Root><DispRangePreset Field="Default" Type="High" ObjFar="40000" ObjMiddle="20000" ObjNear="8000"/><DispRangePreset Field="Default" Type="Middle" ObjFar="30000" ObjMiddle="15000" ObjNear="7000"/></Root>"#,
                "shadowdistpreset.xml" => r#"<Root><ShadowDistPreset Field="Default" Type="High" Distance="1"/><ShadowDistPreset Field="Default" Type="Middle" Distance="0.7"/></Root>"#,
                _ => return None,
            }
            .as_bytes()
            .to_vec(),
        )
    }

    #[test]
    fn reads_a_general_preset_through_its_tables() {
        assert_eq!(Graphics::read("High", &tables).unwrap(), Graphics::HIGH);
        assert_eq!(
            Graphics::read("Middle", &tables).unwrap(),
            Graphics { grass: grass::Preset { density: 0.75, far: [3000.0, 3100.0] }, lod: [8000.0, 25000.0], objects: [7000.0, 15000.0, 30000.0], shadow: 0.7 }
        );
        assert!(Graphics::read("Low", &tables).is_err());
        assert!(Graphics::read("High", &|_| None).is_err());
    }
}
