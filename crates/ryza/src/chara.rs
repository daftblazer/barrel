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

#[cfg(test)]
mod tests {
    #[test]
    fn ryza_from_the_tables() {
        let chara = br#"<Root><Chara BaseTag="PC00B" Size="1,1,1" CamH="100"/><Chara BaseTag="PC00A" Foot="0" Size="1.04,1.04,1.04" Height="154" Radius="44" CamH="160" CamR="50"/></Root>"#;
        let base = br#"<Root><CharaBase BaseTag="PC00A" FMrad="0.28" FMhig="1.5" FMslp="75"/></Root>"#;
        let c = super::Chara::read(chara, base, "PC00A").unwrap();
        assert_eq!((c.size, c.cam_height, c.radius, c.height, c.slope), ([1.04; 3], 160.0, 28.0, 150.0, 75.0));
    }
}
