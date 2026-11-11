//! Ogg Vorbis, decoded natively (the game's voices, music and sounds; `voice::ogg` unwraps the
//! voice clips). Written from the Vorbis I specification (Xiph.Org, 2015) and RFC 3533 (Ogg).
//!
//! So far: Ogg pages into packets, the identification header and the whole setup header
//! (codebooks, floors, residues, mappings, modes). The audio packets follow.

use crate::Error;

/// Splits an Ogg stream into its packets (one logical stream: the page sequence is taken as
/// given; packets that continue over pages are joined).
pub fn packets(ogg: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
    let mut out = Vec::new();
    let mut partial: Vec<u8> = Vec::new();
    let mut at = 0;
    while at < ogg.len() {
        let page = ogg.get(at..at + 27).ok_or(Error::Corrupt("Ogg page header past the end"))?;
        if &page[..4] != b"OggS" {
            return Err(Error::Corrupt("Ogg page without its capture pattern"));
        }
        let segments = page[26] as usize;
        let table = ogg.get(at + 27..at + 27 + segments).ok_or(Error::Corrupt("Ogg segment table past the end"))?;
        let mut body = at + 27 + segments;
        for &len in table {
            let len = len as usize;
            partial.extend_from_slice(ogg.get(body..body + len).ok_or(Error::Corrupt("Ogg segment past the end"))?);
            body += len;
            if len < 255 {
                out.push(std::mem::take(&mut partial));
            }
        }
        at = body;
    }
    if !partial.is_empty() {
        out.push(partial);
    }
    Ok(out)
}

/// Reads bits least significant first, as Vorbis packs them.
pub struct Bits<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, bit: 0 }
    }

    /// `n` bits (up to 32) as an unsigned number; `None` past the end of the packet.
    pub fn read(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u64;
        for i in 0..n {
            let byte = *self.data.get(self.bit / 8)?;
            v |= u64::from((byte >> (self.bit % 8)) & 1) << i;
            self.bit += 1;
        }
        Some(v as u32)
    }

    pub fn flag(&mut self) -> Option<bool> {
        self.read(1).map(|b| b == 1)
    }

    /// The bits read so far.
    pub fn position(&self) -> usize {
        self.bit
    }
}

/// The identification header (packet type 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identification {
    pub channels: u8,
    pub rate: u32,
    /// The two block sizes (short, long), powers of two.
    pub blocksize: [usize; 2],
}

pub fn identification(packet: &[u8]) -> Result<Identification, Error> {
    if packet.len() < 30 || packet[0] != 1 || &packet[1..7] != b"vorbis" {
        return Err(Error::NotThisFormat);
    }
    let u32_at = |o: usize| u32::from_le_bytes(packet[o..o + 4].try_into().unwrap());
    if u32_at(7) != 0 {
        return Err(Error::Corrupt("Vorbis version not 0"));
    }
    let channels = packet[11];
    let rate = u32_at(12);
    let b = packet[28];
    let blocksize = [1usize << (b & 0xf), 1usize << (b >> 4)];
    if channels == 0 || rate == 0 || blocksize[0] < 64 || blocksize[0] > blocksize[1] || blocksize[1] > 8192 || packet[29] & 1 == 0 {
        return Err(Error::Corrupt("Vorbis identification header out of range"));
    }
    Ok(Identification { channels, rate, blocksize })
}

/// The integer log of the specification (`ilog`): the position of the highest set bit.
pub fn ilog(x: u32) -> u32 {
    32 - x.leading_zeros()
}

/// The spec's `float32_unpack`.
fn float32(x: u32) -> f32 {
    let mantissa = (x & 0x1f_ffff) as f32;
    let exponent = ((x & 0x7fe0_0000) >> 21) as i32;
    let m = if x & 0x8000_0000 != 0 { -mantissa } else { mantissa };
    m * 2f32.powi(exponent - 788)
}

/// The spec's `lookup1_values`: the largest r with r^dimensions <= entries.
fn lookup1_values(entries: u32, dimensions: u32) -> u32 {
    let mut r = (entries as f64).powf(1.0 / dimensions as f64).floor() as u32;
    while (r + 1).checked_pow(dimensions).is_some_and(|v| v <= entries) {
        r += 1;
    }
    while r > 0 && r.checked_pow(dimensions).is_none_or(|v| v > entries) {
        r -= 1;
    }
    r
}

