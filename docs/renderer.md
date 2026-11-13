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
bounds without drawing (the device is opened first, as the renderer is set up before any
scene, but nothing is uploaded).

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

**Starting at a line.** Without `--at`, `--line N` starts the event where line N starts on
the schedule (`--at` that time and a frame): the stage, the camera and the cast are as they
are then (`EVENT_MM01_OP1_222`'s line 10 at 82.32 s, the shot with the desk lamp).

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

**Outfits.** `--costume WHO:BODY:HEAD[:FACE]` (repeatable) dresses event character WHO in outfit
BODY (a `CostumeBody` row; its model is that row's `CharaBase` tag) with head accessory HEAD and
face accessory FACE (`CostumeHead` and `CostumeFace` rows), each part resolved on its own: `costume::resolve` gives the accessory attached and the body's own nodes
left off (`P_M_Acc_Hat1` ...), the body's tag being its `COSTUME_BODY_*` enum name. The
accessory (`Data/x64/Accessory/<HEADnn>.elixir.gz`, an unskinned model) hangs from the body's
`head_attach` (a face accessory from `face_attach`) node (its index in the body's `Oid.bin` names): its vertices are moved into that
joint's bind frame and by its `AccessoryAdjust` row (offset, then the rotation in degrees about
x, y, z, then the scale; the order is taken, not read) and bound wholly to the joint, so the pose
carries them. Escha with `--costume 0:0:2:1` wears the Flowery Hat in place of her own and the eyepatch. `--costumes SYSDATA` dresses every
costumed character as a settings save has them: `costumeData` entry k is `CostumeChara.xml` row
k, whose `target_chara` (`CHARA_*`, in party order) names the character, its `Chara.xml` row the
same number but for Nio (10), row 11 (the oracle save: Escha, Logy, Linca, Wilbell and Nio, rows
0, 1, 6, 7 and 11; Escha's uniform with her own Fluffy Hat attaches nothing, the hat built into
the body showing).

**Terrain under the distant view (fixed).** The terrain was recorded before the distant
view, which keeps no depth, so wherever the distant view's ground reached (the towns: `TOWN_04`,
`TOWN_05`) it painted over the terrain as one flat colour. `BARREL_HIDE=sky:` showed the terrain
textured underneath. The terrain now draws after the distant view, with the shared push constants
and instance set put back for the field's models after it. `AREA_01_01` looks as before. The
`.ktf`'s blend section does not hold a fixed two maps. Over the 141 terrains (`barrel
ktf-scan`), the number of blend maps does not follow the number of layers:

- `TOWN_04`: 6 layers and 6 maps.
- Elsewhere: 10 layers with 1 map, 4 layers with 5 maps.

So the maps' channels are not simply layers 0 to 5. The patch tree between 0x98 and the layer
textures must say which map and channels each patch uses for which layers. The viewer still
blends the first two maps over the first six layers, which is right only where that happens
to match. Reading the patch tree is open.

**Chained events.** `barrel-view --field auto OUT.png --event FILE.ebd --chain` plays the
event, then each one the story starts after it (`event_start::Chain`, as `barrel play-event
--chain`): when every step of the event has been applied, its scene is dropped (its geometry,
textures, instances, text layer, readback buffer and semaphores) and the next event's is loaded
under the same renderer, which outlives its scenes: device, layouts, targets, passes and
pipelines are set up once before the first. A chained event plays from its start (`--at` and
`--line` are for the first). Written frames number on across events, `--frames` being the
whole run's; closing the window or Escape ends the chain. From `EVENT_MM01_OP1_111` it runs
111, 121, 131 and on, as the game does.

**Town people.** `barrel-view --field DIR OUT.png --town-people all|GAMEDATA` adds the people
the field load would place on that field (`docs/tables.md`, "Town people" in the field NPC
notes; the map number comes from the directory's name through the `FIELDMAP_*` list):

- **With a save:** each placement whose conditions pass on the save's seen events.
- **With `all`:** each character's first placement.

Each stands at its spot (metres ×100 into field units, x mirrored like the enemies), looping
its `F_WAIT` motion on the viewer's clock, and the camera starts on the first. Random-companion placements are left out. On `TOWN_05` (map 4)
the oracle save (day 229) places one: `Chara` row 15 (`PC35`), an adventurer by a bench. On `TOWN_04` (map 3, 154 placements)
it places one too, `Chara` row 3, at the foot of the plaza steps: `barrel town-people EXE 3 SAVE`
shows three passing placements, all for that row. The save is early in the story (`MQ02_010`
seen, `MQ03_010` not), and most placements also wait for `XF01_030` (condition #159, which holds
while that event is unseen).

**Field enemies.** `barrel-view --field DIR OUT.png --creatures AREA --frames N` adds field
area AREA's enemies (12 is `AREA_01_01`; `barrel_game::field_ai::build`), each drawn as its
encounter group's first monster (`AREA_01_01`: two `MOB15A`, one `MOB01A`) in its `CharaBase`
motions, and steps their scripts each frame while a stand-in player (Escha, walking) goes
west at 2 m/s from 15 m east of the first enemy; each body follows its creature's place and
facing and switches to the motion its script plays. When one touches the player everything
stops there and the encounter group is printed (`AREA_01_01`: the first Ore Rat at frame 512,
group 10, as `barrel field-sim` finds). `--save-every N` writes only every Nth frame and the
last. The camera follows the player with the field's play-camera setting
(`Saves/field/<FIELD>.xml`, `<camera><area><point>`: `length` metres away, `radian` its yaw
and `lookRad` its pitch in degrees (+0x20 and +0x80, which the eye builder `0x1401e68f0` turns
about y and x), `fov`, `diffPosition` metres added to the player's place;
`field_look::CameraPoint`) places it; `AREA_01_01`'s first is 8 m, yaw 21°, pitch 10°, 40°, 0.5 m
up: the field camera keeps each area's own direction. Each `<area>` is a
rectangle (`tlPosition`, `brPosition`, metres) of a kind, its `tag` (`FIELDMAP_CAMERA_DATA_*`,
parsed at `0x140067f58`: `CLOSELY_X` 0, `CLOSELY_Y` 1, `CLOSELY_Z` 2, `CLOSELY_XZ` 3,
`VERTEX_LENGTH` 4; the fields use 77 `CLOSELY_X`, 62 `CLOSELY_Z`, 2 `CLOSELY_Y`), holding points
that also carry `bumper_follow_speed`, `bumper_max_distance`, `zoom_time` and `zoom_distance`
(`field_look::CameraArea`, `CameraPoint`). The play camera (`CameraCFieldMap`, the table at its +0x130,
lookup `0x140066f80`) takes the first area holding the player and blends that area's points
along its axis (`0x140069680`): the nearest point below the player on the axis and the nearest
above, mixed linearly (`0x140066cb0`) by the player's distance from the lower over the gap; one
alone when the player is beyond the last (`barrel_game::play_camera::setting`; `AREA_01_01`:
8 m, yaw 21°, pitch 10° up to z 30 m, then towards 10 m, yaw 7°, pitch 20° at z 45 m). The viewer's creature mode uses it. A setting is 0x40 bytes (`0x14006d7e0`): length, yaw, pitch and field
of view (the angles kept in radians), the look offset, the position, then `bumper_follow_speed`,
`bumper_max_distance`, `zoom_time`, `zoom_distance` (-1, 1, 2 and 0 when missing; the field of
view 22.5). The camera keeps the last four as tweens (+0x150, +0x168, +0x1b0, +0x1c8); nothing
in `CameraCFieldMap`'s own code reads them back, so what they do is not found.

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
terrain's height range to tell such placeholders apart. Such a flat placeholder (every height 0, at most one layer)
is now not drawn: in `EVENT_MM01_OP1_212`, on `TOWN_07`'s (64 × 64 cells of 500), the
event's characters stand at y -255 on the town's own wooden decks, under where the
placeholder would be, so the game cannot be drawing it either (how it decides is not read).
`--at` also schedules the event with the same (English) lines as playback: the reading times
follow the text, and the Japanese lines put the starting stage ahead of the dialogue (Awin
walking off while still talking).

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
the playing track (`Mixer::fade_out`), voice clips play once (`Library::voice`, from
`Sound/Voice`), and event-wide sound effects play at their volume and pan (`Library::effect`; the
pan law, the far side turned down linearly, is ours: the sound library's is not read). Every frame tops the SDL3 stream up to a tenth of a second. Test runs use
`SDL_AUDIO_DRIVER=dummy`; `EVENT_MM08_AS4_130` starts track 105 at its first tick in a
60-frame run under headless gamescope.

**Dialogue.** An event's lines show in the message window over the scene, in the game's English font
(`text.rs`, `barrel_formats::font`): the lines come from `Event_EN` (the original's when no
translation exists) and the speakers' names from `Saves_EN/chara/Chara.xml`. The stage keeps
the latest line (`Stage::line`) and, for unattended playback, takes it down after the same
reading time the schedule holds it for (`Stage::reading`). `<CR>` breaks a line, `<IMnn>`
puts in button icon nn (a glyph of the font, drawn in its own colours), `<CLxx>` changes the
colour by the executable's table and `<CLNR>` goes back to the line's own, and long lines
wrap at spaces (anywhere in Japanese). The window's text is the table's `BR`, a dark brown. The text layer is a second pipeline in the same render pass (no depth, alpha
blending) drawing screen-pixel quads from one vertex buffer that is only rewritten when the
line or the screen size changes; glyphs take their coverage from the atlas's alpha and the
panel is a quad with no texture. Sizes are for a 1080-line screen, scaled. The window is the game's own art,
from `WinXls_EN/a15_common.g1t` texture 28 (1024²): the cream speech bubble (41,4)–(208,173),
its tail on the left, drawn as its left part, a stretched slice of its middle (x 140–150) and
the end cap (732,4)–(786,173) mirrored on the right; the name sits on the leather strap
(14,188)–(344,252) over the window's left end. The same texture has the message log button,
the voice icon and small faces. How the pieces are put together (the window 1.3 times the
art, 12% to 88% of the width, 30 pixels up; the strap at 0.8 of that; dark brown text, cream
names) and the line spacing (53 atlas pixels) are ours: the game's layout code is not read.
`BARREL_TEXT_LOG=1` prints each line as it goes up. Frames written to files show the lines
too.

**Fields that change.** `--field auto` loads every field the event goes to (its
`FieldChange` steps, in order; `EVENT_MM01_OP1_111` visits `TOWN_11`, `TOWN_10`, `TOWN_09`
and `TOWN_04`), each with its own ground; every draw knows its field, and only the field the
stage is in now (`Stage::field`) is drawn, with the event's characters. Each field's textures
follow the last's in the one texture list.

**The event camera in the window.** Each frame takes the camera from the stage's blended
snapshot (the field camera or the manual one, whichever the event set last), mirrored into
render space; until the event sets one, the opening camera stands. The arrow keys still turn
and zoom about it.

**Cross-fades.** `u13` kind 3 (`docs/events.md`, "Screen effects") changes shots with a
cross-fade: the stage keeps the latest screen effect and when it started; on the frame a new
one starts, the last frame's scene image is copied into a "frozen" image before the new frame
is drawn, and the post pass lays the frozen frame over the live one, its weight falling from
1 to 0 over the effect's seconds. The other kinds are kept on the stage but not drawn yet. They are rare: of the `u13`s a raw
scan of the event files reads (673 of the 2,180), 669 are kind 3 and 4 kind 10 (`PostEffectGamma`).

**Fades.** The stage keeps the latest `u22` fade (preset, seconds, start); the viewer reads
the presets from the executable and the post pass lays the colour between the preset's two
over the scene by its opacity, under the text.

**Narration cards.** The viewer finds the cards an event shows (`u55`), reads their pieces
(`layout::pieces`) and loads their sheets into the text layer after the font and the window;
the card is drawn at the stage's opacity, its 1920 × 1080 layout scaled to the height and
centred.

**Dialogue window.** The window is the game's: `a15_common.g1t` texture 4's framed parchment
panel, the brown name tag and the cord from its grommet (texture 28, used before, holds the
message log's bubbles). The layout was measured from an oracle frame of
`EVENT_MM01_OP1_111` at 1280 × 720 (`text::window`: panel 360..960 × 562..710, tag, cord,
text from (409, 596), broken at spaces into lines of at most 48 characters (`LINE_CHARS`), whatever their
width: the game keeps "…, have you, Escha?" (48 characters, 480 pixels) on one line but
moves "trip," after "…in for a long" (54 characters, 464 pixels) down; seven lines of the
two openings agree (the widest, "…Do you know where", 47 characters, is 498 pixels). A
540-pixel width only guards wide glyphs)
and scales with the height, centred. Not yet as the game: the tag's slight tilt (about 5°)
and the small green fleur at the panel's lower right. `--size WxH` renders other shapes
(1280x720 for comparing with oracle frames).

**Shadows.** The sun's shadow map: one 2048² depth map from the field's first directional
light, an orthographic square 3,000 field units across around the camera's target
(`shadow.rs`; the game's four cascades are not read: a deliberate difference meanwhile).
The casters are the characters and the field objects marked `xDS_`, in their name or their
placement's tags (`barrel opl2 FILE#N` lists them): in `TOWN_11`'s workshop the furniture
and a second ceiling, `obj-xDS_appletenjo`, cast; the ceiling drawn, `obj-appletenjo`, does
not. The field's models and terrain take only the sun's part of their light through the map,
as `MapSTD` does (16 taps, the full strength with `atColor` white). Against the oracle frame
of `EVENT_MM01_OP1_111`'s first line, the floor's lit patch through the doorway and its
shaded centre and right measure 101, 64 and 67 against the game's 101, 65 and 67 (red, 0 to
255). With every model casting, the drawn ceiling shaded the whole floor instead.
`BARREL_NO_SHADOWS` turns them off.

