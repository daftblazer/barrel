//! The party's field talk (English Escha executable): the lines a character says on entering a
//! field, while gathering and so on, each tied to a character and to the story events that
//! unlock it. Two tables: 84 groups of 0x24 bytes at [`GROUPS_EN`] and 480 lines of 0x28 bytes
//! at [`LINES_EN`] (the game copies both into its talk manager, `0x140260f80`).
//!
//! A group says when its lines are said: its kind (0 entering a field, 1 gathering in a map,
//! 3 to 9 other moments) and, for the map kinds, the map number it belongs to. A line holds its
//! group, the character who says it (the face cut), two event conditions (the event that must
//! have been seen and the one that must not, -1 for none: `0x14035f2d0`) and the text.
//! The game lists the lines by their group's kind (`0x140260f80` builds a map from kind to
//! lines in table order).

use barrel_formats::pe::Pe;

/// The groups' table.
pub const GROUPS_EN: u64 = 0x1_408e_20c0;
pub const GROUP_COUNT: usize = 84;
const GROUP: usize = 0x24;
/// The lines' table, right after the groups'.
pub const LINES_EN: u64 = 0x1_408e_2c90;
pub const LINE_COUNT: usize = 480;
const LINE: usize = 0x28;

/// When a group's lines are said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    /// The moment: 0 entering any field, 1 gathering in the map `param`, 9 entering the map
    /// `param`, and so on (the game's lookups by kind, `0x14028a2a0`).
    pub kind: i32,
    /// The map number for the map kinds (a character or a step otherwise; -1 for none).
    pub param: i32,
}

/// One line of talk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// Its number (the table's order).
    pub id: usize,
    /// Its group's index.
    pub group: usize,
    /// The character who says it, a party member number (0 Escha, 1 Logy ...); the face cut.
    pub face: i32,
    /// The event that must have been seen first, if any.
    pub needs: Option<usize>,
    /// The event that must not have been seen, if any.
    pub unless: Option<usize>,
    /// The text's number in the game's string table (the lines of other languages share it).
    pub text_id: u32,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Talk {
    pub groups: Vec<Group>,
    pub lines: Vec<Line>,
}

impl Talk {
    /// The group a line belongs to.
    pub fn group_of(&self, line: &Line) -> &Group {
        &self.groups[line.group]
    }
}

fn word(r: &[u8], k: usize) -> i32 {
    i32::from_le_bytes(r[4 * k..4 * k + 4].try_into().unwrap())
}

fn event(v: i32) -> Option<usize> {
    usize::try_from(v).ok()
}

/// Reads the two tables from the executable.
pub fn read(pe: &Pe) -> Option<Talk> {
    let at = pe.offset(GROUPS_EN)?;
    let groups = pe
        .data
        .get(at..at + GROUP_COUNT * GROUP)?
        .chunks_exact(GROUP)
        .map(|r| Group { kind: word(r, 3), param: word(r, 4) })
        .collect();
    let at = pe.offset(LINES_EN)?;
    let lines = pe
        .data
        .get(at..at + LINE_COUNT * LINE)?
        .chunks_exact(LINE)
        .enumerate()
        .map(|(id, r)| {
            let ptr = u64::from_le_bytes(r[32..40].try_into().unwrap());
            let at = pe.offset(ptr)?;
            let rest = pe.data.get(at..)?;
            let text = String::from_utf8_lossy(&rest[..rest.iter().position(|&b| b == 0)?]).into_owned();
            Some(Line {
                id,
                group: usize::try_from(word(r, 1)).ok().filter(|&g| g < GROUP_COUNT)?,
                face: word(r, 2),
                needs: event(word(r, 3)),
                unless: event(word(r, 4)),
                text_id: word(r, 5) as u32,
                text,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Talk { groups, lines })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real tables (`BARREL_EXE` = the English executable): 84 groups, 480 lines; the line
    /// Logy says on entering the Southern Ruins.
    #[test]
    #[ignore = "needs the game's executable (BARREL_EXE)"]
    fn the_tables_read() {
        let Ok(path) = std::env::var("BARREL_EXE") else { return };
        let exe = barrel_files::read(path).unwrap();
        let talk = read(&Pe::parse(&exe).unwrap()).unwrap();
        assert_eq!((talk.groups.len(), talk.lines.len()), (84, 480));
        let l = &talk.lines[5];
        assert_eq!(l.text, "This place is worth investigating. We might find something interesting.");
        assert_eq!((l.face, l.text_id, l.needs, l.unless), (1, 0x2b62, None, None));
        assert_eq!(talk.group_of(l), &Group { kind: 0, param: -1 });
        // Lines after the first three unlock with story events.
        assert_eq!(talk.lines[6].needs, Some(0x1fe));
    }
}
