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

/// An assignment card (`SubjectCardData.xml`, nine, a four-month period each).
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub name: String,
    /// What each line count pays (`0x1403604d0`, the record's +0x20 + 8 × step): steps 0 to 7
    /// are `lineBonusN`/`lineScoreN` for N = 1 to 8, step 8 (every square) `allBonus`/`allScore`;
    /// a `Bonus` row tag (empty for none) and the job points.
    pub steps: [(String, i32); 9],
    /// `score`, and the event it is tied to (`event`).
    pub score: i32,
    pub event: String,
}

pub fn cards(t: &Element) -> Vec<Card> {
    let text = |e: &Element, n: &str| e.attr(n).unwrap_or("").to_string();
    let int = |e: &Element, n: &str| e.attr(n).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    t.children
        .iter()
        .map(|e| Card {
            name: text(e, "Name"),
            steps: std::array::from_fn(|k| {
                if k < 8 {
                    (text(e, &format!("lineBonus{}", k + 1)), int(e, &format!("lineScore{}", k + 1)))
                } else {
                    (text(e, "allBonus"), int(e, "allScore"))
                }
            }),
            score: int(e, "score"),
            event: text(e, "event"),
        })
        .collect()
}

/// An assignment's saved state: five i32 (`subject/subject_save`, 20 bytes a row, at
/// `0x141099e08` in the running game, after the active card at `0x141099e00`): its count so far
/// (not held at the target: 4 on "synthesize one Neutralizer"), its completion in hundredths of
/// a percent (10000 done: `0x14034f1d0` marks an assignment in state 3, 5 or 6 met once it
/// reaches 10000), its state (8 cleared: `0x1403569d0` counts the active card's 25 squares whose
/// assignment is at 8; "Alpha Wolf", cleared by its battle, has a count of 0 and state 8), and two
/// more (0 and 1 on most).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedSubject {
    pub progress: i32,
    /// Completion, 10000 for done.
    pub completion: i32,
    pub state: i32,
    pub rest: [i32; 2],
}

impl SavedSubject {
    /// Cleared (state 8).
    pub fn cleared(&self) -> bool {
        self.state == 8
    }

    /// Achieved but not yet cleared on the card screen (state 7): `0x140357620` takes such a
    /// square, every half-second while the card is shown, to 8 and adds its score.
    pub fn achieved(&self) -> bool {
        self.state == 7
    }
}

pub fn saved(raw: &[u8]) -> Vec<SavedSubject> {
    raw.chunks_exact(20)
        .map(|c| {
            let w = |k: usize| i32::from_le_bytes(c[4 * k..4 * k + 4].try_into().unwrap());
            SavedSubject { progress: w(0), completion: w(1), state: w(2), rest: [w(3), w(4)] }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cards() {
        let x = br#"<Root><SubjectCardData Name="c" lineBonus1="B0" lineScore1="50" lineBonus8="B5" lineScore8="150" allBonus="BA" allScore="0" score="1000" event="E"/></Root>"#;
        let c = &cards(&crate::table::parse(x).unwrap())[0];
        assert_eq!((c.steps[0].clone(), c.steps[1].clone(), c.steps[7].1, c.steps[8].0.as_str(), c.score), (("B0".into(), 50), (String::new(), 0), 150, "BA", 1000));
    }

    #[test]
    fn reads() {
        let x = br#"<Root><SubjectData Name="n" Type="SUBJECT_CATEGORY_BTL" Score="30" CardTag="CARD_00" Pos="A1" isHiddenEvent="E" WorkCond="SUBJECT_WORK_COND_ADV_LV_COUNT_TOTAL" Prm1="8"/></Root>"#;
        let s = &subjects(&crate::table::parse(x).unwrap())[0];
        assert_eq!((s.score, s.square.as_str(), s.params.clone()), (30, "A1", vec!["8".to_string()]));
        let mut raw = Vec::new();
        for v in [8, 10000, 8, 0, 1] {
            raw.extend((v as i32).to_le_bytes());
        }
        let s = saved(&raw)[0];
        assert_eq!((s.progress, s.state, s.cleared()), (8, 8, true));
    }
}
