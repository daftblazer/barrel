//! A minimal PNG writer for tools: 8-bit RGBA, stored (uncompressed) deflate blocks.

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode `rgba` (width×height×4 bytes, rows top to bottom) as a PNG file.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row = width as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * row..(y + 1) * row]);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(b) = blocks.next() {
        z.push(blocks.peek().is_none() as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn idat_inflates_back() {
        let px = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let png = super::encode_rgba(2, 1, &px);
        let idat = png.windows(4).position(|w| w == b"IDAT").unwrap();
        let len = u32::from_be_bytes(png[idat - 4..idat].try_into().unwrap()) as usize;
        let raw = crate::inflate::zlib_decompress(&png[idat + 4..idat + 4 + len], 64).unwrap();
        assert_eq!(raw, [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }
}

/// A decoded 8-bit PNG: `channels` (3 or 4) bytes per pixel, rows top to bottom.
pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub pixels: Vec<u8>,
}

/// Decode an 8-bit RGB or RGBA PNG that is not interlaced.
pub fn decode(b: &[u8]) -> Result<Decoded, String> {
    if b.get(..8) != Some(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10]) {
        return Err("not a PNG".into());
    }
    let (mut w, mut h, mut channels) = (0usize, 0usize, 0usize);
    let mut data = Vec::new();
    let mut at = 8;
    while at + 8 <= b.len() {
        let len = u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize;
        let body = b.get(at + 8..at + 8 + len).ok_or("truncated PNG")?;
        match &b[at + 4..at + 8] {
            b"IHDR" => {
                w = u32::from_be_bytes(body[0..4].try_into().unwrap()) as usize;
                h = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                if body[8] != 8 || body[12] != 0 {
                    return Err("only 8-bit PNGs that are not interlaced".into());
                }
                channels = match body[9] {
                    2 => 3,
                    6 => 4,
                    t => return Err(format!("PNG colour type {t}")),
                };
            }
            b"IDAT" => data.extend_from_slice(body),
            _ => {}
        }
        at += 12 + len;
    }
    let raw = crate::inflate::zlib_decompress(&data, (w * channels + 1) * h).map_err(|e| format!("{e:?}"))?;
    let stride = w * channels;
    let mut rows = vec![0u8; stride * h];
    for y in 0..h {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let (filter, line) = (line[0], &line[1..]);
        for i in 0..stride {
            let left = if i >= channels { rows[y * stride + i - channels] } else { 0 };
            let up = if y > 0 { rows[(y - 1) * stride + i] } else { 0 };
            let corner = if y > 0 && i >= channels { rows[(y - 1) * stride + i - channels] } else { 0 };
            let predicted = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => {
                    let (p, a, bb, c) = (i32::from(left) + i32::from(up) - i32::from(corner), i32::from(left), i32::from(up), i32::from(corner));
                    let (pa, pb, pc) = ((p - a).abs(), (p - bb).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc { left } else if pb <= pc { up } else { corner }
                }
                f => return Err(format!("PNG filter {f}")),
            };
            rows[y * stride + i] = line[i].wrapping_add(predicted);
        }
    }
    Ok(Decoded { width: w, height: h, channels, pixels: rows })
}
