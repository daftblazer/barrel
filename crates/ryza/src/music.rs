//! A field's music: the executable's table of music numbers per field map, and the sound
//! tables (`saves/sound/stream.xml`, `sfx.xml`) that turn a number or a tag into the hash a
//! sound is asked for by (`docs/ryza/open.md`, "Sound cues").

/// A table row's size in the executable's `.rdata`: one per field map, in map order.
const ROW: usize = 0x48;
/// The row's field-id word (`0x000e0000 | (map + 1)`), its offset in the row. The table is found
/// by these ids: rows of [`ROW`] bytes whose ids count up from 1.
const ID_AT: usize = 0x38;
const ID_BASE: u32 = 0x000e_0000;
/// The row's day and night music numbers (-1 for none), their offsets in the row.
const DAY_AT: usize = 0x04;
const NIGHT_AT: usize = 0x08;
/// Music numbers are `5000 +` the number in the stream table's `BGM<nnnnn>` tag.
const NUMBER_BASE: i32 = 5000;
/// Rows that must count up from the first one for a place to be taken for the table.
const PROOF_ROWS: usize = 8;

fn word(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

/// The file offset of the field table's first row, found by its ids.
fn table(exe: &[u8]) -> Option<usize> {
    (0..exe.len().saturating_sub(ROW * PROOF_ROWS)).step_by(4).find(|&row| (0..PROOF_ROWS).all(|k| word(exe, row + k * ROW + ID_AT) == Some(ID_BASE | (k as u32 + 1))))
}

/// The music number of field map `map` (day, or night), from the executable's table; `None`
/// when the table or the row is missing or the field has no music.
pub fn field_number(exe: &[u8], map: usize, night: bool) -> Option<i32> {
    let row = table(exe)? + map * ROW;
    if word(exe, row + ID_AT)? != ID_BASE | (map as u32 + 1) {
        return None;
    }
    let n = word(exe, row + if night { NIGHT_AT } else { DAY_AT })? as i32;
    (n >= NUMBER_BASE).then_some(n)
}

// STAND-IN: Day is the hours 6 to 17 of the save's clock and night the rest; the real thing is the game's own rule (the time of day's period 2 to 4 is night, period 4 at half volume, `0x1405a0b80` / `0x1404d38d0`), not read.
/// Whether the field plays its night music at `hour` (0 to 23).
pub fn is_night(hour: u32) -> bool {
    !(6..18).contains(&hour)
}

/// One row of a sound table: its hash, tag and the container name (`file`) it lives in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub hash: u32,
    pub tag: String,
    pub file: String,
}

/// The rows of a sound table (`<sound hash="0x03BEA3B2" tag="BGM00003_..." file="BGM"/>`).
pub fn parse_table(xml: &[u8]) -> Vec<Entry> {
    let text = String::from_utf8_lossy(xml);
    let attr = |line: &str, name: &str| -> Option<String> {
        let from = line.find(&format!("{name}=\""))? + name.len() + 2;
        Some(line[from..from + line[from..].find('"')?].to_string())
    };
    text.lines()
        .filter(|l| l.contains("<sound "))
        .filter_map(|l| {
            let hash = attr(l, "hash")?;
            Some(Entry { hash: u32::from_str_radix(hash.trim_start_matches("0x").trim_start_matches("0X"), 16).ok()?, tag: attr(l, "tag")?, file: attr(l, "file")? })
        })
        .collect()
}

/// The hash of the music `number` (`5003`): the stream table's row whose tag starts
/// `BGM<number - 5000, 5 digits>`.
pub fn music_hash(streams: &[Entry], number: i32) -> Option<u32> {
    let prefix = format!("BGM{:05}", number - NUMBER_BASE);
    streams.iter().find(|e| e.tag.starts_with(&prefix)).map(|e| e.hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> Vec<u8> {
        let mut d = vec![0u8; 0x100 + ROW * 12];
        for k in 0..12 {
            let row = 0x100 + k * ROW;
            d[row + ID_AT..row + ID_AT + 4].copy_from_slice(&(ID_BASE | (k as u32 + 1)).to_le_bytes());
            d[row + DAY_AT..row + DAY_AT + 4].copy_from_slice(&(5000 + k as i32).to_le_bytes());
            d[row + NIGHT_AT..row + NIGHT_AT + 4].copy_from_slice(&(-1i32).to_le_bytes());
        }
        d
    }

    #[test]
    fn the_table_is_found_by_its_ids() {
        let d = exe();
        assert_eq!(table(&d), Some(0x100));
        assert_eq!(field_number(&d, 5, false), Some(5005));
        assert_eq!(field_number(&d, 5, true), None);
        assert_eq!(field_number(&d, 40, false), None);
    }

    #[test]
    fn a_number_is_a_stream_tables_tag() {
        let xml = b"<Root>\n\t<sound hash=\"0x414C7000\" tag=\"BGM00001_FM_01_ATTIC01\" file=\"BGM\"/>\n\t<sound hash=\"0x03BEA3B2\" tag=\"BGM00003_FM_03_ISLAND01\" file=\"BGM\"/>\n</Root>";
        let t = parse_table(xml);
        assert_eq!(t.len(), 2);
        assert_eq!(music_hash(&t, 5003), Some(0x03be_a3b2));
        assert_eq!(music_hash(&t, 5002), None);
        assert!(is_night(5) && !is_night(10) && is_night(18));
    }

    /// The installed executable (`BARREL_RYZA_EXE`) and its extracted stream table
    /// (`BARREL_RYZA_STREAM_XML`): Rasenboden (map 5) is 5003 by day, 5004 by night, `03bea3b2`.
    #[test]
    fn the_installs_rasenboden_music() {
        let (Some(e), Some(x)) = (std::env::var_os("BARREL_RYZA_EXE"), std::env::var_os("BARREL_RYZA_STREAM_XML")) else { return };
        let exe = std::fs::read(e).unwrap();
        assert_eq!(field_number(&exe, 5, false), Some(5003));
        assert_eq!(field_number(&exe, 5, true), Some(5004));
        let t = parse_table(&std::fs::read(x).unwrap());
        assert_eq!(music_hash(&t, 5003), Some(0x03be_a3b2));
    }
}
