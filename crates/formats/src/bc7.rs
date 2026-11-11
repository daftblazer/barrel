//! BC7 block decoding (KTGL texture format 0x5f, used by Sophie 2 and later).
//!
//! Each 4×4 block is 16 bytes. Its mode (0..7, the position of the lowest set bit) decides
//! how many subsets the block has, how the endpoints are stored and how many bits each
//! pixel's index takes. Follows the BC7 format as Direct3D 11 defines it.

struct Mode {
    subsets: usize,
    partition_bits: u32,
    rotation_bits: u32,
    index_selection_bits: u32,
    color_bits: u32,
    alpha_bits: u32,
    endpoint_pbits: bool,
    shared_pbits: bool,
    index_bits: u32,
    index2_bits: u32,
}

const fn m(s: usize, pb: u32, rb: u32, isb: u32, cb: u32, ab: u32, ep: bool, sp: bool, ib: u32, ib2: u32) -> Mode {
    Mode {
        subsets: s,
        partition_bits: pb,
        rotation_bits: rb,
        index_selection_bits: isb,
        color_bits: cb,
        alpha_bits: ab,
        endpoint_pbits: ep,
        shared_pbits: sp,
        index_bits: ib,
        index2_bits: ib2,
    }
}

const MODES: [Mode; 8] = [
    m(3, 4, 0, 0, 4, 0, true, false, 3, 0),
    m(2, 6, 0, 0, 6, 0, false, true, 3, 0),
    m(3, 6, 0, 0, 5, 0, false, false, 2, 0),
    m(2, 6, 0, 0, 7, 0, true, false, 2, 0),
    m(1, 0, 2, 1, 5, 6, false, false, 2, 3),
    m(1, 0, 2, 0, 7, 8, false, false, 2, 2),
    m(1, 0, 0, 0, 7, 7, true, false, 4, 0),
    m(2, 6, 0, 0, 5, 5, true, false, 2, 0),
];

