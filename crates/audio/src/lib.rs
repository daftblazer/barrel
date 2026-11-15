//! Sound for the engine: [`Mixer`] adds up the playing sounds into one stereo stream, and
//! [`Output`] hands that stream to the system through SDL3 (an audio device stream the engine
//! keeps topped up from its frame loop), or [`Capture`] keeps it for a file. The mixer is plain
//! code, testable without a device.

use std::sync::Arc;

pub mod capture;
pub mod output;
pub use capture::Capture;
pub use output::Output;

/// The rate everything is mixed at (the game's clips are all 48 kHz).
pub const RATE: u32 = 48_000;

/// A decoded sound: one sample list per channel (1 or 2) at `rate`.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl Sound {
    /// A sound from an Ogg Vorbis stream (decoded natively, `barrel_formats::vorbis`).
    pub fn from_ogg(ogg: &[u8]) -> Result<Sound, barrel_formats::Error> {
        let (id, channels) = barrel_formats::vorbis::decode(ogg)?;
        Ok(Sound { rate: id.rate, channels })
    }

    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    /// The (left, right) sample at frame `i` (mono plays on both sides).
    fn frame(&self, i: usize) -> (f32, f32) {
        match self.channels.as_slice() {
            [m] => (m[i], m[i]),
            [l, r, ..] => (l[i], r[i]),
            [] => (0.0, 0.0),
        }
    }
}

/// A long sound decoded as it plays (music): the frames decoded and not yet played, as
/// (left, right), and where it goes back to on reaching its end.
struct Streamed {
    stream: barrel_formats::vorbis::Stream,
    ahead: std::collections::VecDeque<(f32, f32)>,
    loop_from: Option<u64>,
    ended: bool,
}

impl Streamed {
    /// Decodes until `n` frames are ahead, or the stream (when it does not loop) ends.
    fn fill(&mut self, n: usize) {
        while self.ahead.len() < n && !self.ended {
            match self.stream.read() {
                Some(chunk) => self.push(&chunk, 0),
                None => match self.loop_from {
                    // Seamless: the loop's first frames follow the last ones directly.
                    Some(from) => {
                        let skip = self.stream.seek(from) as usize;
                        match self.stream.read() {
                            Some(chunk) => self.push(&chunk, skip),
                            None => self.ended = true,
                        }
                    }
                    None => self.ended = true,
                },
            }
        }
    }

    fn push(&mut self, chunk: &[Vec<f32>], skip: usize) {
        match chunk {
            [m] => self.ahead.extend(m.iter().skip(skip).map(|&x| (x, x))),
            [l, r, ..] => self.ahead.extend(l.iter().zip(r).skip(skip).map(|(&a, &b)| (a, b))),
            [] => {}
        }
    }
}

enum Source {
    Clip(Arc<Sound>),
    Stream(Box<Streamed>),
}

/// A sound being played.
struct Voice {
    id: u64,
    source: Source,
    rate: u32,
    /// The position in the sound's frames (fractional when its rate differs from [`RATE`]); for
    /// a stream, from the first frame still held.
    at: f64,
    gain: f32,
    /// Where it starts again on reaching its end (in its frames), when it loops.
    loop_from: Option<usize>,
    /// While fading out: the gain taken off per output frame (it stops at 0).
    fading: Option<f32>,
    /// The left and right gains from its pan (1 each in the centre).
    sides: (f32, f32),
    kind: Kind,
}

/// What a sound is, for the player's volume of it (the Options screen's BGM, SE and Voice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Music,
    Effect,
    Voice,
}

/// What a [`Mixer::play`] returns, to stop or change the sound later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Playing(u64);

/// The sounds playing now.
#[derive(Default)]
pub struct Mixer {
    voices: Vec<Voice>,
    next: u64,
    /// The overall volume (0 to 1).
    pub master: f32,
    /// The player's volume of each [`Kind`] of sound (0 to 1; music, effects, voices).
    pub kinds: [f32; 3],
}

/// The game's overall level at its default options, measured against the reference recording
/// of the opening: voices and music both come out at 0.8 of their own level there (voices
/// besides at [`VOICE_GAIN`]). Not yet read from the game's option table.
pub const OPTION_VOLUME: f32 = 0.8;

