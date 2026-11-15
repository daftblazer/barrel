//! VC-1 Advanced profile (SMPTE 421M), the video of the games' movies.
//!
//! In ASF (`WVC1`) the stream's codec data holds one byte, then the sequence header
//! (start code `0x0F`) and an entry point (`0x0E`); each media object is one frame, escaped but
//! without a start code of its own (fields `0x0C`, slices `0x0B` or a new entry point inside it
//! come with theirs).

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
        b.read(1); // POSTPROCFLAG
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
        Ok(Sequence { level, chroma_format, max_width, max_height, pulldown, interlace, frame_counter, frame_interpolation, psf, display, frame_rate, hrd_buckets })
    }

    /// The start of a picture header: its type, frame counter and repeat count.
    pub fn picture_start(&self, frame: &[u8]) -> Result<PictureStart> {
        let data = unescape(&frame[..frame.len().min(16)]);
        let mut b = Bits::new(&data);
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
