//! Ryza DX's sound containers: `Data/x64/Sound/*.ktsl2stbin` (streams), the same layout family
//! as the `.ktsl2asbin` banks.
//!
//! A file is a 0x40-byte header, then chunks back to back up to the end of the file:
//!
//! ```text
//! 0x00  "KTSR", a u32, a u32, a u32 (platform id)
//! 0x10  8 zero bytes, the file's size (u32), the file's size again (u32)
//! 0x20  zeros up to 0x40, where the first chunk starts
//! chunk:
//!   +0x00 u32 kind (0x15f4d409 for a stream)
//!   +0x04 u32 the chunk's size, to the next chunk (a multiple of 16)
//!   +0x08 u32 the stream's id
//!   +0x0c u32 where the stream starts in the chunk (0x20)
//!   +0x10 u32 the stream's size
//!   +0x14 12 zero bytes
//!   +0x20 the stream: a `KOVS` clip ([`crate::voice`]), then padding to the chunk's size
//! ```
//!
//! A `.ktsl2asbin` bank (the sound effects' `SE_*`, and the stream banks' tables) is the same
//! chunk run, with two kinds of chunk named by the same hash: a sound definition
//! ([`KIND_DEFINITION`]) and its samples ([`KIND_SAMPLE`]; see [`Sample`]).
//!
//! The readers take a `Read + Seek`, so a 240 MB file is walked by its chunk headers without
//! being held in memory.

use std::io::{Read, Seek, SeekFrom};

use crate::Error;

/// Where the first chunk starts.
const FIRST_CHUNK: u64 = 0x40;
/// A chunk's header size, and where its stream starts.
const HEADER: u64 = 0x20;
/// The kind of a chunk holding a stream.
pub const KIND_STREAM: u32 = 0x15f4_d409;

/// The kind of a bank chunk holding a sound's definition (how it plays; the sfx table's hash).
pub const KIND_DEFINITION: u32 = 0x368c_88bd;
/// The kind of a bank chunk holding a sound's samples.
pub const KIND_SAMPLE: u32 = 0x70cb_ccc5;

/// One stream of a container: its id, and where its `KOVS` clip is in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stream {
    pub id: u32,
    pub offset: u64,
    pub size: u64,
}

fn io(_: std::io::Error) -> Error {
    Error::Corrupt("read past the end")
}

fn u32_at(h: &[u8; HEADER as usize], at: usize) -> u32 {
    u32::from_le_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]])
}

/// The chunk header at `at`: its kind, its size and its stream (checked to lie in the chunk).
fn chunk<R: Read + Seek>(r: &mut R, at: u64) -> Result<(u32, u64, Stream), Error> {
    let mut h = [0u8; HEADER as usize];
    r.seek(SeekFrom::Start(at)).map_err(io)?;
    r.read_exact(&mut h).map_err(io)?;
    let (kind, size) = (u32_at(&h, 0), u64::from(u32_at(&h, 4)));
    let (start, len) = (u64::from(u32_at(&h, 12)), u64::from(u32_at(&h, 16)));
    if size < HEADER || start + len > size {
        return Err(Error::Corrupt("KTSR chunk size"));
    }
    Ok((kind, size, Stream { id: u32_at(&h, 8), offset: at + start, size: len }))
}

/// Every stream of the container, in file order.
pub fn parse_streams<R: Read + Seek>(r: &mut R) -> Result<Vec<Stream>, Error> {
    let mut magic = [0u8; 4];
    r.seek(SeekFrom::Start(0)).map_err(io)?;
    r.read_exact(&mut magic).map_err(io)?;
    if &magic != b"KTSR" {
        return Err(Error::NotThisFormat);
    }
    let end = r.seek(SeekFrom::End(0)).map_err(io)?;
    let (mut at, mut out) = (FIRST_CHUNK, Vec::new());
    while at < end {
        let (kind, size, stream) = chunk(r, at)?;
        if kind == KIND_STREAM {
            out.push(stream);
        }
        at += size;
    }
    Ok(out)
}

/// One sample chunk of a bank: the sound's hash, where the chunk is, and what its header says
/// of the audio (the codec is a 32-bit id: the effect banks' is `0xd2db2960`, the stream banks'
/// `0xb9862b52`; the effects' codec is not decoded, `docs/formats.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub hash: u32,
    /// The chunk's offset and size.
    pub offset: u64,
    pub size: u64,
    pub codec: u32,
    pub channels: u32,
    /// The rate the header gives (near 48000, a little off for most sounds).
    pub rate: u32,
    /// The length in frames.
    pub frames: u32,
}

/// A bank's contents: the hashes of its sound definitions and its sample chunks, in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bank {
    pub definitions: Vec<u32>,
    pub samples: Vec<Sample>,
}

impl Bank {
    /// The sample of the sound `hash`.
    pub fn sample(&self, hash: u32) -> Option<&Sample> {
        self.samples.iter().find(|s| s.hash == hash)
    }
}

