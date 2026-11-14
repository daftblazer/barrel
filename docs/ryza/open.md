# Ryza DX: open problems

Everything Barrel is stuck on for Atelier Ryza DX, has not worked out yet, or knows to be wrong
(addresses are in `Atelier_Ryza_DX.exe`, which is not DRM-wrapped). Same rules as
`docs/escha/open.md`; problems in the shared engine are in `docs/open.md`.

The original Ryza DX is never run through Steam without asking first (its playtime and
achievements are the player's own, and the DX version has not been played there).

## Fields

- **Ryza 3 DX's placement lists** read neither as Escha's nor as Ryza DX's (they fail at the
  first type's instance count): for when Ryza 3 starts (`docs/formats.md`, "Placement lists").
- **Terrain (`.ktf`) in 9 fields** (`s35_s_cove_03`, `s40_m_tower_02`, `s41_m_tower_03`,
  `s45_m_underground_01`, `s53_m_starbottle_01`, `s76`/`s78`/`s80`/`s82` battle points) fails
  with "ktf section offsets"; the other 87 fields load (`barrel ryza-field INSTALL`). Way in:
  compare one's header with a field that reads (docs/formats.md, the terrain section).
- **Field lighting.** `barrel-ryza` lights fields with the environment's diffuse cube map
  alone (as an ambient, spherical harmonics): dim, no sun, no point lights. The cube maps are
  shared between fields, so the field's own light (sun direction and colour, exposure) must be
  in `_env.bin` (12 sets by weather and time; docs/formats.md). The sun, the second light
  and the main ambient are placed (`barrel_ryza::environment::Light`); not yet used by
  `barrel-ryza`, and `indirectStage_` (the cube map's scale, `IDLScalerOfAmbientDiffuseMap`)
  is not placed: the file's blocks shift against the executable's structure (0x44 by the
  lights, more later). Also open: how the game turns elevation and azimuth into a direction,
  and its exposure (`trueHDR_`: middle gray 0.18, `fLumiMax`).
- **Model lighting material.** Ryza's models' G1MG section 0x10001 reads as 0 with Escha's
  layout (`cDiff` / `cAmb`), so `barrel-ryza` gives every draw the full material.
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
