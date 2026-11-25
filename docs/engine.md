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
(1024², 16-bit, an eighth of the memory and bandwidth; part of the handheld preset). Ryza's
image is 2400² (1200² handheld) and each layer holds the game's own map from its corner
(`Frame::shadow_sides`: the cascades 2000 × 2400, 2000 × 2400 and 1600², the static 2048², the
characters' 1024²). In the
first opening (`EVENT_MM01_OP1_111` at 25 s, 640×360) the two differ in about 20 of 230,400
pixels, one by more than 6%. Still to weigh: the glow's eleven passes, and a sunlit scene
for the shadow comparison.

**Render scale.** The scene, its depth, the frozen frame and the glow levels are drawn at the
output size times a render scale (`barrel --scale S`, 0.25 to 2; `Targets::scene_size`),
and the post pass scales the scene to the output (bilinear, through the normalized
coordinates it already sampled by) while the text is drawn at the output's own size, so it
stays sharp. Scale 1 renders exactly as before; 0.5 draws a quarter of the pixels (softer
edges: 27 dB against scale 1 in the first opening) and 1.5 supersamples. The plain bilinear
scale is where an upscaler goes as an option: FSR 1 (AMD FidelityFX Super Resolution 1.0, MIT;
its notice kept in the shaders) is ported to plain GLSL, its edge-adaptive scale
(`shaders/easu.frag`, 12 taps by `textureGather`) and its sharpening (`shaders/rcas.frag`),
with their constants worked out in `fsr.rs`; both need only the finished frame. With
`barrel --fsr` and a scale below 1, `upscale.rs` runs EASU (scene to an output-size
target) and RCAS (0.2 stops below the sharpest) after the glow, and the post pass reads the
sharpened image in the scene's place; the glow and the frozen frame stay at the scene's size.
In the first opening at 1280×720 from half size, FSR reads visibly sharper than the bilinear
scale (closer to the full-size frame by eye) though a little further by PSNR (29.4 against
30.1 dB: sharpening adds edge contrast the measure counts as error); FSR expects an
anti-aliased input, and the scene's hard edges come out crisper rather than smoother, so
anti-aliasing is the next piece. At scale 1 `--fsr` changes nothing.

**Motion vectors.** With `motion` on (`barrel --motion`, `--show-motion` to see them as
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

**Jitter.** With `jitter` on (`barrel --jitter`) each frame's drawn view-projection is
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
`BARREL_SHOTS=1` prints each frame's camera.

**Sharpening.** `sharpen` (`--sharpen`, on in `desktop-taa`) runs FSR 1's RCAS on the scene
when it is not scaled (FSR 1 already sharpens when it scales): the upscaler's targets skip
EASU's image and RCAS reads TAA's result directly, at the same 0.2 stops. At 1080p in the first
opening it gives back the hair strands, eyes and cloth texture TAA softens, while the edges
stay smooth.

Validation (the Khronos layer, every mode): clean. The motion target needs the device's
`independentBlend` (enabled when present; desktop, Turnip and Mali have it), and the post
pipeline has its own one-target blend state.

**GSR 1.** `--gsr` (`Upscaler::Gsr1`) scales the scene with Snapdragon GSR 1 instead of FSR 1:
one pass (`gsr1.frag`, from `sgsr1_shader_mobile.frag` in RGBA mode with green as the luma;
edge threshold 8/255, sharpness 2) straight into the upscaler's output, against FSR 1's two.
Precision qualifiers stay as in the original (mediump colour maths, which Adreno runs at half
precision); desktop GLSL refuses them on constructors, so those were dropped. At 0.75 scale
into 1080p it is far sharper than bilinear and cleaner than FSR 1 (less ringing, a little less
crisp). Which one the handheld preset uses is decided by timing both on the RP6. FSR 1 and the
default frame are byte-identical to before.

**GSR 2.** `--gsr2` (`RenderSettings::gsr2`, with `--scale` below 1) is Snapdragon GSR 2's
2-pass fragment variant, Qualcomm's choice for mobile, in `gsr2.rs`: `gsr2_convert.frag`
writes each render pixel's motion (NDC) and depth clip (how far its depth stands out from the
nearest of its 4 × 4 neighbourhood) into an RGBA16F image. `gsr2_upscale.frag` then
upsamples the jittered scene with a 5-tap Lanczos-like kernel, clamps the reprojected history
to the taps' variance-narrowed box and blends them into one of two RGBA16F output-size images,
reading the other as history: they swap roles each frame, with no copy (the handhelds are short
of bandwidth; a copy was one output-sized read and write a frame). The post pass has two
descriptor sets, one per output, and binds the one just written (`Targets::post_set`). The
frames are byte-identical to the copying version's. It turns on the motion vectors and the jitter and turns off TAA,
MSAA, FSR 1/GSR 1 and RCAS (`RenderSettings::resolved`). Depth is then sampled, stored and left
in `SHADER_READ_ONLY_OPTIMAL` by the scene pass (transient otherwise). It is read through a
nearest sampler, as D32 need not filter. Changes from Qualcomm's shaders: the motion comes
from Barrel's own vectors (×2 into NDC), which already hold the camera's movement, not from a
depth reprojection of static pixels. The taps' texel positions are clamped to the render. The
corner taps the original disables are left out. The constants follow the compute variant
(kernel bias = output/render width, at most 1.99; box scale = min(20, area ratio³)).
`minLerpContribution`, left to the caller, is 0.3 for a still camera and 0 otherwise.
History resets share TAA's cut detection. From half scale into 1080p in the first opening it is
the only upscaler without stair-stepped edges, near native and slightly soft; the walk and
camera pan at 6.4 s show no trails. Validation clean. Still to do: time it on the RP6 against
FSR 1 and GSR 1.

**FSR 3.1.** The upscaler's shaders are vendored unchanged from the FidelityFX
SDK v1.1.4 into `crates/render/shaders/ffx/` (the GPU headers and the Vulkan GLSL pass entry
points, 32 files, all MIT) and compiled by `build.rs` as compute shaders with glslc. The
permutation is LDR colour, render-size motion vectors (the SDK's usual case; full-size ones
make moving pixels read the wrong vectors, which smeared a walking character) without the jitter, depth not inverted,
the LUT Lanczos for reprojection, and no sharpening inside the accumulate pass (RCAS as its own
pass). The SDK's host code (`ffx_fsr3upscaler.cpp`, not vendored) dispatches: prepare inputs →
luma pyramid (SPD) → shading-change pyramid → shading change → prepare reactivity → luma
instability → accumulate → RCAS. `fsr3.rs` has the CPU side, ported from that host code: the main constant buffer
(`Constants`, std140, 148 bytes) and its per-frame update, the Lanczos-2 table (128 × R16_SNORM),
the jitter phase count (8 × upscale ratio², so 32 phases from half scale; Barrel's jitter has to
follow it when FSR 3 runs), SPD's setup and RCAS's constants. Barrel's motion vectors are
current minus previous in texture units, so the motion vector scale is -1.

`--fsr3` (`RenderSettings::fsr3`, with `--scale` below 1) runs it: the eight passes as
compute pipelines whose bindings come from the compiled shaders' reflection (bindings the
compiler dropped are left out), one descriptor set per pass for even and odd frames (the
swapped pairs), every internal image kept in `GENERAL` with one memory barrier between passes,
the constant buffers in one host-visible buffer (main, SPD and RCAS at 256-byte steps), RCAS at
0.8 sharpness (the SDK samples' default) writing the RGBA16F output the post pass reads. The
jitter follows FSR's phase count (`settings::jitter_cycle`); depth is kept as for GSR 2; the
format-less storage writes need `shaderStorageImageWriteWithoutFormat` (enabled when present).
One change from the SDK: the luma history is RGBA8, as the shaders declare it (the SDK makes it
RGBA16F, which a declared `rgba8` storage image does not match in Vulkan). From half scale into
1080p in the first opening it is sharper than GSR 2 and close to native, and the walk at 6.4 s
stays crisp. Validation clean. Still to do: a reactive mask for the transparencies and the
glow, timing, and Barrel's own jitter cycle for TAA and GSR 2 kept at 16.

The images (all at render size unless noted): dilated motion (RG16F), dilated
depth (R32F) and the reconstructed previous nearest depth (R32UI, cleared to 1.0's bits each
frame); luma 1/2 (R16F, swapped each frame); luma history 1/2 (RGBA16F); accumulation 1/2
(R8); internal upscaled 1/2 (RGBA16F, output size); new locks (R8, output size); the SPD mips
(RG16F, half size, 6 levels) and its atomic counter (R32UI, cleared each frame); farthest-depth
mip 1 (R16F, half); shading change (R8, half); dilated reactive masks (RGBA8); an R16F
intermediate shared by farthest depth and luma instability; frame info (RGBA32F 1 × 1); 1 × 1
defaults for exposure (RG32F) and the reactive and transparency masks (R8). Two samplers
(point and linear clamp) at bindings 1000/1001. On a reset, accumulation, the SPD mips and frame
info (-1, 1) are cleared.

**Timings** (`barrel --timings`: GPU timestamps at the stage boundaries, medians after 3
warm-up frames). The first opening's still shot at 25 s, 30 frames, on the desktop's RDNA 4
Radeon (Navi 48), in ms; "AA/up" includes the glow:

| mode | 1080p scene | AA/up | 4K scene | AA/up |
|---|---|---|---|---|
| native | 1.00 | 0.09 | 2.24 | 0.14 |
| MSAA 4× | 1.02 | 0.08 | 2.06 | 0.13 |
| TAA + RCAS | 1.08 | 0.23 | 2.12 | 0.64 |
| FSR 1 from 0.67 | 0.79 | 0.25 | 1.26 | 0.74 |
| GSR 1 from 0.67 | 0.79 | 0.11 | 1.37 | 0.25 |
| GSR 2 from 0.67 | 0.82 | 0.22 | 1.41 | 0.71 |
| FSR 3.1 from 0.67 | 0.83 | 0.64 | 1.40 | 2.09 |
| FSR 3.1 from 0.5 | 0.69 | 0.57 | 1.07 | 1.78 |
| FSR 3.1 at 1.0 (native AA) | 1.08 | 0.82 | | |

The shadow map is a steady 0.34 ms and the post pass 0.03 to 0.2. Escha's scenes are light: on
this GPU no upscaler earns back its cost at 4K. FSR 3.1 costs more than the scene time it saves,
and MSAA 4× costs about nothing. So the desktop presets keep full resolution and choose an
anti-aliasing: `desktop-high` (MSAA 4×), `desktop-taa` (TAA + RCAS), `desktop-fsr3` (FSR 3.1 as
native AA: the cleanest edges, at about 0.8 ms at 1080p). Upscalers are for weak GPUs:
`handheld` (FSR 1 from 0.75) and `handheld-gsr2` (GSR 2 from two thirds), to be chosen
between by timing them on the RP6.

`--timings` also times the CPU's stages of the frame loop (`timing::CpuTimer`: input and
simulation, people and events and poses, acquire, camera and lights, HUD layers, then record,
submit and present, and the wait for the GPU; offscreen the last three are one). On the RP6
(Turnip, Adreno 740) at 1080p with the `handheld` preset, 300 frames, in ms:

