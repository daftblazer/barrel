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

/// One plant: where its foot stands (game units), its size and which kind it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plant {
    pub at: [f32; 3],
    /// Width and height, game units.
    pub size: [f32; 2],
    pub kind: u32,
}

/// A small fixed hash to a number in [0, 1), so a field's plants come out the same each run.
fn unit(mut v: u32) -> f32 {
    v ^= v >> 16;
    v = v.wrapping_mul(0x7feb_352d);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846c_a68b);
    v ^= v >> 16;
    (v >> 8) as f32 / (1u32 << 24) as f32
}

impl Grass {
    /// The plants of every cell, at most `limit`, standing on `ground` (cells it has no height
    /// for are left out). A stand-in for the game's own spread, which is not read: a kind's
    /// last float taken as plants per 100 × 100 units, its first two as the height and its
    /// spread, the next two as the width and its spread. `density` (a preset's, 0 to 1) keeps
    /// that share of them, the same ones each run.
    pub fn plants(&self, limit: usize, density: f32, ground: impl Fn(f32, f32) -> Option<f32>) -> Vec<Plant> {
        let [cx, cz] = self.cell();
        let mut out = Vec::new();
        for (c, &kind) in self.cells.iter().enumerate() {
            let Some(k) = self.kinds.get(kind as usize) else { continue };
            let (i, j) = ((c % self.size[0] as usize) as f32, (c / self.size[0] as usize) as f32);
            let count = (k[4] * cx * cz / 10_000.0).round().max(0.0) as u32;
            for n in 0..count {
                if out.len() >= limit {
                    return out;
                }
                let seed = (c as u32).wrapping_mul(9781).wrapping_add(n.wrapping_mul(6271));
                if unit(seed ^ 0x2545_f491) >= density {
                    continue;
                }
                let (x, z) = ((i + unit(seed)) * cx, (j + unit(seed ^ 0x5bd1_e995)) * cz);
                let Some(y) = ground(x, z) else { continue };
                let h = k[0] + k[1] * (unit(seed ^ 0x68e3_1da4) - 0.5);
                let w = k[2] + k[3] * (unit(seed ^ 0x1b56_c4e9) - 0.5);
                out.push(Plant { at: [x, y, z], size: [w.max(1.0), h.max(1.0)], kind });
            }
        }
        out
    }
}

/// A grass preset of the game's (`saves/graphicssetting/grassqualitypreset.xml` and
/// `grassrangepreset.xml`, `Field="Default"`): how many of the plants are drawn, and the
/// distances from the eye over which they shrink away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub density: f32,
    pub far: [f32; 2],
}

impl Preset {
    /// The game's highest (`Middle`): every plant, shrinking from 8000 to 15000 units.
    pub const MIDDLE: Preset = Preset { density: 1.0, far: [8000.0, 15000.0] };

    /// The preset of `kind` ("Middle", "Low", "VeryLow") from the two tables.
    pub fn read(quality: &[u8], range: &[u8], kind: &str) -> Result<Preset, String> {
        let entry = |bytes: &[u8], tag: &str| -> Result<barrel_formats::table::Element, String> {
            let root = barrel_formats::table::parse(bytes).map_err(|e| format!("{tag}: {e}"))?;
            let mut all = vec![&root];
            let mut found = None;
            while let Some(e) = all.pop() {
                if e.name == tag && e.attr("Type") == Some(kind) && e.attr("Field").is_none_or(|f| f == "Default") {
                    found = Some(e.clone());
                    break;
                }
                all.extend(e.children.iter());
            }
            found.ok_or_else(|| format!("{tag}: no {kind}"))
        };
        let q = entry(quality, "GrassQualityPreset")?;
        let r = entry(range, "GrassRangePreset")?;
        let f = |e: &barrel_formats::table::Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).ok_or_else(|| format!("{}: no {k}", e.name));
        Ok(Preset { density: f(&q, "Density")?, far: [f(&r, "FarClipBegin")?, f(&r, "FarClipEnd")?] })
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
    fn plants_fill_their_cells_on_the_ground() {
        // One kind at 1 plant per 100 × 100 units over 64000-unit cells: 409,600 a cell.
        let g = Grass::parse(&file(&[[100.0, 0.0, 60.0, 0.0, 1.0]], &[NONE, 0, NONE, NONE], 2, 0)).unwrap();
        let p = g.plants(1000, 1.0, |x, _| Some(x / 1000.0));
        assert_eq!(p.len(), 1000);
        assert!(p.iter().all(|p| (64000.0..128000.0).contains(&p.at[0]) && p.at[2] < 64000.0));
        assert!(p.iter().all(|p| p.at[1] == p.at[0] / 1000.0 && p.size == [60.0, 100.0] && p.kind == 0));
        assert_eq!(p, g.plants(1000, 1.0, |x, _| Some(x / 1000.0)));
        assert!(g.plants(1000, 1.0, |_, _| None).is_empty());
        // A preset's density keeps about that share.
        let all = g.plants(usize::MAX, 1.0, |_, _| Some(0.0)).len() as f32;
        let some = g.plants(usize::MAX, 0.5, |_, _| Some(0.0)).len() as f32;
        assert!((some / all - 0.5).abs() < 0.01, "{some} of {all}");
    }

    #[test]
    fn reads_the_presets() {
        let q = br#"<Root><GrassQualityPreset Field="Default" Type="Middle" Density="1"/><GrassQualityPreset Field="Default" Type="Low" Density="0.75"/></Root>"#;
        let r = br#"<Root><GrassRangePreset Field="Default" Type="Middle" FarClipBegin="8000" FarClipEnd="15000"/><GrassRangePreset Field="Default" Type="Low" FarClipBegin="3000" FarClipEnd="3100"/></Root>"#;
        assert_eq!(Preset::read(q, r, "Middle").unwrap(), Preset::MIDDLE);
        assert_eq!(Preset::read(q, r, "Low").unwrap(), Preset { density: 0.75, far: [3000.0, 3100.0] });
        assert!(Preset::read(q, r, "VeryLow").is_err());
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
