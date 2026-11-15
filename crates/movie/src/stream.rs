//! A VC-1 stream's pictures in display order: anchors (I, P, skipped) held back one picture,
//! B and BI pictures shown as they decode between the two anchors around them, each with the
//! loop filter when the entry point turns it on. ASF stamps the media objects in coding order
//! (one frame apart, a B picture after the anchor it precedes on screen), so the n-th picture
//! shown takes the n-th stamp.

use crate::bitplane::Tables as BitplaneTables;
use crate::bits::unescape;
use crate::vc1::{headers, EntryPoint, Error, InterTables, PictureType, Sequence};
use crate::vc1_inter::{decode, decode_b, remapped, PTables};
use crate::vc1_intra::{IntraTables, Picture};
use crate::vc1_loop::{intra_plane, p_plane};

/// A decoded picture and its presentation time.
pub struct Shown {
    pub picture: Picture,
    pub time_ms: i64,
    pub kind: PictureType,
}

struct Anchor {
    picture: Picture,
    colocated: Vec<[i32; 2]>,
    intensity: Option<(u8, u8)>,
    time_ms: i64,
    kind: PictureType,
}

pub struct VideoStream {
    pub sequence: Sequence,
    pub entry: EntryPoint,
    cols: usize,
    rows: usize,
    /// The displayed size (the coded pictures are whole macroblocks).
    pub width: usize,
    pub height: usize,
    intra: IntraTables,
    inter: PTables,
    planes: BitplaneTables,
    headers: InterTables,
    older: Option<Picture>,
    newer: Option<Anchor>,
    stamps: std::collections::VecDeque<i64>,
}

fn filter_intra(p: &mut Picture, q: i32) {
    let (w, h) = (p.width, p.height);
    intra_plane(&mut p.y, w, h, q);
    intra_plane(&mut p.u, w / 2, h / 2, q);
    intra_plane(&mut p.v, w / 2, h / 2, q);
}

impl VideoStream {
    /// From the ASF stream's codec data (sequence and entry-point headers) and picture size.
    pub fn new(codec_data: &[u8], width: usize, height: usize) -> Result<Self, Error> {
        let (sequence, entry) = headers(codec_data)?;
        let (cols, rows) = sequence.macroblocks(&entry);
        Ok(VideoStream { sequence, entry, cols, rows, width, height, intra: IntraTables::default(), inter: PTables::default(), planes: BitplaneTables::default(), headers: InterTables::default(), older: None, newer: None, stamps: Default::default() })
    }

