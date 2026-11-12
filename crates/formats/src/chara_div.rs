//! Motion divisions (`Saves/chara/CharaDiv.xml`, `SubCharaDiv.xml`): the named parts of a
//! character's motion that `s5` and `s11` play (`docs/events.md`, "Motions and moves").
//!
//! ```text
//! <CharaDiv BaseTag="PC20" MotionTag="ACT_01" Div0="0,1.99,0" Div1="2,6,1" Div2="6.01,-1,0"/>
//! ```
//!
//! `BaseTag` is the character's `CharaBase` `FileHead`, `MotionTag` the motion's name (its
//! `MOTION_*` name, the `CharaBase` column: `EV_ACT_1`, not the file it names), and each `DivN` is
//! `start, end, loops` in seconds, an end of -1 meaning the clip's end. The game reads them
//! into 12-byte records `{f32 start, f32 end, u8 loops}` at the character's +0x110 + 8 ×
//! motion (`0x1401f4c50`); without one it plays `{-1, -1, 1}`, the whole clip.

use std::collections::HashMap;

use crate::table::Element;

/// One division: start and end in seconds (`None`: the clip's end), and whether it loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Division {
    pub start: f32,
    pub end: Option<f32>,
    pub loops: bool,
}

/// Every motion's divisions by (base tag, motion tag), indexed by `N` (gaps are `None`).
pub type Divisions = HashMap<(String, String), Vec<Option<Division>>>;

/// Reads a `CharaDiv`/`SubCharaDiv` table's rows into `out` (a later row for the same motion
/// replaces an earlier one).
pub fn read(root: &Element, out: &mut Divisions) {
    for row in &root.children {
        let (Some(base), Some(motion)) = (row.attr("BaseTag"), row.attr("MotionTag")) else { continue };
        let mut divs: Vec<Option<Division>> = Vec::new();
        for (key, value) in &row.attributes {
            let Some(n) = key.strip_prefix("Div").and_then(|n| n.parse::<usize>().ok()) else { continue };
            let v: Vec<f32> = value.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            let [start, end, loops] = v[..] else { continue };
            if divs.len() <= n {
                divs.resize(n + 1, None);
            }
            divs[n] = Some(Division { start, end: (end >= 0.0).then_some(end), loops: loops != 0.0 });
        }
        out.insert((base.to_string(), motion.to_string()), divs);
    }
}

impl Division {
    /// The clip time `t` seconds into the division of a clip `duration` long: looping within it,
    /// or held at its end.
    pub fn clock(&self, t: f32, duration: f32) -> f32 {
        let end = self.end.unwrap_or(duration).clamp(self.start, duration.max(self.start));
        let len = end - self.start;
        if self.loops && len > 0.0 { self.start + t.max(0.0).rem_euclid(len) } else { self.start + t.clamp(0.0, len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_clocks() {
        let xml = r#"<Root><CharaDiv BaseTag="PC20" MotionTag="ACT_01" Div0="0,1.99,0" Div1="2,6,1" Div2="6.01,-1,0"/></Root>"#;
        let mut d = Divisions::new();
        read(&crate::table::parse_str(xml).unwrap(), &mut d);
        let divs = &d[&("PC20".to_string(), "ACT_01".to_string())];
        assert_eq!(divs.len(), 3);
        let loop_part = divs[1].unwrap();
        assert!(loop_part.loops);
        assert_eq!(loop_part.clock(5.0, 8.0), 3.0);
        assert_eq!(divs[2].unwrap().clock(10.0, 8.0), 8.0);
        assert_eq!(divs[0].unwrap().clock(3.0, 8.0), 1.99);
    }
}