/// A codebook: Huffman codes for its entries and, when it has one, the vector each entry stands
/// for.
#[derive(Clone, Debug, PartialEq)]
pub struct Codebook {
    pub dimensions: usize,
    /// Each entry's code length (0: unused).
    pub lengths: Vec<u8>,
    /// Each entry's code, in the order bits are read (the first bit read is bit 0).
    pub codes: Vec<u32>,
    /// `dimensions` values per entry, or empty for a codebook without vectors.
    pub vectors: Vec<f32>,
}

impl Codebook {
    /// Reads one entry's number from the packet (`None` at the end of the packet or on a
    /// code no entry has).
    pub fn decode(&self, bits: &mut Bits) -> Option<usize> {
        let mut code = 0u32;
        for len in 1..=32u8 {
            code |= bits.read(1)? << (len - 1);
            if let Some(i) = (0..self.lengths.len()).find(|&i| self.lengths[i] == len && self.codes[i] == code) {
                return Some(i);
            }
        }
        None
    }
}

/// Reads one codebook from the setup header (section 3.2.1).
pub fn codebook(bits: &mut Bits) -> Result<Codebook, Error> {
    let e = || Error::Corrupt("Vorbis codebook past the end of the packet");
    if bits.read(24).ok_or_else(e)? != 0x564342 {
        return Err(Error::Corrupt("Vorbis codebook sync pattern"));
    }
    let dimensions = bits.read(16).ok_or_else(e)? as usize;
    let entries = bits.read(24).ok_or_else(e)? as usize;
    let mut lengths = vec![0u8; entries];
    if !bits.flag().ok_or_else(e)? {
        let sparse = bits.flag().ok_or_else(e)?;
        for l in lengths.iter_mut() {
            if !sparse || bits.flag().ok_or_else(e)? {
                *l = bits.read(5).ok_or_else(e)? as u8 + 1;
            }
        }
    } else {
        let mut length = bits.read(5).ok_or_else(e)? as u8 + 1;
        let mut i = 0;
        while i < entries {
            let n = bits.read(ilog((entries - i) as u32)).ok_or_else(e)? as usize;
            for l in lengths.iter_mut().skip(i).take(n) {
                *l = length;
            }
            i += n;
            length += 1;
        }
        if i > entries {
            return Err(Error::Corrupt("Vorbis ordered codebook overruns its entries"));
        }
    }
    let codes = huffman(&lengths)?;
    let lookup = bits.read(4).ok_or_else(e)?;
    let mut vectors = Vec::new();
    if lookup == 1 || lookup == 2 {
        let min = float32(bits.read(32).ok_or_else(e)?);
        let delta = float32(bits.read(32).ok_or_else(e)?);
        let value_bits = bits.read(4).ok_or_else(e)? + 1;
        let sequence = bits.flag().ok_or_else(e)?;
        let count = if lookup == 1 { lookup1_values(entries as u32, dimensions as u32) as usize } else { entries * dimensions };
        let mults: Vec<u32> = (0..count).map(|_| bits.read(value_bits)).collect::<Option<_>>().ok_or_else(e)?;
        vectors.reserve(entries * dimensions);
        for entry in 0..entries {
            let mut last = 0.0f32;
            let mut index_divisor = 1usize;
            for d in 0..dimensions {
                let offset = if lookup == 1 { (entry / index_divisor) % count } else { entry * dimensions + d };
                let v = mults[offset] as f32 * delta + min + last;
                if sequence {
                    last = v;
                }
                vectors.push(v);
                if lookup == 1 {
                    index_divisor *= count;
                }
            }
        }
    } else if lookup != 0 {
        return Err(Error::Corrupt("Vorbis codebook lookup type"));
    }
    Ok(Codebook { dimensions, lengths, codes, vectors })
}