| scene | shadow | scene | glow + FSR 1 | post | GPU total | CPU before recording |
|---|---|---|---|---|---|---|
| Sector One, the user's 8/20 save | 0.74 | 5.91 | 1.17 | 0.21 | 8.04 | 0.33 |
| the opening at 20 s (town) | 0.50 | 5.42 | 1.17 | 0.18 | 7.28 | 0.43 |

The CPU's own work before recording is small; a frame is the recording plus the GPU's time, one
after the other while the loop waits for each frame. Offscreen runs read a frame back only when
they write it to a file (the whole image every frame cost about 7 ms there).

**FSR 2, 3 and 4: plan** (researched 2026-11-12).

- *FSR 2 and FSR 3.1's upscaler* are one algorithm line, open under MIT: FSR 2.2's own
  repository had GLSL shaders and a Vulkan backend, and the FidelityFX SDK carries the 3.1
  upscaler (now 3.1.6) with DirectX 12 and Vulkan backends. The SDK builds for Windows only
  (CMake, batch scripts), so Barrel ports the upscaler's compute passes into its own Vulkan
  code, as with FSR 1, rather than linking the SDK. Its inputs are the ones the renderer already
  has: jittered colour, depth, motion vectors (with the jitter left out), the jitter offset,
  a reset flag on cuts, plus a reactive mask for transparencies and the glow. 3.1 is FSR 2's
  successor (less ghosting and shimmer, same inputs), so 2.2 is kept only if it is
  noticeably cheaper on the handhelds, as a separate preset, once both are measured on the RP6.
  Depth must be stored for it (the scene pass's depth is transient now): a sampled depth
  target only when FSR 2/3 is on.
- *FSR 3 frame generation* (interpolated frames) is a separate piece: it needs the swapchain's
  pacing and adds latency. Later, desktop only, off by default.
- *FSR 4* (ML-based; 4.1.2 now also on older Radeons) is binary only. The SDK 2.4 Vulkan
  runtime is a Windows DLL, and the DLL itself decides which GPUs get it. AMD has said nothing
  of Linux; the Linux routes so far load third-party builds through `dlopen`. So Barrel keeps
  FSR 4 as an optional runtime-loaded backend behind the same inputs as 3.1, used only when a
  native Linux library is present. It is never shipped, never required, and 3.1 is the
  fallback. Nothing to build for it until AMD ships one.

- *Snapdragon Game Super Resolution* (Qualcomm, BSD 3-clause, plain shaders on GitHub, so
  nothing tying it to Android or a driver: it runs on Turnip and desktops alike). GSR 1 is
  spatial, one pass that scales and sharpens one image with a 12-tap Lanczos-like filter: a
  one-pass alternative to FSR 1's two (EASU + RCAS) on the handhelds. GSR 2 is temporal (the
  same inputs as FSR 2/3: colour, depth, motion, jitter) and is made for Adreno's tiled GPUs, with
  fewer and lighter passes than FSR 3.1. That makes it the likely temporal upscaler for the RP6,
  with FSR 3.1 for desktops. Both are measured on the RP6 before choosing defaults. Ported into
  Barrel's own passes like FSR 1, the licence notice kept with the shaders.

