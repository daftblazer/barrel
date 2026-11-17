//! A character's settings from `saves\chara\chara.xml` (`<Chara BaseTag=...>`: `Size`, the
//! model's scale; `Height`, `Radius`; `CamH`, `CamR`: the camera's look height and a radius)
//! and `saves\chara\charabase.xml` (`<CharaBase BaseTag=...>`: `FMrad`, `FMhig`, `FMslp`, the
//! field collision's radius and height in metres and its steepest slope in degrees). Ryza
//! (`PC00A`): size 1.04, look height 160, collision radius 0.28 and height 1.5. Docs:
//! docs/formats.md, "Ryza's character tables".

use barrel_formats::table::{self, Element};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chara {
    /// The model's scale.
    pub size: [f32; 3],
    /// The play camera's look height above her feet, units.
    pub cam_height: f32,
    /// Her collision against the field: radius and height in units, steepest slope in degrees.
    pub radius: f32,
    pub height: f32,
    pub slope: f32,
}

impl Chara {
    /// The character `tag`'s settings from the two tables (`chara`, `charabase`).
    pub fn read(chara: &[u8], charabase: &[u8], tag: &str) -> Result<Chara, String> {
        let row = |bytes: &[u8], name: &str| -> Result<Element, String> {
            let root = table::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
            find(&root, name, tag).cloned().ok_or(format!("{name}: no {tag}"))
        };
        let (c, b) = (row(chara, "Chara")?, row(charabase, "CharaBase")?);
        let num = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).ok_or(format!("{tag}: no {k}"));
        let size = c.attr("Size").ok_or(format!("{tag}: no Size"))?.split(',').map(|v| v.trim().parse::<f32>().map_err(|e| e.to_string())).collect::<Result<Vec<f32>, String>>()?;
        Ok(Chara {
            size: [size[0], *size.get(1).unwrap_or(&size[0]), *size.get(2).unwrap_or(&size[0])],
            cam_height: num(&c, "CamH")?,
            radius: num(&b, "FMrad")? * 100.0,
            height: num(&b, "FMhig")? * 100.0,
            slope: num(&b, "FMslp")?,
        })
    }
}

/// The first element named `name` with `BaseTag` `tag`, anywhere under `e`.
fn find<'a>(e: &'a Element, name: &str, tag: &str) -> Option<&'a Element> {
    if e.name == name && e.attr("BaseTag") == Some(tag) {
        return Some(e);
    }
    e.children.iter().find_map(|c| find(c, name, tag))
}

/// The tag of row `row` of `chara.xml`: the character an event's slot number names (slot 2 is
/// `PC02A`).
pub fn row_tag(chara: &[u8], row: usize) -> Result<String, String> {
    let root = table::parse(chara).map_err(|e| format!("Chara: {e}"))?;
    root.children.iter().filter(|e| e.name == "Chara").nth(row).and_then(|e| e.attr("BaseTag")).map(str::to_string).ok_or(format!("Chara: no row {row}"))
}

/// A set of motions: its folder and, for a set shared by a body type, the hip height its
/// motions are made for (`charamotionset.xml`'s `SkeletonBase`; -1 for a character's own set).
#[derive(Clone, Debug, PartialEq)]
pub struct MotionSet {
    pub dir: String,
    pub skeleton_base: Option<f32>,
}

/// The sets a character's motions are looked for in, in order: the base's own motion set
/// (`charabase.xml`'s `motset0`) and the shared one of its body type (`motset1`), each named
/// by `charamotionset.xml`'s `Directory` (`CF01`, `CM01` ...). Ryza keeps one container a
/// motion (`motion::path`), unlike Sophie 2's one a set, so this is the same rule as Sophie
/// 2's (`barrel_formats::chara_motion::Motions::files`) over other files.
pub fn motion_sets(charabase: &[u8], motionset: &[u8], tag: &str) -> Result<Vec<MotionSet>, String> {
    let base = table::parse(charabase).map_err(|e| format!("CharaBase: {e}"))?;
    let sets = table::parse(motionset).map_err(|e| format!("CharaMotionset: {e}"))?;
    let row = find(&base, "CharaBase", tag).ok_or(format!("CharaBase: no {tag}"))?;
    let mut out = Vec::new();
    for key in ["motset0", "motset1"] {
        let Some(name) = row.attr(key).and_then(|t| t.strip_prefix("CHARA_MOTIONSET_")) else { continue };
        let set = sets.children.iter().find(|e| e.attr("MotionsetTag") == Some(name)).ok_or(format!("CharaMotionset: no {name}"))?;
        let skeleton_base = set.attr("SkeletonBase").and_then(|v| v.trim().parse::<f32>().ok()).filter(|&b| b > 0.0);
        out.push(MotionSet { dir: set.attr("Directory").ok_or(format!("CharaMotionset: {name} has no Directory"))?.to_string(), skeleton_base });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn slots_and_motion_folders() {
        let chara = br#"<Root><Chara BaseTag="PC00A"/><Chara BaseTag="PC01A"/><Chara BaseTag="PC02A"/></Root>"#;
        assert_eq!(super::row_tag(chara, 2).unwrap(), "PC02A");
        assert!(super::row_tag(chara, 3).is_err());
        let base = br#"<Root><CharaBase BaseTag="PC02A" motset0="CHARA_MOTIONSET_PC02A" motset1="CHARA_MOTIONSET_CM01"/></Root>"#;
        let sets = br#"<Root><CharaMotionset MotionsetTag="PC02A" Directory="PC02A" SkeletonBase="-1"/><CharaMotionset MotionsetTag="CM01" Directory="CM01" SkeletonBase="98"/></Root>"#;
        let found = super::motion_sets(base, sets, "PC02A").unwrap();
        assert_eq!(found.iter().map(|s| s.dir.as_str()).collect::<Vec<_>>(), ["PC02A", "CM01"]);
        assert_eq!(found.iter().map(|s| s.skeleton_base).collect::<Vec<_>>(), [None, Some(98.0)]);
        assert!(super::motion_sets(base, sets, "PC09A").is_err());
    }

    #[test]
    fn ryza_from_the_tables() {
        let chara = br#"<Root><Chara BaseTag="PC00B" Size="1,1,1" CamH="100"/><Chara BaseTag="PC00A" Foot="0" Size="1.04,1.04,1.04" Height="154" Radius="44" CamH="160" CamR="50"/></Root>"#;
        let base = br#"<Root><CharaBase BaseTag="PC00A" FMrad="0.28" FMhig="1.5" FMslp="75"/></Root>"#;
        let c = super::Chara::read(chara, base, "PC00A").unwrap();
        assert_eq!((c.size, c.cam_height, c.radius, c.height, c.slope), ([1.04; 3], 160.0, 28.0, 150.0, 75.0));
    }
}
