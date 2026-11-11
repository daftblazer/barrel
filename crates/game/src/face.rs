//! Facial expressions in events: what a message line's expression number (its fourth field)
//! does to the speaker's face, as Escha & Logy DX's executable does it
//! (`docs/formats.md`, "Event messages").
//!
//! An expression picks a brow, an eye and a mouth facial motion (or keeps the current one).
//! The motions are animations in the character's `<FileHead>_FACIAL` container, named from
//! the character's file head and these tables.

/// The brow and eye motions, by number.
pub const BROW_EYE: [&str; 7] = ["DEF", "FUN", "ANG", "SAD", "SER", "FUN2", "SAD"];

/// The mouth motions, by number: each mood twice, first talking (`MTH_M_`, a loop that opens
/// and closes the mouth), then still (`MTH_DEF_`).
pub const MOUTH: [&str; 7] = ["DEF", "FUN", "ANG", "SAD", "SER", "FUN2", "SAD"];

/// The brow, eye and mouth motion numbers an expression sets, `None` meaning keep. The mouth
/// number is the talking form; add 1 for the still one.
const EXPRESSIONS: [(Option<u8>, Option<u8>, Option<u8>); 11] = [
    (None, None, Some(0)),
    (None, None, Some(8)),
    (None, None, Some(2)),
    (None, None, Some(4)),
    (None, None, Some(6)),
    (None, None, None),
    (None, None, None),
    (None, None, None),
    (Some(4), Some(4), Some(8)),
    (Some(5), Some(5), Some(10)),
    (Some(6), Some(6), Some(12)),
];

/// Each character's facial animation names by motion number, read from the executable.
/// The executable keeps three lists of records, `{u64 CharaBase row, then the names}`: 7 brow
/// names, 7 eye names and 14 mouth names per character, the names ending in
/// `_M_Brw_B_Def`, `_M_Eye_B_Def` and `_M_Mth_B_Def`.
#[derive(Clone, Debug, Default)]
pub struct Tables {
    pub brow: std::collections::HashMap<u32, Vec<Option<String>>>,
    pub eye: std::collections::HashMap<u32, Vec<Option<String>>>,
    pub mouth: std::collections::HashMap<u32, Vec<Option<String>>>,
}

impl Tables {
    pub fn read(pe: &barrel_formats::pe::Pe) -> Tables {
        let mut t = Tables::default();
        let data = pe.data;
        let ptr = |at: usize| data.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
        let mut at = 8;
        while at + 8 <= data.len() {
            let name = ptr(at).and_then(|q| pe.name_at(q));
            let previous = ptr(at - 8).unwrap_or(0);
            // A record starts where a name follows a small number (the CharaBase row).
            if let (Some(first), true) = (name, previous < 0x10000) {
                let (map, count) = if first.ends_with("_M_Brw_B_Def") {
                    (&mut t.brow, 7)
                } else if first.ends_with("_M_Eye_B_Def") {
                    (&mut t.eye, 7)
                } else if first.ends_with("_M_Mth_B_Def") {
                    (&mut t.mouth, 14)
                } else {
                    at += 8;
                    continue;
                };
                // Slots may be null where the character lacks that motion.
                let names: Vec<Option<String>> =
                    (0..count).map(|k| ptr(at + 8 * k).and_then(|q| pe.name_at(q)).map(str::to_string)).collect();
                map.entry(previous as u32).or_insert(names);
                at += 8 * count;
                continue;
            }
            at += 8;
        }
        t
    }

    /// The facial animations a line with expression `expression` gives the character of
    /// CharaBase row `row`; `talking` picks the talking mouth. Falls back to `expression`'s
    /// names (Escha's order) with `head` for characters the tables do not list. A character
/// may lack some motions (a null in its record); then that part keeps its current motion.
    pub fn expression(&self, row: u32, head: &str, expression: i32, talking: bool) -> Face {
        let Some(&(brow, eye, mouth)) = usize::try_from(expression).ok().and_then(|e| EXPRESSIONS.get(e)) else {
            return Face { brow: None, eye: None, mouth: None };
        };
        let fallback = self::expression(head, expression, talking);
        // A listed character takes its own names (a null slot keeps the current motion);
        // an unlisted one falls back to Escha's order.
        let pick = |m: &std::collections::HashMap<u32, Vec<Option<String>>>, n: Option<u8>, fb: Option<String>| match m.get(&row) {
            Some(names) => n.and_then(|n| names.get(n as usize).cloned().flatten()),
            None => fb,
        };
        Face {
            brow: pick(&self.brow, brow, fallback.brow),
            eye: pick(&self.eye, eye, fallback.eye),
            mouth: pick(&self.mouth, mouth.map(|m| m + !talking as u8), fallback.mouth),
        }
    }
}

/// What one facial part should play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub brow: Option<String>,
    pub eye: Option<String>,
    pub mouth: Option<String>,
}

/// The facial animations a line with expression `expression` gives the character whose
/// file head is `head` (`PC20` for Escha); `talking` picks the talking mouth. Expressions
/// outside 0..=10 (-1 on most lines) change nothing.
pub fn expression(head: &str, expression: i32, talking: bool) -> Face {
    let Some(&(brow, eye, mouth)) = usize::try_from(expression).ok().and_then(|e| EXPRESSIONS.get(e)) else {
        return Face { brow: None, eye: None, mouth: None };
    };
    let part = |n: Option<u8>, kind: &str| n.map(|n| format!("{head}_EYE_M_{}_M_{kind}_B_Def", BROW_EYE[n as usize]));
    Face {
        brow: part(brow, "Brw"),
        eye: part(eye, "Eye"),
        mouth: mouth.map(|m| {
            let mood = MOUTH[m as usize / 2];
            let form = if talking { "M" } else { "DEF" };
            format!("{head}_MTH_{form}_{mood}_M_Mth_B_Def")
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        let f = expression("PC20", 10, true);
        assert_eq!(f.brow.as_deref(), Some("PC20_EYE_M_SAD_M_Brw_B_Def"));
        assert_eq!(f.eye.as_deref(), Some("PC20_EYE_M_SAD_M_Eye_B_Def"));
        assert_eq!(f.mouth.as_deref(), Some("PC20_MTH_M_SAD_M_Mth_B_Def"));
        let f = expression("PC20", 3, false);
        assert_eq!((f.brow, f.mouth.as_deref()), (None, Some("PC20_MTH_DEF_ANG_M_Mth_B_Def")));
        assert_eq!(expression("PC20", -1, true), Face { brow: None, eye: None, mouth: None });
        assert_eq!(expression("PC20", 6, true).mouth, None);
    }
}
