//! A field's plants, `<FIELD>_grass.bin` in its main container (docs/formats.md, "`_grass.bin`
//! and `_grass.g1t`"): the area's size, a draw distance, the plant kinds' records (5 floats
//! each, their meaning not settled) and a grid of 500-unit cells, each naming the kind that
//! grows there (0xff: none). The game builds the plants' quads on the CPU from it and draws
//! them with `ObjectGrassPBS` over the cards in `_grass.g1t`.

/// A cell with no plants.
pub const NONE: u32 = 0xff;
/// Where the kinds' records begin (after the header's seven words).
const RECORDS: usize = 0x1c;

#[derive(Debug, Clone, PartialEq)]
pub struct Grass {
    /// The area the grid covers from the origin, x and z, in game units.
    pub area: [f32; 2],
    /// Likely the draw distance (Rasenboden: 30000).
    pub distance: u32,
    /// The header's other float (Rasenboden: 0.01), use not known.
    pub unknown: f32,
    /// The grid's cells along x and z.
    pub size: [u32; 2],
    /// Each kind's five floats (sizes in units and, last, a density; not settled).
    pub kinds: Vec<[f32; 5]>,
    /// The grid, x fastest: each cell's kind, or `NONE`.
    pub cells: Vec<u32>,
}

impl Grass {
    pub fn parse(b: &[u8]) -> Result<Grass, String> {
        if b.len() < RECORDS {
            return Err("grass: shorter than its header".into());
        }
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let size = [u(0x10), u(0x14)];
        let count = u(0x18) as usize;
        let cells = (size[0] as usize).checked_mul(size[1] as usize).ok_or("grass: grid too large")?;
        // The grid ends the file; the records sit between the header and it, zero-padded.
        let grid = b.len().checked_sub(cells * 4).ok_or_else(|| format!("grass: {}×{} grid needs {} bytes, {} there", size[0], size[1], cells * 4, b.len()))?;
        if grid < RECORDS + count * 20 {
            return Err(format!("grass: {count} kinds do not fit before the grid at {grid:#x}"));
        }
        Ok(Grass {
            area: [f(0), f(4)],
            distance: u(8),
            unknown: f(0xc),
            size,
            kinds: (0..count).map(|k| std::array::from_fn(|i| f(RECORDS + k * 20 + i * 4))).collect(),
            cells: (0..cells).map(|c| u(grid + c * 4)).collect(),
        })
    }

    /// A cell's size along x and z, in game units.
    pub fn cell(&self) -> [f32; 2] {
        [self.area[0] / self.size[0].max(1) as f32, self.area[1] / self.size[1].max(1) as f32]
    }

    /// The kind growing at game point (x, z), if any.
    pub fn kind_at(&self, x: f32, z: f32) -> Option<u32> {
        let [cx, cz] = self.cell();
        if x < 0.0 || z < 0.0 {
            return None;
        }
        let (i, j) = ((x / cx) as usize, (z / cz) as usize);
        if i >= self.size[0] as usize || j >= self.size[1] as usize {
            return None;
        }
        Some(self.cells[j * self.size[0] as usize + i]).filter(|&k| k != NONE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(kinds: &[[f32; 5]], cells: &[u32], size: u32, pad: usize) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [128000.0f32, 128000.0] {
            b.extend(v.to_le_bytes());
        }
        b.extend(30000u32.to_le_bytes());
        b.extend(0.01f32.to_le_bytes());
        b.extend(size.to_le_bytes());
        b.extend(size.to_le_bytes());
        b.extend((kinds.len() as u32).to_le_bytes());
        for k in kinds {
            for v in k {
                b.extend(v.to_le_bytes());
            }
        }
        b.extend(vec![0u8; pad]);
        for c in cells {
            b.extend(c.to_le_bytes());
        }
        b
    }

    #[test]
    fn reads_the_header_kinds_and_grid() {
        let kinds = [[50.0, 100.0, 60.0, 40.0, 0.1], [110.0, 30.0, 60.0, 60.0, 0.1]];
        let cells = [NONE, 1, 0, NONE];
        let g = Grass::parse(&file(&kinds, &cells, 2, 12)).unwrap();
        assert_eq!((g.area, g.distance, g.size), ([128000.0; 2], 30000, [2, 2]));
        assert_eq!(g.kinds, kinds.to_vec());
        assert_eq!(g.cells, cells.to_vec());
        assert_eq!(g.cell(), [64000.0, 64000.0]);
        assert_eq!(g.kind_at(70000.0, 10.0), Some(1));
        assert_eq!(g.kind_at(10.0, 70000.0), Some(0));
        assert_eq!(g.kind_at(10.0, 10.0), None);
        assert_eq!(g.kind_at(-1.0, 10.0), None);
        assert_eq!(g.kind_at(200000.0, 10.0), None);
    }

    #[test]
    fn refuses_a_grid_that_does_not_fit() {
        let b = file(&[[1.0; 5]], &[0; 3], 2, 0);
        assert!(Grass::parse(&b).is_err());
    }

    /// Rasenboden's file, when the install is at hand (`BARREL_RYZA_GRASS` names an unpacked
    /// `S06_T_RASEN_grass.bin`).
    #[test]
    fn rasenboden() {
        let Some(p) = std::env::var_os("BARREL_RYZA_GRASS") else { return };
        let g = Grass::parse(&std::fs::read(p).unwrap()).unwrap();
        assert_eq!((g.size, g.kinds.len(), g.distance), ([256, 256], 19, 30000));
        assert_eq!(g.cells.iter().filter(|&&c| c != NONE).count(), 1446);
        assert_eq!(g.cells.iter().filter(|&&c| c == 7).count(), 785);
    }
}
