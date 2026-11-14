//! The music list compiled into Escha & Logy DX's executable: one 0x38-byte record per track,
//! numbered from 0 (the numbers `ECT_SOUND_BG_PLAY` names).
//!
//! ```text
//! +0x00  pointer to the PC file ("DISC:Sound/BGM/ev_usual01.kvs")
//! +0x08  pointer to the PlayStation file (".at9"), +0x10 the Vita one (".kno")
//! +0x18  two zero pointers
//! +0x28  u32 0, f32 the volume (0.5 for most event music, 0.7 to 1.0 for the rest)
//! +0x30  pointer to the track's Japanese title ("日常1")
//! ```

use barrel_formats::pe::Pe;

/// Where the list is in the English Escha executable.
pub const TRACKS_EN: u64 = 0x1_407e_5ef0;
const RECORD: usize = 0x38;

pub use barrel_audio::Track;

/// The tracks from `table` on, until a record no longer names a `.kvs`.
pub fn tracks(pe: &Pe, table: u64) -> Option<Vec<Track>> {
    let d = pe.data;
    let at = pe.offset(table)?;
    let text = |va: u64| -> Option<String> {
        let o = pe.offset(va)?;
        let end = d[o..].iter().position(|&b| b == 0)?;
        String::from_utf8(d[o..o + end].to_vec()).ok()
    };
    let u64_at = |o: usize| d.get(o..o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
    let mut out = Vec::new();
    for k in 0.. {
        let r = at + k * RECORD;
        let Some(path) = u64_at(r).and_then(text) else { break };
        let Some(file) = path.strip_prefix("DISC:Sound/BGM/").filter(|f| f.ends_with(".kvs")) else { break };
        let volume = f32::from_le_bytes(d.get(r + 0x2c..r + 0x30)?.try_into().unwrap());
        let title = u64_at(r + 0x30).and_then(text).unwrap_or_default();
        out.push(Track { file: file.to_string(), volume, title });
    }
    (!out.is_empty()).then_some(out)
}
