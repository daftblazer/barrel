# Ryza DX: open problems

Everything Barrel is stuck on for Atelier Ryza DX, has not worked out yet, or knows to be wrong
(addresses are in `Atelier_Ryza_DX.exe`, which is not DRM-wrapped). Same rules as
`docs/escha/open.md`; problems in the shared engine are in `docs/open.md`.

The original Ryza DX is never run through Steam without asking first (its playtime and
achievements are the player's own, and the DX version has not been played there).

## The answer key's runs repeat only to a frame

ryza-recomp's scenarios run with virtual clocks by default (RYZA_DETERMINISTIC: each frame is
1/60 s; each clock read in a frame adds 1 µs, shared across threads), and state descriptions are
hashed with their padding zeroed. Two boot-title runs (2026-11-16) still differ: the second is
the first shifted by about one frame (at the best shift 1,143 of 1,924 draws carry identical
vertex constants), and the draw count varies by 1-2 (1,923-1,925). Loading threads finish in real
time, so the frame the title starts on moves. So: compare at aligned frames (`barrel-parity
align`, or find the shift as above) and fit nothing to a single run's random rolls; a check row
states its tolerance. Way in, if it matters: per-thread read counters for the clock, and a
frame-locked wait on the loader's completion (the frame it finishes on, from a hook).

## Fields

- **Ryza 3 DX's placement lists** read neither as Escha's nor as Ryza DX's (they fail at the
  first type's instance count): for when Ryza 3 starts (`docs/formats.md`, "Placement lists").
- **Field lighting.** `barrel --time T --weather W` lights a field in linear light as
  the game does (docs/shading.md, "Ryza DX: the tone curve, saturation and fog passes"): colour
  textures sampled as sRGB, the sun and the second light from the set (their directions match
  the game's `litDir` exactly), the game's filmic curve with the set's `fLumiMax` and `fGamma`,
  the scene fog from the set's `fog_`. Against the game's noon still at the garden gate, from
  the game's own camera (`work/ryza-ref/rasen-gate-constants.png`; `--ryza
  32518.2,6179.6,30331.5 --turn -0.0222`): sunlit sand 221, 203, 157 against 222, 203, 146,
  stone 221, 223, 220 against 224, 227, 216, shaded path 35, 65, 58 against 23, 65, 53, rock
  39, 64, 76 against 36, 67, 83, fence 75, 71, 68 against 71, 70, 62. Stand-ins left:
  (1) **what the shade lacks**: the stage's ambient is the diffuse light map at the set's
  `indirectStage_` scale and colour (`StageLight`: 7 × (0.1, 0.2602, 1) at Rasenboden's noon,
  1 × (1.9, 2.3, 4) in the attic's morning), but the game's shade also carries light that
  does not scale with the surface's colour (dark trunks come out 37, 48, 57 where ours are
  23, 27, 30): the light volume (`sLPVBuffer` × `fDiffuseScale`, 4.2227 in the blocks read),
  the specular light map and the SSAO are not drawn. The word at +0x34 of the stage's block
  (3 at noon, 5 in the morning; the characters' is their sun scale) is not the stage's sun
  scale: the lighting blocks hold the sun unscaled, and 54 blocks of another kind hold it
  times 3 (not placed). (2) **the exposure**: the default override's range top
  (1.2) at every time of day; the game adapts (`fMiddleGray`, `fLightAdaptedScale`,
  `fDarkAdaptedScale`: not read), and night comes out far too bright (the moon is (1, 4, 5)).
  (3) **the bloom**: the set's threshold on the stored scene and its weight; the game's bright
  pass is not read, and sunlit grass stays darker than the still's (129, 177, 34 against 155,
  199, 47). (4) **the sky**: the sky plane's light scaled by a fitted 0.7 and the dome's by
  2.5 (`SKY_SCALE`, `DOME_SCALE`); morning's sky comes out pale. (5) no saturation pass
  (`vSaturationScale`, `SaturationScaleEx`: both worsen the match). Evening, morning and night
  have no reference frame. Ryza takes the characters' light (`indirectChara_` in the set: the
  sun at 0.8, the set's ambient at 0.2 plus the light map's mean at 1.5 × (0.8, 1, 1), flat
  rather than by her normals; `barrel_ryza::environment::CharaLight`): her colours come out
  as the still's, a little brighter (her hat 240, 232, 215 against 225, 215, 195).
- **Model lighting material.** Ryza's models' G1MG section 0x10001 reads as in Escha but holds
  diffuse 0.8 and ambient 0 (`barrel g1m-materials`): their shaders (`PBB-N`, `PBB-EmN`, ...)
  take the ambient from the environment, not `cAmb`. `barrel` gives every draw the full
  material until those shaders are read.
- **Emissive and odd materials.** `PBB-EmN` submeshes (the attic wall's windows; at
  Rasenboden three houses' windows, a bridge pillar and the light poles) have an emission map
  (texture kind 19) and flicker parameters (`emFrq` 1, `emRange` (0, 1) or (1, 1), `emStart`
  0 in Ryza's house). The lighting pass adds `sEmi × |emScl|`: to the light (times the colour)
  when `emScl` ≥ 0, to the finished colour when negative; `emScl` is not among the material's
  parameters (set at run time). `barrel` draws the map as light (`model.frag` flag
  32768, the map bound as the draw's set 3) at a stand-in strength by time of day
  (`EMISSION_BY_TIME`: none by day, 2 at evening, 3 at night): the house's window glows at
  night. The flicker is not done: no pass of `PBB-EmN` (`_gb`, `_gba2`, `_cll`) reads
  `emFrq`, `emRange` or `emStart`; all three take only `emScl` (+16 in the G-buffer passes'
  block, +96 in the lighting pass's), so the game works the flicker out on the CPU into
  `emScl` each frame. Its curve (a wave between `emRange`'s two ends at `emFrq`, offset by
  `emStart`, would fit the names; `emRange` (1, 1) would then hold still) is a guess until a
  run reads `emScl` over a few seconds (tools/oracle/README.md). Not drawn yet from the G-buffer model
  (docs/shading.md, "Ryza DX: the deferred G-buffer"; normal maps are drawn now, their
  green's sign unchecked): the specular map (metal flag, specular amount, roughness), and the lighting pass (`pbb-*_cll.g1s`). Attic view inside: `--look-at 3905,2250,3914 --distance 300 --pitch 10`.
- **Terrain against the models.** Rasenboden's terrain (grass, dirt paths, cobbled road; its
  heights match the heightmap) lies at grass level over the sandy canyon path the cliff models
  make in the village close-up (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`). Every
  Ryza lit terrain draw has kind bit 0, so that bit hides nothing; program bit 4 (on some
  draws) is unread. Needs a frame of the game at that spot, or the field shader pack's terrain
  programs. The layers' normal maps are not drawn yet.
