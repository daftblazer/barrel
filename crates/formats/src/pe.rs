//! Just enough of the Windows executable format (PE32+) to read data the games compile into
//! their executables: the sections, to turn addresses into file offsets, and the enum name
//! arrays (see `docs/tables.md`, "Numbers behind the tags").

use crate::{Error, u32_at};

#[derive(Clone, Copy, Debug)]
struct Section {
    va: u64,
    size: u64,
    raw: usize,
    raw_size: usize,
}

#[derive(Clone, Debug)]
pub struct Pe<'a> {
    pub data: &'a [u8],
    pub image_base: u64,
    sections: Vec<Section>,
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, Error> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Error::Corrupt("read past the end"))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    data.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
}

impl<'a> Pe<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Pe<'a>, Error> {
        if !data.starts_with(b"MZ") {
            return Err(Error::NotThisFormat);
        }
        let pe = u32_at(data, 0x3c)? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(Error::NotThisFormat);
        }
        let count = u16_at(data, pe + 6)? as usize;
        let optional = u16_at(data, pe + 20)? as usize;
        let opt = pe + 24;
        if u16_at(data, opt)? != 0x20b {
            return Err(Error::Corrupt("not a 64-bit executable"));
        }
        let image_base = u64_at(data, opt + 24).ok_or(Error::Corrupt("read past the end"))?;
        let table = opt + optional;
        let mut sections = Vec::with_capacity(count);
        for i in 0..count.min(96) {
            let s = table + 40 * i;
            sections.push(Section {
                size: u32_at(data, s + 8)? as u64,
                va: image_base + u32_at(data, s + 12)? as u64,
                raw_size: u32_at(data, s + 16)? as usize,
                raw: u32_at(data, s + 20)? as usize,
            });
        }
        Ok(Pe { data, image_base, sections })
    }

    /// The file offset of virtual address `va`, if it is in a section's file data.
    pub fn offset(&self, va: u64) -> Option<usize> {
        self.sections.iter().find_map(|s| {
            let rel = va.checked_sub(s.va)?;
            (rel < s.size && (rel as usize) < s.raw_size).then(|| s.raw + rel as usize)
        })
    }

    /// The virtual address of file offset `at`.
    pub fn va(&self, at: usize) -> Option<u64> {
        self.sections
            .iter()
            .find(|s| at >= s.raw && at < s.raw + s.raw_size)
            .map(|s| s.va + (at - s.raw) as u64)
    }

    /// The NUL-terminated string at `va`, if it is short printable ASCII.
    pub fn name_at(&self, va: u64) -> Option<&'a str> {
        let at = self.offset(va)?;
        let rest = self.data.get(at..)?;
        let end = rest.iter().take(96).position(|&b| b == 0)?;
        let s = &rest[..end];
        (!s.is_empty() && s.iter().all(|b| b.is_ascii_graphic())).then(|| std::str::from_utf8(s).unwrap())
    }

    /// The enum name array that holds `name`: the names, in index order, and the index of
    /// `name`. The executables keep each enum's names as an array of pointers to strings,
    /// so an entry's position is its number. Several enums can share one array.
    pub fn enum_names(&self, name: &str) -> Option<(Vec<&'a str>, usize)> {
        let mut needle = Vec::with_capacity(name.len() + 2);
        needle.push(0);
        needle.extend_from_slice(name.as_bytes());
        needle.push(0);
        let mut from = 0;
        while let Some(p) = find(&self.data[from..], &needle) {
            let at = from + p + 1;
            from = at;
            let Some(va) = self.va(at) else { continue };
            let target = va.to_le_bytes();
            let mut q = 0;
            while let Some(p) = find(&self.data[q..], &target) {
                let slot = q + p;
                q = slot + 1;
                if slot % 8 != 0 {
                    continue;
                }
                let read = |s: usize| u64_at(self.data, s).and_then(|v| self.name_at(v));
                let mut first = slot;
                while first >= 8 && read(first - 8).is_some() {
                    first -= 8;
                }
                let mut names = Vec::new();
                let mut s = first;
                while let Some(n) = read(s) {
                    names.push(n);
                    s += 8;
                }
                return Some((names, (slot - first) / 8));
            }
        }
        None
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny PE32+ image: one section at RVA 0x1000, file offset 0x200, holding two
    /// strings and a pointer array to them.
    fn image() -> Vec<u8> {
        let mut d = vec![0u8; 0x400];
        d[0..2].copy_from_slice(b"MZ");
        d[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        d[0x80..0x84].copy_from_slice(b"PE\0\0");
        d[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        d[0x94..0x96].copy_from_slice(&0xf0u16.to_le_bytes());
        let opt = 0x98;
        d[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        d[opt + 24..opt + 32].copy_from_slice(&0x1_4000_0000u64.to_le_bytes());
        let s = opt + 0xf0;
        for (o, v) in [(8, 0x200u32), (12, 0x1000), (16, 0x200), (20, 0x200)] {
            d[s + o..s + o + 4].copy_from_slice(&v.to_le_bytes());
        }
        d[0x211..0x21d].copy_from_slice(b"TAG_FIRST\0\0\0");
        d[0x221..0x22b].copy_from_slice(b"TAG_NEXT\0\0");
        for (i, rva) in [0x1011u64, 0x1021].iter().enumerate() {
            let at = 0x240 + 8 * i;
            d[at..at + 8].copy_from_slice(&(0x1_4000_0000 + rva).to_le_bytes());
        }
        d
    }

    #[test]
    fn sections_and_enums() {
        let d = image();
        let pe = Pe::parse(&d).unwrap();
        assert_eq!(pe.offset(0x1_4000_1011), Some(0x211));
        assert_eq!(pe.va(0x221), Some(0x1_4000_1021));
        let (names, index) = pe.enum_names("TAG_NEXT").unwrap();
        assert_eq!((names, index), (vec!["TAG_FIRST", "TAG_NEXT"], 1));
        assert!(pe.enum_names("TAG_NONE").is_none());
    }
}
