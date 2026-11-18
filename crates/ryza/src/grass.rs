//! A field's plants, `<FIELD>_grass.bin` in its main container (docs/formats.md, "`_grass.bin`
//! and `_grass.g1t`"), as the game's loader reads it (`0x1407c5380`): the area's size, a draw
//! distance, the scale of a density, the plant kinds, the groups of kinds that grow together
//! and a grid of 500-unit cells, each naming the group that grows there (0xff: none). The game
//! builds the plants' quads on the CPU from it, one draw a cell, and draws them with
//! `ObjectGrassPBS` over the cards in `_grass.g1t`.

/// A cell with no plants.
pub const NONE: u32 = 0xff;

/// One kind of plant: a card's width and height, each from a least value over a spread, and
/// how many grow (a density: plants per square of `1 / Grass::density_scale` units).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kind {
    pub width: [f32; 2],
    pub height: [f32; 2],
    pub density: f32,
}

/// One kind growing in a group, and its card in the atlas (row from the top, column).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Member {
    pub card: [u32; 2],
    pub kind: u32,
}

/// The kinds that grow together in a cell, and the cards across the atlas they are cut from
/// (rows, columns: 2 by 4 in every field seen).
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub atlas: [u32; 2],
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grass {
    /// The area the grid covers from the origin, x and z, in game units.
    pub area: [f32; 2],
    /// Likely the draw distance (Rasenboden: 30000).
    pub distance: u32,
    /// What a density is counted over: a kind grows `density` plants a square of
    /// `1 / density_scale` units (0.01: 100 by 100 units, so 25 times it in a 500-unit cell).
    pub density_scale: f32,
    /// The grid's cells along x and z.
    pub size: [u32; 2],
    pub kinds: Vec<Kind>,
    pub groups: Vec<Group>,
    /// The grid, x fastest: each cell's group, or `NONE`.
    pub cells: Vec<u32>,
}

