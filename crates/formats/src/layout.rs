//! The interface layouts compiled into the executable (the `WinXls` sheets ship no data
//! files): what the narration cards (`u55`) and other screens are made of (`docs/formats.md`,
//! "Interface layouts"). Addresses are the English executable's.
//!
//! ```text
//! layout id ≥ 0x165:  LAYOUTS[id - 0x165]      = {u64 nodes, i32 x, i32 y}          16 bytes
//! node:               {i32 part, i32 cut, i32 x, i32 y, i32 kind, ...}             0x28 bytes,
//!                     the list ending with part -1; x, y from the layout's place
//! part < 0x164:       SPRITES[part]            = u64 cuts
//! cut:               {i32 texture, u16 x0, u16 y0, u16 x1, u16 y1, i32, i32}     20 bytes
//! texture:           TEXTURES[texture]        = {u64 dds name, i32 index, i32 file}  16 bytes
//! file:              FILES[file]              = u64 name ("a15_….g1t" in WinXls_<lang>)
//! ```
//!
//! A texture is one picture of a `WinXls` pack: `index` counts in the pack's own list (the
//! lookup is `0x1403740a0`). Node parts from 0x164 on (nested layouts, text) are not read yet.

use crate::pe::Pe;

/// `u55`'s card → layout id table (51 entries).
pub const TELOP_CARDS_EN: u64 = 0x1_40d8_5ce0;
pub const LAYOUTS_EN: u64 = 0x1_40dd_9080;
pub const SPRITES_EN: u64 = 0x1_40db_9f70;
pub const TEXTURES_EN: u64 = 0x1_40db_6800;
pub const FILES_EN: u64 = 0x1_40db_5c50;
/// The first layout id in [`LAYOUTS_EN`].
pub const FIRST_LAYOUT: u32 = 0x165;
/// Textures in [`TEXTURES_EN`] (0 `a15_menu_parts01` to 886 `a15_title64_parts01`) and files
/// in [`FILES_EN`].
const TEXTURE_COUNT: usize = 887;
const FILE_COUNT: usize = 303;

/// One textured rectangle of a layout: `texture` names a `WinXls` file and `index` the picture
/// in it, `src` is (left, top, right, bottom) in its pixels, `at` the top-left on the 1920 ×
/// 1080 screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub texture: String,
    pub index: u32,
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

/// Texture `id`: its `WinXls` file and its place in the file's list of pictures.
pub fn texture(pe: &Pe, id: i32) -> Option<(String, u32)> {
    let k = usize::try_from(id).ok().filter(|&k| k < TEXTURE_COUNT)?;
    let rec = TEXTURES_EN + 16 * k as u64;
    let index = u32::try_from(i32_at(pe, rec + 8)?).ok()?;
    let file = usize::try_from(i32_at(pe, rec + 12)?).ok().filter(|&f| f < FILE_COUNT)?;
    Some((text_at(pe, u64_at(pe, FILES_EN + 8 * file as u64)?)?, index))
}

/// The layout id `u55 card` shows.
pub fn telop_layout(pe: &Pe, card: u32) -> Option<u32> {
    (card <= 50).then(|| i32_at(pe, TELOP_CARDS_EN + 4 * card as u64)).flatten().and_then(|v| u32::try_from(v).ok())
}

/// One node of a layout as stored: `part` below 0x164 is a sprite sheet (`cut` its rectangle),
/// from 0x164 on another kind (nested layouts, text: not read yet); `at` is from the layout's
/// place; `kind` picks the widget that draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub part: i32,
    pub cut: i32,
    pub at: [i32; 2],
    pub kind: i32,
}

/// Layout `id`'s place on the 1920 × 1080 screen and its nodes, or `None` past the table.
pub fn nodes(pe: &Pe, id: u32) -> Option<([i32; 2], Vec<Node>)> {
    let k = id.checked_sub(FIRST_LAYOUT)?;
    let rec = LAYOUTS_EN + 16 * k as u64;
    let (mut node, x, y) = (u64_at(pe, rec)?, i32_at(pe, rec + 8)?, i32_at(pe, rec + 12)?);
    pe.offset(node)?;
    let mut out = Vec::new();
    for _ in 0..256 {
        let part = i32_at(pe, node)?;
        if part < 0 {
            break;
        }
        let f = |i: u64| i32_at(pe, node + 4 * i).unwrap_or(0);
        out.push(Node { part, cut: f(1), at: [f(2), f(3)], kind: f(4) });
        node += 0x28;
    }
    Some(([x, y], out))
}

