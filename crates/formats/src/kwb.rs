//! Sound effect banks (`Sound/SE/<name>.wh2` with its data `<name>.wb2`, Koei Tecmo's KWB2).
//!
//! ```text
//! .wh2: "_HBW0000", u32 file size, "KWB2", u16, u16 sound count (127), u32 data size,
//!       f32 1.0, ..., at +0x24 one u32 offset per sound (into this file)
//!   per sound, one or more blocks:
//!       u16 rate (48000), u8 0, u8 channels, u32 codec (2: 16-bit PCM), u32 0,
//!       u32 samples, u32 offset, u32 size (bytes, in the data after its header)
//! .wb2: "_DBW0000", u32 file size, then the samples (the data starts at 0xc)
//! ```
//!
//! The blocks are found by their shape within each sound's span (rate, channels, and for PCM a
//! size of 2 bytes a sample per channel); a sound often has two blocks with the same samples
//! (what the second is for is not read).

use crate::{u32_at, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub rate: u32,
    pub channels: u8,
    pub codec: u32,
    pub samples: u32,
    /// Bytes from the start of the data (`.wb2` + 0xc).
    pub offset: u32,
    pub size: u32,
}

/// Where a `.wb2`'s samples start.
pub const DATA: usize = 0xc;

/// The bank's sounds, each with its blocks.
pub fn sounds(wh2: &[u8], data_len: usize) -> Result<Vec<Vec<Block>>, Error> {
    if wh2.get(..8) != Some(b"_HBW0000") || wh2.get(0xc..0x10) != Some(b"KWB2") {
        return Err(Error::NotThisFormat);
    }
    let count = u16::from_le_bytes([wh2[0x12], wh2[0x13]]) as usize;
    let offsets: Vec<usize> = (0..count).map(|i| u32_at(wh2, 0x24 + 4 * i).map(|o| o as usize)).collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(count);
    for (i, &start) in offsets.iter().enumerate() {
        let end = offsets.get(i + 1).copied().unwrap_or(wh2.len()).min(wh2.len());
        let mut blocks = Vec::new();
        let mut p = start;
        while p + 24 <= end {
            let rate = u32::from(u16::from_le_bytes([wh2[p], wh2[p + 1]]));
            let (zero, channels) = (wh2[p + 2], wh2[p + 3]);
            let codec = u32_at(wh2, p + 4)?;
            let samples = u32_at(wh2, p + 12)?;
            let offset = u32_at(wh2, p + 16)?;
            let size = u32_at(wh2, p + 20)?;
            let fits = matches!(rate, 22050 | 24000 | 32000 | 44100 | 48000)
                && zero == 0
                && (channels == 1 || channels == 2)
                && samples > 0
                && offset as usize + size as usize <= data_len.saturating_sub(DATA)
                && (codec != 2 || size == samples * 2 * u32::from(channels));
            if fits {
                blocks.push(Block { rate, channels, codec, samples, offset, size });
                p += 24;
            } else {
                p += 2;
            }
        }
        out.push(blocks);
    }
    Ok(out)
}

/// A PCM block's samples, per channel, as floats.
pub fn pcm(wb2: &[u8], b: &Block) -> Result<Vec<Vec<f32>>, Error> {
    if b.codec != 2 {
        return Err(Error::Corrupt("KWB2 codec other than PCM"));
    }
    let start = DATA + b.offset as usize;
    let bytes = wb2.get(start..start + b.size as usize).ok_or(Error::Corrupt("KWB2 block past the data"))?;
    let ch = usize::from(b.channels);
    let mut out = vec![Vec::with_capacity(b.samples as usize); ch];
    for (i, s) in bytes.chunks_exact(2).enumerate() {
        out[i % ch].push(f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0);
    }
    Ok(out)
}
