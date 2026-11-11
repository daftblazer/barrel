# Engine design

Barrel plays the games as they play, on an engine built new: clean internals, no memory
layouts or code shapes carried over from the originals. Where Barrel behaves differently
from the original on purpose, it is listed under "Deliberate differences" so lockstep and
replay checks can allow for exactly that.

## Timing

Game logic runs on a **fixed tick**; the screen draws at its own rate and blends ticks.

- Logic (events, movers, turns, battle, the calendar) steps by exactly one tick each update,
  60 a second by default (`barrel_game::clock`, `DEFAULT_RATE`). Nothing in the logic reads
  the wall clock or a frame time.
- Each frame the host passes the real time since the last one to `Clock::advance`, which
  says how many ticks to run and how far the screen is into the next (`alpha`). The
  renderer keeps the state before and after the last tick (`stage::Snapshot`) and draws
  `Snapshot::blend(before, after, alpha)`: positions in a straight line, facings the short way
  round. So the picture is smooth at any refresh rate (60, 144, 180 Hz) and the logic is the
  same at all of them.
- A long stall (loading, a suspended window) runs at most 0.25 s of logic (15 ticks) when it
  ends instead of racing to catch up (`Clock::max_frame`; a host writing frames offline,
  where a long frame is not a stall, turns it off).
- `stage::Playback` is the host's side: each tick applies the event steps due by then and
  runs one tick of movers; it gives the same state as playing the steps straight through.
  `barrel play-event FILE --stage --fps HZ` drives it as a renderer at HZ would: for
  `EVENT_MM01_OP1_111` (97 s) 30, 60, 144 and 179.9 Hz all run 5,832 ticks to the same end
  state, byte for byte.
- The tick rate is a setting. Everything that moves takes its step as a parameter, so a
  higher rate is a one-line change, at the cost of a deliberate difference (below).

Why: the same inputs give the same run on every machine, so runs can be recorded, replayed
and checked in lockstep against the original; and drawing never waits for, or speeds up,
the game.

## Deliberate differences

- **Fixed tick.** The original updates by each frame's measured time
  (`docs/events.md`, "Frame time"), with exactly 1/60 s as its fixed fallback. At a steady
  60 fps the two agree; when the original's frames are uneven its moves come out slightly
  differently (a bowed walk adds its velocity before taking off its push each frame, so the
  path depends on the step lengths). Barrel's results are those of a steady 60 fps. Lockstep
  checks compare against the original running at a steady 60 fps.
