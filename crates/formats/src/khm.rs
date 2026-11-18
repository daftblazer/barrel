//! Field heightmaps (`<AREA>.khm`, magic `_MHK0100`).
//!
//! ```text
//! 32-byte header: "_MHK", "0100", u32 0, u32 file size, u16 cells across, u16 cells down,
//!                 f32 cell size, f32 lowest height, f32 highest height
//! (across + 1) × (down + 1) u32 vertices, row by row: the top 24 bits are the height,
//!   0..2^24-1 from lowest to highest; the low byte is a flag (0 or 2)
//! ```

use crate::{Error, u32_at};

#[derive(Clone, Debug)]
pub struct Heightmap {
    pub cells_x: usize,
    pub cells_z: usize,
    pub cell_size: f32,
    /// The header's lowest and highest heights.
    pub range: [f32; 2],
    pub heights: Vec<f32>,
    pub flags: Vec<u8>,
}

impl Heightmap {
    pub fn height(&self, x: usize, z: usize) -> f32 {
        self.heights[z * (self.cells_x + 1) + x]
    }

    /// The height at (x, z) in field units, bilinear between the vertices; none outside the map.
    pub fn at(&self, x: f32, z: f32) -> Option<f32> {
        let (fx, fz) = (x / self.cell_size, z / self.cell_size);
        if !(fx >= 0.0 && fz >= 0.0 && fx < self.cells_x as f32 && fz < self.cells_z as f32) {
            return None;
        }
        let (ix, iz) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let row = |z: usize| self.height(ix, z) * (1.0 - tx) + self.height(ix + 1, z) * tx;
        Some(row(iz) * (1.0 - tz) + row(iz + 1) * tz)
    }

    /// The height at (x, z) on the cell's two triangles, split along the diagonal from its
    /// (x + 1, z) corner to its (x, z + 1) corner: the ground Atelier Ryza stands a character
    /// on (her feet in a captured frame are at 6193.34 where this gives 6193.32 and the
    /// bilinear height 6191.98); none outside the map.
    pub fn at_faces(&self, x: f32, z: f32) -> Option<f32> {
        let (fx, fz) = (x / self.cell_size, z / self.cell_size);
        if !(fx >= 0.0 && fz >= 0.0 && fx < self.cells_x as f32 && fz < self.cells_z as f32) {
            return None;
        }
        let (ix, iz) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let (near, far) = (self.height(ix + 1, iz), self.height(ix, iz + 1));
        Some(if tx + tz <= 1.0 {
            let corner = self.height(ix, iz);
            corner + (near - corner) * tx + (far - corner) * tz
        } else {
            let corner = self.height(ix + 1, iz + 1);
            corner + (far - corner) * (1.0 - tx) + (near - corner) * (1.0 - tz)
        })
    }
}

pub fn parse(d: &[u8]) -> Result<Heightmap, Error> {
    if !d.starts_with(b"_MHK") {
        return Err(Error::NotThisFormat);
    }
    let dims = |at: usize| d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or(Error::Corrupt("khm header"));
    let (cells_x, cells_z) = (dims(16)?, dims(18)?);
    let f = |at| u32_at(d, at).map(f32::from_bits);
    let (cell_size, low, high) = (f(20)?, f(24)?, f(28)?);
    let count = (cells_x + 1) * (cells_z + 1);
    if cells_x == 0 || cells_z == 0 || cells_x > 4096 || cells_z > 4096 || d.len() < 32 + 4 * count {
        return Err(Error::Corrupt("khm size"));
    }
    let mut heights = Vec::with_capacity(count);
    let mut flags = Vec::with_capacity(count);
    for i in 0..count {
        let v = u32_at(d, 32 + 4 * i)?;
        heights.push(low + (v >> 8) as f32 / 16_777_215.0 * (high - low));
        flags.push(v as u8);
    }
    Ok(Heightmap { cells_x, cells_z, cell_size, range: [low, high], heights, flags })
}

#[cfg(test)]
mod tests {
    #[test]
    fn grid() {
        let mut d = b"_MHK0100".to_vec();
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        for v in [100.0f32, -10.0, 10.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        for v in [0u32, 0xffff_ff00, 0x7fff_ff02, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        let h = super::parse(&d).unwrap();
        assert_eq!(h.height(0, 0), -10.0);
        assert_eq!(h.height(1, 0), 10.0);
        assert!(h.height(0, 1).abs() < 0.01);
        assert_eq!(h.flags[2], 2);
        // Corners -10 (0,0), 10 (1,0), 0 (0,1), -10 (1,1): the cell's centre lies on the
        // diagonal between (1,0) and (0,1), at 5 on the triangles and -2.5 bilinear.
        assert!((h.at_faces(50.0, 50.0).unwrap() - 5.0).abs() < 0.01);
        assert!((h.at(50.0, 50.0).unwrap() + 2.5).abs() < 0.01);
        assert!((h.at_faces(25.0, 25.0).unwrap() + 2.5).abs() < 0.01);
        assert!((h.at_faces(75.0, 75.0).unwrap() + 2.5).abs() < 0.01);
    }
}
