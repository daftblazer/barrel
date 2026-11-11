//! Event messages (`EVENT_MESSAGE_*.ebm`): the dialogue lines of an event, one file per
//! event beside its `.ebd`, with a copy per language under `Event_EN`, `Event_SC` and
//! `Event_TC`.
//!
//! ```text
//! u32 message count
//! per message: u32 kind (1 for most lines; 2, 4 and 0 also occur)
//!              i32 speaker (a character number; -1 for system text)
//!              i32 ×6 (portrait, expression and placement values, not decoded yet)
//!              u32 length, then that many bytes of UTF-8 text ending in 0
//! ```
//!
//! The text uses `<CR>` for line breaks, `<IMnn>` for inline icons and `<CLGR>` / `<CLNR>`
//! to change and reset the colour.
//!
//! Some lines in each translated language were left in Japanese, and their text was mangled
//! on the way: the Shift-JIS bytes were read two at a time as UTF-16 (エ, `83 47`, became
//! U+4783; `<CR>` became U+433C U+3E52), then written as UTF-8. [`Message::repaired_text`]
//! undoes it; characters whose pair fell on a UTF-16 surrogate were lost and show as 〓.

use crate::{Error, u32_at};

#[derive(Clone, Debug)]
pub struct Message {
    pub kind: u32,
    pub speaker: i32,
    pub fields: [i32; 6],
    /// The text as stored.
    pub text: String,
}

impl Message {
    /// The text, with mangled Shift-JIS lines turned back into Japanese.
    pub fn repaired_text(&self) -> String {
        unpack_sjis(&self.text).unwrap_or_else(|| self.text.clone())
    }
}

/// Undo "Shift-JIS bytes read two at a time as UTF-16": None unless `text` looks like that.
///
/// Every code point stands for two bytes (low, high), ASCII pairs included (`<CR>` became
/// U+433C U+3E52). Pairs that landed on UTF-16 surrogates became U+FFFD and are lost; they
/// come back as 〓. To keep real text safe,
/// a line only counts as mangled if it has no kana, at least half its characters are CJK,
/// Hangul or private-use code points, and the bytes decode as Shift-JIS to mostly Japanese.
fn unpack_sjis(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    let kana = |c: char| ('\u{3040}'..='\u{30ff}').contains(&c);
    let wide = |c: char| (c as u32) >= 0x3400;
    if chars.is_empty() || chars.iter().any(|&c| kana(c)) || chars.iter().filter(|&&c| wide(c)).count() * 2 < chars.len() {
        return None;
    }
    // Each code point is a pair whose first byte (the low one) starts a Shift-JIS character
    // or is printable ASCII paired with printable ASCII. Real kanji almost never fit.
    let pair_ok = |c: char| {
        let c = c as u32;
        let (lo, hi) = ((c & 0xff) as u8, (c >> 8) as u8);
        c < 0x80 || c == 0xfffd || matches!(lo, 0x81..=0x9f | 0xe0..=0xfc) || ((0x20..0x7f).contains(&lo) && (0x20..0x7f).contains(&hi))
    };
    if !text.chars().all(|c| c.is_whitespace() || pair_ok(c)) {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len());
    for c in text.chars() {
        let c = c as u32;
        if c == 0xfffd || c > 0xffff {
            // A pair that fell in the UTF-16 surrogate range could not be written as UTF-8
            // and became U+FFFD: the character is lost. 〓 (geta) marks the gap.
            bytes.extend_from_slice(&[0x81, 0xac]);
            continue;
        }
        bytes.push(c as u8);
        if c >> 8 != 0 {
            bytes.push((c >> 8) as u8);
        }
    }
    let (s, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
    let out: Vec<char> = s.chars().filter(|c| !c.is_whitespace() && !c.is_ascii()).collect();
    let japanese = out.iter().filter(|&&c| kana(c) || ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3000}'..='\u{303f}').contains(&c) || ('\u{ff00}'..='\u{ffef}').contains(&c)).count();
    if s.chars().any(|c| c.is_control() && c != '\n') || japanese * 2 < out.len() {
        return None;
    }
    Some(s.into_owned())
}

pub fn parse(data: &[u8]) -> Result<Vec<Message>, Error> {
    let count = u32_at(data, 0)? as usize;
    if count > data.len() / 36 {
        return Err(Error::Corrupt("ebm message count"));
    }
    let mut out = Vec::with_capacity(count);
    let mut p = 4;
    for _ in 0..count {
        let word = |k: usize| u32_at(data, p + 4 * k);
        let mut fields = [0i32; 6];
        for (i, f) in fields.iter_mut().enumerate() {
            *f = word(2 + i)? as i32;
        }
        let len = word(8)? as usize;
        let raw = data.get(p + 36..p + 36 + len).ok_or(Error::Corrupt("ebm text past the end"))?;
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let text = std::str::from_utf8(&raw[..end]).map_err(|_| Error::Corrupt("ebm text is not UTF-8"))?.to_owned();
        out.push(Message { kind: word(0)?, speaker: word(1)? as i32, fields, text });
        p += 36 + len;
    }
    if p != data.len() {
        return Err(Error::Corrupt("ebm has data after its messages"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_and_repair() {
        let mut f = 2u32.to_le_bytes().to_vec();
        for (kind, speaker, text) in [(1u32, 3i32, "Hi<CR>there"), (2, -1, "\u{4783}\u{5883}")] {
            for w in [kind as i32, speaker, 0, 3, 0, -1, 2, 0] {
                f.extend_from_slice(&w.to_le_bytes());
            }
            f.extend_from_slice(&(text.len() as u32 + 1).to_le_bytes());
            f.extend_from_slice(text.as_bytes());
            f.push(0);
        }
        let m = parse(&f).unwrap();
        assert_eq!(m[0].text, "Hi<CR>there");
        assert_eq!(m[0].repaired_text(), "Hi<CR>there");
        assert_eq!(m[1].speaker, -1);
        assert_eq!(m[1].repaired_text(), "エス");
        assert_eq!(unpack_sjis("あった"), None);
        assert_eq!(unpack_sjis("It\u{2019}s fine"), None);
        assert_eq!(unpack_sjis("【削除】"), None);
        assert_eq!(unpack_sjis("宝箱？"), None);
        assert_eq!(unpack_sjis("\u{4783}\u{433c}\u{3e52}\u{5883}"), Some("エ<CR>ス".into()));
    }
}