/// The chunks of a `.ktsl2asbin` bank: its sound definitions and samples (any other kind of
/// chunk is skipped).
pub fn parse_bank<R: Read + Seek>(r: &mut R) -> Result<Bank, Error> {
    // The sample chunk's header: the codec at +0x28, the channels at +0x2c, the rate at +0x34
    // and the length in frames at +0x38 (the fields between and after are not read).
    const SAMPLE_HEADER: usize = 0x3c;
    let mut magic = [0u8; 4];
    r.seek(SeekFrom::Start(0)).map_err(io)?;
    r.read_exact(&mut magic).map_err(io)?;
    if &magic != b"KTSR" {
        return Err(Error::NotThisFormat);
    }
    let end = r.seek(SeekFrom::End(0)).map_err(io)?;
    let (mut at, mut bank) = (FIRST_CHUNK, Bank::default());
    while at < end {
        let mut h = [0u8; SAMPLE_HEADER];
        r.seek(SeekFrom::Start(at)).map_err(io)?;
        let got = r.read(&mut h).map_err(io)?;
        let word = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
        if got < 16 {
            return Err(Error::Corrupt("KTSR chunk cut short"));
        }
        let (kind, size, hash) = (word(0), u64::from(word(4)), word(8));
        if size < 16 {
            return Err(Error::Corrupt("KTSR chunk size"));
        }
        match kind {
            KIND_DEFINITION => bank.definitions.push(hash),
            KIND_SAMPLE if got == SAMPLE_HEADER => bank.samples.push(Sample { hash, offset: at, size, codec: word(0x28), channels: word(0x2c), rate: word(0x34), frames: word(0x38) }),
            _ => {}
        }
        at += size;
    }
    Ok(bank)
}

/// The stream whose `KOVS` clip is at `offset`, from its chunk's header just before it.
pub fn stream_at<R: Read + Seek>(r: &mut R, offset: u64) -> Result<Stream, Error> {
    let (kind, _, stream) = chunk(r, offset.checked_sub(HEADER).ok_or(Error::Corrupt("KTSR offset"))?)?;
    if kind != KIND_STREAM || stream.offset != offset {
        return Err(Error::Corrupt("no KTSR stream at this offset"));
    }
    Ok(stream)
}

/// A stream's `KOVS` bytes.
pub fn read_stream<R: Read + Seek>(r: &mut R, s: &Stream) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; usize::try_from(s.size).map_err(|_| Error::TooLarge)?];
    r.seek(SeekFrom::Start(s.offset)).map_err(io)?;
    r.read_exact(&mut out).map_err(io)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn chunk_bytes(kind: u32, id: u32, body: &[u8]) -> Vec<u8> {
        let size = (HEADER as usize + body.len()).next_multiple_of(16);
        let mut c = Vec::new();
        for v in [kind, size as u32, id, HEADER as u32, body.len() as u32, 0, 0, 0] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(body);
        c.resize(size, 0);
        c
    }

    fn file() -> Vec<u8> {
        let mut f = b"KTSR".to_vec();
        f.resize(FIRST_CHUNK as usize, 0);
        f.extend(chunk_bytes(KIND_STREAM, 7, b"KOVS-first-clip"));
        f.extend(chunk_bytes(0x1234, 8, b"other"));
        f.extend(chunk_bytes(KIND_STREAM, 9, b"KOVS-2"));
        f
    }

    #[test]
    fn streams_are_listed_in_order() {
        let f = file();
        let s = parse_streams(&mut Cursor::new(&f)).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].id, s[0].offset, s[0].size), (7, 0x60, 15));
        assert_eq!((s[1].id, s[1].size), (9, 6));
        assert_eq!(&f[s[1].offset as usize..][..6], b"KOVS-2");
    }

    #[test]
    fn a_stream_is_found_by_its_offset_and_read() {
        let f = file();
        let mut c = Cursor::new(&f);
        let s = stream_at(&mut c, 0x60).unwrap();
        assert_eq!(s.id, 7);
        assert_eq!(read_stream(&mut c, &s).unwrap(), b"KOVS-first-clip");
        assert!(stream_at(&mut c, 0x61).is_err());
        assert!(stream_at(&mut c, 3).is_err());
    }

    #[test]
    fn a_bank_lists_its_definitions_and_samples() {
        let mut f = b"KTSR".to_vec();
        f.resize(FIRST_CHUNK as usize, 0);
        f.extend(chunk_bytes(KIND_DEFINITION, 0x0d0f_cb87, &[0; 0x20]));
        let at = f.len() as u64;
        // a sample chunk: kind, size, hash, then the header fields the reader takes
        let mut c = vec![0u8; 0x40];
        for (i, v) in [(0, KIND_SAMPLE), (4, 0x40), (8, 0x0d0f_cb87), (0x28, 0xd2db_2960), (0x2c, 2), (0x34, 48_001), (0x38, 60_256)] {
            c[i..i + 4].copy_from_slice(&u32::to_le_bytes(v));
        }
        f.extend(c);
        let b = parse_bank(&mut Cursor::new(&f)).unwrap();
        assert_eq!(b.definitions, [0x0d0f_cb87]);
        let s = b.sample(0x0d0f_cb87).unwrap();
        assert_eq!((s.offset, s.codec, s.channels, s.rate, s.frames), (at, 0xd2db_2960, 2, 48_001, 60_256));
        assert!(b.sample(1).is_none());
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(matches!(parse_streams(&mut Cursor::new(b"NOPE".to_vec())), Err(Error::NotThisFormat)));
        let mut f = file();
        f[0x51] = 0xff; // the first stream's size now runs past its chunk
        assert!(parse_streams(&mut Cursor::new(&f)).is_err());
    }
}