    /// Decodes one media object; returns the pictures it lets out, in display order.
    pub fn push(&mut self, data: &[u8], time_ms: i64) -> Result<Vec<Shown>, Error> {
        let (s, e) = (&self.sequence, &self.entry);
        let kind = s.picture_start(data)?.kind;
        let data = unescape(data);
        let (cols, rows, coded) = (self.cols, self.rows, (self.width, self.height));
        let filter = e.loop_filter;
        self.stamps.push_back(time_ms);
        let mut out = Vec::new();
        match kind {
            PictureType::I | PictureType::P | PictureType::Skipped => {
                let (picture, colocated, intensity) = match kind {
                    PictureType::I => {
                        let h = s.intra_header(e, &data, &self.planes)?;
                        let mut p = Picture::new(cols, rows);
                        crate::vc1_intra::decode(&self.intra, &h, &data, cols, rows, &mut p)?;
                        if filter {
                            filter_intra(&mut p, h.pquant as i32);
                        }
                        (p, vec![[0; 2]; cols * rows], None)
                    }
                    PictureType::P => {
                        let reference = &self.newer.as_ref().ok_or_else(|| Error("a P picture before any anchor".into()))?.picture;
                        let h = s.inter_header(e, &data, &self.planes, &self.headers)?;
                        if std::env::var_os("BARREL_VC1_HEADERS").is_some() {
                            eprintln!("P at {time_ms}: pq {} idx {} half {} uniform {} round {} range {} {:?} ic {:?} tt {:?}/{} ac {} dc {}", h.pquant, h.pqindex, h.half_qp, h.uniform, h.round_control, h.mv_range, h.mv_mode, h.intensity, h.frame_transform, h.mb_transform, h.ac_set, h.dc_high);
                        }
                        let mut p = Picture::new(cols, rows);
                        let r = decode(&self.intra, &self.inter, &h, &data, cols, rows, coded, e.fast_uvmc, e.overlap, reference, &mut p)?;
                        if filter {
                            let (q, w, hh) = (h.pquant as i32, p.width, p.height);
                            p_plane(&mut p.y, w, hh, q, &r.cells[0]);
                            p_plane(&mut p.u, w / 2, hh / 2, q, &r.cells[1]);
                            p_plane(&mut p.v, w / 2, hh / 2, q, &r.cells[2]);
                        }
                        (p, r.colocated, h.intensity)
                    }
                    _ => {
                        let reference = &self.newer.as_ref().ok_or_else(|| Error("a skipped picture before any anchor".into()))?.picture;
                        (reference.clone(), vec![[0; 2]; cols * rows], None)
                    }
                };
                if let Some(prev) = self.newer.take() {
                    out.push(Shown { picture: prev.picture.clone(), time_ms: 0, kind: prev.kind });
                    self.older = Some(prev.picture);
                }
                self.newer = Some(Anchor { picture, colocated, intensity, time_ms, kind });
            }
            PictureType::B | PictureType::BI => {
                let mut p = Picture::new(cols, rows);
                if kind == PictureType::BI {
                    let h = s.intra_header(e, &data, &self.planes)?;
                    crate::vc1_intra::decode(&self.intra, &h, &data, cols, rows, &mut p)?;
                    if filter {
                        filter_intra(&mut p, h.pquant as i32);
                    }
                } else {
                    let h = s.inter_header(e, &data, &self.planes, &self.headers)?;
                    let next = self.newer.as_ref().ok_or_else(|| Error("a B picture before two anchors".into()))?;
                    let prev = self.older.as_ref().ok_or_else(|| Error("a B picture before two anchors".into()))?;
                    let remap;
                    let forward = match next.intensity {
                        Some((scale, shift)) => {
                            remap = remapped(prev, scale, shift);
                            &remap
                        }
                        None => prev,
                    };
                    decode_b(&self.intra, &self.inter, &h, &data, cols, rows, coded, e.fast_uvmc, forward, &next.picture, &next.colocated, &mut p)?;
                    if filter {
                        filter_intra(&mut p, h.pquant as i32);
                    }
                }
                out.push(Shown { picture: p, time_ms: 0, kind });
            }
        }
        for shown in &mut out {
            shown.time_ms = self.stamps.pop_front().unwrap_or(time_ms);
        }
        Ok(out)
    }

    /// The last anchor, once the stream has ended.
    pub fn finish(&mut self) -> Option<Shown> {
        let time_ms = self.stamps.pop_front();
        self.newer.take().map(|a| Shown { picture: a.picture, time_ms: time_ms.unwrap_or(a.time_ms), kind: a.kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_formats::asf::{Asf, Kind};

    /// Prints the first pictures' kinds and times, in coding and display order.
    #[test]
    #[ignore]
    fn display_times() {
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let bytes = std::fs::read(path).unwrap();
        let asf = Asf::parse(&bytes).unwrap();
        let video = asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).unwrap();
        let Kind::Video { extra, width, height, .. } = &video.kind else { unreachable!() };
        let mut v = VideoStream::new(extra, *width as usize, *height as usize).unwrap();
        let (mut coding, mut shown) = (Vec::new(), Vec::new());
        for o in asf.objects().unwrap().iter().filter(|o| o.stream == video.number).take(16) {
            coding.push((format!("{:?}", v.sequence.picture_start(&o.data).unwrap().kind), o.time_ms));
            shown.extend(v.push(&o.data, o.time_ms).unwrap().into_iter().map(|s| (format!("{:?}", s.kind), s.time_ms)));
        }
        eprintln!("coding {coding:?}\nshown {shown:?}");
    }
}