const WEIGHTS2: [u32; 4] = [0, 21, 43, 64];
const WEIGHTS3: [u32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHTS4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

/// Pixel → subset for the 64 two-subset partitions, two bits a pixel, pixel 0 lowest.
const PARTITION2: [u32; 64] = [
    0xCCCC, 0x8888, 0xEEEE, 0xECC8, 0xC880, 0xFEEC, 0xFEC8, 0xEC80, 0xC800, 0xFFEC, 0xFE80, 0xE800, 0xFFE8, 0xFF00,
    0xFFF0, 0xF000, 0xF710, 0x008E, 0x7100, 0x08CE, 0x008C, 0x7310, 0x3100, 0x8CCE, 0x088C, 0x3110, 0x6666, 0x366C,
    0x17E8, 0x0FF0, 0x718E, 0x399C, 0xAAAA, 0xF0F0, 0x5A5A, 0x33CC, 0x3C3C, 0x55AA, 0x9696, 0xA55A, 0x73CE, 0x13C8,
    0x324C, 0x3BDC, 0x6996, 0xC33C, 0x9966, 0x0660, 0x0272, 0x04E4, 0x4E40, 0x2720, 0xC936, 0x936C, 0x39C6, 0x639C,
    0x9336, 0x9CC6, 0x817E, 0xE718, 0xCCF0, 0x0FCC, 0x7744, 0xEE22,
];

/// Pixel → subset for the 64 three-subset partitions, written out row by row.
const PARTITION3: [[u8; 16]; 64] = [
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 2, 2, 1, 2, 2, 2, 2],
    [0, 0, 0, 1, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2, 2, 1],
    [0, 0, 0, 0, 2, 0, 0, 1, 2, 2, 1, 1, 2, 2, 1, 1],
    [0, 2, 2, 2, 0, 0, 2, 2, 0, 0, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2],
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 2, 2, 0, 0, 2, 2],
    [0, 0, 2, 2, 0, 0, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2],
    [0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2],
    [0, 1, 2, 2, 0, 1, 2, 2, 0, 1, 2, 2, 0, 1, 2, 2],
    [0, 0, 1, 1, 0, 1, 1, 2, 1, 1, 2, 2, 1, 2, 2, 2],
    [0, 0, 1, 1, 2, 0, 0, 1, 2, 2, 0, 0, 2, 2, 2, 0],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 2, 1, 1, 2, 2],
    [0, 1, 1, 1, 0, 0, 1, 1, 2, 0, 0, 1, 2, 2, 0, 0],
    [0, 0, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2],
    [0, 0, 2, 2, 0, 0, 2, 2, 0, 0, 2, 2, 1, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1, 0, 2, 2, 2, 0, 2, 2, 2],
    [0, 0, 0, 1, 0, 0, 0, 1, 2, 2, 2, 1, 2, 2, 2, 1],
    [0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 2, 2, 0, 1, 2, 2],
    [0, 0, 0, 0, 1, 1, 0, 0, 2, 2, 1, 0, 2, 2, 1, 0],
    [0, 1, 2, 2, 0, 1, 2, 2, 0, 0, 1, 1, 0, 0, 0, 0],
    [0, 0, 1, 2, 0, 0, 1, 2, 1, 1, 2, 2, 2, 2, 2, 2],
    [0, 1, 1, 0, 1, 2, 2, 1, 1, 2, 2, 1, 0, 1, 1, 0],
    [0, 0, 0, 0, 0, 1, 1, 0, 1, 2, 2, 1, 1, 2, 2, 1],
    [0, 0, 2, 2, 1, 1, 0, 2, 1, 1, 0, 2, 0, 0, 2, 2],
    [0, 1, 1, 0, 0, 1, 1, 0, 2, 0, 0, 2, 2, 2, 2, 2],
    [0, 0, 1, 1, 0, 1, 2, 2, 0, 1, 2, 2, 0, 0, 1, 1],
    [0, 0, 0, 0, 2, 0, 0, 0, 2, 2, 1, 1, 2, 2, 2, 1],
    [0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 2, 2, 1, 2, 2, 2],
    [0, 2, 2, 2, 0, 0, 2, 2, 0, 0, 1, 2, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 2, 2, 0, 2, 2, 2],
    [0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2, 0],
    [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 0, 0],
    [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0],
    [0, 1, 2, 0, 2, 0, 1, 2, 1, 2, 0, 1, 0, 1, 2, 0],
    [0, 0, 1, 1, 2, 2, 0, 0, 1, 1, 2, 2, 0, 0, 1, 1],
    [0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 0, 0, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 2, 1, 2, 1, 2, 1],
    [0, 0, 2, 2, 1, 1, 2, 2, 0, 0, 2, 2, 1, 1, 2, 2],
    [0, 0, 2, 2, 0, 0, 1, 1, 0, 0, 2, 2, 0, 0, 1, 1],
    [0, 2, 2, 0, 1, 2, 2, 1, 0, 2, 2, 0, 1, 2, 2, 1],
    [0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 0, 1, 0, 1],
    [0, 0, 0, 0, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1],
    [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 2, 2, 2, 2],
    [0, 2, 2, 2, 0, 1, 1, 1, 0, 2, 2, 2, 0, 1, 1, 1],
    [0, 0, 0, 2, 1, 1, 1, 2, 0, 0, 0, 2, 1, 1, 1, 2],
    [0, 0, 0, 0, 2, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2],
    [0, 2, 2, 2, 0, 1, 1, 1, 0, 1, 1, 1, 0, 2, 2, 2],
    [0, 0, 0, 2, 1, 1, 1, 2, 1, 1, 1, 2, 0, 0, 0, 2],
    [0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 2, 2, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 2, 2, 1, 1, 2],
    [0, 1, 1, 0, 0, 1, 1, 0, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 0, 2, 2, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 2, 2],
    [0, 0, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2, 0, 0, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 2],
    [0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1],
    [0, 2, 2, 2, 1, 2, 2, 2, 0, 2, 2, 2, 1, 2, 2, 2],
    [0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 1, 1, 1, 2, 0, 1, 1, 2, 2, 0, 1, 2, 2, 2, 0],
];

