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
instance matrices and joint palettes are rewritten; each character keeps the motion it had
at the start, sampled at its time (looping motions wrap). Frames go to `OUT_000.png`,
`OUT_001.png`, ... In `EVENT_MM08_AS4_130` (`TOWN_04`), 4 frames at 2 Hz from 0.5 s show Logy
and Escha walking into the plaza towards Threia and Reyfer, mid-stride. Not yet: motion
changes during the event, characters added after the start, the stage camera, faces.
