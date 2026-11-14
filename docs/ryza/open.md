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
  (2) one exposure mapping an upward surface of albedo 0.25 to `fMiddleGray`, where the game
  adapts within `vExposureRange` (night comes out too bright); (3) the ambient's shape: the game
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
  (docs/shading.md, "Ryza DX: the deferred G-buffer"): normal maps, the specular map (metal
  flag, specular amount, roughness), and the lighting pass (`pbb-*_cll.g1s`). Attic view inside: `--look-at 3905,2250,3914 --distance 300 --pitch 10`.
- **Terrain against the models.** Rasenboden's terrain (grass, dirt paths, cobbled road; its
  heights match the heightmap) lies at grass level over the sandy canyon path the cliff models
  make in the village close-up (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`). Every
  Ryza lit terrain draw has kind bit 0, so that bit hides nothing; program bit 4 (on some
  draws) is unread. Needs a frame of the game at that spot, or the field shader pack's terrain
  programs. The layers' normal maps are not drawn yet.
- **The sand square** in the overview (`obj-A21-S06-FLOOR-01` at 0,0,0?) sits where no ground is
  expected.
- **Other field files** not in Escha: `.trmd`, `.g1pf`, `.koc` next to the containers.

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
  units across each way); the game's cascades (`shadowMap_`) are not read, so far ground
  stays unshadowed, and the shade comes out lighter than the game's.
- **The sky.** `barrel-ryza` draws the sky plane with the game's measured scattering (the set's
  `skyFog_`, the sun times its power). Stand-ins: the path length's terms (`skyPos`, `fog`:
  an atmosphere `end` thick over a ground ten times as wide) and no glow around the sun
  (`vGlowDir`, `cGlow`, `hGlowParam` not read). Against the game at Rasenboden's noon the blue
  comes out deeper and without the white haze toward the horizon, which the path length and the
  glow shape. The sunny morning's sky comes out dark grey (its `skyFog_`: power 10, rayleigh
  (1.5, 1.98, 3.0), nearly grey), where the game's opening shows a blue morning sky: the path
  length's terms or the glow must carry more there. The set's cloud dome is drawn (unlit,
  texture × vertex colour, alpha-blended over the sky plane; its shader `PBSkyDome-NT0Tn` lights
  it with a normal map, not read) centred under the view's target at height 0: where the game
  puts it (`SkyDomeWorldPosition`, `WorldScale`) is not placed.
- **Grass.** The game's grass is blades over the ground; the terrain is flat colour here.
- **Brightness and bloom.** The game is brighter and more saturated, with bloom on the sky
  (left over: exposure, `trueHDR_`).

## Ryza in the field

- **Her shading.** Drawn as a field model (the environment's light) until her own shaders
  are read.

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
