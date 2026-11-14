//! The effect attachment points of each model (`Saves/chara/CharaEffect.xml`, 7,826 rows for
//! 253 model bases, loaded by `0x1401ee3f0` as 0x78-byte rows at the character data's
//! +0x216c70): per row the model base (`BaseTag`, +0), the bone it hangs on (`Node`, +8),
//! `FollowAlpha` (+0x28) and `AddEffect` (+0x29) bytes, `Type` (+0x2c) and `Number` (+0x30),
//! then `Pos`, `Rot`, `Scale` and `Color` as floats from +0x34. `0x140202ab0 (base, out)` lists a
//! base's rows with a node name, in file order; `0x1401f7ce0` takes the k-th, so a row's place
//! within its base is its slot number. 7,763 rows name no effect (`Type` -1, `Number` -1:
//! attachment points with an offset and scale only; `FollowAlpha` 0, `AddEffect` 1); 63, all on
//! monster bases (`MOB*`), carry `Type` 0 and an effect number from 701 to 715 with
//! `FollowAlpha` 1 and `AddEffect` 0, and 8 more `Type` -1, `Number` 80.

use barrel_formats::table::Element;

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub node: String,
    pub follow_alpha: bool,
    pub add_effect: bool,
    pub kind: i32,
    pub number: i32,
    pub pos: [f32; 3],
    /// Degrees, presumably (as `AccessoryAdjust`'s).
    pub rot: [f32; 3],
    pub scale: [f32; 3],
    pub color: [f32; 4],
}

/// A model base's points, in file order (the slot numbers).
#[derive(Clone, Debug, PartialEq)]
pub struct Base {
    pub tag: String,
    pub points: Vec<Point>,
}

fn floats<const N: usize>(v: Option<&str>) -> [f32; N] {
    let mut out = [0.0; N];
    for (o, p) in out.iter_mut().zip(v.unwrap_or("").split(',')) {
        *o = p.trim().parse().unwrap_or(0.0);
    }
    out
}

pub fn bases(t: &Element) -> Vec<Base> {
    let mut out: Vec<Base> = Vec::new();
    for e in t.children.iter().filter(|e| e.name == "CharaEffect") {
        let tag = e.attr("BaseTag").unwrap_or("");
        let num = |n: &str| e.attr(n).and_then(|v| v.trim().parse().ok()).unwrap_or(-1);
        let point = Point {
            node: e.attr("Node").unwrap_or("").to_string(),
            follow_alpha: e.attr("FollowAlpha") == Some("1"),
            add_effect: e.attr("AddEffect") == Some("1"),
            kind: num("Type"),
            number: num("Number"),
            pos: floats(e.attr("Pos")),
            rot: floats(e.attr("Rot")),
            scale: floats(e.attr("Scale")),
            color: floats(e.attr("Color")),
        };
        // 0x140202ab0 skips rows with no node name.
        if point.node.is_empty() {
            continue;
        }
        match out.iter_mut().find(|b| b.tag == tag) {
            Some(b) => b.points.push(point),
            None => out.push(Base { tag: tag.to_string(), points: vec![point] }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn groups_by_base() {
        let x = br#"<Root><CharaEffect BaseTag="PC00" Node="H_0_GodNull" FollowAlpha="0" AddEffect="1" Type="-1" Number="-1" Pos="0,0,0" Rot="0,0,0" Scale="1,1,1" Color="0,0,0,0"/><CharaEffect BaseTag="MOB00B" Node="H_0_GodNull" FollowAlpha="1" AddEffect="0" Type="0" Number="705" Pos="0,0.1,0" Rot="0,0,0" Scale="40,40,40" Color="0,0,0,0"/><CharaEffect BaseTag="PC00" Node="H_4_L_HandAttach" FollowAlpha="0" AddEffect="1" Type="-1" Number="-1" Pos="1,2,3" Rot="0,0,0" Scale="1,1,1" Color="0,0,0,0"/></Root>"#;
        let b = super::bases(&barrel_formats::table::parse(x).unwrap());
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].points.len(), b[0].points[1].pos), (2, [1.0, 2.0, 3.0]));
        assert_eq!((b[1].points[0].number, b[1].points[0].follow_alpha, b[1].points[0].scale[0]), (705, true, 40.0));
    }
}
