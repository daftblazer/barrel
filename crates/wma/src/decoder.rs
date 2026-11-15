//! Frames to samples: subframes, the channel transform, scale factors, coefficients (vector
//! coded, then run / level), inverse quantization, the inverse MDCT and the sine window's
//! overlap with the previous block (FFmpeg's `decode_subframe` and what it calls).

use crate::bits::Reader;
use crate::frames::{FrameHeader, Packets};
use crate::params::{Params, MAX_BANDS};
use crate::tables as t;
use crate::vlc::Vlc;
use crate::Error;

struct Tables {
    scale: Vlc,
    scale_rl: Vlc,
    coef: [Vlc; 2],
    vec4: Vlc,
    vec2: Vlc,
    vec1: Vlc,
}

#[derive(Clone)]
struct Channel {
    prev_block_len: usize,
    /// One and a half frames: the frame being built, its second half's overlap with the next.
    out: Vec<f32>,
    subframes: Vec<usize>,
    cur: usize,
    decoded: usize,
    grouped: bool,
    transmit: bool,
    num_vec_coeffs: usize,
    quant_step: i32,
    reuse_sf: bool,
    sf_step: i32,
    max_sf: i32,
    saved_sf: [[i32; MAX_BANDS]; 2],
    /// Which `saved_sf` holds the last transmitted factors.
    sf_idx: usize,
    /// Which `saved_sf` the current subframe uses.
    sf_use: usize,
    /// The block size the last transmitted factors were for.
    table_idx: usize,
}

struct Group {
    channels: Vec<usize>,
    transform: bool,
    matrix: Vec<f32>,
    bands: [bool; MAX_BANDS],
}

/// Decodes a WMA Pro stream's packets into planar `f32` samples (−1 … 1 for a full scale
/// 16-bit source).
pub struct Decoder {
    pub params: std::sync::Arc<Params>,
    packets: Packets,
    tables: Tables,
    channels: Vec<Channel>,
    /// Sine windows by log2 length.
    windows: Vec<Vec<f32>>,
    sin64: [f32; 33],
    first: bool,
    pub frames: usize,
}

impl Decoder {
    pub fn new(params: Params) -> Self {
        let spf = params.samples_per_frame;
        let channel = Channel {
            prev_block_len: spf,
            out: vec![0.0; spf * 2],
            subframes: Vec::new(),
            cur: 0,
            decoded: 0,
            grouped: false,
            transmit: false,
            num_vec_coeffs: 0,
            quant_step: 0,
            reuse_sf: false,
            sf_step: 1,
            max_sf: 0,
            saved_sf: [[0; MAX_BANDS]; 2],
            sf_idx: 0,
            sf_use: 1,
            table_idx: 0,
        };
        let windows = (0..=13u32)
            .map(|bits| {
                let n = 1usize << bits;
                (0..n).map(|i| ((i as f64 + 0.5) * (std::f64::consts::PI / (2.0 * n as f64))).sin() as f32).collect()
            })
            .collect();
        Decoder {
            packets: Packets::new(&params),
            tables: Tables {
                scale: Vlc::new(&t::SCALE),
                scale_rl: Vlc::new(&t::SCALE_RL),
                coef: [Vlc::new(&t::COEF0), Vlc::new(&t::COEF1)],
                vec4: Vlc::new(&t::VEC4),
                vec2: Vlc::new(&t::VEC2),
                vec1: Vlc::new(&t::VEC1),
            },
            channels: vec![channel; params.channels],
            windows,
            sin64: std::array::from_fn(|i| (i as f64 * std::f64::consts::PI / 64.0).sin() as f32),
            first: true,
            frames: 0,
            params: std::sync::Arc::new(params),
        }
    }

    /// Decodes a stream payload (whole packets); returns the samples it finished, one vector a
    /// channel.
    pub fn push(&mut self, payload: &[u8]) -> Result<Vec<Vec<f32>>, Error> {
        let mut pcm = vec![Vec::new(); self.params.channels];
        for f in self.packets.push(payload)? {
            let mut r = Reader::new(&f.data, f.bits);
            let h = FrameHeader::read(&self.params, &mut r)?;
            self.frame(&h, &mut r).map_err(|e| Error(format!("frame {}: {}", self.frames, e.0)))?;
            self.frames += 1;
            let spf = self.params.samples_per_frame;
            let output = !std::mem::replace(&mut self.first, false);
            for (c, ch) in self.channels.iter_mut().enumerate() {
                if output && h.trim_start + h.trim_end < spf {
                    pcm[c].extend_from_slice(&ch.out[h.trim_start..spf - h.trim_end]);
                }
                ch.out.copy_within(spf..spf + spf / 2, 0);
            }
        }
        Ok(pcm)
    }

