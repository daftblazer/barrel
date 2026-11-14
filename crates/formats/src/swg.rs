//! Swing settings (`*.swg`, next to a character's model in its container, the Ryza games):
//! which bones swing (the hair, the skirt, the bust) and how. Magic `SWGQ`, then `{u32 header
//! size (0x24), u32 count, 8-byte name, f32 (1.0), 12 bytes 0}`, then `count` records of 0x48 bytes:
//! `{u16 group, u16 bone id, 15 floats, 4 u16}`. The group is the hundreds in the bone's name
//! (`SWING000_L_Bust` 0, `SWING400_L_Bottom000` 400, `SWING500_Hair000` 500); the bone ids are
//! the model's own (`*Oid.bin`, `oid::by_id`). What the floats and the last words are is not
//! known: in Ryza's (`PC00A_MODEL.swg`, `def014`) the bust has (60, 20, 25, 25, 0, 0, 3, 1, 2, 2,
//! 0, 0, 0, 1, 0) and (10, 3, 25, 25), the skirt four 90s and (25, 4, 25, 100), the hair four
//! angles (45, 0 to 45) then 0.8 (or -69 on the side locks' roots) and 5, and (25, 4, 15 or 25,
//! 100). The first four floats look like angle limits in degrees. Docs: docs/formats.md, "Swing settings".

use crate::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Swing {
    pub name: String,
    /// The float after the name (1.0 in Ryza's).
    pub value: f32,
    pub records: Vec<Record>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub group: u16,
    pub bone: u16,
    pub floats: [f32; 15],
    pub words: [u16; 4],
}

const RECORD: usize = 0x48;

pub fn parse(data: &[u8]) -> Result<Swing, Error> {
    if data.get(..4) != Some(b"SWGQ") {
        return Err(Error::Corrupt("swg magic"));
    }
    let u32_at = |p: usize| data.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let (header, count) = (u32_at(4).ok_or(Error::Corrupt("swg header"))? as usize, u32_at(8).ok_or(Error::Corrupt("swg header"))? as usize);
    let name = data.get(12..20).ok_or(Error::Corrupt("swg name"))?;
    let name = String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
    let value = f32::from_le_bytes(data.get(20..24).ok_or(Error::Corrupt("swg header"))?.try_into().unwrap());
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let r = data.get(header + i * RECORD..header + (i + 1) * RECORD).ok_or(Error::Corrupt("swg record past the end"))?;
        let u16_at = |p: usize| u16::from_le_bytes([r[p], r[p + 1]]);
        records.push(Record {
            group: u16_at(0),
            bone: u16_at(2),
            floats: std::array::from_fn(|k| f32::from_le_bytes(r[4 + 4 * k..8 + 4 * k].try_into().unwrap())),
            words: std::array::from_fn(|k| u16_at(64 + 2 * k)),
        });
    }
    Ok(Swing { name, value, records })
}

#[cfg(test)]
mod tests {
    #[test]
    fn one_record() {
        let mut d = b"SWGQ".to_vec();
        d.extend(0x24u32.to_le_bytes());
        d.extend(1u32.to_le_bytes());
        d.extend(b"def014\0\0");
        d.extend(1f32.to_le_bytes());
        d.extend([0; 12]);
        d.extend(500u16.to_le_bytes());
        d.extend(952u16.to_le_bytes());
        for k in 0..15 {
            d.extend((k as f32).to_le_bytes());
        }
        for w in [25u16, 4, 15, 100] {
            d.extend(w.to_le_bytes());
        }
        let s = super::parse(&d).unwrap();
        assert_eq!((s.name.as_str(), s.value, s.records.len()), ("def014", 1.0, 1));
        let r = &s.records[0];
        assert_eq!((r.group, r.bone, r.floats[14], r.words), (500, 952, 14.0, [25, 4, 15, 100]));
    }
}
