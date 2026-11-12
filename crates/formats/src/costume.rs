//! Costumes (`Saves/costume/*.xml`): a character's outfit is a body (a `CharaBase` model of its
//! own) with a head and a face accessory attached at the body's nodes.

use crate::table::Element;

/// `CostumeBody.xml`: an outfit.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub name: String,
    /// Whose outfit it is (`COSTUME_CHARA_ESCHA` ...).
    pub chara: String,
    /// Its model: a `CharaBase` row's tag (`CHARA_BASE_PC20B` → `PC20B`).
    pub base: String,
    pub priority: i32,
    /// The nodes the head and face accessories hang from.
    pub head_attach: String,
    pub face_attach: String,
}

/// `CostumeHead.xml` / `CostumeFace.xml`: an accessory (`file` empty for none: "なし").
#[derive(Clone, Debug, PartialEq)]
pub struct Accessory {
    pub name: String,
    /// The model's name stem (`HEAD00`).
    pub file: String,
    pub priority: i32,
}

/// `AccessoryAdjust.xml`: how an accessory sits on a body.
#[derive(Clone, Debug, PartialEq)]
pub struct Adjust {
    pub body: String,
    /// `COSTUME_PARTS_HEAD` or `COSTUME_PARTS_FACE`.
    pub parts: String,
    pub accessory: String,
    pub scale: [f32; 3],
    /// Degrees.
    pub rotation: [f32; 3],
    pub offset: [f32; 3],
}

/// `PartsInvisible.xml`: the body's own parts hidden while an accessory is worn.
#[derive(Clone, Debug, PartialEq)]
pub struct Hide {
    pub body: String,
    pub parts: String,
    pub accessory: String,
    pub exclude: bool,
    /// Node names (`P_M_Acc_Hat1`).
    pub hidden: Vec<String>,
}

/// `CostumeChara.xml`: a character's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Chara {
    pub name: String,
    pub target: String,
    pub body: String,
    pub head: String,
    pub face: String,
}

fn floats(e: &Element, name: &str, default: f32) -> [f32; 3] {
    let v: Vec<f32> = e.attr(name).unwrap_or("").split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if v.len() == 3 { [v[0], v[1], v[2]] } else { [default; 3] }
}

fn text(e: &Element, name: &str) -> String {
    e.attr(name).unwrap_or("").to_string()
}

fn int(e: &Element, name: &str) -> i32 {
    e.attr(name).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

pub fn bodies(t: &Element) -> Vec<Body> {
    t.children
        .iter()
        .map(|e| Body {
            name: text(e, "name"),
            chara: text(e, "costume_chara"),
            base: text(e, "chara_base").trim_start_matches("CHARA_BASE_").to_string(),
            priority: int(e, "priority"),
            head_attach: text(e, "head_attach"),
            face_attach: text(e, "face_attach"),
        })
        .collect()
}

pub fn accessories(t: &Element) -> Vec<Accessory> {
    t.children.iter().map(|e| Accessory { name: text(e, "name"), file: text(e, "filename_base"), priority: int(e, "priority") }).collect()
}

pub fn adjusts(t: &Element) -> Vec<Adjust> {
    t.children
        .iter()
        .map(|e| Adjust {
            body: text(e, "body"),
            parts: text(e, "parts"),
            accessory: text(e, "accessory"),
            scale: floats(e, "scale", 1.0),
            rotation: floats(e, "rotation", 0.0),
            offset: floats(e, "offset", 0.0),
        })
        .collect()
}

pub fn hides(t: &Element) -> Vec<Hide> {
    t.children
        .iter()
        .map(|e| Hide {
            body: text(e, "body"),
            parts: text(e, "parts"),
            accessory: text(e, "accessory"),
            exclude: e.attr("exclude") == Some("true"),
            hidden: (0..8).filter_map(|k| e.attr(&format!("invisible_parts{k}")).map(str::to_string)).collect(),
        })
        .collect()
}

pub fn charas(t: &Element) -> Vec<Chara> {
    t.children
        .iter()
        .map(|e| Chara { name: text(e, "name"), target: text(e, "target_chara"), body: text(e, "default_body"), head: text(e, "default_head"), face: text(e, "default_face") })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows() {
        let x = br#"<Root><AccessoryAdjust body="B" parts="COSTUME_PARTS_HEAD" accessory="COSTUME_HEAD_HEAD01" scale="0.9,0.9,0.9" rotation="2,0,0" offset="0,-8.2,-2"/></Root>"#;
        let t = crate::table::parse(x).unwrap();
        let a = &adjusts(&t)[0];
        assert_eq!((a.scale, a.rotation, a.offset), ([0.9; 3], [2.0, 0.0, 0.0], [0.0, -8.2, -2.0]));
        let x = br#"<Root><PartsInvisible body="B" parts="P" accessory="A" exclude="true" invisible_parts0="P_M_Acc_Hat1"/></Root>"#;
        let h = &hides(&crate::table::parse(x).unwrap())[0];
        assert_eq!((h.exclude, h.hidden.clone()), (true, vec!["P_M_Acc_Hat1".to_string()]));
        let x = br#"<Root><CostumeBody name="n" costume_chara="COSTUME_CHARA_ESCHA" chara_base="CHARA_BASE_PC20B" priority="1" head_attach="H" face_attach="F"/></Root>"#;
        assert_eq!(bodies(&crate::table::parse(x).unwrap())[0].base, "PC20B");
    }
}