/// The sprite pieces of layout `id`, placed on the screen, with those of the layouts it nests
/// (a node part above 0x164 is that layout, placed at the node; taken from the node counts, the
/// drawing code is not read).
pub fn pieces(pe: &Pe, id: u32) -> Vec<Piece> {
    let mut out = Vec::new();
    add_pieces(pe, id, [0, 0], 0, &mut out);
    out
}

fn add_pieces(pe: &Pe, id: u32, origin: [i32; 2], depth: u32, out: &mut Vec<Piece>) {
    let Some(([x, y], nodes)) = nodes(pe, id) else { return };
    let (x, y) = (origin[0] + x, origin[1] + y);
    for n in nodes {
        let at = [x + n.at[0], y + n.at[1]];
        if n.part < 0x164 {
            let piece = (|| {
                let cuts = u64_at(pe, SPRITES_EN + 8 * n.part as u64)?;
                let c = cuts + 20 * n.cut.max(0) as u64;
                let (texture, index) = texture(pe, i32_at(pe, c)?)?;
                let o = pe.offset(c + 4)?;
                let w = |i: usize| pe.data.get(o + 2 * i..o + 2 * i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
                Some(Piece { texture, index, src: [w(0)?, w(1)?, w(2)?, w(3)?], at })
            })();
            out.extend(piece);
        } else if n.part > 0x164 && depth < 8 && n.part as u32 != id {
            add_pieces(pe, n.part as u32, at, depth + 1, out);
        }
    }
}

/// The event still pictures (`u1/0x30`, `IpuGallery`): 32-byte records `{i32 width, height, f32
/// scale x, scale y, u64 folder ("Ipu/"), u64 file}` from `0x1408f3d00` in the English
/// executable, indexed by the command's number (`0x140299209`: up to 0xfe). Pictures 0 and 1
/// have no file (nothing shown); 2 is `bg_black` (64 × 64 scaled 32 × 18 over the screen), 3
/// `bg_white`, then logos, title screens and the `ev_still_*` event pictures.
pub const STILLS_EN: u64 = 0x1_408f_3d00;

/// One still: its file in `Data/x64/<folder>` and its size on the 1920 × 1080 screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Still {
    pub folder: String,
    pub file: String,
    pub size: [f32; 2],
}

/// Still `n`, if it shows a picture.
pub fn still(pe: &Pe, n: u32) -> Option<Still> {
    if n > 0xfe {
        return None;
    }
    let rec = STILLS_EN + 32 * u64::from(n);
    let o = pe.offset(rec)?;
    let f = |i: usize| pe.data.get(o + 4 * i..o + 4 * i + 4).map(|b| [b[0], b[1], b[2], b[3]]);
    let (w, h) = (i32::from_le_bytes(f(0)?), i32::from_le_bytes(f(1)?));
    let (sx, sy) = (f32::from_le_bytes(f(2)?), f32::from_le_bytes(f(3)?));
    let folder = text_at(pe, u64_at(pe, rec + 16)?)?;
    let file = text_at(pe, u64_at(pe, rec + 24)?)?;
    Some(Still { folder: folder.trim_end_matches('/').to_string(), file, size: [w as f32 * sx, h as f32 * sy] })
}

/// The names the message window shows (an `ebm` line's second field indexes it): 40-byte records
/// `{u64 name, i32 3, i32 -1, u32 0, f32 × 4 (255)}` from `0x1408117a0` in the English
/// executable: the cast (0 Escha to 17 Flameu, as `Chara.xml`), then names for those not
/// introduced or not named (19 "Gloomy Man" ... 59 "???", 60 "!!!", 62 "Everyone" ...).
pub const SPEAKER_NAMES_EN: u64 = 0x1_4081_17a0;

pub fn speaker_names(pe: &Pe) -> Vec<String> {
    let mut out = Vec::new();
    for k in 0..4096u64 {
        let Some(name) = u64_at(pe, SPEAKER_NAMES_EN + 40 * k).and_then(|p| text_at(pe, p)) else { break };
        out.push(name);
    }
    out
}
