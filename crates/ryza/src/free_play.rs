//! The field's own random draws while she plays: the ambient particles (`0x14008e0f0` and its follow-ups `0x14008e3cd` to
//! `0x14008e5a3`) draw from the game's one generator every 20 to 60 frames, and a gathering's draws come after them, so a run
//! that wants the game's gatherings (`barrel_ryza::gather`) has to make them too to keep the generator's index in step.
//!
//! What the game does, read off `0x14008e0f0` (the per-frame function) and `gather-rasen2`'s hook log: it keeps two timers for each
//! of thirty emitters (6 groups of 5), counts each down by the frame and, when one runs out with no particle alive, draws
//! `frand(0, 15)` (`0x14008e28b`: the timer's next length, added to the emitter's base) and, when it spawns a particle, the particle's
//! spread: `frand(-250, 250)` twice (`0x14008e3cd`, `0x14008e3e2`), `frand(0, 3)` (`0x14008e424`), `frand(0, 1)` (`0x14008e473`) and
//! `frand(0, 0.1)` (`0x14008e489`), then five more `frand(0, 0)` that draw nothing (`0x14008e4a1` to `0x14008e5a3`). A frame in which
//! several timers ran out draws their lengths first (up to five in the capture), then the one spawn's spread.
//!
//! STAND-IN: which timer runs out on which frame depends on the emitters' bases, their visibility and the field's own state, none of which
//! is read; a run takes the frames and what each drew from the capture's schedule (`tools/free_play.py` writes it from the hook log).
//! Without one, free play makes no draws, and the generator's index runs behind the game's.

use crate::random::Random;

/// What the emitters drew on one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emission {
    /// The frame (the capture's).
    pub frame: u32,
    /// How many timers were set again (`frand(0, 15)` each).
    pub timers: u32,
    /// Whether a particle was spawned with its spread (five more draws).
    pub spawn: bool,
}

impl Emission {
    /// Makes the draws.
    pub fn draw(&self, rng: &mut Random) {
        for _ in 0..self.timers {
            rng.frand(0.0, 15.0);
        }
        if self.spawn {
            for (a, b) in [(-250.0, 250.0), (-250.0, 250.0), (0.0, 3.0), (0.0, 1.0), (0.0, 0.1)] {
                rng.frand(a, b);
            }
        }
    }
}

/// A schedule from its text: one `FRAME TIMERS SPAWN` line each (SPAWN 0 or 1), `#` starting a comment, frames rising.
pub fn parse(text: &str) -> Result<Vec<Emission>, String> {
    let mut out: Vec<Emission> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let v: Vec<u32> = line.split_whitespace().map(str::parse).collect::<Result<_, _>>().map_err(|e| format!("line {}: {e}", n + 1))?;
        let [frame, timers, spawn] = v[..] else { return Err(format!("line {}: FRAME TIMERS SPAWN", n + 1)) };
        if out.last().is_some_and(|l| l.frame > frame) {
            return Err(format!("line {}: frames must rise", n + 1));
        }
        out.push(Emission { frame, timers, spawn: spawn != 0 });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_schedule_reads_its_lines() {
        let s = parse("# frame timers spawn\n2102 1 0\n2405 5 1 # a spray\n").unwrap();
        assert_eq!(s, vec![Emission { frame: 2102, timers: 1, spawn: false }, Emission { frame: 2405, timers: 5, spawn: true }]);
        assert!(parse("2405 5\n").is_err());
        assert!(parse("2405 5 1\n2102 1 0\n").is_err());
    }

    #[test]
    fn a_spray_is_ten_draws_after_five_timers_and_a_single_timer_one() {
        let mut r = Random::after(1, 0);
        r.record();
        Emission { frame: 0, timers: 5, spawn: true }.draw(&mut r);
        assert_eq!(r.draws().len(), 10);
        let before = r.index();
        Emission { frame: 1, timers: 1, spawn: false }.draw(&mut r);
        assert_eq!(r.index(), (before + 1) % 1248);
    }
}
