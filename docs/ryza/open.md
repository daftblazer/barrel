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
- **Field lighting.** `barrel-ryza` draws fields unlit (texture times vertex colour) until
  Ryza's lights are read: `_light.bin` in the field container, `_env.bin` and the `_ad` / `_as`
  probes in `_env`. Ryza has no Escha-style look table per field.
- **Terrain against the models.** Rasenboden's terrain draws since its flag bits are read
  apart (docs/formats.md, terrain draws), but in the village close-up it lies over the sandy
  canyon path the models make, and one layer shows flat blue where the streams run (the field
  has a `_river.bin`: a water surface drawn over it?). Open: whether the flag bits (kind bit 0,
  program bit 4) cut holes or mark another height; compare a frame of the game at that spot
  (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`).
- **The sand square** in the overview (`obj-A21-S06-FLOOR-01` at 0,0,0?) sits where no ground is
  expected.
- **The environment container** (`<field>_env.elixir.gz`: `<field>_env.bin`, `_ad.g1t`,
  `_as.g1t`, likely diffuse and specular light probes) is new; nothing reads it yet.
- **Other field files** not in Escha: `.trmd`, `.g1pf`, `.koc` next to the containers.

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