**`xEF_` models are not lit.** Field objects whose names hold `xEF_` (TOWN_03's
`obj-xEF_windows_tw03`, the light through the bureau's windows; TOWN_11's `obj-xEF_einn`,
`_eine`, `_einw`) are drawn as the distant view's clouds are: their texture on their vertex
colour, no field light. The bureau's window (event `EVENT_MM01_OP1_121`) goes from 93, 123, 92
to 186, 189, 184 (game 224, 220, 197); Escha's workshop is unchanged. The desk lamps (TOWN_03:
145 in the game, 75 here) are still dull: neither an emissive parameter, a placed effect nor
point light units (metres blow the workshop out, clamped or not) explain them.

**Finding a moment.** `BARREL_SCHEDULE_LOG=1` prints when playback shows each narration card,
line and field, to pair the viewer's frames with oracle shots by what is on screen. Escha's
town tour (`EVENT_MM01_OP1_111`, cards 42 to 44 from 100.5, 111.2 and 120.9 s) matches the game
card for card in framing and cast; the differences left are the sky (a bright sunset there,
dull here) and the glowing lamp on the bureau's dome (not checked again since the point
lights' fix).

**Distant view order.** The distant view draws without depth, so its models go back to
front by kind: the dome (`Sphere_sky`), the clouds (`xEF_`), then everything else (ground,
stones, `ns_` trees, `xS3_` cliffs) in file order. In file order the clouds came after
`obj-ns_a15_obj_objtree_leaf_a` and the cliffs and veiled them: the dark bush and spires in
front of the green ruins at TOWN_07's airship dock (event `MM01_OP1_212`) were missing. How
much fog those near pieces get is still open.

