//! What Ryza plays: the game's sound requests (cues) with the frame each is made at, a log of
//! them (`BARREL_TRACE_CUES`), and playback on the audio device. A cue names a sound by its
//! hash, as the game asks for it: a stream id of the install's sound containers (music and
//! voice, `barrel_formats::ktsr`) or a row of `sfx.xml` (an effect, from an `SE_*` bank).

use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use barrel_audio::{Kind, Playing};
use barrel_formats::{ktsr, voice, vorbis};

use crate::music::Entry;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The folder of the sound containers under the install.
pub const SOUND_DIR: &str = "Data/x64/Sound";

/// The title's frames a second (the cues are timed in them).
pub const FPS: f32 = 60.0;

/// The title's music (`BGM00054_SY_02_TITLE01`) and its voice clip, by stream id.
pub const TITLE_MUSIC: u32 = 0xa5ba_0cbd;
pub const TITLE_VOICE: u32 = 0x8c7e_ffb2;
/// The menu's sound effects (`sfx.xml`: `SE_ETC_008_PRESS_START`, `SE_SYS_001_CLICK`,
/// `SE_SYS_002_CANCEL`, `SE_SYS_005_WINDOW`, `SE_SYS_006_CURSOR`, `SE_SYS_007_PAGE`).
pub const PRESS_START: u32 = 0xa699_d027;
pub const CLICK: u32 = 0x09c3_999b;
pub const CANCEL: u32 = 0x22bd_a2fa;
pub const WINDOW: u32 = 0x0d0f_cb87;
pub const CURSOR: u32 = 0x5e3d_ae22;
pub const PAGE: u32 = 0x5ce8_51ca;

/// Frames from the load's stop to the field's music (the answer key: the stop at game frame
/// 1725, the music at 1751).
pub const FIELD_MUSIC_AFTER_STOP: i64 = 26;

/// What a cue asks the sound manager for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    Music,
    Voice,
    Effect,
    /// Every channel stops (the load).
    Stop,
}

/// One request: when (a frame, the title's first frame 0 and counting on through the load into
/// the field), what, and the sound's hash (unused by a stop).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub at_frame: i64,
    pub what: What,
    pub hash: u32,
}

impl Cue {
    pub fn music(at_frame: i64, hash: u32) -> Cue {
        Cue { at_frame, what: What::Music, hash }
    }
    pub fn voice(at_frame: i64, hash: u32) -> Cue {
        Cue { at_frame, what: What::Voice, hash }
    }
    pub fn effect(at_frame: i64, hash: u32) -> Cue {
        Cue { at_frame, what: What::Effect, hash }
    }
    pub fn stop(at_frame: i64) -> Cue {
        Cue { at_frame, what: What::Stop, hash: 0 }
    }

    /// The cue's line in the cue log: `<frame> <kind> <hash, 8 hex digits>` (`-` for a stop).
    pub fn line(&self) -> String {
        match self.what {
            What::Music => format!("{} music {:08x}", self.at_frame, self.hash),
            What::Voice => format!("{} voice {:08x}", self.at_frame, self.hash),
            What::Effect => format!("{} effect {:08x}", self.at_frame, self.hash),
            What::Stop => format!("{} stop -", self.at_frame),
        }
    }
}

// STAND-IN: The title's music and voice start at the frames a run of the game started them (the music two frames into the title with saves, 18 without; the voice 96 frames after the music request, the 1.6 s delay in it, and 8 earlier without saves); the real thing is the game's own title code.
/// The title's cues: the music (it loops), then the voice clip (once). With saves (the game's
/// shorter boot) they start at frames 2 and 98, without at 18 and 106.
pub fn title_cues(with_saves: bool) -> [Cue; 2] {
    let (music, voice) = if with_saves { (2, 98) } else { (18, 106) };
    [Cue::music(music, TITLE_MUSIC), Cue::voice(voice, TITLE_VOICE)]
}

/// The title frame `seconds` into the title is on.
pub fn frame_at(seconds: f32) -> i64 {
    // a little over, so a time given as frame / 60 lands on that frame
    (seconds * FPS + 1e-3).floor() as i64
}

/// With `BARREL_TRACE_CUES=<file>` set, appends the cue's line to it.
pub fn trace(cue: &Cue) {
    let Some(path) = std::env::var_os("BARREL_TRACE_CUES") else { return };
    let written = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{}", cue.line()));
    if let Err(e) = written {
        eprintln!("cue log: {e}");
    }
}

/// The containers a kind of stream is looked for in, in order.
fn stream_files(what: What) -> &'static [&'static str] {
    match what {
        What::Voice => &["voice.ktsl2stbin"],
        _ => &["bgm.ktsl2stbin", "BGM_DX_add.ktsl2stbin", "BGM_DLC.ktsl2stbin"],
    }
}

