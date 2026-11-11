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

An entry's name and its data are both XORed with two 32-byte keys, each repeating every
32 bytes: the entry's own key (the 32 bytes at +132, the same for every entry of an
archive in practice, and looking like leftover memory) and a fixed key for the game.
Sophie 2's fixed key is the text `dGGKXLHLuCJwv8aBc3YQX6X6sREVPchs`; with it, all 13,764
names in its archives and DLC read as plain paths (`\data\x64\res_cmn\ui\a23_item_l_139.g1t`,
`\data\x64\character\npc31b\npc31b_model.g1t`), and the data comes out as the formats
Escha uses (`GT1G0600`, zlib-compressed elixir containers).

The executable could not show the key: its `.text` is encrypted (entropy 8.0, with a
`.bind` section, the Steam DRM wrapper). It came from the names instead
(`tools/pak_key.py`): with the entry key removed, the byte at each position that makes the
most names read as paths, which repeats every 32 bytes.

`barrel_formats::pak` reads the archives; `barrel pak-ls FILE.PAK` lists one and
`barrel pak-get FILE.PAK NAME OUT` extracts a file. Sophie 2's UI textures use format
0x5f (BC7), which `barrel_formats::bc7` decodes.

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
entries with the layout given under Sophie 2, and the same two-key scramble. Only the
fixed key differs; each is a 32-character text that shows in plain sight wherever a name's
padding is zero (Ryza's entry keys are 20 bytes followed by 12 zeros, which leaves the
fixed key bare from byte 20 on):

| Game | Fixed key | Names |
|---|---|---|
| Sophie 2 | `dGGKXLHLuCJwv8aBc3YQX6X6sREVPchs` | 13,764 |
| Ryza DX | `EGkUyYzPidcRx5yTecqk5AYJeSzzc5BR` | 21,821 |
| Ryza 2 DX | `ivZBFzn82RVuYnmgXfeFEgk33ALZcZDP` | 28,065 |
| Ryza 3 DX | `fyrixtT9AhA4v0cFahgMcgVwxFrry42A` | 36,792 |

Every name in every archive reads as a path (`\data`, `\saves`, `\event`, `\dlc`), and
`barrel_formats::pak::Pak::parse_any` picks the key by that test. A Ryza event extracted
this way (`\event\skill\skill_party_tao_99.ebd`) reads with Escha's event reader. The
Ryza tables come as `.xml.e` next to `.xml`, a further encryption still to look at.

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
