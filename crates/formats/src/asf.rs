//! ASF (Advanced Systems Format), the container of the games' `.wmv` movies: a header of
//! GUID-tagged objects (file properties, one stream properties object a stream, some of them inside
//! the header extension), then a data object of fixed-size packets whose payloads carry pieces of
//! each stream's media objects (a video frame, a block of audio frames).
//!
//! [`Asf::parse`] reads the header; [`Asf::objects`] walks the packets and hands back each media
//! object whole, in file order, with its stream, presentation time and key flag.

use std::fmt;

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "asf: {}", self.0)
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

fn err<T>(what: impl Into<String>) -> Result<T> {
    Err(Error(what.into()))
}

/// A GUID as the file stores it (the first three fields little-endian).
type Guid = [u8; 16];

const fn guid(a: u32, b: u16, c: u16, d: [u8; 8]) -> Guid {
    let (a, b, c) = (a.to_le_bytes(), b.to_le_bytes(), c.to_le_bytes());
    [a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]
}

const HEADER: Guid = guid(0x75B2_2630, 0x668E, 0x11CF, [0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C]);
const DATA: Guid = guid(0x75B2_2636, 0x668E, 0x11CF, [0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C]);
const FILE_PROPERTIES: Guid = guid(0x8CAB_DCA1, 0xA947, 0x11CF, [0x8E, 0xE4, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
const STREAM_PROPERTIES: Guid = guid(0xB7DC_0791, 0xA9B7, 0x11CF, [0x8E, 0xE6, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
const HEADER_EXTENSION: Guid = guid(0x5FBF_03B5, 0xA92E, 0x11CF, [0x8E, 0xE3, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
const EXTENDED_STREAM_PROPERTIES: Guid = guid(0x14E6_A5CB, 0xC672, 0x4332, [0x83, 0x99, 0xA9, 0x69, 0x52, 0x06, 0x5B, 0x5A]);
const AUDIO_MEDIA: Guid = guid(0xF869_9E40, 0x5B4D, 0x11CF, [0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B]);
const VIDEO_MEDIA: Guid = guid(0xBC19_EFC0, 0x5B4D, 0x11CF, [0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B]);

/// What a stream carries, from its stream properties' type-specific data.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// `WAVEFORMATEX`: the codec tag (0x162 WMA 9 Pro), channels, sample rate, average bytes a
    /// second, block align, bits a sample and the codec's own data after it.
    Audio { tag: u16, channels: u16, rate: u32, bytes_per_second: u32, block_align: u16, bits: u16, extra: Vec<u8> },
    /// The picture size and `BITMAPINFOHEADER`'s compression code (`WVC1`) with the codec's own
    /// data after it (for VC-1 Advanced: its sequence and entry-point headers).
    Video { width: u32, height: u32, fourcc: [u8; 4], extra: Vec<u8> },
    Other,
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub number: u8,
    pub kind: Kind,
}

/// A media object put back together from its payloads.
#[derive(Debug, Clone)]
pub struct Object {
    pub stream: u8,
    /// Presentation time in milliseconds, the preroll taken off.
    pub time_ms: i64,
    pub key: bool,
    pub data: Vec<u8>,
}

pub struct Asf<'a> {
    bytes: &'a [u8],
    pub streams: Vec<Stream>,
    /// Packet size (the file properties' minimum and maximum agree for these files).
    pub packet_size: usize,
    pub packet_count: u64,
    pub preroll_ms: u64,
    /// The play duration in 100 ns units, the preroll still in it.
    pub play_duration: u64,
    /// Where the first packet starts.
    data_start: usize,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| Error(format!("short read at {}", self.at)))?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn guid(&mut self) -> Result<Guid> {
        Ok(self.take(16)?.try_into().unwrap())
    }
    /// A field whose width a 2-bit length type gives (0 none, 1 byte, 2 word, 3 dword).
    fn sized(&mut self, ty: u8) -> Result<u32> {
        Ok(match ty & 3 {
            0 => 0,
            1 => self.u8()? as u32,
            2 => self.u16()? as u32,
            _ => self.u32()?,
        })
    }
}

/// Walks the GUID + size objects in `b`.
fn objects_in(b: &[u8]) -> impl Iterator<Item = Result<(Guid, &[u8])>> {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at + 24 > b.len() {
            return None;
        }
        let id: Guid = b[at..at + 16].try_into().unwrap();
        let size = u64::from_le_bytes(b[at + 16..at + 24].try_into().unwrap()) as usize;
        if size < 24 || at + size > b.len() {
            at = b.len();
            return Some(err(format!("object of size {size} at {at} runs out")));
        }
        let body = &b[at + 24..at + size];
        at += size;
        Some(Ok((id, body)))
    })
}

fn stream_properties(body: &[u8]) -> Result<Stream> {
    let mut r = Reader { b: body, at: 0 };
    let ty = r.guid()?;
    r.guid()?; // error correction type
    r.u64()?; // time offset
    let specific_len = r.u32()? as usize;
    r.u32()?; // error correction data length
    let flags = r.u16()?;
    r.u32()?;
    let specific = r.take(specific_len)?;
    let mut s = Reader { b: specific, at: 0 };
    let kind = if ty == AUDIO_MEDIA {
        let tag = s.u16()?;
        let channels = s.u16()?;
        let rate = s.u32()?;
        let bytes_per_second = s.u32()?;
        let block_align = s.u16()?;
        let bits = s.u16()?;
        let n = if s.at < specific.len() { s.u16()? as usize } else { 0 };
        let extra = s.take(n.min(specific.len() - s.at))?.to_vec();
        Kind::Audio { tag, channels, rate, bytes_per_second, block_align, bits, extra }
    } else if ty == VIDEO_MEDIA {
        let width = s.u32()?;
        let height = s.u32()?;
        s.u8()?;
        s.u16()?; // format data size
        let header_size = s.u32()? as usize;
        s.u32()?;
        s.u32()?;
        s.u16()?;
        s.u16()?;
        let fourcc: [u8; 4] = s.take(4)?.try_into().unwrap();
        s.take(20)?;
        let extra = s.take(header_size.saturating_sub(40).min(specific.len() - s.at))?.to_vec();
        Kind::Video { width, height, fourcc, extra }
    } else {
        Kind::Other
    };
    Ok(Stream { number: (flags & 0x7f) as u8, kind })
}

impl<'a> Asf<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let mut r = Reader { b: bytes, at: 0 };
        if r.guid()? != HEADER {
            return err("not an ASF file");
        }
        let header_size = r.u64()? as usize;
        r.u32()?;
        r.take(2)?;
        let header = bytes.get(30..header_size).ok_or_else(|| Error("header runs out".into()))?;
        let (mut streams, mut props) = (Vec::new(), None);
        for o in objects_in(header) {
            let (id, body) = o?;
            if id == FILE_PROPERTIES {
                let mut f = Reader { b: body, at: 0 };
                f.take(16 + 8 + 8)?;
                let packets = f.u64()?;
                let play = f.u64()?;
                f.u64()?;
                let preroll = f.u64()?;
                f.u32()?;
                let min = f.u32()?;
                let max = f.u32()?;
                if min != max {
                    return err(format!("packet sizes vary ({min} to {max})"));
                }
                props = Some((packets, play, preroll, min as usize));
            } else if id == STREAM_PROPERTIES {
                streams.push(stream_properties(body)?);
            } else if id == HEADER_EXTENSION && body.len() >= 22 {
                // Streams past the first ones may hide their stream properties at the end of an
                // extended stream properties object.
                for e in objects_in(&body[22..]) {
                    let (id, body) = e?;
                    if id != EXTENDED_STREAM_PROPERTIES {
                        continue;
                    }
                    let mut x = Reader { b: body, at: 0 };
                    x.take(8 * 2 + 4 * 6 + 4 + 2 + 2 + 8)?;
                    let names = x.u16()?;
                    let systems = x.u16()?;
                    for _ in 0..names {
                        x.u16()?;
                        let n = x.u16()? as usize;
                        x.take(n)?;
                    }
                    for _ in 0..systems {
                        x.guid()?;
                        x.u16()?;
                        let n = x.u32()? as usize;
                        x.take(n)?;
                    }
                    if let Some(Ok((id, body))) = objects_in(&body[x.at..]).next()
                        && id == STREAM_PROPERTIES {
                            streams.push(stream_properties(body)?);
                        }
                }
            }
        }
        let (packet_count, play_duration, preroll_ms, packet_size) = props.ok_or_else(|| Error("no file properties".into()))?;
        let mut d = Reader { b: bytes, at: header_size };
        if d.guid()? != DATA {
            return err("no data object after the header");
        }
        d.u64()?;
        d.guid()?;
        d.u64()?;
        d.u16()?;
        Ok(Asf { bytes, streams, packet_size, packet_count, preroll_ms, play_duration, data_start: d.at })
    }

    pub fn stream(&self, number: u8) -> Option<&Stream> {
        self.streams.iter().find(|s| s.number == number)
    }

    /// Every media object, in the order its last piece comes in the file.
    pub fn objects(&self) -> Result<Vec<Object>> {
        let mut out = Vec::new();
        let mut partial: Vec<Option<(u32, u32, Object)>> = vec![None; 128];
        for k in 0..self.packet_count as usize {
            let start = self.data_start + k * self.packet_size;
            let Some(packet) = self.bytes.get(start..start + self.packet_size) else { break };
            self.packet(packet, &mut partial, &mut out).map_err(|e| Error(format!("packet {k}: {}", e.0)))?;
        }
        Ok(out)
    }

    fn packet(&self, packet: &[u8], partial: &mut [Option<(u32, u32, Object)>], out: &mut Vec<Object>) -> Result<()> {
        let mut r = Reader { b: packet, at: 0 };
        let mut flags = r.u8()?;
        if flags & 0x80 != 0 {
            // Error correction data first, then the length type flags.
            if flags & 0x60 != 0 {
                return err("error correction length type not 0");
            }
            r.take((flags & 0x0f) as usize)?;
            flags = r.u8()?;
        }
        let property = r.u8()?;
        let multiple = flags & 1 != 0;
        let packet_length = r.sized(flags >> 5)? as usize;
        r.sized(flags >> 1)?; // sequence
        let mut padding = r.sized(flags >> 3)? as usize;
        if packet_length != 0 && packet_length < self.packet_size {
            padding += self.packet_size - packet_length;
        }
        r.u32()?; // send time
        r.u16()?; // duration
        let end = self.packet_size.checked_sub(padding).filter(|&e| e >= r.at).ok_or_else(|| Error("padding past the payloads".into()))?;
        let (count, length_type) = if multiple {
            let p = r.u8()?;
            ((p & 0x3f) as usize, p >> 6)
        } else {
            (1, 0)
        };
        for _ in 0..count {
            let number = r.u8()?;
            let (stream, key) = (number & 0x7f, number & 0x80 != 0);
            let object = r.sized(property >> 4)?;
            let offset = r.sized(property >> 2)?;
            let replicated = r.sized(property)? as usize;
            if replicated == 1 {
                // Compressed payloads: `offset` is the presentation time, a time delta, then
                // whole small objects, each after its length byte.
                let delta = r.u8()? as i64;
                let len = if multiple { r.sized(length_type)? as usize } else { end - r.at };
                let body = r.take(len)?;
                let (mut at, mut time) = (0, offset as i64);
                while at < body.len() {
                    let n = body[at] as usize;
                    let data = body.get(at + 1..at + 1 + n).ok_or_else(|| Error("compressed payload runs out".into()))?.to_vec();
                    out.push(Object { stream, time_ms: time - self.preroll_ms as i64, key, data });
                    at += 1 + n;
                    time += delta;
                }
                continue;
            }
            let rep = r.take(replicated)?;
            let (size, time) = if replicated >= 8 {
                (u32::from_le_bytes(rep[0..4].try_into().unwrap()), u32::from_le_bytes(rep[4..8].try_into().unwrap()))
            } else {
                (0, 0)
            };
            let len = if multiple { r.sized(length_type)? as usize } else { end.checked_sub(r.at).ok_or_else(|| Error("payload past the end".into()))? };
            let data = r.take(len)?;
            let slot = &mut partial[stream as usize];
            if offset == 0 {
                *slot = Some((object, size, Object { stream, time_ms: time as i64 - self.preroll_ms as i64, key, data: Vec::with_capacity(size as usize) }));
            }
            // A piece of an object whose start was not seen (or of another object) is dropped.
            let Some((n, size, o)) = slot.as_mut().filter(|(n, _, o)| *n == object && o.data.len() == offset as usize) else {
                *slot = None;
                continue;
            };
            o.data.extend_from_slice(data);
            if o.data.len() >= *size as usize {
                let _ = n;
                out.push(slot.take().unwrap().2);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny file made by hand: one video stream, one packet holding a whole 5-byte object.
    #[test]
    fn a_single_payload_packet_gives_its_object() {
        let mut f = Vec::new();
        let mut header_body = Vec::new();
        let object = |id: Guid, body: &[u8]| {
            let mut o = id.to_vec();
            o.extend_from_slice(&(24 + body.len() as u64).to_le_bytes());
            o.extend_from_slice(body);
            o
        };
        let mut props = vec![0u8; 16 + 8 + 8];
        props.extend_from_slice(&1u64.to_le_bytes());
        props.extend_from_slice(&0u64.to_le_bytes());
        props.extend_from_slice(&0u64.to_le_bytes());
        props.extend_from_slice(&3000u64.to_le_bytes());
        props.extend_from_slice(&2u32.to_le_bytes());
        props.extend_from_slice(&32u32.to_le_bytes());
        props.extend_from_slice(&32u32.to_le_bytes());
        props.extend_from_slice(&0u32.to_le_bytes());
        header_body.extend(object(FILE_PROPERTIES, &props));
        let mut sp = VIDEO_MEDIA.to_vec();
        sp.extend_from_slice(&[0; 16]);
        sp.extend_from_slice(&0u64.to_le_bytes());
        let mut specific = Vec::new();
        specific.extend_from_slice(&64u32.to_le_bytes());
        specific.extend_from_slice(&48u32.to_le_bytes());
        specific.push(2);
        specific.extend_from_slice(&40u16.to_le_bytes());
        specific.extend_from_slice(&40u32.to_le_bytes());
        specific.extend_from_slice(&[0; 8 + 4]);
        specific.extend_from_slice(b"WVC1");
        specific.extend_from_slice(&[0; 20]);
        sp.extend_from_slice(&(specific.len() as u32).to_le_bytes());
        sp.extend_from_slice(&0u32.to_le_bytes());
        sp.extend_from_slice(&2u16.to_le_bytes());
        sp.extend_from_slice(&0u32.to_le_bytes());
        sp.extend_from_slice(&specific);
        header_body.extend(object(STREAM_PROPERTIES, &sp));
        f.extend_from_slice(&HEADER);
        f.extend_from_slice(&(30 + header_body.len() as u64).to_le_bytes());
        f.extend_from_slice(&2u32.to_le_bytes());
        f.extend_from_slice(&[1, 2]);
        f.extend(header_body);
        f.extend_from_slice(&DATA);
        f.extend_from_slice(&(50 + 32u64).to_le_bytes());
        f.extend_from_slice(&[0; 16]);
        f.extend_from_slice(&1u64.to_le_bytes());
        f.extend_from_slice(&[1, 1]);
        // The packet: no error correction, single payload, padding a byte; property flags:
        // replicated a byte, offset a dword, object number a byte, stream a byte.
        let mut p = vec![0b0000_1000, 0b0101_1101, 0];
        p.extend_from_slice(&0u32.to_le_bytes());
        p.extend_from_slice(&0u16.to_le_bytes());
        p.push(0x82); // stream 2, key
        p.push(7);
        p.extend_from_slice(&0u32.to_le_bytes());
        p.push(8);
        p.extend_from_slice(&5u32.to_le_bytes());
        p.extend_from_slice(&3040u32.to_le_bytes());
        p.extend_from_slice(b"frame");
        let pad = 32 - p.len();
        p[2] = pad as u8;
        p.resize(32, 0);
        f.extend(p);
        let asf = Asf::parse(&f).unwrap();
        assert_eq!(asf.streams.len(), 1);
        assert!(matches!(&asf.streams[0].kind, Kind::Video { width: 64, height: 48, fourcc, .. } if fourcc == b"WVC1"));
        let objects = asf.objects().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!((objects[0].stream, objects[0].time_ms, objects[0].key, &objects[0].data[..]), (2, 40, true, &b"frame"[..]));
    }

    /// The real pre-title movie against ffmpeg's packet counts (282 audio, 2,883 video objects;
    /// the first video object 9,199 bytes and a key frame), when the install is there.
    #[test]
    #[ignore]
    fn avantitle_demuxes_as_ffmpeg_reads_it() {
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let bytes = std::fs::read(path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let objects = asf.objects().unwrap();
        let count = |s| objects.iter().filter(|o| o.stream == s).count();
        let (audio, video) = (asf.streams.iter().find(|s| matches!(s.kind, Kind::Audio { .. })).unwrap().number, asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).unwrap().number);
        assert_eq!((count(audio), count(video)), (282, 2883));
        let first = objects.iter().find(|o| o.stream == video).unwrap();
        assert_eq!((first.data.len(), first.key), (9199, true));
    }
}
