//! Prefix codes read through lookup tables: a first table on the next `ROOT` bits, longer codes
//! through sub-tables on the bits after.

use crate::bits::Reader;

const ROOT: u32 = 10;
const SUB: u32 = 6;

#[derive(Clone, Copy, Default)]
enum Entry {
    #[default]
    Invalid,
    /// A code ending here: its symbol and length (within this table's bits).
    Leaf(i16, u8),
    /// A longer code: the sub-table and its index width.
    Table(u32, u8),
}

pub struct Vlc {
    tables: Vec<Vec<Entry>>,
}

impl Vlc {
    /// From (code, length, symbol) rows.
    pub fn new(rows: &[(u32, u8, i16)]) -> Self {
        let mut v = Vlc { tables: vec![vec![Entry::Invalid; 1 << ROOT]] };
        for &(code, len, sym) in rows {
            v.insert(0, ROOT, code, len as u32, sym);
        }
        v
    }

    fn insert(&mut self, table: usize, bits: u32, code: u32, len: u32, sym: i16) {
        if len <= bits {
            let first = (code << (bits - len)) as usize;
            for e in &mut self.tables[table][first..first + (1 << (bits - len))] {
                *e = Entry::Leaf(sym, len as u8);
            }
            return;
        }
        let rest = len - bits;
        let index = (code >> rest) as usize;
        let (sub, sub_bits) = match self.tables[table][index] {
            Entry::Table(t, b) => (t as usize, b as u32),
            _ => {
                let t = self.tables.len();
                self.tables.push(vec![Entry::Invalid; 1 << SUB]);
                self.tables[table][index] = Entry::Table(t as u32, SUB as u8);
                (t, SUB)
            }
        };
        self.insert(sub, sub_bits, code & ((1 << rest) - 1), rest, sym);
    }

    /// The next symbol, or None for bits that are no code.
    pub fn read(&self, r: &mut Reader) -> Option<i16> {
        let (mut table, mut bits) = (0usize, ROOT);
        loop {
            match self.tables[table][r.peek(bits) as usize] {
                Entry::Leaf(sym, len) => {
                    r.skip(len as usize);
                    return Some(sym);
                }
                Entry::Table(t, b) => {
                    r.skip(bits as usize);
                    table = t as usize;
                    bits = b as u32;
                }
                Entry::Invalid => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_prefix_codes_and_read_back() {
        for (name, rows) in [("scale", &crate::tables::SCALE[..]), ("scale rl", &crate::tables::SCALE_RL[..]), ("coef0", &crate::tables::COEF0[..]), ("coef1", &crate::tables::COEF1[..]), ("vec4", &crate::tables::VEC4[..]), ("vec2", &crate::tables::VEC2[..]), ("vec1", &crate::tables::VEC1[..])] {
            let kraft: f64 = rows.iter().map(|r| 0.5f64.powi(r.1 as i32)).sum();
            assert!(kraft <= 1.0 + 1e-12, "{name}: {kraft}");
            let vlc = Vlc::new(rows);
            for &(code, len, sym) in rows {
                let word = (code as u64) << (40 - len as u32);
                let bytes = word.to_be_bytes();
                let mut r = Reader::new(&bytes[3..], 40);
                assert_eq!(vlc.read(&mut r), Some(sym), "{name} code {code:b}/{len}");
                assert_eq!(r.position(), len as usize);
            }
        }
    }
}
