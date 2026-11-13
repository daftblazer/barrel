//! Event bytecode (`*.ebd`): story events, battle skills, shop and system scenes.
//!
//! A file is a list of actions (the event editor's blocks), then a short tail:
//!
//! ```text
//! u32 action count
//! per action:  u32 flags (always 0 so far), string name, string description,
//!              u32 word count, that many u32 words (the body), ending in 0x7f000000
//! tail:        u32 1, u32 1, 0x7f000000  (an empty body)
//!              u32 resource count, then that many strings, the last one possibly cut
//!              off by the end of the file
//! ```
//!
//! A string is `u32 length` (the terminating 0 included) and its bytes, in Shift-JIS, with
//! no padding, so words that follow a string are not 4-byte aligned in the file.
//!
//! In a body, a word whose top byte is 0x70..=0x7e is a command: the top byte is its group
//! (written `p`..`~`), bits 16..24 are flags and the low 16 bits are its number, so `q4` is
//! group 0x71 command 4. 0x7f000000 ends the body. The other words are operands, mostly
//! `(value, type)` pairs, with type 0 an integer literal. How many operands each command
//! takes is not fully known yet.

use crate::{Error, u32_at};

/// The word that ends a body.
pub const END: u32 = 0x7f00_0000;
/// Event files are small; this bounds counts read from corrupt data.
const MAX_WORDS: usize = 1 << 22;

#[derive(Clone, Debug)]
pub struct Action {
    pub flags: u32,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub words: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Ebd {
    pub actions: Vec<Action>,
    /// Resource names the event uses (cameras, "ULT" settings and the like).
    pub resources: Vec<Vec<u8>>,
}

/// A body word, classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    Command { group: u8, flags: u8, number: u16 },
    End,
    Operand(u32),
}

impl Word {
    pub fn classify(w: u32) -> Word {
        let top = (w >> 24) as u8;
        match top {
            0x70..=0x7e => Word::Command { group: top, flags: (w >> 16) as u8, number: w as u16 },
            0x7f if w == END => Word::End,
            _ => Word::Operand(w),
        }
    }
}

impl std::fmt::Display for Word {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Word::Command { group, flags: 0, number } => write!(f, "{}{number}", group as char),
            Word::Command { group, flags, number } => write!(f, "{}{number}/{flags:#x}", group as char),
            Word::End => write!(f, "END"),
            Word::Operand(v) => write!(f, "{}", v as i32),
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u32(&mut self) -> Result<u32, Error> {
        let v = u32_at(self.data, self.pos)?;
        self.pos += 4;
        Ok(v)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn string(&mut self) -> Result<Vec<u8>, Error> {
        let len = self.u32()? as usize;
        let bytes = self.data.get(self.pos..self.pos + len).ok_or(Error::Corrupt("ebd string past the end"))?;
        self.pos += len;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(bytes[..end].to_vec())
    }

    fn body(&mut self) -> Result<Vec<u32>, Error> {
        let count = self.u32()? as usize;
        if count == 0 || count > MAX_WORDS {
            return Err(Error::Corrupt("ebd body size"));
        }
        let mut words = Vec::with_capacity(count);
        for _ in 0..count {
            words.push(self.u32()?);
        }
        if words.last() != Some(&END) {
            return Err(Error::Corrupt("ebd body does not end with END"));
        }
        Ok(words)
    }
}

/// A Shallie event command word as Escha's that means the same (`docs/events.md`, "Shallie's
/// `q` commands ..."): the `q` commands matched by their code (most one number down from `q15`
/// on), the rest of `q` marked as Shallie's own (flags `0xee`: no Escha command, so passed by),
/// every other group as it is (the `u` group lines up; `s`, `t` and the others not checked).
pub fn shallie_to_escha(word: u32) -> u32 {
    if word >> 16 != 0x7100 {
        return word;
    }
    let n = word & 0xffff;
    let escha = match n {
        2 | 6 | 11 | 12 => Some(n),
        16 | 17 | 25 | 34 | 35 | 41..=46 | 55..=57 => Some(n - 1),
        // Passed by in both games.
        3..=5 | 7..=9 | 13 | 14 => Some(n),
        _ => None,
    };
    escha.map_or(0x71ee_0000 | n, |e| 0x7100_0000 | e)
}

impl Ebd {
    /// This event with every command word put through `map` (operands left as they are), and
    /// the operand table for the new words (each carrying its old word's count).
    pub fn translate(&self, table: &OperandTable, map: impl Fn(u32) -> u32) -> Result<(Ebd, OperandTable), Error> {
        let mut entries: Vec<(u32, Option<u8>)> = Vec::new();
        let mut actions = Vec::with_capacity(self.actions.len());
        for a in &self.actions {
            let mut words = Vec::with_capacity(a.words.len());
            for c in decode(&a.words, table)? {
                let w = map(c.word);
                if !entries.iter().any(|(k, _)| *k == w) {
                    entries.push((w, table.get(c.word).flatten().map(Some).unwrap_or(None)));
                }
                words.push(w);
                words.extend_from_slice(c.operands);
            }
            words.push(END);
            actions.push(Action { words, ..a.clone() });
        }
        Ok((Ebd { actions, resources: self.resources.clone() }, OperandTable::new(&entries)))
    }

