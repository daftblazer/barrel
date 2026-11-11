# Survey

The first look at the Steam releases, taken from the installed files and the executables'
strings and metadata. Nothing here has been extracted beyond file indexes yet.

## Feasibility in short

The data is well structured:
- the gameplay tables are XML;
- the events are a bytecode with inline strings;
- the field AI is Squirrel source;
- the assets are KTGL formats that modders have largely documented.

The hard part is the game rules (synthesis, battle, calendar, menus, saves). They are
native x64 code with no symbols beyond C++ RTTI class names, and must be reverse-engineered
for each game. There is no recompiled build to check against. The plan is a logging DLL
injected into the original under Proton, which records the values at identified functions
as the oracle the native rules are checked against.

## The games

### Escha & Logy DX, Shallie DX

The two share one layout and could share one set of loaders.

| Data | Where | Notes |
|---|---|---|
| Containers | `Data/x64/**/*.elixir.gz` | Gust "elixir" archives, magic `CRAE`. They hold models, effects and characters. |
| Textures | `*.g1t` | KTGL textures |
| Shaders | `Data/x64/Shader/*.g1s` | compiled; the look has to be reproduced in new shaders |
| Audio | `Data/x64/Sound/**/*.kvs` | Koei Tecmo's wrapped Ogg Vorbis |
| Movies | `Data/x64/Movie*/*.wmv` | WMV3 / WMA |
| Tables | `Saves*/**/*.xml`, some `*.xml.e` | Shift-JIS XML, one directory per language; `.e` files are encrypted |
| Events | `Event*/**/*.ebd`, `*.ebm` | `.ebd` is event bytecode (numbered commands, inline Shift-JIS strings); `.ebm` holds the messages |
| Field AI | `Script/*.nut` | plain Squirrel source, Shift-JIS with comments |

The executables are mixed C++/CLI builds. Their .NET metadata names ~5,500 (Escha) and
~6,800 (Shallie) native types with their sizes, mostly ktgl and D3D11 types. Only ~300
methods are IL, and almost all of those are CRT. The game logic is native. The game code's
RTTI names 472 (Escha) and 546 (Shallie) classes.

### Ayesha DX

This is a hybrid. It keeps the PS3 game's PSSG data (`*.PSSG`) alongside KTGL formats.
Its scripts are `Script/Nut/*.nut` and also compiled `*.cnut`. Its events and tables follow
the same model as Escha's. The RTTI names 589 game classes. It needs its own loaders on top
of the shared ones.

### Sophie 2

The data is in 35 `Data/*.PAK` archives (24 GB). The executable refers to ~405 `.xml`
tables, ~1,850 `.ebd` and `.ebm` files, and to g1t/g1m and ktsl2 sound banks. It renders
through D3D11.

The RTTI names 1,100 game classes. The largest groups are field (`FM*`, 267), battle
(`Btl*`, 152), UI (57), shop (26) and synthesis (`Mix*`, 21).

The PAK header is `{version 0x20000, count, 16, flags}`. Each entry is 184 bytes:

| Offset | Field |
|---|---|
| +0 | name[128], XOR-scrambled |
| +128 | size |
| +132 | a 20-byte key |
| +168 | offset (u64) |

XORing a name with its 20-byte key cycled shows the padding (leftover memory from the
packing tool). The name is still scrambled by a second, fixed layer: one byte per position,
the same for every entry (entries whose names differ in one character differ in that byte
only). The executable cannot show the key directly: its `.text` is encrypted (entropy 8.0,
with a `.bind` section, the Steam DRM wrapper), so its code has to be unwrapped first.

The key also comes out of the names themselves. Assuming they start with `\Data\x64\`
(a string the executable holds), `tools/pak_key.py` guesses the rest position by position
from how the names read. On Sophie 2's `PACK00_01.PAK` and `PACK00_04_01.PAK` (1,924
entries) most names come out readable, such as `\Data\x64\..._ui_g23_iten_o_095.g1t` and
`....elixir.gr`, with some positions still wrong where letters are equally likely; known
names (extensions, the executable's own paths) are what fix those. The fixed bytes look
like text themselves (`dgGKXLHLuC@pa8...`).

### Ryza DX, Ryza 2 DX, Ryza 3 DX

| | Ryza DX | Ryza 2 DX | Ryza 3 DX |
|---|---|---|---|
| Install | 28 GB | 35 GB | 46 GB |
| RTTI game classes | 925 | 1,117 | 1,441 |
| `.ebd` / `.ebm` named in the exe | 1,691 / 1,683 | 2,141 / 2,133 | 3,382 / 3,379 |
| `.xml` tables named | 326 | 393 | 557 |
| Squirrel | no | no | no |

All three render through D3D11 on KTGL and elixir, the same base as Sophie 2. None of them
embeds Squirrel, so their AI is either native code or events, still to be checked.

### PAK archives (Sophie 2 and the Ryza trilogy)

All four games use the same PAK format: header `{0x20000, count, 16, 13}`, then 184-byte
entries with the layout given under Sophie 2. The second, fixed scramble over the names is
present in all four, so one reader with a key for each game covers them.

## What the games share

- KTGL (Koei Tecmo's engine) for models, textures, effects and sound, with Gust's elixir
  containers on top.
- XML tables for items, recipes, shops, quests and characters.
- The `.ebd`/`.ebm` event system, in every game. The command set grows from game to game.
- Squirrel 2.2.5, in Ayesha, Escha, Shallie and Sophie 2. One Rust Squirrel 2.2 VM serves
  them all.

## Plan

1. Format crates: PAK, elixir, g1t, g1m, kvs/ktsl2, the XML tables, and `.xml.e` decryption.
2. A viewer for models, maps and events, on a Vulkan renderer that also targets aarch64.
3. The event interpreter and the Squirrel 2.2 VM.
4. The oracle: a logging DLL inside the original game under Proton.
5. The game rules, Escha & Logy DX first: it has loose files, plain-text scripts and the
   simplest rendering. Shallie DX next, as it reuses almost all of Escha. Then Sophie 2 and
   the Ryza trilogy, and Ayesha DX last because of its PSSG data.

Mods are a load order of folders whose loose files override the game's: tables, events,
scripts and assets. They are part of the loaders from the start, not added later.