/// One plant: where its foot stands (game units), its size, its card and the cell it grows in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plant {
    pub at: [f32; 3],
    /// Width and height, game units.
    pub size: [f32; 2],
    /// The card's left, top, width and height in the atlas, as shares of it.
    pub card: [f32; 4],
    pub kind: u32,
    /// The cell's place in the grid (x, z).
    pub cell: [u32; 2],
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
    pub fn parse(b: &[u8]) -> Result<Grass, String> {
        let mut at = 0usize;
        let mut word = |what: &str| -> Result<[u8; 4], String> {
            let w = b.get(at..at + 4).ok_or_else(|| format!("grass: ends in {what} at {at:#x}"))?;
            at += 4;
            Ok(w.try_into().unwrap())
        };
        let area = [f32::from_le_bytes(word("the header")?), f32::from_le_bytes(word("the header")?)];
        let distance = u32::from_le_bytes(word("the header")?);
        let density_scale = f32::from_le_bytes(word("the header")?);
        let size = [u32::from_le_bytes(word("the header")?), u32::from_le_bytes(word("the header")?)];
        // a count, checked against the file's size before anything is set aside for it
        let fits = |n: [u8; 4], what: &str, each: usize| -> Result<usize, String> {
            let n = u32::from_le_bytes(n) as usize;
            if n.saturating_mul(each) > b.len() { Err(format!("grass: {n} {what} do not fit")) } else { Ok(n) }
        };
        let mut kinds = Vec::new();
        for _ in 0..fits(word("kinds")?, "kinds", 20)? {
            let mut f = [0f32; 5];
            for v in &mut f {
                *v = f32::from_le_bytes(word("the kinds")?);
            }
            kinds.push(Kind { width: [f[0], f[1]], height: [f[2], f[3]], density: f[4] });
        }
        // The groups (rows, columns, how many members), then every group's members in turn.
        let mut heads = Vec::new();
        for _ in 0..fits(word("groups")?, "groups", 12)? {
            let mut u = [0u32; 3];
            for v in &mut u {
                *v = u32::from_le_bytes(word("the groups")?);
            }
            heads.push(u);
        }
        let mut members = Vec::new();
        for _ in 0..fits(word("members")?, "members", 12)? {
            let mut u = [0u32; 3];
            for v in &mut u {
                *v = u32::from_le_bytes(word("the members")?);
            }
            if u[2] as usize >= kinds.len() {
                return Err(format!("grass: a member of kind {} of {}", u[2], kinds.len()));
            }
            members.push(Member { card: [u[0], u[1]], kind: u[2] });
        }
        if heads.iter().map(|h| h[2] as usize).sum::<usize>() != members.len() {
            return Err(format!("grass: the groups name {} members, {} there", heads.iter().map(|h| h[2] as usize).sum::<usize>(), members.len()));
        }
        let mut rest = members.into_iter();
        let groups = heads.iter().map(|h| Group { atlas: [h[0], h[1]], members: rest.by_ref().take(h[2] as usize).collect() }).collect();
        let n = fits(word("cells")?, "cells", 4)?;
        if n != (size[0] as usize).saturating_mul(size[1] as usize) {
            return Err(format!("grass: {n} cells for a {}×{} grid", size[0], size[1]));
        }
        let mut cells = Vec::with_capacity(n);
        for _ in 0..n {
            cells.push(u32::from_le_bytes(word("the cells")?));
        }
        Ok(Grass { area, distance, density_scale, size, kinds, groups, cells })
    }

    /// A cell's size along x and z, in game units.
    pub fn cell(&self) -> [f32; 2] {
        [self.area[0] / self.size[0].max(1) as f32, self.area[1] / self.size[1].max(1) as f32]
    }

    /// How many plants of `kind` grow in a cell: its density over the cell's area, rounded
    /// down (2.34 in a 500-unit cell: 58; 1.5: 37; 0.05: 1, as the game draws).
    pub fn count(&self, kind: &Kind) -> u32 {
        let [cx, cz] = self.cell();
        (kind.density * cx * self.density_scale * cz * self.density_scale).floor().max(0.0) as u32
    }

    // STAND-IN: A preset's density keeps that share of the plants by a hash; the real thing is how the game thins them at the lower grass qualities.
    /// The plants of every cell, at most `limit`, standing on `ground` (cells it has no height
    /// for are left out), a cell's in the order of its group's members. Which kinds, how many,
    /// their cards and their sizes' ranges are the file's; where each stands in its cell and
    /// where in its ranges its size falls are the game's own numbers ([`Scatter`]); `density`
    /// (a preset's, 0 to 1) keeps that share of them, the same ones each run.
    pub fn plants(&self, limit: usize, density: f32, ground: impl Fn(f32, f32) -> Option<f32>) -> Vec<Plant> {
        let [cx, cz] = self.cell();
        let mut out = Vec::new();
        for (c, &group) in self.cells.iter().enumerate() {
            let Some(g) = self.groups.get(group as usize) else { continue };
            let cell = [(c % self.size[0] as usize) as u32, (c / self.size[0] as usize) as u32];
            let (w, h) = (1.0 / g.atlas[1].max(1) as f32, 1.0 / g.atlas[0].max(1) as f32);
            let mut seeds = Scatter::of_cell(cell);
            for (m, member) in g.members.iter().enumerate() {
                let k = &self.kinds[member.kind as usize];
                let mut numbers = seeds.member();
                for n in 0..self.count(k) {
                    if out.len() >= limit {
                        return out;
                    }
                    // Four numbers a plant, in the game's order: x, z, width, height.
                    let [u, v, wide, tall] = [numbers.next(), numbers.next(), numbers.next(), numbers.next()];
                    let thin = (c as u32).wrapping_mul(9781).wrapping_add(n.wrapping_mul(6271)).wrapping_add((m as u32).wrapping_mul(40503));
                    if unit(thin ^ 0x2545_f491) >= density {
                        continue;
                    }
                    let (x, z) = ((cell[0] as f32 + u) * cx, (cell[1] as f32 + v) * cz);
                    let Some(y) = ground(x, z) else { continue };
                    let size = [k.width[0] + k.width[1] * wide, k.height[0] + k.height[1] * tall];
                    out.push(Plant { at: [x, y, z], size, card: [member.card[1] as f32 * w, member.card[0] as f32 * h, w, h], kind: member.kind, cell });
                }
            }
        }
        out
    }
}

/// The game's random numbers as it builds a cell's plants (its builder `0x140a77060`): a
/// generator `s = s × 69069 + 1` over 32 bits whose number is the top 16 bits over 65535.
/// A cell's seed comes from its place in the grid; each member of the cell's group takes the
/// generator's next 16 bits as the seed of its own run, and each of its plants four numbers
/// from that run: where it stands along x and z in the cell, then its width and its height
/// within the kind's ranges. A captured cell's 114 plants come out to the float.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scatter(u32);

