//! BC6H (BPTC float): 16-bit float RGB in 4×4 blocks of 16 bytes, the format of the later
//! games' HDR environment maps (`docs/formats.md`, "Textures"). Written from the BPTC_FLOAT
//! specification: 14 modes, one or two regions, endpoints stored either whole or as a base
//! and signed deltas.

/// Mode: (endpoint bits, delta bits r g b, transformed, regions, bit layout after the mode
/// bits). The layout lists fields in stream order: `field:bit` or `field:from-to` (bits read
/// from the stream in that order, so `r0:11-10` is reversed); fields are r0 g0 b0 (the first
/// endpoint), r1 g1 b1, r2 g2 b2, r3 g3 b3.
struct Mode {
    bits: u32,
    delta: [u32; 3],
    transformed: bool,
    regions: usize,
    layout: &'static str,
}

const MODES: [Mode; 14] = [
    Mode { bits: 10, delta: [5, 5, 5], transformed: true, regions: 2, layout: "g2:4 b2:4 b3:4 r0:0-9 g0:0-9 b0:0-9 r1:0-4 g3:4 g2:0-3 g1:0-4 b3:0 g3:0-3 b1:0-4 b3:1 b2:0-3 r2:0-4 b3:2 r3:0-4 b3:3" },
    Mode { bits: 7, delta: [6, 6, 6], transformed: true, regions: 2, layout: "g2:5 g3:4 g3:5 r0:0-6 b3:0 b3:1 b2:4 g0:0-6 b2:5 b3:2 g2:4 b0:0-6 b3:3 b3:5 b3:4 r1:0-5 g2:0-3 g1:0-5 g3:0-3 b1:0-5 b2:0-3 r2:0-5 r3:0-5" },
    Mode { bits: 11, delta: [5, 4, 4], transformed: true, regions: 2, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-4 r0:10 g2:0-3 g1:0-3 g0:10 b3:0 g3:0-3 b1:0-3 b0:10 b3:1 b2:0-3 r2:0-4 b3:2 r3:0-4 b3:3" },
    Mode { bits: 11, delta: [4, 5, 4], transformed: true, regions: 2, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-3 r0:10 g3:4 g2:0-3 g1:0-4 g0:10 g3:0-3 b1:0-3 b0:10 b3:1 b2:0-3 r2:0-3 b3:0 b3:2 r3:0-3 g2:4 b3:3" },
    Mode { bits: 11, delta: [4, 4, 5], transformed: true, regions: 2, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-3 r0:10 b2:4 g2:0-3 g1:0-3 g0:10 b3:0 g3:0-3 b1:0-4 b0:10 b2:0-3 r2:0-3 b3:1 b3:2 r3:0-3 b3:4 b3:3" },
    Mode { bits: 9, delta: [5, 5, 5], transformed: true, regions: 2, layout: "r0:0-8 b2:4 g0:0-8 g2:4 b0:0-8 b3:4 r1:0-4 g3:4 g2:0-3 g1:0-4 b3:0 g3:0-3 b1:0-4 b3:1 b2:0-3 r2:0-4 b3:2 r3:0-4 b3:3" },
    Mode { bits: 8, delta: [6, 5, 5], transformed: true, regions: 2, layout: "r0:0-7 g3:4 b2:4 g0:0-7 b3:2 g2:4 b0:0-7 b3:3 b3:4 r1:0-5 g2:0-3 g1:0-4 b3:0 g3:0-3 b1:0-4 b3:1 b2:0-3 r2:0-5 r3:0-5" },
    Mode { bits: 8, delta: [5, 6, 5], transformed: true, regions: 2, layout: "r0:0-7 b3:0 b2:4 g0:0-7 g2:5 g2:4 b0:0-7 g3:5 b3:4 r1:0-4 g3:4 g2:0-3 g1:0-5 g3:0-3 b1:0-4 b3:1 b2:0-3 r2:0-4 b3:2 r3:0-4 b3:3" },
    Mode { bits: 8, delta: [5, 5, 6], transformed: true, regions: 2, layout: "r0:0-7 b3:1 b2:4 g0:0-7 b2:5 g2:4 b0:0-7 b3:5 b3:4 r1:0-4 g3:4 g2:0-3 g1:0-4 b3:0 g3:0-3 b1:0-5 b2:0-3 r2:0-4 b3:2 r3:0-4 b3:3" },
    Mode { bits: 6, delta: [6, 6, 6], transformed: false, regions: 2, layout: "r0:0-5 g3:4 b3:0 b3:1 b2:4 g0:0-5 g2:5 b2:5 b3:2 g2:4 b0:0-5 g3:5 b3:3 b3:5 b3:4 r1:0-5 g2:0-3 g1:0-5 g3:0-3 b1:0-5 b2:0-3 r2:0-5 r3:0-5" },
    Mode { bits: 10, delta: [10, 10, 10], transformed: false, regions: 1, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-9 g1:0-9 b1:0-9" },
    Mode { bits: 11, delta: [9, 9, 9], transformed: true, regions: 1, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-8 r0:10 g1:0-8 g0:10 b1:0-8 b0:10" },
    Mode { bits: 12, delta: [8, 8, 8], transformed: true, regions: 1, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-7 r0:11-10 g1:0-7 g0:11-10 b1:0-7 b0:11-10" },
    Mode { bits: 16, delta: [4, 4, 4], transformed: true, regions: 1, layout: "r0:0-9 g0:0-9 b0:0-9 r1:0-3 r0:15-10 g1:0-3 g0:15-10 b1:0-3 b0:15-10" },
];

struct Bits<'a> {
    data: &'a [u8; 16],
    pos: u32,
}

impl Bits<'_> {
    fn bit(&mut self) -> u32 {
        let b = (self.data[(self.pos / 8) as usize] >> (self.pos % 8)) & 1;
        self.pos += 1;
        b as u32
    }
    fn bits(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |v, i| v | (self.bit() << i))
    }
}

fn sign_extend(v: u32, bits: u32) -> i32 {
    let s = 32 - bits;
    ((v << s) as i32) >> s
}

fn unquantize(v: i32, bits: u32, signed: bool) -> i32 {
    if !signed {
        if bits >= 15 || v == 0 {
            v
        } else if v == (1 << bits) - 1 {
            0xffff
        } else {
            ((v << 16) + 0x8000) >> bits
        }
    } else {
        if bits >= 16 {
            return v;
        }
        let (neg, a) = (v < 0, v.abs());
        let u = if a == 0 {
            0
        } else if a >= (1 << (bits - 1)) - 1 {
            0x7fff
        } else {
            ((a << 15) + 0x4000) >> (bits - 1)
        };
        if neg { -u } else { u }
    }
}

/// A half float's bits to f32.
pub fn half_to_f32(h: u16) -> f32 {
    let (s, e, m) = ((h >> 15) as u32, ((h >> 10) & 0x1f) as u32, (h & 0x3ff) as u32);
    let f = if e == 0 {
        (m as f32) * 2f32.powi(-24)
    } else if e == 31 {
        if m == 0 { f32::INFINITY } else { f32::NAN }
    } else {
        f32::from_bits(((e + 112) << 23) | (m << 13))
    };
    if s == 1 { -f } else { f }
}

/// One block's 16 pixels as half floats (RGB), `signed` for the signed variant.
pub fn decode_block(block: &[u8; 16], signed: bool) -> [[u16; 3]; 16] {
    let mut r = Bits { data: block, pos: 0 };
    let low = r.bits(2);
    let index = if low < 2 {
        low as usize
    } else {
        let m = low | (r.bits(3) << 2);
        match m {
            2 => 2,
            6 => 3,
            10 => 4,
            14 => 5,
            18 => 6,
            22 => 7,
            26 => 8,
            30 => 9,
            3 => 10,
            7 => 11,
            11 => 12,
            15 => 13,
            _ => return [[0; 3]; 16], // reserved modes decode to black
        }
    };
    let mode = &MODES[index];
    // e[endpoint][channel]
    let mut e = [[0u32; 3]; 4];
    for item in mode.layout.split(' ') {
        let (field, range) = item.split_once(':').unwrap();
        let ch = match &field[..1] {
            "r" => 0,
            "g" => 1,
            _ => 2,
        };
        let ep: usize = field[1..].parse().unwrap();
        let (a, b): (i32, i32) = match range.split_once('-') {
            Some((a, b)) => (a.parse().unwrap(), b.parse().unwrap()),
            None => {
                let a = range.parse().unwrap();
                (a, a)
            }
        };
        let step = if b >= a { 1 } else { -1 };
        let mut i = a;
        loop {
            e[ep][ch] |= r.bit() << i;
            if i == b {
                break;
            }
            i += step;
        }
    }
    let partition = if mode.regions == 2 { r.bits(5) as usize } else { 0 };
    let ends = mode.regions * 2;
    // Endpoints as signed values in the mode's precision.
    let mut v = [[0i32; 3]; 4];
    let mask = (1u32 << mode.bits) - 1;
    for c in 0..3 {
        let base = e[0][c];
        v[0][c] = if signed { sign_extend(base, mode.bits) } else { base as i32 };
        for k in 1..ends {
            let raw = if mode.transformed {
                let d = sign_extend(e[k][c], mode.delta[c]);
                (base as i32).wrapping_add(d) as u32 & mask
            } else {
                e[k][c]
            };
            v[k][c] = if signed || mode.transformed && signed { sign_extend(raw, mode.bits) } else { raw as i32 };
        }
    }
    let q: Vec<[i32; 3]> = v[..ends].iter().map(|p| p.map(|x| unquantize(x, mode.bits, signed))).collect();
    let index_bits = if mode.regions == 2 { 3 } else { 4 };
    let weights: &[i32] = if index_bits == 3 { &[0, 9, 18, 27, 37, 46, 55, 64] } else { &[0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64] };
    let mut out = [[0u16; 3]; 16];
    for (p, px) in out.iter_mut().enumerate() {
        let subset = if mode.regions == 2 { crate::bc7::subset_of(2, partition, p) } else { 0 };
        let anchor = crate::bc7::is_anchor(mode.regions, partition, p);
        let i = r.bits(if anchor { index_bits - 1 } else { index_bits }) as usize;
        let w = weights[i];
        for c in 0..3 {
            let (a, b) = (q[2 * subset][c], q[2 * subset + 1][c]);
            let x = ((64 - w) * a + w * b + 32) >> 6;
            let h = if signed {
                if x < 0 { (((-x) * 31) >> 5) as u16 | 0x8000 } else { ((x * 31) >> 5) as u16 }
            } else {
                ((x * 31) >> 6) as u16
            };
            px[c] = h;
        }
    }
    out
}

/// A whole image as RGBA8 for viewing: each channel mapped by x / (1 + x) (Reinhard), then
/// gamma 2.2.
pub fn decode(src: &[u8], width: usize, height: usize, signed: bool) -> Vec<u8> {
    decode_rgb(src, width, height, signed)
        .into_iter()
        .flat_map(|p| {
            let t = |f: f32| ((f.max(0.0) / (1.0 + f.max(0.0))).powf(1.0 / 2.2) * 255.0 + 0.5) as u8;
            [t(p[0]), t(p[1]), t(p[2]), 255]
        })
        .collect()
}

/// An image's linear RGB values, rows top to bottom.
pub fn decode_rgb(src: &[u8], width: usize, height: usize, signed: bool) -> Vec<[f32; 3]> {
    let mut out = vec![[0f32; 3]; width * height];
    let bw = width.div_ceil(4);
    for (b, block) in src.chunks_exact(16).enumerate() {
        let (bx, by) = (b % bw * 4, b / bw * 4);
        if by >= height {
            break;
        }
        let px = decode_block(block.try_into().unwrap(), signed);
        for (i, p) in px.iter().enumerate() {
            let (x, y) = (bx + i % 4, by + i / 4);
            if x < width && y < height {
                out[y * width + x] = p.map(half_to_f32);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mode 11 (one region, whole 10-bit endpoints): black to white; the anchor pixel's index
    /// `anchor` (3 bits), every other pixel's `rest` (4 bits).
    fn mode11_block(anchor: u32, rest: u32) -> [u8; 16] {
        let mut bits: Vec<u32> = vec![1, 1, 0, 0, 0]; // 00011, low bit first
        let put = |v: u32, n: u32, bits: &mut Vec<u32>| (0..n).for_each(|i| bits.push((v >> i) & 1));
        for _ in 0..3 {
            put(0, 10, &mut bits);
        }
        for _ in 0..3 {
            put(1023, 10, &mut bits);
        }
        put(anchor, 3, &mut bits);
        for _ in 1..16 {
            put(rest, 4, &mut bits);
        }
        let mut b = [0u8; 16];
        for (i, &v) in bits.iter().enumerate() {
            b[i / 8] |= (v as u8) << (i % 8);
        }
        b
    }

    #[test]
    fn mode_11_ends() {
        let px = decode_block(&mode11_block(0, 15), false);
        assert_eq!(px[0], [0, 0, 0]);
        // 1023 unquantizes to 0xffff; index 15 (weight 64) gives it, * 31 >> 6 = 0x7bff.
        assert_eq!(px[1], [0x7bff; 3]);
        // Index 7, weight 30: (30 * 0xffff + 32) >> 6 = 30720, * 31 >> 6 = 14880.
        assert_eq!(decode_block(&mode11_block(7, 7), false)[0][0], 14880);
        assert_eq!(half_to_f32(0x7bff), 65504.0);
        assert_eq!(half_to_f32(0x3c00), 1.0);
    }
}
