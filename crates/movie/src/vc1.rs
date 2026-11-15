//! VC-1 Advanced profile (SMPTE 421M), the video of the games' movies.
//!
//! In ASF (`WVC1`) the stream's codec data holds one byte, then the sequence header
//! (start code `0x0F`) and an entry point (`0x0E`); each media object is one frame, escaped but
//! without a start code of its own (fields `0x0C`, slices `0x0B` or a new entry point inside it
//! come with theirs).

use crate::bitplane::{Bitplane, Tables as BitplaneTables};
use crate::bits::{Bits, unescape, units};

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "vc1: {}", self.0)
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

/// The sequence header (6.1.1 to 6.1.8, advanced profile).
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub level: u8,
    pub chroma_format: u8,
    /// Pictures carry `POSTPROC`.
    pub postproc: bool,
    pub max_width: u32,
    pub max_height: u32,
    /// Pictures carry repeat counts (`RPTFRM`, progressive) or field flags (`TFF`/`RFF`).
    pub pulldown: bool,
    pub interlace: bool,
    /// Pictures carry an 8-bit frame counter (`TFCNTR`).
    pub frame_counter: bool,
    /// Pictures carry `INTERPFRM`.
    pub frame_interpolation: bool,
    pub psf: bool,
    pub display: Option<(u32, u32)>,
    /// Display frame rate as numerator / denominator.
    pub frame_rate: Option<(u32, u32)>,
    pub hrd_buckets: u8,
}

/// The entry point header (6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct EntryPoint {
    pub broken_link: bool,
    pub closed: bool,
    pub panscan: bool,
    pub refdist: bool,
    pub loop_filter: bool,
    pub fast_uvmc: bool,
    pub extended_mv: bool,
    pub dquant: u8,
    pub vstransform: bool,
    pub overlap: bool,
    pub quantizer: u8,
    pub coded_size: Option<(u32, u32)>,
    pub extended_dmv: bool,
    pub range_map_y: Option<u8>,
    pub range_map_uv: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureType {
    I,
    P,
    B,
    BI,
    Skipped,
}

/// The first fields of a progressive picture header (7.1.1): its type and how many times more
/// it is shown (`RPTFRM`, 0 to 3, with `PULLDOWN`).
#[derive(Debug, Clone, PartialEq)]
pub struct PictureStart {
    pub kind: PictureType,
    pub frame_counter: Option<u8>,
    pub repeat: u8,
}

/// The NTSC-style rates `FRAMERATENR` names (1 to 7), and the `FRAMERATEDR` divisors (1, 2).
const RATE_NR: [u32; 7] = [24, 25, 30, 50, 60, 48, 72];

impl Sequence {
    pub fn parse(body: &[u8]) -> Result<Self> {
        let data = unescape(body);
        let mut b = Bits::new(&data);
        if b.read(2) != 3 {
            return Err(Error("not advanced profile".into()));
        }
        let level = b.read(3) as u8;
        let chroma_format = b.read(2) as u8;
        b.read(3); // FRMRTQ_POSTPROC
        b.read(5); // BITRTQ_POSTPROC
        let postproc = b.bit();
        let max_width = b.read(12) * 2 + 2;
        let max_height = b.read(12) * 2 + 2;
        let pulldown = b.bit();
        let interlace = b.bit();
        let frame_counter = b.bit();
        let frame_interpolation = b.bit();
        b.read(1);
        let psf = b.bit();
        let (mut display, mut frame_rate) = (None, None);
        if b.bit() {
            display = Some((b.read(14) + 1, b.read(14) + 1));
            if b.bit() && b.read(4) == 15 {
                b.read(16);
            }
            if b.bit() {
                frame_rate = Some(if b.bit() {
                    (b.read(16) + 1, 32)
                } else {
                    let nr = b.read(8);
                    let dr = b.read(4);
                    let n = RATE_NR.get(nr.wrapping_sub(1) as usize).copied().unwrap_or(0);
                    if dr == 2 { (n * 1000, 1001) } else { (n, 1) }
                });
            }
            if b.bit() {
                b.read(24);
            }
        }
        let mut hrd_buckets = 0;
        if b.bit() {
            hrd_buckets = b.read(5) as u8;
            b.read(8);
            b.skip(32 * hrd_buckets as u32);
        }
        if b.over() {
            return Err(Error("sequence header runs out".into()));
        }
        Ok(Sequence { level, chroma_format, postproc, max_width, max_height, pulldown, interlace, frame_counter, frame_interpolation, psf, display, frame_rate, hrd_buckets })
    }