impl Scatter {
    /// The grid index's row length in the seed (the game's builder's own value, read on
    /// Rasenboden's 128 by 128 grid: `row × 256 + column`).
    const ROW: u32 = 256;
    /// The seed's multiplier (the float at `0x141792f58`) and scale (`0x141792f5c`).
    const TURN: f32 = 0.362_436_06;
    const SCALE: f32 = 100_000_000.0;

    /// A cell's generator: its seed is the fraction of its index times `TURN`, times `SCALE`,
    /// in single floats as the game computes it.
    pub fn of_cell(cell: [u32; 2]) -> Scatter {
        let turned = (cell[1] * Self::ROW + cell[0]) as f32 * Self::TURN;
        Scatter(((turned - turned.trunc()) * Self::SCALE) as u32)
    }

    fn step(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(69069).wrapping_add(1);
        self.0 >> 16
    }

    /// The next member's own run.
    pub fn member(&mut self) -> Scatter {
        Scatter(self.step())
    }

    /// The run's next number, 0 to 1.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        self.step() as f32 / 65535.0
    }
}

/// A grass preset of the game's (`saves/graphicssetting/grassqualitypreset.xml` and
/// `grassrangepreset.xml`, read by `presets::Graphics`): how many of the plants are drawn, and the
/// distances from the eye over which they shrink away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub density: f32,
    pub far: [f32; 2],
}

impl Preset {
    /// The game's highest (`Middle`): every plant, shrinking from 8000 to 15000 units.
    pub const MIDDLE: Preset = Preset { density: 1.0, far: [8000.0, 15000.0] };
}

#[cfg(test)]
mod tests {
    #[test]
    fn scatter_is_the_games() {
        // Cell (66, 60) of Rasenboden: the game's first member's run starts at 13093 and its
        // first plant stands 105.27 and 360.02 units into the 500-unit cell.
        let mut cell = super::Scatter::of_cell([66, 60]);
        let mut run = cell.member();
        assert_eq!(run, super::Scatter(13093));
        assert!((run.next() * 500.0 - 105.272).abs() < 0.002);
        assert!((run.next() * 500.0 - 360.021).abs() < 0.002);
        assert!((25.0 + 40.0 * run.next() - 32.520).abs() < 0.002);
        assert!((25.0 + 40.0 * run.next() - 33.305).abs() < 0.002);
    }

    use super::*;

