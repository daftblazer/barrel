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

## Sound

`barrel-audio` keeps sound apart from the renderer. `Mixer` adds the playing sounds (each a
`Sound`: samples per channel, decoded natively) into one interleaved stereo float stream at
48 kHz, with a gain per sound, looping, and linear stepping for sounds at another rate; it is
plain code, tested without a device. `Output` is an SDL3 device stream in the same format: the
engine calls `pump(&mut mixer)` from its frame loop, which mixes just enough to keep about a
set time queued (a tenth of a second in the test), so the sound runs on the device's clock
while the logic keeps its fixed tick.

**Streaming.** Music is not decoded up front (a long track is tens of millions of frames and
takes seconds): `barrel_formats::vorbis::Stream` keeps the compressed packets and, from a pass
over each packet's mode bits (its block size), the frame each one's samples start at (a packet
gives a quarter of the previous block plus a quarter of its own). Seeking to a frame restarts
the decoder one packet early (that packet only primes the overlap), so the samples after a
seek are the same as a whole decode's, bit for bit (`barrel stream-check FILE.kvs`). The
mixer's stream voice (`Mixer::play_stream`) decodes just ahead of what it mixes and, at the
end, seeks to the loop's start and carries on in the same buffer, so the wrap is seamless and
interpolates across it as a whole clip would (the ignored test `streams_like_clips` compares
the two over two wraps). Decoding happens in the frame loop's pump, a packet (about a
thousandth of a second of work on the PC) at a time.
