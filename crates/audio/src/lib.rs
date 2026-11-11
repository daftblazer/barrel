//! Sound for the engine: [`Mixer`] adds up the playing sounds into one stereo stream, and
//! [`Output`] hands that stream to the system through SDL3 (an audio device stream the engine
//! keeps topped up from its frame loop). The mixer is plain code, testable without a device.

use std::sync::Arc;

pub mod output;
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

/// A sound being played.
#[derive(Clone, Debug)]
struct Voice {
    id: u64,
    sound: Arc<Sound>,
    /// The position in the sound's frames (fractional when its rate differs from [`RATE`]).
    at: f64,
    gain: f32,
    /// Where it starts again on reaching its end (in its frames), when it loops.
    loop_from: Option<usize>,
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
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer { voices: Vec::new(), next: 0, master: 1.0 }
    }

    /// Starts a sound at `gain`: once, or (with `loop_from`) going back to that frame each
    /// time it ends.
    pub fn play(&mut self, sound: Arc<Sound>, gain: f32, loop_from: Option<usize>) -> Playing {
        self.next += 1;
        let loop_from = loop_from.filter(|&f| f < sound.frames());
        self.voices.push(Voice { id: self.next, sound, at: 0.0, gain, loop_from });
        Playing(self.next)
    }

    pub fn stop(&mut self, p: Playing) {
        self.voices.retain(|v| v.id != p.0);
    }

    pub fn set_gain(&mut self, p: Playing, gain: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.id == p.0) {
            v.gain = gain;
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
            let step = f64::from(v.sound.rate) / f64::from(RATE);
            let len = v.sound.frames();
            if len == 0 {
                continue;
            }
            for f in 0..frames {
                let i = v.at as usize;
                if i >= len {
                    match v.loop_from {
                        Some(from) => v.at -= (len - from) as f64,
                        None => break,
                    }
                }
                let i = v.at as usize;
                let t = (v.at - i as f64) as f32;
                let (l0, r0) = v.sound.frame(i);
                let (l1, r1) = match (i + 1 < len, v.loop_from) {
                    (true, _) => v.sound.frame(i + 1),
                    (false, Some(from)) => v.sound.frame(from),
                    (false, None) => (l0, r0),
                };
                out[2 * f] += (l0 + (l1 - l0) * t) * v.gain;
                out[2 * f + 1] += (r0 + (r1 - r0) * t) * v.gain;
                v.at += step;
            }
        }
        self.voices.retain(|v| v.loop_from.is_some() || (v.at as usize) < v.sound.frames());
        for s in out.iter_mut() {
            *s = (*s * self.master).clamp(-1.0, 1.0);
        }
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
}
