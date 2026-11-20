# Renderer

`crates/render` draws the games' data with Vulkan 1.1 through `ash`. It is a library that
knows no game: `renderer::Renderer` is built for one set of graphics settings (the scene and
post passes, the pipelines, the shadow map, glow, the anti-aliasing and upscaler targets) and
draws into a window's images or an offscreen one; a game hands it each frame as a
`renderer::Frame` (the scene's geometry and draws, the cameras, the glow, the fade and the 2D
layers) with two answers of its own: which draws it shows this frame (`shown`), and each
draw's material bits and tint (`material`: Escha's come from mesh names, its field's sway
groups and the area map). Escha & Logy DX's program, `barrel` (`crates/escha/src/play`),
is the one that drives it so far; most of what follows was worked out on it.

The renderer is built once the graphics settings are final: the command line's, else the
config's (`BARREL_CONFIG` or `~/.config/barrel/graphics.conf`). Before 2026-11-13 the texture
sampler and the shadow map were made before the config was read, so the Options page's
Texture Filtering and Shadows were saved but drawn with the desktop defaults (16× and 2048);
they now take (checked offscreen: `shadows low` from the config gives the same frame as
`--shadows handheld`, `anisotropy 1` a different one from the default).

`barrel --field FIELD_DIR out.png` draws a field's objects: every instance in the
field's placement list, with its model from the `_Obj` container and the field's shared
texture pack (the object models carry no textures of their own). Each instance is scaled,
rotated (x, then y, then z, in radians) and moved on the CPU. AREA_01_01 places 112
objects (126 draws, 52,089 triangles), and they come out where they belong: a ring of
cliffs around a valley of trees, rocks and flowers. Under them it draws the
terrain from the field's `.ktf`: the terrain's own mesh (`ktf::mesh`, docs/formats.md
"Patches and their draws"), its finest patches' lit draws, each triangle carrying its draw's
one or two layers and its blend map (the draw's fourth byte) and channel pair. `terrain.frag`
mixes the two layers by that pair, each divided by their sum, as the field's own programs 9 to
11 do; a one-layer draw is that layer alone. The bake (programs 12 to 15: the patch's one-layer
draws, each its layer under one map channel, which the game paints into a screen-sized target
first) is summed in the shader instead: the ground is drawn a patch at a time with the patch's
bake list (up to 10 entries) in the push constants, and a baked draw adds the list's tiles,
weighted, to its own and divides by the whole weight as the game's programs do. That gives
the same colour without the extra target. Each layer is tiled every n cells (its record's repeat). In AREA_01_01
that gives the sandy path and the grass beside it. Not yet the game's: the stored normals (the
heightmap's are used), the tiles' alpha as the specular strength, the depth draws, the per-patch texture list the layer numbers presumably count in, and the coarse
patches at a distance.

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

**One frame of a moving event.** A single frame (`--frames 1`, the default) has no playback,
so the field drawn is the one the stage is in at `--at` (or `--line`'s start), as playback
would draw it: one frame is the first of several, pixel for pixel. Before, a one-frame render
of an event that had moved on (`EVENT_MM01_OP1_111` at 58 s, in TOWN_10) drew the field it
started in, and nothing showed but the characters.

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
palette, a new place a new instance matrix; the vertex buffer never changes. A vertex also
carries the mesh's tangent as one packed word (signed 10-10-10-2: xyz, and the bitangent's
sign in the top two bits; zero for a mesh without one), baked like the normal by the
transform's own 3×3 and turned by the joints in the vertex program: 72 bytes a vertex, 4 more
than before. Checked against
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

So the maps' channels are not simply layers 0 to 5: each patch's draws name their layers
and their map and channels, and the viewer now follows them (above).

**Chained events.** `barrel --field auto OUT.png --event FILE.ebd --chain` plays the
event, then each one the story starts after it (`event_start::Chain`, as `barrel play-event
--chain`): when every step of the event has been applied, its scene is dropped (its geometry,
textures, instances, text layer, readback buffer and semaphores) and the next event's is loaded
under the same renderer, which outlives its scenes: device, layouts, targets, passes and
pipelines are set up once before the first. A chained event plays from its start (`--at` and
`--line` are for the first). Written frames number on across events, `--frames` being the
whole run's; closing the window or Escape ends the chain. From `EVENT_MM01_OP1_111` it runs
111, 121, 131 and on, as the game does.

**Free play after the story.** With `--free-play` as well (`--event EVENT_MM01_OP1_111.ebd
--field auto --chain --free-play`, `--hero logy` for Logy's opening and Logy walking), when no
event starts after one ends the viewer loads the field that event last changed to and puts the
hero where the event left them, under the player's control (`--player`'s walker). From
`EVENT_MM01_OP1_310` (its dialogue runs to 104.6 s at reading speed; the plain step schedule
says 10.5 s) Escha stands at (10000, 10950) in `TOWN_04`. `Playback::end` gives an event's last
step time.

**A new game, saved and loaded.** `barrel --field auto OUT --new-game ROOT [--hero logy]
--window FRAMES` plays a new game: the opening the choice starts (`event_start::new_game_event`),
the story after it (`--chain`), then free play (`--free-play`), where the story goes on as a
`session::Session`. With `--save FILE`, F5 (or `BARREL_SAVE_AT=FRAME`, counted in the free-play
scene) saves it in Barrel's text format; `--load FILE` (with `--field` naming any field of the
install) starts in free play from such a save, or from the game's own `GAMEDATA<NN>.pcsave`
(told apart by its bytes: Barrel's text starts `barrel-save`; see `docs/formats.md`, "Starting from a game save"). From the end of `EVENT_MM01_OP1_310` a save at
free-play frame 60 holds Escha at (10000, 10950) in field 3 with events 15, 518, 520 and 521
seen on day 90, and loading it puts her back there. A door walked through in the story's free play asks the session first
(`Session::walked`, the story's `MAP_FROM_TO_*` triggers): an event that starts plays, and
free play goes on after it. After the opening, from the Hall (map 3) the door at (10000, 14850)
leads to the Town Entrance (map 4) and starts `EVENT_MM02_OP2_111`; going to the Administration
Office (map 2) would start `EVENT_MM02_OP2_510` (`barrel event-chain ROOT EVENT_MM01_OP1_310
0.3>0.4`). `barrel new-game` prints the session's save at free play. G (or `BARREL_GATHER_AT=FRAME`) gathers at the nearest
gathering point of the map within its reach (the point's size, 2 m in most, from its place on
the ground; the game's own reach is not read): one pick (`gather::Gathering::gather`) into the
session's basket, half a day of field time. In `AREA_01_01` from (3700, 4100) point 0, 76 units
away, gave item 0 at quality 50. The points are not drawn yet. A world map exit (`WORLDMAP_CHANGE_EX`) takes the player onto the world map;
with no world map screen yet, `--travel-to MARK` says where to (`barrel world-routes`): the
session takes the trip (`Session::travelled`: leaving a field area passes its carried time, the
route's days pass, an event on the road or on arriving plays) and the trip ends on the area
map of the place reached, the player on its point (a field on no point is loaded at once, at
its own world map exit). From the Town Entrance (map 4, its exit at (6010, 3740); a save
there at (5700, 3740), `--walk 300,0`) `--travel-to A0p0` passes four days (90 to 94) and
lands on the Forest Entrance's point; Enter goes into `AREA_01_01` at its arrival place. In the story's free play a field area (map 12 on) has its enemies out as
`--creatures` has them (their scripts, the touch and strike checks); from (4500, 3520) in
`AREA_01_01` walking west, the first Ore Rat closes in and touches Escha at frame 68
(encounter group 10). The encounter is then fought out on the console, without a battle
screen yet (`barrel_game::auto_battle`: the group's monsters drawn as the game draws them,
against the lead and the other alchemist at level 1 with their base stats, no equipment or
items, each standing in for the player as `skirmish` does), and the field goes on with that
enemy gone, whoever won (not yet the game's: no results, no game over). With the session's
party (level 1 in the new game's kit) and the damage rate (a flank attack's 25 %, a
critical's 10 %), Escha and Logy beat group 10's two Ore Rats on the battle screen (Escha 5 a
hit, Logy 7 to 8; 6 experience and 2 money). In the story's free play the encounter
opens its battle instead: the run starts again (as Return to Title does, the window kept) on
the area's battle stage (`BF_POINTnn`, the area's word 5) with `--battle GROUP`, the area's look
and the lead, carrying the story's save text and the way back (`Carry`); when the battle is
played out (a won battle's money, experience and drops paid into that save first,
`Session::won_in_save`, and its Result screen shown over the battle scene: `result.rs`, layout
898's heading, a row per member with portrait, level, experience to the next and "Level Up!",
the battle's EXP and Cole; Enter closes it, offscreen it stays 3 s; group 10's Ore Rats: EXP 6,
Cole 2, Escha and Logy at level 1 with 4 to the next) the run starts again in the field from that save, where the touch happened, with
the beaten enemies gone (`Carry::defeated`). From `AREA_01_01` at (4500, 3520) walking west the
Ore Rat touches Escha, stage 19 plays the battle, and the field comes back at (4140, 3520)
without it. A town map in free play has its people, placed by the field load's
conditions over what the story has seen (`town_people`, as `--town-people` does from a save).
Right after the opening, the Atelier, the Administration Office, the Urban Area and the Apple
Orchard have one or two each and the Tavern three (chara 14, `PC34`, and two townsfolk,
`NPC00B` and `NPC04B`); the Plaza has none yet (its regular, chara 8, waits for
`XF01_080`).

**The player.** `barrel --field DIR OUT.png --player X,Z` stands Escha (`Chara.xml`
row 0, `PC20`) at (X, Z) on the field's ground there (`game::walk::Ground`, dropped from above
onto the highest of the heightmap and the collision floors), in her wait motion, and points
the camera at her. The spot has to be one she could walk to: over a rock that has no collision
floor on top (AREA_01_01's plateaus) the ground found is the valley under it. AREA_01_01's
start, (5577, 3520), has her on the valley floor (`BARREL_HIDE=tree --pitch 50` to see her past
the trees). `--walk DX,DZ` has her run that way (DX and DZ units a second, her `RUN` motion,
`PC20_ACT_RUN`; `WALK` is the slow one) through `game::walk` each frame, against the field's
walls and onto its ground, turned the way she goes (as the field creatures are, from +z), the
camera carried along with her: `--player 5577,3520 --walk -300,0 --frames 30 --fps 30` runs her
300 units west along the valley. In a window W A S D steer her instead, relative to the
camera (the arrows still turn it; T is the game's run toggle, × 1.5; J jumps, `docs/engine.md`), at the game's run speed, 500 units a second
(`game::walk::player_run_speed`; 625 after `EVENT_XG05_110`, which the viewer takes as unseen); her walk, run and wait motions are
the game's choice by speed (`game::walk::player_gait`; `--walk -150,0` walks, `-500,0` runs),
facing the way she last went. She walks the field as `game::walk::Field` (its walls, ground and gimmicks);
stepping into a town door ends the scene and builds the next map's, her at the door's spot
(as the chained events rebuild theirs): `--field .../TOWN_01 --player 600,1000 --walk -300,0`
leaves the Atelier at frame 25 for Marion's office (`TOWN_02`, at (820, 1050)). The camera is
the field's play camera on her (`game::play_camera::setting` for where she stands: distance,
yaw, pitch, field of view, look offset), as the creatures' mode has it, so the Atelier and
Marion's office are framed from their own settings; `--free-camera` carries the set camera
along instead. (The game moves between settings smoothly and follows with the bumper speed;
here the camera takes each setting at once.)

**Town people.** `barrel --field DIR OUT.png --town-people all|GAMEDATA` adds the people
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

**Battle stages.** `--field .../BF_POINTnn` loads a battle stage like a field (docs/tables.md,
"The field's battle start"); with no look table of its own, `--look-from FIELD` lends it
`Saves/field/FIELD.xml` (the field area's; whether the game does so is not read yet), e.g.
`BF_POINT19 --look-from AREA_01_01 --player 3200,3200`. Without one it draws black.
`--battle GROUP` lays a battle out on the stage (`battle_people`): encounter group GROUP's
monsters drawn as the game draws them (`auto_battle::draw`) and scattered about the centre
(`battle::scatter`), the lead and the other alchemist at their places (`battle::party_spot`,
member k in sector k mod 3), all in their `WAIT` motion (the battle idle, `B_WAIT` for the
alchemists), the party facing the centre and the monsters the lead, seen from behind the lead.
Not yet the game's: the centre taken as the stage's middle (3200, 0, 3200), the camera, the
facings (the formation's own, `0x1401b0e30`, not used yet). Group 12 on `BF_POINT19` gives two
Ore Rats and two Stray Dogs, the largest at the centre. The battle is then fought out
(`auto_battle::fight_drawn` on the drawn monsters: it scatters them first, so a copy of its
generator taken before gives the scene the battle's own spots) and played (`BattlePlay`): an
event every 1.6 s, the actor in its `ATTACK00` motion and the target in `DAMAGE` (a fallen
member `DOWN` then `DOWN_LOOP`, a fallen monster gone once its damage motion ends), each back
to `WAIT` when its motion ends, the event's line on the console. Not yet the game's: the
timing, one motion for every action, no one running to their target, no effects, numbers or
sounds, no results. The battle HUD (`battlehud`, layout 886) is drawn over it: each member's
plate with their portrait and their HP and MP bars cut to what is left, following the events.
A member's turn waits for a command, chosen as the game's battle has it: the command row
(layout 887: Attack, Item, Skill, Move, Wait, Change, Run; left and right), Enter on Attack goes to
the target (left and right among the standing monsters, its name right of centre), on Skill to
the list of skills it may use (up and down, their MP costs, those it cannot pay for grey; Enter
then to the target); Enter on the target gives it, Back goes a step back (offscreen
`BARREL_BATTLE_KEYS`, a key a frame while a turn waits: `confirm,confirm` attacks the first
monster). Item opens the list of the battle items the member carries (the session's `carry`
lines, `auto_battle::Tables::carry`; name and uses left, spent ones grey), Enter on one thrown at
the monsters goes to the target, on one for the party uses it on the member itself. However
the battle ends, the uses left go back into the field's save (`Session::spent_in_save`; the
field's own auto battles through `Session::use_items`). Move goes to
the next sector round (`fight::Battle::move_turn`). Wait and Run are given (Run: `fight::Battle::run_turn`; escaped, the battle ends and
goes back to the field with no spoils and no lost day); Change is not yet. The battle is then fought
again from its start with the commands so far (`auto_battle::fight_commanded`: the same
generator and commands always give the same battle, so what was played stays) up to the next
member turn. In a window the battle plays its music from the start (`battle::music`: the encounter group's
kind, word 2, picks the boss theme, else the lead's theme, Escha's `bt_girl01` or Logy's
`bt_boy01`, or the vocal song once the field's save has flag 149), in place of the field's;
the Result screen starts the victory music in its place (`battle::victory_music`, as
`0x1401d819b`: `sy_btwin_girl01` for Escha, `sy_btwin_boy01` for Logy).
Each action's name stands in the action banner under the turn bar while it plays
(`battlehud::banner`: layout 885's plate, part 0xf8, at full size, centred on the screen, its
top at y 180, the name's width plus 306 wide; measured on the ruins-entrance-battle-result
recording, which is 1280 × 720, scaled to 1080p; the game sizes and centres it in code). A party action
also shows the damage rate panel for as long (`battlehud::rate_panel`, layout 926 at (1364, 150):
"Damage Rate:" with 100 + the action's rate in the purple digits 0xe3 and "%", "Total Damage:"
with the action's damage in the red 0xe4; the rate comes from `fight::Event::Rate`, sent as a
party action ends: its steps before the hits plus each effect's act step, the act table's last
word). While a party hit plays, LB, Y and RB (1, 2, 3; `BARREL_BATTLE_SUPPORTS` offscreen, members
in order) give that plate's member's Support Attack (`auto_battle::Command::Support` after the
action, the battle fought again; refused when the gauge is short or the target down). A monster's action on a member is held for a second first,
"Support Guard" bubbles over the plates of those who could guard, the same buttons standing guard
(`Command::Guard { by, at }`, `BARREL_BATTLE_GUARDS` offscreen). A hit's damage (and a heal's amount, a poison tick's) comes up over the fighter as the game's
number (`battlehud::number`: part 0xe0's white brush digits, sign first, 36 apart and centred,
at about 140%, fading in over 0.1 s from 1.6 times that by 0.2 s, gone after 1 s; measured on
the ruins-entrance-battle-result recording at 149.8 s, a 1280 × 720 recording), 100 units above the fighter's place.
Not yet the game's: the list (the game's command layout is not identified), no
mark on the target in the scene, no items, guarding or fleeing, the pad not mapped.

**The question.** Back on an area map asks "Return to World Map?" as the game does
(`question.rs`, the recording `colseit-arrival-60hz.mkv` at 1:05): the system message window
(layout 701's place, `a15_common.g1t` texture 4's window piece stretched to 900 × 222 with its
ends kept) and the Yes / No box over its top edge (texture 23's ornate panel stretched keeping
its corners, texture 3's fading yellow band on the lit row, texture 16's feather). Yes lit
first; up and down choose; confirm on Yes goes to the world map, No or Back closes it. The
neighbours' labels go while it asks. Stretched pieces are `hud::Element::Slice` (`hud::nine`
keeps a panel's corners). Checked side by side with the recording.

**The area map.** With `--load SAVE`, `--area-map` starts on the area map at the save's
field's point (as a lost battle does). `barrel --field .../A15_WMAP OUT --world-point SP_A2P0 --area-map` puts
Escha on a region's point of the world map with the area map's cursor (`AreaMap`, over
`game::area_map`): W A S D (offscreen `BARREL_AREA_KEYS`, up / down / left / right / confirm,
one a frame once she stands still) take the route in that way's link slot, which she runs
along its dots (`world::route_path`, queued as the walker's waypoints) to the next place's
mark; the panel shows the place's name and description; Enter walks into the field the point
opens (`world::point_maps`, through `Session::walked`, so an event there plays) at its arrival
place. Checked: the Ruins Entrance, down along `A2r1` (7 dots) to Sector One, Enter: field 26,
`AREA_03_02`, at its arrival. The camera looks down on her from the south (screen up is -z, left -x) at 35 degrees from
7600 units with a 10.7-degree view, at a point on the ground 178 units north of her (`AREA_CAMERA_*`), fitted
to the recordings (`colseit-arrival-60hz.mkv` and `ruins-entrance-lost-battle-60hz.mkv` at 0:45,
the same view: Sector One's row of dots at z 31300 and the column toward Ruins Entrance at x
26250, z 30000 to 31000, their centres found in the frame and the projection fitted to them:
0.85 pixels RMS over 14 dots at 720p; view and distance trade off, 10 degrees from 8150 fits
within 1.1). The earlier fit by eye (43 degrees, 5400, 15 degrees) was 28 pixels off on the
column, the far side spread too much. She is drawn 1.75 times
her size there (`AREA_MAP_SCALE`, as the recording has her), and the neighbours' labels (layout 638, "Ruins Entrance", "Sector Two") stand on
the route beside her, a step out each way, not on the neighbour's mark. The marks of sectors closed or not known
yet are not drawn, like the dots of routes not known (the recording at 0:45 shows none for
A2p2 and A2p4 south of Sector One). Each sector has two marks placed on it, `point_sm_on` (red) and
`point_sm_off` (yellow); the recording has Ruins Entrance (visited) red, so a visited sector
shows the first and one only seen the second (Barrel's rule, matching the save's 15 / 10
knowledge values; a seen sector's mark not yet checked on screen). The world map leaves out the sectors' marks and dots
altogether (`point_sm_*`, `ns_a15_point_r_off`): the recording at 1:12 shows only the regions'
marks and the routes between them. The routes between regions show only once walked (both
ends visited, the ends as the route table has them: R2 is `T0` - `A1p0`, though it sits in the
Old Highway region's links), white; the others are not drawn. The world's places open with the
story, not by lying next to a visited one: the user's save at 8/20 (`barrel save`, "knowledge
route") knows R0, R1 and R2, all at 15, and none of Colseit's other routes, and the recording at
1:12 shows the same. The maps draw no sun shadow (the recordings show
none under her; the ruins' dark undersides are in their own textures). The map cursors `E3D_WMAP_CUR00` / `CUR01`
(CharaBase rows with Directory "Obj", `Data/x64/Obj`, each with a 3.37 s looping
`BASE_MOTION1`; CHARA enum 705 and 740) are upright markers, claws around a blue crystal, 260
wide and 385 tall, CUR00 with an additive glow (a 4 × 512 purple-to-teal strip). The map's
setup creates CUR00 (`0x140383c42`), and `0x140388820` sets the walker's model by its
character: 705 shows CUR00 at scale 1, the lead character is drawn at 3.2 times (+0x250,
`0x140d65518`), anyone else at 1. The blue ring under her in the recordings is her start
mark tinted: the maps tint the way chosen (`AreaMap::chosen`: the start's mark, the cursor's
place's mark and the dots of the route between them; colseit-arrival 1:17, the dots past the
cursor's place white) by a per-channel transform on the display colour, `c * mul + add`
(`CHOSEN_TINT`: red 0.8, green 0.97, blue 0.91 + 0.28 on the unlit colour; first fitted on
the recording's screen colours of a white dot, the town mark's rim and its red middle, which
is why no plain multiply fitted, then adjusted for the overlays laid after it: the mark, its
rim and a dot within 14 levels at 1:25). The marks and dots are drawn unlit: their models'
colour pass is `MapSTDNA` (the clouds' and props' `MapSTD-B0` take the map's light), and the
recording shows their texels as they are (the town mark's red middle 175, 92, 75 in the
texture, 179, 91, 74 on screen); the renderer's draws carry their shader's name
(`Draw::shader`) and Escha draws `MapSTDNA` without light, as it does `xEF_` models. The renderer's instances carry
it (`Instance::tint_mul` / `tint_add`, off by default: Escha's and Ryza's frames unchanged), so
the maps' marks and dots take their own instances. Not yet the game's:
her shadow there (the recording shows none), the labels' plates (the neighbours' names now stand on their routes' third dot,
48 pixels up, while she stands still: `area_panel::label`), the routes not yet open (the
recording at Sector One shows no way down to Sector Three: Barrel draws and takes every
route), the scroll's art and its "X →" button, the item lines (the panels are the game's layouts
637, 640 and 641 placed as the recording has them: `area_panel`), the
buttons (636), the place labels (638), the party member's "Enter / Quit" prompt and Back to
the world map. A lost battle lands there too, as the game's wipe does (`docs/events.md`, "A
lost battle"): the carried save comes back with `to_area_map`, so the run starts on the world
map at the battle field's point in area map mode (no enemies, no field HUD, the walk
stopped). Checked with the lost-battle recipe in `AREA_01_01`: the wipe at frame 149, then the
Forest Entrance with the day passed (Left 114 to 113); Enter goes back into the field. Leaving a field area by its world map exit without
`--travel-to` lands on its area map at the field's point (`Session::left_for_area_map`: the
carried field time passes, as between fields): from (3700, 3680) in `AREA_01_01` walking west,
the exit at frame 6, the Forest Entrance (`SP_A0P0`), confirm, back in `AREA_01_01` at its
arrival place. The town's exit still ends the run (no world map screen).

**Field enemies.** `barrel --field DIR OUT.png --creatures AREA --frames N` adds field
area AREA's enemies (12 is `AREA_01_01`; `barrel_game::field_ai::build`), each drawn as the
model the game gives it (`FieldEnemy::model`, a `CharaBase` row; its encounter group's first
monster when missing, which agrees: `AREA_01_01`: two `MOB15A`, one `MOB01A`) in its `CharaBase`
motions, and steps their scripts each frame while a stand-in player (Escha, walking) goes
west at 2 m/s from 15 m east of the first enemy; each body follows its creature's place and
facing and switches to the motion its script plays. When one touches the player everything
stops there and the encounter group is printed (`AREA_01_01`: the first Ore Rat at frame 512,
group 10, as `barrel field-sim` finds). `--save-every N` writes only every Nth frame and the
last; an offscreen run writes at most 600 frames, so it may run 600 × N frames (a field battle
fought to its Result and back: `--frames 3000 --save-every 20`). The camera follows the player with the field's play-camera setting
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

**Playing events.** `barrel --field DIR OUT.png --event FILE.ebd [--at S] --frames N
--fps HZ` builds the scene once, then plays the event on (one frame too: the event runs up to
`--at` first, so a still shows what its commands have set by then, such as the Atelier's
`xEV_` west wall that `q10 1` shows from 3 s into `EVENT_MM01_OP1_111`; a still once skipped
the playing and drew the room open on that side, which the game's own shot showed wrong): an `event::Player` schedule in a
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
Logy and character 7 appear by 0.5 s, placed at 0.1 and 0.2 s). `--field auto` takes the field from the event (`docs/events.md`, "Which field"). Faces change as the event goes: every character with a `Facial` container gets all its
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

**Live window.** `--window FRAMES` (0: until the player quits, as a game launch uses it) opens
a 1280×720 window (winit 0.30, Wayland or X11) and presents through a FIFO swapchain
(`present.rs`: the surface, the swapchain, acquire and present; vsync, so the display paces the
loop), with the engine clock taking each frame's real time (`docs/engine.md`, "Timing").
`Gpu::new` takes the window's display handle to add the surface and swapchain extensions.

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

**Texture filtering.** The scene's textures are filtered anisotropically (`RenderSettings::
anisotropy`: 16 on the desktop, 4 on the handhelds, capped by the device's
`maxSamplerAnisotropy`; off where `samplerAnisotropy` is missing). Against the game's frame of
`EVENT_MM01_OP1_111`'s arch shot (`TOWN_09`), trilinear filtering blurred the ground's cobbles
into a plain band a few metres off where the game keeps them sharp; with it they carry as far.
The game's own sampler states are not read (by the look, not yet the game's setting).

**The finished frame** (`post.frag`): the scene doubled plus the glow (with a rate of 0,
the bloom's passes are skipped, the exposure metering alone running, and the glow is not
read; at the handheld's settings on the PC the glow stage went from 0.21 to 0.05 ms), then for a game that
asks (`Frame::tone_white`, `Frame::saturation`; Escha passes 0 for both and is unchanged) a
tone curve, `c (1 + c / w²) / (1 + c)` with white point w, and a saturation scale per channel
about the BT.601 luma. Ryza takes w from its set's `fLumiMax` and the scale from its
`vSaturationScale` (docs/ryza/open.md). The scene image is 8-bit RGBA at half range (so 2.0 at
most), or with `RenderSettings::hdr_scene` (a game's choice, Ryza's) `B10G11R11_UFLOAT`, the
same size, unclipped for the tone curve; the TAA and upscaler targets take the scene's
format, GSR 2 and FSR 3 are float already.

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

The map is an array of five layers (`shadow::CASCADES`; the fifth below). Layer 0 is the square above (Escha's
only one: its frames are unchanged, the other layers cleared once and never read). The Ryza
games draw the game's three cascades into layers 0 to 2 every frame (`Frame::shadow_cascades`:
layer 0 is `shadow_vp`) and a static layer into layer 3 (`Renderer::far_shadow(want)` each
frame, before it writes the look slots 87 to 91): the casters of the whole field in one box,
drawn once, whole, in the frame it is first asked for. A cascade is the smallest box in the
sun's view around a slice of the view's frustum (`shadow::fit_view_proj`; docs/shading.md,
"The cascades"); the receivers pick the cascade by the distance along the view
(`shadow_cascades.glsl`, from the look slots `SHADOW_CASCADE_SLOT`) and fade into the static
layer before the last cascade's end. The fifth layer (`shadow::SUB_LAYER`) is the characters' own map, the game's `sSubShadow`
(`Frame::shadow_sub`, Ryza's player): the player's draws (`Draw::sub_bias`) are drawn into it
every frame, 1024 texels a side in the layer's corner (`shadow::SUB_SIZE`), from the smallest
box in the sun's view around her box, with the game's bias (a set per draw, the push
constant's alpha reference, slope and constant rewritten between draws); and they are left out of
the cascades, as the game's three passes hold none of her draws (docs/shading.md). The receivers
take the smaller light of the cascades and this map before the fade into the static layer
(`sub_light` in `shadow_cascades.glsl`, the look slots `SHADOW_SUB_SLOT`). The cascades' depth
is written with the game's own bias
(`shadow_cascade.frag`, `shadow::Bias`; the near square keeps the fixed bias), and casters
under four of a layer's texels across are left out of layers 1 to 3 (`Bounds::extent`). At
Rasenboden's gate (1920x1080) the shadow pass took 6.9 ms and the frame 11.6 ms (6.4 with the one
near square): 1,100 caster draws a frame (160, 235 and 687 in the cascades; the game's passes
hold 104, 177 and 669). That was the geometry's place, not the draws, the vertex work or the
textures: the vertices and indices sat in host-visible memory, which the vertex fetch read
across the bus in every pass (draws cut to three indices each took the ambient occlusion's pass
from 4.4 ms to 0.2; a position-only vertex shader, no textures and no descriptor changes moved
nothing). They are copied into device memory once through a staging buffer
(`SceneGpu::new`): the shadow pass takes 1.86 ms, the scene pass 1.25 (4.85) and the frame 3.7
ms with the occlusion (0.48) in it (median of 37 frames, the gate on an RX 9070 XT; the
instances, palette and morphs stay in host memory, as they change each frame). The preset's shadow distance (`shadow_reach`, 1 on High, 0.7 on Middle)
scales the cascades' ends; how the game applies it is not known.
The canopies' sun share against the game's alpha at the gate: left canopy 0.010 (the game's
0.010), dark canopy 0.012 (0.009), trunk 0.002 (0.001), middle tree 0.112 (0.117); the 3% of lit
pixels the canopies leaked were the leaf cards' back faces culled in the main pass (docs/shading.md,
"Two-sided materials"), not the casters. With them two-sided the scene pass takes 1.5 ms (1.25
before: the crowns are denser) and the frame 4.0 ms.

**Ambient occlusion.** The Ryza games' frames (`Frame::occlusion`; the graphics setting
`occlusion`, on in the desktop presets and off in the handheld's, config `occlusion on | off`)
draw a pass before the scene (`ao.rs`): the terrain, the field's models and the distant view's
solid pieces at half the scene's size into a target of world normal and linear depth (the models'
and the terrain's own fragment programs, specialised by `AO_PASS` to write them; the normal maps
they read give the normals; the depth test culls the rest), then a full-screen pass (`ao.frag`)
turns that into the occlusion, an 8-bit half size target (docs/shading.md, "The ambient
occlusion"). The scene's fragments read it bilinearly through their position (`occlusion.glsl`,
set 5 of the model layout and set 2 of the terrain's; a one-texel white map where there is none,
so Escha's frames and a setting off draw exactly as before) and apply it as the game's lighting
pass: to all the diffuse light, tinted by the colour, and to the direct and ambient specular.
`BARREL_DUMP_STAGES` writes it as `ao`. At the gate (1920 × 1080) it costs 0.48 ms
(`--timings`: "occlusion"): the normal pass 0.3 ms for 950 draws and 680,000 triangles at half
size, the full-screen pass 0.2.

**Normals and depth without the occlusion.** The geometry pre-pass runs only with the occlusion
(the AO has to be known before the scene's lighting in a forward pass). The reflections, the depth
of field and the water's refraction read the same half-size target (`ao::GBUF_FORMAT`: world normal,
view depth) where the occlusion is off, and there the scene pass makes it: the models and the
terrain write their world normal, octahedral in two half floats (`normal_octahedral.glsl`,
`ao::NORMAL_FORMAT`), into the motion vectors' attachment (location 1, which a scene pass without
`motion` does not have; `NORMAL_AS_MOTION`, a specialisation of the scene's pipelines; the
other programs leave it unwritten), and the pass keeps its depth buffer (stored, read in its
shader layout). When the scene pass ends (a frame with water: when its opaque first part ends)
one full-screen pass (`gbuf_derive.frag`, `Occlusion::derive`) builds the half-size target from
the depth (the view depth from the projection's depth row, `device_depth_to_view`; the clear depth
where the depth is 1) and the normal, one scene texel per half-size texel. Nothing is drawn twice.
A frame with `motion` (a temporal upscaler) keeps the pre-pass. Differences from the pre-pass's
target: the distant view's solid pieces (flag 16), drawn on the far plane, hold the clear depth, the
normals are octahedral to a half float, and the outline hulls and the characters' hull pass may
add depth. At the rp6's settings (Low reflections, no occlusion; the gate, `--timings`, 240
frames, the desktop GPU) 1920 x 1080: occlusion 0.222 -> 0.005 ms, scene 1.560 -> 1.616, the
three stages 3.399 -> 3.220 ms; 3840 x 2160: occlusion 0.416 -> 0.010, scene 4.456 -> 4.717, 6.481
-> 6.323. The scene pass costs more here (the normal's write, the depth kept, the full-screen
pass); the geometry drawn twice is what a tiler (the RP6) pays most for. At High (occlusion on) the
frame is as before (319 pixels differ by at most 3 at 1080p).

**Depth of field.** Ryza's frames (`RenderSettings::depth_of_field`, on in `ryza`'s settings;
like the reflections it needs the normals and depth target and single-sampled drawing) soften the far
view as the game's type 0 does (`dof.rs`; docs/shading.md, "The depth of field (type 0)"). After
the scene (before the glow) three passes run: the weight (`dof_weight.frag`: the occlusion's
view depth turned to the game's raw depth and looked up in the curve, at the scene's size), the
two quarter-size layers (`dof_down.frag`: four bilinear taps of the scene and the weight, split
by the weight) and the blur (`dof_blur.frag`: 16 taps of the layers, into two more
quarter-size targets). The post pass reads the weight and the two blurs (bindings 7 to 9;
`screen.z` is 1 for a frame that made them) and, on the scene at its exposure, mixes the far
view's blur by the weight's ramp where the weight is 1/2 or more, then the near view's by its
own alpha (`post.frag`'s `soften`). The layers are kept in the scene's units; the game's carry
its adapted luminance (0.9 at the gate), which the post pass applies to the scene and the
layers alike. `BARREL_DUMP_STAGES` writes `dof_weight`, `dof_layers` (near view) and
`dof_layers_back`, the board's `dof_weight`, `dof_layers` and `dof_layers_back` (and their
`_alpha`s; the board brings Barrel's layers to the game's exposure). At the gate the weight
agrees to 0.0004 (alpha 0 to 1), the layers to 0.008 (their colour is the scene's, which
agrees to 0.016), the tone to 3.67 from 3.89 (turn 4.39 from 4.57, left 4.50 from 4.74).

**Screen-space reflections.** Ryza's frames (the game's choice in `RenderSettings::reflections`,
on in `ryza`'s settings; it needs the normals and depth target and single-sampled drawing, so
MSAA leaves it out) add the game's reflections to the specular ambient (`reflect.rs`; docs/shading.md,
"The screen-space reflections"). The scene pass has two more colour targets (`Targets::spec`,
rgba16f at the scene's size, written by `model.frag` and cleared to nothing): the specular
ambient's per-pixel factor (`o' F (o' + specOcc (1 - o'))` times the fog's extinction; the gloss
in alpha) and the scaled cube light it reflects. After the opaque scene (in a frame with water,
before the copy the water refracts) three passes run: the march (`reflect.frag`: 256 steps of a
half-size pixel over the occlusion's normals and view depth, the lit scene's colour at the hit, the
alpha the fades' complement) into level 0 of a six-level rgba16f chain (960 by 540 down to 30 by
16), the mip passes (`reflect_mip.frag`: one bilinear sample of the level above each), and the
application (`reflect_apply.frag`), which adds `factor * w * (reflection - cube light)` to the
scene, `w` the level 0 alpha times `vMerge.x` and the colour taken at lod `5 (1 - gloss)`. The scene
already holds the cube light (the models add it as before), so this moves it toward the reflection
as the game's mix does. The forward pass over her blends the factor by one minus its alpha as it
does the scene. `BARREL_DUMP_STAGES` writes the march as `reflections` (the board's `reflections`
and `reflections_alpha`). Against the game's march at the gate the weight over her thighs is 0.25
(the game's 0.26) and over her boots 0.44 (0.44); the colour 0.34 0.29 0.18 against 0.38 0.31
0.20.

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

**The sky plane** (the Ryza games). A frame with `Frame::sky_plane` set draws, first among the
far plane's fragments (the distant view's pass: depth range 1 to 1, compared `LESS_OR_EQUAL`
against the cleared depth, no depth write), a triangle over the screen with `sky.vert` /
`sky.frag`: each pixel's view ray from the look's eye and inverted `view_proj` (slots 4 and
44), shaded as `skyplane.g1s` does (docs/shading.md, "Ryza DX: the sky") from the look's slots
`SKY_SLOT` (76) to 86. The distant view's draws (the cloud dome) blend over it. Escha never sets
it, so its frames are unchanged.

**Distant view order.** The distant view draws without depth, so its models go back to
front by kind: the dome (`Sphere_sky`), the clouds (`xEF_`), then everything else (ground,
stones, `ns_` trees, `xS3_` cliffs) in file order. In file order the clouds came after
`obj-ns_a15_obj_objtree_leaf_a` and the cliffs and veiled them: the dark bush and spires in
front of the green ruins at TOWN_07's airship dock (event `MM01_OP1_212`) were missing. How
much fog those near pieces get is still open.

The dome (`Sphere_sky`) and the clouds (`xEF_`) are drawn unlit (texture × vertex colour) and
alpha-blended; the solid pieces are lit like the field's models (field light on the vertex
colour) and alpha-tested. The solid pieces are fogged as at the fog's end (`s = 1`), the
dome and clouds at their own distance (below). Lighting the pieces
brings TOWN_07's ruins from 107 to 96 (game 93, 97, 87) and leaves TOWN_10's street as it was
(the shader packs agree: `MapSTDNA`, texture × `atColor` only, fits the dome; the others are
lit). Still open: TOWN_07's `ns_` bush is dark in the game (52) and pale here, as if it took
less fog (fogged by its own distance it turns dark, but TOWN_10's distant trees then lose
their haze); and the fogged dome is 1.3 times too dark (176 against 243). The dome's mesh groups name
`MapSTD` (not `MapSTDNA`, as taken above: `docs/shading.md`, "Which texture is which"), so it
is lit in the game; lit here as the solid pieces are (field light on its vertex colour) it
turns darker still (a patch of `EVENT_MM01_OP1_212`'s first shot from 183 to 158), so the
missing brightness is not that light: what `MapSTD` adds for the dome (its specular, the
light texture, or the fog's in-scattering at its distance) is still to find, and the dome
stays unlit until then. Against the reference recording (`EVENT_MM01_OP1_111`'s upward arch
shot in `TOWN_09`, the game at 77.5 s, the renderer at 74 s) the game's sky is a saturated
yellow with the dome's clouds plainly showing through and a bright patch toward the sun (top
246, 234, 184; the renderer 220, 195, 153, a flat cream with the clouds all but gone). Unfogged,
the dome is a purple-blue cloudy sky: so the game fogs it, but less than the fog's full span,
and its in-scattering peaks toward the sun (`TOWN_09`'s `mieG` is 1.0: a sharp Mie peak). Taking
the scattering angle the other way round changed nothing visible. The dome (`Sphere_sky`,
radius 32,000 to 35,400 units) and the clouds (`xEF_…_Sphere_cloud`, 26,900 to 33,100) now
take the fog at their own distance, as their programs (`MapSTD`, `MapSTD-T0`) compute it with
nothing clamped: s ≈ 4.7 in `TOWN_09` (fog 15 to 80 m), so almost all of what shows is the
in-scattered light, Rayleigh and the ambient. The arch shot's sky went from 220, 195, 153 to
241, 210, 147 at the top (the game 246, 234, 184) and 206, 179, 120 in the middle (195, 182,
120). The solid pieces keep the fog's full span (s = 1). `TOWN_10`'s narration sky is
unchanged (92, 89, 41 both ways; the game's frame there is framed differently). The game's
brighter patches in that sky are not the dome's clouds (fogged away at that distance) but
likely its glow. Its Mie term is 0 with `mieG` 1 (the factor (1 − g)², checked live in three
fields).

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
from above); Ryza's alpha-tested field materials (`PBBA2-N`, docs/shading.md, "Two-sided
materials") are not culled either, a second scene pipeline and a second normal-pass
pipeline for them. Rooms such as `TOWN_11` (the workshop in the opening, `EVENT_MM01_OP1_111`) are
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

## Frames in flight

The CPU prepares a frame while the GPU draws the one before (`ring.rs`), in a window and
offscreen alike. On the RP6 a frame was about 0.3 ms of simulation and 0.8 ms of recording on the
CPU and 8 ms on the GPU, and the loop used to wait for each frame before starting the next, so
the frame took the sum; now it takes the larger.

- Two slots (`ring::SLOTS`). The frame being prepared is `Gpu::frame`, counted up at each
  submit; its data goes into slot `frame % 2`. At the top of each frame the loop waits for the
  fence of that slot's last frame (two back), then frees its command buffer
  (`ring::Frames::wait_slot`).
- Per-frame data the CPU writes lives in `ring::Ring` buffers, one copy per slot, mapped once:
  the scene's instances, joint palette (this frame's, then last frame's) and morph weights, and
  the lighting block (`Look`). The scene's set 1 exists per slot and field
  (`SceneGpu::instance_set`). A text layer keeps a vertex buffer per slot: the first change
  in a frame goes to the one no frame in flight draws from, later ones in the same frame to
  that one again. FSR 3.1's constant buffers have a copy per frame parity, as its sets do.
- Each frame's commands start with a full barrier (all commands, memory writes to reads and
  writes), so the GPU finishes the frame before on the shared targets (scene, depth, glow,
  upscaler and TAA histories) before this one starts on them.
- The post pass has explicit dependencies: in, its layout change waits at the colour output
  stage (the one the acquire semaphore guards); out, offscreen, the move to TRANSFER_SRC is
  done before the readback copy.
- Offscreen, a frame written to a file is waited for and read back; the others stay in flight.
  `--timings` keeps one frame at a time (its timestamps are read after each frame).
- Each swapchain image has its own "drawn" semaphore for its present; each slot its own acquire
  semaphore and fence.

Checked with the Khronos validation layer's synchronisation checks (no hazards, window and
offscreen), and offscreen frames are byte-identical with and without overlap. On the RP6 at 1080p
(`handheld`), offscreen, 300 frames: Sector One 8.95 → 8.12 ms a frame, the opening's town
7.90 → 7.11 ms, each now the GPU's own time (8.04 and 7.28): the GPU is what is left.

## View culling

A field object wholly outside the view is left out of the scene pass, and one wholly outside the
sun's shadow volume out of the shadow map (`Bounds`): its box in model space (its vertices on the
scene's starting joints, padded by a quarter of its longest side and a metre for the grass sway
and the trees' springs, and by its longest billboard corner) goes through the instance's model
matrix each frame and is tested against the six planes of the view-projection drawn with
(`view_planes`). Characters and the distant view are always drawn. `BARREL_NO_CULL=1` draws
everything; frames are byte-identical either way (32 frames: Sector One, the opening from 20 s,
TOWN_04 from four sides). `--timings` also prints the draws recorded every 100 frames.

**Ryza DX: which field models are drawn.** A placed object is in or out as a whole: every
part of it is drawn when the sphere around the box of all its parts (the box's centre, half
its diagonal) reaches into the view (`Bounds::round`, `Draw::object`). A rigid part's box is
not padded (nothing moves it but its placement; a skinned one keeps the padding). Measured
against the game's draws in Rasenboden (913 draws, frame 2550, paired by index count and
place): with each part tested by its own box, 41 of the game's field model draws had no pair,
among them parts of the gate behind the camera and small parts a step from the eye; with
each part's own sphere, 22; with the object's sphere, 2 (measured under a 45.4-degree view, 1% wider than the game's own
45 degrees: with the exact view two more of the game's draws have no pair, `docs/ryza/open.md`). What the game really tests is not
read from its code.

**Ryza DX: objects behind the ground.** The game also leaves out models inside the view
that something stands in front of (fences, stairs, rocks: 46 draws on the title, some 190
in Rasenboden, and the townsfolk out of sight), hidden and drawn ones at the same distances
and of the same draw-distance classes. How it tests is not read: its files hold no occluder
shapes, and it is not the GPU's occlusion queries (the answer key reports none visible, and
most objects are drawn; with `RYZA_TRACE_D3D` the game makes only event, timestamp and
disjoint queries, a few a frame, and no occlusion query or predicate at all). The ground explains most of it: of Rasenboden's 107 objects the
game hides whole, 92 have the heightmap between the eye and all eight corners of their box,
against 3 of the 213 it draws whole. Barrel's test (`barrel_ryza::horizon`, a stand-in)
takes the ground's skyline from the eye: for each of 1,440 directions, the steepest slope
up to any ground within each distance, half a cell a step (made for the directions asked,
kept while the eye stays). An object is left out of the scene when, in every direction its
sphere spans (the sphere of the view test), ground nearer than the sphere stands steeper
than the sphere's top. A townsperson is a sphere of 120 units around a point 90 above the
feet. It hides 65 of those 107 objects and none the game draws (the box in place of the
sphere hides 92, and 3 objects the game draws), and the six townsfolk the game leaves out
of the nine it does not draw; Rasenboden goes from 1,182 scene draws to 971 against the
game's 913, the picture the same to the byte there and from eight other views. On the PC
(RX 9070 XT, 1920x1080, offscreen, 117 frames, median) the scene pass goes from 5.42 to
4.69 ms and the frame from 6.92 to 6.20. The shadow
pass is not tested (a hidden object's shadow can fall in sight); the title has no test.
`--no-skyline` draws everything.

**Ryza DX: object draw ranges.** The field's camera file gives a draw range for each class
of object (`disp_range`: `obj_near`, `obj_middle`, `obj_far`, `obj_landmark`, in metres), and
the graphics setting for object distance (`DispRangeType`) gives its own near, middle and
far. The PC game uses none of them at the top setting: its test (`0x1404d1ae0`, each frame,
each class) takes 999999.9 for every class unless a flag is set, and the flag is set by a
`use_disp_range` in the field's file (no shipped file has one; the PC's default is off,
`0x1404fc3d0`) or by an object-distance setting below the top (`0x14011a32c`). Logged in the
running game at High: the fallback is taken every frame, on the title and in Rasenboden,
where it draws near-class objects 8,000 to 12,000 units away. With the flag set, the
setting's three ranges replace the field's (the field's are set first, then the setting's:
`0x14011a2ec`) and the landmarks keep the field's; an object is drawn while its point is
within its class's range of the camera measured over the ground (x and z alone), fades out
past it and comes back only 300 units inside it. Barrel does the same test without the fade
or the 300 units, from the centre of the object's box (`camera::DrawRanges`,
`presets::Graphics::ranged`; `--draw-ranges pc` or `nx` forces the ranges on). The people
and gimmicks have ranges of their own in the same file (`chara`, `gimmick`), not applied.

**Ryza DX: levels of detail.** At the top setting the game draws an object's full model at
any distance (Rasenboden's far mountains, 45,000 to 78,000 units away, are their full
13,752-index models, past both of the setting's distances). Barrel uses the lower models
only below the top `LODDistType`, by the setting's two distances: a guess, as the game's
own switch is not read (the field file's `lod` block is not read by the PC executable at
all: its names are not in it).

**Ryza DX: field objects that move.** A placed object whose container has a motion under
its name (`<model>.g1a` beside `<model>_hi.g1m`: Rasenboden's windmill and boat) is skinned
by its joints, the motion looped in real time (the windmill's 12 s a turn, as the capture's
0.5° a frame); the game draws these with its skinned vertex shader, the rest baked. Escha's
fields do not ask for it (`Placement::moves`).

**Ryza DX: which plant cells are drawn.** A plant cell (`_grass.bin`, one draw a cell) has its
own test, the game's (its plants' update, `0x140a78dd0`): the cell is drawn unless its test box
is wholly outside one of the view's four sides or a far plane at the grass range's end (the
preset's `FarClipEnd`, 15,000 on High), measured along the view from the eye. The game's sixth
plane, the near one, is left out (a cell box behind the eye is mostly outside a side already). The test box is the cell's 500-unit square from the heightmap's lowest height to
its highest plus the cell's tallest plant (the field gives the plants the heightmap header's
range: Rasenboden's 1,766 and 7,000; the title's field 3,000 and 9,600). Hooked in the
recompiled game, its planes are the drawn camera's, re-run every frame. The objects' draw
ranges do not apply to plant cells. With the game's own planes the test reproduces its cells
but for 1 of 1,024 on the title and 2 of 1,122 at the gate; Barrel draws 1 and 3 cells more
than the game.

The gain is small: only the current field's objects were drawn before too (an event's other
fields wait), and from the third-person camera most of a field is in view. On the RP6 at native
1080p: Sector One 179 → 171 scene draws and 131 → 106 shadow draws (shadow 0.73 → 0.65 ms); the
opening's town 168 → 112 scene draws (scene 7.82 → 7.64 ms).

**The distant view after the field.** The scene pass draws the terrain and the field's models
first and the distant view last, with a viewport whose depth range is 1 to 1: its clipping is as
before, every fragment of it lands on the far plane, and with the depth test at less-or-equal and
no depth write it passes only where the depth is still the cleared 1.0, the pixels nothing
covered. The field pass is opaque (no blending), so the frame is the one drawing the sky first
gave, and the sky's layers still blend among themselves in their order; the covered sky is no
longer shaded. Frames are byte-identical to the old order (36 frames: Sector One, the opening,
TOWN_04 and AREA_01_01 from four sides each, and Sector One with MSAA 4×). RP6, Sector One:
native 8.48 → 6.17 ms scene (GPU 9.52 → 7.21 ms), 75% with FSR 1 5.87 → 4.51 ms. The opening's
town at 20 s is unchanged (7.65 ms native): its cost lies elsewhere.

What the scene pass cost there before this was mostly per pixel: in Sector One at native 8.48 ms, 75% 5.87,
50% 5.53, 25% 2.44 (75% to 50% is a step in the tiler's bins, not a fixed cost). The distant view
alone is 2.4 ms of the 8.48 at native (6.04 without it): drawn first, its layered dome meshes
shade the whole screen, most of it then covered by the field.

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

## Draw priority

The game queues each placement's draw (`0x1404d8869` → `0x140338710`) as a 0x60-byte command:
`+0x8` the draw layer, `+0x9` the model's slot, `+0xa` the kind of draw (0, 1, 2: three loops
over the model's pieces), `+0x10` a word from the caller, `+0x12` the placement's `xS` offset
(its `xS1_`/`xS2_`/`xS3_` tag: 0, 1, 2; none: 0), `+0x18` a pointer, `+0x50` the model. Queuing
counts every piece of the model into one of 1,024 buckets (`0x1403387e0`: the queue's `+0x20`
array, its total at `+0x1020`) at the piece's own priority word (`+0x4`) plus that offset, and
the commands are then laid out bucket by bucket (`0x1403380d0`, copying `+0x12` to `+0x2a` of
the sorted entry): a counting sort, so pieces draw in priority order, and an `xS` tag pushes a
whole object's pieces up by one or two places. Where a piece's own priority comes from is not traced yet (not
the mesh groups' spare words, which are 0 or a shadow-pass count and -1), and the viewer does not order by it.

## The field HUD

In free play (a session, no event playing) the viewer draws the game's field HUD over the frame
from its own layout (`crates/render/src/hud.rs`; the layouts in `docs/formats.md`, "Interface
layouts"): layout 625's assignment panel (605), time dial (609) and button list (620), with the
session's date. Checked against a screenshot of the game on August 20th of year 1 ("Left 100",
"8 /20", the year disc "1"): the text after "Left" is the days to the assignment period's last
day, the two big numbers the month and the day. The offsets measured from that screenshot (the
list's rise, the days' number, the text size) and the dial as the tenths of the day still free
are ours (`docs/escha/open.md`). `BARREL_NO_HUD` turns it off.

## The title screen

`barrel --field auto OUT --title ROOT` starts at the title screen (`crates/render/src/title.rs`,
layout 1056 and still 9): "Press any button", then the six entries (arrows or W/S, Enter, Space
or Z to pick, Escape, Backspace or X back). New Game opens the hero choice (`hero.rs`, layout
1023: the two profile cards, left and right switch, confirm asks Yes / No with Yes at the left)
and its Yes starts the new game with that lead, as `--new-game ROOT [--hero logy]`; Load Game opens the save list (`loadlist.rs`, layout 1035) over Barrel's own save
slots (`barrel_game::saves`: `slotNN.save` in `--saves DIR`, by default
`$XDG_DATA_HOME/barrel/escha/saves`), and a saved slot loads as `--load FILE`; Options opens the
Options screen (the graphics page, as from the main menu) and comes back to the title: a change
is saved to the config and Barrel starts again in place (the process re-executes itself, at the
title), since the renderer is built for the settings at its start; Exit Game ends; Dressing Room
and Extras are not there yet. In free play F5 without `--save FILE` writes the saves
folder's next free slot. The screen is drawn with the text layer alone,
before any field is loaded. Offscreen, `BARREL_TITLE_KEYS=confirm,down` presses buttons and
writes the screen as it then stands to OUT (`confirm,confirm,confirm,left,confirm` goes on into
a new game with Escha).

## The main menu

In free play (a field with the story loaded and no event playing) Space, the pad's Start, opens
the main menu (`crates/render/src/menu.rs`, layouts 383, 361 and 374: `docs/formats.md`). Up and
down choose among the ten commands (eight shown, the list scrolling), Back closes it and play
goes on where it stood. Status opens the lead's Status page (`crates/render/src/status.rs`:
level, experience to the next, HP, MP, attack, defence, speed and resistances: the session's
level and experience through the game's growth and experience tables, plus what the lead's
equipment gives; a save with Escha at level 4, 40 experience, in her starting kit shows LV 4,
Next 30, HP 21, MP 32, ATK 10, DEF 9, SPD 8). Basket shows the items
gathered (the save's `item` lines) in the game's item grid, with their pictures and qualities,
the chosen one's name below (`crates/render/src/basket.rs`). Assignment shows the period's
card from `SubjectData.xml` (`crates/render/src/assignment.rs`): each square's task icon, "?"
while the task's event is unseen, and the chosen task's name and description. In the Atelier (map 0) C opens the cauldron (`mix`; offscreen `BARREL_MIX_AT` and
`BARREL_MIX_KEYS`): the recipes the basket can make, then the chosen one's ingredients (the
first that fit) and the quality it would come out at; confirm makes it (`Session::synthesize`:
the materials leave the basket, the item goes in, its days pass). Not yet the game's: plain text
over the desk (the game's synthesis screens, layouts 720 to 851 on `a15_mix.g1t`, not drawn),
no choosing among fitting materials, no skills, traits or count. The menu's Adventure Equip and Battle Equip open the Equip page (`equipping.rs`; offscreen
`BARREL_EQUIP_AT` and `BARREL_EQUIP_KEYS`, the run going on): left / right choose the member, up /
down a slot (weapon, armour, two accessories; a slot with nothing in the basket to fit it grey),
confirm lists the basket items that fit (`ItemRow::fits`) and confirm again puts one on
(`Session::equip`). Checked: Escha's Traditional Staff swapped for a Feather Staff raises her
Status page's ATK from 6 to 23 and SPD from 6 to 16, and an F5 save and reload keeps it
(`member 0 1 0 366,417,-1,-1`, the old staff back in the basket). Not yet the game's: plain text
over the desk, no stat comparison before choosing, one change a visit, both commands the same.
The page is now drawn from the game's equipment screen (layout 404, `equipping.rs`): the
member's Status notebook, the slot panel with what each slot holds, and while an item is being
chosen the values it would give with red and blue arrows (the Feather Staff: ATK 23 and SPD 16,
both up); the fitting items are text lines under the slot panel (the game's grid and card not
drawn).
After a won battle's Result closes, its drops show on a screen of their own (`drops.rs`;
offscreen `BARREL_DROPS_KEYS`, written to OUT-drops.png): the desk, the drops in the Basket
page's grid (tab "Drops"), the chosen one's basket count in the "Possessed" box (layout 706,
where the game has it: 1202, 146) and "OK"; confirm closes it. Checked: an `AREA_01_01` win
(drops 111 and 9) shows both, the second chosen with 1 possessed. What is not yet the game's is
in `drops.rs`'s header (the game's narrower grid with check marks, the item card).
A lost battle (all members down) passes a day in the field's carried save
(`Session::lost_in_save`) and goes straight back to the field where the enemy was touched, the
lead's wipe line on the console. Before it goes back the battle fades to white over 2 s
(`hud::Element::Fill`, a quad of one colour) and the field HUD's assignment panel (605,
`hud::date_panel`) shows alone on the white with the new day for 2 s more (`WIPE_FADE`,
`WIPE_SECONDS`, from the recording); back in the field the lead's line is in the message window
(`Saves_EN/systemMessage/SysMess.xml` row 240 for Escha, "Oww... that was bad. Let's be more
careful next time.", the one the game showed; 241 for Logy; carried as `Carry::notice`) until
Enter (offscreen `NOTICE_SECONDS`, 3). Checked: a level-1 party with nothing equipped walking into
`AREA_01_01`'s first Ore Rat (`--creatures 12 --walk -300,0`, `BARREL_BATTLE_KEYS` confirms)
loses (frame 129), and a save right after reads `today 95` from 94. Not yet the game's (its wipe,
`docs/events.md`): the slow fall, the panel's date turning over (the new day shows at once), and the area map
it lands on (Barrel has none: the touch point instead, the line over the field).
A won round trip, checked 2026-11-15: the copied save in `AREA_01_01` (`--load`, `--walk -300,0`,
`BARREL_BATTLE_KEYS` confirms) touches the first Ore Rat at frame 68, opens its battle on
`BF_POINT19` (group 10), fights it command by command (Logy and Escha attack, the rats throw and
ram), plays the victory scene and music and the result, and at frame 1860 goes back to the field
where she stood, the beaten rat gone.
In a town map (0 to 11) B opens a shop (`shopping.rs`; offscreen `BARREL_SHOP_AT` and
`BARREL_SHOP_KEYS`): the shop with the most wares on sale (`Saves_EN/shop/ShopData.xml` against
the events seen; right after the opening the Homunculus Shop, ten wares from 90 to 15,000
cole), its list with the prices (those the money does not cover grey), left / right for the
basket's items at their worth; confirm buys or sells one (`Session::buy`, `Session::sell`):
from 100 cole the Mutated Taun leaves 10, a Corundum Stone at quality 50 brings 12. Buying is
drawn from the game's shop screen (layout 664: title bar, money box, product list with the lit
row, Price / Total; `shopping.rs`), and so is selling (the grid of 30 cells with the basket's
items and their qualities, the chosen one's worth in Price / Total; arrows move in the grid,
left on its first column and right on its last change back to buying); one deal a
visit, no amounts or stock, which shop stands where not read. Option opens its list: Save writes the game as F5 does (the next free
slot); Return to Title starts the run again at the title, and Load at the title's Load Game list
(so Back there goes to the title, not to the game: not yet the game's). Starting again keeps the
window and builds everything else anew (`view` returns `Ended::Restart` with the arguments less
the start's, `title_args`; `--title-load` opens the title on its list). Change BGM, Options and
the commands without a page only say on the console that they are not there yet.
The menu has its own frame loop (`crates/render/src/screen.rs`, shared with the title), so play
stands still while it is open. Offscreen, `BARREL_MENU_AT=N` opens it at frame N with the buttons
in `BARREL_MENU_KEYS` (as `BARREL_TITLE_KEYS`), writes it to OUT and ends the run.

What is not yet the game's is listed in `menu.rs`'s header: how the chosen button is lit, the
Option rows' wording and text size, the buttons dimmed under the Option list, Escha's figure for
both leads, and no party plates, help line, opening motion or "NEW" badges.

## The world map

The area map's top level (`AreaMap` with `world`): the town and the regions (the places of
kind 0, `world::places`), joined by the world routes R0 to R12, whose numbers are those
places' links (the town's `[0, 3, 1, 2]`: R0 right to the Twilight forest, R3 left, R1 down
to the Southern Ruins, R2 up). The town's world map exit lands there on the town's mark
(`LP_T0`); an area map's Back (Backspace or X; offscreen "back") lands there on the region's
mark. A direction runs her along the route's dots to the next place's mark; confirm takes
the trip from where she started (`Session::travelled`: the field left passes its time, the
route's days pass, an event on the road or on arriving plays, the arrival field being the
region's first sector across its entry's route, or the town's entrance) and lands on the
region's area map at its entry (`LP_` mark), or in the town's entrance at its world map exit.
Offscreen `BARREL_WORLD_KEYS` and `BARREL_AREA_KEYS`, `|` parting them by visit (the first
part for the first scene at that level, and so on). The whole round trip, checked: from the
Town Entrance, `BARREL_WORLD_KEYS='down,confirm|up,confirm'` and `BARREL_AREA_KEYS=back`:
the world map at Colseit, R1 to the Southern Ruins (day 90 to 94), its area map, Back, the
world map at the Southern Ruins, R1 to Colseit (day 98), the Town Entrance at its exit with
the HUD's date at 4/9. Checked: from
the Town Entrance (a save at (5700, 3740), `--walk 300,0`), `down,confirm` on the world map:
R1, four days (90 to 94), the Southern Ruins' area map; `down,back` there: the Ruins
Entrance, then the world map at the Southern Ruins. On the maps she runs the dots without the
field's walls (a mark's collision held her short of it), and no faster than reaches the
next waypoint in a frame (at 5 fps a 100-unit step overshot the 30-unit reach both ways).
Its camera is fitted the same way as the area map's (`colseit-arrival-60hz.mkv` at 1:25, the
cursor on Colseit: its mark, R0's dots east to the Twilight Forest's mark, R1 south, R2 north
to the Old Highway's mark with one dot under its label; 5.6 pixels RMS over 13 points at 720p,
and the 1:12 view on the Southern Ruins lines up with it): 35 degrees from 8016 units at a
point on the ground 357 units north of her, a 21-degree view (`WORLD_CAMERA_*`; the earlier
by-eye 15 degrees from 12000: 16 pixels); she is drawn 3.7 times her size there (`WORLD_MAP_SCALE`), as tall on
screen as on the area map. Its panels (`area_panel::world_elements`, `travel`; the recording
`colseit-arrival-60hz.mkv` at 1:15 and 1:25): the place's name at the top right (637); for a
region the scroll with its description and the "Map" tab over its sectors (the item panel 641's rows and
rules, as on the area map but 38 higher: rows 45 apart in the recording at 1:12, not 642's 50;
the scroll upside down, flat on top and rolled at its foot); for the town no scroll; and, while the cursor stands on another place than
the start, the travel panel (639, hung under the cursor's mark: the mark's screen place plus (-191, 11), refitted at 1:25 with the refitted camera): the trip's days
(`WorldMap::trip`, the events seen shortening routes) and the arrival's month / day in 639's
digit sprites (part 0xcf: cut = digit, cut 12 the "/"; placed as measured on the recording at
1:18 and 1:25, the labels in its tan). The neighbours' labels stand on their route's middle dot,
of two the one nearer the town (1:25 from Colseit: R0's 3 dots → the 2nd, R1's 4 → the 2nd,
R2's 6 → the 3rd; 1:12 from the Southern Ruins: R1's 3rd), farther off it than on an area map:
one under its route 80 higher, one beside it 56 farther in and 11 higher (1080p; the Left one
mirrored, not yet seen; refitted by patch search of the plates' ends), each name 8 higher on
its plate than 623's text node (both maps); the game likely places them in world units. Checked
against the recording's Southern Ruins scroll (the same lines, the same places to within a
few pixels; the game's list rows are a little closer than 642's 50). The marks of regions and the town show as their place is known: the `_on` model (red) once
visited, the `_off` one (olive green in `A15_WMAP.g1t` 0; the Old Highway at 1:25, seen not
visited) before, as the sectors' marks do. The world map's cursor: once the walker leaves her start (60 units), `E3D_WMAP_CUR00` (its
CharaBase row's Directory "Obj": `Data/x64/Obj`) stands at the walker's place at scale 1 and
she waits on her start mark (colseit-arrival 1:16 to 1:25: the crystal leaves her on the
Southern Ruins and goes up R1; its size matches). She waits in her F_WAIT pose,
turned as she started. The cursor plays its one motion on a loop: an object's CharaBase row
(Directory "Obj") names no motion columns, its motion is `<FileHead>.g1a` in its `Motion1`
container (`E3D_WMAP_CUR00_BASE_MOTION1`), asked for as `BASE_MOTION1`. Not yet the game's:
the cursor's float (a little higher in the recording), the routes the
story has not opened yet (R3 west of Colseit is unknown on the user's save),
the "Area Info" button. The sectors listed are the known ones (`area_map::known`); the "Return
to World Map?" question is `question.rs`.

## Graphics settings

The PC release changes its graphics in a separate launcher; Barrel has none. Its graphics
options go on a page of the game's own Options screen (main menu → Option → Options, layout
511, `docs/formats.md`), drawn in that screen's style. The page's choices are
`graphics_options`: render scale (50% to 200%), anti-aliasing (off, MSAA 2x / 4x / 8x, TAA,
FSR 3 native AA), upscaler (bilinear, FSR 1, GSR 1, GSR 2, FSR 3), sharpening, shadows and
texture filtering. A choice that conflicts is greyed out with the help bar's reason: under a
temporal upscaler (GSR 2, FSR 3 below full size) no anti-aliasing can be chosen (it does its
own) and sharpening is fixed; at 100% or more no upscaler can be; native AA needs 100% or
more and no spatial upscaler. Choosing keeps the settings as `RenderSettings::resolved` has
them (a test walks every open choice three deep from each preset), so the page never shows
what the renderer would undo. The occlusion is a row of its own (`Row::Occlusion`, "Ambient Occlusion": On, Off; `ROWS_WITH_OCCLUSION`) for a game that has the pass: Escha's page lists `ROWS` and has none. Ryza has no Options screen yet, so its row is not drawn anywhere; the setting is changed by the config key. Dropping the scale under 100% under native AA makes FSR 3 the
upscaler; raising it to 100% drops GSR 2 and its anti-aliasing.

The page (`options.rs`) is the second of the Options screen's two (the game's own comes first; Down off its "Confirm" or Up off its first row turns to this one, and back the same way). It opens from the menu's Option command (turning the page), the Option → Options row, and the title's Options (offscreen: `BARREL_OPTIONS_AT`
with `BARREL_OPTIONS_KEYS`, drawn to OUT-options.png): the Options screen's desk and its
"Option" title bar (layout 511). The page is the game page's sibling (`Page::elements` over
`Layouts`' shared pieces, `optionpage.rs`): the same paper panel, rules, plate, quill glow, header
and footer bars (footer "Select / Change / Return" from the game's words; header "Graphics:" and
the chosen row's help line, so the header names the page). The eight rows sit in the game's row
slots, the names in the game's ink, values at the game's choice columns: a two-choice row (Display,
Sharpening, Shadows) shows both words as "Read" / "All" are, the chosen one dark with the plate under
it; a longer row shows its value dark; a row with no other open choice shows its value in the game's
grey ink (the unavailable choice's). The game shows no "<" ">" arrows, so none are drawn; the footer's "Change" says it. Then "Default" and "Confirm" on the game's button plates.
Left / right step a row to its next open choice; a row with no other
open choice shows its value grey without arrows; the header bar names the closed
choices with their reason. Checked offscreen: Native AA greys the upscaler and sharpening;
150% greys every upscaler ("Upscalers need a render scale under 100%."). Confirm keeps the choice in Barrel's own config (`~/.config/barrel/graphics.conf`, or
`$XDG_CONFIG_HOME`, or the file `BARREL_CONFIG` names; `key value` lines:
`RenderSettings::to_config`), then the scene starts again where she stands (the carried save)
without the command line's setting options, so the renderer's targets and pipelines are
built anew for the choice. A windowed run with no setting option on its command line starts
from the config; offscreen runs read and write one only when `BARREL_CONFIG` names it (tests
never touch the user's). Checked offscreen: MSAA 2x confirmed into a scratch config, the next
run starting with it. The page's first two rows are the window: "Display" (Window,
Fullscreen: borderless on its screen) and "Window Size" (1280 x 720 to 3840 x 2160, with the
ultrawide 2560 x 1080 and 3440 x 1440; greyed under fullscreen, "Fullscreen takes the screen's
own size."), kept in the same config (`window WxH`, `fullscreen on|off`:
`settings::Display`); the window opens as the config has it (whatever the command line's
setting options) and a change takes at once (`Pump::set_display`, the presenter following the
resize as it follows the user's), with no restart. Checked: a 20-frame windowed run from a
scratch config opened at 1600 x 900 with MSAA 2x. In a window `BARREL_MENU_AT` opens the menu at that frame with `BARREL_MENU_KEYS`
and `BARREL_OPTIONS_KEYS` pressed first (once a run), a scripted check of the page:
`up,confirm,down,down,down,confirm` reaches Option → Options. Checked in a window (KDE,
Wayland): MSAA 2x confirmed → saved, the scene again at her spot with MSAA 2x; 1600 x 900,
85% and FSR 1 confirmed → saved, the scene again at 85% with FSR 1. A live window keeps
its size under KWin on Wayland (`request_inner_size` gave a 1600 x 900 swapchain, then the
compositor's 1280 x 720 came back), so a new size or fullscreen closes the window and opens
it again at the next scene's start (`Pump::pending`, `Pump::reopen`: the same event loop,
after the presenter is gone): checked, 1600 x 900 confirmed, the scene again in a 1600 x 900
window. Not yet: the sound page, the game's own look of the page title and help bar.

## Controllers

With a window, a game controller works through SDL3's gamepad layer (`crates/render/src/pad.rs`;
buttons by place, so south is A on an Xbox pad and cross on a PlayStation one; the first pad
found, looked for again each second while none is open). It drives the same actions as the
keys (`window::Pump`):

| Pad | Menus | Field |
| --- | --- | --- |
| Left stick | steps (leaned past half way) | walks, partway when leaned partway |
| D-pad | steps | walks |
| Right stick | | turns the camera |
| A (south) | confirm | gather; next line in an event |
| B (east) | back | jump |
| X (west) | | strike |
| Y (north) | | running on / off; auto lines in an event |
| LB / RB | | camera out / in |
| Right stick click | | camera back |
| Start | | the main menu in free play; pause the event |
| Back | | save (F5) |

No button quits (Escape). `barrel --field auto /dev/null --pad-test` prints the pad's name,
presses and sticks for ten seconds without a window. Not yet the game's: which button does what
in the field (the game's own key config is not read).

## Motion blends

A character's new motion blends in over the time its command gives (event motions; gestures
0.2 s; her gait, jump and strike 0.15 s), the old motion running on under it. When a switch
comes while a blend is still running, or a gesture moves on to the next division of the same
motion, the new motion blends out of the pose last shown instead, so the pose never jumps:
dropping a half-done blend snapped a joint 30 units in one frame in the opening (character 16,
WALK → F_WAIT → EV_ACT_1 three frames apart). Whether the game blends out of the shown pose
the same way is not read. `BARREL_POSE_JUMPS=N` logs every frame where an event character's
joint moves more than N units, to find pops (a limb in normal motion moves about 5 a frame at
30 fps).

## Advancing lines

In a window an event's line waits at the end of its reading time (`event::reading_time`) for A
or Enter, as the game's message window waits for a button; pressed earlier, A ends the line at
once and stops its voice. Auto mode (T, or Y on a pad, while an event plays) lets each line go
on by itself once its reading time is up and its voice has finished. Space still pauses the
whole event. Offscreen renders keep the timed
schedule. Not yet the game's: the reading time a line is held before it waits (the game shows
the whole line at once and waits; its text speed and auto mode are not read).

## A draw's textures

A model draw binds three single-texture sets: 0 its colour, 2 its mask (or, for Ryza's field
models, the normal map; for her, the toon ramp) and 3 its emission map (`Draw::emission`, the
material's kind-19 texture), white where a draw has none. The fragment program reads set 3
only under flag 32768, so a game that never sets it (Escha) draws as before.

## The sun flare (Ryza)

After the tone curve the game washes the frame with a warm flare along the top and a paraffin
tint where the sun is not blocked (its pass 73 and 74; `render::flare`, `barrel_ryza::flare`,
`post.frag`'s `wash`). The sun's visibility is a one-pixel value (two R16 targets, drawn from
the kept one and kept again) that moves a sixtieth a frame toward 1 while a reference point near
the player is in the first cascade's shadow and toward 0 while it is lit (`sunvis.frag`). The
point is the camera's focus (the player at her look height) in the first cascade's map, uv and
depth; Barrel's cascade is the game's mirrored, so the dump turns its uv back. The wash is read
from a small uniform buffer written every frame with `cmd_update_buffer` (no settings: off, which
is Escha's case). The tilt of the screen is the angle of the sun's direction on screen from
vertical with the uv square's aspect (`atan(-x / (y * aspect))`, fitted to three views). The
numbers are the environment template's (`saves/fieldmap/cameraparam/env/envtemplate.xml`, block
`flare_paraffin`, a row per time and weather; `barrel_ryza::flare::look`), read by the
executable's key names: it spells two `..._corve_center`, and the file's `Paraffin_amount` is not
its key, so the paraffin amount stays the default 0.2 (as the game's constants show). The game
draws the wash twice, the scene at `intensity` and the sky (depth at the far plane) at
`sky_intensity`; Barrel's post pass has no depth yet, so the sky takes `intensity` too.

## The forward pass over Ryza's characters

After the scene's models (and the Escha outlines' place), the draws that take the toon ramp
(flag 128) are drawn again with a pipeline of their own (`Renderer::forward_pipeline`: front
faces culled, source-alpha blended, depth tested less or equal and not written) and
`model.vert`/`model.frag`'s flag 1 << 25: the hull of the game's forward pass, inflated along the
clip-space normal (docs/shading.md, "Her forward passes"), its colour her tile plus the specular
highlights, at the game's units times the scene's exposure, blended by the tile's alpha. It runs
for the finished scene only (`dump::Stop::outlines`), so the board's `lit` stage does not have it.
A character's vertex colour alpha (0.4 on her) is kept for it; the rest of her vertex colour is
white as the field models'. Her specular ambient (the sheen on her metal) is in the main pass:
flag 1 << 24 with 128, her specular map in set 4 and `look.chara_spec` (the cube map's scale
times its tint from the set's `indirectChara_` block, and the exposure in w).

**Which passes run (Ryza's graphics settings).** The normals and depth target (`ao.rs`,
`Occlusion::on`) is made when the ambient occlusion, the reflections or the depth of field is on
(the game runs reflections and depth of field with SSAO off): by the geometry pre-pass with the
occlusion, else from the scene pass (above); the occlusion program itself
(`RenderSettings::occlusion`) only with the occlusion, else the map reads 1. The reflections'
march takes the game's level (`reflect::Rlr`, `rlrpreset.xml`: 256 / 128 / 64 steps of 1 / 2 / 4
half-size pixels, push constants of the march; the game's `vReflParam.x` is the step times the
pixels and `vSampleEdgeFadeParam.x` two over the steps, as `gs-q2`, `gs-q1`, `gs-q0` show). Bloom off
sends the glow's rate 0 (the game's Low draws no bloom or star passes). The game's shafts are
not built. TAA follows the game's `Taa`; texture quality is `texture_bias`; Linear filtering is
anisotropy 1, Anisotropic 16. These apply when the player gave any of the game's settings
(`barrel_ryza::graphics::Settings::given`).

## The scene pass's three parts

The field's models are drawn in three parts (`ScenePass`, `Renderer::scene_pipelines`). First the
draws whose textures are opaque (`Texture::opaque`: no texel of any level under full alpha), with
`model.frag`'s alpha test compiled out (`ALPHA_TEST`), so the GPU keeps its early depth test (on
a tiler, its coarse depth: Adreno's LRZ). Then the alpha-tested draws' depth alone
(`DEPTH_ONLY`: the texture's alpha against the reference, no colour written), then their full
program over that depth, tested less or equal and not written: without a depth write the early
test holds despite the discard, and each pixel under many leaves is lit once. `model.vert`'s
position is `invariant` so the two passes agree. At the Rasenboden gate (1080p, the game's Low)
the trees' cards were 5.2 of the scene's 9.0 million pixel-shader runs; on the PC the scene stage
went from 1.62 to 1.11 ms, the image the same but for 54 pixels (2026-11-19). `--timings` prints
the scene pass's pixel and vertex shader runs where the device has pipeline statistics, and its
stage `reflect + water` is what a frame with water does after its opaque scene (the normals, the
reflections, the copy and the water's part). Per part at that gate: 1.21 million runs of the
whole program for the opaque draws, 4.94 million of the test alone, 1.05 million of the whole
program for the tested draws.

Each part's pipelines are made for the field's models and for the others (`model.frag`'s
`KIND`, by `Draw::field`): the program's paths for the distant view, the characters' hull and
the other kind are left out, so its registers are its own path's. On RADV the program for
every draw had 7,018 instructions and 72 registers; the two kinds' are 4,604 and 60, and 1,903
and 48. Turnip for an Adreno 740 (`work/ir3-run.sh`, the freedreno drm-shim, nothing drawn):
the field models' program about 5,100 instructions and 19 vec4 registers (10 waves of 16), the
other models' about 1,900 and 12 (8 waves); the tested draws' depth alone 11 and 3 (16 waves).
The HALF build (below) leaves the field models' at 19 registers (their peak is the shadows',
kept 32-bit) and takes the others' to 9 (10 waves).

The shadow maps' casters go the same way: those with opaque textures first, through
`shadow_opaque_pipelines` (`shadow_pipeline` and `shadow_cascade_pipeline` without a fragment
program: depth alone, the cheapest a tiler writes), then the alpha-tested ones through the
pipelines with the test. Depth alone does not depend on the order, so the maps are the same; on
the PC the gate's shadow stage went from 0.905 to 0.781 ms. Each layer's pass covers only its
map's corner of the layer (the render area: a tiler bins and stores that alone), but for the
layer's first clear.

For a tiler what the scene pass keeps a pixel decides its bins: at the game's Low with
reflections, the scene image (B10G11R11, 4 bytes), depth (4), the normals (R16G16, 4) and the
reflections' two inputs (`reflect::TARGET_FORMATS`, 4 each: 8-bit sRGB and B10G11R11, where
two 16-bit floats were 16), 20 bytes where it was 28.

The model program is built twice (`build.rs`): as written, and with `-DHALF` (`FRAG_HALF`), its
colour arithmetic marked mediump (RelaxedPrecision), which Turnip runs at 16 bits (Adreno's
half floats: twice the rate, half the registers). Positions, texture coordinates, the
derivative tangent frames, the shadows and the highlights' powers stay 32-bit. RADV ignores the
marking, so on the PC the two draw the same pixels. For now `BARREL_HALF=1` picks it, to be
timed and compared on a handheld before it becomes any setting's default.

## Foliage

`RenderSettings::foliage` (full, reduced, least; config `foliage`) is how much of a field's
plants a game draws, named for the game's own grass presets (Ryza: Middle, Low, VeryLow, with
their density and draw range). The desktop presets take full, the handheld ones reduced.

**Skill scenes in battle** (on; `BARREL_SKILL_SCENES=0` leaves them out). A member's action plays (its command by
`auto_battle::Tables::command_of`: 0 the attack, 1 to 4 the skills, 8 and 9 the supports) `Skill_PARTY_<member>_<command>` through `barrel_escha::skill_scene::play`.
The actor's motion lengths come from its clips and `CharaDiv` divisions, and the target is
anchored by the way from the actor to it. The step lasts as long as the scene. Each frame the
actor's body takes the scene's place, facing and motion (its clip, in the `CharaDiv` division the scene names) and the camera takes the scene's eye and
target; both go back when the scene ends.

A test battle run with `--load SAVE` also draws the save's field (its town people and props)
around the battle stage. A field encounter starts its battle without it, carrying the save
instead. Offscreen battle renders are made without `--load` (`BARREL_BATTLE_SAVE=FILE` gives the party from a save instead), or the field shows through: in
BF_POINT19 it put the scene camera inside AREA_01_01's trees.

## Movies

`barrel_render::movie::MovieLayer` shows a movie picture: its I420 planes copied each frame
into a persistent staging buffer, uploaded to three `R8_UNORM` images (3 MB a 1080p picture,
half of RGBA) and turned into RGB in `movie.frag` (limited range, BT.709: the files carry no
colour description and Media Foundation takes HD video without one as BT.709). Scaled to fit
and centred, black around. Escha's startup plays `opening.wmv` after the logos with it
(`play/movie.rs`): the picture follows the sound's clock (samples the device has played), any
button skips. Offscreen, `BARREL_SPLASH_AT` past the logos and the 0.15 s gap renders a movie
picture: at 20 s it is within 1 level of ffmpeg's BT.709 conversion (mean 0.05; BT.601 would be
up to 12 off).

## Ryza's title screen

`barrel ... INSTALL FIELD OUT.png --title [--title-at S]` (or `BARREL_TITLE_AT=S`) draws Atelier Ryza
DX's title as it stands with no input, 2D only: `barrel_ryza::title::Title` lays out the logo, the
"press any button" prompt, the version string with its outline and the copyright line, and each
frame `quads(t, screen)` gives the quads of a `render::text::Layer` (font atlas = `FONT`, the
`a21_title.g1t` atlas = `UI`), so the 2D draw trace records them. `t` is the seconds into the
title; `--title-at` is the moment of Barrel's first frame, then 60 Hz offscreen or the window's
clock. The field named on the command line is ignored: `--title` always loads `title::FIELD`
(`s19_l_highway_03`) and draws it, as the backdrop, behind the layer.

Draw order, as the capture has it: logo, prompt, the version text 9 times black at half alpha
(a 3 x 3 grid of offsets of `bold_size` 1 layout pixel, columns first) then once white, the
copyright line. Everything comes from the archives (`docs/formats.md`, "Ryza's title layout")
and the executable's font table (`barrel_ryza::font`, 1,774 glyphs at a fixed address), except
the constants `title.rs` names: `TEXT_TOP` (the text line's top against its node, fitted),
`OUTLINE_ALPHA` (read from the capture's `materialColor`), `PATCH_VERSION` ("1.02", the capture's
build; the executable's patch table lists the strings but the level in force is not read) and
`font::ATLAS_SIZE` (36, fitted: a size-11 glyph at 1080 lines is 16.5 pixels).

The prompt's opacity is the slide-in's colour curve (it replaces the plain fade-in bound beside
it) times the blink: `anm_common_juvenile_blink` runs 255 to 127 over 0.75 s with an in-cubic
ease; the capture's pulse is symmetric about its dip, so it plays forward and then back, a dip
every 1.5 s starting at 0.75 s, one dip per period although the layout also binds a reversed
copy to the image (the capture shows a single dip; not explained). The prompt appears 2 s in and
slides up 30 layout pixels over 0.5 s.

Parity: `barrel-parity diff` on `ryza-recomp`'s `title-cycle` capture, frames 1400 to 1800, 0
mismatches (`docs/ryza/checks-title.txt`). Frame N maps to `--title-at (N - 1140) / 60`, except
frames 1700 and 1800 (9.30 and 10.9667 instead of 9.3333 and 11.0): the capture's clock drifts
from 60 Hz by a couple of frames by then, which the pulse's phase shows. Two things in the
diff tool serve this screen: Ryza's 2D pixel shader multiplies by `materialColor` (the outline's
black half alpha; the hidden autosave notice drawn at alpha 0), and quads wholly off the screen
draw nothing (sprites parked at x = -1338).

Not drawn yet: the fade from black in the first second of the title (a full-screen fill drawn
by a separate shader, frames 1140 to 1275 in the capture) and the menu behind the prompt.

### The backdrop

What the capture's G-buffer holds (frame 1400, 995 draws) was worked out with `barrel-parity`:

- **The field** is `s19_l_highway_03`, not `event_titleback` (the name suggests it; its terrain and
  textures do not match the capture) and not the `charatest` set the lighting names. The
  texture index (`barrel-parity textures`, which now also reads the textures packed in the
  archives' `.elixir.gz` containers, so the field packs' hashes resolve) names 28 of the
  scene's textures that sit in this field's pack alone, and Barrel's field draws reappear with
  the same textures and index counts (one 13,752-index draw, 84 draws of 2,841 indices ...).
- **The camera** is fixed over every captured frame (300 to 1900): from the first G-buffer draw's
  `mW2P`, eye (29789.2, 6485.0, 41557.7), forward (0.4045, 0.1053, -0.9085) in the game's world,
  45 degrees vertical, 16:9, near 50, far 300000 (the projection's depth terms give 300073: the
  far is a 5-digit division of float noise, taken as 300000). `title::CAMERA` holds it;
  `play::look_for` takes the clip planes it fixes in place of its distance rule. In render
  space (x mirrored) Barrel's `view_proj` equals the game's `mW2P` with its x column and y row
  negated: `diff3d` max difference 0.03 on entries up to 60,000.
- **The characters**, from the skinned draws' bone palettes (33 bones; palette bone 0's
  translation): Ryza `pc00a` (29812.7, 6383.6, 41279.1), `pc02a` (30696.9, 6436.2, 40649.8) and
  `pc03a` (30803.4, 6536.4, 40656.6), within 0.2 units of those at every frame from 1380 to 1900,
  the palettes changing each frame (an idle motion). They stand 280 to 900 units before the
  camera. With them: `npc00a`, `npc02a`, `npc34a`, the `btl_obj_03_blackbox`, `ev_obj_28_fruit`
  and the `pc0x_wep_01` weapons. **Drawn** (`play/actors.rs`): Ryza, Lent and Tao and the three weapons from the
  event (below); not yet the NPCs, blackbox and fruit; the charatest light is
  not decoded.
- **The scene is an event**: the game reads `\event\event_jp\tt01\event_tt01_010.ebd` at
  capture frame 1152, then the field at 1155 and the weapons from 1155 to 1182 (found with
  ryza-recomp's `RYZA_TRACE_FILES`, which now logs every read, and `barrel-tool pak-reads`,
  which names the archive file each read falls in). The event's actions: 3 switches the map
  (`q2 18`), 4 adds the characters (`q5`, slots 0, 2, 3, 216 ...), 7 to 9 place Ryza, Lent and
  Tao (`s2` position, `s13` turn, `s34` motion), 10 and 11 set the camera (`t25`), 12 is
  Ryza's action (motions 120, 92510, then back to 92511), 13 and 14 turn Tao's head (`s90`).
  Ryza: `s2 0 0 0 0 29797 6381 41302`, `s13 0 0 -233 0`, `s34 0 92511 0 0 0`, face `s88 0 -7`.
  Lent: `s2 2 0 0 0 30694 6514 40693`, `s13 2 0 -184 0`, `s34 2 2190 0 0 0`. Tao:
  `s2 3 0 0 0 30806 6535 40682`, `s13 3 0 -192 0`, `s34 3 12301 0 0 0`. The motions are in the
  shared sets, not the characters' own: `cf01_92511`, `cf01_92510`, `cm00` or `cm01` `_02190`,
  `cm01` or `cf01` `_12301` (which set: `charabase.xml`'s `motset1`). `tt01_020` to `_050` are
  the title's other scenes.
- **The cast** is read from the event's stage, not placed by hand: slot n is row n of
  `chara.xml` (0 `PC00A`, 2 `PC02A`, 3 `PC03A`), its position and turn are the event's (no
  ground snap; the 78 units Lent's palette sits below the event's y are her motion's hip, not a
  snap), its motion `s34` (translated as Escha's `s5`; `p6`, the random wait, as `p1` for its
  shortest, which keeps Ryza's action 12 past 10 s, where the capture shows her still in 92511
  since stage 0). A motion is looked for in `motset0`'s folder, then `motset1`'s
  (`barrel_ryza::chara::motion_sets`). Three measured rules, each exact against the capture:
  the event's clock starts 35 ticks (`EVENT_START`, 0.583 s) into the title (fitted to frames
  1400, 1500 and 1700: stage tick = capture frame - 1140 - 35); a motion of a shared set moves
  the hip's height only in proportion to the model's own hip (model hip bind y / the set's
  `SkeletonBase`: Tao's 80.1 against CM01's 98 stands 17.9 lower; x and z are the motion's); a
  motion plays looped. Submeshes whose normal is a Float4 (the cloth parts, 10 of Ryza's 25) were
  not drawn before and are now (rigid on one joint as Barrel draws such submeshes).
- **The weapons** (slots 216, 226 and 231: rows `PC00_WEP_01`, `PC02_WEP_01`, `PC03_WEP_01`)
  are three more characters the event's `q5` adds
  (`{slot, 0, 0, 0, x, y, z, rx, ry, rz, 0, 0, 2}`), lying in the scene, not held, with no
  motion. Their model is `character\pc0x_wep_01\pc0x_wep_01` (the entries `<TAG>` and
  `<TAG>_default`, without a person's `_MODEL`), drawn in its bind pose at the `q5`'s position
  with the matrix `Rz Ry Rx` of its three angles in degrees, which `diff3d` finds the game's
  world matrices to within 0.002 (the unit test `event::props_turn_as_the_games_matrices`
  holds all three). Shared `Place` and `Turn` steps keep only a yaw and Ryza's `q5` is not
  carried over to them, so `play` reads the position and angles from the event itself
  (`event::added`) and holds them fixed. The second weapon's 630-index part is skinned, drawn
  from the same bind pose.
- **Pose against the capture** (`diff3d`'s `pose` lines): Lent and Tao within 0.012 units and
  0.0000 (rotation, matrix entries) at frames 1400, 1500 and 1700; Ryza's body the same, her head
  (joint 6 and what hangs from it) 13.7 units and 0.126 off at every frame: the game turns it
  toward the camera (`s88 0 -7`, look target; not done). Hair and skirt joints no motion moves are
  the game's cloth simulation and are left out of the comparison.
- **Lit** by the field's own environment (as every field): the capture's characters use the
  `charatest` probes, which a field draw does not.
- **Draw ranges are off** in the title as in every field at the top object-distance setting
  ("Ryza DX: object draw ranges"); the boot screens leave the scene out until the title.

`diff3d` at frame 1400 (see `docs/parity.md`): 611 of 995 game draws matched against 733 Barrel
draws, from 13 with the wrong field. Left: the 348 grass cards (Barrel draws grass as its own
instanced pass, not in the trace), 32 terrain patches (different splits), the weapons (4).

### The title's menu

A button at the prompt brings up the menu (`crates/ryza/src/title_menu.rs`; the title's code is
`0x1400d6400`-`0x1400dc900`, its sequencer's states are set through `0x1400db160`). Frames below
count from the button's frame P.

- **The button** (`0x1400d7640`) counts only once the prompt's slide-in has ended. The prompt's
  pulse stops at its value of the frame before, and its `in` animation (the common fade-in,
  bound at half speed: 30 frames) plays backwards from two steps in: opacity
  `1 - (k + 2) / 30` of that value on frame P + k. A wait of 0.25 s follows (`0x1400d80d0`: a
  32-bit sum of frame times, the first step on P itself, going on once past 0.25), so the prompt
  is last drawn on P + 14 and the menu's first frame is M = P + 15.
- **The items** (`0x1400dc440`) are the six nodes `menu/new_game`, `load_game`, `setting`,
  `extra`, `episode`, `end_game`. New Game, Settings and End always show; Load Game with a save;
  Extra once unlocked; Episodes when any of the five is owned. With n showing, the k-th sits at
  `(6 s + b) / 2 - n s / 2 + k s` within the menu node, b being New Game's layout y and s the
  distance to Load Game's (four items: 648.75, 714.75, 780.75, 846.75 on 1080 lines; five:
  from 615.75). Each is drawn at its picture's own size (Load Game's `size_wh` is not used) and
  `offset_uvwh` moves the picture in the atlas by three atlas pixels per unit. The cursor starts
  on Load Game with a save, else on New Game.
- **The entrance** (`0x1400d8220`): every item plays `up2`, the common slide-in from below at
  speed 1.5, item k starting 0.08 s of animation time after the one above: on frame M - 1 + e
  item k is `1.5 e / 60 - 0.08 k` seconds in. When the last has ended (`0x1400d6db0`; e = 30 for
  four items, 33 for five) the next frame shows the cursor, the selected item's second picture
  and its pulse (the prompt's pulse shape, counted from 2 on that frame).
- **The cursor** is three pictures of `a21_balloon_frame.g1t` (two props and a core) drawn
  before the items. Its `in` plays the common slide-in from the left (30 frames) and a full turn
  about the upright axis at speed 3 (20 frames, 18 degrees a frame) about the cursor node's own
  origin, seen in perspective from the layout's centre. The two props then turn against each
  other 1.5 degrees a frame (`anm_rotate`, 240 frames); at the loop's end the frame shows what
  is past the end and the next starts from 0, so the first step is shown twice running.
- **Moving**: up or down selects the neighbour on the press's frame (its pulse starts over, the
  old item is plain at once) and the cursor's line travels there in six frames, whatever the
  distance. A press at either end goes round to the other. A held direction repeats after 37
  frames and then every 15, and a repeat stops at the ends.

Barrel takes the same from a window (Enter, the arrows, the pad's buttons standing for them) or
from `--title-press F`, `--title-keys F:down:N,...` and `--title-saves` (title frames). The
captures are ryza-recomp's `title-menu`, `title-menu-wrap` and `title-menu-saves` scenarios;
with a save the boot is 332 frames shorter (the title starts at frame 543, not 875), and the
prompt's pulse keeps its place relative to the title's start.

### The title's sound

`barrel_ryza::sound` holds the title's cues as data (`title_cues`): the music from title frame
18 (the `bgm` container's stream at offset 151456224; it loops from the clip's own loop start)
and one voice clip from frame 106 (the `voice` container's stream at 94885392; played once),
the frames at which a run of the game starts the two sounds, 60 a second from the title's first
frame (after the boot screens, `boot::TITLE_START` in). They are a stand-in for the game's own
choice of streams. `TitleSound::tick` starts each cue when the title's clock reaches its frame,
through `barrel_audio::Mixer` and `Output` in a window; an offscreen run opens no device. With
`BARREL_TRACE_CUES=<file>` set, each cue started appends a line `<frame> <kind> <file> <offset>`
(`18 music bgm.ktsl2stbin 151456224`); an offscreen run at `--title-at T` logs every cue at or
before T's frame, so the log can be compared with the original's.

### The boot screens

`--boot [--boot-at S]` (or `BARREL_BOOT_AT=S`; implies `--title`) plays what comes before the
title, `barrel_ryza::boot::Boot::quads(t, screen)` on the same text layer (it holds the font, the
title atlas and six more textures, `boot::TEXTURES`), then hands over to the title at
`boot::TITLE_START` (14.4 s; the screen is black from the window's end until then). In order, as
the `title-cycle` capture has them (capture frame 278 is t = 0, so frame N shows at
`--boot-at (N - 278) / 60`):

1. The "Creating system data..." notice (`a21_saveload.g1t`, frames to 398; the capture starts
   inside it): the saving icon (a 5-entry picture list stepping every `flick_time` 0.3 s), 21 letter
   images hopping (`anm_hopping`: lift 15, a full turn and a shrink to 0.6 over 24 frames, each
   letter `delay` later, the control at speed 1.3; the dots only lift) and the pen
   (`anm_penmove`, speed 1.33, bobbing and then writing; its loop restarts on the first 60 Hz tick
   after its period, see `boot::loop_time`). It fades out (`anm_common_fade_out`) from frame 383.
2. The Koei Tecmo logo and the Gust logo (`logo_kt.g1t`, `logo_gust.g1t`): one full-screen picture
   each, 0.5 s fade in, 119 frames hold, 0.5 s out (the capture's `fAlphaRate` ramps 1/30 a
   frame); Koei Tecmo from frame 437, Gust from 616.
3. The loading screen is the autosave window (`uil_a21_notification_display.xml`, sentence
   `STR_UI_0896` = string 6620033 of `str_ui.xml`; the string number is `STR_UI_BASE` plus the
   `STR_UI` number): a nine-slice of `gen_a21_mmap03_info_base` (`edge_margin` left, top, right,
   bottom; the middle tiles repeat and the last is cropped), the sentence wrapped to
   `line_char_length` x `font_size` and the 8-orb spinner (orb i at 45 i degrees; each has a glow
   copy over it lit by `anm_loading_icon_eff_i`, speed 0.8). The window slides in (`anm_common_slide_in_bottom`) from frame 795; at 1065 the spinner fades out (the fade at speed 0.3) and the
   window slides back out from 1077 to 1107. Opacities are bytes multiplied down the node tree and
   truncated (`boot::byte`). Barrel loads nothing, so the length (about 5.2 s) is the capture's.

The event turns Ryza's head: `s88 0 -7 0 0` holds it 7 degrees about the model's x axis, in
the head joint's bind frame, over her motion (`actors::HEAD_TURN`; with it her bones are
within 0.008 of the capture's, without 13.6 off). The command is read from the event's own
words (`event::schedule_drawn_taken`): the steps carry Escha's words for what is carried over.

The title comes up black and fades in: its event's `u49 1 2 0` and, 55 frames on, `u49 1 3
150` (docs/events.md) drive a black quad over everything, 1/90 clearer each frame from capture
frame 929 and gone at 1019. Barrel draws it from the event's stage (`play::actors::cover`), as
a point of the Gust logo's texture coloured black.

The game's own sequence (found with the answer key's hooks): one object, stepped each frame
from `0x1404db7ac`, whose state `0x1400db160` sets: 1 the notice (frame 11 of the capture's
run), 2 and then 8 polled every 4 frames while the first loading runs (136 to 169), 4 the Koei
Tecmo logo (173), 5 the Gust logo (353), 6 the loading window (533), 7 and 10 (854), 11 the
title (877). A logo fades in, then `0x1400db440` runs a timer against a pair of floats for
that screen in the table at `0x140e4c838` (when a button may end it early, how long it holds:
0 and 2.0 s for both logos; the first entry, 1.0 and 4.0, is for a screen this run does not
show), then it fades out over the 0.5 s at `0x140de75c8`. So `LOGO_FADE` and `LOGO_HOLD` are
the game's; the notice's end, the window's stay and the title's start wait on loading, not on
timers, and stay the capture's. Barrel does not yet let a button end a logo early.

Read from the capture, not from the game's code (each named in `boot.rs`): `CAPTURE_START` (12:
the notice's start), the schedule's capture frames (notice fade at 118, logos at 172 and 352,
window at 532, getOut at 802, the title at 875: the frames of the current capture, 265 lower
than the first capture's that the text above cites, less the two frames the exact clock moved
them; the capture made since timeouts end on the virtual clock starts the boot a frame later
and the title at the same frame), `LINE_STEP` (22 layout lines between the
sentence's lines), `TEXT_TOP` (-4, the same as the title's version string) and `WRAP_FACTOR`.
The loops (the pen, the glows, the title prompt's pulse) follow the game's own rule, a 32-bit
float sum restarted at the loop's length (`boot::loop_time`): nothing fitted. The prompt's
pulse counts from a frame fixed after the notice's start (`title::PULSE_FROM`, 864 within its
91-frame loop), not from the title's first frame: it moved with the boot.

Parity (`docs/ryza/checks-boot.txt`, local): 0 mismatches at capture frames 300, 370, 390
(notice), 445 and 500 (Koei Tecmo), 625 and 780 (Gust), 800, 900 and 1090 (the window sliding in,
steady and out) and 1400 (the title after the boot), and also 305 to 395, 440, 590 to 640, 765 to
790, 825 to 1105 as tried. The title rows are unchanged. Two shared changes serve it: the trace writer
(`render::trace::quads`) lists a turned quad as the capture reads one (the picture's corner
nearest the screen's top left names (u0, v0); quads under `EDGE_ON` pixels across are left as
they were), and `barrel-parity` settles ties between a turned rectangle's equidistant corners
(a 45 degree diamond's left and top, where the capture's float noise picks either) when
`PARITY_TURNED_TIES=left` is set; off by default because Escha's diamonds are matched to the
noise as it falls (the checks-boot rows set it). Escha's 15 2D rows stay 0 (run with the
change).

### Ryza DX: objects behind the field's occluders

The field's `<field>_le.koc` quads are also the game's occluders for placed objects
(`barrel_ryza::occluders`; the walker's walls use the same file). Each frame the game keeps
the quads whose triangles (0,1,2) or (0,2,3) touch the view, sorts them by their nearest
corner's depth, gives each five planes (its face, inside away from the eye, and one through
the eye and each edge, the quad's centre inside) and drops a quad wholly inside an earlier
one. An object's query (`0x140a39fc0`) walks the kept quads: it stops, object drawn, at the
first quad whose nearest corner is deeper than the object's nearest point; an object whose
box lies strictly inside all five planes of a quad is hidden. The box is one oriented box per
placed object (the placement's transform of the model's own bounds, `field::PlacedBox`),
so all its draws, levels of detail included, go together. Only placed field objects take
the test (not plants, terrain, sky or characters); the skyline stand-in stays. `--no-occluders`
draws them all.

Rasenboden's answer key (`load-rasen`): the gate's Barrel-only field parts go from 78 to 38
and the title is unchanged (963 matched, no `_le.koc` use on the title). Gate matched goes
859 to 858: object 1147 (a stump, class 2) is strictly inside two kept quads (the nearest, q7,
by 316 units at its least plane) yet the game draws it; not explained. Dropping the frustum
step, the culling of covered quads or the depth stop changes nothing here, and the stored
bounds and the drawn vertices' bounds give the same box. About 38 remain: the large cliffs and
mountain (objects 8, 78, 95, 96, 586, 587) are not inside any single quad's planes by 20 to
thousands of units, and the far trees (785 to 1183).