    fn frame(&mut self, h: &FrameHeader, r: &mut Reader) -> Result<(), Error> {
        for (c, ch) in self.channels.iter_mut().enumerate() {
            ch.subframes = h.subframes[c].clone();
            ch.decoded = 0;
            ch.cur = 0;
            ch.reuse_sf = false;
        }
        let spf = self.params.samples_per_frame;
        while self.channels.iter().any(|c| c.decoded < spf) {
            self.subframe(r)?;
        }
        if r.over() {
            return Err(Error("frame runs out".into()));
        }
        Ok(())
    }

    fn subframe(&mut self, r: &mut Reader) -> Result<(), Error> {
        let p = self.params.clone();
        let spf = p.samples_per_frame;
        // The next block: the channel with the fewest decoded samples.
        let (mut offset, mut len) = (spf, spf);
        for ch in &mut self.channels {
            ch.grouped = false;
            if offset > ch.decoded {
                offset = ch.decoded;
                len = ch.subframes[ch.cur];
            }
        }
        let here: Vec<usize> = (0..self.channels.len()).filter(|&c| self.channels[c].decoded == offset && self.channels[c].subframes.get(self.channels[c].cur) == Some(&len)).collect();
        for &c in &here {
            self.channels[c].decoded += len;
        }
        let table_idx = p.size_index(len);
        let bands = &p.sfb_offsets[table_idx];
        let num_bands = bands.len() - 1;
        let start = offset + spf / 2;
        let esc_len = (len as u32 - 1).ilog2() + 1;
        // Extended header: fill bits.
        if r.bit() {
            let mut fill = r.read(2) as usize;
            if fill == 0 {
                let n = r.read(4);
                fill = r.read(n) as usize + 1;
            }
            r.skip(fill);
        }
        if r.bit() {
            return Err(Error("the reserved subframe bit".into()));
        }
        // Channel transform.
        let mut groups: Vec<Group> = Vec::new();
        if p.channels > 1 {
            if r.bit() {
                return Err(Error("the channel transform bit".into()));
            }
            let mut remaining = here.len();
            while remaining > 0 && groups.len() < here.len() {
                let mut g = Group { channels: Vec::new(), transform: false, matrix: Vec::new(), bands: [false; MAX_BANDS] };
                if remaining > 2 {
                    for &c in &here {
                        if !self.channels[c].grouped && r.bit() {
                            self.channels[c].grouped = true;
                            g.channels.push(c);
                        }
                    }
                } else {
                    for &c in &here {
                        if !self.channels[c].grouped {
                            g.channels.push(c);
                        }
                        self.channels[c].grouped = true;
                    }
                }
                let n = g.channels.len();
                if n == 2 {
                    if r.bit() {
                        if r.bit() {
                            return Err(Error("an unknown channel transform".into()));
                        }
                    } else {
                        g.transform = true;
                        g.matrix = if p.channels == 2 { vec![1.0, -1.0, 1.0, 1.0] } else { vec![0.70703125, -0.70703125, 0.70703125, 0.70703125] };
                    }
                } else if n > 2 && r.bit() {
                    g.transform = true;
                    if r.bit() {
                        g.matrix = self.decorrelation(r, n);
                    } else {
                        return Err(Error(format!("the default matrix for {n} channels: not supported yet")));
                    }
                }
                if g.transform {
                    if !r.bit() {
                        for b in 0..num_bands {
                            g.bands[b] = r.bit();
                        }
                    } else {
                        g.bands = [true; MAX_BANDS];
                    }
                }
                if n == 0 {
                    return Err(Error("an empty channel group".into()));
                }
                remaining -= n;
                groups.push(g);
            }
        }
        let mut any = false;
        for &c in &here {
            self.channels[c].transmit = r.bit();
            any |= self.channels[c].transmit;
        }
        let mut transmit_num_vec = false;
        if any {
            let mut quant_step = (90 * p.bits_per_sample >> 4) as i32;
            transmit_num_vec = r.bit();
            if transmit_num_vec {
                let bits = ((len as u32 + 3) / 4).ilog2() + 1;
                for &c in &here {
                    let n = (r.read(bits) as usize) << 2;
                    if n > len {
                        return Err(Error(format!("{n} vector coded coefficients")));
                    }
                    self.channels[c].num_vec_coeffs = n;
                }
            } else {
                for &c in &here {
                    self.channels[c].num_vec_coeffs = len;
                }
            }
            let step = ((r.read(6) as i32) << 26) >> 26;
            quant_step += step;
            if step == -32 || step == 31 {
                let negative = step == -32;
                let (mut quant, mut s) = (0, 0);
                while r.position() + 5 < r.len() {
                    s = r.read(5) as i32;
                    if s != 31 {
                        break;
                    }
                    quant += 31;
                }
                quant_step += if negative { -(quant + s) } else { quant + s };
            }
            if here.len() == 1 {
                self.channels[here[0]].quant_step = quant_step;
            } else {
                let modifier_len = r.read(3);
                for &c in &here {
                    self.channels[c].quant_step = quant_step;
                    if r.bit() {
                        self.channels[c].quant_step += if modifier_len != 0 { r.read(modifier_len) as i32 + 1 } else { 1 };
                    }
                }
            }
            self.scale_factors(r, &here, table_idx, num_bands)?;
        }
        // Coefficients into each channel's block.
        for &c in &here {
            let ch = &mut self.channels[c];
            let block = &mut ch.out[start..start + len];
            block.fill(0.0);
            if ch.transmit && r.position() < r.len() {
                coefficients(&self.tables, r, block, ch.num_vec_coeffs, transmit_num_vec, esc_len)?;
            }
        }
        if any {
            // Inverse channel transform.
            for g in &groups {
                if !g.transform {
                    continue;
                }
                let n = g.channels.len();
                for b in 0..num_bands {
                    let (lo, hi) = (bands[b], bands[b + 1].min(len));
                    if g.bands[b] {
                        for y in start + lo..start + hi {
                            let data: Vec<f32> = g.channels.iter().map(|&c| self.channels[c].out[y]).collect();
                            for (row, &c) in g.channels.iter().enumerate() {
                                let mut sum = 0f32;
                                for (k, d) in data.iter().enumerate() {
                                    sum += d * g.matrix[row * n + k];
                                }
                                self.channels[c].out[y] = sum;
                            }
                        }
                    } else if p.channels == 2 {
                        for &c in &g.channels {
                            for v in &mut self.channels[c].out[start + lo..start + hi] {
                                *v *= 181.0 / 128.0;
                            }
                        }
                    }
                }
            }
            // Inverse quantization, then the inverse MDCT in place.
            let scale = 1.0 / (len as f64 / 2.0) / (1u64 << (p.bits_per_sample - 1)) as f64;
            for &c in &here {
                let ch = &mut self.channels[c];
                let sf = ch.saved_sf[ch.sf_use];
                let mut tmp = vec![0f32; len];
                for b in 0..num_bands {
                    let (lo, hi) = (bands[b], bands[b + 1].min(len));
                    let exp = ch.quant_step - (ch.max_sf - sf[b]) * ch.sf_step;
                    let quant = 10f64.powf(exp as f64 / 20.0) as f32;
                    for i in lo..hi {
                        tmp[i] = ch.out[start + i] * quant;
                    }
                }
                imdct_half(&tmp, &mut ch.out[start..start + len], scale);
            }
        }
        // The sine window over the overlap with the previous block.
        for &c in &here {
            let ch = &mut self.channels[c];
            let mut winlen = ch.prev_block_len;
            let mut at = start - winlen / 2;
            if len < winlen {
                at += (winlen - len) / 2;
                winlen = len;
            }
            let w = &self.windows[winlen.ilog2() as usize];
            let half = winlen / 2;
            let d = &mut ch.out[at..at + winlen];
            for k in 0..half {
                let (s0, s1) = (d[k], d[winlen - 1 - k]);
                let (wi, wj) = (w[k], w[winlen - 1 - k]);
                d[k] = s0 * wj - s1 * wi;
                d[winlen - 1 - k] = s0 * wi + s1 * wj;
            }
            ch.prev_block_len = len;
            ch.cur += 1;
        }
        Ok(())
    }

