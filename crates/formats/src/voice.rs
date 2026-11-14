//! Voice packs (`Data/x64/Sound/Voice*/SeriesNNNN.fp` with its list `SeriesNNNN.fpl`, Escha &
//! Logy DX): each series holds up to 1,000 voice clips. (Which clip an event's line plays is
//! the game's: Escha's, `barrel_escha::data::voice`.)
//!
//! ```text
//! .fpl: u32 count (1000), then count × {u32 size, u32 offset} into the .fp (size 0: no clip)
//! .fp:  the clips one after another, each a KOVS file:
//!       "KOVS", u32 Ogg size, 24 bytes (0 for the voices; for music the loop, [`looping`]),
//!       then the Ogg Vorbis stream with its first
//!       0x100 bytes XORed with their index ("OfeP" for "OggS")
//! ```

use crate::{Error, u32_at};

/// A series' clips: (offset, size) in the `.fp`, by clip number; `None` where there is none.
pub fn list(fpl: &[u8]) -> Result<Vec<Option<(usize, usize)>>, Error> {
    let count = u32_at(fpl, 0)? as usize;
    if count > 100_000 || fpl.len() < 4 + 8 * count {
        return Err(Error::Corrupt("voice list size"));
    }
    (0..count)
        .map(|i| {
            let size = u32_at(fpl, 4 + 8 * i)? as usize;
            let offset = u32_at(fpl, 8 + 8 * i)? as usize;
            Ok((size > 0).then_some((offset, size)))
        })
        .collect()
}

/// How a KOVS stream repeats (the header after its size; from the music, the code that plays it
/// is not read): `once` for the pieces played a single time (+0x10 is 1: the item jingle, the
/// orchestral title), otherwise looping from `start` (+0x08, in samples) to `end` (+0x0c; 0 in
/// every file so far, the end of the stream).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Looping {
    pub start: u32,
    pub end: u32,
    pub once: bool,
}

pub fn looping(kovs: &[u8]) -> Result<Looping, Error> {
    if !kovs.starts_with(b"KOVS") {
        return Err(Error::NotThisFormat);
    }
    Ok(Looping { start: u32_at(kovs, 8)?, end: u32_at(kovs, 12)?, once: u32_at(kovs, 16)? == 1 })
}

/// A KOVS clip's Ogg Vorbis stream, unscrambled.
pub fn ogg(kovs: &[u8]) -> Result<Vec<u8>, Error> {
    if !kovs.starts_with(b"KOVS") {
        return Err(Error::NotThisFormat);
    }
    let size = u32_at(kovs, 4)? as usize;
    let mut out = kovs.get(0x20..0x20 + size).ok_or(Error::Corrupt("KOVS size past the end"))?.to_vec();
    for (i, b) in out.iter_mut().take(0x100).enumerate() {
        *b ^= i as u8;
    }
    if !out.starts_with(b"OggS") {
        return Err(Error::Corrupt("KOVS does not hold Ogg"));
    }
    Ok(out)
}

/// An Ogg Vorbis stream's length in seconds: the last page's granule position (samples) over
/// the identification header's sample rate.
pub fn ogg_seconds(ogg: &[u8]) -> Option<f32> {
    // The identification header: page header (27 bytes + segment table), then
    // "\x01vorbis", version (4), channels (1), rate (4).
    let segments = *ogg.get(26)? as usize;
    let packet = 27 + segments;
    if ogg.get(packet..packet + 7)? != b"\x01vorbis" {
        return None;
    }
    let rate = u32::from_le_bytes(ogg.get(packet + 12..packet + 16)?.try_into().ok()?);
    let last = ogg.windows(4).rposition(|w| w == b"OggS")?;
    let granule = u64::from_le_bytes(ogg.get(last + 6..last + 14)?.try_into().ok()?);
    (rate > 0).then(|| granule as f32 / rate as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_unscrambles_and_has_a_length() {
        // An Ogg stream of an identification page and a last page at 48,000 samples (2 s at
        // 24 kHz), scrambled into a KOVS clip.
        let mut page = b"OggS\0\x02".to_vec();
        page.extend_from_slice(&0u64.to_le_bytes());
        page.extend_from_slice(&[0; 12]);
        page.push(1);
        page.push(30);
        page.extend_from_slice(b"\x01vorbis");
        page.extend_from_slice(&0u32.to_le_bytes());
        page.push(1);
        page.extend_from_slice(&24_000u32.to_le_bytes());
        page.resize(27 + 1 + 30, 0);
        let mut last = b"OggS\0\x04".to_vec();
        last.extend_from_slice(&48_000u64.to_le_bytes());
        last.extend_from_slice(&[0; 13]);
        let stream = [page, last].concat();
        let mut kovs = b"KOVS".to_vec();
        kovs.extend_from_slice(&(stream.len() as u32).to_le_bytes());
        kovs.resize(0x20, 0);
        kovs.extend(stream.iter().enumerate().map(|(i, b)| if i < 0x100 { b ^ i as u8 } else { *b }));
        let ogg = ogg(&kovs).unwrap();
        assert_eq!(ogg, stream);
        assert_eq!(ogg_seconds(&ogg), Some(2.0));
        let mut fpl = 2u32.to_le_bytes().to_vec();
        for v in [10u32, 0, 0, 0] {
            fpl.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(list(&fpl).unwrap(), [Some((0, 10)), None]);
    }
}