/// A voice line's gain: the game puts a mono voice clip in the middle at about 1/√2 a side
/// (constant power), against the same clip fitted into the reference recording (0.57 of it
/// with [`OPTION_VOLUME`], over two clips).
pub const VOICE_GAIN: f32 = std::f32::consts::FRAC_1_SQRT_2;

impl Mixer {
    pub fn new() -> Mixer {
        Mixer { voices: Vec::new(), next: 0, master: 1.0, kinds: [1.0; 3] }
    }

    /// A mixer at the game's default level ([`OPTION_VOLUME`]).
    pub fn game() -> Mixer {
        Mixer { master: OPTION_VOLUME, ..Mixer::new() }
    }

    /// Starts a sound at `gain`: once, or (with `loop_from`) going back to that frame each
    /// time it ends.
    pub fn play(&mut self, sound: Arc<Sound>, gain: f32, loop_from: Option<usize>) -> Playing {
        self.next += 1;
        let loop_from = loop_from.filter(|&f| f < sound.frames());
        let rate = sound.rate;
        self.voices.push(Voice { id: self.next, source: Source::Clip(sound), rate, at: 0.0, gain, loop_from, fading: None, sides: (1.0, 1.0), kind: Kind::Effect });
        Playing(self.next)
    }

    /// Starts a stream at `gain`, decoding it as it plays: once, or going back to frame
    /// `loop_from` each time it ends.
    pub fn play_stream(&mut self, stream: barrel_formats::vorbis::Stream, gain: f32, loop_from: Option<u64>) -> Playing {
        self.next += 1;
        let loop_from = loop_from.filter(|&f| f < stream.frames());
        let rate = stream.id().rate;
        let s = Streamed { stream, ahead: Default::default(), loop_from, ended: false };
        self.voices.push(Voice { id: self.next, source: Source::Stream(Box::new(s)), rate, at: 0.0, gain, loop_from: None, fading: None, sides: (1.0, 1.0), kind: Kind::Music });
        Playing(self.next)
    }

    /// Files a sound under `kind` for the player's volumes (a clip starts as an effect, a stream
    /// as music).
    pub fn set_kind(&mut self, p: Playing, kind: Kind) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == p.0) {
            v.kind = kind;
        }
    }

    pub fn stop(&mut self, p: Playing) {
        self.voices.retain(|v| v.id != p.0);
    }

    pub fn set_gain(&mut self, p: Playing, gain: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == p.0) {
            v.gain = gain;
        }
    }

    /// Places a sound from -1 (left) to 1 (right): the far side is turned down linearly and
    /// the near side kept (the game's sound library's own pan law is not read).
    pub fn set_pan(&mut self, p: Playing, pan: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == p.0) {
            let pan = pan.clamp(-1.0, 1.0);
            v.sides = ((1.0 - pan).min(1.0), (1.0 + pan).min(1.0));
        }
    }

    /// Fades a sound out over `seconds` (at once for 0), then stops it.
    pub fn fade_out(&mut self, p: Playing, seconds: f32) {
        if seconds <= 0.0 {
            self.stop(p);
        } else if let Some(v) = self.voices.iter_mut().find(|v| v.id == p.0) {
            v.fading = Some(v.gain / (seconds * RATE as f32));
        }
    }

    pub fn is_playing(&self, p: Playing) -> bool {
        self.voices.iter().any(|v| v.id == p.0)
    }

    /// Mixes the next `out.len() / 2` stereo frames into `out` (interleaved left, right),
    /// moving every sound on; sounds that end are dropped, looping ones start again. A sound at
    /// another rate is stepped through at its rate (linear interpolation).
    pub fn mix(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        for v in &mut self.voices {
            let step = f64::from(v.rate) / f64::from(RATE);
            let player = self.kinds[v.kind as usize];
            let level = Level { gain: &mut v.gain, fading: v.fading, sides: (v.sides.0 * player, v.sides.1 * player) };
            let out = &mut out[..2 * frames];
            match &mut v.source {
                Source::Clip(sound) => mix_clip(sound, &mut v.at, v.loop_from, step, level, out),
                Source::Stream(s) => mix_stream(s, &mut v.at, step, level, out),
            }
        }
        self.voices.retain(|v| {
            let faded = v.fading.is_some() && v.gain <= 0.0;
            let more = match &v.source {
                Source::Clip(s) => v.loop_from.is_some() || (v.at as usize) < s.frames(),
                Source::Stream(s) => !s.ended || (v.at as usize) < s.ahead.len(),
            };
            !faded && more
        });
        for s in out.iter_mut() {
            *s = (*s * self.master).clamp(-1.0, 1.0);
        }
    }
}

