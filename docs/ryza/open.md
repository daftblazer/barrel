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
- **Field lighting.** `barrel-ryza` draws fields unlit. Ryza lights them from per-field HDR
  cube maps (`_ad` diffuse, `_as` specular, BC6H) plus the `_light.bin` point lights (read, no
  colour) and `_env.bin` (98 KB, layout unread): docs/formats.md, "Ryza field environment".
  Way in: the diffuse cube map as the ambient (needs BC6H cube upload), then `_env.bin`'s
  exposure, then the field's own shaders (`.g1s`) for the exact model.
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
