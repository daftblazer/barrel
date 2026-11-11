//! Gust's "elixir" containers (`*.elixir.gz`), which hold models, animations, textures,
//! effects and collision for characters, items, fields and effects.
//!
//! A file is either a container as-is (magic `CRAE`) or the same container compressed as a
//! run of zlib chunks, each `{u32 size, zlib stream}`, ended by a size of 0.
//!
//! The container is a 28-byte header followed by an entry table and the entries' data:
//!
//! | Offset | Field |
//! |---|---|
//! | 0 | magic `CRAE` |
//! | 4 | version: 0 (40-byte entries, 32-byte names) or 1 (56-byte entries, 48-byte names) |
//! | 8 | size of the data after the table |
//! | 12 | header size, 28 |
//! | 16 | table size |
//! | 20 | entry count |
//! | 24 | flags (10 in most files, 0 in some) |
//!
//! Each entry is `{u32 offset from the file start, u32 size, name}`. Entries of size 0 named
//! `dummy` are placeholders the game skips.

use std::borrow::Cow;

use crate::{Error, inflate, u32_at};

const MAGIC: &[u8; 4] = b"CRAE";
const HEADER_SIZE: usize = 28;
/// No container in the games decompresses to more than a few tens of MB.
pub const MAX_SIZE: usize = 1 << 30;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

impl Entry {
    pub fn is_dummy(&self) -> bool {
        self.size == 0
    }
}

pub struct Elixir<'a> {
    pub version: u32,
    pub flags: u32,
    pub entries: Vec<Entry>,
    data: Cow<'a, [u8]>,
    /// Whether the file was zlib-chunked.
    pub compressed: bool,
}

/// The container bytes of a file, decompressing a chunked file.
pub fn unwrap(file: &[u8]) -> Result<Cow<'_, [u8]>, Error> {
    if file.starts_with(MAGIC) {
        return Ok(Cow::Borrowed(file));
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 4 <= file.len() {
        let size = u32_at(file, pos)? as usize;
        pos += 4;
        if size == 0 {
            break;
        }
        let chunk = file.get(pos..pos + size).ok_or(Error::Corrupt("elixir chunk past the end"))?;
        let left = MAX_SIZE.saturating_sub(out.len());
        out.extend(inflate::zlib_decompress(chunk, left)?);
        pos += size;
    }
    if !out.starts_with(MAGIC) {
        return Err(Error::NotThisFormat);
    }
    Ok(Cow::Owned(out))
}

impl<'a> Elixir<'a> {
    pub fn parse(file: &'a [u8]) -> Result<Self, Error> {
        let compressed = !file.starts_with(MAGIC);
        let data = unwrap(file)?;
        let version = u32_at(&data, 4)?;
        let data_size = u32_at(&data, 8)? as usize;
        let header_size = u32_at(&data, 12)? as usize;
        let table_size = u32_at(&data, 16)? as usize;
        let count = u32_at(&data, 20)? as usize;
        let flags = u32_at(&data, 24)?;
        let entry_size = match version {
            0 => 40,
            1 => 56,
            _ => return Err(Error::Corrupt("unknown elixir version")),
        };
        if header_size != HEADER_SIZE {
            return Err(Error::Corrupt("elixir header size"));
        }
        if count.checked_mul(entry_size) != Some(table_size) {
            return Err(Error::Corrupt("elixir table size"));
        }
        if HEADER_SIZE + table_size + data_size != data.len() {
            return Err(Error::Corrupt("elixir sizes do not add up"));
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let at = HEADER_SIZE + i * entry_size;
            let offset = u32_at(&data, at)? as usize;
            let size = u32_at(&data, at + 4)? as usize;
            let raw = &data[at + 8..at + entry_size];
            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            let name = String::from_utf8_lossy(&raw[..end]).into_owned();
            if size != 0 && offset.checked_add(size).is_none_or(|e| e > data.len()) {
                return Err(Error::Corrupt("elixir entry past the end"));
            }
            entries.push(Entry { name, offset, size });
        }
        Ok(Elixir { version, flags, entries, data, compressed })
    }

    pub fn data(&self, entry: &Entry) -> &[u8] {
        if entry.is_dummy() {
            return &[];
        }
        &self.data[entry.offset..entry.offset + entry.size]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(version: u32, entries: &[(&str, &[u8])]) -> Vec<u8> {
        let entry_size = if version == 0 { 40 } else { 56 };
        let table = entries.len() * entry_size;
        let data_size: usize = entries.iter().map(|e| e.1.len()).sum();
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        for v in [version, data_size as u32, 28, table as u32, entries.len() as u32, 10] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let mut offset = 28 + table;
        for (name, body) in entries {
            out.extend_from_slice(&(offset as u32).to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            let mut raw = vec![0u8; entry_size - 8];
            raw[..name.len()].copy_from_slice(name.as_bytes());
            out.extend_from_slice(&raw);
            offset += body.len();
        }
        for (_, body) in entries {
            out.extend_from_slice(body);
        }
        out
    }

    #[test]
    fn both_versions() {
        for version in [0, 1] {
            let file = container(version, &[("a.g1m", b"_M1Gxx"), ("dummy", b""), ("b.g1t", b"GT1G")]);
            let e = Elixir::parse(&file).unwrap();
            assert_eq!(e.entries.len(), 3);
            assert_eq!(e.entries[0].name, "a.g1m");
            assert_eq!(e.data(&e.entries[0]), b"_M1Gxx");
            assert!(e.entries[1].is_dummy());
            assert_eq!(e.data(&e.entries[2]), b"GT1G");
        }
    }

    #[test]
    fn rejects_bad_sizes() {
        let mut file = container(1, &[("a", b"1234")]);
        file[8] = 99;
        assert!(Elixir::parse(&file).is_err());
    }
}
