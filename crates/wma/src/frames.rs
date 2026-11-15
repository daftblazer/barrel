//! Packets and frames. A stream is cut into packets of `block_align` bytes: a 4-bit sequence
//! number, 2 unknown bits, then how many bits finish the frame the previous packet began, then
//! whole frames, the last of which may run on into the next packet. With `len_prefix` each
//! frame starts with its own length in bits, which includes its last bit: "another frame
//! follows in this packet".

use crate::bits::{Reader, Writer};
use crate::params::Params;
use crate::Error;

/// A frame's bits, starting at bit 0.
#[derive(Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub bits: usize,
}

/// Cuts packets into whole frames.
pub struct Packets {
    log2_frame_size: u32,
    block_align: usize,
    len_prefix: bool,
    /// The partial frame carried over from the previous packet.
    carry: Writer,
    /// The next packet's sequence number, once one was read.
    expect: Option<u8>,
    /// Packets that did not follow their predecessor (their carried-over frame is dropped).
    pub lost: usize,
}

impl Packets {
    pub fn new(p: &Params) -> Self {
        Packets { log2_frame_size: p.log2_frame_size, block_align: p.block_align, len_prefix: p.len_prefix, carry: Writer::default(), expect: None, lost: 0 }
    }

    /// The whole frames finished in a stream payload (one or more packets of `block_align`
    /// bytes), in order.
    pub fn push(&mut self, payload: &[u8]) -> Result<Vec<Frame>, Error> {
        if !self.len_prefix {
            return Err(Error("frames without a length prefix: not supported yet".into()));
        }
        let mut frames = Vec::new();
        for packet in payload.chunks(self.block_align) {
            if packet.len() < self.block_align {
                return Err(Error(format!("a short packet of {} bytes", packet.len())));
            }
            self.packet(packet, &mut frames)?;
        }
        Ok(frames)
    }

    fn packet(&mut self, packet: &[u8], frames: &mut Vec<Frame>) -> Result<(), Error> {
        let mut r = Reader::new(packet, packet.len() * 8);
        let sequence = r.read(4) as u8;
        r.skip(2);
        let mut previous = r.read(self.log2_frame_size) as usize;
        let continues = self.expect == Some(sequence) && self.carry.len > 0;
        if self.expect.is_some() && self.expect != Some(sequence) {
            self.lost += 1;
        }
        self.expect = Some((sequence + 1) & 15);
        if previous > 0 {
            let left = r.left().max(0) as usize;
            let all = previous >= left;
            previous = previous.min(left);
            if continues {
                self.carry.take(&mut r, previous);
                frames.push(Frame { data: std::mem::take(&mut self.carry.data), bits: self.carry.len });
                self.carry.clear();
            } else {
                r.skip(previous);
            }
            if all {
                return Ok(());
            }
        }
        self.carry.clear();
        // Frames starting in this packet.
        loop {
            let left = r.left().max(0) as usize;
            let size = r.peek(self.log2_frame_size) as usize;
            if left <= self.log2_frame_size as usize || size == 0 || size > left {
                break;
            }
            let mut w = Writer::default();
            w.take(&mut r, size);
            let more = w.data[(size - 1) >> 3] & (0x80 >> ((size - 1) & 7)) != 0;
            frames.push(Frame { data: w.data, bits: size });
            if !more {
                break;
            }
        }
        // The rest begins a frame finished by the next packet.
        let left = r.left().max(0) as usize;
        self.carry.take(&mut r, left);
        Ok(())
    }
}

/// What a frame says before its subframes.
#[derive(Debug, Clone, Default)]
pub struct FrameHeader {
    pub len: usize,
    /// Each channel's subframe lengths, in order.
    pub subframes: Vec<Vec<usize>>,
    pub drc_gain: u8,
    pub trim_start: usize,
    pub trim_end: usize,
    /// The bit after the header: the first subframe's.
    pub end: usize,
}

/// A subframe's length (`decode_subframe_length`) at sample `offset` of the frame.
fn subframe_length(p: &Params, r: &mut Reader, offset: usize) -> Result<usize, Error> {
    if offset == p.samples_per_frame - p.min_samples_per_subframe {
        return Ok(p.min_samples_per_subframe);
    }
    let shift = if p.max_subframe_len_bit {
        if r.bit() { 1 + r.read(p.subframe_len_bits - 1) } else { 0 }
    } else {
        r.read(p.subframe_len_bits)
    };
    let len = p.samples_per_frame >> shift;
    if len < p.min_samples_per_subframe {
        return Err(Error(format!("subframe of {len} samples")));
    }
    Ok(len)
}