/// The anchor pixel of subset 1 in each two-subset partition.
const ANCHOR2: [u8; 64] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2,
    15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, 6, 2, 6, 8, 15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15,
];

/// The anchor pixels of subsets 1 and 2 in each three-subset partition.
const ANCHOR3_1: [u8; 64] = [
    3, 3, 15, 15, 8, 3, 15, 15, 8, 8, 6, 6, 6, 5, 3, 3, 3, 3, 8, 15, 3, 3, 6, 10, 5, 8, 8, 6, 8, 5, 15, 15, 8, 15, 3,
    5, 6, 10, 8, 15, 15, 3, 15, 5, 15, 15, 15, 15, 3, 15, 5, 5, 5, 8, 5, 10, 5, 10, 8, 13, 15, 12, 3, 3,
];
const ANCHOR3_2: [u8; 64] = [
    15, 8, 8, 3, 15, 15, 3, 8, 15, 15, 15, 15, 15, 15, 15, 8, 15, 8, 15, 3, 15, 8, 15, 8, 3, 15, 6, 10, 15, 15, 10, 8,
    15, 3, 15, 10, 10, 8, 9, 10, 6, 15, 8, 15, 3, 6, 6, 8, 15, 3, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 3, 15, 15, 8,
];

struct Bits {
    v: u128,
    at: u32,
}

impl Bits {
    fn take(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let r = ((self.v >> self.at) & ((1u128 << n) - 1)) as u32;
        self.at += n;
        r
    }
}

fn subset_of(subsets: usize, partition: usize, pixel: usize) -> usize {
    match subsets {
        2 => ((PARTITION2[partition] >> pixel) & 1) as usize,
        3 => PARTITION3[partition][pixel] as usize,
        _ => 0,
    }
}

fn is_anchor(subsets: usize, partition: usize, pixel: usize) -> bool {
    pixel == 0
        || match subsets {
            2 => pixel == ANCHOR2[partition] as usize,
            3 => pixel == ANCHOR3_1[partition] as usize || pixel == ANCHOR3_2[partition] as usize,
            _ => false,
        }
}

fn weight(bits: u32, index: u32) -> u32 {
    match bits {
        2 => WEIGHTS2[index as usize],
        3 => WEIGHTS3[index as usize],
        _ => WEIGHTS4[index as usize],
    }
}

fn interpolate(a: u32, b: u32, w: u32) -> u8 {
    (((64 - w) * a + w * b + 32) >> 6) as u8
}