/// A cue's stream, ready for the mixer, with the frame it loops from (music only; the clip's
/// own loop start, unless it is marked to play once).
pub struct Opened {
    pub stream: vorbis::Stream,
    pub loop_from: Option<u64>,
}

/// The install's sound containers, each indexed once when first needed.
struct Library {
    root: PathBuf,
    /// The stream ids of each container opened so far.
    streams: HashMap<&'static str, Vec<ktsr::Stream>>,
    /// The effects' tags and banks, by hash (`sfx.xml`).
    effects: HashMap<u32, Entry>,
    banks: HashMap<String, ktsr::Bank>,
}

impl Library {
    fn dir(&self) -> PathBuf {
        self.root.join(SOUND_DIR)
    }

    /// Opens the stream `hash` of the kind's containers.
    fn open(&mut self, what: What, hash: u32) -> Result<Opened> {
        for &name in stream_files(what) {
            let path = self.dir().join(name);
            if !self.streams.contains_key(name) {
                let list = match std::fs::File::open(&path) {
                    Ok(f) => ktsr::parse_streams(&mut BufReader::new(f))?,
                    Err(_) => Vec::new(),
                };
                self.streams.insert(name, list);
            }
            let Some(stream) = self.streams[name].iter().find(|s| s.id == hash).copied() else { continue };
            let mut file = BufReader::new(std::fs::File::open(&path)?);
            let kovs = ktsr::read_stream(&mut file, &stream)?;
            let looping = voice::looping(&kovs)?;
            let loop_from = (what == What::Music && !looping.once).then_some(u64::from(looping.start));
            return Ok(Opened { stream: vorbis::Stream::open(&voice::ogg(&kovs)?)?, loop_from });
        }
        Err(format!("stream {hash:08x}: not in the sound containers").into())
    }

    /// What the effect banks say of the effect `hash`: its tag, bank and sample.
    fn effect(&mut self, hash: u32) -> Result<String> {
        let entry = self.effects.get(&hash).ok_or_else(|| format!("effect {hash:08x}: not in sfx.xml"))?.clone();
        if !self.banks.contains_key(&entry.file) {
            let path = self.dir().join(format!("{}.ktsl2asbin", entry.file));
            self.banks.insert(entry.file.clone(), ktsr::parse_bank(&mut BufReader::new(std::fs::File::open(path)?))?);
        }
        let s = self.banks[&entry.file].sample(hash).ok_or_else(|| format!("effect {} ({hash:08x}): no sample in {}", entry.tag, entry.file))?;
        Ok(format!("{} ({}): {} Hz, {} channels, {} frames, codec {:#010x}", entry.tag, entry.file, s.rate, s.channels, s.frames, s.codec))
    }
}

/// The game's sound: cues scheduled for frames, each logged and started when its frame is
/// reached, on the audio device when there is one (a window; an offscreen run only logs).
pub struct Sound {
    queue: Vec<Cue>,
    /// Events already scheduled (`request`), by what they are and their frame.
    seen: HashSet<(&'static str, i64)>,
    library: Option<Library>,
    device: Option<(barrel_audio::Output, barrel_audio::Mixer)>,
    playing: Vec<Playing>,
    /// Effects already reported as unplayable, once each.
    reported: HashSet<u32>,
}

impl Sound {
    /// `root` the game's folder; `effects` the rows of `sfx.xml`; `with_device` for a window.
    pub fn new(root: &Path, effects: Vec<Entry>, with_device: bool) -> Sound {
        let device = with_device.then(|| barrel_audio::Output::open(0.1).map_err(|e| eprintln!("sound: none: {e}")).ok()).flatten().map(|o| (o, barrel_audio::Mixer::game()));
        let library = device.is_some().then(|| Library { root: root.to_path_buf(), streams: HashMap::new(), effects: effects.into_iter().map(|e| (e.hash, e)).collect(), banks: HashMap::new() });
        Sound { queue: Vec::new(), seen: HashSet::new(), library, device, playing: Vec::new(), reported: HashSet::new() }
    }

    /// Schedules a cue (kept in frame order, equal frames in the order given).
    pub fn schedule(&mut self, cue: Cue) {
        let at = self.queue.partition_point(|c| c.at_frame <= cue.at_frame);
        self.queue.insert(at, cue);
    }

    /// Schedules `cue` once for the event `what` at `frame` (the menus ask every frame).
    pub fn request(&mut self, what: &'static str, cue: Cue) {
        if self.seen.insert((what, cue.at_frame)) {
            self.schedule(cue);
        }
    }