/// The setup header's codebooks (packet type 5, its first part; the rest follows them).
/// Returns them and the reader positioned after them.
pub fn codebooks(packet: &[u8]) -> Result<(Vec<Codebook>, Bits<'_>), Error> {
    if packet.len() < 8 || packet[0] != 5 || &packet[1..7] != b"vorbis" {
        return Err(Error::NotThisFormat);
    }
    let mut bits = Bits::new(&packet[7..]);
    let count = bits.read(8).ok_or(Error::Corrupt("Vorbis setup header empty"))? as usize + 1;
    let books = (0..count).map(|_| codebook(&mut bits)).collect::<Result<Vec<_>, _>>()?;
    Ok((books, bits))
}

/// A floor (section 6 for type 0, section 7 for type 1).
#[derive(Clone, Debug, PartialEq)]
pub enum Floor {
    Zero { order: u32, rate: u32, bark_map_size: u32, amplitude_bits: u32, amplitude_offset: u32, books: Vec<u8> },
    One(Floor1),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Floor1 {
    /// Each partition's class.
    pub partitions: Vec<u8>,
    /// Per class: dimensions, subclass bits, master book, subclass books (-1 none).
    pub classes: Vec<Floor1Class>,
    /// 1 to 4.
    pub multiplier: u32,
    /// The X positions, starting with 0 and 2^rangebits.
    pub xs: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Floor1Class {
    pub dimensions: u32,
    pub subclasses: u32,
    pub masterbook: u8,
    pub subbooks: Vec<i32>,
}

/// A residue (section 8): its type (0, 1 or 2) and layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Residue {
    pub kind: u16,
    pub begin: u32,
    pub end: u32,
    pub partition_size: u32,
    pub classifications: u32,
    pub classbook: u8,
    /// For each classification, the book of each of the 8 passes (-1 none).
    pub books: Vec<[i32; 8]>,
}

/// A mapping (section 4.2.4, type 0).
#[derive(Clone, Debug, PartialEq)]
pub struct Mapping {
    /// (magnitude channel, angle channel) coupling steps.
    pub coupling: Vec<(u32, u32)>,
    /// Each channel's submap.
    pub mux: Vec<u8>,
    /// Per submap: (floor, residue).
    pub submaps: Vec<(u8, u8)>,
}

/// A mode: whether it uses the long block, and its mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    pub long: bool,
    pub mapping: u8,
}

/// The whole setup header (packet type 5).
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub codebooks: Vec<Codebook>,
    pub floors: Vec<Floor>,
    pub residues: Vec<Residue>,
    pub mappings: Vec<Mapping>,
    pub modes: Vec<Mode>,
}