- **`obj-A21-S06-FLOOR-01`** (placed at the origin, its geometry in field coordinates, x 1210
  to 56805, z 33342 to 53823) is the stone paving along the canal and its bridge: hidden
  (`--hide FLOOR-01`), the overview shows a hole there. It draws where it belongs; only its
  collision meshes are off (below). Not a stray "sand square", as first noted.
- **Other field files** not in Escha: `.trmd` (`TRMD`, 5 MB, 1024 × 1024 header words then
  runs of `24 ff ff ff ff`), `.g1pf` (`FP1G`, a grid over 128,000 units, likely navigation),
  `.koc` (read: the field walls) next to the containers.

## Against the game

A recorded run of the original (a new game: the opening, the attic event at dusk, free play
from the path outside Ryza's house; kept outside the repository) is the reference. Ryza's house
is `obj-A21-S06-HOUSE-01` (32538, 6194, 28804); free play starts on the path down from its gate,
past the signpost (32148, 6003, 31151) toward the light poles. Close to the game's first view:
`barrel INSTALL s06_t_rasen OUT --look-at 32400,6350,30600 --distance 800 --pitch 12 --yaw 150
--time morning`. What differs there, largest first:

- **Shadows.** The game's path and rocks lie in the trees' shade, lit blue by the sky.
  `barrel` now casts the sun's shadow from every placed model and Ryza (`--no-shadows`
  to compare) over one square around the target (`1.5 ×` the camera distance, 1500 to 6000
  units across each way) and a far layer out to 30000 units, fading from 20000 (the
  override's `shadowMap_ShadowErase*`; docs/renderer.md, "Shadows"). The far square moves in
  2000-unit steps and is drawn again over 8 frames (about 1 ms each on the PC; on the RP6 to
  be measured). The shade
  comes out lighter than the game's.
- **The sky.** `barrel` draws the sky plane with the game's measured scattering (the set's
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
  plant kinds per 500 units and 8 card textures, docs/formats.md): `barrel` draws them
  as upright cards turning about the vertical (look slots 37 to 39), on the heightmap, one
  draw per 8 × 8 cells: Rasenboden's 44,015 plants in 66 draws, about 0.14 ms at the gate
  view on the PC. Stand-ins, not read: the spread (a kind's last float as plants per 100 ×
  100 units, its first two as height and spread, the next two as width and spread), which card
  each kind draws (`PLANT_CARDS`, by the stills: kind 6 along the paths as the grass tuft,
  kind 7 in the yellow fields as wheat), a sway of the cards' tops by stand-in wind
  values (`PLANT_SWAY`: the game's `g_vAttCache0` / `g_fPhase` are set at run time; sway
  slots 0 and 1, free in Ryza's fields), no shadows cast, lit
  as the field's models with an upward normal, normal-mapped by the cards' own normal maps
  (`_grass.g1t`'s third texture, flag 8192, as the game's plant program does). In the shade
  they come out teal where the game's are green: the cards are green (the grass tuft's opaque
  pixels average 60, 83, 25) but the sky light from above is blue (noon's 2.7, 3.0, 5.2 ×
  the cube map); the game's pigment mix most likely carries the green. They shrink away
  over the game's grass preset's range as the game's do (`model.vert` flag 16384, look slot
  93); the near clip and the far scale-up are not done. The preset follows the graphics'
  `foliage` (full: Middle, density 1, 8000 to 15000; reduced, the handheld preset: Low, 0.75,
  3000 to 3100; least: VeryLow, 0.5, 2000 to 2100; `barrel_ryza::grass::Preset`): at the gate
  44,015 plants and 5.91 ms on the desktop preset, 32,980 and 5.71 ms on the handheld one
  (PC). The game's plant program mixes the vertex colour
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
- **Walking her** (`barrel ... --ryza X,Y,Z --window`, `barrel_ryza::player`). Stand-ins,
  not read from the game: the lean that starts a run (0.6), how fast she turns and the step she climbs (60). The camera looks 128 above her feet
  (measured: the game's eye and her position at the gate with the field camera's 8° and 400;
  about 0.8 of `chara.xml`'s `CamH` 160, the rule not read). With it
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
  model: likely in field space; the canal's paving, above). `a21-s09-stair-03`'s hull fits.
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
  and draws. The field's draw distances are applied (`barrel --draw-ranges pc|nx|off`, the
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
- **On the RP6 itself** (Adreno 740, Turnip, Mesa 26.2.4; 2026-11-14; the aarch64 package
  from `tools/package.sh aarch64 … ryza`, the game read from the device's own install on its SD
  card; offscreen, 1920x1080, the garden gate at noon, 240 frames, median of 237). It runs
  unchanged and draws as on the PC. Desktop preset: 18.8 ms (shadow map 2.67, scene 15.73, glow
  and AA 0.24, post 0.18), 225 MB of textures, 44,015 plants: short of 60 fps. Handheld preset
  (75 % scale, Middle tables, mip bias 1): 14.5 ms (shadow map 1.25, scene 11.83, glow, AA and
  upscale 1.20, post 0.19), 59 MB of textures, 32,980 plants: 60 fps with about 2 ms to spare
  at the gate. The scene pass is three times the PC's and still most of the frame; the
  landmarks' triangles (above) are where it goes next, and a walk through the village (not
  only the gate) is still to be timed.
