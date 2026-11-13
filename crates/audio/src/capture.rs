//! A file instead of a device: the mixer's output taken exactly up to a given time, so an
//! offscreen run's sound lines up with its frames (and with a recording of the game).

use crate::{Mixer, RATE};

/// The stereo samples mixed so far, from the capture's start.
#[derive(Default)]
pub struct Capture {
    samples: Vec<f32>,
}

impl Capture {
    pub fn new() -> Capture {
        Capture::default()
    }

    /// Mixes until `seconds` after the start (nothing if already there).
    pub fn until(&mut self, mixer: &mut Mixer, seconds: f64) {
        let want = (seconds.max(0.0) * f64::from(RATE)).round() as usize * 2;
        if want > self.samples.len() {
            let from = self.samples.len();
            self.samples.resize(want, 0.0);
            mixer.mix(&mut self.samples[from..]);
        }
    }

    /// The stereo frames captured.
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// As a 16-bit stereo WAV file at [`RATE`].
    pub fn wav(&self) -> Vec<u8> {
        let data = (self.samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data.to_le_bytes());
        for s in &self.samples {
            out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sound;
    use std::sync::Arc;

    #[test]
    fn captures_exactly_to_the_time_and_writes_a_wav() {
        let mut mixer = Mixer::new();
        mixer.play(Arc::new(Sound { rate: RATE, channels: vec![vec![0.5; RATE as usize]] }), 1.0, None);
        let mut c = Capture::new();
        c.until(&mut mixer, 0.25);
        c.until(&mut mixer, 0.1);
        c.until(&mut mixer, 0.5);
        assert_eq!(c.frames(), RATE as usize / 2);
        let wav = c.wav();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 44 + c.frames() * 4);
        assert!(i16::from_le_bytes([wav[44], wav[45]]) > 0);
    }
}
