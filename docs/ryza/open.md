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
  adapts within `vExposureRange` (night comes out too bright); (3) how the game combines the
  ambient colour with the cube map (multiplied here), with `indirectStage_`'s scale unplaced
  (the file's blocks shift against the executable's structure after the HDR block). The
  field's shaders (`.g1s`) and a reference frame would settle all three.
- **Model lighting material.** Ryza's models' G1MG section 0x10001 reads as in Escha but holds
  diffuse 0.8 and ambient 0 (`barrel g1m-materials`): their shaders (`PBB-N`, `PBB-EmN`, ...)
  take the ambient from the environment, not `cAmb`. `barrel-ryza` gives every draw the full
  material until those shaders are read.
- **Emissive and odd materials.** `PBB-EmN` submeshes (the attic wall's windows) have an
  emission mask (texture kind 19) and flicker parameters (`emFrq`, `emRange`, `emStart`), not
  drawn. The attic's cabinet (`obj-A21-S01-CABINET-01`) shows red and black: its material is
  not read right. Attic view inside: `--look-at 3905,2250,3914 --distance 300 --pitch 10`.
- **Terrain against the models.** Rasenboden's terrain (grass, dirt paths, cobbled road; its
  heights match the heightmap) lies at grass level over the sandy canyon path the cliff models
  make in the village close-up (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`). Every
  Ryza lit terrain draw has kind bit 0, so that bit hides nothing; program bit 4 (on some
  draws) is unread. Needs a frame of the game at that spot, or the field shader pack's terrain
  programs. The layers' normal maps are not drawn yet.
- **The sand square** in the overview (`obj-A21-S06-FLOOR-01` at 0,0,0?) sits where no ground is
  expected.
- **Other field files** not in Escha: `.trmd`, `.g1pf`, `.koc` next to the containers.

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
