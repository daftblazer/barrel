# Ryza DX: open problems

Everything Barrel is stuck on for Atelier Ryza DX, has not worked out yet, or knows to be wrong
(addresses are in `Atelier_Ryza_DX.exe`, which is not DRM-wrapped). Same rules as
`docs/escha/open.md`; problems in the shared engine are in `docs/open.md`.

The original Ryza DX is never run through Steam without asking first (its playtime and
achievements are the player's own, and the DX version has not been played there).

## Fields

- **Ryza 3 DX's placement lists** read neither as Escha's nor as Ryza DX's (they fail at the
  first type's instance count): for when Ryza 3 starts (`docs/formats.md`, "Placement lists").
- **Field lighting.** `barrel-ryza --time T --weather W` lights a field with its environment
  set: the cube map's light times `mainAmbientLight` as the ambient, the sun and the second
  light as directional lights. Stand-ins, not read from the game: (1) the angles' convention
  (azimuth from +z toward +x, elevation up; the noon sun may come from the wrong side);
  (2) one exposure mapping the light on an upward surface over 22 to `fMiddleGray`, capped
  at the default override's `vExposureRange` top (1.2), with a tone curve whose white point is
  the set's `fLumiMax` (below), all fitted together against the game's noon still at the
  garden gate (`work/ryza-ref/rasen-gate-noon.png`: grey percentiles 25 / 50 / 90 come out
  58 / 69 / 166 against 55 / 70 / 219, the sky darker than the still's). Our light is taken
  in the game's units as is (an earlier fit had it at 0.75 of them). The sets' own range (0.9
  to 2.4) would leave evening and night uncapped, and noon's gamma is 1.8 against 2.2: the
  game's tone chain is not read. Morning's set has ten times noon's ambient and is
  exposed at 0.30 uncapped; night comes out too bright and green (the moon's colour 1, 4, 5);
  its sunlit ground clipped until a tone curve went on the finished frame (`post.frag`, an
  extended Reinhard with the set's `fLumiMax` as white, Rasenboden's noon 4.3: the
  still's sunlit grass 182, 216, 69, ours 168, 214, 47 instead of 241, 254, 22; the 90th
  percentile drops to about 175 against the still's 219, its sky being brighter). Ryza now draws its scene into a float target
  (`RenderSettings::hdr_scene`, `B10G11R11_UFLOAT`, the same size as 8-bit RGBA; TAA and the
  upscalers' targets follow it; Escha keeps 8 bits): error 183 to 163 at the same exposure,
  the sky 98, 161, 157 to 87, 199, 199 against the still's 172, 218, 233, the 90th
  percentile 168. The clouds are lit by the light on an upward surface (a stand-in for
  the dome's own lighting, docs/shading.md): at the gate's noon they match the still's
  (ten regions' RGB error 197 to 151). The clear sky between them comes out 163, 201, 199 against the still's
  172, 218, 233 with the sky plane's light scaled by a fitted 2.5 (`SKY_SCALE`; the
  game's own scale is not read); its green and blue stay put whatever the scale or the path
  length, so the remaining gap is in the sky's hue, not its brightness. The shade's colour: the default override's
  `indirectStage_IDLScalerOfAmbientDiffuseMap` 2.5 × `..ColorRGB` (1.642, 1.978, 4.0) tints
  the stage's ambient light map blue and `indirectStage_AmbientLightScale` 0 leaves the set's
  ambient colour out; the field has one light map for all times (`_ad`) and the override no
  per-set values (`env.bin` holds none of them; its words 0x2c4 on are SSAO's, then a per-set
  saturation scale as at 0x44; not applied, see docs/formats.md), so `barrel-ryza` keeps the set's
  colour and tints it by the tint's hue (over its luma) to the power 0.4, fitted over six
  shaded regions of the gate's noon still (RGB error 77; the full hue 118, none 95): tree
  shade 62, 58, 49 against the still's 45, 64, 70, rocks 75, 82, 104 against 53, 77, 103.
  Ours stays lighter and less green in the shade; (3) the ambient's shape: the game
  adds `ambLit` to the cube map's light times `vAmbParam.x × vAmbDifScl` (docs/shading.md,
  the lighting pass), `barrel-ryza` multiplies the cube map by the 0x484 colour, with `indirectStage_`'s scale unplaced
  (the file's blocks shift against the executable's structure after the HDR block). The
  field's shaders (`.g1s`) and a reference frame would settle all three.
- **Model lighting material.** Ryza's models' G1MG section 0x10001 reads as in Escha but holds
  diffuse 0.8 and ambient 0 (`barrel g1m-materials`): their shaders (`PBB-N`, `PBB-EmN`, ...)
  take the ambient from the environment, not `cAmb`. `barrel-ryza` gives every draw the full
  material until those shaders are read.
- **Emissive and odd materials.** `PBB-EmN` submeshes (the attic wall's windows) have an
  emission mask (texture kind 19) and flicker parameters (`emFrq`, `emRange`, `emStart`), not
  drawn. Not drawn yet from the G-buffer model
  (docs/shading.md, "Ryza DX: the deferred G-buffer"; normal maps are drawn now, their
  green's sign unchecked): the specular map (metal flag, specular amount, roughness), and the lighting pass (`pbb-*_cll.g1s`). Attic view inside: `--look-at 3905,2250,3914 --distance 300 --pitch 10`.
- **Terrain against the models.** Rasenboden's terrain (grass, dirt paths, cobbled road; its
  heights match the heightmap) lies at grass level over the sandy canyon path the cliff models
  make in the village close-up (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`). Every
  Ryza lit terrain draw has kind bit 0, so that bit hides nothing; program bit 4 (on some
  draws) is unread. Needs a frame of the game at that spot, or the field shader pack's terrain
  programs. The layers' normal maps are not drawn yet.
- **The sand square** in the overview (`obj-A21-S06-FLOOR-01` at 0,0,0?) sits where no ground is
  expected.
- **Other field files** not in Escha: `.trmd` (`TRMD`, 5 MB, 1024 × 1024 header words then
  runs of `24 ff ff ff ff`), `.g1pf` (`FP1G`, a grid over 128,000 units, likely navigation),
  `.koc` (read: the field walls) next to the containers.

## Against the game

A recorded run of the original (a new game: the opening, the attic event at dusk, free play
from the path outside Ryza's house; kept outside the repository) is the reference. Ryza's house
is `obj-A21-S06-HOUSE-01` (32538, 6194, 28804); free play starts on the path down from its gate,
past the signpost (32148, 6003, 31151) toward the light poles. Close to the game's first view:
`barrel-ryza INSTALL s06_t_rasen OUT --look-at 32400,6350,30600 --distance 800 --pitch 12 --yaw 150
--time morning`. What differs there, largest first:

- **Shadows.** The game's path and rocks lie in the trees' shade, lit blue by the sky.
  `barrel-ryza` now casts the sun's shadow from every placed model and Ryza (`--no-shadows`
  to compare) over one square around the target (`1.5 ×` the camera distance, 1500 to 6000
  units across each way) and a far layer out to 30000 units, fading from 20000 (the
  override's `shadowMap_ShadowErase*`; docs/renderer.md, "Shadows"). The far square moves in
  2000-unit steps and is drawn again over 8 frames (about 1 ms each on the PC; on the RP6 to
  be measured). The shade
  comes out lighter than the game's.
- **The sky.** `barrel-ryza` draws the sky plane with the game's measured scattering (the set's
  `skyFog_`, the sun times its power). Stand-ins: the path length's terms (`skyPos`, `fog`:
  an atmosphere `end` thick over a ground ten times as wide) and no glow around the sun
  (`vGlowDir`, `cGlow`, `hGlowParam` not read). Against the game at Rasenboden's noon the blue
  comes out deeper and without the white haze toward the horizon, which the path length and the
  glow shape. The sunny morning's sky comes out dark grey (its `skyFog_`: power 10, rayleigh
  (1.5, 1.98, 3.0), nearly grey), where the game's opening shows a blue morning sky: the path
  length's terms or the glow must carry more there. The set's cloud dome is drawn (texture × vertex
  colour, lit by the light on an upward surface, alpha-blended over the sky plane; its shader
  `PBSkyDome-NT0Tn` lights it with a normal map, not read) centred under the view's target (or under Ryza, and moving
  with her in a window) at height 0: where the game puts it (`SkyDomeWorldPosition`,
  `WorldScale`) is not placed.
- **Which set at which hour.** The game's first view of free play
  (`work/ryza-ref/rasen-path-start.png`, the clock at 10 AM) looks like our noon set there:
  deep shade, blue-lit path and rocks, a clear blue sky. Our morning set comes out pale and
  hazy with hardly any shade: its ambient is ten times noon's (19.7, 29.6, 35.6) and its
  `skyFog_` a thick grey haze (mie 4, 3.4, 2.5; rayleigh 1.5, 2, 3). The HUD's clock shows which:
  its icon is one of four (`gen_a21_field01_clock_icon` in `ui/a21_field.g1t`: rising sun,
  full sun, setting sun, moon, in the set order morning, noon, evening, night), and at 10 AM
  in every still it is the full sun, so 10 AM is already the noon set. The hours where the
  others begin are not read yet (the executable names `time_zone`, `EVENT_SET_TIMEZONE`). Night has no reference
  frame yet, so its colour (too bright and green here) cannot be checked.
- **Grass.** The game's grass is blades over the ground; the terrain is flat colour here.
  Its plants (flowers, weeds, tufts, wheat) come from `_grass.bin` / `_grass.g1t` (a grid of
  plant kinds per 500 units and 8 card textures, docs/formats.md): `barrel-ryza` draws them
  as upright cards turning about the vertical (look slots 37 to 39), on the heightmap, one
  draw per 8 × 8 cells: Rasenboden's 44,015 plants in 66 draws, about 0.14 ms at the gate
  view on the PC. Stand-ins, not read: the spread (a kind's last float as plants per 100 ×
  100 units, its first two as height and spread, the next two as width and spread), which card
  each kind draws (`PLANT_CARDS`, by the stills: kind 6 along the paths as the grass tuft,
  kind 7 in the yellow fields as wheat), a sway of the cards' tops by stand-in wind
  values (`PLANT_SWAY`: the game's `g_vAttCache0` / `g_fPhase` are set at run time; sway
  slots 0 and 1, free in Ryza's fields), no shadows cast, lit
  as the field's models with an upward normal (a camera-facing one made them pale; up, they
  take the ground's light, darker and bluer in the shade than the game's green). They shrink away
  from 8000 to 15000 units from the eye as the game's do (the Middle grass preset;
  `model.vert` flag 16384, look slot 93); the near clip and the far scale-up are not done. The game's plant program mixes the vertex colour
  with a field-wide pigment map by `g_vPigmentMapScale` (docs/shading.md, "Ryza DX: the
  plants"); which texture is the pigment and
  the scale's values are set at run time, not read. Whether the dense
  short grass is these cards or the terrain's own is not known.
- **Brightness and bloom.** The game is brighter and more saturated, with bloom on the sky
  (left over: exposure, `trueHDR_`). The glow now takes the set's `fLumiThreshold` (0.3,
  evening 0.8) on the stored scene and its `fBloomWeight` (0.4, evening 0.6): a soft halo on
  the sunlit leaves at the gate, the still's haze is wider; which space the game thresholds
  in is not read. Against the still the leaves' and rocks' shade is too blue (the ambient
  tint's hue, below). The default environment override's
  `trueHDR_vSaturationScale` (1.2) is laid on the finished frame about its BT.601 luma (where
  in the game's HDR chain it applies is not read); its bloom weight (0.3), FXAA and exposure
  range (0.9 to 1.2) are not. Only `s02_l_hap_main` and `s04_l_sad_main` have their own
  overrides (`env_<field>.xml`): every field takes `env_default.xml` here.

## Ryza in the field

- **Ground and walls.** `--ryza` stands her on the field's heightmap (`<FIELD>.khm`, through the
  shared `walk::Ground::of_heightmap`): at the garden gate she stands on the stepping stones as
  in the game. The fields have no `_col.g1co` as Escha's do; their collision is likely in
  `<FIELD>_coll.bin` (224 bytes in Rasenboden), `.g1pf` or `_le.koc`. The `.koc` walls are
  read (`barrel_ryza::koc`, docs/formats.md) and stop her in a window (run from the garden
  gate toward the house, she stops at the quad across its front, z 28966, her radius short of
  it). Some quads there cross each other in an X behind that one, so not every quad may be a
  plain wall (volumes outlined by quads?). Its sloped and flat quads are not used; `_coll.bin`'s boxes and cylinder are
  (half-sizes or full ones, and where a cylinder starts, not checked).
  The placed models' collision meshes (`_cbvh`, docs/formats.md) are: walls and floors on
  models, and she steps over a wall whose top is within a step (60) of her feet. The boxes and capsules are
  used too (boxes as boxes, capsules as eight-sided prisms): kept as they are when they lie mostly
  inside their model's bounds, turned from z-up to y-up when that fits better (the light poles),
  left out when neither fits (the bags): a stand-in until where the game places them is known
  (a z-up flag, or the model's own data). The hulls (`_cplh`) are read
  and used the same way (Rasenboden: 449 of them; with the meshes, boxes and capsules 1,577
  placed shapes, 293,000 triangles). The camera keeps out of
  the ground behind her (`camera::clear_distance`: pulled in where terrain or a floor comes
  within 30 below to 400 above its line, no nearer than 60, easing back out): banks no longer
  swallow it; it also stops short of the first wall its line passes through
  (`walk::Walls::first_hit`). It has no size: a rock just beside the line can still fill the
  view's side. The game's own camera collision and how it pulls in (pitching up?) are not read. Heights look only at the floors in
  their 256-unit cell (`walk::Ground`): 0.6 µs a height in Rasenboden on the PC, from 1.8 ms
  searching all 282,000 triangles.
- **Walking her** (`barrel-ryza ... --ryza X,Y,Z --window`, `barrel_ryza::player`). Stand-ins,
  not read from the game: the lean that starts a run (0.6), how fast she turns, the step she climbs (60), and the camera's look point (110
  above her feet; `chara.xml`'s `CamH` 160 for it puts the gate view inside a tree). With it
  and the field camera's 45°, 400 and 8° (vertical field of view), the scene at the garden gate
  frames as in the game's still (the fence, the path and its steps, the light pole, the trees in
  the same places); she stands nearer the camera in the save than at the spot used here. Her speeds (walk 71, run 432 units a second) are those her
  feet keep still at (docs/formats.md, "Ryza's speeds"): whether the game moves her that fast
  is unchecked. Her size against the walls (28 by 150)
  and her model's scale (1.04) are the character tables' (`barrel_ryza::chara`). The camera's field of view, distance and pitch are
  the field's (`barrel_ryza::camera`); `at_diff_y` and the look-up adjustment are not placed,
  and the field of view taken as vertical frames the gate as the game does. Setting off plays the gait's
  start motion (10, 20) then its loop, letting go its stop motion (12, 22) then the stand;
  walk and run change loop to loop, each change mixing the last motion out over 0.2 s. How
  the game moves her during the start and stop (here eased in and out) and its mixing time
  are not read.
- **The garden stairs' collision** (worked around; `a21-s07-stair-01`, placed at 32507, 6006, 30997 nearly
  unturned): its `_cbvh` mesh (20 triangles; `_cbvh1` and `_cbvh2` are a thin side piece) spans
  local z -707 to 391 and y -279 to 295, its model z -499 to 546 and y -252 to 181 (one joint at
  the origin, no offset; the mesh's header has no transform). Down the stairs' middle (x 32537)
  the heights are:

  | z | 30300 | 30400 | 30500 | 30600 | 30800 | 31000 | 31200 |
  |---|---|---|---|---|---|---|---|
  | terrain (`.khm`) | 6182 | 6176 | 6170 | 6136 | 6069 | 6001 | 6001 |
  | with the mesh's floors | 6287 | 6268 | 6235 | 6201 | 6134 | 6067 | 6001 |

  The terrain and the model agree (the top at 6182 to 6186); the mesh's slope is as steep but
  twice as long, its top 105 higher, with a landing at z 30284 to 30330. Starting there
  (`--ryza 32537,0,30300`) she is inside the mesh's end, between faces 33 units apart: a walk
  cannot get out (a run's longer steps can). Not a mirror or a turn of the model (no flip of
  y or z fits). The mesh is read right (8 distinct corners of a ramp: its top slopes 369 over
  1057, 19°, as the model's step edges do, 350 over 1000), but it lies about 180 units toward
  -z from the model (its ends -208 and -155 past the model's, its plane 56 above the step
  edges, which a 180 shift along the slope also gives). Across Rasenboden's 37 `_cbvh` meshes
  most lie inside their model's bounds (parts of it); the ones that do not: this one,
  `a21-s09-stair-02` (y ±485 against ±380, z -278 to 97 against ±258), the tunnel rocks
  (`a21-common-rock-tunnel-01/02`: 1314 × 202 × 1352 against the model's 216 × 1341 × 1364, x
  and y swapped) and `a21-s06-floor-01` (four meshes tens of thousands of units outside the
  model: likely in field space; "the sand square" above). `a21-s09-stair-03`'s hull fits.
  `.g1pf` is not a placement list (an `FP1G` grid over 128,000 units: likely navigation).
  `a21-s06-floor-01` is placed at the origin unturned, and its four meshes land near it (x -3144
  to 4223, z -1207 to 1867), but its model's geometry is in field coordinates (x 1210 to 56805,
  z 33342 to 53823): the meshes are not. The tunnel rocks are placed turned about 86° round z,
  and their meshes look like their models already turned once more. Nothing found places the
  meshes: the stair model's chunks hold no offset (`MM1G` an identity matrix, `FM1G` zeros, one
  joint at the origin), the container's slots around the meshes are empty `dummy` entries, and
  the mesh files have no transform. Their placements tell them apart: the placement word
  (`opl2::Instance::word`) of `a21-s06-floor-01`, the four `a21-s07-stair-01` and
  `a21-s09-stair-02` all have bits 0xc000 (`0x0c0ab`), and no placement whose meshes fit has
  them (`a21-s09-stair-03`, whose hull fits, has 0x12000; cliffs `0x000ab`, rocks `0x000a9`,
  trees `0x0006b`, tunnel rocks `0x0002b`). `solids::NO_COLLISION` leaves those placements'
  meshes out (a stand-in for what the bits mean): the terrain carries her over the stairs, and
  a walk from the gate goes down them. The tunnel rocks' meshes (no such bits) are still used
  as they are.
- **Her shading.** Drawn as a field model (the environment's light) with the characters' toon
  ramp on it (`model.frag` flag 128: `chara_rmp`'s row by her materials' `rampIndex`, u the
  sun's half-Lambert term as the default environment override's `ir_` settings give it, and
  the rim from them; docs/shading.md). Which override a field uses (most have none:
  `env_default`), the second light, the point lights' ramp, the specular are not done;
  the hair highlight is laid on fixed (the game slides it with the view's pitch). Her own G-buffer programs are not read: her mesh names them by hashes that match no shader pack's name (docs/formats.md,
  "Ryza's character files"); where they live is not found (no
  pack, table string or executable bytes match). The deferred character lighting pass
  (`DSChara-Cll`: a ramp, a rim table; read in docs/shading.md): her materials' ramp rows
  (`rampIndex` 3 for her skin, 1 elsewhere) are read; the pass's run-time constants are not) and the eyes' `EYET` are named in `shader.xml`; her
  G-buffer programs are likely `PBB` variants picked by her materials' textures. Her face takes its default expression
  and blinks (every 4 s, a stand-in timing); the other expressions and the eyes' directions
  are not used. Her hair, skirt and bust do not swing: the settings (`.swg`, 10 records over
  652 swing bones, docs/formats.md "Swing settings") are read but their floats are not
  understood, and the game's swing physics is not.

## On handhelds

- **Geometry, not pixels.** Rasenboden from the garden gate draws 2,087 draws and 1.85 million
  triangles, every placed object at full detail. On the PC (RX 9070 XT, 1920x1080, offscreen,
  60 frames, median) the frame is 5.97 ms at the desktop preset and 5.89 ms at the handheld one
  (75 % scale): the scene pass barely changes with the resolution, so it is bound by vertices
  and draws. The field's draw distances are applied (`barrel-ryza --draw-ranges pc|nx|off`, the
  PC's by default: a field object farther from the eye than its class's `disp_range`, taken as
  metres, is left out): at the gate they leave out 45 (PC) or 63 (Switch) pixels of 1920x1080
  and take the frame from 5.97 to 5.89 or 5.88 ms, since the landmarks (3000 m) carry the cost.
  Whether the units are metres is a guess (the near class at 70 would be 70 units otherwise,
  a few steps). Choosing the Switch's ranges belongs with the handheld preset (a graphics
  setting, not yet). LOD (`lod`, off on the PC, on for the Switch's `lod_NX`) is not applied.
  Where the frame goes at the gate (1920x1080, desktop preset, 57 frames, median 5.83 ms): the
  shadow map 1.70 ms, the scene 4.04, the rest 0.08. After view culling, by placement class
  (draws / triangles):

  | pass | near | middle | far | landmark |
  |---|---|---|---|---|
  | scene | 39 / 51k (with Ryza) | 62 / 67k | 146 / 67k | 472 / 430k |
  | shadow map | 72 / 54k | 27 / 21k | 53 / 20k | 142 / 160k |

  The shadow map (3000 units square around her, ±8000 along the sun) draws 255k triangles,
  the scene 615k: about the same cost a triangle, and two thirds of both are landmarks (the
  canyon's cliffs). The game's own shadow range (`disp_range` `shadow`, 300 m) cuts nothing
  here. For the RP6: the landmarks' triangles, then the shadow map's casters. The scene's
  landmark triangles at the gate by model: `a21-common-cliff-block-30m-02` 110k (80 draws),
  `a21-common-desert-cliff-06` 52k, `a21-s06-windmill-01` 35k (95 draws), the houses
  (`a21-s06-house-01`, `-03`, `-04`) 20k to 26k each, `a21-common-mountain-02` 18k, the rest
  under 15k. None of these has a `_mid` or `_low` model, and a `_hi` model holds one level (its
  mesh groups are all LOD 0), so the game's LOD would not lighten them; whether 615k triangles
  and 717 draws are too many for the RP6 is for its timings. The placements'
  classes (docs/formats.md, "Placement lists") put the cost in the landmark class: leaving each
  class out in turn (1080p, 37 frames), near saves 0.36 ms, middle 0.39, far 0.51, landmark 3.9
  of 5.9 (646 cliffs and mountains, 1.26 of the 1.85 million triangles). Landmarks are drawn to
  3000 (the farthest), so distance culling saves little; lower-detail models exist for few
  objects (26 `_mid`, 14 `_low` in Rasenboden's objects, mostly flowers, carts and the woods; the
  cliffs have only `_hi`), and the PC game runs with `use_lod` off. Whether the RP6 needs more
  than the culling we have is for its own timings.
- **Floors on models**: read, see "Ryza in the field".
- **The files.** A Rasenboden run reads `PACK00_02.PAK`, `PACK00_03.PAK` and `PACK02.PAK`
  (about 16 GB of the install's 28 GB).

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
