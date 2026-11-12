//! Assignments (`Saves/subject/SubjectData.xml`, 457 rows) and the game save's record of them
//! (`subject/subject_save`: one 20-byte entry per row).

use crate::table::Element;

/// An assignment.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    pub name: String,
    /// `SUBJECT_CATEGORY_*` (`BTL`, ...).
    pub category: String,
    pub score: i32,
    /// The card and square it sits on (`CARD_00`, `A1`), when it has them.
    pub card: String,
    pub square: String,
    /// What it counts (`SUBJECT_WORK_COND_*`), when it counts something, and its parameters
    /// (`Prm1` .. `Prm5`; `Prm1` the target for the counting ones).
    pub condition: String,
    pub params: Vec<String>,
    /// The event that must have been seen before it shows (`isHiddenEvent`).
    pub hidden_until: String,
    /// The `Bonus` row it grants, and whether it comes back (`Renew`).
    pub bonus: String,
    pub renew: String,
}

pub fn subjects(t: &Element) -> Vec<Subject> {
    let text = |e: &Element, n: &str| e.attr(n).unwrap_or("").to_string();
    t.children
        .iter()
        .map(|e| Subject {
            name: text(e, "Name"),
            category: text(e, "Type"),
            score: e.attr("Score").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
            card: text(e, "CardTag"),
            square: text(e, "Pos"),
            condition: text(e, "WorkCond"),
            params: (1..=5).map_while(|k| e.attr(&format!("Prm{k}")).map(str::to_string)).collect(),
            hidden_until: text(e, "isHiddenEvent"),
            bonus: text(e, "Bonus"),
            renew: text(e, "Renew"),
        })
        .collect()
}

/// An assignment's saved state: five i32 (`subject/subject_save`, 20 bytes a row). The first is
/// its count so far, not held at the target (the oracle save: 8 on "adventurer level 8", 4 on
/// "synthesize one Neutralizer"); the other four read (10000, 8, 0, 1) on every counting
/// assignment and (0, 1, 0, 0) on one an event clears ("Fix the Windmill!"); not named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedSubject {
    pub progress: i32,
    pub words: [i32; 4],
}

pub fn saved(raw: &[u8]) -> Vec<SavedSubject> {
    raw.chunks_exact(20)
        .map(|c| {
            let w = |k: usize| i32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap());
            SavedSubject { progress: w(0), words: [w(1), w(2), w(3), w(4)] }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads() {
        let x = br#"<Root><SubjectData Name="n" Type="SUBJECT_CATEGORY_BTL" Score="30" CardTag="CARD_00" Pos="A1" isHiddenEvent="E" WorkCond="SUBJECT_WORK_COND_ADV_LV_COUNT_TOTAL" Prm1="8"/></Root>"#;
        let s = &subjects(&crate::table::parse(x).unwrap())[0];
        assert_eq!((s.score, s.square.as_str(), s.params.clone()), (30, "A1", vec!["8".to_string()]));
        let mut raw = Vec::new();
        for v in [8, 10000, 8, 0, 1] {
            raw.extend((v as i32).to_le_bytes());
        }
        assert_eq!(saved(&raw), vec![SavedSubject { progress: 8, words: [10000, 8, 0, 1] }]);
    }
}