    fn decorrelation(&self, r: &mut Reader, n: usize) -> Vec<f32> {
        let mut m = vec![0f32; n * n];
        let rotations: Vec<usize> = (0..n * (n - 1) / 2).map(|_| r.read(6) as usize).collect();
        for i in 0..n {
            m[n * i + i] = if r.bit() { 1.0 } else { -1.0 };
        }
        let mut offset = 0;
        for i in 1..n {
            for x in 0..i {
                for y in 0..=i {
                    let (v1, v2) = (m[x * n + y], m[i * n + y]);
                    let k = rotations[offset + x];
                    let (sinv, cosv) = if k < 32 { (self.sin64[k], self.sin64[32 - k]) } else { (self.sin64[64 - k], -self.sin64[k - 32]) };
                    m[y + x * n] = v1 * sinv - v2 * cosv;
                    m[y + i * n] = v1 * cosv + v2 * sinv;
                }
            }
            offset += i;
        }
        m
    }

    fn scale_factors(&mut self, r: &mut Reader, here: &[usize], table_idx: usize, num_bands: usize) -> Result<(), Error> {
        for &c in here {
            let params = self.params.clone();
            let sf_offsets = &params.sf_offsets[table_idx][self.channels[c].table_idx];
            let ch = &mut self.channels[c];
            let (cur, other) = (ch.sf_idx, 1 - ch.sf_idx);
            ch.sf_use = other;
            if ch.reuse_sf {
                for b in 0..num_bands {
                    ch.saved_sf[other][b] = ch.saved_sf[cur][sf_offsets[b]];
                }
            }
            if ch.cur == 0 || r.bit() {
                if !ch.reuse_sf {
                    ch.sf_step = r.read(2) as i32 + 1;
                    let mut val = 45 / ch.sf_step;
                    for b in 0..num_bands {
                        val += self.tables.scale.read(r).ok_or_else(|| Error("scale factor code".into()))? as i32;
                        ch.saved_sf[other][b] = val;
                    }
                } else {
                    let mut i = 0;
                    while i < num_bands {
                        let idx = self.tables.scale_rl.read(r).ok_or_else(|| Error("scale factor run code".into()))? as usize;
                        let (skip, val, negative);
                        if idx == 0 {
                            let code = r.read(14);
                            val = (code >> 6) as i32;
                            negative = code & 1 == 0;
                            skip = ((code & 0x3f) >> 1) as usize;
                        } else if idx == 1 {
                            break;
                        } else {
                            skip = t::SCALE_RL_RUN[idx] as usize;
                            val = t::SCALE_RL_LEVEL[idx] as i32;
                            negative = !r.bit();
                        }
                        i += skip;
                        if i >= num_bands {
                            return Err(Error("scale factor run past the bands".into()));
                        }
                        ch.saved_sf[other][i] += if negative { -val } else { val };
                        i += 1;
                    }
                }
                ch.sf_idx = other;
                ch.table_idx = table_idx;
                ch.reuse_sf = true;
            }
            ch.max_sf = ch.saved_sf[other][..num_bands].iter().copied().max().unwrap_or(0);
        }
        Ok(())
    }
}