    /// The start of a picture header: its type, frame counter and repeat count.
    pub fn picture_start(&self, frame: &[u8]) -> Result<PictureStart> {
        let data = unescape(&frame[..frame.len().min(16)]);
        let mut b = Bits::new(&data[..]);
        if self.interlace && b.bit() {
            return Err(Error("interlaced pictures (FCM) are not supported".into()));
        }
        let kind = match (b.bit(), b.bit(), b.bit(), b.bit()) {
            (false, ..) => PictureType::P,
            (true, false, ..) => PictureType::B,
            (true, true, false, _) => PictureType::I,
            (true, true, true, false) => PictureType::BI,
            (true, true, true, true) => PictureType::Skipped,
        };
        // The bits read past the type's code go back.
        let used = match kind {
            PictureType::P => 1,
            PictureType::B => 2,
            PictureType::I => 3,
            _ => 4,
        };
        let mut b = Bits::new(&data);
        b.skip(self.interlace as u32 + used);
        let frame_counter = self.frame_counter.then(|| b.read(8) as u8);
        let mut repeat = 0;
        if self.pulldown {
            if !self.interlace || self.psf {
                repeat = b.read(2) as u8;
            } else {
                b.read(2); // TFF, RFF
            }
        }
        Ok(PictureStart { kind, frame_counter, repeat })
    }
}

/// Table 36: PQUANT for PQINDEX 0 to 31 with the implicit quantizer (non-uniform from 9).
const PQUANT_IMPLICIT: [u8; 32] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 27, 29, 31];

/// An I or BI picture's header (Table 18), progressive.
#[derive(Debug, Clone, PartialEq)]
pub struct IntraHeader {
    pub kind: PictureType,
    pub repeat: u8,
    pub round_control: bool,
    pub pqindex: u8,
    pub pquant: u8,
    pub half_qp: bool,
    pub uniform: bool,
    pub ac_pred: Bitplane,
    /// `CONDOVER`: 0 no overlap smoothing, 1 all macroblocks, 2 by `over_flags`; `None` when the
    /// picture does not carry it (overlap off or PQUANT above 8: smoothing by PQUANT alone).
    pub cond_over: Option<u8>,
    pub over_flags: Option<Bitplane>,
    /// The coding sets for the colour-difference (`TRANSACFRM`) and luma (`TRANSACFRM2`) blocks.
    pub ac_set_chroma: u8,
    pub ac_set_luma: u8,
    /// `TRANSDCTAB`: the high-motion DC tables.
    pub dc_high: bool,
    /// Where the macroblock layer starts, in bits into the unescaped frame.
    pub mb_start: usize,
}

impl Sequence {
    /// Macroblock columns and rows for a coded size.
    pub fn macroblocks(&self, entry: &EntryPoint) -> (usize, usize) {
        let (w, h) = entry.coded_size.unwrap_or((self.max_width, self.max_height));
        (w.div_ceil(16) as usize, h.div_ceil(16) as usize)
    }

    /// Reads an I or BI picture's header from the unescaped frame.
    pub fn intra_header(&self, entry: &EntryPoint, data: &[u8], planes: &BitplaneTables) -> Result<IntraHeader> {
        let start = self.picture_start(data)?;
        if !matches!(start.kind, PictureType::I | PictureType::BI) {
            return Err(Error(format!("{:?} picture, not intra", start.kind)));
        }
        if entry.panscan || entry.quantizer != 0 || entry.dquant != 0 {
            return Err(Error("pan-scan, an explicit quantizer or DQUANT: not supported yet".into()));
        }
        let mut b = Bits::new(data);
        b.skip(self.interlace as u32 + if start.kind == PictureType::I { 3 } else { 4 });
        if self.frame_counter {
            b.read(8);
        }
        if self.pulldown {
            b.read(2);
        }
        let round_control = b.bit();
        if self.interlace {
            b.read(1);
        }
        if self.frame_interpolation {
            b.read(1);
        }
        let pqindex = b.read(5) as u8;
        if pqindex == 0 {
            return Err(Error("PQINDEX 0".into()));
        }
        let half_qp = pqindex <= 8 && b.bit();
        let pquant = PQUANT_IMPLICIT[pqindex as usize];
        let uniform = pqindex <= 8;
        if self.postproc {
            b.read(2);
        }
        let (cols, rows) = self.macroblocks(entry);
        let ac_pred = planes.read(&mut b, cols, rows).ok_or_else(|| Error("ACPRED bitplane".into()))?;
        let (mut cond_over, mut over_flags) = (None, None);
        if entry.overlap && pquant <= 8 {
            let c = if !b.bit() { 0 } else if !b.bit() { 1 } else { 2 };
            if c == 2 {
                over_flags = Some(planes.read(&mut b, cols, rows).ok_or_else(|| Error("OVERFLAGS bitplane".into()))?);
            }
            cond_over = Some(c);
        }
        let set = |b: &mut Bits| if !b.bit() { 0 } else if !b.bit() { 1 } else { 2 };
        let ac_set_chroma = set(&mut b);
        let ac_set_luma = set(&mut b);
        let dc_high = b.bit();
        if b.over() {
            return Err(Error("picture header runs out".into()));
        }
        Ok(IntraHeader { kind: start.kind, repeat: start.repeat, round_control, pqindex, pquant, half_qp, uniform, ac_pred, cond_over, over_flags, ac_set_chroma, ac_set_luma, dc_high, mb_start: b.position() })
    }
}