    pub fn parse(data: &[u8]) -> Result<Ebd, Error> {
        let mut r = Reader { data, pos: 0 };
        let count = r.u32()? as usize;
        if count > data.len() / 16 {
            return Err(Error::Corrupt("ebd action count"));
        }
        let mut actions = Vec::with_capacity(count);
        for _ in 0..count {
            let flags = r.u32()?;
            let name = r.string()?;
            let description = r.string()?;
            let words = r.body()?;
            actions.push(Action { flags, name, description, words });
        }
        if r.u32()? != 1 || r.body()? != [END] {
            return Err(Error::Corrupt("ebd tail"));
        }
        let mut resources = Vec::new();
        if !r.at_end() {
            let n = r.u32()? as usize;
            if n > data.len() {
                return Err(Error::Corrupt("ebd resource count"));
            }
            for _ in 0..n {
                if r.at_end() {
                    break;
                }
                resources.push(r.string()?);
            }
        }
        if !r.at_end() {
            return Err(Error::Corrupt("ebd has data after its resources"));
        }
        Ok(Ebd { actions, resources })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(out: &mut Vec<u8>, text: &[u8]) {
        out.extend_from_slice(&(text.len() as u32 + 1).to_le_bytes());
        out.extend_from_slice(text);
        out.push(0);
    }

    #[test]
    fn small_event() {
        let mut f = Vec::new();
        for w in [1u32, 0] {
            f.extend_from_slice(&w.to_le_bytes());
        }
        s(&mut f, b"name");
        s(&mut f, b"desc");
        for w in [3u32, 0x7100_000c, 11, END, 1, 1, END, 2] {
            f.extend_from_slice(&w.to_le_bytes());
        }
        s(&mut f, b"");
        s(&mut f, b"ULTSET_0");
        let e = Ebd::parse(&f).unwrap();
        assert_eq!(e.actions.len(), 1);
        assert_eq!(e.actions[0].name, b"name");
        assert_eq!(Word::classify(e.actions[0].words[0]), Word::Command { group: 0x71, flags: 0, number: 12 });
        assert_eq!(Word::classify(e.actions[0].words[0]).to_string(), "q12");
        assert_eq!(e.resources, vec![b"".to_vec(), b"ULTSET_0".to_vec()]);
    }
}

/// How many operand words each command takes, for one game.
pub struct OperandTable {
    counts: std::collections::HashMap<u32, Option<u8>>,
}

impl OperandTable {
    pub fn new(entries: &[(u32, Option<u8>)]) -> Self {
        OperandTable { counts: entries.iter().copied().collect() }
    }

    pub fn escha() -> Self {
        Self::new(crate::ebd_tables::ESCHA)
    }

    /// Atelier Ayesha DX's (measured, `ebd_tables::AYESHA`).
    pub fn ayesha() -> Self {
        Self::new(crate::ebd_tables::AYESHA)
    }

    /// Atelier Shallie DX's (measured, `ebd_tables::SHALLIE`).
    pub fn shallie() -> Self {
        Self::new(crate::ebd_tables::SHALLIE)
    }

    /// Atelier Sophie 2's (measured, `ebd_tables::SOPHIE2`).
    pub fn sophie2() -> Self {
        Self::new(crate::ebd_tables::SOPHIE2)
    }

    /// The table measured from a game's own bodies: for each command, the number of words up
    /// to the next command word (or `END`); a command always followed by the same number gets
    /// that count, one seen with several gets `None` (runs to the next command). Operands that
    /// look like command words make a count look smaller, so a measured table is a first
    /// draft to check against the executable's handlers, as Escha's was.
    pub fn measure<'a>(bodies: impl IntoIterator<Item = &'a [u32]>) -> Self {
        let mut seen: std::collections::HashMap<u32, std::collections::BTreeSet<usize>> = std::collections::HashMap::new();
        for body in bodies {
            let idx: Vec<usize> = (0..body.len()).filter(|&k| is_command_word(body[k])).collect();
            for pair in idx.windows(2) {
                let w = body[pair[0]];
                if matches!(Word::classify(w), Word::Command { .. }) {
                    seen.entry(w).or_default().insert(pair[1] - pair[0] - 1);
                }
            }
        }
        let counts = seen
            .into_iter()
            .map(|(w, c)| (w, if c.len() == 1 { c.first().and_then(|&n| u8::try_from(n).ok()) } else { None }))
            .collect();
        OperandTable { counts }
    }