/// `ff_wma_get_large_val`: 8, 16, 24 or 31 bits after a unary length.
fn large_value(r: &mut Reader) -> u32 {
    let mut n = 8;
    if r.bit() {
        n += 8;
        if r.bit() {
            n += 8;
            if r.bit() {
                n += 7;
            }
        }
    }
    r.read(n)
}

/// A channel's coefficients for one block (`decode_coeffs`): vectors of 4, 2 or 1 values with
/// signs until a run of zeros switches to run / level coding (or until `num_vec` when it was
/// sent), the rest run / level coded.
fn coefficients(t: &Tables, r: &mut Reader, block: &mut [f32], num_vec: usize, transmit_num_vec: bool, esc_len: u32) -> Result<(), Error> {
    let len = block.len();
    let table = r.bit() as usize;
    let (run, level): (&[u16], &[f32]) = if table == 1 { (&t::COEF1_RUN, &t::COEF1_LEVEL) } else { (&t::COEF0_RUN, &t::COEF0_LEVEL) };
    let (mut rl_mode, mut cur, mut zeros) = (false, 0usize, 0usize);
    let bad = || Error("coefficient vector code".into());
    while (transmit_num_vec || !rl_mode) && cur + 3 < num_vec {
        let mut vals = [0u32; 4];
        let idx = t.vec4.read(r).ok_or_else(bad)?;
        if idx < 0 {
            for i in [0, 2] {
                let idx = t.vec2.read(r).ok_or_else(bad)?;
                if idx < 0 {
                    for k in 0..2 {
                        let mut v = t.vec1.read(r).ok_or_else(bad)? as u32;
                        if v == t::VEC1.len() as u32 - 1 {
                            v += large_value(r);
                        }
                        vals[i + k] = v;
                    }
                } else {
                    vals[i] = (idx >> 4) as u32;
                    vals[i + 1] = (idx & 15) as u32;
                }
            }
        } else {
            let idx = idx as u32;
            vals = [idx >> 12, (idx >> 8) & 15, (idx >> 4) & 15, idx & 15];
        }
        for v in vals {
            if v != 0 {
                block[cur] = if r.bit() { v as f32 } else { -(v as f32) };
                zeros = 0;
            } else {
                block[cur] = 0.0;
                zeros += 1;
                rl_mode |= zeros > len >> 8;
            }
            cur += 1;
        }
    }
    // Run / level (`ff_wma_run_level_decode`, version 1).
    let mask = len - 1;
    let mut offset = cur;
    while offset < len {
        let code = t.coef[table].read(r).ok_or_else(|| Error("coefficient run code".into()))? as usize;
        if code > 1 {
            offset += run[code] as usize;
            let l = level[code];
            block[offset & mask] = if r.bit() { l } else { -l };
        } else if code == 1 {
            break;
        } else {
            let l = large_value(r) as i32;
            if r.bit() {
                if r.bit() {
                    if r.bit() {
                        return Err(Error("a broken escape".into()));
                    }
                    offset += r.read(esc_len) as usize + 4;
                } else {
                    offset += r.read(2) as usize + 1;
                }
            }
            block[offset & mask] = if r.bit() { l as f32 } else { -l as f32 };
        }
        offset += 1;
    }
    if offset > len + 1 {
        return Err(Error("coefficients past the block".into()));
    }
    Ok(())
}

