//! Software decoding of BC1 and BC3 (DXT1, DXT5) to 8-bit RGBA, for tools and tests. The
//! renderer uploads the blocks to the GPU as they are.

fn rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 31) as u32;
    let g = ((c >> 5) & 63) as u32;
    let b = (c & 31) as u32;
    [((r * 527 + 23) >> 6) as u8, ((g * 259 + 33) >> 6) as u8, ((b * 527 + 23) >> 6) as u8]
}

/// The 16 colours of a BC1 colour block. `alpha_mode` allows the 3-colour + transparent
/// form, which BC1 uses but BC3's colour half does not.
fn color_block(b: &[u8], alpha_mode: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([b[0], b[1]]);
    let c1 = u16::from_le_bytes([b[2], b[3]]);
    let (p0, p1) = (rgb565(c0), rgb565(c1));
    let mix = |a: u8, b: u8, wa: u32, wb: u32, d: u32| ((a as u32 * wa + b as u32 * wb) / d) as u8;
    let mut pal = [[0u8; 4]; 4];
    pal[0] = [p0[0], p0[1], p0[2], 255];
    pal[1] = [p1[0], p1[1], p1[2], 255];
    if c0 > c1 || !alpha_mode {
        for i in 0..3 {
            pal[2][i] = mix(p0[i], p1[i], 2, 1, 3);
            pal[3][i] = mix(p0[i], p1[i], 1, 2, 3);
        }
        pal[2][3] = 255;
        pal[3][3] = 255;
    } else {
        for i in 0..3 {
            pal[2][i] = mix(p0[i], p1[i], 1, 1, 2);
        }
        pal[2][3] = 255;
        pal[3] = [0, 0, 0, 0];
    }
    let bits = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let mut out = [[0u8; 4]; 16];
    for (i, px) in out.iter_mut().enumerate() {
        *px = pal[((bits >> (2 * i)) & 3) as usize];
    }
    out
}

fn alpha_block(b: &[u8]) -> [u8; 16] {
    let (a0, a1) = (b[0] as u32, b[1] as u32);
    let mut pal = [0u8; 8];
    pal[0] = a0 as u8;
    pal[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7 {
            pal[i + 1] = (((7 - i) as u32 * a0 + i as u32 * a1) / 7) as u8;
        }
    } else {
        for i in 1..5 {
            pal[i + 1] = (((5 - i) as u32 * a0 + i as u32 * a1) / 5) as u8;
        }
        pal[6] = 0;
        pal[7] = 255;
    }
    let mut bits = 0u64;
    for (i, &x) in b[2..8].iter().enumerate() {
        bits |= (x as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        *a = pal[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

/// Decode a BC4 (one channel, shown grey) or BC5 (two channels, red and green) level.
pub fn decode_bc45(src: &[u8], width: usize, height: usize, bc5: bool) -> Vec<u8> {
    let block = if bc5 { 16 } else { 8 };
    let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
    let mut out = vec![0u8; width * height * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let at = (by * bw + bx) * block;
            let Some(b) = src.get(at..at + block) else { return out };
            let r = alpha_block(&b[..8]);
            let g = if bc5 { Some(alpha_block(&b[8..16])) } else { None };
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= width || y >= height {
                    continue;
                }
                let px = match g {
                    Some(g) => [r[i], g[i], 0, 255],
                    None => [r[i], r[i], r[i], 255],
                };
                out[(y * width + x) * 4..][..4].copy_from_slice(&px);
            }
        }
    }
    out
}

/// Decode a level of `width`×`height` pixels. `bc3` selects BC3; otherwise BC1.
pub fn decode(src: &[u8], width: usize, height: usize, bc3: bool) -> Vec<u8> {
    let block = if bc3 { 16 } else { 8 };
    let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
    let mut out = vec![0u8; width * height * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let at = (by * bw + bx) * block;
            let Some(b) = src.get(at..at + block) else { return out };
            let (colors, alpha) = if bc3 {
                (color_block(&b[8..], false), Some(alpha_block(&b[..8])))
            } else {
                (color_block(b, true), None)
            };
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= width || y >= height {
                    continue;
                }
                let mut px = colors[i];
                if let Some(a) = alpha {
                    px[3] = a[i];
                }
                out[(y * width + x) * 4..][..4].copy_from_slice(&px);
            }
        }
    }
    out
}

/// Decode a BC2 (DXT3) level: each 16-byte block an explicit alpha (4 bits a pixel, from the
/// low bits of the first eight bytes, little-endian), then a BC1 colour block (always four
/// colours). Ayesha DX's PSSG textures use it.
pub fn decode_bc2(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
    let mut out = vec![0u8; width * height * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let at = (by * bw + bx) * 16;
            let Some(b) = src.get(at..at + 16) else { return out };
            let colors = color_block(&b[8..], false);
            let alpha = u64::from_le_bytes(b[..8].try_into().unwrap());
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= width || y >= height {
                    continue;
                }
                let mut px = colors[i];
                px[3] = ((alpha >> (4 * i)) & 0xf) as u8 * 17;
                out[(y * width + x) * 4..][..4].copy_from_slice(&px);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn bc2_takes_its_alpha_as_written() {
        // Alpha nibbles 0, 1, ..., 15 in pixel order; a plain white colour block.
        let mut b = [0u8; 16];
        for i in 0..8 {
            b[i] = (2 * i as u8) | ((2 * i as u8 + 1) << 4);
        }
        b[8..12].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        let px = decode_bc2(&b, 4, 4);
        assert_eq!((px[3], px[4 * 1 + 3], px[4 * 15 + 3]), (0, 17, 255));
        assert_eq!(&px[..3], &[255, 255, 255]);
    }

    use super::*;

    #[test]
    fn solid_blocks() {
        // BC1: colour 0 pure red (0xf800), all indices 0.
        let red = [0x00, 0xf8, 0, 0, 0, 0, 0, 0];
        let px = decode(&red, 4, 4, false);
        assert_eq!(&px[..4], &[255, 0, 0, 255]);
        // BC1 transparent: c0 <= c1 and index 3.
        let clear = [0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert_eq!(&decode(&clear, 4, 4, false)[..4], &[0, 0, 0, 0]);
        // BC3: alpha 128 everywhere (a0 = 128, indices 0), colour white.
        let mut b = [128u8, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0];
        b[8..10].copy_from_slice(&0xffffu16.to_le_bytes());
        assert_eq!(&decode(&b, 4, 4, true)[..4], &[255, 255, 255, 128]);
    }
}
