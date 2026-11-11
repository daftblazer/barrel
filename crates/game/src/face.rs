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
