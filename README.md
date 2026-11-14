# Barrel

A native engine for Gust's Atelier games, written in Rust. The player's Steam copy supplies
the data, and nothing from it is shipped here. There is one engine for all the games: the
format readers, renderer, audio, the event interpreter and the Squirrel VM are shared. Each
game adds its own rules on top: synthesis, battle, calendar and menus.

| Game | Steam release | Base |
|---|---|---|
| Atelier Ayesha DX | 2021 | Older PSSG data with KTGL, Squirrel 2.2.5 |
| Atelier Escha & Logy DX | 2021 | KTGL + elixir, Squirrel 2.2.5 |
| Atelier Shallie DX | 2021 | KTGL + elixir, Squirrel 2.2.5 |
| Atelier Sophie 2 | 2022 | KTGL + elixir, PAK archives, Squirrel 2.2.5 |
| Atelier Ryza DX | 2023 | KTGL + elixir, PAK archives, no Squirrel |
| Atelier Ryza 2 DX | 2023 | KTGL + elixir, PAK archives, no Squirrel |
| Atelier Ryza 3 DX | 2024 | KTGL + elixir, PAK archives, no Squirrel |

The goals:
- run natively on Linux, including aarch64 handhelds (Vulkan);
- mod support built in (a load order of loose files that override any table, event,
  script or asset);
- graphics beyond the DX11 originals once the original look is reproduced;
- uncapped frame rates, any aspect ratio, and UI scaling for handhelds.

## Status