- **Floors on models**: read, see "Ryza in the field".
- **The files.** A Rasenboden run (with her) reads 22 files from four archives (2026-11-14):
  `PACK00_01.PAK` (3.06 GB: only `shader\texture\chara_rmp.g1t`), `PACK00_02.PAK` (2.04 GB:
  her model, face and motions), `PACK00_03.PAK` (13.69 GB: the field, its objects, environment
  and walls, the sky dome) and `PACK02.PAK` (0.05 GB: the tables under `saves`), 18.8 GB of
  the install's 21. The voice archives `PACK00_04_*` (3.6 GB) and `Data/x64` (6.1 GB: sound
  and UI resources) are not read yet.

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
- **Levels of detail.** Rasenboden's object container holds 138 `_hi` models, 26 `_mid` and
  14 `_low`. `barrel` loads all three (`Placement::lods`, `Draw::lod`) and shows each by
  the distance from the eye to its box's centre at the game's High LOD preset (10000, 30000;
  docs/formats.md, "the graphics presets"). In a far view of the village it saves about
  0.08 ms of 7.2 on the PC with 12 pixels changed; the landmarks (cliffs, windmills) have no
  lower levels. The graphics' `foliage` picks the game's general preset
  (`barrel_ryza::presets::Graphics`: full High, reduced Middle, least Low), and the plants and
  these distances come from the tables it names (Middle 8000 / 25000 on the handheld preset).
  The general preset's object draw ranges (`DispRangeType`) cap the field's own
  (`DrawRanges::limit`: the smaller of the two for near, middle and far; landmarks keep the
  field's). That rule is a guess: Rasenboden's own (7000, 20000, 40000) are at or under the
  High preset's (8000, 20000, 40000), so the desktop draws as before, and the handheld's Middle
  (7000, 15000, 30000) drops some far small objects: 7.14 to 6.77 ms in a far view (PC). How
  the game really combines the field's `disp_range` with the preset is unknown (an oracle read
  of the ranges in use would tell). The shadow distance (`ShadowDistType`: 1, 0.7, 0.5) scales the far
  shadow layer's fade (20000 to 30000 on High; 14000 to 21000 on the handheld's Middle): a
  guess at what it scales, as the game's cascades (1200 / 6000 / 13000) are not drawn as such.
  In the far view 0.5% of the pixels change and the time does not (the far layer is redrawn
  only when it moves). The texture mip bias (`TextureType`) leaves out each texture's largest
  levels at upload (`g1t::Texture::without_top_levels`, kept at 4 × 4 or more): Rasenboden's
  526 textures take 224.9 MB on the desktop preset and 59.3 MB on the handheld's (bias 1), at
  the gate 5.73 against 5.60 ms (PC); the picture softens as the game's Middle does.