/// A P picture's header (Table 20), progressive.
#[derive(Debug, Clone, PartialEq)]
pub struct InterHeader {
    pub repeat: u8,
    pub round_control: bool,
    pub pqindex: u8,
    pub pquant: u8,
    pub half_qp: bool,
    pub uniform: bool,
    /// MVRANGE 0 to 3 (Table 37).
    pub mv_range: u8,
    /// The motion mode, MVMODE2's when MVMODE asks for intensity compensation.
    pub mv_mode: crate::vc1_small::MvMode,
    /// LUMSCALE and LUMSHIFT with intensity compensation.
    pub intensity: Option<(u8, u8)>,
    /// MVTYPEMB with mixed motion vectors: one or four vectors a macroblock.
    pub mv_type: Option<Bitplane>,
    pub skipped: Bitplane,
    pub mv_table: u8,
    pub cbp_table: u8,
    /// TTFRM: the transform for every inter block when TTMBF is set.
    pub frame_transform: Option<u8>,
    /// TTMBF off: each macroblock signals its transform.
    pub mb_transform: bool,
    pub ac_set: u8,
    pub dc_high: bool,
    pub mb_start: usize,
}

/// The P header's small tables, built once.
pub struct InterTables {
    mvmode: [crate::vc1_small::Coded<crate::vc1_small::MvMode>; 2],
    mvmode2: [crate::vc1_small::Coded<crate::vc1_small::MvMode>; 2],
    mvrange: crate::vc1_small::Coded<u8>,
}

impl Default for InterTables {
    fn default() -> Self {
        use crate::vc1_small::*;
        InterTables {
            mvmode: [Coded::new(MVMODE_LOW, |r| *r), Coded::new(MVMODE_HIGH, |r| *r)],
            mvmode2: [Coded::new(MVMODE2_LOW, |r| *r), Coded::new(MVMODE2_HIGH, |r| *r)],
            mvrange: Coded::new(MVRANGE, |r| *r),
        }
    }
}

