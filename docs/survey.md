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

**How far Escha's readers reach in Shallie DX** (`barrel scan`, `g1m-scan`, `g1t-scan`,
`g1a-scan`, `g1h-scan`, `opl2-scan`, `ktf-scan`, `ebd-scan`, `ebm-scan`, `xml-scan`,
`voice-scan`, `vorbis-scan` over its install, unchanged but for the morph fix noted):

| Data | Read | Failed |
|---|---|---|
| Containers | 2,153 (106 zlib-chunked; versions 0 and 1) | 0 |
| Models | 6,771 (13,493 submeshes, 6,771 skeletons) | 0 |
| Textures | 11,885 in 2,259 packs | 0 |
| Animations | 8,286 | 0 |
| Morph files | 92 (211 sets, 1,986 targets; after two fixes: the older `0200` header and targets with more than a position per vertex) | 0 |
| Placement lists, terrains | 136, 136 | 0 |
| Events | 2,194 (270 distinct commands) | 0 |
| Messages | 53,027 in 5,040 files | 0 |
| Tables | 1,545 | 0 |
| Voices | 17,000 clips each in Japanese (19.2 hours) and English (11.5 hours) | 0 |
| Music | 246 | 0 |

The save coding is the same too: Shallie's `SYSDATA.pcsave` decodes with `save::decode` (2,992
bytes to 19,704, checksums matching) into the same record names (`system_data`, `ipu`, `bgm`,
`eventData`, ...), its data lengths big-endian where Escha's are little-endian (both
read); only the hand-written records stay raw, as in Escha, and the costume flag lists. What differs is the executable: every address in the docs is
Escha's, and Shallie's game rules (its calendar works differently) need their own reading.

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

The executable could not show the key: its code is encrypted by Steam's DRM wrapper.The Ryza DX executables are not wrapped (their `.text` reads
as plain code, entropy 6.4), so they can be studied as they are. It came from the names instead
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
this way (`\event\skill\skill_party_tao_99.ebd`) reads with Escha's event reader. 

**How far Escha's readers reach.** `barrel pak-scan FILE.PAK...` reads every texture,
model, animation, morph file, event and message in archives, inside containers too. Over
every archive of the four games, with Escha's readers plus the additions noted:

| Game | Containers | Models | Textures | Animations | Events | Messages |
|---|---|---|---|---|---|---|
| Sophie 2 | 1,067 | 9,274 of 9,281 | 3,439 of 3,461 | 6,092 of 6,095 | 2,780 | 3,607 |
| Ryza DX | 4,116 | 8,142 of 8,149 | 3,425 of 3,624 | 3,565 | 2,333 | 8,415 |
| Ryza 2 DX | 5,967 | 10,421 of 10,446 | 4,099 of 4,112 | 10,787 | 3,168 | 11,766 of 11,767 |
| Ryza 3 DX | 9,684 of 10,039 | 4,373 of 4,381 | 6,425 of 6,477 | 11,081 | 4,806 | 9,629 of 9,630 |

