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

impl Ebd {
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
