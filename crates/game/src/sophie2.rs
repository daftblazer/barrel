//! Atelier Sophie 2 pieces that differ from Escha & Logy DX's (docs/events.md, "Atelier
//! Sophie 2's events").

use barrel_formats::table::Element;
use std::collections::HashMap;

/// Where a character's motions come from: `saves/chara/chara.xml` (a row's `BaseTag`),
/// `charabase.xml` (the base's `motset0` and `motset1`, `CHARA_MOTIONSET_<tag>`),
/// `charamotionset.xml` (a set's `Directory` and `BaseName`) and `charamotionconvert.xml`
/// (per base, motion numbers swapped for others).
pub struct Motions {
    bases: Vec<String>,
    sets: HashMap<String, [Option<String>; 2]>,
    dirs: HashMap<String, (String, String)>,
    convert: HashMap<(String, u32), u32>,
}

impl Motions {
    pub fn new(chara: &Element, charabase: &Element, motionset: &Element, convert: &Element) -> Motions {
        let a = |e: &Element, k: &str| e.attr(k).unwrap_or_default().to_string();
        Motions {
            bases: chara.children.iter().map(|e| a(e, "BaseTag")).collect(),
            sets: charabase
                .children
                .iter()
                .map(|e| {
                    let set = |k: &str| e.attr(k).and_then(|t| t.strip_prefix("CHARA_MOTIONSET_")).map(str::to_string);
                    (a(e, "BaseTag"), [set("motset0"), set("motset1")])
                })
                .collect(),
            dirs: motionset.children.iter().map(|e| (a(e, "MotionsetTag"), (a(e, "Directory"), a(e, "BaseName")))).collect(),
            convert: convert
                .children
                .iter()
                .filter_map(|e| Some(((a(e, "CharaBaseTag"), e.attr("OrigninalMotionNo")?.trim().parse().ok()?), e.attr("ModifiedMotionNo")?.trim().parse().ok()?)))
                .collect(),
        }
    }

    /// The files a character row's motion may be in, in the order to look: the base's own
    /// set, then its shared one; each `(container, g1a name)`, the container
    /// `data/x64/character/<dir>/<base>_motion.elixir.gz` (lower case, as the archives
    /// store it) and the name `<BASE>_<motion as six digits>.g1a`. The motion goes through
    /// the base's conversion first.
    pub fn files(&self, row: usize, motion: u32) -> Vec<(String, String)> {
        let Some(base) = self.bases.get(row) else { return Vec::new() };
        let motion = self.convert.get(&(base.clone(), motion)).copied().unwrap_or(motion);
        let Some(sets) = self.sets.get(base) else { return Vec::new() };
        sets.iter()
            .flatten()
            .filter_map(|s| self.dirs.get(s))
            .map(|(dir, name)| {
                (format!("data/x64/character/{}/{}_motion.elixir.gz", dir.to_lowercase(), name.to_lowercase()), format!("{}_{motion:06}.g1a", name.to_uppercase()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rows_motion_files() {
        let t = |x: &str| barrel_formats::table::parse(x.as_bytes()).unwrap();
        let m = Motions::new(
            &t(r#"<Root><Chara BaseTag="PC00A"/><Chara BaseTag="PC01A"/></Root>"#),
            &t(r#"<Root><CharaBase BaseTag="PC01A" motset0="CHARA_MOTIONSET_PC01A" motset1="CHARA_MOTIONSET_CF01"/></Root>"#),
            &t(r#"<Root><CharaMotionset MotionsetTag="PC01A" Directory="PC01A" BaseName="PC01A"/><CharaMotionset MotionsetTag="CF01" Directory="CF01" BaseName="CF01"/></Root>"#),
            &t(r#"<Root><CharaMotionConvert CharaBaseTag="PC01A" OrigninalMotionNo="10" ModifiedMotionNo="800010"/></Root>"#),
        );
        assert_eq!(
            m.files(1, 1000),
            [("data/x64/character/pc01a/pc01a_motion.elixir.gz".to_string(), "PC01A_001000.g1a".to_string()), ("data/x64/character/cf01/cf01_motion.elixir.gz".to_string(), "CF01_001000.g1a".to_string())]
        );
        assert_eq!(m.files(1, 10)[0].1, "PC01A_800010.g1a");
        assert!(m.files(0, 1000).is_empty());
    }
}
