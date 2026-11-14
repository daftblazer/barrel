//! A field's `<field>_le.koc` (`_COK0100`): quads, mostly upright, that the player does not
//! pass (the field's edges, cliffs, the river). Header: magic `_COK0100`, u32 file size, u32
//! quad count; then each quad's four corners as (x, y, z, 1) floats, game space. An upright
//! quad is (a, top), (a, bottom), (b, bottom), (b, top). A few are not upright (Rasenboden:
//! two slopes and one flat), not used yet (`docs/ryza/open.md`). Docs: docs/formats.md.

use barrel_engine::walk::Wall;

pub type Quad = [[f32; 3]; 4];

pub fn parse(b: &[u8]) -> Result<Vec<Quad>, String> {
    if b.len() < 16 || &b[..8] != b"_COK0100" {
        return Err("koc: not _COK0100".into());
    }
    let count = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    if b.len() < 16 + count * 64 {
        return Err(format!("koc: {count} quads need {} bytes, {} there", 16 + count * 64, b.len()));
    }
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    Ok((0..count).map(|q| std::array::from_fn(|k| { let o = 16 + q * 64 + k * 16; [f(o), f(o + 4), f(o + 8)] })).collect())
}

/// The upright quads as walls (a quad is upright when its two ends stand straight, to a
/// hundredth of a unit).
pub fn walls(quads: &[Quad]) -> Vec<Wall> {
    let same = |p: [f32; 3], q: [f32; 3]| (p[0] - q[0]).abs() < 0.01 && (p[2] - q[2]).abs() < 0.01;
    quads
        .iter()
        .filter(|q| same(q[0], q[1]) && same(q[2], q[3]))
        .map(|q| {
            let ys = q.map(|p| p[1]);
            Wall { a: [q[0][0], q[0][2]], b: [q[2][0], q[2][2]], low: ys.iter().copied().fold(f32::MAX, f32::min), high: ys.iter().copied().fold(f32::MIN, f32::max) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_upright_quad_is_a_wall() {
        let mut b = b"_COK0100".to_vec();
        b.extend(96u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        for p in [[10.0f32, 50.0, 20.0], [10.0, 0.0, 20.0], [30.0, 0.0, 40.0], [30.0, 50.0, 40.0]] {
            for v in p.into_iter().chain([1.0]) {
                b.extend(v.to_le_bytes());
            }
        }
        let q = super::parse(&b).unwrap();
        let w = super::walls(&q);
        assert_eq!((w[0].a, w[0].b, w[0].low, w[0].high), ([10.0, 20.0], [30.0, 40.0], 0.0, 50.0));
    }
}