Order: store depth, then GSR 1 (small, handheld), the temporal upscalers on the shared inputs
(GSR 2, then FSR 3.1), a reactive mask, then (desktop) frame generation.

**Wide screens.** The projection keeps the vertical field of view and widens the horizontal
one with the aspect, so 21:9 (3440×1440) and 32:9 (5120×1440) show more to the sides without
stretching (`barrel --size` takes up to 8192 either way; the first opening at 5120×1440
with 4× MSAA renders in about 0.6 s offscreen, 4K about the same). Event scenes were framed for
16:9: at 32:9 the first opening's camera sees past the end of the room's set into the empty
background on the right. Still to decide: fill such views (pillarbox events to the game's
16:9, or extend sets), as a deliberate difference either way.

**Settings.** `settings::RenderSettings` holds the scale, the upscaler (bilinear or FSR 1), the
MSAA samples and the shadow map, with presets: `DESKTOP` (full size, no MSAA, the large shadow
map: the renderer as it was), `DESKTOP_HIGH` (4× MSAA) and `HANDHELD` (three-quarter scale
through FSR 1, no MSAA, the small 16-bit shadow map). `barrel --preset NAME` picks one and
the single options (`--scale`, `--fsr`, `--msaa`, `--shadows`) change fields over it. In the
first opening the desktop preset matches the earlier plain frame and desktop-high the earlier
4× MSAA frame pixel for pixel; the handheld preset comes to 31.1 dB against the full frame.

**MSAA.** `barrel --msaa N` (1, 2, 4 or 8, capped at what the device allows for both
colour and depth) draws the scene with N samples a pixel into a transient multisampled colour
and depth, resolved into the scene image at the end of the scene pass (so on the handhelds the
samples stay in tile memory and only the resolved image is written out); the scene pipelines
take N samples, the shadow and post pipelines one. At 1 the frame is exactly as before; at 4 in
the first opening (1280×720) about 1,300 pixels change, all along edges, which come out
smooth. It suits FSR, which expects an anti-aliased input: from half size with 4× MSAA, FSR
comes to 32.1 dB against the full-size 4× frame, where without it it came to 29.4.

## Mods: a core feature

