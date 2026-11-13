//! Voice packs (`Data/x64/Sound/Voice*/SeriesNNNN.fp` with its list `SeriesNNNN.fpl`, Escha &
//! Logy DX): each series holds up to 1,000 voice clips.
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

/// The first voice clip of each event's lines in the English executable: an i32 per `EVENT_*`
/// name (1,165, in the enum's order) at this address, -1 for an event without voices; read by
/// `0x140213420 (event)`. A line's clip is its event's first clip plus the line's index.
pub const FIRST_CLIPS_EN: u64 = 0x1408_2d710;
pub const EVENTS: usize = 1165;

/// The events' first voice clips (`None` for an unvoiced event), from the executable.
pub fn first_clips(pe: &crate::pe::Pe, at: u64) -> Option<Vec<Option<u32>>> {
    let o = pe.offset(at)?;
    let d = pe.data.get(o..o + 4 * EVENTS)?;
    Some(d.chunks_exact(4).map(|c| u32::try_from(i32::from_le_bytes(c.try_into().unwrap())).ok()).collect())
}

/// Where an event's lines most likely start among the clips, guessed without the executable
/// ([`first_clips`] has them; this cross-checks it, docs/formats.md, "Story lines' voices"): a story line's clip is its scene's first clip plus the line's index, and a clip's
/// size follows the line's length, so the first clip is the place where the run of `sizes`
/// (by clip number, 0 for none) correlates best with the lines' `lengths`. Returns the clip
/// and the correlation (Pearson, -1 to 1); `None` for fewer than 3 lines.
pub fn story_start(sizes: &[u32], lengths: &[usize]) -> Option<(usize, f32)> {
    let n = lengths.len();
    if n < 3 || sizes.len() < n {
        return None;
    }
    let lens: Vec<f64> = lengths.iter().map(|&l| l as f64).collect();
    let mean_l = lens.iter().sum::<f64>() / n as f64;
    let dl: Vec<f64> = lens.iter().map(|l| l - mean_l).collect();
    let var_l: f64 = dl.iter().map(|d| d * d).sum();
    if var_l == 0.0 {
        return None;
    }
    let mut best: Option<(usize, f32)> = None;
    for start in 0..=sizes.len() - n {
        let win = &sizes[start..start + n];
        if win.contains(&0) {
            continue;
        }
        let mean_s = win.iter().map(|&s| s as f64).sum::<f64>() / n as f64;
        let (mut cov, mut var_s) = (0.0, 0.0);
        for (d, &s) in dl.iter().zip(win) {
            let ds = s as f64 - mean_s;
            cov += d * ds;
            var_s += ds * ds;
        }
        if var_s == 0.0 {
            continue;
        }
        let r = (cov / (var_l * var_s).sqrt()) as f32;
        if best.is_none_or(|(_, b)| r > b) {
            best = Some((start, r));
        }
    }
    best
}

#[cfg(test)]
mod story_tests {
    #[test]
    fn finds_the_run_that_follows_the_lengths() {
        let mut sizes = vec![500u32; 40];
        let lengths = [10usize, 40, 20, 80, 15];
        for (k, &l) in lengths.iter().enumerate() {
            sizes[17 + k] = 100 * l as u32 + 7;
        }
        let (start, r) = super::story_start(&sizes, &lengths).unwrap();
        assert_eq!(start, 17);
        assert!(r > 0.99);
    }
}
