//! Ogg Vorbis, decoded natively (the game's voices, music and sounds; `voice::ogg` unwraps the
//! voice clips). Written from the Vorbis I specification (Xiph.Org, 2015) and RFC 3533 (Ogg).
//!
//! So far: Ogg pages into packets, the identification header and the whole setup header
//! (codebooks, floors, residues, mappings, modes) and the audio packets ([`Decoder`], [`decode`]).

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
    /// The decode tree: per node, the next node for a 0 and a 1 bit (0: none; below 0: the
    /// entry -1 - value).
    tree: Vec<[i32; 2]>,
}

impl Codebook {
    /// Reads one entry's number from the packet (`None` at the end of the packet or on a
    /// code no entry has).
    pub fn decode(&self, bits: &mut Bits) -> Option<usize> {
        // A codebook with a single used entry of length 1 still reads its one bit.
        let mut node = 0usize;
        loop {
            let next = *self.tree.get(node)?.get(bits.read(1)? as usize)?;
            match next {
                0 => return None,
                n if n < 0 => return Some((-1 - n) as usize),
                n => node = n as usize,
            }
        }
    }

    /// The vector of entry `e`.
    pub fn vector(&self, e: usize) -> &[f32] {
        &self.vectors[e * self.dimensions..(e + 1) * self.dimensions]
    }
}

