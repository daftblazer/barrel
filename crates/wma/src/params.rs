//! A stream's fixed parameters, from its `WAVEFORMATEX` extra data (18 bytes: bits per sample,
//! channel mask, decode flags), sample rate and block alignment, and the tables derived from
//! them for every block size.

use crate::Error;

/// The largest number of scale factor bands a block has.
pub const MAX_BANDS: usize = 29;
pub const BLOCK_MIN_BITS: u32 = 6;
pub const BLOCK_MAX_BITS: u32 = 13;

/// Band edges in Hz (FFmpeg's `critical_freq` in `wmaprodata.h`).
const CRITICAL_FREQS: [u32; 28] = [
    100, 200, 300, 400, 510, 630, 770, 920, 1080, 1270, 1480, 1720, 2000, 2320, 2700, 3150, 3700, 4400, 5300, 6400, 7700, 9500, 12000, 15500,
    20675, 28575, 41375, 63875,
];

#[derive(Debug, Clone)]
pub struct Params {
    pub channels: usize,
    pub rate: u32,
    pub block_align: usize,
    pub bits_per_sample: u32,
    pub channel_mask: u32,
    pub decode_flags: u16,
    /// Frames start with their length in bits.
    pub len_prefix: bool,
    /// Frames carry a dynamic range control gain.
    pub drc: bool,
    /// Bits of a frame length (and of a packet's "bits for the previous frame").
    pub log2_frame_size: u32,
    pub samples_per_frame: usize,
    pub max_subframes: usize,
    /// The first subframe length bit says "the whole frame".
    pub max_subframe_len_bit: bool,
    pub subframe_len_bits: u32,
    pub min_samples_per_subframe: usize,
    pub lfe_channel: Option<usize>,
    /// By block size (frame >> i): scale factor band edges (the last is the block length).
    pub sfb_offsets: Vec<Vec<usize>>,
    /// By block size i, other block size x and band b: the band of x a scale factor of i's
    /// band b is taken from.
    pub sf_offsets: Vec<Vec<Vec<usize>>>,
    pub subwoofer_cutoffs: Vec<usize>,
}

fn log2(v: u32) -> u32 {
    if v == 0 { 0 } else { 31 - v.leading_zeros() }
}

impl Params {
    pub fn new(extra: &[u8], rate: u32, block_align: usize, channels: usize) -> Result<Self, Error> {
        if extra.len() < 18 {
            return Err(Error(format!("extra data of {} bytes, not 18", extra.len())));
        }
        let le16 = |at: usize| u16::from_le_bytes([extra[at], extra[at + 1]]);
        let bits_per_sample = le16(0) as u32;
        let channel_mask = u32::from_le_bytes([extra[2], extra[3], extra[4], extra[5]]);
        let decode_flags = le16(14);
        let channels = if channel_mask != 0 { channel_mask.count_ones() as usize } else { channels };
        if !(1..=32).contains(&bits_per_sample) || !(1..=8).contains(&channels) || block_align == 0 || rate == 0 {
            return Err(Error(format!("{bits_per_sample} bits, {channels} channels, block {block_align}, {rate} Hz")));
        }
        let log2_frame_size = log2(block_align as u32) + 4;
        // ff_wma_get_frame_len_bits, version 3.
        let mut frame_bits: i32 = match rate {
            ..=16000 => 9,
            ..=22050 => 10,
            ..=48000 => 11,
            ..=96000 => 12,
            _ => 13,
        };
        frame_bits += match decode_flags & 6 {
            2 => 1,
            4 => -1,
            6 => -2,
            _ => 0,
        };
        if frame_bits > BLOCK_MAX_BITS as i32 {
            return Err(Error("14-bit block sizes".into()));
        }
        let samples_per_frame = 1usize << frame_bits;
        let log2_max_subframes = ((decode_flags & 0x38) >> 3) as u32;
        let max_subframes = 1usize << log2_max_subframes;
        let min_samples_per_subframe = samples_per_frame / max_subframes;
        if max_subframes > 32 || min_samples_per_subframe < (1 << BLOCK_MIN_BITS) {
            return Err(Error(format!("{max_subframes} subframes of {min_samples_per_subframe}")));
        }
        let lfe_channel = (channel_mask & 8 != 0).then(|| (0..4).filter(|k| channel_mask & (1 << k) != 0).count() - 1);
        let sizes = log2_max_subframes as usize + 1;
        let mut sfb_offsets = Vec::with_capacity(sizes);
        for i in 0..sizes {
            let len = samples_per_frame >> i;
            let mut bands = vec![0usize];
            for &f in CRITICAL_FREQS.iter().take(MAX_BANDS - 1) {
                if *bands.last().unwrap() >= len {
                    break;
                }
                let offset = ((len as u64 * 2 * f as u64) / rate as u64 + 2) as usize & !3;
                if offset > *bands.last().unwrap() {
                    bands.push(offset);
                }
                if offset >= len {
                    break;
                }
            }
            *bands.last_mut().unwrap() = len;
            if bands.len() < 2 {
                return Err(Error("no scale factor bands".into()));
            }
            sfb_offsets.push(bands);
        }
        let mut sf_offsets = vec![vec![vec![0usize; MAX_BANDS]; sizes]; sizes];
        for i in 0..sizes {
            for b in 0..sfb_offsets[i].len() - 1 {
                let offset = ((sfb_offsets[i][b] + sfb_offsets[i][b + 1] - 1) << i) >> 1;
                for x in 0..sizes {
                    let mut v = 0;
                    while (sfb_offsets[x].get(v + 1).copied().unwrap_or(usize::MAX) << x) < offset {
                        v += 1;
                    }
                    sf_offsets[i][x][b] = v;
                }
            }
        }
        let subwoofer_cutoffs = (0..sizes)
            .map(|i| {
                let block = samples_per_frame >> i;
                (((440 * block as u64) + 3 * (rate as u64 >> 1) - 1) / rate as u64).clamp(4, block as u64) as usize
            })
            .collect();
        Ok(Params {
            channels,
            rate,
            block_align,
            bits_per_sample,
            channel_mask,
            decode_flags,
            len_prefix: decode_flags & 0x40 != 0,
            drc: decode_flags & 0x80 != 0,
            log2_frame_size,
            samples_per_frame,
            max_subframes,
            max_subframe_len_bit: max_subframes == 16 || max_subframes == 4,
            subframe_len_bits: log2(log2_max_subframes) + 1,
            min_samples_per_subframe,
            lfe_channel,
            sfb_offsets,
            sf_offsets,
            subwoofer_cutoffs,
        })
    }

    /// The scale factor bands of a block of `len` samples: its block size index.
    pub fn size_index(&self, len: usize) -> usize {
        (self.samples_per_frame / len).trailing_zeros() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avantitle_parameters() {
        let extra = [0x10, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xe0, 0, 0, 0];
        let p = Params::new(&extra, 48000, 8192, 2).unwrap();
        assert_eq!((p.channels, p.bits_per_sample, p.samples_per_frame, p.max_subframes), (2, 16, 2048, 16));
        assert!(p.len_prefix && p.drc && p.max_subframe_len_bit);
        assert_eq!((p.subframe_len_bits, p.min_samples_per_subframe, p.log2_frame_size), (3, 128, 17));
        assert_eq!(p.sfb_offsets.len(), 5);
        assert_eq!(*p.sfb_offsets[0].last().unwrap(), 2048);
        assert!(p.sfb_offsets.iter().all(|b| b.len() <= MAX_BANDS && b.windows(2).all(|w| w[0] < w[1])));
    }
}