impl Sequence {
    /// Reads a P picture's header from the unescaped frame.
    pub fn inter_header(&self, entry: &EntryPoint, data: &[u8], planes: &BitplaneTables, tables: &InterTables) -> Result<InterHeader> {
        use crate::vc1_small::MvMode;
        let start = self.picture_start(data)?;
        if start.kind != PictureType::P {
            return Err(Error(format!("{:?} picture, not P", start.kind)));
        }
        if entry.panscan || entry.quantizer != 0 || entry.dquant != 0 {
            return Err(Error("pan-scan, an explicit quantizer or DQUANT: not supported yet".into()));
        }
        let mut b = Bits::new(data);
        b.skip(self.interlace as u32 + 1);
        if self.frame_counter {
            b.read(8);
        }
        if self.pulldown {
            b.read(2);
        }
        let round_control = b.bit();
        if self.interlace {
            b.read(1);
        }
        if self.frame_interpolation {
            b.read(1);
        }
        let pqindex = b.read(5) as u8;
        if pqindex == 0 {
            return Err(Error("PQINDEX 0".into()));
        }
        let half_qp = pqindex <= 8 && b.bit();
        let pquant = PQUANT_IMPLICIT[pqindex as usize];
        if self.postproc {
            b.read(2);
        }
        let mv_range = if entry.extended_mv { tables.mvrange.read(&mut b).ok_or_else(|| Error("MVRANGE".into()))? } else { 0 };
        let rate = (pquant <= 12) as usize;
        let mut mv_mode = tables.mvmode[rate].read(&mut b).ok_or_else(|| Error("MVMODE".into()))?;
        let mut intensity = None;
        if mv_mode == MvMode::IntensityCompensation {
            mv_mode = tables.mvmode2[rate].read(&mut b).ok_or_else(|| Error("MVMODE2".into()))?;
            intensity = Some((b.read(6) as u8, b.read(6) as u8));
        }
        let (cols, rows) = self.macroblocks(entry);
        let mv_type = if mv_mode == MvMode::Mixed { Some(planes.read(&mut b, cols, rows).ok_or_else(|| Error("MVTYPEMB bitplane".into()))?) } else { None };
        let skipped = planes.read(&mut b, cols, rows).ok_or_else(|| Error("SKIPMB bitplane".into()))?;
        let mv_table = b.read(2) as u8;
        let cbp_table = b.read(2) as u8;
        let (mut frame_transform, mut mb_transform) = (None, false);
        if entry.vstransform {
            if b.bit() {
                frame_transform = Some(b.read(2) as u8);
            } else {
                mb_transform = true;
            }
        }
        let ac_set = if !b.bit() { 0 } else if !b.bit() { 1 } else { 2 };
        let dc_high = b.bit();
        if b.over() {
            return Err(Error("picture header runs out".into()));
        }
        Ok(InterHeader { repeat: start.repeat, round_control, pqindex, pquant, half_qp, uniform: pqindex <= 8, mv_range, mv_mode, intensity, mv_type, skipped, mv_table, cbp_table, frame_transform, mb_transform, ac_set, dc_high, mb_start: b.position() })
    }
}

impl EntryPoint {
    pub fn parse(body: &[u8], sequence: &Sequence) -> Result<Self> {
        let data = unescape(body);
        let mut b = Bits::new(&data);
        let broken_link = b.bit();
        let closed = b.bit();
        let panscan = b.bit();
        let refdist = b.bit();
        let loop_filter = b.bit();
        let fast_uvmc = b.bit();
        let extended_mv = b.bit();
        let dquant = b.read(2) as u8;
        let vstransform = b.bit();
        let overlap = b.bit();
        let quantizer = b.read(2) as u8;
        b.skip(8 * sequence.hrd_buckets as u32);
        let coded_size = b.bit().then(|| (b.read(12) * 2 + 2, b.read(12) * 2 + 2));
        let extended_dmv = extended_mv && b.bit();
        let range_map_y = b.bit().then(|| b.read(3) as u8);
        let range_map_uv = b.bit().then(|| b.read(3) as u8);
        if b.over() {
            return Err(Error("entry point runs out".into()));
        }
        Ok(EntryPoint { broken_link, closed, panscan, refdist, loop_filter, fast_uvmc, extended_mv, dquant, vstransform, overlap, quantizer, coded_size, extended_dmv, range_map_y, range_map_uv })
    }
}

