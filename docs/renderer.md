# Renderer

`crates/render` draws the games' data with Vulkan 1.1 through `ash`. For now it is a single
tool, `barrel-view`, that renders one model offscreen and writes the frame as PNG.

`barrel-view --field FIELD_DIR out.png` draws a field's objects: every instance in the
field's placement list, with its model from the `_Obj` container and the field's shared
texture pack (the object models carry no textures of their own). Each instance is scaled,
rotated (x, then y, then z, in radians) and moved on the CPU. AREA_01_01 places 112
objects (126 draws, 52,089 triangles), and they come out where they belong: a ring of
cliffs around a valley of trees, rocks and flowers. Under them it draws the
terrain from the field's `.ktf`: the heightmap as a grid, shaded by `terrain.frag`, which
reads the two blend maps across the whole field (UV 0..1) and uses their six channels
(map 0 RGB, map 1 RGB) as weights for the first six ground layers, each tiled at the scale
its layer record gives (AREA_01_01: 300, 300, 400, 300, 200 and 100 units a repeat). In
AREA_01_01 that gives sand on the open ground and dirt and grass in the valley under the
trees. Still to check against the game: the blend maps' orientation, and fields with more
than six layers, where the settings block must say which layers a map's channels weight.

Options: `--size N` (64..1280), `--yaw DEG`, `--pitch DEG` (45 by default for fields),
`--no-alpha` to turn off the alpha test. `BARREL_DRY=1` builds the scene and prints its
bounds without touching the GPU.

What it does today:
- uploads the model's submeshes (positions, normals, UVs from `g1m::Geometry::simple_mesh`)
  and the texture pack's BC1/BC3 textures as they are, with all mip levels;
- picks each submesh's texture from its material (the first texture of type 1, else the
  first texture);
- draws with depth testing, no culling, and an alpha test at 0.3 for cut-out textures
  (hair, lace);
- places a perspective camera on the model's bounds, in front of it.

Conventions learned from the data:
- The games are DirectX, left-handed. The renderer mirrors x on the vertices as it builds
  the scene (not in the projection, so the camera is placed on the mirrored scene), and
  models face +z.
- Texture rows are top to bottom, as Vulkan expects; UVs are used as they are.

**Posing.** `--skeleton FILE#N --anim FILE#N --time SECONDS` poses a character: the
skeleton comes from the base model (`PC20_MODEL.elixir.gz#0` for Escha), the animation from
a motion container, and the vertices are skinned on the CPU. Each joint's matrix is its
posed world transform times the inverse of its bind world transform; joints without a
track keep their bind pose; a vertex blends up to four joints by its weights (a missing
last weight is 1 minus the others). Escha at 2 s into `PC20_STAND_HAPPY` stands with a hand
to her cheek and the other at her side, the mesh whole.

**Sophie 2 characters.** The model container holds the meshes (`PC01A_MODEL_default.g1m`
is the body, `PC01A_MODEL.g1m` the skeleton), the texture pack is a loose `.g1t` beside it
(the viewer takes a plain path as well as `FILE#N`), and the motions are in
`pc01a_motion.elixir.gz`. Most joints' tracks there are rotation only (flag 2); they keep
the joint's bind translation and scale. `pc01a` at 1 s into `PC01A_000900` stands with both
arms raised, the mesh whole.

**Faces.** `--morph FILE#N --face FILE#N [--face FILE#N ...]` applies facial animations:
the morph file is the model's `.g1h` (`PC20_MODEL.elixir.gz#15` for Escha), each `--face` a
brow, eye or mouth animation from the `_FACIAL` container. Each animation goes to the morph
set with one target fewer than it has tracks, its weights are taken at `--time`, and the
blended offsets are added to that vertex layout's positions before skinning (see
`formats.md`, "Morph targets"). Escha with `PC20_EYE_M_ANG_M_*` at 2 s opens her mouth and
moves her brows and eyes.

**Expressions at a line.** With `--event`, `--line N` gives each placed character the
expression the event's messages have set by line N (each line's fourth field; see
`formats.md`, "Event messages"), with line N's speaker talking. The names come from the
executable's facial tables (`barrel_game::face::Tables`, read from the install's own
executable), the animations from the character's `Facial` container, the shapes from its
model's `.g1h`. In `MM08_AS4_130` at line 5 Escha speaks with the sad talking mouth
(`PC20_MTH_M_SAD_M_Mth_B_Def`), Logy keeps a closed sad mouth, Threia and Reyfer the default.
The characters also take their latest gesture (each line's third field) as their pose:
the gesture's main motion from `barrel_game::gesture` (the table found in the executable by
its first entry), its `CharaBase` column, then `<FileHead>_<value>.g1a` from the `Motion1`
container or its own one. At line 5 Escha is in `STAND_NORMAL_2`, Logy `STAND_NORMAL`,
Threia `STAND_YES_2` and Reyfer `STAND_ANGRY`; characters without a gesture keep `F_WAIT`.

`--look-at N` replaces the event camera with a close-up of character N: aimed 140 units
above where the event placed them, 160 units away, from the direction they face (yaw 180°
minus their angle), field of view 30°. Escha at line 5 of `MM08_AS4_130` stands in front of
the church doors with her hands clasped, green-eyed, mouth set for the sad line.

