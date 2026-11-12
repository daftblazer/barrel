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
- **Desktop**, with newer features as options: upscaling (FSR 1, 2, 3 and 4.x where it can
  run), anti-aliasing (SMAA, FXAA, MSAA, TAA) and higher resolutions and rates.

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
- **Temporal upscaling** (FSR 2, 3 and 4.x, all wanted, each kept unless one is not worth
  keeping) and TAA need more from the frame than FSR 1: a per-pixel motion vector (each
  pixel's movement since the last frame, so skinned characters need last frame's bone
  matrices too, and moving objects last frame's transforms), a sub-pixel camera jitter that
  changes every frame, the depth buffer kept (not transient) for the upscaler to read,
  and masks for what it should not accumulate (particles, glow, transparent effects). The
  scene pass is planned around those. FSR 2 and 3 are compute passes that run on any Vulkan
  GPU (heavy for the handhelds); FSR 4 is a learned model for recent AMD GPUs, so it is a
  desktop option where its runtime is available for Vulkan on Linux.
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
(1024², 16-bit, an eighth of the memory and bandwidth; part of the handheld preset). In the
first opening (`EVENT_MM01_OP1_111` at 25 s, 640×360) the two differ in about 20 of 230,400
pixels, one by more than 6%. Still to weigh: the glow's eleven passes, and a sunlit scene
for the shadow comparison.

**Render scale.** The scene, its depth, the frozen frame and the glow levels are drawn at the
output size times a render scale (`barrel-view --scale S`, 0.25 to 2; `Targets::scene_size`),
and the post pass scales the scene to the output (bilinear, through the normalized
coordinates it already sampled by) while the text is drawn at the output's own size, so it
stays sharp. Scale 1 renders exactly as before; 0.5 draws a quarter of the pixels (softer
edges: 27 dB against scale 1 in the first opening) and 1.5 supersamples. The plain bilinear
scale is where an upscaler goes as an option: FSR 1 (AMD FidelityFX Super Resolution 1.0, MIT;
its notice kept in the shaders) is ported to plain GLSL, its edge-adaptive scale
(`shaders/easu.frag`, 12 taps by `textureGather`) and its sharpening (`shaders/rcas.frag`),
with their constants worked out in `fsr.rs`; both need only the finished frame. With
`barrel-view --fsr` and a scale below 1, `upscale.rs` runs EASU (scene to an output-size
target) and RCAS (0.2 stops below the sharpest) after the glow, and the post pass reads the
sharpened image in the scene's place; the glow and the frozen frame stay at the scene's size.
In the first opening at 1280×720 from half size, FSR reads visibly sharper than the bilinear
scale (closer to the full-size frame by eye) though a little further by PSNR (29.4 against
30.1 dB: sharpening adds edge contrast the measure counts as error); FSR expects an
anti-aliased input, and the scene's hard edges come out crisper rather than smoother, so
anti-aliasing is the next piece. At scale 1 `--fsr` changes nothing.

**Motion vectors.** With `motion` on (`barrel-view --motion`, `--show-motion` to see them as
red and green, 40 times their size) the scene pass writes a second colour target
(`R16G16_SFLOAT`, single-sampled: motion turns MSAA off): each pixel's movement since the last
frame in texture coordinates, `(this clip position − last frame's) / 2` from the vertex shader,
last frame's view-projection riding in the `Look` block (entries 40 to 43). Objects and characters
move in it too: each instance keeps last frame's model matrix (`Instance::prev_model`) and the
palette buffer holds last frame's joints after this frame's (`Instance::prev_palette`), so the
vertex shader skins each vertex twice, with this frame's joints and matrix and with last
frame's (the facial morphs are taken as this frame's on both sides). Both are snapshotted at
the start of each frame and uploaded every frame while motion is on. In the first opening at
7 s, with the camera finishing a move and a character walking, the characters stand out from
the camera's smooth field; at 8.5 s, the camera still and only idle animation left, the vectors
are a few thousandths and only on the characters. In the first opening's camera
move (2.5 s on, a 5-second turn and dolly) the vectors come to about 0.0014 across and 0.0008
down a frame at 60 Hz, a smooth field over the screen, as the move works out to; a still
camera gives zero. The viewer's seek to `--at` falls between its first two frames, which the
temporal methods have to treat as a cut (they drop their history there). With motion off the
frame is exactly as before.