fn tree(lengths: &[u8], codes: &[u32]) -> Vec<[i32; 2]> {
    let mut t = vec![[0i32; 2]];
    for (e, (&len, &code)) in lengths.iter().zip(codes).enumerate() {
        let mut node = 0usize;
        for b in 0..len as u32 {
            let bit = ((code >> b) & 1) as usize;
            if b + 1 == len as u32 {
                t[node][bit] = -1 - e as i32;
            } else {
                if t[node][bit] <= 0 {
                    t.push([0, 0]);
                    t[node][bit] = (t.len() - 1) as i32;
                }
                node = t[node][bit] as usize;
            }
        }
    }
    t
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
    let tree = tree(&lengths, &codes);
    Ok(Codebook { dimensions, lengths, codes, vectors, tree })
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

/// Floor 1's amplitude to linear table (the specification's `floor1_inverse_dB_table`): 140 dB
/// over 255 steps of 0.5469 dB, 1.0 at the top.
fn inverse_db(i: usize) -> f32 {
    (1.064_986_3f64.powf(i as f64 - 255.0)) as f32
}

fn low_neighbor(xs: &[u32], i: usize) -> usize {
    (0..i).filter(|&n| xs[n] < xs[i]).max_by_key(|&n| xs[n]).unwrap_or(0)
}

fn high_neighbor(xs: &[u32], i: usize) -> usize {
    (0..i).filter(|&n| xs[n] > xs[i]).min_by_key(|&n| xs[n]).unwrap_or(0)
}

fn render_point(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> i32 {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let off = dy.abs() * (x - x0) / adx;
    if dy < 0 { y0 - off } else { y0 + off }
}

fn render_line(x0: i32, y0: i32, x1: i32, y1: i32, v: &mut [i32]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let base = dy / adx;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let ady = dy.abs() - base.abs() * adx;
    let (mut y, mut err) = (y0, 0);
    let n = v.len() as i32;
    if x0 < n {
        v[x0 as usize] = y;
    }
    for x in x0 + 1..x1.min(n) {
        err += ady;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
        v[x as usize] = y;
    }
}

/// Decodes a channel's floor 1 from the packet (section 7.2.2): its Y values, or `None` when
/// the channel is unused in this packet.
fn floor1_decode(f: &Floor1, books: &[Codebook], bits: &mut Bits) -> Option<Option<Vec<i32>>> {
    if !bits.flag()? {
        return Some(None);
    }
    let range = [256, 128, 86, 64][f.multiplier as usize - 1];
    let width = ilog(range - 1);
    let mut y = vec![bits.read(width)? as i32, bits.read(width)? as i32];
    for &c in &f.partitions {
        let class = &f.classes[c as usize];
        let csub = (1u32 << class.subclasses) - 1;
        let mut cval = if class.subclasses > 0 { books[class.masterbook as usize].decode(bits)? as u32 } else { 0 };
        for _ in 0..class.dimensions {
            let book = class.subbooks[(cval & csub) as usize];
            cval >>= class.subclasses;
            y.push(if book >= 0 { books[book as usize].decode(bits)? as i32 } else { 0 });
        }
    }
    Some(Some(y))
}

/// Floor 1's curve over `n2` = n / 2 bins (section 7.2.4).
fn floor1_curve(f: &Floor1, ys: &[i32], n2: usize) -> Vec<f32> {
    let range = [256, 128, 86, 64][f.multiplier as usize - 1];
    let count = f.xs.len();
    let mut used = vec![false; count];
    let mut fy = vec![0i32; count];
    used[0] = true;
    used[1] = true;
    fy[0] = ys[0];
    fy[1] = ys[1];
    for i in 2..count {
        let low = low_neighbor(&f.xs, i);
        let high = high_neighbor(&f.xs, i);
        let predicted = render_point(f.xs[low] as i32, fy[low], f.xs[high] as i32, fy[high], f.xs[i] as i32);
        let val = ys[i];
        let highroom = range - predicted;
        let lowroom = predicted;
        let room = if highroom < lowroom { highroom * 2 } else { lowroom * 2 };
        if val != 0 {
            used[low] = true;
            used[high] = true;
            used[i] = true;
            fy[i] = if val >= room {
                if highroom > lowroom { val - lowroom + predicted } else { predicted - val + highroom - 1 }
            } else if val % 2 == 1 {
                predicted - (val + 1) / 2
            } else {
                predicted + val / 2
            };
        } else {
            fy[i] = predicted;
        }
    }
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|&i| f.xs[i]);
    let mult = f.multiplier as i32;
    let mut v = vec![0i32; n2];
    let (mut lx, mut ly) = (0i32, fy[order[0]] * mult);
    for &i in &order[1..] {
        if used[i] {
            let (hx, hy) = (f.xs[i] as i32, fy[i] * mult);
            render_line(lx, ly, hx, hy, &mut v);
            lx = hx;
            ly = hy;
        }
    }
    for x in (lx.max(0) as usize)..n2 {
        v[x] = ly;
    }
    v.iter().map(|&y| inverse_db(y.clamp(0, 255) as usize)).collect()
}

/// Decodes a residue into `vectors` (each n / 2 long) for the channels not marked `skip`
/// (section 8.6).
fn residue_decode(r: &Residue, books: &[Codebook], bits: &mut Bits, vectors: &mut [Vec<f32>], skip: &[bool], n2: usize) {
    if r.kind == 2 {
        if skip.iter().all(|&s| s) {
            return;
        }
        let ch = vectors.len();
        let mut joined = vec![vec![0f32; n2 * ch]];
        residue_partitions(r, books, bits, &mut joined, &[false], n2 * ch, 1);
        for (i, v) in joined[0].iter().enumerate() {
            vectors[i % ch][i / ch] = *v;
        }
        return;
    }
    residue_partitions(r, books, bits, vectors, skip, n2, r.kind);
}

fn residue_partitions(r: &Residue, books: &[Codebook], bits: &mut Bits, vectors: &mut [Vec<f32>], skip: &[bool], size: usize, kind: u16) {
    let begin = (r.begin as usize).min(size);
    let end = (r.end as usize).min(size);
    let psize = r.partition_size as usize;
    let classbook = &books[r.classbook as usize];
    let classwords = classbook.dimensions.max(1);
    let to_read = end.saturating_sub(begin);
    let partitions = to_read / psize;
    if partitions == 0 {
        return;
    }
    let mut classes = vec![vec![0usize; partitions + classwords]; vectors.len()];
    'passes: for pass in 0..8 {
        let mut p = 0;
        while p < partitions {
            if pass == 0 {
                for (ch, class) in classes.iter_mut().enumerate() {
                    if skip[ch] {
                        continue;
                    }
                    let Some(mut temp) = classbook.decode(bits) else { break 'passes };
                    for i in (0..classwords).rev() {
                        class[p + i] = temp % r.classifications as usize;
                        temp /= r.classifications as usize;
                    }
                }
            }
            for _ in 0..classwords {
                if p >= partitions {
                    break;
                }
                for ch in 0..vectors.len() {
                    if skip[ch] {
                        continue;
                    }
                    let book = r.books[classes[ch][p]][pass];
                    if book < 0 {
                        continue;
                    }
                    let book = &books[book as usize];
                    let offset = begin + p * psize;
                    let v = &mut vectors[ch];
                    let dim = book.dimensions;
                    if kind == 0 {
                        let step = psize / dim;
                        for j in 0..step {
                            let Some(e) = book.decode(bits) else { break 'passes };
                            for (k, x) in book.vector(e).iter().enumerate() {
                                v[offset + j + k * step] += x;
                            }
                        }
                    } else {
                        let mut i = 0;
                        while i < psize {
                            let Some(e) = book.decode(bits) else { break 'passes };
                            for x in book.vector(e) {
                                if i < psize {
                                    v[offset + i] += x;
                                }
                                i += 1;
                            }
                        }
                    }
                }
                p += 1;
            }
        }
    }
}

/// The Vorbis window's value at `i` of a slope `n` long (section 4.3.1).
fn slope(i: usize, n: usize) -> f32 {
    let x = (i as f64 + 0.5) / n as f64 * std::f64::consts::FRAC_PI_2;
    (std::f64::consts::FRAC_PI_2 * x.sin().powi(2)).sin() as f32
}

/// An in-place radix-2 complex FFT (e^-2πi nk/N), N a power of two.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// The DCT-IV of `x` (M points, M a power of two): u[k] = Σ x[n] cos(π / M (n + ½)(k + ½)),
/// through an M/2-point complex FFT.
fn dct4(x: &[f32]) -> Vec<f64> {
    let m = x.len();
    let h = m / 2;
    let pi = std::f64::consts::PI;
    let (mut re, mut im) = (vec![0f64; h], vec![0f64; h]);
    for n in 0..h {
        let (a, b) = (x[2 * n] as f64, x[m - 1 - 2 * n] as f64);
        let t = -pi * (4 * n + 1) as f64 / (4 * m) as f64;
        let (c, s) = (t.cos(), t.sin());
        re[n] = a * c - b * s;
        im[n] = a * s + b * c;
    }
    fft(&mut re, &mut im);
    let mut u = vec![0f64; m];
    for k in 0..h {
        let t = -pi * k as f64 / m as f64;
        let (c, s) = (t.cos(), t.sin());
        let (r, i) = (re[k] * c - im[k] * s, re[k] * s + im[k] * c);
        u[2 * k] = r;
        u[m - 1 - 2 * k] = -i;
    }
    u
}

/// The inverse MDCT of `x` (n / 2 coefficients) into n samples, y[i] = Σ x[k] cos(π / (2n)
/// (2i + 1 + n / 2)(2k + 1)): the DCT-IV of the coefficients, unfolded.
fn imdct(x: &[f32]) -> Vec<f32> {
    let n2 = x.len();
    let n = n2 * 2;
    let n4 = n / 4;
    let u = dct4(x);
    (0..n)
        .map(|i| {
            (if i < n4 {
                u[n4 + i]
            } else if i < 3 * n4 {
                -u[3 * n4 - 1 - i]
            } else {
                -u[i - 3 * n4]
            }) as f32
        })
        .collect()
}

/// The inverse MDCT straight from its definition (the reference for [`imdct`]).
#[cfg(test)]
fn imdct_direct(x: &[f32]) -> Vec<f32> {
    let n2 = x.len();
    let n = n2 * 2;
    let f = std::f64::consts::PI / (2.0 * n as f64);
    (0..n)
        .map(|i| {
            let a = (2 * i + 1 + n / 2) as f64;
            x.iter().enumerate().map(|(k, &c)| c as f64 * (f * a * (2 * k + 1) as f64).cos()).sum::<f64>() as f32
        })
        .collect()
}

/// A stream being decoded, packet by packet.
pub struct Decoder {
    pub id: Identification,
    pub setup: Setup,
    /// The previous block's right half, per channel, and its size.
    previous: Option<(usize, Vec<Vec<f32>>)>,
}

impl Decoder {
    pub fn new(id: Identification, setup: Setup) -> Decoder {
        Decoder { id, setup, previous: None }
    }

    /// Decodes one audio packet, returning the samples it completes per channel (none for
    /// the first packet, which only fills the overlap).
    pub fn packet(&mut self, packet: &[u8]) -> Result<Vec<Vec<f32>>, Error> {
        let e = || Error::Corrupt("Vorbis audio packet past its end");
        let mut bits = Bits::new(packet);
        if bits.flag().ok_or_else(e)? {
            return Err(Error::Corrupt("Vorbis audio packet type"));
        }
        let modes = self.setup.modes.len() as u32;
        let mode = self.setup.modes[bits.read(ilog(modes - 1)).ok_or_else(e)? as usize];
        let n = self.id.blocksize[usize::from(mode.long)];
        let short = self.id.blocksize[0];
        let (prev_long, next_long) = if mode.long { (bits.flag().ok_or_else(e)?, bits.flag().ok_or_else(e)?) } else { (false, false) };
        let n2 = n / 2;
        let mapping = &self.setup.mappings[mode.mapping as usize];
        let ch = usize::from(self.id.channels);
        let books = &self.setup.codebooks;
        // Floors.
        let mut floors: Vec<Option<(usize, Vec<i32>)>> = Vec::with_capacity(ch);
        for c in 0..ch {
            let submap = mapping.mux[c] as usize;
            let fi = mapping.submaps[submap].0 as usize;
            match &self.setup.floors[fi] {
                Floor::One(f) => {
                    let y = floor1_decode(f, books, &mut bits);
                    // A packet that ends early leaves the rest silent (the specification's rule).
                    floors.push(y.flatten().map(|y| (fi, y)));
                }
                Floor::Zero { .. } => return Err(Error::Corrupt("Vorbis floor 0 not supported")),
            }
        }
        let mut skip: Vec<bool> = floors.iter().map(Option::is_none).collect();
        for &(m, a) in &mapping.coupling {
            if !skip[m as usize] || !skip[a as usize] {
                skip[m as usize] = false;
                skip[a as usize] = false;
            }
        }
        // Residues, per submap.
        let mut vectors = vec![vec![0f32; n2]; ch];
        for (s, &(_, ri)) in mapping.submaps.iter().enumerate() {
            let chans: Vec<usize> = (0..ch).filter(|&c| mapping.mux[c] as usize == s).collect();
            let mut sub: Vec<Vec<f32>> = chans.iter().map(|_| vec![0f32; n2]).collect();
            let sub_skip: Vec<bool> = chans.iter().map(|&c| skip[c]).collect();
            residue_decode(&self.setup.residues[ri as usize], books, &mut bits, &mut sub, &sub_skip, n2);
            for (k, &c) in chans.iter().enumerate() {
                vectors[c] = std::mem::take(&mut sub[k]);
            }
        }
        // Inverse coupling.
        for &(m, a) in mapping.coupling.iter().rev() {
            let (m, a) = (m as usize, a as usize);
            for i in 0..n2 {
                let (mv, av) = (vectors[m][i], vectors[a][i]);
                let (nm, na) = if mv > 0.0 {
                    if av > 0.0 { (mv, mv - av) } else { (mv + av, mv) }
                } else if av > 0.0 {
                    (mv, mv + av)
                } else {
                    (mv - av, mv)
                };
                vectors[m][i] = nm;
                vectors[a][i] = na;
            }
        }
        // Floor × residue, inverse MDCT, window.
        let (ls, le, ln) = if mode.long && !prev_long { (n / 4 - short / 4, n / 4 + short / 4, short / 2) } else { (0, n2, n2) };
        let (rs, re, rn) = if mode.long && !next_long { (3 * n / 4 - short / 4, 3 * n / 4 + short / 4, short / 2) } else { (n2, n, n2) };
        let mut blocks = Vec::with_capacity(ch);
        for c in 0..ch {
            let mut spectrum = vec![0f32; n2];
            if let Some((fi, y)) = &floors[c] {
                if let Floor::One(f) = &self.setup.floors[*fi] {
                    let curve = floor1_curve(f, y, n2);
                    for i in 0..n2 {
                        spectrum[i] = curve[i] * vectors[c][i];
                    }
                }
            }
            let mut out = imdct(&spectrum);
            for (i, v) in out.iter_mut().enumerate() {
                let w = if i < ls {
                    0.0
                } else if i < le {
                    slope(i - ls, ln)
                } else if i < rs {
                    1.0
                } else if i < re {
                    slope(rn - 1 - (i - rs), rn)
                } else {
                    0.0
                };
                *v *= w;
            }
            blocks.push(out);
        }
        // Overlap with the previous block's right half: what lies between the two centres.
        let out = match self.previous.take() {
            None => vec![Vec::new(); ch],
            Some((pn, prev)) => (0..ch)
                .map(|c| {
                    let cur = &blocks[c];
                    let p = &prev[c];
                    if pn == n {
                        (0..n2).map(|i| p[i] + cur[i]).collect()
                    } else if pn > n {
                        let off = pn / 4 - n / 4;
                        let mut o: Vec<f32> = p[..off].to_vec();
                        o.extend((0..n2).map(|i| p[off + i] + cur[i]));
                        o
                    } else {
                        let off = n / 4 - pn / 4;
                        let mut o: Vec<f32> = (0..pn / 2).map(|i| p[i] + cur[off + i]).collect();
                        o.extend_from_slice(&cur[off + pn / 2..n2]);
                        o
                    }
                })
                .collect(),
        };
        self.previous = Some((n, blocks.into_iter().map(|b| b[n2..].to_vec()).collect()));
        Ok(out)
    }
}

/// Decodes a whole Ogg Vorbis stream to samples per channel, cut to the last page's granule
/// position.
pub fn decode(ogg: &[u8]) -> Result<(Identification, Vec<Vec<f32>>), Error> {
    let packets = packets(ogg)?;
    let id = identification(packets.first().ok_or(Error::Corrupt("Vorbis stream empty"))?)?;
    let setup = setup(packets.get(2).ok_or(Error::Corrupt("Vorbis stream without its setup header"))?, id.channels)?;
    let mut d = Decoder::new(id, setup);
    let mut out = vec![Vec::new(); usize::from(id.channels)];
    for p in &packets[3..] {
        for (c, s) in d.packet(p)?.into_iter().enumerate() {
            out[c].extend(s);
        }
    }
    let last = ogg.windows(4).rposition(|w| w == b"OggS").and_then(|i| ogg.get(i + 6..i + 14)).map(|g| u64::from_le_bytes(g.try_into().unwrap()));
    if let Some(total) = last {
        for c in &mut out {
            c.truncate(total as usize);
        }
    }
    Ok((id, out))
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
    fn fast_imdct_matches_its_definition() {
        for n2 in [128usize, 1024] {
            let x: Vec<f32> = (0..n2).map(|k| ((k * 7919 % 1000) as f32 / 500.0 - 1.0) * if k % 3 == 0 { 2.0 } else { 0.5 }).collect();
            let (a, b) = (imdct(&x), imdct_direct(&x));
            let worst = a.iter().zip(&b).map(|(p, q)| (p - q).abs()).fold(0f32, f32::max);
            assert!(worst < 1e-2, "n2 {n2}: off by {worst}");
        }
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
