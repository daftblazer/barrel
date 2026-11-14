//! Telops: the pre-drawn caption strips events show (the opening's narration, the character
//! name cards), docs/events.md, "Atelier Ryza DX's events".
//!
//! `u65 N` (`EVENT_TELOP_SET`) names one by its place in the executable's list of layout node
//! names: `name99`, `name00` to `name13` (the name cards), `telop00` to `telop26`, then `telop36`
//! to `telop86` (93 in all, so N runs 0 to 92). A `telopN` node of
//! `saves/ui_<lang>/a21_telop/uil_a21_telop01.xml` shows image N of the strips
//! `saves/ui_<lang>/etc/uis_a21_telop.xml` lists.

use barrel_formats::table;

/// The layout node a telop number (`u65`'s operand) names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    /// `name00` ... `name13` (and `name99`): a character's name card.
    Name(u32),
    /// `telopN`: image N of the caption strips.
    Caption(u32),
}

impl Node {
    /// The node for a telop number, as the executable's list orders them.
    pub fn of(number: u32) -> Option<Node> {
        match number {
            0 => Some(Node::Name(99)),
            1..=14 => Some(Node::Name(number - 1)),
            15..=41 => Some(Node::Caption(number - 15)),
            42..=92 => Some(Node::Caption(number - 42 + 36)),
            _ => None,
        }
    }
}

/// One caption strip: in which atlas (`ui/a21_telop01.g1t` ...) and where, in the layout's
/// 2160-line pixels (`uvwh`: left, top, width, height).
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub texture: String,
    pub rect: [f32; 4],
}

/// The strips of `uis_a21_telop.xml`, by image number (`etc_a21_telop_NNNN`).
pub fn strips(bytes: &[u8]) -> Result<Vec<Strip>, String> {
    let root = table::parse(bytes).map_err(|e| format!("uis_a21_telop: {e}"))?;
    let list = root.children.iter().find(|e| e.name == "image_list").ok_or("uis_a21_telop: no image_list")?;
    let mut out: Vec<(u32, Strip)> = Vec::new();
    for e in list.children.iter().filter(|e| e.name == "image") {
        let n = e.attr("name").and_then(|n| n.rsplit('_').next()).and_then(|n| n.parse::<u32>().ok()).ok_or("uis_a21_telop: an image without its number")?;
        let rect: Vec<f32> = e.attr("uvwh").unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let rect: [f32; 4] = rect.try_into().map_err(|_| format!("uis_a21_telop: image {n}'s uvwh"))?;
        out.push((n, Strip { texture: e.attr("texture").unwrap_or("").to_string(), rect }));
    }
    out.sort_by_key(|(n, _)| *n);
    if out.iter().enumerate().any(|(i, (n, _))| *n as usize != i) {
        return Err("uis_a21_telop: the images are not numbered 0, 1, 2, ...".into());
    }
    Ok(out.into_iter().map(|(_, s)| s).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_name_the_executables_nodes() {
        assert_eq!(Node::of(0), Some(Node::Name(99)));
        assert_eq!(Node::of(14), Some(Node::Name(13)));
        // The opening's narration (`event_mm00_010`: 15 to 26) and the ending's (29 to 40).
        assert_eq!(Node::of(15), Some(Node::Caption(0)));
        assert_eq!(Node::of(26), Some(Node::Caption(11)));
        assert_eq!(Node::of(29), Some(Node::Caption(14)));
        assert_eq!(Node::of(41), Some(Node::Caption(26)));
        assert_eq!(Node::of(42), Some(Node::Caption(36)));
        assert_eq!(Node::of(92), Some(Node::Caption(86)));
        assert_eq!(Node::of(93), None);
    }

    #[test]
    fn reads_the_strips() {
        let xml = br#"<root resolution_hd="2160"><image_list name="etc_a21_telop">
            <image name="etc_a21_telop_0001" texture="ui/a21_telop01.g1t" texture_index="0" uvwh=" 10, 170, 2380, 156"/>
            <image name="etc_a21_telop_0000" texture="ui/a21_telop01.g1t" texture_index="0" uvwh=" 10, 10, 2380, 156"/>
        </image_list></root>"#;
        let s = strips(xml).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[1], Strip { texture: "ui/a21_telop01.g1t".into(), rect: [10.0, 170.0, 2380.0, 156.0] });
        assert!(strips(br#"<root><image_list><image name="etc_a21_telop_0001" uvwh="0,0,1,1"/></image_list></root>"#).is_err());
    }

    /// The install's English strips, when unpacked (`BARREL_RYZA_TELOPS` names
    /// `uis_a21_telop.xml`): 108, twelve to an atlas, the opening's first 2380 × 156 at 10, 10.
    #[test]
    fn the_installs_strips() {
        let Some(p) = std::env::var_os("BARREL_RYZA_TELOPS") else { return };
        let s = strips(&std::fs::read(p).unwrap()).unwrap();
        assert_eq!(s.len(), 108);
        assert_eq!(s[0], Strip { texture: "ui/a21_telop01.g1t".into(), rect: [10.0, 10.0, 2380.0, 156.0] });
        assert_eq!(s[12].texture, "ui/a21_telop02.g1t");
    }
}