    /// The entries, sorted by word.
    pub fn entries(&self) -> Vec<(u32, Option<u8>)> {
        let mut v: Vec<_> = self.counts.iter().map(|(&w, &c)| (w, c)).collect();
        v.sort();
        v
    }

    /// `Some(Some(n))` for a fixed count, `Some(None)` for a command whose count varies,
    /// `None` for a word this game does not use.
    pub fn get(&self, word: u32) -> Option<Option<u8>> {
        self.counts.get(&word).copied()
    }
}

/// One decoded command: its word and the operand words after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command<'a> {
    pub word: u32,
    pub operands: &'a [u32],
}

fn is_command_word(w: u32) -> bool {
    matches!(Word::classify(w), Word::Command { .. } | Word::End)
}

/// Split a body into commands. The final `END` is not returned.
///
/// A command with a fixed count takes exactly that many words, and the word after them must
/// be a command word or `END`. A command with a varying count runs to the next command word.
/// An unknown command is an error, so a table that is wrong for a game shows up at once.
pub fn decode<'a>(body: &'a [u32], table: &OperandTable) -> Result<Vec<Command<'a>>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let word = *body.get(i).ok_or(Error::Corrupt("ebd body ends without END"))?;
        if word == END {
            if i + 1 != body.len() {
                return Err(Error::Corrupt("ebd words after END"));
            }
            return Ok(out);
        }
        if !matches!(Word::classify(word), Word::Command { .. }) {
            return Err(Error::Corrupt("ebd operand where a command was expected"));
        }
        let count = match table.get(word).ok_or(Error::Corrupt("ebd command not in the operand table"))? {
            Some(n) => n as usize,
            None => body[i + 1..].iter().position(|&w| is_command_word(w)).unwrap_or(body.len() - i - 1),
        };
        let operands = body.get(i + 1..i + 1 + count).ok_or(Error::Corrupt("ebd operands past the end"))?;
        if !body.get(i + 1 + count).is_some_and(|&w| is_command_word(w)) {
            return Err(Error::Corrupt("ebd operand count does not reach the next command"));
        }
        out.push(Command { word, operands });
        i += 1 + count;
    }
}

#[cfg(test)]
mod shallie_tests {
    use super::*;

    #[test]
    fn shallie_words_become_eschas() {
        assert_eq!(shallie_to_escha(0x7100_0037), 0x7100_0036, "q55, the point light, is Escha's q54");
        assert_eq!(shallie_to_escha(0x7100_0006), 0x7100_0006);
        assert_eq!(shallie_to_escha(0x7100_001a), 0x71ee_001a, "q26, not matched, is Shallie's own");
        assert_eq!(shallie_to_escha(0x7500_0022), 0x7500_0022, "u keeps its numbers");
        // A body translated: the command word changes, its operands stay, the count carries.
        let table = OperandTable::new(&[(0x7100_0037, Some(2))]);
        let e = Ebd { actions: vec![Action { flags: 0, name: vec![], description: vec![], words: vec![0x7100_0037, 5, 6, END] }], resources: vec![] };
        let (t, table) = e.translate(&table, shallie_to_escha).unwrap();
        assert_eq!(t.actions[0].words, vec![0x7100_0036, 5, 6, END]);
        assert_eq!(table.get(0x7100_0036), Some(Some(2)));
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    #[test]
    fn fixed_and_varying() {
        let table = OperandTable::new(&[(0x7700_0001, Some(1)), (0x7300_0012, None), (0x7400_001d, Some(0))]);
        let body = [0x7700_0001, 1, 0x7300_0012, 5, 6, 7, 0x7400_001d, 0x7700_0001, 2, END];
        let cmds = decode(&body, &table).unwrap();
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[1], Command { word: 0x7300_0012, operands: &[5, 6, 7] });
        assert_eq!(cmds[2].operands, &[] as &[u32]);
        let measured = OperandTable::measure([&body[..], &[0x7300_0012, 1, END][..]]);
        assert_eq!(measured.get(0x7700_0001), Some(Some(1)));
        assert_eq!(measured.get(0x7300_0012), Some(None));
        assert_eq!(cmds[3].operands, &[2]);
    }

    #[test]
    fn wrong_count_is_an_error() {
        let table = OperandTable::new(&[(0x7700_0001, Some(2))]);
        assert!(decode(&[0x7700_0001, 1, END], &table).is_err());
        assert!(decode(&[0x7300_0004, END], &table).is_err());
    }
}