/// Decode one 16-byte block into 16 RGBA pixels, row by row.
pub fn decode_block(block: &[u8; 16]) -> [[u8; 4]; 16] {
    let mut bits = Bits { v: u128::from_le_bytes(*block), at: 0 };
    let Some(mode_n) = (0..8).find(|&i| block[0] & (1 << i) != 0) else {
        return [[0; 4]; 16]; // reserved: transparent black
    };
    bits.at = mode_n + 1;
    let mode = &MODES[mode_n as usize];
    let partition = bits.take(mode.partition_bits) as usize;
    let rotation = bits.take(mode.rotation_bits);
    let index_selection = bits.take(mode.index_selection_bits);
    let ends = mode.subsets * 2;
    // endpoints[e][channel]
    let mut ep = [[0u32; 4]; 6];
    for c in 0..3 {
        for e in ep.iter_mut().take(ends) {
            e[c] = bits.take(mode.color_bits);
        }
    }
    if mode.alpha_bits > 0 {
        for e in ep.iter_mut().take(ends) {
            e[3] = bits.take(mode.alpha_bits);
        }
    }
    let (mut cbits, mut abits) = (mode.color_bits, mode.alpha_bits);
    if mode.endpoint_pbits || mode.shared_pbits {
        let mut p = [0u32; 6];
        if mode.endpoint_pbits {
            for v in p.iter_mut().take(ends) {
                *v = bits.take(1);
            }
        } else {
            for s in 0..mode.subsets {
                let b = bits.take(1);
                p[2 * s] = b;
                p[2 * s + 1] = b;
            }
        }
        for (e, &pb) in ep.iter_mut().zip(&p).take(ends) {
            for c in 0..4 {
                e[c] = (e[c] << 1) | pb;
            }
        }
        cbits += 1;
        if abits > 0 {
            abits += 1;
        }
    }
    for e in ep.iter_mut().take(ends) {
        for c in 0..3 {
            e[c] = (e[c] << (8 - cbits)) | (e[c] >> (2 * cbits - 8));
        }
        e[3] = if abits == 0 { 255 } else { (e[3] << (8 - abits)) | (e[3] >> (2 * abits - 8)) };
    }
    let mut index1 = [0u32; 16];
    for (px, v) in index1.iter_mut().enumerate() {
        let n = mode.index_bits - is_anchor(mode.subsets, partition, px) as u32;
        *v = bits.take(n);
    }
    let mut index2 = [0u32; 16];
    if mode.index2_bits > 0 {
        for (px, v) in index2.iter_mut().enumerate() {
            *v = bits.take(mode.index2_bits - (px == 0) as u32);
        }
    }
    let mut out = [[0u8; 4]; 16];
    for (px, o) in out.iter_mut().enumerate() {
        let s = subset_of(mode.subsets, partition, px);
        let (a, b) = (ep[2 * s], ep[2 * s + 1]);
        let (cw, aw) = if mode.index2_bits == 0 {
            let w = weight(mode.index_bits, index1[px]);
            (w, w)
        } else {
            let w1 = weight(mode.index_bits, index1[px]);
            let w2 = weight(mode.index2_bits, index2[px]);
            if index_selection == 1 { (w2, w1) } else { (w1, w2) }
        };
        for c in 0..3 {
            o[c] = interpolate(a[c], b[c], cw);
        }
        o[3] = interpolate(a[3], b[3], aw);
        match rotation {
            1 => o.swap(0, 3),
            2 => o.swap(1, 3),
            3 => o.swap(2, 3),
            _ => {}
        }
    }
    out
}

/// Decode a BC7 image (`width`×`height`, blocks row by row) to RGBA, rows top to bottom.
pub fn decode(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let bw = width.div_ceil(4);
    for (i, block) in src.chunks_exact(16).enumerate().take(bw * height.div_ceil(4)) {
        let px = decode_block(block.try_into().unwrap());
        let (bx, by) = (i % bw * 4, i / bw * 4);
        for (k, p) in px.iter().enumerate() {
            let (x, y) = (bx + k % 4, by + k / 4);
            if x < width && y < height {
                out[(y * width + x) * 4..][..4].copy_from_slice(p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_are_in_their_subsets() {
        for p in 0..64 {
            assert_eq!(subset_of(2, p, ANCHOR2[p] as usize), 1, "partition2 {p}");
            assert_eq!(subset_of(3, p, ANCHOR3_1[p] as usize), 1, "partition3 {p} subset 1");
            assert_eq!(subset_of(3, p, ANCHOR3_2[p] as usize), 2, "partition3 {p} subset 2");
            assert_eq!(subset_of(2, p, 0), 0);
            assert_eq!(subset_of(3, p, 0), 0);
        }
    }

    #[test]
    fn mode6_solid() {
        // Mode 6 (bit 6 set): both endpoints the same colour, so every pixel is that colour.
        // R0 R1 G0 G1 B0 B1 A0 A1 are 7 bits each, then two p-bits, then the indices.
        let mut v: u128 = 1 << 6;
        let mut at = 7;
        let mut put = |x: u128, n: u32| {
            v |= x << at;
            at += n;
        };
        for c in [100u128, 50, 25, 127] {
            put(c, 7);
            put(c, 7);
        }
        put(1, 1);
        put(1, 1);
        let px = decode_block(&v.to_le_bytes());
        // 7-bit 100 with p-bit 1 is 201.
        assert!(px.iter().all(|p| *p == [201, 101, 51, 255]), "{:?}", px[0]);
    }
}
