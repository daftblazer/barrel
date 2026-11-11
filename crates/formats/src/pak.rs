//! PAK archives: Atelier Sophie 2 and the Ryza DX trilogy keep their data in `Data/*.PAK`
//! (see `docs/survey.md`, "PAK archives").
//!
//! ```text
//! header (16 bytes): 0x20000, entry count, 16, 13
//! entries (184 bytes each):
//!   +0    name[128]
//!   +128  u32 size
//!   +132  key[32]
//!   +164  u32 (not known yet)
//!   +168  u64 offset of the data, from the end of the entry table
//!   +176  u64 (not known yet)
//! ```
//!
//! An entry's name and data are both XORed with its own 32-byte key and with a fixed
//! 32-byte key for the game, each repeating every 32 bytes. Entries whose key is all zero
//! are stored plain.

use crate::{Error, u32_at};

const ENTRY: usize = 184;
const MAX_ENTRIES: usize = 1 << 20;

/// Atelier Sophie 2's fixed key.
pub const SOPHIE2_KEY: [u8; 32] = *b"dGGKXLHLuCJwv8aBc3YQX6X6sREVPchs";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The name, such as `\data\x64\res_cmn\ui\a23_item_l_139.g1t`.
    pub name: String,
    pub size: u32,
    /// Where the data starts in the archive file.
    pub offset: u64,
    key: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct Pak {
    pub entries: Vec<Entry>,
    game_key: [u8; 32],
}

impl Pak {
    /// The size of the header and entry table for `count` entries: read this much of the
    /// file to call [`Pak::parse`].
    pub fn table_size(header: &[u8]) -> Result<usize, Error> {
        if u32_at(header, 0)? != 0x20000 {
            return Err(Error::NotThisFormat);
        }
        let count = u32_at(header, 4)? as usize;
        if count > MAX_ENTRIES {
            return Err(Error::Corrupt("pak entry count"));
        }
        Ok(16 + ENTRY * count)
    }

    /// Read the entry table (`table` holds at least [`Pak::table_size`] bytes).
    pub fn parse(table: &[u8], game_key: &[u8; 32]) -> Result<Pak, Error> {
        let size = Pak::table_size(table)?;
        let count = (size - 16) / ENTRY;
        let base = size as u64;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let e = table.get(16 + ENTRY * i..16 + ENTRY * (i + 1)).ok_or(Error::Corrupt("pak table past the end"))?;
            let key: [u8; 32] = e[132..164].try_into().unwrap();
            let mut name = Vec::with_capacity(64);
            for (p, &b) in e[..128].iter().enumerate() {
                let c = b ^ key[p % 32] ^ game_key[p % 32];
                if c == 0 {
                    break;
                }
                name.push(c);
            }
            let name = String::from_utf8(name).map_err(|_| Error::Corrupt("pak name is not text"))?;
            let rel = u64::from_le_bytes(e[168..176].try_into().unwrap());
            let offset = base.checked_add(rel).ok_or(Error::Corrupt("pak offset"))?;
            entries.push(Entry { name, size: u32_at(e, 128)?, offset, key });
        }
        Ok(Pak { entries, game_key: *game_key })
    }

    /// Unscramble `data`, which was read from `entry.offset + start` onward.
    pub fn unscramble(&self, entry: &Entry, start: u64, data: &mut [u8]) {
        if entry.key == [0; 32] {
            return;
        }
        for (i, b) in data.iter_mut().enumerate() {
            let p = ((start + i as u64) % 32) as usize;
            *b ^= entry.key[p] ^ self.game_key[p];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_data() {
        let key = [7u8; 32];
        let name = b"\\data\\x64\\a.g1t";
        let body = b"GT1G0600";
        let mut f = Vec::new();
        for v in [0x20000u32, 1, 16, 13] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        let mut e = vec![0u8; ENTRY];
        for p in 0..128 {
            let c = name.get(p).copied().unwrap_or(0);
            e[p] = c ^ key[p % 32] ^ SOPHIE2_KEY[p % 32];
        }
        e[128..132].copy_from_slice(&(body.len() as u32).to_le_bytes());
        e[132..164].copy_from_slice(&key);
        f.extend_from_slice(&e);
        let at = f.len() as u64;
        f.extend(body.iter().enumerate().map(|(p, &c)| c ^ key[p] ^ SOPHIE2_KEY[p]));
        assert_eq!(Pak::table_size(&f).unwrap(), 16 + ENTRY);
        let pak = Pak::parse(&f, &SOPHIE2_KEY).unwrap();
        let entry = &pak.entries[0];
        assert_eq!((entry.name.as_str(), entry.size, entry.offset), ("\\data\\x64\\a.g1t", 8, at));
        let mut data = f[at as usize + 4..].to_vec();
        pak.unscramble(entry, 4, &mut data);
        assert_eq!(data, b"0600");
    }
}