/// The inverse MDCT's middle half for `x.len()` = N coefficients: N samples
/// `y[m] = −scale · Σ x[k] cos(π/N (m + N + 1/2) (k + 1/2))` (FFmpeg's sign).
fn imdct_half(x: &[f32], y: &mut [f32], scale: f64) {
    let n = x.len();
    for (m, out) in y.iter_mut().enumerate() {
        let mut s = 0f64;
        for (k, &v) in x.iter().enumerate() {
            s += v as f64 * (std::f64::consts::PI / n as f64 * (m as f64 + n as f64 + 0.5) * (k as f64 + 0.5)).cos();
        }
        *out = (-s * scale) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::asf::{Asf, Kind};

    /// `BARREL_MOVIE`'s audio against ffmpeg's float output (a PC-only test oracle through a
    /// pipe): the largest difference and the signal-to-error ratio.
    #[test]
    #[ignore]
    fn movie_audio_against_ffmpeg() {
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let limit: usize = std::env::var("BARREL_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        let bytes = std::fs::read(&path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let audio = asf.streams.iter().find(|s| matches!(s.kind, Kind::Audio { .. })).unwrap();
        let Kind::Audio { rate, block_align, channels, extra, .. } = &audio.kind else { unreachable!() };
        let mut d = Decoder::new(Params::new(extra, *rate, *block_align as usize, *channels as usize).unwrap());
        let t0 = std::time::Instant::now();
        let mut ours: Vec<Vec<f32>> = vec![Vec::new(); d.params.channels];
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == audio.number) {
            let pcm = d.push(&o.data).unwrap();
            for (c, v) in pcm.into_iter().enumerate() {
                ours[c].extend(v);
            }
            if d.frames >= limit {
                break;
            }
        }
        let secs = t0.elapsed().as_secs_f64();
        let out = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", &path, "-map", "0:a", "-f", "f32le", "-"]).output().unwrap().stdout;
        let theirs: Vec<f32> = out.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let n = ours[0].len().min(theirs.len() / 2);
        let (mut sig, mut err, mut max, mut at, mut exact) = (0f64, 0f64, 0f32, 0usize, 0usize);
        for i in 0..n {
            for c in 0..2 {
                let (a, b) = (ours[c][i], theirs[i * 2 + c]);
                sig += (b as f64).powi(2);
                err += ((a - b) as f64).powi(2);
                exact += (a == b) as usize;
                if (a - b).abs() > max {
                    max = (a - b).abs();
                    at = i;
                }
            }
        }
        eprintln!("{} frames, {n} samples a channel (ffmpeg {}) in {secs:.2} s; max diff {max:e} at {at}, SNR {:.1} dB, {exact} of {} exact", d.frames, theirs.len() / 2, 10.0 * (sig / err.max(1e-30)).log10(), 2 * n);
        eprintln!("first samples ours {:?} ffmpeg {:?}", &ours[0][1000..1004], [theirs[2000], theirs[2002], theirs[2004], theirs[2006]]);
    }
}
