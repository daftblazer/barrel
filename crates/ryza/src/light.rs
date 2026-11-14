//! A field's point lights (`<FIELD>_light.bin` in its container): a u32 count, then 40 bytes
//! each, `{f32 x, y, z, f32 × 6, u32 kind}` (`docs/formats.md`, "Ryza field lights"). Ryza 2 DX's
//! are laid out the same; Ryza 3 DX's fields have none.

/// One light: where it stands (field units, as the placement list gives them), its six values
/// (not settled: 5 in every light read, two strengths or falloffs, the range, two weights), and
/// its kind (0 indoors, 2 in Rasenboden's lamps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: [f32; 3],
    pub values: [f32; 6],
    pub range: f32,
    pub kind: u32,
}

const RECORD: usize = 40;

pub fn parse(d: &[u8]) -> Result<Vec<Light>, String> {
    let word = |o: usize| d.get(o..o + 4).map(|b| [b[0], b[1], b[2], b[3]]).ok_or("light list cut short");
    let count = u32::from_le_bytes(word(0)?) as usize;
    if d.len() != 4 + RECORD * count {
        return Err(format!("light list: {} bytes for {count} lights", d.len()));
    }
    (0..count)
        .map(|k| {
            let at = 4 + RECORD * k;
            let f = |i: usize| word(at + 4 * i).map(f32::from_le_bytes);
            let values = [f(3)?, f(4)?, f(5)?, f(6)?, f(7)?, f(8)?];
            Ok(Light { position: [f(0)?, f(1)?, f(2)?], values, range: values[3], kind: u32::from_le_bytes(word(at + 36)?) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attic_lights() {
        // The first of S01_I_ATTIC's two lights.
        let mut d = 1u32.to_le_bytes().to_vec();
        for v in [4042.6, 2209.6, 3811.9, 5.0, 2.55, 1.5, 700.0, 0.5, 0.3f32] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&0u32.to_le_bytes());
        let l = parse(&d).unwrap();
        assert_eq!((l.len(), l[0].range, l[0].kind, l[0].position[1]), (1, 700.0, 0, 2209.6));
        assert!(parse(&d[..d.len() - 1]).is_err());
    }
}
