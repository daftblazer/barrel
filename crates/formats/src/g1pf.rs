//! A field's walkable surface, `<field>.g1pf` (`FP1G`; the Ryza games): a mesh of convex
//! polygons over the ground plane (x, z), with each polygon's neighbours. The edges no polygon
//! lies across are the borders of where a character may go (the game keeps Ryza 28 units, her
//! collision radius, from them: `docs/formats.md`, "Ryza's walkable mesh").
//!
//! ```text
//! 0x00 "FP1G", 01 02 02 01, u32 0, u32 file size, f32 128000, f32 128000
//! 0x18 u32 layer count (2); then per layer 12 u32: vertices, indices, indices again,
//!      polygons, and eight more counts of the tables after the polygons (not read);
//!      then six more u32 (not read)
//! 0x94 layer 0: vertices (x, z) f32 pairs; indices u32 (each polygon's corners in turn);
//!      neighbours u32 (the polygon across the edge from each corner to the next, 0xffffffff
//!      for none); polygons, 36 bytes each: u32 first corner, u32 corner count (3 to 5),
//!      the first corner again, u32 (0 or a small number: not read), five f32 (not read)
//! ```
//!
//! Layer 1 and the tables after the polygons are not read.

use crate::{Error, u32_at};

/// The mesh's first layer.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub vertices: Vec<[f32; 2]>,
    /// Each polygon's corners, indices into `vertices`, in order around it.
    pub polygons: Vec<Vec<u32>>,
    /// For each polygon, the polygon across each of its edges (corner `i` to `i + 1`), if any.
    pub neighbours: Vec<Vec<Option<u32>>>,
}

const NONE: u32 = 0xffff_ffff;
const FIRST_COUNTS: usize = 0x1c;
const DATA: usize = 0x94;
const POLYGON_SIZE: usize = 36;

pub fn parse(b: &[u8]) -> Result<Mesh, Error> {
    if b.get(..4) != Some(b"FP1G") {
        return Err(Error::Corrupt("g1pf: not FP1G"));
    }
    let count = |k: usize| u32_at(b, FIRST_COUNTS + 4 * k).map(|v| v as usize);
    let (vertices, indices, neighbours, polygons) = (count(0)?, count(1)?, count(2)?, count(3)?);
    if indices != neighbours {
        return Err(Error::Corrupt("g1pf: corners and neighbours differ in number"));
    }
    let at_indices = DATA + vertices * 8;
    let at_neighbours = at_indices + indices * 4;
    let at_polygons = at_neighbours + neighbours * 4;
    if b.len() < at_polygons + polygons * POLYGON_SIZE {
        return Err(Error::Corrupt("g1pf: tables past the end"));
    }
    let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let vertices: Vec<[f32; 2]> = (0..vertices).map(|v| [f(DATA + v * 8), f(DATA + v * 8 + 4)]).collect();
    let (mut corners, mut across) = (Vec::with_capacity(polygons), Vec::with_capacity(polygons));
    for p in 0..polygons {
        let (first, n) = (u32_at(b, at_polygons + p * POLYGON_SIZE)? as usize, u32_at(b, at_polygons + p * POLYGON_SIZE + 4)? as usize);
        if first + n > indices {
            return Err(Error::Corrupt("g1pf: a polygon's corners past the table"));
        }
        let ring = (first..first + n).map(|i| u32_at(b, at_indices + i * 4)).collect::<Result<Vec<_>, _>>()?;
        if ring.iter().any(|&v| v as usize >= vertices.len()) {
            return Err(Error::Corrupt("g1pf: a corner is not a vertex"));
        }
        let next = (first..first + n).map(|i| u32_at(b, at_neighbours + i * 4).map(|v| (v != NONE).then_some(v))).collect::<Result<Vec<_>, _>>()?;
        corners.push(ring);
        across.push(next);
    }
    Ok(Mesh { vertices, polygons: corners, neighbours: across })
}

impl Mesh {
    /// The edges with no polygon across (from, to, in the polygon's order around), on the ground (x, z).
    pub fn borders(&self) -> Vec<([f32; 2], [f32; 2])> {
        let mut out = Vec::new();
        for (ring, across) in self.polygons.iter().zip(&self.neighbours) {
            for (i, a) in across.iter().enumerate() {
                if a.is_none() {
                    out.push((self.vertices[ring[i] as usize], self.vertices[ring[(i + 1) % ring.len()] as usize]));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles sharing an edge: a square cut along its diagonal.
    fn square() -> Vec<u8> {
        let mut b = b"FP1G\x01\x02\x02\x01".to_vec();
        b.resize(DATA, 0);
        let counts = [4u32, 6, 6, 2];
        for (k, c) in counts.iter().enumerate() {
            b[FIRST_COUNTS + 4 * k..FIRST_COUNTS + 4 * k + 4].copy_from_slice(&c.to_le_bytes());
        }
        for v in [[0.0f32, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]] {
            b.extend(v[0].to_le_bytes());
            b.extend(v[1].to_le_bytes());
        }
        for i in [0u32, 1, 2, 0, 2, 3] {
            b.extend(i.to_le_bytes());
        }
        for n in [NONE, NONE, 1, 0, NONE, NONE] {
            b.extend(n.to_le_bytes());
        }
        for (first, n) in [(0u32, 3u32), (3, 3)] {
            for w in [first, n, first, 0] {
                b.extend(w.to_le_bytes());
            }
            b.extend([0u8; 20]);
        }
        b
    }

    #[test]
    fn the_borders_are_the_edges_with_no_neighbour() {
        let m = parse(&square()).unwrap();
        assert_eq!(m.polygons, vec![vec![0, 1, 2], vec![0, 2, 3]]);
        // The diagonal is shared; the square's four sides are borders.
        let b = m.borders();
        assert_eq!(b.len(), 4);
        assert!(b.contains(&([0.0, 0.0], [10.0, 0.0])) && b.contains(&([10.0, 10.0], [0.0, 10.0])));
    }

    #[test]
    fn a_short_file_is_an_error() {
        let b = square();
        assert!(parse(&b[..b.len() - 1]).is_err());
        assert!(parse(b"FP1X").is_err());
    }
}
