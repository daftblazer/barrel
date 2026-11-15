//! A movie to play: the sound decoded at once (WMA Pro, `barrel_wma`), the pictures decoded on
//! a thread a few ahead, and the picture to show picked by the caller's clock (the sound's
//! position), late ones dropped.

use std::sync::mpsc::{sync_channel, Receiver, TrySendError};
use std::sync::Arc;

use barrel_formats::asf::{Asf, Kind};

use crate::stream::VideoStream;

/// The sound track: one sample list per channel, −1 to 1.
pub struct Audio {
    pub rate: u32,
    pub channels: Vec<Vec<f32>>,
}

/// A picture to show: I420 planes at the movie's size, and when.
pub struct Frame {
    pub time: f64,
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

pub struct Movie {
    pub width: usize,
    pub height: usize,
    pub audio: Option<Audio>,
    /// The last picture's time plus a frame, in seconds.
    pub duration: f64,
    frames: Receiver<Result<Frame, String>>,
    next: Option<Frame>,
    current: Option<Frame>,
    done: bool,
    /// Pictures dropped for being late.
    pub dropped: usize,
    pub error: Option<String>,
}

/// How many decoded pictures may wait (each 3 MB at 1080p).
const AHEAD: usize = 6;

impl Movie {
    /// Opens a `.wmv` file's bytes: decodes its sound, starts its pictures decoding.
    pub fn open(bytes: Vec<u8>) -> Result<Movie, String> {
        let bytes = Arc::new(bytes);
        let asf = Asf::parse(&bytes).map_err(|e| format!("{e:?}"))?;
        let video = asf.streams.iter().find(|s| matches!(s.kind, Kind::Video { .. })).ok_or("no video stream")?;
        let Kind::Video { width, height, extra, .. } = &video.kind else { unreachable!() };
        let (width, height, codec) = (*width as usize, *height as usize, extra.clone());
        let objects = asf.objects().map_err(|e| format!("{e:?}"))?;
        let number = video.number;
        let duration = objects.iter().filter(|o| o.stream == number).map(|o| o.time_ms).max().unwrap_or(0) as f64 / 1000.0 + 1001.0 / 30000.0;
        let mut audio = None;
        if let Some(a) = asf.streams.iter().find(|s| matches!(s.kind, Kind::Audio { .. })) {
            let Kind::Audio { tag, rate, block_align, channels, extra, .. } = &a.kind else { unreachable!() };
            if *tag == 0x162 {
                let params = barrel_wma::Params::new(extra, *rate, *block_align as usize, *channels as usize).map_err(|e| e.0)?;
                let mut d = barrel_wma::Decoder::new(params);
                let mut pcm: Vec<Vec<f32>> = vec![Vec::new(); d.params.channels];
                for o in objects.iter().filter(|o| o.stream == a.number) {
                    for (c, v) in d.push(&o.data).map_err(|e| e.0)?.into_iter().enumerate() {
                        pcm[c].extend(v);
                    }
                }
                audio = Some(Audio { rate: *rate, channels: pcm });
            }
        }
        let (tx, rx) = sync_channel(AHEAD);
        let data = Arc::clone(&bytes);
        std::thread::Builder::new()
            .name("movie video".into())
            .spawn(move || {
                let crop = |s: crate::stream::Shown| {
                    let p = s.picture;
                    let (cw, ch) = (width / 2, height / 2);
                    let mut f = Frame { time: s.time_ms as f64 / 1000.0, width, height, y: Vec::with_capacity(width * height), u: Vec::with_capacity(cw * ch), v: Vec::with_capacity(cw * ch) };
                    for y in 0..height {
                        f.y.extend_from_slice(&p.y[y * p.width..y * p.width + width]);
                    }
                    for y in 0..ch {
                        f.u.extend_from_slice(&p.u[y * p.width / 2..y * p.width / 2 + cw]);
                        f.v.extend_from_slice(&p.v[y * p.width / 2..y * p.width / 2 + cw]);
                    }
                    f
                };
                let run = || -> Result<(), String> {
                    let asf = Asf::parse(&data).map_err(|e| format!("{e:?}"))?;
                    let mut stream = VideoStream::new(&codec, width, height).map_err(|e| e.0)?;
                    for o in asf.objects().map_err(|e| format!("{e:?}"))?.iter().filter(|o| o.stream == number) {
                        for s in stream.push(&o.data, o.time_ms).map_err(|e| e.0)? {
                            if tx.send(Ok(crop(s))).is_err() {
                                return Ok(());
                            }
                        }
                    }
                    if let Some(s) = stream.finish() {
                        let _ = tx.send(Ok(crop(s)));
                    }
                    Ok(())
                };
                if let Err(e) = run() {
                    let _ = tx.try_send(Err(e)).or_else(|e| match e {
                        TrySendError::Full(m) | TrySendError::Disconnected(m) => tx.send(m).map_err(|_| ()),
                    });
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Movie { width, height, audio, duration, frames: rx, next: None, current: None, done: false, dropped: 0, error: None })
    }

    fn pull(&mut self, wait: bool) -> Option<Frame> {
        let got = if wait { self.frames.recv().ok() } else { self.frames.try_recv().ok() };
        match got {
            Some(Ok(f)) => Some(f),
            Some(Err(e)) => {
                self.error = Some(e);
                self.done = true;
                None
            }
            None => {
                // Disconnected: the decoder finished (or nothing ready yet when not waiting).
                if wait {
                    self.done = true;
                }
                None
            }
        }
    }

    /// The picture to show at `clock` seconds: the newest one due by then (pictures due
    /// earlier and never shown are dropped, counted in `dropped`). `wait` blocks for pictures
    /// not decoded yet (an offscreen render); otherwise the last one stays until the next is
    /// ready.
    pub fn frame_at(&mut self, clock: f64, wait: bool) -> Option<&Frame> {
        let mut replaced = 0;
        loop {
            if self.next.is_none() && !self.done {
                self.next = self.pull(wait);
            }
            if !self.next.as_ref().is_some_and(|f| f.time <= clock) {
                break;
            }
            self.current = self.next.take();
            replaced += 1;
        }
        self.dropped += replaced.max(1) - 1;
        self.current.as_ref()
    }

    /// Whether the movie has played out by `clock` seconds.
    pub fn ended(&self, clock: f64) -> bool {
        clock >= self.duration || (self.done && self.next.is_none() && self.error.is_some())
    }
}