/// The stream's headers from its ASF codec data.
pub fn headers(codec_data: &[u8]) -> Result<(Sequence, EntryPoint)> {
    let (mut sequence, mut entry) = (None, None);
    for (code, body) in units(codec_data) {
        match code {
            Some(0x0f) => sequence = Some(Sequence::parse(body)?),
            Some(0x0e) => {
                let s = sequence.as_ref().ok_or_else(|| Error("entry point before the sequence header".into()))?;
                entry = Some(EntryPoint::parse(body, s)?);
            }
            _ => {}
        }
    }
    Ok((sequence.ok_or_else(|| Error("no sequence header".into()))?, entry.ok_or_else(|| Error("no entry point".into()))?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `avantitle.wmv`'s codec data, as the file holds it.
    const AVANTITLE: [u8; 31] = [
        0x25, 0x00, 0x00, 0x01, 0x0f, 0xdb, 0xfe, 0x3b, 0xf2, 0x1b, 0x8a, 0x3b, 0xf8, 0x86, 0xf1, 0x80, 0xc9, 0x09, 0x36, 0xac, 0xf7, 0x27, 0x04, 0x00, 0x00, 0x01, 0x0e, 0x5a, 0x67, 0xf8, 0x40,
    ];

    #[test]
    fn avantitles_sequence_header() {
        // The last bytes of the entry point are cut here: only the sequence header is checked.
        let units = units(&AVANTITLE);
        let s = Sequence::parse(units.iter().find(|u| u.0 == Some(0x0f)).unwrap().1).unwrap();
        assert_eq!((s.max_width, s.max_height, s.pulldown, s.interlace, s.level), (1920, 1080, true, false, 3));
    }
}

#[cfg(test)]
mod movie_tests {
    use super::*;
    use barrel_formats::asf::{Asf, Kind};

    /// The real pre-title movie (`BARREL_MOVIE`): its headers and each frame's type and repeat.
    #[test]
    #[ignore]
    fn avantitle_headers_and_frame_types() {
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let bytes = std::fs::read(path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let video = asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).unwrap();
        let Kind::Video { extra, .. } = &video.kind else { unreachable!() };
        let (s, e) = headers(extra).unwrap();
        eprintln!("{s:?}\n{e:?}");
        let mut counts = std::collections::BTreeMap::new();
        let (mut shown, mut codes) = (0u32, std::collections::BTreeMap::new());
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == video.number) {
            for (code, _) in units(&o.data) {
                *codes.entry(code).or_insert(0) += 1;
            }
            let p = s.picture_start(&o.data).unwrap();
            *counts.entry((format!("{:?}", p.kind), p.repeat, o.key)).or_insert(0) += 1;
            shown += 1 + p.repeat as u32;
        }
        eprintln!("{counts:?}\nunits {codes:?}\nshown {shown}");
        let planes = crate::bitplane::Tables::default();
        let mut seen = std::collections::BTreeMap::new();
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == video.number) {
            let data = unescape(&o.data);
            if !matches!(s.picture_start(&o.data).unwrap().kind, PictureType::I | PictureType::BI) {
                continue;
            }
            let h = s.intra_header(&e, &data, &planes).unwrap();
            let plane = |p: &Bitplane| match p { Bitplane::Raw => "raw".to_string(), Bitplane::Bits(v) => format!("{}set", v.iter().filter(|&&x| x != 0).count()) };
            *seen.entry((h.pqindex, h.cond_over, h.ac_set_luma, h.ac_set_chroma, h.dc_high)).or_insert(0) += 1;
            if seen.len() < 4 {
                eprintln!("{:?} pq {} acpred {} over {:?} {:?} mb at {}", h.kind, h.pquant, plane(&h.ac_pred), h.cond_over, h.over_flags.as_ref().map(plane), h.mb_start);
            }
        }
        eprintln!("(pqindex, condover, set luma, set chroma, dc high): {seen:?}");
        let inter = InterTables::default();
        let mut p_seen = std::collections::BTreeMap::new();
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == video.number) {
            if s.picture_start(&o.data).unwrap().kind != PictureType::P {
                continue;
            }
            let data = unescape(&o.data);
            let h = s.inter_header(&e, &data, &planes, &inter).unwrap();
            let skip = match &h.skipped { Bitplane::Raw => "raw", _ => "plane" };
            *p_seen.entry(format!("{:?} range {} ic {} mvtab {} cbptab {} tt {:?}/{} {}", h.mv_mode, h.mv_range, h.intensity.is_some(), h.mv_table, h.cbp_table, h.frame_transform, h.mb_transform, skip)).or_insert(0) += 1;
        }
        for (k, n) in &p_seen {
            eprintln!("P {n}: {k}");
        }
        assert!(s.pulldown && !s.interlace);
    }
}

#[cfg(test)]
mod table_tests {
    use crate::vc1_tables::*;

    /// The generated tables hang together: each AC set's run/level rows cover its codes but the
    /// escape, and the intra scans start as the standard prints them.
    #[test]
    fn tables_hang_together() {
        assert_eq!(HIGH_MOTION_INTRA_RUN_LEVEL_0.len() + HIGH_MOTION_INTRA_RUN_LEVEL_1.len() + 1, HIGH_MOTION_INTRA_VLC.len());
        assert_eq!(HIGH_MOTION_INTRA_LAST_FROM, HIGH_MOTION_INTRA_RUN_LEVEL_0.len());
        assert_eq!(&INTRA_NORMAL_SCAN[..8], &[0, 8, 1, 2, 9, 16, 24, 17]);
        assert_eq!(I_CBPCY[0], (1, 1));
        assert_eq!(DC_LOW_LUMA.len(), 120);
    }
}
