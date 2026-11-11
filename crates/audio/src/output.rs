//! The system's audio output through SDL3: a device stream at [`crate::RATE`], stereo f32,
//! that the engine fills from its frame loop ([`Output::pump`]), keeping about `ahead` seconds
//! queued so a late frame does not starve it. Tests run with `SDL_AUDIO_DRIVER=dummy`.

use sdl3_sys::audio::*;
use sdl3_sys::init::{SDL_Init, SDL_QuitSubSystem, SDL_INIT_AUDIO};

use crate::{Mixer, RATE};

pub struct Output {
    stream: *mut SDL_AudioStream,
    /// How much to keep queued, in frames.
    ahead: usize,
    buffer: Vec<f32>,
}

impl Output {
    /// Opens the default playback device; `ahead` is the queue to keep, in seconds.
    pub fn open(ahead: f32) -> Result<Output, String> {
        unsafe {
            if !SDL_Init(SDL_INIT_AUDIO) {
                return Err(error());
            }
            let spec = SDL_AudioSpec { format: SDL_AUDIO_F32, channels: 2, freq: RATE as i32 };
            let stream = SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, &spec, None, std::ptr::null_mut());
            if stream.is_null() {
                return Err(error());
            }
            SDL_ResumeAudioStreamDevice(stream);
            Ok(Output { stream, ahead: (ahead * RATE as f32) as usize, buffer: Vec::new() })
        }
    }

    /// Mixes and queues enough frames to keep `ahead` queued. Returns the frames added.
    pub fn pump(&mut self, mixer: &mut Mixer) -> usize {
        let queued = unsafe { SDL_GetAudioStreamQueued(self.stream) }.max(0) as usize / 8;
        let want = self.ahead.saturating_sub(queued);
        if want == 0 {
            return 0;
        }
        self.buffer.resize(want * 2, 0.0);
        mixer.mix(&mut self.buffer);
        unsafe {
            SDL_PutAudioStreamData(self.stream, self.buffer.as_ptr().cast(), (self.buffer.len() * 4) as i32);
        }
        want
    }

    /// Frames still queued, not yet played.
    pub fn queued(&self) -> usize {
        unsafe { SDL_GetAudioStreamQueued(self.stream) }.max(0) as usize / 8
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        unsafe {
            SDL_DestroyAudioStream(self.stream);
            SDL_QuitSubSystem(SDL_INIT_AUDIO);
        }
    }
}

fn error() -> String {
    unsafe { std::ffi::CStr::from_ptr(sdl3_sys::error::SDL_GetError()) }.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens the device and plays half a second of a tone; run with `SDL_AUDIO_DRIVER=dummy`
    /// (`cargo test -p barrel-audio -- --ignored`).
    #[test]
    #[ignore]
    fn plays_through_sdl() {
        let mut out = Output::open(0.1).expect("audio device");
        let mut mixer = Mixer::new();
        let tone: Vec<f32> = (0..RATE as usize / 2).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin() * 0.1).collect();
        let p = mixer.play(std::sync::Arc::new(crate::Sound { rate: RATE, channels: vec![tone] }), 1.0, false);
        let mut fed = 0;
        let start = std::time::Instant::now();
        while mixer.is_playing(p) && start.elapsed().as_secs_f32() < 3.0 {
            fed += out.pump(&mut mixer);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(fed >= RATE as usize / 2, "fed {fed} frames");
        assert!(!mixer.is_playing(p));
    }
}