/// A voice's gain and fade.
struct Level<'a> {
    gain: &'a mut f32,
    fading: Option<f32>,
    sides: (f32, f32),
}

impl Level<'_> {
    /// Adds output frame `f` at the gain and moves the fade on; false once it has faded out.
    fn add(&mut self, out: &mut [f32], f: usize, (l, r): (f32, f32)) -> bool {
        out[2 * f] += l * *self.gain * self.sides.0;
        out[2 * f + 1] += r * *self.gain * self.sides.1;
        if let Some(d) = self.fading {
            *self.gain -= d;
            if *self.gain <= 0.0 {
                *self.gain = 0.0;
                return false;
            }
        }
        true
    }
}

fn lerp((l0, r0): (f32, f32), (l1, r1): (f32, f32), t: f32) -> (f32, f32) {
    (l0 + (l1 - l0) * t, r0 + (r1 - r0) * t)
}

fn mix_clip(sound: &Sound, at: &mut f64, loop_from: Option<usize>, step: f64, mut level: Level, out: &mut [f32]) {
    let len = sound.frames();
    if len == 0 {
        return;
    }
    for f in 0..out.len() / 2 {
        if *at as usize >= len {
            match loop_from {
                Some(from) => *at -= (len - from) as f64,
                None => break,
            }
        }
        let i = *at as usize;
        let t = (*at - i as f64) as f32;
        let a = sound.frame(i);
        let b = match (i + 1 < len, loop_from) {
            (true, _) => sound.frame(i + 1),
            (false, Some(from)) => sound.frame(from),
            (false, None) => a,
        };
        *at += step;
        if !level.add(out, f, lerp(a, b, t)) {
            break;
        }
    }
}

fn mix_stream(s: &mut Streamed, at: &mut f64, step: f64, mut level: Level, out: &mut [f32]) {
    let frames = out.len() / 2;
    // Enough decoded for this mix and the frame after, to interpolate towards.
    s.fill((*at + step * frames as f64) as usize + 2);
    for f in 0..frames {
        let i = *at as usize;
        let Some(&a) = s.ahead.get(i) else { break };
        let b = s.ahead.get(i + 1).copied().unwrap_or(a);
        let t = (*at - i as f64) as f32;
        *at += step;
        if !level.add(out, f, lerp(a, b, t)) {
            break;
        }
    }
    // Drop what has been played.
    let done = (*at as usize).min(s.ahead.len());
    s.ahead.drain(..done);
    *at -= done as f64;
}

/// A music track: its file under `Data/x64/Sound/BGM/` ("ev_usual01.kvs"), its volume and
/// its title.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub file: String,
    pub volume: f32,
    pub title: String,
}

/// The game's sounds from its install folder, decoded on first use and kept: voice clips by
/// number (`Data/x64/Sound/<voices>/SeriesNNNN.fpl` and `.fp`, a bank per thousand) and music
/// tracks by the executable's list (`Data/x64/Sound/BGM/<file>`, with its volume and loop).
pub struct Library {
    sound: std::path::PathBuf,
    voices: String,
    tracks: Vec<Track>,
    banks: std::collections::HashMap<u32, Option<(Vec<Option<(usize, usize)>>, Vec<u8>)>>,
    /// The sound effect bank (`Sound/SE/A15_bank_se`): each sound's first block and the data.
    effects: Option<Option<(Vec<Option<barrel_formats::kwb::Block>>, Vec<u8>)>>,
}

/// A sound effect ready to play.
pub enum Effect {
    /// One of the bank's sounds.
    Clip(Arc<Sound>),
    /// An ambience loop (`Sound/SE/env/`), with its loop start.
    Stream(barrel_formats::vorbis::Stream, Option<u64>),
}

