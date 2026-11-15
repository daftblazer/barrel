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

/// The layout (`uil_a21_telop01.xml`): where `node`'s image sits, in the layout's 1280 × 720
/// space (its top-left corner: the image's `pos` plus its parent nodes'), and which strip it
/// shows (`image_no`). The opening's twelve all sit at (240, 333), their 2380 × 156 strips a
/// third of that size there, so centred on a 1280 × 720 screen as in the game's recording.
pub fn place(layout: &[u8], node: &str) -> Result<Option<([f32; 2], u32)>, String> {
    let root = table::parse(layout).map_err(|e| format!("uil_a21_telop: {e}"))?;
    fn pos(e: &table::Element) -> [f32; 2] {
        let v: Vec<f32> = e.attr("pos").unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0)]
    }
    fn find(e: &table::Element, node: &str, at: [f32; 2]) -> Option<([f32; 2], u32)> {
        let here = pos(e);
        let at = [at[0] + here[0], at[1] + here[1]];
        if e.name == "node" && e.attr("name") == Some(node) {
            let image = e.children.iter().find(|c| c.name == "image")?;
            let p = pos(image);
            return Some(([at[0] + p[0], at[1] + p[1]], image.attr("image_no")?.trim().parse().ok()?));
        }
        e.children.iter().find_map(|c| find(c, node, at))
    }
    Ok(find(&root, node, [0.0, 0.0]))
}

/// The layout's space: 720 lines (the strips are drawn at 2160, three times as large).
pub const LAYOUT_HEIGHT: f32 = 720.0;
pub const STRIP_SCALE: f32 = LAYOUT_HEIGHT / 2160.0;

/// Where a strip at layout point `at` goes on a `screen` (pixels: left, top, right, bottom,
/// the layout scaled to the screen's height and centred across its width), and its texture
/// coordinates in an atlas `atlas` pixels wide and high that holds the strips at `scale` of
/// their 2160-line size (the `neo_` atlases 1, the others 0.5).
pub fn quad(strip: &Strip, at: [f32; 2], screen: (u32, u32), atlas: (u32, u32), scale: f32) -> ([f32; 4], [f32; 4]) {
    let k = screen.1 as f32 / LAYOUT_HEIGHT;
    let left = (screen.0 as f32 - 1280.0 * k) * 0.5;
    let [x, y, w, h] = strip.rect;
    let rect = [left + at[0] * k, at[1] * k, left + (at[0] + w * STRIP_SCALE) * k, (at[1] + h * STRIP_SCALE) * k];
    let (aw, ah) = (atlas.0 as f32, atlas.1 as f32);
    let uv = [x * scale / aw, y * scale / ah, (x + w) * scale / aw, (y + h) * scale / ah];
    (rect, uv)
}

#[cfg(test)]
mod tests {
    #[test]
    fn places_a_node_and_its_strip() {
        let xml = br#"<root><node name="top"><node name="telop01" pos="0, 0, 0"><image name="image" pos="240.00, 333.00, 0.00" image_no="1"/></node>
            <node name="name_group" pos="366.17, 460.00, 0.00"><node name="name00" pos="0, 0, 0"><image name="image" pos="10, 5, 0" image_no="0"/></node></node></node></root>"#;
        assert_eq!(super::place(xml, "telop01").unwrap(), Some(([240.0, 333.0], 1)));
        assert_eq!(super::place(xml, "name00").unwrap(), Some(([376.17, 465.0], 0)));
        assert_eq!(super::place(xml, "telop02").unwrap(), None);
        // The opening's first strip on a 1280 × 720 screen from the plain atlas (1200 × 1024):
        // centred (240 + 793 / 2 = 637), as the game shows it.
        let strip = super::Strip { texture: "ui/a21_telop01.g1t".into(), rect: [10.0, 10.0, 2380.0, 156.0] };
        let (rect, uv) = super::quad(&strip, [240.0, 333.0], (1280, 720), (1200, 1024), 0.5);
        assert!((rect[0] - 240.0).abs() < 1e-3 && (rect[2] - 1033.333).abs() < 1e-2 && (rect[3] - 385.0).abs() < 1e-3);
        assert!((uv[0] - 5.0 / 1200.0).abs() < 1e-6 && (uv[2] - 1195.0 / 1200.0).abs() < 1e-6);
        // A 16:10 screen: the layout centred across it.
        let (rect, _) = super::quad(&strip, [240.0, 333.0], (1920, 1200), (1200, 1024), 0.5);
        assert!((rect[0] - (1920.0 - 1280.0 * 1200.0 / 720.0) * 0.5 - 400.0).abs() < 1e-2);
    }


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
        let s = strips(&barrel_files::read(p).unwrap()).unwrap();
        assert_eq!(s.len(), 108);
        assert_eq!(s[0], Strip { texture: "ui/a21_telop01.g1t".into(), rect: [10.0, 10.0, 2380.0, 156.0] });
        assert_eq!(s[12].texture, "ui/a21_telop02.g1t");
    }
}
