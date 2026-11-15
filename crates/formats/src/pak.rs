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

use crate::game::Game;
use crate::{Error, u32_at};

const ENTRY: usize = 184;
const MAX_ENTRIES: usize = 1 << 20;

/// Atelier Sophie 2's fixed key.
pub const SOPHIE2_KEY: [u8; 32] = *b"dGGKXLHLuCJwv8aBc3YQX6X6sREVPchs";

/// Each game's fixed key. They are text; each shows in plain sight wherever a name's padding
/// is zero, since only the two keys are left there.
pub const KEYS: [(Game, [u8; 32]); 4] = [
    (Game::Sophie2, SOPHIE2_KEY),
    (Game::RyzaDx, *b"EGkUyYzPidcRx5yTecqk5AYJeSzzc5BR"),
    (Game::Ryza2Dx, *b"ivZBFzn82RVuYnmgXfeFEgk33ALZcZDP"),
    (Game::Ryza3Dx, *b"fyrixtT9AhA4v0cFahgMcgVwxFrry42A"),
];

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

    /// Read the entry table with whichever game's key makes every name a path (each starts
    /// with a backslash and is printable ASCII).
    pub fn parse_any(table: &[u8]) -> Result<(Pak, Game), Error> {
        for (game, key) in &KEYS {
            if let Ok(pak) = Pak::parse(table, key) {
                let paths = pak.entries.iter().all(|e| {
                    e.name.starts_with('\\') && e.name.bytes().all(|b| b.is_ascii_graphic())
                });
                if paths {
                    return Ok((pak, *game));
                }
            }
        }
        Err(Error::Corrupt("no known game's key gives readable names"))
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

/// Fills `buf` from `offset` without moving the file's position (`pread`; on Windows a
/// `seek_read` loop, which moves it: the archive's one reader is not shared across threads there).
fn read_exact_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_exact_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut done = 0;
        while done < buf.len() {
            match file.seek_read(&mut buf[done..], offset + done as u64)? {
                0 => return Err(std::io::ErrorKind::UnexpectedEof.into()),
                n => done += n,
            }
        }
        Ok(())
    }
}

/// An archive file opened for reading: its table, and its entries read from it on demand
/// (positional reads: no seeking, so a shared reference reads).
pub struct Archive {
    file: std::fs::File,
    pub pak: Pak,
    pub game: Game,
}

impl Archive {
    pub fn open(path: &std::path::Path) -> Result<Archive, String> {
        use std::io::Read;
        let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        let mut file = barrel_files::open(path).map_err(|e| err(&e))?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header).map_err(|e| err(&e))?;
        let size = Pak::table_size(&header).map_err(|e| err(&e))?;
        let mut table = header.to_vec();
        table.resize(size, 0);
        file.read_exact(&mut table[16..]).map_err(|e| err(&e))?;
        let (pak, game) = Pak::parse_any(&table).map_err(|e| err(&e))?;
        Ok(Archive { file, pak, game })
    }

    /// An entry's data, unscrambled.
    pub fn read(&self, entry: &Entry) -> std::io::Result<Vec<u8>> {
        let mut data = vec![0u8; entry.size as usize];
        read_exact_at(&self.file, &mut data, entry.offset)?;
        self.pak.unscramble(entry, 0, &mut data);
        Ok(data)
    }
}

/// A game's archives (`Data/*.PAK`), their files found by name in any case (`\data\x64\...`).
/// A name in more than one archive is taken from the last in name order (Ryza DX's
/// `PACK03.PAK` repeats two of `PACK00_02.PAK`'s, as an update would).
pub struct Archives {
    archives: Vec<Archive>,
    index: std::collections::HashMap<String, (usize, usize)>,
}

impl Archives {
    /// Every `*.PAK` in `data` (an install's `Data` folder), through the layered files.
    pub fn open_dir(data: &std::path::Path) -> Result<Archives, String> {
        // The names in the layers' `Data` folders: a mod's archive of another name is one more.
        let mut paths: Vec<_> = barrel_files::list(data)
            .map_err(|e| format!("{}: {e}", data.display()))?
            .into_iter()
            .map(|n| data.join(n))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pak")))
            .collect();
        paths.sort();
        if paths.is_empty() {
            return Err(format!("{}: no PAK archives", data.display()));
        }
        let archives = paths.iter().map(|p| Archive::open(p)).collect::<Result<Vec<_>, _>>()?;
        let mut index = std::collections::HashMap::new();
        for (a, archive) in archives.iter().enumerate() {
            for (e, entry) in archive.pak.entries.iter().enumerate() {
                index.insert(entry.name.to_ascii_lowercase(), (a, e));
            }
        }
        Ok(Archives { archives, index })
    }

    /// The game whose key the archives open with.
    pub fn game(&self) -> Game {
        self.archives[0].game
    }

    /// Every file's name, lower case.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(String::as_str)
    }

    /// A file by name (any case); `None` when no archive has it.
    pub fn read(&self, name: &str) -> Option<std::io::Result<Vec<u8>>> {
        let &(a, e) = self.index.get(&name.to_ascii_lowercase())?;
        let archive = &self.archives[a];
        Some(archive.read(&archive.pak.entries[e]))
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
        assert_eq!(Pak::parse_any(&f).unwrap().1, Game::Sophie2);
        let entry = &pak.entries[0];
        assert_eq!((entry.name.as_str(), entry.size, entry.offset), ("\\data\\x64\\a.g1t", 8, at));
        let mut data = f[at as usize + 4..].to_vec();
        pak.unscramble(entry, 4, &mut data);
        assert_eq!(data, b"0600");
    }
}
