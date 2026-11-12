//! The interface layouts compiled into the executable (the `WinXls` sheets ship no data
//! files): what the narration cards (`u55`) and other screens are made of (`docs/formats.md`,
//! "Interface layouts"). Addresses are the English executable's.
//!
//! ```text
//! layout id ≥ 0x165:  LAYOUTS[id - 0x165]      = {u64 nodes, i32 x, i32 y}          16 bytes
//! node:               {i32 part, i32 cut, i32 x, i32 y, i32 kind, ...}             0x28 bytes,
//!                     the list ending with part -1; x, y from the layout's place
//! part < 0x164:       SPRITES[part]            = u64 cuts
//! cut:                {i32 texture, u16 x0, u16 y0, u16 x1, u16 y1, i32, i32}     20 bytes
//! texture ≥ 583:      TEXTURES[texture - 583]  = u64 name ("a15_….g1t" in WinXls_<lang>)
//! ```
//!
//! Texture ids below 583 (other sheets) and node parts from 0x164 on (nested layouts, text)
//! are not read yet.

use crate::pe::Pe;

/// `u55`'s card → layout id table (51 entries).
pub const TELOP_CARDS_EN: u64 = 0x1_40d8_5ce0;
pub const LAYOUTS_EN: u64 = 0x1_40dd_9080;
pub const SPRITES_EN: u64 = 0x1_40db_9f70;
pub const TEXTURES_EN: u64 = 0x1_40db_5c50;
/// The first layout id in [`LAYOUTS_EN`], and the first texture id in [`TEXTURES_EN`].
pub const FIRST_LAYOUT: u32 = 0x165;
pub const FIRST_TEXTURE: u32 = 583;
const TEXTURE_COUNT: usize = 303;

/// One textured rectangle of a layout: `texture` names a `WinXls` file, `src` is (left, top,
/// right, bottom) in its pixels, `at` the top-left on the 1920 × 1080 screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub texture: String,
    pub src: [u16; 4],
    pub at: [i32; 2],
}

fn i32_at(pe: &Pe, va: u64) -> Option<i32> {
    let o = pe.offset(va)?;
    Some(i32::from_le_bytes(pe.data.get(o..o + 4)?.try_into().ok()?))
}

fn u64_at(pe: &Pe, va: u64) -> Option<u64> {
    let o = pe.offset(va)?;
    Some(u64::from_le_bytes(pe.data.get(o..o + 8)?.try_into().ok()?))
}

fn text_at(pe: &Pe, va: u64) -> Option<String> {
    let o = pe.offset(va)?;
    let end = pe.data.get(o..)?.iter().take(96).position(|&b| b == 0)?;
    String::from_utf8(pe.data[o..o + end].to_vec()).ok()
}

/// The `WinXls` file of texture id `id`.
pub fn texture_name(pe: &Pe, id: i32) -> Option<String> {
    let k = usize::try_from(id.checked_sub(FIRST_TEXTURE as i32)?).ok().filter(|&k| k < TEXTURE_COUNT)?;
    text_at(pe, u64_at(pe, TEXTURES_EN + 8 * k as u64)?)
}

/// The layout id `u55 card` shows.
pub fn telop_layout(pe: &Pe, card: u32) -> Option<u32> {
    (card <= 50).then(|| i32_at(pe, TELOP_CARDS_EN + 4 * card as u64)).flatten().and_then(|v| u32::try_from(v).ok())
}

/// The sprite pieces of layout `id`, placed on the screen.
pub fn pieces(pe: &Pe, id: u32) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(k) = id.checked_sub(FIRST_LAYOUT) else { return out };
    let rec = LAYOUTS_EN + 16 * k as u64;
    let (Some(mut node), Some(x), Some(y)) = (u64_at(pe, rec), i32_at(pe, rec + 8), i32_at(pe, rec + 12)) else { return out };
    for _ in 0..256 {
        let Some(part) = i32_at(pe, node) else { break };
        if part < 0 {
            break;
        }
        let (cut, nx, ny) = (i32_at(pe, node + 4).unwrap_or(0).max(0), i32_at(pe, node + 8).unwrap_or(0), i32_at(pe, node + 12).unwrap_or(0));
        if part < 0x164 {
            let piece = (|| {
                let cuts = u64_at(pe, SPRITES_EN + 8 * part as u64)?;
                let c = cuts + 20 * cut as u64;
                let texture = texture_name(pe, i32_at(pe, c)?)?;
                let o = pe.offset(c + 4)?;
                let w = |i: usize| pe.data.get(o + 2 * i..o + 2 * i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
                Some(Piece { texture, src: [w(0)?, w(1)?, w(2)?, w(3)?], at: [x + nx, y + ny] })
            })();
            out.extend(piece);
        }
        node += 0x28;
    }
    out
}