/// The ambience loops for effects 128 to 151 (`0x140d5fb10`), in order.
pub const AMBIENCE: [&str; 24] = [
    "00ENV_CAVE01", "01ENV_FOREST01", "02ENV_FOREST02", "03ENV_FOREST03", "04ENV_LAKE01", "05ENV_RIVER01",
    "06ENV_RUIN01", "07ENV_STORM01", "08ENV_TOWN01", "09ENV_WATER01", "10ENV_WHEEL01", "11ENV_WIND01",
    "12ENV_WIND02", "13ENV_MOB01", "14ENV_MOB02", "15ENV_KOUJI01", "16ENV_SEA01", "17ENV_SEA02",
    "18ENV_SHORE01", "19ENV_LAVA01", "20ENV_LAVA02", "21ENV_MOB03", "22ENV_PLAIN01", "23ENV_RAIN01",
];

/// A music track ready to play.
pub struct Music {
    /// Decoded as it plays ([`Mixer::play_stream`]).
    pub stream: barrel_formats::vorbis::Stream,
    pub volume: f32,
    pub loop_from: Option<u64>,
}

impl Library {
    /// `install` is the game's folder; `voices` the voice folder under `Sound/` (`Voice` for
    /// Japanese, `Voice_EN` for English); `tracks` the game's music list (Escha's: `barrel_escha::data::bgm::tracks`).
    pub fn new(install: &std::path::Path, voices: &str, tracks: Vec<Track>) -> Library {
        Library { sound: install.join("Data/x64/Sound"), voices: voices.to_string(), tracks, banks: Default::default(), effects: None }
    }

    pub fn voice(&mut self, clip: u32) -> Option<Arc<Sound>> {
        let bank = clip / 1000 * 1000;
        let path = self.sound.join(&self.voices).join(format!("Series{bank:04}.fpl"));
        let entry = self.banks.entry(bank).or_insert_with(|| {
            let list = barrel_formats::voice::list(&barrel_files::read(&path).ok()?).ok()?;
            Some((list, barrel_files::read(path.with_extension("fp")).ok()?))
        });
        let (list, fp) = entry.as_ref()?;
        let (off, size) = list.get((clip - bank) as usize).copied().flatten()?;
        let ogg = barrel_formats::voice::ogg(fp.get(off..off + size)?).ok()?;
        Sound::from_ogg(&ogg).ok().map(Arc::new)
    }

    /// Effect `n` the way the game's player (`0x140160070`) takes it: 0 to 127 the bank's
    /// sounds by number, 128 to 151 the ambience loops, anything else nothing.
    pub fn effect(&mut self, n: u32) -> Option<Effect> {
        match n {
            0..=127 => {
                let sound = &self.sound;
                let bank = self.effects.get_or_insert_with(|| {
                    let wb2 = barrel_files::read(sound.join("SE/A15_bank_se.wb2")).ok()?;
                    let sounds = barrel_formats::kwb::sounds(&barrel_files::read(sound.join("SE/A15_bank_se.wh2")).ok()?, wb2.len()).ok()?;
                    Some((sounds.into_iter().map(|b| b.first().copied()).collect(), wb2))
                });
                let (blocks, wb2) = bank.as_ref()?;
                let block = (*blocks.get(n as usize)?)?;
                let channels = barrel_formats::kwb::pcm(wb2, &block).ok()?;
                Some(Effect::Clip(Arc::new(Sound { rate: block.rate, channels })))
            }
            128..=151 => {
                let kovs = barrel_files::read(self.sound.join("SE/env").join(format!("{}.kvs", AMBIENCE[(n - 128) as usize]))).ok()?;
                let looping = barrel_formats::voice::looping(&kovs).ok()?;
                let stream = barrel_formats::vorbis::Stream::open(&barrel_formats::voice::ogg(&kovs).ok()?).ok()?;
                Some(Effect::Stream(stream, (!looping.once).then_some(u64::from(looping.start))))
            }
            _ => None,
        }
    }