/// The tile header (`decode_tilehdr`): how each channel's samples split into subframes. When
/// the layout is not fixed, each channel at the shortest total so far says whether it takes
/// the next subframe.
fn tile_header(p: &Params, r: &mut Reader) -> Result<Vec<Vec<usize>>, Error> {
    let n = p.channels;
    let mut subframes = vec![Vec::new(); n];
    let mut samples = vec![0usize; n];
    let fixed = p.max_subframes == 1 || r.bit();
    let (mut min_len, mut channels_here) = (0usize, n);
    loop {
        let contains: Vec<bool> = (0..n)
            .map(|c| samples[c] == min_len && (fixed || channels_here == 1 || min_len == p.samples_per_frame - p.min_samples_per_subframe || r.bit()))
            .collect();
        let len = subframe_length(p, r, min_len)?;
        min_len += len;
        for c in 0..n {
            if contains[c] {
                if subframes[c].len() >= 32 {
                    return Err(Error("more than 31 subframes".into()));
                }
                subframes[c].push(len);
                samples[c] += len;
                if samples[c] > p.samples_per_frame {
                    return Err(Error("a channel longer than its frame".into()));
                }
            } else if samples[c] <= min_len {
                if samples[c] < min_len {
                    channels_here = 0;
                    min_len = samples[c];
                }
                channels_here += 1;
            }
        }
        if min_len >= p.samples_per_frame {
            return Ok(subframes);
        }
    }
}

impl FrameHeader {
    /// Reads a frame's header (`decode_frame` up to the subframes).
    pub fn read(p: &Params, r: &mut Reader) -> Result<Self, Error> {
        let mut h = FrameHeader::default();
        if p.len_prefix {
            h.len = r.read(p.log2_frame_size) as usize;
        }
        h.subframes = tile_header(p, r)?;
        // A post-processing transform: read past.
        if p.channels > 1 && r.bit() && r.bit() {
            r.skip(4 * p.channels * p.channels);
        }
        if p.drc {
            h.drc_gain = r.read(8) as u8;
        }
        if r.bit() {
            let bits = (2 * p.samples_per_frame as u32).ilog2();
            if r.bit() {
                h.trim_start = r.read(bits) as usize;
            }
            if r.bit() {
                h.trim_end = r.read(bits) as usize;
            }
        }
        if r.over() {
            return Err(Error("frame header runs out".into()));
        }
        h.end = r.position();
        Ok(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::asf::{Asf, Kind};

    /// `BARREL_MOVIE`'s audio cut into frames, each frame's header read.
    #[test]
    #[ignore]
    fn movie_audio_frames() {
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let bytes = std::fs::read(path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let audio = asf.streams.iter().find(|s| matches!(s.kind, Kind::Audio { .. })).unwrap();
        let Kind::Audio { rate, block_align, channels, extra, .. } = &audio.kind else { unreachable!() };
        let p = Params::new(extra, *rate, *block_align as usize, *channels as usize).unwrap();
        eprintln!("{p:?}");
        let mut packets = Packets::new(&p);
        let (mut frames, mut layouts) = (0usize, std::collections::BTreeMap::<String, usize>::new());
        let (mut trims, mut drc) = (Vec::new(), std::collections::BTreeSet::new());
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == audio.number) {
            for f in packets.push(&o.data).unwrap() {
                let mut r = Reader::new(&f.data, f.bits);
                let h = FrameHeader::read(&p, &mut r).unwrap();
                assert_eq!(h.len, f.bits, "frame {frames}: its length field");
                *layouts.entry(format!("{:?}", h.subframes)).or_default() += 1;
                if h.trim_start != 0 || h.trim_end != 0 {
                    trims.push((frames, h.trim_start, h.trim_end));
                }
                drc.insert(h.drc_gain);
                frames += 1;
            }
        }
        let mut common: Vec<_> = layouts.iter().collect();
        common.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        eprintln!("{frames} frames, {} packets out of sequence; {} layouts, most common {:?}; trims {trims:?}; drc gains {}..{}", packets.lost, layouts.len(), &common[..common.len().min(4)], drc.first().unwrap(), drc.last().unwrap());
    }
}