Escha & Logy DX comes first. The other six games' data reads with the same code (checked over
every file, 2026-11-12): Shallie DX's and Ayesha DX's events, messages and tables (each with its
own measured event command table), and every texture, model, animation, event and message in
Sophie 2's and the Ryza games' archives, but for two Japanese test-event message files
(`docs/formats.md`). Ayesha's models and motions are in the older PSSG format: its files read, with their meshes
(`barrel pssg-obj`), textures, skeletons and motions; materials and drawing them not yet.
Escha plays from its own title screen through the opening into free play (fields, world and
area maps, gathering, battles, the cauldron, shops, menus, saves), on the desktop and on an
aarch64 handheld; `docs/escha/remaining.md` lists what is left for the whole game. The systems
below are checked against the original (its executable's code and recorded runs). Ryza DX
is next (`docs/ryza/`).

The code is split by game; nothing shared names a game's rules:

| Crate | What it holds |
|---|---|
| `crates/formats` | The games' file formats: containers, PAK archives, textures, models, motions, terrain, events and messages, XML tables, voice packs, Vorbis |
| `crates/render` | The Vulkan renderer (`renderer::Renderer`, drawing the `Frame` a game hands it), the text and 2D layer, the window, the pad, the graphics settings |
| `crates/engine` | What every game's play shares: the event player and its stage, cameras, the Squirrel VM, walking against a field's collision, posing skeletons, the clock, wind |
| `crates/audio` | The mixer and SDL3 output over Barrel's own Vorbis decoder |
| `crates/escha` | Escha & Logy DX: its rules, its own data (`data`: its executable's tables, its saves, its own XML tables) and the `barrel-escha` program (`src/play`: its screens and run) |
| `crates/cli` | The `barrel` tool over all of them |

Open problems are kept per game (`docs/escha/open.md`, `docs/ryza/open.md`) and for the shared
engine (`docs/open.md`).

| Part | Where | State |
|---|---|---|
| Formats | `crates/formats` | Containers, PAK archives (all four keys), tables, events and messages, textures, models, animations, terrain (its layer blending in part), sound and voices, saves (read). Every Escha file of these kinds reads. |
| Events | `engine::event`, `engine::stage` | The event bytecode played on a clock: lines, cameras, characters walking, gestures and faces, lights, fades, narration cards, variables and choices. Movies are not played (`docs/engine.md`, "Movies"). |
| Story | `escha::event_start` | Which event starts when: event ends, walks between maps, days passing. Both openings chain as the game plays them. |
| Battle | `escha::battle`, `fight`, `skirmish`, `monster_ai` | Damage, turn order, skills, items, monster rules, falling and guts, rewards and levels, played on a battle screen (not yet the game's layout); not yet checked against a full recorded battle. |
| Synthesis | `escha::synthesis` | Recipes, effects by element values, traits, quality. Property points are not in. |
| Field | `escha::field_ai`, `encounter`, `gather` | The field enemies' Squirrel scripts in Barrel's own VM, encounters, gathering lists and spots, the player walking and jumping (`escha::player`, `engine::walk`). |
| Calendar | `escha::days`, `calendar`, `quest`, `shop`, `rank` | Days passing, requests, shops' restock, the month report and the adventurer rank. |
| Renderer | `crates/render` | Vulkan: fields with terrain, sky and cloud shadows, characters with motions and faces, events played in a window or to files, upscalers (FSR 1 and 3, GSR 1 and 2), TAA, shadows. |
| Sound | `crates/audio` | Barrel's own Vorbis decoder, a mixer and SDL3 output: music and voices in events. |

Not started: Shallie, Ayesha, Sophie 2 and the Ryza games beyond their data. `docs/survey.md` has the games,
`docs/formats.md`, `docs/tables.md` and `docs/events.md` the data and rules, `docs/renderer.md`,
`docs/shading.md` and `docs/engine.md` the engine.

```sh
cargo build --release
cargo test --release
target/release/barrel game "<steam>/steamapps/common/Atelier Escha and Logy DX"
target/release/barrel scan  "<install>/Data"           # every container and what it holds
target/release/barrel ls     FILE.elixir.gz
target/release/barrel unpack FILE.elixir.gz OUTDIR
target/release/barrel ebd-scan "<install>/Event"       # every event and its commands
target/release/barrel ebd-decode "<install>/Event"     # split every event into commands
target/release/barrel ebd    FILE.ebd
target/release/barrel play-event "<install>/Event/event/MM08_AS4/EVENT_MM08_AS4_130.ebd" EN
target/release/barrel pak-ls "<Sophie 2>/Data/PACK00_01.PAK"   # list an archive (Sophie 2, Ryza 1-3)
target/release/barrel recipes "<install>/Atelier_Escha_and_Logy_EN.exe"   # the synthesis recipes
target/release/barrel synth "<install>" 195 44 29 41          # Craft from Buried Bomb, Bubbling Water, Dragon Core
target/release/barrel ebm-scan "<install>"            # every dialogue line, all languages
target/release/barrel ebm    FILE.ebm
target/release/barrel xml-scan "<install>"            # every XML table and its rows
target/release/barrel xml    FILE.xml
target/release/barrel g1t-scan "<install>/Data"       # every texture
target/release/barrel g1t    FILE.g1t OUTDIR           # a texture pack as PNG files
target/release/barrel g1m-scan "<install>/Data"       # every model
target/release/barrel g1m-obj FILE.elixir.gz#N OUT.obj  # a model as OBJ
```

`barrel-escha` is Escha & Logy DX's program (a Kintsugi launch starts it at the title, below);
as a tool it draws a model, a field or an event: to PNG files offscreen, or live in a window (`--window FRAMES`). With an event it plays
it on (`--at`, `--line`, `--frames`, `--fps`) and with `--chain` goes on to the events the story
starts after it (`docs/renderer.md`).

```sh
target/release/barrel-escha MODEL.elixir.gz#N TEXTURES.elixir.gz#N out.png [--size N] [--yaw DEG]
target/release/barrel-escha MODEL#N TEXTURES#N out.png --skeleton BASE_MODEL#N --anim MOTION#N --time 2
target/release/barrel-escha --field "<install>/Data/x64/Field/AREA_01_01" out.png
target/release/barrel-escha --field auto out.png --event "<install>/Event/event/MM01_OP1/EVENT_MM01_OP1_111.ebd" --window 100000 --chain
BARREL_VALIDATION=1 target/release/barrel-escha ...      # with the Khronos validation layer
```

Building the renderer needs `glslc` (shaderc, part of the Vulkan SDK) to compile its shaders;
set `GLSLC` to use another path.

`crates/audio` is the sound: a mixer over clips decoded by Barrel's own Vorbis decoder
(`barrel_formats::vorbis`) and an SDL3 output stream. Building it needs SDL3 (found through
`pkg-config`). `barrel voice-wav SeriesNNNN.fpl CLIP OUT.wav` decodes a voice clip; the output
test runs with `SDL_AUDIO_DRIVER=dummy cargo test -p barrel-audio -- --ignored`.

Extracted data goes in `work/`, which git ignores. No game data belongs in the repository.

The name comes from the series' running "Barrel!" joke.

## Playing through Steam (Kintsugi)

Barrel runs as a [Kintsugi](https://github.com/daftblazer/kintsugi) engine: in the game's
Properties > Compatibility, force the Kintsugi tool, and Steam starts Barrel at the title screen
on the game's own files instead of the Windows game. Escha & Logy DX (app 1152310) is wired up;
Ryza DX (app 3365030) starts Ryza walking Rasenboden from her garden gate (no title or story
yet).

```
tools/deploy.sh                 # build for this machine and install the engine here
tools/deploy.sh user@host       # cross-build for aarch64 and install it on a handheld over SSH
BARREL_GAME=ryza tools/deploy.sh [user@host]   # the same for Ryza DX
tools/package.sh ARCH OUT [escha|ryza]         # just make the engine folder (x86_64 or aarch64)
```

The aarch64 build needs an arm64 root filesystem (glibc no newer than the device's, the Vulkan
loader, libwayland-client) in `BARREL_AARCH64_SYSROOT` and a prefix with arm64 SDL3 in
`BARREL_AARCH64_DEPS`; SDL3 ships in the engine's `lib/`. On an aarch64 device the first start is
fullscreen with the handheld preset (`BARREL_DEFAULT_PRESET`, `BARREL_DEFAULT_FULLSCREEN`, used
only while there is no `~/.config/barrel/graphics.conf`); after that the in-game Options page
decides. The engine's log is `~/.local/state/barrel/barrel.log`, saves are in
`~/.local/share/barrel/saves`.

A save from the original game carries over with `barrel import-save INSTALL GAMEDATA00.pcsave >
~/.local/share/barrel/saves/slot01.save` (the lead, date, money, party and equipment, basket,
known items, map progress, field effect, events seen and flags; Barrel starts it in the field of
the place the party is at, at its arrival spot, where the game opens that place's area map).

## Third-party code

Barrel's renderer carries ports of two upscalers' shaders, with their notices in the shader
sources (`crates/render/shaders/`):

- AMD FidelityFX Super Resolution 1.0 (`easu.frag`, `rcas.frag`): Copyright (c) 2021 Advanced
  Micro Devices, Inc., MIT License.
- AMD FidelityFX Super Resolution 3.1's upscaler (`ffx/`, vendored unchanged from the FidelityFX
  SDK v1.1.4): Copyright (C) 2024 Advanced Micro Devices, Inc., MIT License (`ffx/LICENSE.txt`).
- Snapdragon Game Super Resolution 1 and 2 (`gsr1.frag`, `gsr2_convert.frag`,
  `gsr2_upscale.frag`): Copyright (c) 2023-2024, Qualcomm Innovation Center, Inc., BSD 3-Clause
  License. Binary builds reproduce its notice, conditions and
  disclaimer as given in those files.