The dome (`Sphere_sky`) and the clouds (`xEF_`) are drawn unlit (texture × vertex colour) and
alpha-blended; the solid pieces are lit like the field's models (field light on the vertex
colour) and alpha-tested. All are fogged as at the fog's end (`s = 1`). Lighting the pieces
brings TOWN_07's ruins from 107 to 96 (game 93, 97, 87) and leaves TOWN_10's street as it was
(the shader packs agree: `MapSTDNA`, texture × `atColor` only, fits the dome; the others are
lit). Still open: TOWN_07's `ns_` bush is dark in the game (52) and pale here, as if it took
less fog (fogged by its own distance it turns dark, but TOWN_10's distant trees then lose
their haze); and the fogged dome is 1.3 times too dark (176 against 243).

**Accessories.** A character's container holds its body (`<stem>_default.g1m`), its skeleton
(`<stem>.g1m`) and part models (`<stem>_P_M_*.g1m`). The parts named in
`CharaPartsChange.xml` (weapons, options: `P_M_Wep_*`, `P_M_Opt_*`) are switched on by group
and left off here; the accessories (`P_M_Acc_*`, Escha's hat `PC20_MODEL_P_M_Acc_Hat1`) are
not listed there and are always worn, so they are drawn with the body, on its skeleton and
instance (missing before: the oracle close-up of `EVENT_MM01_OP1_111` showed the hat).

**Finding objects.** `BARREL_HIDE=a,b` leaves out the draws whose object name holds any of
the strings (field objects by placement name, the distant view as `sky:<model>`): hiding
`sky:obj-a15a01_Sphere_ground` showed that the dark band behind `TOWN_10`'s street was the
distant view's ground ring.

**Distant view and fog.** The distant view stands in for what lies beyond the field: it takes
the fog as at the fog's full span (s = 1, nearly all in-scattered light), not at its own
distance (it is drawn around the camera). The street's background then turns the hazy
yellow of the oracle frame (`EVENT_MM01_OP1_111` at the narration); as fogged by its own
distance it stayed dark (80 against the game's 119 in red).

**Handedness.** The game's x is mirrored into render space (right-handed, like the vertices),
and the projection mirrors clip-space x back, so the screen shows the scene left to right as
the game does (checked against an oracle screenshot of `EVENT_MM01_OP1_111`'s first line:
Escha left, Clone right, the drawers top left); the front faces then wind counter-clockwise
again. Before this the whole picture was mirrored.

**Culling.** Models are drawn with back faces culled (front faces wind clockwise once x is
mirrored into render space); the terrain is not culled (it winds the other way and is only seen
from above). Rooms such as `TOWN_11` (the workshop in the opening, `EVENT_MM01_OP1_111`) are
one-sided boxes with a ceiling that the event camera looks into from outside: without
culling, the outside of a wall filled the view.

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

## Checked against the game

Oracle frames (headless runs of the original, `tools/oracle`) taken through both openings and
paired with the viewer by line or narration card (`BARREL_SCHEDULE_LOG`):

| Event | Field | What was compared | Result |
|---|---|---|---|
| `EVENT_MM01_OP1_111` | TOWN_11, 10, 09, 04 | workshop lines, close-up, town tour cards 38–44 | match; sky and outdoor shade darker in the game |
| `EVENT_MM01_OP1_121` | TOWN_03 | Marion's name card, lines 1–4 | match |
| `EVENT_MM01_OP1_131` | R&D room | room pan, lines 0–3, Logy's arrival (lines 7, 8) | match |
| `EVENT_MM01_OP1_212` | TOWN_07 | Logy's lines 1–6, overhead shot | match; sky too dark, `ns_` bush paler |
| `EVENT_MM01_OP1_222` | TOWN_03 | Colland's lines 0, 1, 7, 10 | match; desk lamp as the game since the point lights' `att2` fix |

Cameras, cuts, positions, walks, gestures, faces, name cards, line breaks and the window art
agree. The gaps left are lighting: the sky dome's brightness, the outdoor ambient (walls in
shade), the cloud-shadow light texture and the glow's level combination
(`docs/shading.md`).
