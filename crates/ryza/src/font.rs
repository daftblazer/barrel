//! Ryza's text font: the glyph table of `MainFont01_HD1080_0.g1t`, kept in the executable.
//!
//! The atlas (`data/x64/res_<lang>/font/mainfont01_hd1080_0.g1t`, 4096 x 2048 BC3) holds the
//! glyphs white in its alpha. Where each one is comes from a table in the executable, 28 bytes
//! a record in rising code order, the layout Escha's font table has too:
//!
//! ```text
//! u32 code, u16 x, u16 y, u16 width, u16 height, i32 x offset, i32 y offset, u32 advance, u32 0
//! ```
//!
//! Codes below 0x80 are ASCII; the rest the character's UTF-8 bytes read as a big-endian number.
//! The y offset runs from the line's top to the glyph's top. Unlike Escha's tables this one has
//! no descriptor with a file name before it: it starts at a fixed place (the table of
//! `MainFont00` follows it elsewhere, 2,820 glyphs against this one's 1,774).

use std::collections::HashMap;

use barrel_formats::pe::Pe;
use barrel_render::text::Glyph;

/// The `MainFont01_HD1080` table's first record in the Steam executable (code 0, a 4 x 3
/// blank), found by matching a captured glyph (`V` at 2129, 523 in the atlas) to its record.
pub const MAIN_FONT_01_HD1080: u64 = 0x1_4130_9320;
/// How many glyphs the table holds (the run of records that rise in code and end in a 0 word).
pub const MAIN_FONT_01_HD1080_COUNT: usize = 1774;
// STAND-IN: The atlas size (ATLAS_SIZE) is fitted to one captured glyph; the real thing is the font's own size from the game's text code.
/// The font's size in the atlas: a glyph set at layout size `s` on a screen k times the 720-line
/// layout is drawn at `s * k / ATLAS_SIZE` of its atlas pixels. Fitted to the capture: the
/// title's size-11 version string at 1080 lines draws its 29-pixel `V` 13.29 pixels wide
/// (11 * 1.5 / 36).
pub const ATLAS_SIZE: f32 = 36.0;

const RECORD: usize = 28;

/// The `MainFont00_HD1080` table's first record: its 1,774 records end 8 bytes before the
/// `MainFont01` table's (the load screen's heading and figures are set in it; found by matching
/// the capture's glyphs to the records, as for the other).
pub const MAIN_FONT_00_HD1080: u64 = MAIN_FONT_01_HD1080 - 0xC210;

/// The glyphs by character, from the executable at `exe`'s bytes.
pub fn glyphs(exe: &[u8]) -> Option<HashMap<char, Glyph>> {
    glyphs_at(exe, MAIN_FONT_01_HD1080)
}

/// The glyphs of the table whose first record is at virtual address `va`.
pub fn glyphs_at(exe: &[u8], va: u64) -> Option<HashMap<char, Glyph>> {
    let pe = Pe::parse(exe).ok()?;
    let at = pe.offset(va)?;
    let table = exe.get(at..at + MAIN_FONT_01_HD1080_COUNT * RECORD)?;
    let u16_at = |r: &[u8], o: usize| u16::from_le_bytes([r[o], r[o + 1]]);
    let u32_at = |r: &[u8], o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    let mut out = HashMap::with_capacity(table.len() / RECORD);
    for r in table.chunks_exact(RECORD) {
        let Some(c) = character(u32_at(r, 0)) else { continue };
        out.insert(
            c,
            Glyph {
                x: u16_at(r, 4),
                y: u16_at(r, 6),
                width: u16_at(r, 8),
                height: u16_at(r, 10),
                x_offset: u32_at(r, 12) as i32,
                y_offset: u32_at(r, 16) as i32,
                advance: u32_at(r, 20),
            },
        );
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
        assert_eq!(character(0x56), Some('V'));
        assert_eq!(character(0xc2a2), Some('¢'));
        assert_eq!(character(0xff), None);
    }

    /// The installed executable's table (`BARREL_RYZA_EXE` names it; not in the repository):
    /// `V` is where the capture's glyph is (2129, 523, 29 x 34, drawn 1 left of the pen and 12
    /// down, advancing 28).
    #[test]
    fn the_installs_table() {
        let Some(p) = std::env::var_os("BARREL_RYZA_EXE") else { return };
        let g = glyphs(&barrel_files::read(p).unwrap()).unwrap();
        assert_eq!(g[&'V'], Glyph { x: 2129, y: 523, width: 29, height: 34, x_offset: -1, y_offset: 12, advance: 28 });
        assert_eq!(g[&' '].advance, 12);
    }
}