/// Reads the setup header to its framing bit (section 4.2.4); `channels` from the
/// identification header.
pub fn setup(packet: &[u8], channels: u8) -> Result<Setup, Error> {
    let (codebooks, mut bits) = codebooks(packet)?;
    let e = || Error::Corrupt("Vorbis setup header past the end of the packet");
    let books = codebooks.len() as u32;
    let book = |b: u32| if b < books { Ok(b as u8) } else { Err(Error::Corrupt("Vorbis setup names a codebook it lacks")) };
    let times = bits.read(6).ok_or_else(e)? + 1;
    for _ in 0..times {
        if bits.read(16).ok_or_else(e)? != 0 {
            return Err(Error::Corrupt("Vorbis time domain transform not 0"));
        }
    }
    let mut floors = Vec::new();
    for _ in 0..bits.read(6).ok_or_else(e)? + 1 {
        match bits.read(16).ok_or_else(e)? {
            0 => {
                let order = bits.read(8).ok_or_else(e)?;
                let rate = bits.read(16).ok_or_else(e)?;
                let bark_map_size = bits.read(16).ok_or_else(e)?;
                let amplitude_bits = bits.read(6).ok_or_else(e)?;
                let amplitude_offset = bits.read(8).ok_or_else(e)?;
                let n = bits.read(4).ok_or_else(e)? + 1;
                let list = (0..n).map(|_| bits.read(8).ok_or_else(e).and_then(book)).collect::<Result<_, _>>()?;
                floors.push(Floor::Zero { order, rate, bark_map_size, amplitude_bits, amplitude_offset, books: list });
            }
            1 => {
                let partitions: Vec<u8> = (0..bits.read(5).ok_or_else(e)?).map(|_| bits.read(4).map(|c| c as u8).ok_or_else(e)).collect::<Result<_, _>>()?;
                let max = partitions.iter().copied().max().map_or(-1, i32::from);
                let mut classes = Vec::new();
                for _ in 0..=max {
                    let dimensions = bits.read(3).ok_or_else(e)? + 1;
                    let subclasses = bits.read(2).ok_or_else(e)?;
                    let masterbook = if subclasses > 0 { book(bits.read(8).ok_or_else(e)?)? } else { 0 };
                    let subbooks = (0..1u32 << subclasses)
                        .map(|_| {
                            let b = bits.read(8).ok_or_else(e)? as i32 - 1;
                            if b >= 0 {
                                book(b as u32)?;
                            }
                            Ok(b)
                        })
                        .collect::<Result<_, Error>>()?;
                    classes.push(Floor1Class { dimensions, subclasses, masterbook, subbooks });
                }
                let multiplier = bits.read(2).ok_or_else(e)? + 1;
                let rangebits = bits.read(4).ok_or_else(e)?;
                let mut xs = vec![0, 1 << rangebits];
                for &c in &partitions {
                    for _ in 0..classes[c as usize].dimensions {
                        xs.push(bits.read(rangebits).ok_or_else(e)?);
                    }
                }
                let mut sorted = xs.clone();
                sorted.sort_unstable();
                sorted.dedup();
                if sorted.len() != xs.len() || xs.len() > 65 {
                    return Err(Error::Corrupt("Vorbis floor 1 X list repeats or is too long"));
                }
                floors.push(Floor::One(Floor1 { partitions, classes, multiplier, xs }));
            }
            _ => return Err(Error::Corrupt("Vorbis floor type")),
        }
    }
    let mut residues = Vec::new();
    for _ in 0..bits.read(6).ok_or_else(e)? + 1 {
        let kind = bits.read(16).ok_or_else(e)? as u16;
        if kind > 2 {
            return Err(Error::Corrupt("Vorbis residue type"));
        }
        let begin = bits.read(24).ok_or_else(e)?;
        let end = bits.read(24).ok_or_else(e)?;
        let partition_size = bits.read(24).ok_or_else(e)? + 1;
        let classifications = bits.read(6).ok_or_else(e)? + 1;
        let classbook = book(bits.read(8).ok_or_else(e)?)?;
        let mut cascade = Vec::new();
        for _ in 0..classifications {
            let low = bits.read(3).ok_or_else(e)?;
            let high = if bits.flag().ok_or_else(e)? { bits.read(5).ok_or_else(e)? } else { 0 };
            cascade.push(high * 8 + low);
        }
        let mut list = Vec::new();
        for c in cascade {
            let mut row = [-1i32; 8];
            for (j, slot) in row.iter_mut().enumerate() {
                if c & (1 << j) != 0 {
                    *slot = i32::from(book(bits.read(8).ok_or_else(e)?)?);
                }
            }
            list.push(row);
        }
        residues.push(Residue { kind, begin, end, partition_size, classifications, classbook, books: list });
    }
    let mut mappings = Vec::new();
    for _ in 0..bits.read(6).ok_or_else(e)? + 1 {
        if bits.read(16).ok_or_else(e)? != 0 {
            return Err(Error::Corrupt("Vorbis mapping type"));
        }
        let submap_count = if bits.flag().ok_or_else(e)? { bits.read(4).ok_or_else(e)? + 1 } else { 1 };
        let mut coupling = Vec::new();
        if bits.flag().ok_or_else(e)? {
            let width = ilog(u32::from(channels) - 1);
            for _ in 0..bits.read(8).ok_or_else(e)? + 1 {
                let m = bits.read(width).ok_or_else(e)?;
                let a = bits.read(width).ok_or_else(e)?;
                if m == a || m >= u32::from(channels) || a >= u32::from(channels) {
                    return Err(Error::Corrupt("Vorbis coupling channels"));
                }
                coupling.push((m, a));
            }
        }
        if bits.read(2).ok_or_else(e)? != 0 {
            return Err(Error::Corrupt("Vorbis mapping reserved bits"));
        }
        let mux = if submap_count > 1 {
            (0..channels).map(|_| bits.read(4).map(|m| m as u8).ok_or_else(e)).collect::<Result<_, _>>()?
        } else {
            vec![0; channels as usize]
        };
        let mut submaps = Vec::new();
        for _ in 0..submap_count {
            bits.read(8).ok_or_else(e)?;
            let floor = bits.read(8).ok_or_else(e)? as u8;
            let residue = bits.read(8).ok_or_else(e)? as u8;
            if usize::from(floor) >= floors.len() || usize::from(residue) >= residues.len() {
                return Err(Error::Corrupt("Vorbis mapping names a floor or residue it lacks"));
            }
            submaps.push((floor, residue));
        }
        mappings.push(Mapping { coupling, mux, submaps });
    }
    let mut modes = Vec::new();
    for _ in 0..bits.read(6).ok_or_else(e)? + 1 {
        let long = bits.flag().ok_or_else(e)?;
        let window = bits.read(16).ok_or_else(e)?;
        let transform = bits.read(16).ok_or_else(e)?;
        let mapping = bits.read(8).ok_or_else(e)? as u8;
        if window != 0 || transform != 0 || usize::from(mapping) >= mappings.len() {
            return Err(Error::Corrupt("Vorbis mode"));
        }
        modes.push(Mode { long, mapping });
    }
    if !bits.flag().ok_or_else(e)? {
        return Err(Error::Corrupt("Vorbis setup header framing bit"));
    }
    Ok(Setup { codebooks, floors, residues, mappings, modes })
}

