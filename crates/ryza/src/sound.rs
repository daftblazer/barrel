//! What Ryza plays: for now the title's two cues, the music and one voice clip, with the
//! frame of the title each starts at, and the way to open one from the install's sound
//! containers (`barrel_formats::ktsr`).

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use barrel_audio::Kind;
use barrel_formats::{ktsr, voice, vorbis};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The folder of the sound containers under the install.
pub const SOUND_DIR: &str = "Data/x64/Sound";

/// The title's frames a second (the cues are timed in them).
pub const FPS: f32 = 60.0;

/// One sound the title starts: when (a frame of the title, 0 its first), what kind it is, and
/// which stream of which container it plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub at_frame: i64,
    pub kind: Kind,
    pub file: &'static str,
    /// The stream's id in its container (`ktsr::Stream::id`).
    pub id: u32,
    /// The stream's `KOVS` clip's offset in the container.
    pub offset: u64,
}

impl Cue {
    /// The cue's line in the cue log: `<frame> <kind> <file> <offset>`.
    pub fn line(&self) -> String {
        let kind = match self.kind {
            Kind::Music => "music",
            Kind::Effect => "effect",
            Kind::Voice => "voice",
        };
        format!("{} {kind} {} {}", self.at_frame, self.file, self.offset)
    }
}

// STAND-IN: The title's music and voice are the streams the original's run read; the real thing is the game's own choice of them (its sound banks and title code).
/// The title's cues in order: the music (offset 151456224 of the BGM container; it loops) from
/// frame 18, and one voice clip (offset 94885392 of the voice container; it plays once) from
/// frame 106. The frames are those at which a run of the game starts the two sounds.
pub fn title_cues() -> [Cue; 2] {
    [
        Cue { at_frame: 18, kind: Kind::Music, file: "bgm.ktsl2stbin", id: 0xa5ba_0cbd, offset: 151_456_224 },
        Cue { at_frame: 106, kind: Kind::Voice, file: "voice.ktsl2stbin", id: 0x8c7e_ffb2, offset: 94_885_392 },
    ]
}

/// The title frame `seconds` into the title is on.
pub fn frame_at(seconds: f32) -> i64 {
    // a little over, so a time given as frame / 60 lands on that frame
    (seconds * FPS + 1e-3).floor() as i64
}

/// Hands out each cue once, when the title's clock reaches its frame.
pub struct Cues {
    list: Vec<Cue>,
    next: usize,
}

impl Cues {
    pub fn new(list: &[Cue]) -> Cues {
        Cues { list: list.to_vec(), next: 0 }
    }

    /// The cues not yet handed out whose frame is `frame` or before.
    pub fn due(&mut self, frame: i64) -> Vec<Cue> {
        let from = self.next;
        while self.list.get(self.next).is_some_and(|c| c.at_frame <= frame) {
            self.next += 1;
        }
        self.list[from..self.next].to_vec()
    }
}

/// With `BARREL_TRACE_CUES=<file>` set, appends the cue's line to it.
pub fn trace(cue: &Cue) {
    let Some(path) = std::env::var_os("BARREL_TRACE_CUES") else { return };
    let written = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{}", cue.line()));
    if let Err(e) = written {
        eprintln!("cue log: {e}");
    }
}

/// A cue's stream, ready for the mixer, with the frame it loops from (music only; the clip's
/// own loop start, unless it is marked to play once).
pub struct Opened {
    pub stream: vorbis::Stream,
    pub loop_from: Option<u64>,
}

/// Opens the cue's stream from the install (`root` the game's folder).
pub fn open(cue: &Cue, root: &Path) -> Result<Opened> {
    let mut file = BufReader::new(std::fs::File::open(root.join(SOUND_DIR).join(cue.file))?);
    let stream = ktsr::stream_at(&mut file, cue.offset)?;
    if stream.id != cue.id {
        return Err(format!("{}: the stream at {} is not {:#x}", cue.file, cue.offset, cue.id).into());
    }
    let kovs = ktsr::read_stream(&mut file, &stream)?;
    let looping = voice::looping(&kovs)?;
    let loop_from = (cue.kind == Kind::Music && !looping.once).then_some(u64::from(looping.start));
    Ok(Opened { stream: vorbis::Stream::open(&voice::ogg(&kovs)?)?, loop_from })
}

/// The title's sound: starts each cue as the title's clock reaches it, on the audio device when
/// there is one (a window; an offscreen run only logs).
pub struct TitleSound {
    cues: Cues,
    root: PathBuf,
    device: Option<(barrel_audio::Output, barrel_audio::Mixer)>,
}

impl TitleSound {
    pub fn new(root: &Path, with_device: bool) -> TitleSound {
        let device = with_device.then(|| barrel_audio::Output::open(0.1).map_err(|e| eprintln!("title: no sound: {e}")).ok()).flatten().map(|o| (o, barrel_audio::Mixer::game()));
        TitleSound { cues: Cues::new(&title_cues()), root: root.to_path_buf(), device }
    }

    /// Called every frame with the seconds into the title: starts the cues now due (logging
    /// each), and keeps the device fed.
    pub fn tick(&mut self, seconds: f32) {
        for cue in self.cues.due(frame_at(seconds)) {
            trace(&cue);
            let Some((_, mixer)) = self.device.as_mut() else { continue };
            match open(&cue, &self.root) {
                Ok(o) => {
                    let gain = if cue.kind == Kind::Voice { barrel_audio::VOICE_GAIN } else { 1.0 };
                    let p = mixer.play_stream(o.stream, gain, o.loop_from);
                    mixer.set_kind(p, cue.kind);
                }
                Err(e) => eprintln!("title: {}: {e}", cue.line()),
            }
        }
        if let Some((out, mixer)) = self.device.as_mut() {
            out.pump(mixer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_starts_its_music_then_its_voice() {
        let c = title_cues();
        assert_eq!((c[0].at_frame, c[0].kind, c[0].offset), (18, Kind::Music, 151_456_224));
        assert_eq!((c[1].at_frame, c[1].kind, c[1].offset), (106, Kind::Voice, 94_885_392));
        assert_eq!(c[1].line(), "106 voice voice.ktsl2stbin 94885392");
    }

    #[test]
    fn cues_come_due_once_in_order() {
        let mut q = Cues::new(&title_cues());
        assert!(q.due(17).is_empty());
        assert_eq!(q.due(18).len(), 1);
        assert!(q.due(100).is_empty());
        assert_eq!(q.due(525).len(), 1);
        assert!(q.due(9999).is_empty());
        assert_eq!(Cues::new(&title_cues()).due(525).len(), 2);
        assert_eq!(frame_at(8.75), 525);
        assert_eq!(frame_at(18.0 / 60.0), 18);
    }

    /// Decodes the first second of both title streams from a real install:
    /// `BARREL_RYZA=<game folder> cargo test -p barrel-ryza -- --ignored --nocapture title_streams`.
    #[test]
    #[ignore]
    fn title_streams_decode() {
        let Ok(root) = std::env::var("BARREL_RYZA") else { return };
        for cue in title_cues() {
            let mut o = open(&cue, Path::new(&root)).unwrap();
            let id = o.stream.id();
            let mut frames = 0;
            while frames < id.rate as usize {
                frames += o.stream.read().expect("a second of sound")[0].len();
            }
            eprintln!("{}: {} Hz, {} channels, {} frames, loop from {:?}", cue.file, id.rate, id.channels, o.stream.frames(), o.loop_from);
        }
    }
}