- **`SaturationScaleEx`** (`env_override.xml`, docs/formats.md): if Rasenboden is
  `LGT01_DEFAULT_01`, a saturation of 1.6 at noon on top of (or instead of) the environment's
  `vSaturationScale` (1.27, 1.3, 1.2), which matched the noon still worse when laid on as
  saturation (so Barrel applies none). Which set a field uses and where the game lays the
  scale (everything, or the characters: `S999_CHARA_LIGHT`) are unknown; a reference frame of
  a saturated object at noon against Barrel's with each guess would tell. Also the fifth time
  zone, `TIME_ZONE_MIDNIGHT`, and which of the four sets it shows.
- **Plants against the measured constants** (docs/shading.md, "the plants' constants as the
  game sets them"): the sway is now the game's own formula with its measured `g_vAttCache0` and
  `g_fOscPhase` speed; only the gusts' curve over time is not read (the strength holds 9.6, the
  middle of the 8.7 to 10.5 seen), and the plants' own phases differ from the game's since
  Barrel places its plants itself. The plants sample no pigment map at the gate, so their
  teal is not a missing pigment: the pixel side (`cDiff`, `fPtn`, the ambient terms of
  `objectgrasspbs`'s 416-byte block) is not in the saved ring where looked: the lit blocks
  beside the plants' vertex blocks (sun (−0.354, −0.707, −0.612), `litCol` (4.78, 5.0, 3.5),
  `ambLit` (3, 3, 4), `cDiff` 1 or 0.8, `fPtn` (1, 1, 1, 20)) end in matrices, not the ambient
  terms, so they are other programs'. Against the matched still (`rasen-gate-noon.png`) the
  game's plants at the gate are dark green (about 12, 44, 36 in a clean patch) where Barrel's
  are lighter and bluer: too much of the bluish ambient, or a darker card texture. A run that
  reads the block by its own layout (the 416 bytes after `vEye` at +80 with `vATest` at +176)
  would settle it. `emScl` by evening and its
  flicker need a run at evening.