**Jitter.** With `jitter` on (`barrel-view --jitter`) each frame's drawn view-projection is
shifted by a sub-pixel offset, the Halton sequence in bases 2 and 3 over 16 frames
(`settings::jitter`, -0.5 to 0.5 pixel), applied as a clip-space translation (2 × offset /
the scene's size) so it holds for any projection. The motion vectors leave it out: the shaders
take this frame's unjittered view-projection from `Look` (entries 44 to 47) for them, so a still
scene's vectors stay at zero while the frames differ (about 1% of pixels, along edges, from one
jittered frame to the next in the first opening). With jitter off the frame is as before.

**TAA.** `--taa` (preset `desktop-taa`) turns on the motion vectors and the jitter and
drops MSAA (`RenderSettings::resolved`). After the scene pass, `taa.frag` blends the jittered
scene into the history: the history is reprojected by the pixel's motion vector, clamped in
YCoCg to the min/max of the current 3 × 3 neighbourhood (so what has left the pixel does not
linger), and 10% of the new frame goes in (`taa::BLEND`). Pixels whose history falls off the
screen take the new frame alone. The result lands in a fixed `resolved` image that the post pass
(or FSR 1) reads, and is then copied into `history`, which keeps the descriptor sets fixed. The
history starts empty when the targets are made (first frame, resize). The glow still reads
the raw scene. In the first opening at 1080p, a still shot after 24 frames is about as smooth
as 4× MSAA on geometry edges, and also smooths the alpha-tested edges (lashes, outline tips)
that MSAA leaves. It is slightly softer, as TAA is. A camera pan with a character walking shows no
trails. On a camera cut the history starts afresh and the motion vectors take this frame's view
for the last (`taa::is_cut`: the eye jumps by more than a fifth of its distance to the target,
the view turns more than 10° or the field of view changes by more than 5° in one frame; a new
field counts too). Event tweens move a few hundredths of that per frame. After the jump that
`--at` makes on its first frame, TAA's next frame differs from the plain frame in 0.15% of
pixels (the jitter), against 0.55% without the reset (the old shot leaking through the clamp).
`BARREL_SHOTS=1` prints each frame's camera. Still to do: a sharpening pass after TAA (RCAS can
serve).

Validation (the Khronos layer, every mode): clean. The motion target needs the device's
`independentBlend` (enabled when present; desktop, Turnip and Mali have it), and the post
pipeline has its own one-target blend state.

**Wide screens.** The projection keeps the vertical field of view and widens the horizontal
one with the aspect, so 21:9 (3440×1440) and 32:9 (5120×1440) show more to the sides without
stretching (`barrel-view --size` takes up to 8192 either way; the first opening at 5120×1440
with 4× MSAA renders in about 0.6 s offscreen, 4K about the same). Event scenes were framed for
16:9: at 32:9 the first opening's camera sees past the end of the room's set into the empty
background on the right. Still to decide: fill such views (pillarbox events to the game's
16:9, or extend sets), as a deliberate difference either way.

**Settings.** `settings::RenderSettings` holds the scale, the upscaler (bilinear or FSR 1), the
MSAA samples and the shadow map, with presets: `DESKTOP` (full size, no MSAA, the large shadow
map: the renderer as it was), `DESKTOP_HIGH` (4× MSAA) and `HANDHELD` (three-quarter scale
through FSR 1, no MSAA, the small 16-bit shadow map). `barrel-view --preset NAME` picks one and
the single options (`--scale`, `--fsr`, `--msaa`, `--shadows`) change fields over it. In the
first opening the desktop preset matches the earlier plain frame and desktop-high the earlier
4× MSAA frame pixel for pixel; the handheld preset comes to 31.1 dB against the full frame.

**MSAA.** `barrel-view --msaa N` (1, 2, 4 or 8, capped at what the device allows for both
colour and depth) draws the scene with N samples a pixel into a transient multisampled colour
and depth, resolved into the scene image at the end of the scene pass (so on the handhelds the
samples stay in tile memory and only the resolved image is written out); the scene pipelines
take N samples, the shadow and post pipelines one. At 1 the frame is exactly as before; at 4 in
the first opening (1280×720) about 1,300 pixels change, all along edges, which come out
smooth. It suits FSR, which expects an anti-aliased input: from half size with 4× MSAA, FSR
comes to 32.1 dB against the full-size 4× frame, where without it it came to 29.4.

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