    /// Called every frame with the frame reached: starts the cues now due (logging each), and
    /// keeps the device fed.
    pub fn tick(&mut self, frame: i64) {
        let due = self.queue.partition_point(|c| c.at_frame <= frame);
        let cues: Vec<Cue> = self.queue.drain(..due).collect();
        for cue in cues {
            trace(&cue);
            self.start(&cue);
        }
        if let Some((out, mixer)) = self.device.as_mut() {
            out.pump(mixer);
        }
    }

    fn start(&mut self, cue: &Cue) {
        let (Some((_, mixer)), Some(library)) = (self.device.as_mut(), self.library.as_mut()) else { return };
        match cue.what {
            What::Stop => {
                for p in self.playing.drain(..) {
                    mixer.stop(p);
                }
            }
            What::Music | What::Voice => match library.open(cue.what, cue.hash) {
                Ok(o) => {
                    let (kind, gain) = if cue.what == What::Voice { (Kind::Voice, barrel_audio::VOICE_GAIN) } else { (Kind::Music, 1.0) };
                    let p = mixer.play_stream(o.stream, gain, o.loop_from);
                    mixer.set_kind(p, kind);
                    self.playing.push(p);
                }
                Err(e) => eprintln!("sound: {}: {e}", cue.line()),
            },
            // STAND-IN: Effects are logged and their sample found in the bank, but not played: the banks' codec (`0xd2db2960`) is not decoded yet (docs/formats.md, "Ryza DX: sound containers").
            What::Effect => match library.effect(cue.hash) {
                Ok(info) if self.reported.insert(cue.hash) => eprintln!("sound: effect {info}: codec not decoded, silent"),
                Ok(_) => {}
                Err(e) => eprintln!("sound: {}: {e}", cue.line()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Sound {
        Sound::new(Path::new("."), Vec::new(), false)
    }

    #[test]
    fn the_title_starts_its_music_then_its_voice() {
        let c = title_cues(true);
        assert_eq!(c[0].line(), "2 music a5ba0cbd");
        assert_eq!(c[1].line(), "98 voice 8c7effb2");
        assert_eq!(title_cues(false).map(|c| c.at_frame), [18, 106]);
        assert_eq!(Cue::effect(692, PRESS_START).line(), "692 effect a699d027");
        assert_eq!(Cue::stop(1182).line(), "1182 stop -");
    }

    #[test]
    fn cues_come_due_once_in_frame_order() {
        let mut s = quiet();
        for c in title_cues(true) {
            s.schedule(c);
        }
        s.schedule(Cue::effect(50, CLICK));
        s.tick(1);
        assert_eq!(s.queue.len(), 3);
        s.tick(60);
        assert_eq!(s.queue.iter().map(|c| c.at_frame).collect::<Vec<_>>(), [98]);
        s.request("press", Cue::effect(692, PRESS_START));
        s.request("press", Cue::effect(692, PRESS_START));
        assert_eq!(s.queue.len(), 2);
        s.tick(9999);
        assert!(s.queue.is_empty());
        assert_eq!(frame_at(8.75), 525);
        assert_eq!(frame_at(18.0 / 60.0), 18);
    }

    /// Opens the title's streams, the field's music and a menu effect's bank sample from a real
    /// install: `BARREL_RYZA=<game folder> BARREL_RYZA_SFX_XML=<sfx.xml> cargo test -p barrel-ryza
    /// -- --ignored --nocapture library_opens`.
    #[test]
    #[ignore]
    fn library_opens_the_installs_sounds() {
        let (Ok(root), Ok(xml)) = (std::env::var("BARREL_RYZA"), std::env::var("BARREL_RYZA_SFX_XML")) else { return };
        let effects = crate::music::parse_table(&std::fs::read(xml).unwrap());
        let mut lib = Library { root: PathBuf::from(root), streams: HashMap::new(), effects: effects.into_iter().map(|e| (e.hash, e)).collect(), banks: HashMap::new() };
        for (what, hash) in [(What::Music, TITLE_MUSIC), (What::Voice, TITLE_VOICE), (What::Music, 0x03be_a3b2)] {
            let mut o = lib.open(what, hash).unwrap();
            let rate = o.stream.id().rate as usize;
            let mut frames = 0;
            while frames < rate {
                frames += o.stream.read().expect("a second of sound")[0].len();
            }
            eprintln!("{hash:08x}: {} Hz, {} frames, loop from {:?}", rate, o.stream.frames(), o.loop_from);
        }
        for hash in [PRESS_START, CLICK, CANCEL, WINDOW, CURSOR, PAGE] {
            eprintln!("{}", lib.effect(hash).unwrap());
        }
    }
}