/// Assigns the Huffman codes from the lengths the way the specification does (each entry, in
/// order, takes the lowest free code of its length), returned bit-reversed so that the first
/// bit read is bit 0. `free[l]` holds the free code of length l, aligned to the top of 32 bits.
fn huffman(lengths: &[u8]) -> Result<Vec<u32>, Error> {
    let mut free = [0u32; 33];
    let mut codes = vec![0u32; lengths.len()];
    let mut started = false;
    for (i, &len) in lengths.iter().enumerate() {
        if len == 0 {
            continue;
        }
        let len = len as usize;
        if len > 32 {
            return Err(Error::Corrupt("Vorbis code longer than 32 bits"));
        }
        let code;
        if !started {
            code = 0u32;
            for (l, f) in free.iter_mut().enumerate().take(len + 1).skip(1) {
                *f = 1u32 << (32 - l);
            }
            started = true;
        } else {
            // The longest free code no longer than this one.
            let Some(z) = (1..=len).rev().find(|&z| free[z] != 0) else {
                return Err(Error::Corrupt("Vorbis codebook is overspecified"));
            };
            code = free[z];
            free[z] = 0;
            for (y, f) in free.iter_mut().enumerate().take(len + 1).skip(z + 1).rev() {
                *f = code + (1u32 << (32 - y));
            }
        }
        codes[i] = code.reverse_bits();
    }
    Ok(codes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_come_low_first() {
        let mut b = Bits::new(&[0b1010_0110, 0xff]);
        assert_eq!(b.read(3), Some(0b110));
        assert_eq!(b.read(5), Some(0b10100));
        assert_eq!(b.read(4), Some(0xf));
    }

    #[test]
    fn huffman_codes() {
        // The specification's example: lengths 2 4 4 4 4 2 3 3 give 00 0100 0101 0110 0111 10
        // 110 111 (most significant bit first).
        let lengths = [2u8, 4, 4, 4, 4, 2, 3, 3];
        let codes = huffman(&lengths).unwrap();
        let msb: Vec<u32> = codes.iter().zip(&lengths).map(|(&c, &l)| c.reverse_bits() >> (32 - l)).collect();
        assert_eq!(msb, [0b00, 0b0100, 0b0101, 0b0110, 0b0111, 0b10, 0b110, 0b111]);
    }

    #[test]
    fn ilog_and_lookup() {
        assert_eq!((ilog(0), ilog(1), ilog(7), ilog(8)), (0, 1, 3, 4));
        assert_eq!(lookup1_values(81, 4), 3);
        assert_eq!(lookup1_values(80, 4), 2);
    }
}