    /// A file of `kinds`, groups of (row, column, kind) members and a square grid.
    fn file(kinds: &[[f32; 5]], groups: &[&[[u32; 3]]], cells: &[u32], size: u32) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [128000.0f32, 128000.0] {
            b.extend(v.to_le_bytes());
        }
        b.extend(30000u32.to_le_bytes());
        b.extend(0.01f32.to_le_bytes());
        b.extend(size.to_le_bytes());
        b.extend(size.to_le_bytes());
        b.extend((kinds.len() as u32).to_le_bytes());
        for v in kinds.iter().flatten() {
            b.extend(v.to_le_bytes());
        }
        b.extend((groups.len() as u32).to_le_bytes());
        for g in groups {
            for v in [2, 4, g.len() as u32] {
                b.extend(v.to_le_bytes());
            }
        }
        b.extend((groups.iter().map(|g| g.len()).sum::<usize>() as u32).to_le_bytes());
        for v in groups.iter().flat_map(|g| g.iter()).flatten() {
            b.extend(v.to_le_bytes());
        }
        b.extend((cells.len() as u32).to_le_bytes());
        for c in cells {
            b.extend(c.to_le_bytes());
        }
        b
    }

    #[test]
    fn reads_the_header_kinds_groups_and_grid() {
        let kinds = [[50.0, 100.0, 60.0, 40.0, 0.1], [110.0, 30.0, 60.0, 60.0, 0.1]];
        let cells = [NONE, 1, 0, NONE];
        let g = Grass::parse(&file(&kinds, &[&[[0, 1, 1], [1, 1, 0]], &[[0, 2, 0]]], &cells, 2)).unwrap();
        assert_eq!((g.area, g.distance, g.density_scale, g.size), ([128000.0; 2], 30000, 0.01, [2, 2]));
        assert_eq!(g.kinds[1], Kind { width: [110.0, 30.0], height: [60.0, 60.0], density: 0.1 });
        assert_eq!(g.groups[0], Group { atlas: [2, 4], members: vec![Member { card: [0, 1], kind: 1 }, Member { card: [1, 1], kind: 0 }] });
        assert_eq!(g.groups[1].members, vec![Member { card: [0, 2], kind: 0 }]);
        assert_eq!(g.cells, cells.to_vec());
        assert_eq!(g.cell(), [64000.0, 64000.0]);
    }

    #[test]
    fn a_cell_grows_its_groups_kinds_as_the_title_draws_them() {
        // The title's field, 500-unit cells: its group 0 is one plant of a sparse kind on the
        // top row's second card and 37 of a dense one on the card below (38 a draw in the
        // capture, widths 60 to 120, heights 40 to 170); its group 4 is 58 of one kind on the
        // top row's third card, 100 to 150 wide and 60 to 120 high.
        let kinds = [[60.0, 50.0, 60.0, 110.0, 0.05], [60.0, 60.0, 40.0, 40.0, 1.5], [100.0, 50.0, 60.0, 60.0, 2.34]];
        let mut cells = vec![NONE; 256 * 256];
        (cells[0], cells[257]) = (0, 1);
        let g = Grass::parse(&file(&kinds, &[&[[0, 1, 0], [1, 1, 1]], &[[0, 2, 2]]], &cells, 256)).unwrap();
        let p = g.plants(usize::MAX, 1.0, |x, _| Some(x / 1000.0));
        let (a, b): (Vec<&Plant>, Vec<&Plant>) = p.iter().partition(|p| p.cell == [0, 0]);
        assert_eq!((a.len(), b.len()), (38, 58));
        assert_eq!((a[0].kind, a[0].card), (0, [0.25, 0.0, 0.25, 0.5]));
        assert!(a[1..].iter().all(|p| p.kind == 1 && p.card == [0.25, 0.5, 0.25, 0.5]));
        assert!(a[1..].iter().all(|p| (60.0..120.0).contains(&p.size[0]) && (40.0..80.0).contains(&p.size[1])));
        assert!(a.iter().all(|p| p.at[0] < 500.0 && p.at[2] < 500.0 && p.at[1] == p.at[0] / 1000.0));
        assert!(b.iter().all(|p| p.cell == [1, 1] && p.card == [0.5, 0.0, 0.25, 0.5] && (500.0..1000.0).contains(&p.at[0])));
        assert!(b.iter().all(|p| (100.0..150.0).contains(&p.size[0]) && (60.0..120.0).contains(&p.size[1])));
        assert_eq!(p, g.plants(usize::MAX, 1.0, |x, _| Some(x / 1000.0)));
        assert!(g.plants(usize::MAX, 1.0, |_, _| None).is_empty());
        assert_eq!(g.plants(10, 1.0, |_, _| Some(0.0)).len(), 10);
        // A preset's density keeps about that share.
        let some = g.plants(usize::MAX, 0.5, |_, _| Some(0.0)).len();
        assert!((30..66).contains(&some), "{some} of 96");
    }

    #[test]
    fn refuses_a_file_that_does_not_add_up() {
        let good = file(&[[1.0; 5]], &[&[[0, 0, 0]]], &[0; 4], 2);
        assert!(Grass::parse(&good).is_ok());
        assert!(Grass::parse(&good[..good.len() - 4]).is_err());
        assert!(Grass::parse(&file(&[[1.0; 5]], &[&[[0, 0, 0]]], &[0; 3], 2)).is_err());
        assert!(Grass::parse(&file(&[[1.0; 5]], &[&[[0, 0, 1]]], &[0; 4], 2)).is_err());
    }

    /// Rasenboden's file, when the install is at hand (`BARREL_RYZA_GRASS` names an unpacked
    /// `S06_T_RASEN_grass.bin`).
    #[test]
    fn rasenboden() {
        let Some(p) = std::env::var_os("BARREL_RYZA_GRASS") else { return };
        let g = Grass::parse(&barrel_files::read(p).unwrap()).unwrap();
        assert_eq!((g.size, g.kinds.len(), g.distance), ([256, 256], 19, 30000));
        assert_eq!(g.cells.iter().filter(|&&c| c != NONE).count(), 1446);
        assert_eq!(g.cells.iter().filter(|&&c| c == 7).count(), 785);
    }
}