Mod support is part of the engine's design from the start, though its own work comes after
each game is playable. Planned: texture packs and model overrides (PNG and glTF accepted, not
only the game's formats), shader packs with access to the scene's depth, normals and what each
pixel is (characters, terrain, sky), new outfits as added slots rather than overwrites, new
items, recipes and events as data, photo mode, and a Mods page in the game's own Settings
menu (no launcher). So everything built now follows these rules:

- **One way to find a file.** Every original file is read by its game path through one layered
  file system (`barrel-files`, below); a mod folder is one more layer in front of it. Nothing
  keeps archive offsets or handles of its own.
- **Lists, not fixed slots.** Outfits, items, recipes, characters and the like are held as
  growable lists keyed by name, never as arrays sized by the original's counts; menus show what
  the list holds.
- **Saves by name.** Barrel's own save data refers to content by a stable name, so adding a mod
  never shifts others, and a removed mod's content falls back cleanly instead of breaking the
  save. (Reading the original's saves maps its numbers to these names.)
- **Data stays data.** Tables, events and scripts are loaded and interpreted (the event player
  runs the game's own scripts from a command table), not compiled in, so new ones can be added.
- **Formats behind loaders.** A texture or model is loaded by kind through one function, so a
  PNG or glTF can stand in where a `.g1t` or `.g1m` is expected.
- **Shaders replaceable.** The renderer's shaders are compiled in today (SPIR-V through
  `include_bytes!`); a shader pack needs them loaded from files with these as the fallback, and
  the scene's buffers kept available to post-processing.

### The layered files (`crates/files`, `barrel-files`)

Every read of an original game file (the tables, the executable's PE, archives, movies, fonts,
field folders, sounds, `Saves_EN/...`) goes through `barrel_files`: `read`, `read_to_string`,
`open`, `exists`, `is_dir`, `is_file`, `list`. They take the install's own path
(`<install>/Data/x64/...`, what the code already builds with `root.join(..)`); a path under the
installed layers' base folder becomes the game-relative path (`Data/x64/...`) and is looked up
from the topmost root down, case-insensitively and with either slash (the game's paths are
Windows paths); any other path is an ordinary file's, so saves, the config and the capture
files need no special case. `GameFs` is the same API on a value (`GameFs::new(install)
.with_layer(name, root)`, `read(rel)`, `layer_of(rel)`, a merged `list(rel)`), used by the tests.

- The lookup tries the exact spelling first (one stat; with no mods the read is the one the code
  made before), then a per-folder listing cache for the case-insensitive match.
- `Archives::open_dir` lists `Data` through the layers, so a mod may replace a `.PAK` by name or
  add archives (later names win a path, as the game's do).
- Only bytes or an open `File` come from the layer; `pak`, `g1t`, `table` and the others are
  unchanged. `barrel_formats::table::read(path)` is the table file read and parsed in one step.
- `barrel-cli` (the developer tool) reads files directly: it works on extracted or original
  folders, not on a played game.
- The layers are installed once a run (`barrel_files::install`, from `barrel_render::locate::
  resolve` at the start of `barrel` and `barrel`); before that, and in tests, reads
  are plain. The unit tests cover override order, case-insensitive lookup, missing-file errors
  and a mod overriding one file (`crates/files`, `crates/escha/tests/mod_layers.rs`).

Mods and the config: see `docs/mods.md`. The code is `config` (the config folder and its
`games.toml`, `mods.toml`), `mods` (`mod.toml`, load order, `needs`), `find` (the search for the
install, Steam's `libraryfolders.vdf` and `appmanifest_*.acf` through `vdf`), `toml` (the small
subset the files use) and `browse` (the Locate screen's folder list). The screen itself is
`barrel_render::locate`: flat rectangles and its own font atlas (`locate_font`, Noto Sans,
regenerated by `tools/gen-locate-font.py`), through the same text layer as the game's screens,
since the game's art cannot be loaded before the game is found. It opens its own window and
hands it on (a process has one event loop).

## Controller (being read)

The game reads a pad through XInput (`XINPUT1_3` ordinals 2 and 3, `XInputGetState` and
`XInputSetState`, called from `0x1405d1160`) and DirectInput 8 for other pads
(`DirectInput8Create`, `0x1405cc880`). Barrel reads pads through SDL3's gamepad layer
(`crates/render/src/pad.rs`), whose buttons are by place (south is A on an Xbox pad).

The engine's pad object (vtable `0x140978700`) keeps a byte per raw button at +0x190 + slot,
filled from XInput's button bits by the table at `0x140978720` (14 records `{i32 slot, u16
mask}`): slot 0 X, 1 Y, 2 A, 3 B (the PlayStation order: square, triangle, cross, circle), 4 LB,
5 RB, 6 and 7 the triggers (each past 30 of 255), 8 and 9 the stick clicks, 10 Back, 11 Start,
21 to 24 the D-pad up, right, down, left. The sticks are at +0x160 to +0x16c (each axis's top
byte, y negated). The DirectInput path (`0x1405d1280`) fills the same slots from its button
bits.

Above that the game asks for its own numbered buttons (`0x1404d3d70 (button, ...)`; the
menus' cursor asks `0x1404d3960 (0 to 6)`, which passes on 14 and up). A game button reads the
slot in the table at `0x14095a040`: 0 X, 1 A, 2 B, 3 Y, 4 LB, 5 LT, 6 L3, 7 RB, 8 RT, 9 R3,
10 Back, 11 Start, 12 to 15 the D-pad left, right, up, down, 16 Y, 17 A, 18 LB, 19 L3. Before
the table, buttons 1 and 2 trade places unless `Setting.ini`'s `[Pad] AB_SET` is set (the byte
`0x140de5ea0`, read at start-up by `0x1404d3770`; 0 without the file, as on a fresh install:
read live), and 17 and 18 always read buttons 1 and 2's places. So by default game button 1 (the
field's jump) is B and 2 is A. The game's settings tool (`Atelier_Escha_and_LogyEnv.exe`)
writes `Setting.ini` (`[Pad] AB_SET`, the keyboard's `KPAD_1` to `KPAD_7`, `[Input]
MOUSE_ENABLE`).

What each button does, read off the original on a fresh install with an Xbox 360 pad (the
escha-pad-buttons recording, 2026-11-14) and the button legends the game draws:

| Where | Button | Does | Barrel |
|---|---|---|---|
| Title | any | leaves "Press any button" | yes |
| Menus, lists, messages | A / B / D-pad | confirm / back / move; a message window goes on with A or B | yes |
| Field | left stick | walk (the game's 0.2 dead zone) | yes |
| Field | right stick | turn the camera | yes |
| Field | A | gather, talk | yes |
| Field | B | jump (game button 1) | yes |
| Field | X | strike | yes |
| Field | Y | the main menu | yes |
| Field | L3 | "Run On" / off (game button 6) | yes |
| Field | R3 | the camera back behind | yes (not checked in the game) |
| Field | D-pad | "Search Equip" | not yet |
| Field | Menu (Start) | "Requests" | not yet |
| Field | View (Back) | "Message Log" | not yet |
| Field | LB | "Assignment" (the main menu's page alone; B goes back to play) | yes |
| Field | RB | "Leave Map": onto the area map at the field's point (the town: the world map), as its exit | yes |
| Field | LT, RT | nothing (held 1.5 s and 3 s: the camera stays; escha-triggers recording) | nothing |
| Area map | B | "Return" | yes |
| Area map | Y | the main menu (escha-areamap-buttons recording) | yes |
| Area map | LB | "Assignment": the page alone, B back to the map | yes |
| Area map | RB | "System": the Option list alone (Save, Load, Erase Save Data, Change BGM, Dressing Room, Options, Return to Title), B back to the map | yes (Barrel's list lacks Erase Save Data and Dressing Room) |
| Area map | Menu / View | "Requests" / "Message Log" | not yet |
| Area map | X | the panel: Gatherable Items ↔ Monsters (the sector's out enemies' groups, each monster once: `field_ai::sector_monsters`) | yes |
| Battle | D-pad left/right, A, B | the command row, confirm, back | yes |
| Battle | L3 | "Fast Forward": Off → ×1.5 → ×2 → Off (`0x141077da0`, speeds 1, 1.5, 2 at `0x140d772c0`), kept between battles | yes (F on the keyboard) |
| Battle | View | "Field Information" | not yet |
| Events | A or B | the next line | yes |
| Events | X | "Fast Forward" on/off (the legend's; the escha-event-buttons recording) | Barrel's auto lines in its place |
| Events | View | "Message Log" | not yet |
| Events | Start, Y, LB, RB | nothing seen | nothing |

The "not yet" rows wait for screens or systems Barrel does not have yet: the Message Log
(View in the field, events and the area map), the Requests page (Menu), the field's search
equipment (the D-pad), the pages' LB/RB tabs (the Assignment page's "Change Entry", with its
Bonus tab), the System list's Erase Save Data and Dressing Room rows, and the battle's Fast
Forward (L3) and Field Information (View), which come with the battle (`docs/escha/loop.md`,
stage 17). Each gets its button as it comes; the table says which.

Barrel reads the pad through SDL3 by place: A, B and the D-pad stand for Enter, Backspace and
the arrows on every screen (`render::window::pad_keys`); the field and events take the other
buttons as pad buttons by what is on screen (`Pump::take_input`). The D-pad no longer walks
and the shoulders no longer move the camera: the game uses them for other things.

## Walking (not yet the game's)

A character walks against its field's collision mesh (`<AREA>_col.g1co`, `docs/formats.md`,
"Collision") through `game::walk`: an upright cylinder pushed back out of the mesh's upright
triangles (seen from above as segments, found through the hierarchy's leaves), the move cut into
steps of at most half the radius so none passes through a wall; a move into a wall slides along
it (`barrel walk FIELD_DIR X Y Z DX DZ STEPS`: in TOWN_10 a walk north from (6000, 10500) stops
at the wall near z = 11,347 and slides along it). The game moves characters through ktgl's
collision engine (`elixir::EXCollisionScene` over ktgl's broad and narrow phases, `S_CE_BNP_INPUT`
and `_OUTPUT`), which is not read. The character's size is its `CharaBase.xml` row's: the radius its `FMrad`
(0.48 for the player: 48 units, as the field enemies' scripts take it), the height its `FMhig`
(1.5: 150 units); the steepest slope it walks, degrees, is the larger of its `FMslp` (75 for
the player) and the field's (75 in nearly all, `docs/shading.md`, "The distant view"), all
three handed to the collision engine by `0x14026c3c0` (`game::walk::walkable_slope`; the
lead's partner gets 0.4 of its radius and a height of 200). Here a collision triangle steeper
than that is a wall, the others floors. The steps, the step-up height and how corners and slopes are taken are this module's,
to be replaced by the game's once read. The ground (`game::walk::Ground`) is the highest of the
heightmap (`<AREA>.khm`, bilinear between its vertices) and the collision mesh's floors (its
triangles that are not upright) under the character and no more than a step (50 here) above its
feet; with nothing under it the walk does not move (`barrel walk` across AREA_01_01 from (3000,
3000) eastward goes over the valley floor between heights 0 and 46). The floors are kept in
square cells of 256 units on the ground, each in every cell it reaches, so a height looks only
at its own cell's floors. Ryza's walk asks `height_on_ball` instead: on a floor tilted up to 30
degrees it stands the character a ball's rest higher (`radius * (1 / cos - 1)`, 1.94 on 21
degrees for 28), as the game does (docs/ryza/open.md, "Ryza in the field"); Escha's walk
does not.

The player runs at 500 units a second, 625 once `EVENT_XG05_110` (event 734) has been seen
(`game::walk::player_run_speed`): the user brain (`clsFMBrainUser`) reads it from `0x140234910`,
a test of that event's record and the constants 500 and 625; it does not override the brains'
`getMoveSpeed` (the base one returns 1). A field as walked is `game::walk::Field` (its walls, ground and gimmicks, the map's
from the executable's table); a walk that steps into a door (`FIELDMAP_CHANGE_EX`) goes on in
the map it names, standing where it says (`Gimmick::destination`, metres): `barrel walk` west
out of the Atelier (`TOWN_01`, from (600, 1000)) comes out in Marion's office (`TOWN_02`) at
(820, 1050) and walks on into the room. What the game shows between (a fade, the loading) is
not done here. Its move (`0x140234950`, `game::walk::player_move`) takes the stick
(`0x1404d33c0`, its second axis negated): leaning less than 0.2 it is nothing, else along the
stick at that speed times the lean (at most 1) times the frame's time. Pad button 6 is the run toggle
("Run On" / "Run Off", the global `0x141078088`, off at the start): on, the move is half as
fast again (× 1.5: 750 units a second at full lean). Her motion follows how fast she went this frame (`0x140235410`,
`game::walk::player_gait`): `MOTION_RUN` (33) from 0.42 of the run speed, `MOTION_WALK` (32)
below it, waiting when still, blended in over 0.15 seconds and played at the speed times
0.00225 (never slower than normal: 1.125 at 500). The viewer does the same: a new motion starts
from its beginning and is mixed in over 0.15 seconds while the old one plays on (joint by joint,
rotations by slerp). The user brain's button 1 starts an action in
`MOTION_JUMP` (49; the player's `JUMP` column, `PC20_ACT_JUMP`) played at 1.5 with sound 0x23
(`0x140234cbf` and on): a jump. Each one may come with a voice: a chance, 5 at first
(`0x14023429f`), is rolled; a hit plays a clip from 11315 to 11317 for Escha or 11318 to 11320 for
Logy and sets the chance back to 5, a miss raises it by 10 (`game::walk::JumpVoice`). The jump is
timed at +0x124 (and +0x128, against 1.7 s); on landing (`0x140234e12`) the motion turns to 4
with a landing sound picked by the ground under her (the map's 64-byte record at the field's
+0x10e8, its word +0x10 a material from 0 to 9, whose sound is the first word of a 16-byte entry
at +0x2f68). The motion carries a rise of its own: in `PC20_ACT_JUMP` (1.93 s) her
hips (bone 4; the root and the next two stay at the origin) go from 88 units up to 111 by 0.5 s,
hold near 95 from 1.0 to 1.5 s (the pose in the air), drop to a 52-unit crouch at 1.7 s and are
back at 88 at the end (`barrel g1a FILE#N 4` samples a bone). The code lifts her too: the jump runs in
phases (+0x138: 0 the take-off, 1 in the air, 4 the landing), and going into the air
(`0x140234f13`) plays the motion's next division and gives the character an upward speed (its
+0x150 velocity set to (0, 5, 0) metres a second, or (0, 25, 0) when its +0x12d is set). Each frame
(`0x14026bc80`) a character moves by v·dt − a·dt²/2 (the height × 100 into field units) and its
velocity drops by a·dt, a being its +0x160 acceleration, whose upward part is the gravity, 14.71
m/s² (`0x14026b9c9`). So a jump peaks 0.85 m (85 units) up after 0.34 s and is back down after
0.68 s (`game::walk::jump_height`). The phases follow the motion's divisions (`CharaDiv.xml`, `PC20` `JUMP`: 0 from 0 to 0.1 s, 1
to 0.6, 2 to 1.1, 3 from 1.1 to 1.5 looping, 4 from 1.5 to the end): the take-off plays division
0 and launches her when it is done (`0x140234f13`, which waits for the motion); in the air the
motion runs on into division 3's loop; she lands as soon as she is on the ground again
(`0x14026c550`: the character's ground flag, +0x12e, or always while its +0x128 is clear), which
plays division 4, zeroes her velocity and sounds the ground (`0x140234e05`). The viewer jumps so
(J in a window, `BARREL_JUMP_AT=FRAME` headless), on the flat arc of `jump_height` against the
ground she left; her gait takes over when the landing ends. In a window it sounds as the game's
does: effect 0x23 at 0.6 and, when `JumpVoice` says so, one of Escha's three jump voices.

## The session's calendar

A session in free play carries the calendar (`days::World` over `days::Data`, the tables
`barrel days` reads), and the story's date follows it: field time (`Session::spend`, in tenths
of a day: a gathering's `days::gather_tenths`, a battle's time) passes whole days and carries
the rest; leaving a field area (map 12 or above) for another map with time carried passes one
day and drops the rest (`World::leave_field`); town maps carry none. Each new day runs the
calendar's work (board, month reports, rank) and the shops restock. The session keeps no items
yet, so the board's tables see no held items, every request as known and no assignment cleared.
The main random generator is not saved by the game either; a fixed seed stands in. Saves keep
the carried time (`tenths N`).

## Arriving in a field

The player arrives at the field's record in the executable's arrival table (`world::arrivals`,
`docs/tables.md`, "Arrival places"): a place and a facing per field map. Found from two live
reads (2026-11-13, from the Southern Ruins' area map): Sector One (map 26) put Escha at
(53.32, 0, 102.80) m, its record exactly, and the Ruins Entrance (map 25) at (129.08, 0, 200)
against its record's (129, 0, 200) (likely the ground settling her). Both are about 3.5 m from the
field's exit, which is why the old stand-in (standing in the exit) was close. The viewer's world
map trips use it (a field without a record falls back to the exit), facing the record's way
(degrees, as `atan2(x, z)`, the events' convention: Sector One's 180 has Escha's back to its
camera, which looks along -z, as the running game showed). That world map trips read the same table as the area
map is assumed, not seen.

## Play camera follow

Read from the running game (2026-11-13: the user's save, Southern Ruins Sector One, map 26,
`AREA_03_02`, whose one area is `CLOSELY_X` with points at x 42, 52 and 62 m, lengths 7.5, 6 and
8 m, `bumper_follow_speed` 5 and `zoom_distance` 0).
The play camera (`CameraCFieldMap`, vtable `0x14080c428`: its +0x88 and +0xd8 are the known
`0x1401ec650` and `0x1401e6e10`; one object, found by scanning for that pointer) was dumped
every 0.1 s while Escha stood, ran 1.5 s and stopped:

| Offset | What |
|---|---|
| +0x18, +0x30, ... +0x1c0 | tweens, each `{vtable 0x140725e10, current, from, to, ...}` |
| +0x98 (the +0x90 tween) | the setting's length: 624 at x 53.2 m and 680.6 at x 56.0 m, exactly `play_camera::setting`'s blend (6.24 and 6.806 m); in `AREA_03_01` (`CLOSELY_Z`) at z 200 m, 1008.3 against the blend's 10.083 m. So the blend along the area's axis is the game's |
| +0xb0 | the player's position one frame earlier |
| +0xc0, +0xd0 | the player's position |
| +0x188 | the look point: the player's position plus `diffPosition` (y + 100 here) |
| +0x158 | 5.0, `bumper_follow_speed` |
| +0x19c | seconds since the player last started or stopped moving |
| +0x1a0 | the move zoom blend: 0 to 1 at 1 / `zoom_time` a second while moving |

So the look point does not trail the player (`play_camera::Follow` keeps it on her; it used to
trail at `bumper_follow_speed` per second, a guess from `_def.xml`). Not seen yet: what the
bumper numbers change, the camera's eye (not in the object's first 4 KB), and how the zoom
blend comes back after a stop (it fell at the same rate once, stayed at 1 the other time; Barrel
brings it back at the same rate). The settings still switch at once between camera areas.

## Ryza DX: the random generator

Every random number of Ryza DX comes from one `std::mt19937` (MSVC's layout: a u32 index, then a
sliding window of 1,248 words) at `0x141b74600`: seeded 5489 statically (`0x1400114e0`) and reseeded
once at start-up from the clock (`0x1406ae440`, `time(0)`). Under ryza-recomp's virtual clock the
seed is 1767225600; the generator state dumped at frame 2040 of `gather-rasen` equals Python's
`mt19937(1767225600)` after 7,306 draws (the window index 0x1ba = (624 + 7306) mod 1248; the
current block is the first half of the window up to index 624, the second half from there).
Everything goes through `rand15` (`0x1406ae6b0`, under a lock: `_Rng_from_urng` over the 32-bit
engine with the range 0x8000, which is the low 15 bits of one output and never redraws) and its
wrappers: `irand(a, b)` (`0x1406ae5e0`: either order, equal bounds give the number with no draw,
else `low + draw % (high - low + 1)`), `frand(a, b)` (`0x1406ae340`: `draw * (1/32767)` as a double
rounded to float, times `high + 1e-4 - low`, plus `low`, all in float; equal bounds no draw), and
`0x1406ae640` (`draw % n + 1`), `0x1406ae680` (`draw % n`), `0x1406ae410` (the float 0 to 1),
`0x1406ae3d0` (it times `a`), `0x1406ae2e0` (`draw * 2a - (a + 1e-4)`). `barrel_ryza::random`
has all of them, and the window index (`Random::index`), the game's own number in its hook logs.

Barrel seeds from the clock too. `BARREL_RANDOM_STATE=FILE` (a dump of the game's window, ryza-recomp's
`dumps/F.1.bin`) starts a run from the game's state instead, for the lockstep checks only
(`docs/ryza/parity.md`, "gather"). Free play draws on its own from 2040 on (`frand(0, 15)` every
20 to 60 frames from `0x14008e290`, some field timer); Barrel does not make those draws yet, so
its index runs behind the game's after the first gathering.

## Movies

Eight movies play at the openings, chapter breaks and endings (`docs/events.md`, "Movies":
`u46`), each an ASF (`.wmv`) of VC-1 Advanced profile video (1920 × 1080, 29.97 or 59.94
frames a second) and WMA 9 Professional audio (48 kHz stereo). Chosen 2026-11-14: native
decoders in Rust on the CPU (`barrel-movie`, and `barrel-wma` under the LGPL: `docs/formats.md`,
"Movies"); FFmpeg is only a PC test oracle, never linked or shipped. Checked against it:
`opening` (2,818 pictures), `avantitle` (2,883), `opening_escha` (2,584) and `opening_logy`
(1,871) bit-exact, their sound within 135 dB. A movie's file is looked up in `Movie_EN` then
`Movie` (the openings are only in `Movie`). The boot plays movie 0, `opening`, after the logos:
the escha-recomp's file trace shows the game asking for `Movie_EN/opening.wmv` there
(`avantitle` is the chapter 3 event's, `docs/events.md`; Barrel played it at the boot by
mistake for a day). `u46` in the events does not play its movie yet.

## Deliberate differences

(Ryza's graphics settings apply live from the Settings screen's Yes: `Renderer::apply_settings`, docs/renderer.md "Rebuilding for new settings".)

- **Ryza's field menu opens on Tab or the pad's Y in a window.** The game's keyboard map is not read;
  the arrows move, Enter chooses, Backspace or Escape closes (as the title's keys), and the field
  stands still under the menu (the walk takes no sticks). Offscreen the script's pad frames
  (`--menu-keys`) drive it.
  On a character page (Equipment, from A on a card) up and down move the slot cursor along its two rows of two, left
  and right along a row, and Q and E (the pad's shoulder buttons; offscreen `--menu-keys` `lt` and `rt`, also `lb`
  and `rb`) the previous and next character round the party: Barrel's choice, the game's own keys on the PC are not read.

- **Ryza's arrival after the opening shows the autosave mark and the story marquee, and writes no save.** The game saves as the free play begins
  and tells that an entry was added to the story (docs/ryza/open.md, "The opening (P5b)", part 4); Barrel draws both (the spinner at the top left, the
  marquee's scrolling text) but, as everywhere, never writes the original's saves. The story entry named is the first one (the game takes it from the
  story's state).

- **Ryza's run loop enters at a fitted phase.** When her run motion takes over from the start
  motion it begins 0.35 s into its loop (`player::RUN_LOOP_PHASE`, a stand-in: the game's
  hand-over is not read); with 0 the pelvis's cloth pins were 3.1 cm off the game's over the
  run, with 0.35 1.0 cm. Her position does not depend on it (she moves at the lean's speed).
  Lockstep checks on her walk see the pose of the run differ in phase, not the path.

- **Ryza's volume and camera-speed rows follow Barrel's curves.** The Audio rows' volumes step in
  decibels about the default (`preferences::volume_gain`: 6 dB a step below row 5, 1.2 above, row 0
  silent) and the Camera rows' speeds are linear in the stick's rate (`walk::Feel::of`), stand-ins:
  the game's curves are not read. At the defaults both are exactly as before.

- **Ryza's Fullscreen is borderless.** The Window Settings row's Borderless and Fullscreen both open a
  borderless window on the window's screen (the saved `fullscreen on`); the Screen Resolution row sizes the
  window only. Frame Rate Limit sleeps the loop to its interval (30, 60, 120, 144 or none) before each present;
  Vertical Sync off presents in MAILBOX (else IMMEDIATE) without holding the loop to the refresh. As the game,
  a window starts at its default of 60 frames with vsync on (also with no config); Unlimited or 120/144 on
  the row lift it at once.

- **The Options screen's second page.** The game's Options screen (layout 511) has one page
  (volumes, Fast Forward Event, Voice Language, Default, Confirm); Barrel adds a second, the
  graphics page (`play/options.rs`, `docs/renderer.md`, "Graphics settings"). Down off "Confirm"
  or Up off the first row turns between them (the game's screens have no shoulder paging). The
  game's page opens and draws as the game's does. Its values are kept in `options.conf` beside
  the graphics config; BGM, SE and Voice set the mixer's music, effect and voice volumes
  (ambience counts as an effect); the other volumes, Fast Forward Event and Voice Language are
  kept but nothing reads them yet.

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

- **The title's pick never runs past its range.** Ryza DX's `u104` maps a number from 0 to
  32767 onto its range in equal shares and, once in 32768 draws, lands one past the highest
  (the title would then play no action that round). Barrel keeps it at the highest
  (`ryza::event::clock_pick`).
- **Recipe effect flags.** When an item gains an effect that is not one of its recipe's, the
  original's lookup comes back -1 and it marks the previous recipe's tenth effect as revealed
  (one byte before the right recipe's flags, `0x140367ad0`). That is corruption of another
  recipe's state, so Barrel sets no flag then (`library::reveal_effect`). Lockstep checks of
  `mix_recipe` skip that byte.
- **Text window between lines (stand-in).** After a line goes down Barrel keeps the text
  window up, empty, for 0.6 s of event time (`WINDOW_GRACE` in `crates/render/src/main.rs`,
  `Stage::window`), so it does not close for a frame between the lines of a conversation; how
  the original keeps its window between lines is not read. Presentation only: no event state
  depends on it.
- **Cloth: the parent's move is exact.** The cloth solver (`crates/engine/src/cloth.rs`)
  takes `D = old parent x inverse(new parent)` in f64; the original's float32 product carries
  about 0.004 of noise at world coordinates near 32,000 (the whole-frame check against the
  captured skirt is within 0.004 in position, 0.18 in velocity, for that reason).
- **Cloth starts settled.** At a scene's first frame a cloth piece is placed at rest and
  stepped 120 times on that pose (`play/cloth.rs`, `SETTLE_STEPS`), so it never opens falling
  from its rest shape; the original's has been running since the model was loaded. A frame
  catches up at most 4 steps of 1/60 s.
- **Cloth leaves out** the parent's rotation limit (`rotation_limit`, the skirt has none), the
  acceleration smoothing's finer filter (the clamps of the move's acceleration are applied
  to the plain acceleration against the smoothed velocity) and, for now, every cloth but the
  player's (the title's and the townsfolk's characters hang at rest). The wind's strength is
  the plants' `Gust::at` stand-in times `clothWindScale`.
- **Depth of field: stand-ins** (Ryza, `docs/renderer.md`, "Depth of field"). The weight's
  curve is a table read off the capture (`dof::CURVE_BASE`, `CURVE_TAIL`), not what the game's
  CPU builds, so a field with other focus settings softens as the gate's. The weight is made at
  the scene's size from the occlusion's half-size depth (nearest, no full-size depth), so its
  steps follow the half-size depth's edges. With MSAA above 1, or a frame whose stage dump
  stops before the finished scene, there is none. The weight, layers and blur are half-float
  at the scene's size and a quarter of it (the game's: the same at 1920 by 1080).
- **Reflections: stand-ins** (Ryza, `docs/renderer.md`, "Screen-space reflections"). The game's
  scene colour the march reads has no specular ambient; Barrel's holds the cube light (the
  models add it before the reflections exist), so a reflected surface is a little brighter.
  The march reads the occlusion's half-size normals (the game: the full-size target). The terrain
  and the plants take no reflections (their specular ambient is the cube light alone). With
  MSAA above 1 the reflections are left out (the extra targets are single-sampled), and a
  frame whose stage dump stops before the finished scene has none.
- **FXAA: a stand-in** (Ryza). The game's Middle preset (and a Custom setting with
  `GRAPHICS_ANTIAILIASING=1`) uses FXAA; Barrel has none and draws no anti-aliasing there
  (`presets::Graphics::apply`). The Low preset's none and High's TAA are as the game's.
- **Ryza's graphics settings** (`docs/formats.md`, "Ryza DX: the graphics presets"): the
  shadow quality item is stored and changes nothing (the PC game's maps are the same size at
  every level, and Barrel keeps those); light shafts are not built (so the item switches
  nothing); point lights, water quality and effects quality are stored only. The config's old
  `foliage` key no longer picks Ryza's plants (the game's grass items do). Without any of the
  game's settings given, Barrel's own `--preset` and config choices (TAA, anisotropy, and the
  occlusion, reflections and depth of field switches) stand, and everything else is High.
- **The shadow casters' slope bias** (Ryza; `docs/shading.md`, "The casters' pass"): the game
  writes the depth plus its slope scale times |ddx| + |ddy|; Barrel's raster depth bias takes
  the larger of the two (or their length), so a few shadow edges sit a texel off. It lets the
  depth test run before the pixel program, which halves the pass's cost.
- **The reflections' per-pixel inputs at 8 bits** (Ryza; `reflect::TARGET_FORMATS`): the
  scene pass leaves the reflections' factor and gloss as sRGB 8-bit and the cube's light as
  B10G11R11 where the game keeps 16-bit floats, so a handheld's tiler holds 8 bytes a pixel less
  (fewer bins). At the Rasenboden gate 1.2% of the pixels move, all but 5 by at most 4 levels
  (the most 7); every check scores the same.
- **The Ryza Settings screen's leave and its values** (`crates/ryza/src/options.rs`, `leave.rs`):
  Escape or B in a window asks "Do you want to apply the changes?" when something changed
  (as the game's, read off its captures), then writes the changed rows to Barrel's graphics config
  (`option_<key> N` lines and, once a graphics row was changed, the game's graphics keys) on Yes and drops
  them on No; the graphics apply on the next start, not live. Game, Camera and Audio values are kept and
  read by nothing yet. The tabs turn with Q and E in a window; Keyboard and Controller assign nothing.
- **Barrel's own rows on the Ryza Settings screen's Graphics tab** (the PC release sets such options in
  a launcher, which Barrel does not have; the user chose them woven into the game's page,
  `docs/ryza/open.md`, "Barrel's own rows"): Render Scale, Upscaler and Sharpening between Window
  Settings and Frame Rate Limit, and the game's Anti-Aliasing and Texture Filtering rows extended
  with MSAA 2x, 4x, 8x and FSR native AA, and anisotropic 2x, 4x, 8x and 16x. A choice that conflicts
  with another is closed, as on Escha's page (`barrel_render::graphics_options`), and the description
  panel names it with the reason in red. They are not in the game, so lockstep and parity runs of the
  Graphics tab pass `--settings-as-game`, which lists the game's 22 rows and its own choices only. The
  game's Anisotropic filtering keeps the count the config's `anisotropy` holds (the game's is 16x).

- **Ryza's menu stick.** The left stick steps menus (title, load list, Settings) once it leans
  past 0.4 and stops being a direction under 0.2257 (the field walk's dead zone), so a stick
  resting near one threshold does not retrigger. The game's own thresholds are not read.
  `BARREL_PAD_SCRIPT` (a scripted pad, `crates/render/src/pad.rs`) is a test aid, not a game
  feature.

- **The occlusion noise moves under a temporal resolve** (Ryza; `ao.frag` `noise`,
  `ao::View::temporal`). The game's 4 by 4 noise is filled at run time and not read, so a Bayer
  matrix stands in; held still it shows as a fine grid of dots on skin close up (the user saw it
  on her neck, 2026-11-21; gone with a flat noise). With TAA, FSR 3 or GSR 2 on, the pattern
  moves one texel a frame through its 16 places and the resolve averages it away; without one,
  and in single-frame offscreen pictures, it stays put (frame 0), so checks are unchanged.

## Sound

`barrel-audio` keeps sound apart from the renderer. `Mixer` adds the playing sounds (each a
`Sound`: samples per channel, decoded natively) into one interleaved stereo float stream at
48 kHz, with a gain per sound, looping, and linear stepping for sounds at another rate; it is
plain code, tested without a device. `Output` is an SDL3 device stream in the same format: the
engine calls `pump(&mut mixer)` from its frame loop, which mixes just enough to keep about a
set time queued (a tenth of a second in the test), so the sound runs on the device's clock
while the logic keeps its fixed tick.

**One output for the run.** The viewer opens the output and mixer once and keeps them across
scenes: when one event hands over to the next, or ends into free play, the old scene's music
and field ambience stop, but a voice or effect still sounding plays to its end. (Opening a new
output per scene dropped them mid-sound: the user's "audio during scenes cuts short".) Whether
the game lets a scene's last sounds ring on into the next is not read.

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

**To a file.** `barrel ... --event FILE.ebd --audio OUT.wav` (offscreen) mixes the event's
sound into a `Capture` instead of a device: at each frame exactly up to the event's clock (from
`--at`), so the WAV (16-bit stereo, 48 kHz) lines up with the frames written, run after run.
It is written when the run ends. The reference recording of the opening carries the game's own
mix (`tools/oracle/README.md`, "Recording"), so the two can be set side by side:
`tools/oracle/clip-fit.py MIX.wav CLIP.wav` finds a clip in a mix and its gain; `tools/oracle/audio-offset.py GAME.wav BARREL.wav` cross-correlates their loudness envelopes
(10 ms steps) for the offset and prints the levels second by second. First look, event 111's
first 20 s: the recording starts 4.87 s into the event (clip 12900 at 1.635 s there, 6.5 s in
Barrel), so Barrel's silent first 4 s (music 57 starts at 4.0 s) are before it; the music then
lines up (Barrel's 5.8 s found at 0.930 s in the recording, 0.935 expected). The game's lines
wait for a button, so the two timelines only line up line by line.

**Levels.** A voice clip (`barrel voice-wav`) fitted into each track (its offset by
cross-correlation, its gain by least squares: the music under it does not correlate) gives the
level each mix plays it at, and what is left over the music and effects. The game played
clips 12900 and 12901 at 0.80 and 0.79 of the clip (both tracks taken to mono by FFmpeg's
downmix, which scales a centred sound by √2); Barrel, at gain 1, 1.40. So the game's voice
lines are 0.57 of Barrel's: 0.8 (`OPTION_VOLUME`, the default options' level, by
measurement: not yet read from the game's option table) times 1/√2 (`VOICE_GAIN`, a mono clip
put in the middle at constant power), and its music and effects about 2 dB down (the same
0.8). The viewer's mixers start at that level (`Mixer::game`) and voice lines play at
`VOICE_GAIN`: then 0.79 and 0.79 for the voices, -21.3 and -20.0 dB left over (the game
-21.0 and -20.9), -21.2 dB overall against -20.9, no clipping (peak -3.1 dB; it hit full scale
before). The effects' own level is not measured apart.

## World map travel

`game::travel::WorldMap` holds the world map's routes (`barrel_formats::world::areas`, their
ends as the routes' dots lie: `docs/tables.md`, "The world map's field") and finds the fewest
days between two marks (`trip`, the paths inside a region taking none, each route its
`trip_days` for the events seen). `barrel travel EXE FROM TO` prints the trip and the fields its
end opens: from the town (`T0`) to the Twilight Forest's third point (`A0p2`) 4 days by `R0` and
the forest's paths, opening `AREA_01_03`; from the town to `D2` 6 days by `R3` and `R6`. The game
lets the player pick each route rather than finding the shortest; this search is for trying
trips, and the days it adds are what the story's travel trigger takes: `Chain::travel` passes the
trip's days (the shorter trips counting by the events the story has seen), fires the travel
trigger once, and if nothing starts on the road walks the player from the world map into the
field the end point opens; `barrel event-chain ROOT START trip:T0>A0p0` takes it after the
opening (4 days, nothing starts). `WorldMap::path` lays the trip out as
the routes' dots in order (`world::route_path`, each next dot the nearest left, from the end
at the mark the trip is at) between the marks; `barrel --field .../A15_WMAP OUT
--world-trip T0 A0p0` has Escha run that way at her run speed (13 waypoints: from the town past
`R0`'s marks, through the forest's path dots, onto `A0p0`'s mark), the play camera following.
How the game itself moves the player along a route (its speed, the marker, the days ticking
by), and what happens on the way, is not read.
