//! The engine's clock: game logic steps at a fixed tick, the screen draws at its own rate
//! (`docs/engine.md`, "Timing").
//!
//! Each frame the host passes the real time since the last frame; [`Clock::advance`] says how
//! many whole ticks of logic to run and how far the screen is into the next one, so the
//! renderer can blend the last two ticks' states ([`crate::stage::Snapshot::blend`]). Logic never
//! sees a variable step, so a run is the same at any frame rate and can be replayed.

/// The logic rate the original was designed for (its fixed fallback step is exactly 1/60 s).
pub const DEFAULT_RATE: u32 = 60;

/// The longest real frame the clock accepts: a longer stall (loading, a breakpoint, a
/// suspended window) runs at most this much logic instead of catching up all of it.
pub const MAX_FRAME: f64 = 0.25;

#[derive(Clone, Debug)]
pub struct Clock {
    rate: u32,
    /// Real time not yet run as ticks, in seconds (f64 so it does not drift).
    owed: f64,
    /// Ticks run since the start.
    pub ticks: u64,
}

/// What a frame should do: run `ticks` logic ticks, then draw at `alpha` (0 to 1) of the way
/// from the state before the last tick to the state after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub ticks: u32,
    pub alpha: f32,
}

impl Clock {
    pub fn new(rate: u32) -> Clock {
        Clock { rate: rate.max(1), owed: 0.0, ticks: 0 }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// One tick's length in seconds, the step every logic update gets.
    pub fn tick(&self) -> f32 {
        1.0 / self.rate as f32
    }

    /// Logic time so far, in seconds.
    pub fn time(&self) -> f64 {
        self.ticks as f64 / self.rate as f64
    }

    /// Takes `real` seconds of wall time (clamped to [`MAX_FRAME`]).
    pub fn advance(&mut self, real: f64) -> Frame {
        let step = 1.0 / self.rate as f64;
        self.owed += real.clamp(0.0, MAX_FRAME);
        let ticks = (self.owed / step).floor();
        self.owed -= ticks * step;
        self.ticks += ticks as u64;
        Frame { ticks: ticks as u32, alpha: (self.owed / step).clamp(0.0, 1.0) as f32 }
    }
}

impl Default for Clock {
    fn default() -> Clock {
        Clock::new(DEFAULT_RATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_the_same_at_any_frame_rate() {
        // One second at 144 Hz and at 30 Hz both run 60 ticks.
        for hz in [144.0, 30.0, 60.0, 59.94] {
            let mut c = Clock::default();
            let mut ran = 0;
            let frames = (hz as f64).round() as usize;
            for _ in 0..frames {
                let f = c.advance(1.0 / hz);
                assert!((0.0..=1.0).contains(&f.alpha));
                ran += f.ticks;
            }
            assert!((ran as i64 - 60).abs() <= 1, "{hz} Hz ran {ran}");
        }
    }

    #[test]
    fn a_stall_runs_a_bounded_catch_up() {
        let mut c = Clock::default();
        assert_eq!(c.advance(5.0).ticks, 15);
        assert_eq!(c.advance(-1.0).ticks, 0);
    }
}