    pub fn music(&self, track: u32) -> Option<Music> {
        let t = self.tracks.get(track as usize)?;
        let kovs = barrel_files::read(self.sound.join("BGM").join(&t.file)).ok()?;
        let looping = barrel_formats::voice::looping(&kovs).ok()?;
        let stream = barrel_formats::vorbis::Stream::open(&barrel_formats::voice::ogg(&kovs).ok()?).ok()?;
        Some(Music { stream, volume: t.volume, loop_from: (!looping.once).then_some(u64::from(looping.start)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, v: f32) -> Arc<Sound> {
        Arc::new(Sound { rate: RATE, channels: vec![vec![v; n]] })
    }

    #[test]
    fn sounds_add_up_and_end() {
        let mut m = Mixer::new();
        let a = m.play(tone(3, 0.25), 1.0, None);
        m.play(tone(5, 0.5), 0.5, None);
        let mut out = [0f32; 8];
        m.mix(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.25, 0.25]);
        assert!(!m.is_playing(a));
        m.mix(&mut out);
        assert_eq!(&out[..4], &[0.25, 0.25, 0.0, 0.0]);
    }

    #[test]
    fn looping_and_rates() {
        let mut m = Mixer::new();
        let s = Arc::new(Sound { rate: RATE / 2, channels: vec![vec![0.0, 1.0], vec![0.0, -1.0]] });
        let p = m.play(s, 1.0, Some(0));
        let mut out = [0f32; 8];
        m.mix(&mut out);
        // At half the rate each frame lasts two output frames: 0, 0.5, 1, 0.5 (wrapping to 0).
        assert_eq!(out, [0.0, 0.0, 0.5, -0.5, 1.0, -1.0, 0.5, -0.5]);
        assert!(m.is_playing(p));
        m.stop(p);
        assert!(!m.is_playing(p));
        // Looping from frame 2 of 4: 0 1 2 3 2 3 2 3.
        let s = Arc::new(Sound { rate: RATE, channels: vec![vec![0.0, 1.0, 2.0, 3.0]] });
        m.play(s, 0.25, Some(2));
        let mut out = [0f32; 16];
        m.mix(&mut out);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert_eq!(left, [0.0, 0.25, 0.5, 0.75, 0.5, 0.75, 0.5, 0.75]);
    }

    /// A stream plays as the whole clip does, through its loop's wrap. Needs a looping
    /// KOVS file from the game: `BARREL_KVS=<file> cargo test -- --ignored streams_like_clips`.
    #[test]
    #[ignore]
    fn streams_like_clips() {
        let Ok(path) = std::env::var("BARREL_KVS") else { return };
        let kovs = barrel_files::read(path).unwrap();
        let ogg = barrel_formats::voice::ogg(&kovs).unwrap();
        let start = u64::from(barrel_formats::voice::looping(&kovs).unwrap().start);
        let clip = Arc::new(Sound::from_ogg(&ogg).unwrap());
        let len = clip.frames();
        let (mut a, mut b) = (Mixer::new(), Mixer::new());
        a.play(clip, 0.5, Some(start as usize));
        b.play_stream(barrel_formats::vorbis::Stream::open(&ogg).unwrap(), 0.5, Some(start));
        // Two passes over the end, in mixes of an odd size.
        let (mut x, mut y) = (vec![0f32; 2 * 997], vec![0f32; 2 * 997]);
        for _ in 0..(2 * len - start as usize) / 997 + 2 {
            a.mix(&mut x);
            b.mix(&mut y);
            assert_eq!(x, y);
        }
    }

    #[test]
    fn panning() {
        let mut m = Mixer::new();
        let p = m.play(tone(2, 1.0), 1.0, None);
        m.set_pan(p, -0.5);
        let mut out = [0f32; 4];
        m.mix(&mut out);
        assert_eq!(out, [1.0, 0.5, 1.0, 0.5]);
    }

    #[test]
    fn fading_out() {
        let mut m = Mixer::new();
        let p = m.play(tone(RATE as usize, 1.0), 1.0, Some(0));
        m.fade_out(p, 4.0 / RATE as f32);
        let mut out = [0f32; 12];
        m.mix(&mut out);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert_eq!(left, [1.0, 0.75, 0.5, 0.25, 0.0, 0.0]);
        assert!(!m.is_playing(p));
    }
}