**Eyes.** An eye material has four textures, none of subtype 1 (Escha's materials 3 and 4):
the eye white on a skin-coloured ground, the iris with alpha, a green glow and the highlight
outlines, all on the same UVs. The viewer lays the iris over the white by its alpha into a
new texture when it loads them, which gives her green eyes; the glow and highlights, and
whatever the game's shader does to move the iris, are left out for now.

**Event openings.** `--event FILE.ebd` with `--field` adds the characters an event
places (see `events.md`) with their own texture packs; the scene's draws index one list of
textures gathered from every pack. The camera then centres on the characters.

Not done yet: skinning on the GPU, the games' own shaders and
lighting (toon shading, rim light), materials beyond the first texture, a window and
interactive camera, and fields.

Every run is one frame and exits; set `BARREL_VALIDATION=1` to run with the Khronos
validation layer, which Escha's model and an item pass without errors.

**Instances.** Every draw names an instance: a model matrix in a storage buffer (set 1,
binding 0) that `model.vert` reads by the draw's instance number (a push constant at offset
112, after the camera's `view_proj` and the shared parameters). Instance 0 is the identity,
for geometry baked in place (the field's objects and terrain); each event character gets an
instance of its own and keeps its vertices in model space, so it moves by changing its
matrix, not its vertices. The vertices are mirrored in x as they are built, so a game-space
placement T becomes S·T·S (S the x mirror) in render space, still a rotation and a
translation, which is why the shader turns normals by the matrix's upper 3×3. This is the
base for playing events in time: per frame, the blended stage snapshot
(`docs/engine.md`, "Timing") only rewrites the characters' matrices. Skinning is on the GPU: each vertex
carries up to four joints (u16, indices into its model's skeleton) and weights (unorm8,
renormalised to their sum), and each instance says where its joints' matrices begin in a
second storage buffer, the palette (set 1, binding 1), held in render space as S·M·S like
placements. Vertices with all weights 0 (the field) are not skinned. So a new pose is a new
palette, a new place a new instance matrix; the vertex buffer never changes. Checked against
the CPU-skinned frame of `EVENT_MM08_AS4_130`: 745 of 2,073,600 bytes differ by more than 8
(from the 8-bit weights), the same picture.

**Playing events.** `barrel-view --field DIR OUT.png --event FILE.ebd [--at S] --frames N
--fps HZ` builds the scene once, then plays the event on: an `event::Player` schedule in a
`stage::Playback` stepped by a `clock::Clock` fed 1/HZ a frame, each frame drawing the last
two ticks blended (`docs/engine.md`, "Timing"). Between frames only the characters'
instance matrices and joint palettes are rewritten; each character plays the motion the
stage gives it (`s5`/`s10`), loaded by its `MOTION_*` name from the character's `CharaBase`
row the first time it is needed and kept, from the time the stage started it (looping
motions wrap); a character the stage has given no motion keeps its field wait. A new motion
blends in over the time its command gives (`s10`'s third operand / 100, 0.4 s in
`EVENT_MM01_OP1_111`): each joint's local scale, rotation (slerp) and translation are mixed
from the old motion, which runs on from where it was, to the new one, then skinned
(`local_pose`, `mix_pose`, `skin`). With no blend (the plaza event) the frames are
pixel-identical to before; the blend itself is not yet checked by eye. Frames written to files are not real time,
so the viewer's clock has no stall cap (`Clock::max_frame`). Frames go to `OUT_000.png`,
`OUT_001.png`, ... In `EVENT_MM08_AS4_130` (`TOWN_04`), 4 frames at 2 Hz from 0.5 s show Logy
and Escha walking into the plaza towards Threia and Reyfer, mid-stride; 4 frames at 1 Hz from 1.5 s show them arriving and changing from `WALK` to
`F_WAIT` at 2.4 s, as the event says. Every character the event ever places is loaded up front; a character the stage does not
have at a frame (not placed yet, or removed by `q5`) is not drawn (its draws are skipped
by instance), so characters come and go as the event says (`EVENT_MM08_AS4_020`: Escha,
Logy and character 7 appear by 0.5 s, placed at 0.1 and 0.2 s). `--field auto` takes the field from the event (`docs/events.md`, "Which field"). A field
with one terrain blend map (`TOWN_03`) gets a black second map, so its layers 4 to 6 have no
weight. Faces change as the event goes: every character with a `Facial` container gets all its
morph sets, and each frame its weights are rebuilt from the stage (its latest expression and
whether it is talking, through `face::Tables`), brow and eyes held at 0.5 s into their
animations and a talking mouth moving through its animation in time (looping), a quiet one
held at its end. `BARREL_FACE_LOG=1` prints each character's face and weight sum per frame:
in `EVENT_MM08_AS4_130` Logy's mouth moves while he talks (weight sums 1.35, 1.15, 0.90) and
settles when Escha's line comes. A line's gesture (its third field, a `gesture::read` entry) plays its main motion on the
speaker from that line until the next motion command or gesture, blending in over 0.2 s;
how long the game holds a gesture, and the blend, are assumptions. The game holds each line until the
player presses a button (it has no auto-advance setting), so playback, which nobody
attends, holds each line for a reading time of its own (`event::reading_time`: 1 s plus
0.08 s a character; `Player::reading_time` turns it on, and both `--at` and `--frames` use
it). It is a viewer convenience, not game behaviour, so `play-event --timed` leaves it
off. In `EVENT_MM08_AS4_130` the gestures now come a line at a time, seconds apart. Not yet: the stage camera; `EVENT_MM08_AS4_110` in `TOWN_03` does not
look right yet: `TOWN_03` is a small round room near the field's origin (67 objects) over a
flat placeholder terrain (64×64 cells of 100, every height 0, one layer, drawn black), and
the event's camera sees only a large surface close in front, with no characters in view,
with or without the terrain (`--no-terrain` leaves it out). The field loader prints each
terrain's height range to tell such placeholders apart.

**Faces.** Facial morph targets are on the GPU too. Every target of a character's `g1h`
sets goes to a storage buffer of offsets (set 1, binding 2: each morphed vertex's offset in
each of its set's targets, one after another), and each instance has a block of target
weights (binding 3; the instance's `weights` says where it begins). A vertex carries where
its offsets begin and its target count and weight offset; the vertex shader adds the
weighted offsets (mirrored in x like the vertices) before skinning, as the CPU did. So a
face changes by writing its weights, like a pose by writing its palette. The weights start
from the line's expression (`--line`): in `EVENT_MM08_AS4_130`, line 2, 108 weights over three
faces, 3 not zero (the mouths), and the frame is pixel-identical to the CPU-morphed one.

**Live window.** `barrel-view --window FRAMES` opens a 1280×720 window (winit 0.30, Wayland or
X11) and presents through a FIFO swapchain (`present.rs`: the surface, the swapchain, acquire
and present; vsync, so the display paces the loop), with the engine clock taking each
frame's real time (`docs/engine.md`, "Timing"). It clears each frame to a colour that moves
with the logic time, a bounded smoke test of the window, swapchain and clock; drawing the
event's scene into it is next. Run inside headless gamescope (1280×720 at 30 Hz), 60
frames present and the clock runs 112 logic ticks: about 1.9 s at 60 ticks a second, the
display pacing the loop. `Gpu::new` takes the window's display handle to add the
surface and swapchain extensions.

**Events in the window.** `--window FRAMES` with `--field`/`--event` plays the event live:
the window is opened first (`window::Pump`, winit's events pumped once a frame without
blocking, so the frame loop stays the renderer's own), the device gets the surface and
swapchain extensions, the render pass targets the swapchain's format and ends in
`PRESENT_SRC`, there is one framebuffer per swapchain image (one depth image at the
window's size), and the projection takes the window's aspect. Each frame pumps events,
advances the clock by the real time since the last frame (with the stall cap, as a live
window should), updates the characters' matrices, poses and faces, then acquires an image,
draws, and presents with one frame in flight. Without a window the same code draws into the
offscreen image (pixel-identical to before). In headless gamescope at 30 Hz,
`EVENT_MM08_AS4_130 --at 0.5 --window 90` plays Escha and Logy walking in and changing to
`F_WAIT` at frame 61 (2.5 s), on the event's own time.

**Sound in the window.** A windowed event also plays its sound (frames written to files stay
silent): the stage turns the music and voice steps into cues (`stage::Cue`), and after each
tick the viewer takes them: a track starts at its listed volume times the event's, looping from
its loop point unless it plays once (`barrel_audio::Library::music`; music is decoded as it plays, see `docs/engine.md`, "Sound"), a stop or fade-out fades
the playing track (`Mixer::fade_out`), and voice clips play once (`Library::voice`, from
`Sound/Voice`). Every frame tops the SDL3 stream up to a tenth of a second. Test runs use
`SDL_AUDIO_DRIVER=dummy`; `EVENT_MM08_AS4_130` starts track 105 at its first tick in a
60-frame run under headless gamescope.

**Resizing.** Viewport and scissor are dynamic state, so the pipelines survive a resize;
the depth image and framebuffers are one `Targets` set, remade at the new size; the
swapchain is rebuilt (`Presenter::rebuild`, handing over the old one) when the window
reports a new size or when acquire or present says it is stale or suboptimal; a minimised
window (0 × 0) skips frames; and the projection takes the current aspect each frame. Not
yet exercised by an actual resize: headless gamescope scales the 1280×720 window into a
smaller output instead of resizing it.

**Controls.** In the window: Space pauses and resumes the event (the clock is given no
time while paused; the frame still draws), arrows turn the camera about its target (left
and right about the vertical, up and down over it, stopping short of straight above or
below), `=`/`+` and `-` move it in and out, R puts it back, Escape quits. The pump keeps
which keys are held and which were pressed since the last frame (repeats left out; all
released when the window loses focus); turning and zooming go by each frame's real time.
With the camera as it starts, frames are pixel-identical to before. Not yet driven by real
key presses in a test: headless gamescope has no keyboard.