Additions so far: BC7, BC4, BC5, BC6H and float textures, cube maps and volumes;
animation flags 2 and 0x66; 32-bit index buffers; the later message layouts; container
version 2 (Ryza 3's environment and lighting containers, 72-byte entries). After them, what
fails is a handful per game: shader textures of another texture kind (`raindrop`,
`hair_highlight`, ...), two `headsup` textures in an unknown format, a few Ryza 3 models
with an unknown vertex attribute type, the light animations (`*-light.g1a`, Sophie 2) and
one Ryza 3 door animation, and the two test events' messages.

**Encrypted tables (`.xml.e`).** Many tables also come as `.xml.e`: a 16-byte header of
big-endian u32s `{2, plain size, 0, 0}` (`freeachievement.xml.e`: 2, 23,144, the size of
the `.xml` next to it), then data that is compressed and encrypted (entropy 7.94; 3,504
bytes for a table zlib packs into 2,232 to 3,288, so not plain zlib, and XORing it with
zlib's output gives a different stream for each file). Most have a plain twin in the same
archive, which Barrel can read instead:

| Game | `.xml.e` | with a plain `.xml` twin | Only encrypted |
|---|---|---|---|
| Sophie 2 | 377 | 373 | `saves\mix\mixpanellineeffectdata*`, `saves\recipeidea\recipetext*` |
| Ryza DX | 399 | 391 | `saves\fieldmap\sky\*` (6), `saves\recipeidea\*` (2) |
| Ryza 2 DX | 459 | 451 | 8 |
| Ryza 3 DX | 735 | 598 | `saves\fieldmap\cameraparam\enced\*` (133), `saves\mix` (2), `saves\recipeidea` (2) |

The ones with no twin include the synthesis panel and recipe-idea tables, so the format
has to be broken. The Ryza DX executable (not DRM-wrapped) shows how it reads them:

- `0x140094ee0` builds `%s.e` from a table's name; if that file exists it is read with
  `0x14049b590`, otherwise the plain `.xml` with `0x14049b2c0`.
- `0x140477010` checks the header: the first two u32s, big-endian; the first (the version)
  must be 1 or 2.
- `0x1404770b0` unscrambles the data in place, after `0x140477580` has dropped the
  16-byte header. Every stage draws from one generator: state = state × m + 0x2f09
  (32-bit), giving (state >> 16) & 0x7fff, with m = 0x3b9a73c9 unless a stage changes it.
  1. **Bit shuffle** of the last 0x800 bytes (`0x1404785b0`, seed 0x6d3f, in chunks of
     0x100 bytes): for each chunk, list its bit numbers, draw them out one by one at
     random index (value mod the number left), and swap the bits of each drawn pair
     (bit n is bit n & 7 of byte n >> 3).
  2. **Words** (seed 0xcb53): for each big-endian 16-bit word, draw r; if r mod 0x1462 ≥
     0xa31 XOR the word with r; then subtract r.
  3. The data now ends in a 0xff marker, then zeros, then 12 bytes: a sum check, a XOR
     check and a key (big-endian u32s). **Bytes**: m becomes key + 0x3b9a73c9, and three
     generators (states 0xa83b, 0xb11d, 0x88a5) take turns XORing bytes up to the marker
     with the low byte of their draws, in runs of 0x1d, 0x13 and 0xb bytes, each run one
     longer after every round of three.
  4. **Checks**: over the big-endian u32s before the marker, 0 minus their sum and the XOR
     of their complements must equal the two stored checks.
  5. **Bit shuffle** of the first 0x800 bytes (seed 0x74b9, chunks of 0x80), still with
     stage 3's multiplier (it is put back to 0x3b9a73c9 only afterwards).
  6. **Decompression** (`0x1404755e0`). The data is big-endian u32s and streams: the
     output size; the length of a command stream, then the stream; the length of a
     literal stream, then the stream; the length of an extra stream, then the stream.
     The command stream is packed itself (`0x140475e00`): a u32 count, then that many
     bytes in an Elias-gamma code read most significant bit first (k zero bits, then k + 1
     bits for the value; eight zeros mean 0). The commands then build the output:

     | Command | Then | Does |
     |---|---|---|
     | 1 | | copy one literal |
     | 2 | d | copy the byte d back |
     | 3 | a, n | copy n + 1 bytes from a + n + 1 back, plus one |
     | 4 | n (and a literal a) | the same, with a from the literal stream |
     | 5 | h, n (and a literal l) | the same, from (h << 8) + l + n + 1 back, plus one |
     | 6 | n | copy n + 8 literals |
     | 7 | (n from the extra stream) | copy n + 14 literals |

  `barrel_formats::xml_e::decrypt` does all of this (`barrel xml-e FILE OUT`;
  `barrel pak-xml-e FILE.PAK` decrypts every table in an archive and compares it with its
  plain twin). Over the four games:

  | Game | Identical to the twin | Differ | No twin, decrypted | Fail |
  |---|---|---|---|---|
  | Sophie 2 | 371 | 1 | 0 | 5 |
  | Ryza DX | 386 | 5 | 0 | 8 |
  | Ryza 2 DX | 450 | 1 | 0 | 8 |
  | Ryza 3 DX | 593 | 4 | 133 | 5 |

  Where a table differs, the shipped plain twin is the odd one out (in one, a damaged byte
  in a Japanese comment that reads 基本設定 in the decrypted copy). The failures are always
  the same few: `saves\mix\mixpanellineeffectdata*`, `saves\recipeidea\*` and
  `saves\fieldmap\sky\*`, which have no end marker after stage 3. No game loads them:
  none of the executables names `recipetext`, `mixpanellineeffectdata`, `a17_wmap` or the
  `bf_pointNN` tables (Sophie 2 loads `recipeIdeaScenario*.xml` instead, the Ryza games only
  `sky_default.xml` and `sky_weather.xml` from that folder), so they are leftovers in the
  archives and Barrel does not need them. `tools/xml_e.py` is the Python prototype.

  The executables also hold a second scheme, used by the game's own encoder at
  `0x140477630` (Ryza DX): word stage with seed 0xd5c1, test r mod 0xdb2 ≥ 0x6d9 and the
  XOR from a second draw; bit shuffle seeded 0x3745d in one 0x800-byte chunk; byte stage
  with states 0x9f73, 0xaee3, 0x394d and runs 7, 0xd, 0x17; plain sum and XOR checks; the
  key from a clock (`0x140b8c9f0`). It is probably for save data; the leftover tables do not
  decode with it either.

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
