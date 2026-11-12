//! The game's font: one atlas texture (`Data/x64/Font_EN/MainFont_x64_0.g1t`, 2048×2048 BC3,
//! the glyphs in its alpha) and a glyph table in the executable.
//!
//! ```text
//! descriptor (just before the file name): ..., u64 pointer to the name, u64 glyph count
//! the name ("MainFont_x64_0.g1t", 0x18 bytes) and then the glyphs, 28 bytes each:
//!   u32 code, u16 x, u16 y, u16 width, u16 height, i32 x offset, i32 y offset, u32 advance,
//!   u32 0
//! ```
//!
//! The codes rise: 0 to 126 are ASCII by number (0 to 31 are 3×3 blanks), the rest the
//! character's UTF-8 bytes read as a big-endian number (`0xc2a2` is ¢). The y offset is from
//! the top of the line to the top of the glyph.

use crate::pe::Pe;

/// The English font's file name in the English executable (its glyphs follow it).
pub const FONT_EN: u64 = 0x1407_9ee58;

const RECORD: usize = 28;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub x_offset: i32,
    pub y_offset: i32,
    pub advance: u32,
}

/// The glyphs by character.
pub fn glyphs(pe: &Pe, name: u64) -> Option<std::collections::HashMap<char, Glyph>> {
    let d = pe.data;
    let at = pe.offset(name)?;
    let u64_at = |o: usize| d.get(o..o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
    // The descriptor's pointer to the name, then the count.
    if u64_at(at.checked_sub(16)?)? != name {
        return None;
    }
    let count = usize::try_from(u64_at(at - 8)?).ok().filter(|&n| n <= 0x10000)?;
    let u16_at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    // The whole table is there.
    d.get(at + 0x18..at + 0x18 + count * RECORD)?;
    let mut out = std::collections::HashMap::with_capacity(count);
    for i in 0..count {
        let r = at + 0x18 + i * RECORD;
        let code = u32_at(r);
        let Some(c) = character(code) else { continue };
        out.insert(c, Glyph {
            x: u16_at(r + 4),
            y: u16_at(r + 6),
            width: u16_at(r + 8),
            height: u16_at(r + 10),
            x_offset: u32_at(r + 12) as i32,
            y_offset: u32_at(r + 16) as i32,
            advance: u32_at(r + 20),
        });
    }
    (!out.is_empty()).then_some(out)
}

/// A glyph code's character: ASCII below 0x80, otherwise its UTF-8 bytes as a number.
pub fn character(code: u32) -> Option<char> {
    if code < 0x80 {
        return char::from_u32(code);
    }
    let bytes = code.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0)?;
    std::str::from_utf8(&bytes[start..]).ok()?.chars().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(character(0x41), Some('A'));
        assert_eq!(character(0xc2a2), Some('¢'));
        assert_eq!(character(0xe280a6), Some('…'));
        assert_eq!(character(0xefbd9e), Some('～'));
        assert_eq!(character(0xff), None);
    }
}
