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

## Targets

The engine has to run well across three tiers, set early so the design fits them:

- **Handheld, high end: Adreno GPUs on Mesa's Turnip driver**, aarch64 Linux (not Android),
  such as a Snapdragon 8 Gen 2 handheld. Every game (the Dusk titles, Sophie 2, the Ryza
  trilogy).
- **Handheld, low end: Mali GPUs** such as the RK3566's Mali-G52, aarch64 Linux, for the
  older Dusk titles (Ayesha, Escha & Logy, Shallie), as far as its Vulkan driver allows: a
  best effort, never a reason for a second backend.
- **Desktop**, with newer features as options: upscaling (FSR), anti-aliasing (SMAA, FXAA,
  MSAA, TAA) and higher resolutions and rates.

What follows for the design:

- Both handheld GPUs are tile-based: render passes with clear or don't-care loads and
  don't-care stores wherever a target is not read back, few passes and few full-screen
  round trips through memory, no read-back of the frame in the middle of it.
- Bandwidth is the scarce thing on Mali: the narrowest targets that hold the look
  (`B10G11R11` or 8-bit over 16-bit float where it suffices), small shadow maps, mipmapped
  and block-compressed textures (BC on desktop; the handhelds need what their drivers
  sample: Turnip has BC, Mali needs ETC2/ASTC or a decode on load).
- Shaders kept simple, with half precision where it is exact enough; no geometry or
  tessellation shaders.
- The scene drawn at an internal resolution and scaled to the screen, so the handhelds can
  draw below native and the desktop above it; desktop upscalers (FSR 1 needs only the frame;
  FSR 2/3 and TAA need motion vectors and a jittered projection, kept in mind for the
  passes) and anti-aliasing are optional passes, off on the handhelds by default.
- **Vulkan only**, written directly and tuned for Linux's Mesa drivers (Turnip, RADV and the
  others): no OpenGL or OpenGL ES backend. Vulkan 1.1 is the baseline today
  (`crates/render`). If a device's driver cannot run it (the RK3566 depends on Mesa's PanVK
  for its Mali generation, not checked yet), that device goes without.
- Measured on the devices themselves, one test at a time, bounded (the RP6 notes for Turnip
  apply).

Where the renderer stands (audit of its passes): the scene draws into an 8-bit RGBA target
(the 16-bit float format is only an output option); its colour is cleared and stored (the post
pass and glow read it), its depth cleared and not stored, and the depth image is a transient
attachment with lazily allocated memory where the driver has it, so on Turnip and Mali it
should cost tile memory only (on RADV, which has no such memory, it falls back to device memory;
renders are identical either way). The shadow map is cleared and stored (2048² of 32-bit
depth, read by the scene pass); the glow levels are 8-bit; the final pass does not load the
image it covers. `shadow::Shadows` sets the shadow map: `DESKTOP` (2048², 32-bit float) or `HANDHELD`
(1024², 16-bit, a eighth of the memory and bandwidth; `barrel-view --handheld`). In the
first opening (`EVENT_MM01_OP1_111` at 25 s, 640×360) the two differ in about 20 of 230,400
pixels, one by more than 6%. Still to weigh: the glow's eleven passes, and a sunlit scene
for the shadow comparison.

## Deliberate differences

- **Fixed tick.** The original updates by each frame's measured time
  (`docs/events.md`, "Frame time"), with exactly 1/60 s as its fixed fallback. At a steady
  60 fps the two agree; when the original's frames are uneven its moves come out slightly
  differently (a bowed walk adds its velocity before taking off its push each frame, so the
  path depends on the step lengths). Barrel's results are those of a steady 60 fps. Lockstep
  checks compare against the original running at a steady 60 fps.
- **No Steam achievements (for now).** The original unlocks Steam achievements from events
  (`u38`, `docs/events.md`) and elsewhere. While the engine is being built and tested Barrel
  never touches the player's Steam account, so the event step (`event::Step::Trophy`) does
  nothing; once the engine is finished, unlocking them could become an option the player turns
  on. Lockstep checks skip the step (it changes no game state).

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
