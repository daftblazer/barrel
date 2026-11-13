# Formats

What Barrel reads so far, and how. Each section names the module that reads the format and
how far it has been checked against the games' own files.

## Elixir containers (`*.elixir.gz`)

Read by `barrel_formats::elixir`. In Escha & Logy DX all 1,580 containers read without
error.

The `.gz` in the name is misleading. A file is either:
- the container itself (magic `CRAE`, 1,301 files in Escha), or
- the same container compressed as a run of `{u32 size, zlib stream}` chunks, ended by a
  size of 0 (279 files).

The container layout:

| Offset | Field |
|---|---|
| 0 | magic `CRAE` |
| 4 | version, 0 or 1 |
| 8 | size of the data after the table |
| 12 | header size, always 28 |
| 16 | table size |
| 20 | entry count |
| 24 | flags: 10 in most files, 0 in a few |

Each entry is `{u32 offset from the container start, u32 size, name}`. The name is 48 bytes
in version 1 and 32 bytes in version 0; only two files, the battle containers, are
version 0. Entries of size 0 named `dummy` are placeholders: 18,959 of Escha's 33,332
entries.

What Escha's containers hold, by the first four bytes of each entry:

| Count | Size | Format |
|---|---|---|
| 6,010 | 504 MB | `_A1G` g1a animation |
| 6,005 | 367 MB | `_M1G` g1m model |
| 655 | 1,110 MB | `GT1G` g1t textures |
| 536 | 2 MB | `XF1G` g1e effect |
| 298 | 0.5 MB | `.bin`, no magic |
| 162 | 1 MB | `OC1G` g1co collision |
| 141 | 2 MB | `.opl2`, no magic |
| 141 | 3 MB | `_MHK` khm heightmap |
| 141 | 388 MB | `KFTK` ktf, alongside each field's heightmap, so likely terrain |
| 140 | 10 MB | `_S2G` g1s shaders |
| 126 | 37 MB | `_H1G` g1h morphs |
| 16 | 0.1 MB | `_SPK` kps |

The magic numbers are KTGL's own, byte-reversed (`_M1G` is G1M_).

## Textures (`*.g1t`)

Read by `barrel_formats::g1t`. Escha's 1,830 texture packs, loose and inside containers,
hold 8,778 textures, and all of them read. `barrel g1t FILE OUTDIR` writes a pack as PNG
files (`FILE.elixir.gz#N` names entry N of a container); `barrel g1t-scan DIR` reads every
pack. The decoded textures were checked by eye (an item's texture and a battle UI sheet).

```text
header (28 bytes): "GT1G", version as 4 ASCII digits ("0600" or "4600"), file size,
                   offset of the offset table, texture count, platform (10 for DX11 in
                   all but 14 packs), size of extra data (0)
u32 flags per texture, from offset 28
offset table: u32 per texture, relative to the table's start
per texture:  u8   mip count << 4 | depth
              u8   format
              u8   log2 height << 4 | log2 width
              u8×4 0, 0, 0x10, 0x21 in model textures (other values in terrain)
              u8   flags: 0x10 when an extended header follows (0x02 in terrain textures)
              [u32 extended size, 12 or 20, then size - 4 bytes; at 20 the true width and
               height are u32 at +12 and +16, for sizes that are not powers of two]
              the mip levels, largest first, tightly packed, rows top to bottom
```

| Format | Escha textures | Data |
|---|---|---|
| `0x59` | 5,357 | BC1 |
| `0x5b` | 3,283 | BC3 |
| `0x08` | 73 | BC3 (DXT5) |
| `0x06` | 62 | BC1 (DXT1) |
| `0x10` | 2 | BC1 |
| `0x01` | 1 | 8-bit RGBA |
| `0x5f` | none | BC7 (Sophie 2's UI textures, such as its 256×256 item icons) |
| `0x5c`, `0x5d`, `0x5e` | none | BC4, BC5, BC6H (the later games; BC6H for HDR environment cube maps) |
| `0x02` | none | one 32-bit float a pixel (Ryza 3's shore heightmaps) |
| `0x60` | none | 8 bytes a 4×4 block, read as BC1; the only ones seen (Ryza's `headsup.g1t`) are all zeros, so BC1 or BC4 is a guess |

**Format 0x03** is four 16-bit floats a pixel (RGBA16F). Sophie 2 uses it for its towns'
and fields' lighting volumes (`*_lpv.elixir.gz`, `*_ShIrradianceVolumes.g1t`, 9 files): three
texture arrays of 64 × 32 × 144 (kind 3, layer count 0x90 in the extended header; 2,359,296
bytes each, exactly 8 bytes a texel). Every value is a finite half float: the first channel 0
to 10.5 and never negative, the other three signed and small (-0.5 to 2.8), as the constant
and three linear terms of spherical-harmonics lighting would be; about a quarter are 0
(empty cells, presumably). Previews tone-map the colour channels like BC6H's.

**BC6H** (`barrel_formats::bc6h`, written from the BPTC_FLOAT specification: 14 modes, one or
two regions, whole endpoints or a base and signed deltas; previews tone-map with x / (1 + x)
and gamma 2.2). `charatest_ad.g1t` decodes to a smooth lighting cube, blue sky on face 2,
brown ground on face 3, sides shading from one to the other, which confirms the Direct3D face
order (+X, -X, +Y, -Y, +Z, -Z). Its blocks use modes 1, 3, 11, 12, 13 (22,682 of 24,576
blocks; the mode whose high endpoint bits are stored reversed) and 14; Ryza 3 uses all 14 (`barrel pak-bc6h
FILE.PAK...` counts the modes of every BC6H texture and names one that uses each): its
`charatest_as.g1t` and `field/env/texture/s013_default_as.g1t` cover the rest, and both
decode cleanly, the second a sharp beach panorama (palms, sea, cliffs; clouds and the sun on
face 2, sand on face 3) with no stray blocks, so every mode's bit layout checks out by eye.
Only the unsigned variant has been seen.

**Cube maps and volumes.** The first header byte's low nibble, 0 in all of Escha's
textures, says what a texture is: 1 a cube map (each level holds six faces;
`charatest_ad.g1t`, 256×256 BC6H, is exactly 6 × 65,536 bytes), 2 a volume, whose depth is 2
to the power of the fourth header byte's low nibble and halves with each level (Ryza's
`wavevolume.g1t`: 512×512×128 BC5, 2 levels, 37,748,736 bytes). 3 is a texture array and 4 an array
of cube maps; their count is bits 4 to 15 of the extended header's third word (Ryza 2's
`raindrop.g1t`: 0x100, 16 BC7 layers; Ryza's `hair_highlight.g1t`: 0x50, 5 cube maps, 30
faces; a top byte of 1 there is another flag). Inside a level the layers follow one another, each a whole image (`Texture::layer_rgba8`;
`barrel g1t` writes them all, `NNN_L.png`). In `hair_highlight.g1t`'s first cube map, faces 2
and 3 are empty and 0, 1, 4 and 5 carry the same horizontal highlight band, which fits the
Direct3D order +X, -X, +Y, -Y, +Z, -Z (up and down third and fourth), as `charatest_ad.g1t`
confirms (below); the side faces' orientation still needs a seam check, and the level order in arrays of cube maps is
still to be checked.

Every texture's data is exactly the size its format, size and mip count give, so there is
no padding or swizzling on PC. The renderer can upload BC1, BC3 and BC7 as they are;
`barrel_formats::bc` and `barrel_formats::bc7` decode them in software for tools. The BC7
decoder covers all eight modes; its partition and anchor tables are checked against each
other by a test (every anchor pixel lies in its own subset).

## Models (`*.g1m`)

Read by `barrel_formats::g1m`. All 6,005 of Escha's models read: 10,068 submeshes, 7.8
million vertices, 6.6 million triangles. `barrel g1m-obj FILE.elixir.gz#N OUT.obj` writes a
model as OBJ; `barrel g1m-scan DIR` reads every model and checks every index. Escha's own
model (`Character/PC20`), drawn from the OBJ with a software rasterizer, comes out right.

A model is a header (`_M1G`, version `0073`; `0053` in 8 old models) and chunks, each
`{magic, version, size}` with the size counting the 12-byte header:

| Chunk | What |
|---|---|
| `FM1G` (G1MF) | counts and statistics |
| `SM1G` (G1MS) | the skeleton (below) |
| `MM1G` (G1MM) | matrices |
| `GM1G` (G1MG) | the geometry, version `0044` in every model |
| `RTXE` (EXTR) | extra data |

G1MG starts with a platform tag (`DX11`, or `DX9` in the old models; the layout is the
same), a u32, a bounding box (six floats) and a count of sections. Each section is
`{u32 type, u32 size, u32 count}` followed by its entries. All nine types are in every
model:

| Type | Entries |
|---|---|
| 0x10001 | not decoded yet |
| 0x10002 | materials: `{u32, u32 texture count, u32, u32}`, then per texture `{u16 index into the model's g1t, u16 layer, u16 type, u16 subtype, u16 tile u, u16 tile v}` |
| 0x10003 | shader parameters, not decoded yet |
| 0x10004 | vertex buffers: `{u32, u32 stride, u32 vertex count, u32}`, then the vertices |
| 0x10005 | vertex layouts: `{u32 buffer count, u32 buffer numbers[], u32 attribute count}`, then per attribute `{u16 buffer (position in this layout's list), u16 offset, u8 type, u8, u8 semantic, u8 layer}` |
| 0x10006 | bone palettes: `{u32 count}`, then per entry `{u32, u32 flags (0x80000000), u32 joint}` |
| 0x10007 | index buffers: `{u32 count, u32 bits (always 16), u32}`, then the indices, padded to 4 bytes |
| 0x10008 | submeshes, 14 u32 each: flags, **layout**, bone map, ?, ?, shader parameters, material, index buffer, ?, primitive (3 triangle list, 4 strip), first vertex, vertex count, first index, index count |
| 0x10009 | mesh groups: per LOD and pass (0 colour, 1 shadow) the shader by name and the submeshes drawn with it (`Geometry::mesh_groups`, `shading.md`); after each name two words, 0 or (in the shadow pass) a count in the top byte, then -1: no draw priority |

A submesh names a vertex layout, not a buffer. A layout lists one or more buffers of the same
length: characters keep positions in a 12-byte buffer of their own and the rest in a
second one.

Attribute types: 0, 1, 2, 3 are 1 to 4 floats; 5 is four bytes (bone indices); 13 is four
normalized bytes (colour). Semantics: 0 position, 1 weights, 2 bone indices, 3 normal,
4 tangent, 5 UV, 10 colour.

### Skeleton (G1MS)

All 6,005 skeletons read: 37,956 joints. Offsets in G1MS count from the start of the chunk,
its 12-byte header included.

| Offset | What |
|---|---|
| 12 | u32 offset of the joints |
| 16 | u32, 0 |
| 20 | u16 joint count |
| 22 | u16 index map count |
| 24 | u16 layer |
| 28 | the index map: u16 per entry, 0xffff for none, padded to the joints |
| joints | 48 bytes each: scale (3 floats), u32 parent (top bit set for a root), rotation quaternion (x, y, z, w), position (3 floats and a 1.0) |

A joint's parent nearly always comes before it, but not always: in Ryza 3's
`obj-A24-S052-FACILITY-DOOR-02_hi.g1m` joint 1's parent is joint 2. So the reader only rules out
parents outside the skeleton and cycles, and `Skeleton::order` gives a parents-first order to
compose world transforms in:
position = parent position + parent rotation × local position, rotation = parent rotation ×
local rotation. Escha's skeleton (`Character/PC20/PC20_MODEL.g1m`, 345 joints) comes out
standing in its bind pose: pelvis at 88 units, knees at 51, toes near 0, head at 157.

Characters split the skeleton from the meshes. The base model (`PC20_MODEL.g1m`) holds the
skeleton and almost no geometry; the `_default` model holds the meshes and a G1MS with one
placeholder joint and the index map alone. The Oid.bin names are the scene's nodes, not the
joints one for one (helpers such as the thigh-twist nodes sit in between), so naming the
joints needs the index map; that is still to be worked out.

### Skinning

The semantics are Direct3D's declaration usages: 0 position, 1 blend weights, 2 blend
indices, 3 normal, 4 PSIZE (billboard corners, `docs/shading.md`, "Billboards"), 5 texture
coordinates, 10 colour.

A vertex's bone-index byte (semantic 2) divided by 3 is a slot in the bone palette its
submesh names (submesh word 2); the slot's joint word, without its top bit, is the joint
number in the skeleton. The top bit marks a joint of the base model's skeleton, which is
how a `_default` model's meshes reach the skeleton in `PC20_MODEL.g1m`. Weights are
semantic 1.

Checked on Escha: with this mapping her vertices lie a median 7.9 units from the joint that
weighs most on them; reading the joint through the index map instead gives 23, and the
palette's first word 69 to 79. Across all of Escha's models, every vertex's bone slot is
inside its palette (`barrel g1m-scan` checks it).

## Animations (`*.g1a`)

Read by `barrel_formats::g1a`. All 6,010 of Escha's animations read (version `0042` in
every one): 430,213 bone tracks, 21.2 million keys. `barrel g1a-scan DIR` reads them all.

Sizes and offsets count 16-byte units.

```text
header:  "_A1G", "0042", u32 file size (units), u16 type, u16, f32 duration in seconds,
         u32 data offset (units), u32 ×4
at 0x30: u16 bone entry count, u16 highest bone number,
         then per entry {u32 bone, u32 offset of its block, in units from 0x30}
block:   u32 flag, then {u32 key count, u32 offset in units from the block} per component
keys:    key count × (a, b, c, d) floats, then key count floats: each segment's end time
```

| Header type | Files | Flag | Blocks | Components |
|---|---|---|---|---|
| 0x3100 | 1,575 | 6 | 380,476 | ten: scale x, y, z; rotation quaternion x, y, z, w; translation x, y, z |
| 0x2100 | 60 | 4 | 803 | seven: rotation, translation (no scale); the field objects' animations |
| 0x104 | 4,375 | 0x191 | 48,934 | one scalar: the facial animations (`*_M_Brw_*`, `*_M_Eye_*`, `*_M_Mth_*`), whose "bones" are morph weights |

Sophie 2's animations are version "2400" and add two flags: **2**, four components, the
rotation only (most joints of its body motions, `CF00_000000.g1a`: 59 of 63 blocks), and
**0x66**, nine components, a camera: eye x, y, z, target x, y, z, then three more (0, 0.7
and 1.0 in `PC02_01230_cam.g1a`; roll and the field of view in radians, probably). In
Sophie 2 the blocks' headers sit together and their keys after them, so a block's offsets
are not right behind its own header. For a flag it does not know, the reader infers the
component count from the offsets when they do follow the header.

Ryza 3 adds flag **8** in one file, `obj-S161-TEMPLE-DOOR-01.g1a` (a door swinging open): seven
components, scale x, y, z (1.0) then the rotation quaternion (turning about y), no
translation; the joint keeps its bind position. Its blocks' headers sit one after another, so
the reader cannot infer the count there and knows the flag instead.

The header type and the flag always go together, and every one of the 21.3 million keys
decodes. A facial track is the same kind of spline over a single value: an eye blink eases
from 0 to 1 with (-2, 3, 0, 0) and back with (2, -3, 0, 1).

The order of flag 6's components comes from the data: the first three are 1.0 in 95-99% of
blocks (scale at rest), the seventh's median is 0.97 (a quaternion's w).

A component is a cubic spline: inside a segment, value = a·s³ + b·s² + c·s + d with s from 0
to 1 between the previous key's end time (0 for the first) and this key's. The segments
join: the end value of one is the d of the next, which confirms the 0..1 parameter.
`g1a::sample` evaluates a component and `Track::pose` gives a bone's rotation, translation
and scale at a time. The breakable barrel's animation (`obj-a15_taru_brk_a.g1a`, 0.53 s)
moves its pieces outward; most bones hold still with one key.

**Eye materials.** In the character models, a material with four textures and none of
subtype 1 is an eye: texture 0 the eye white, 1 the iris (BC3, with alpha), 2 a glow, 3 the
highlights. All four use the eye mesh's single UV set.

## Decoding Vorbis (`barrel_formats::vorbis`)

The voices (and the other Ogg sounds) are decoded natively, from the Vorbis I specification
and RFC 3533: no outside decoder. Done so far: Ogg pages into packets, the identification
header, and the whole setup header: codebooks (Huffman codes assigned from the lengths, lowest
free code first; lookup types 1 and 2 unpacked into vectors), floors (types 0 and 1),
residues (types 0 to 2, with their cascade books), mappings (coupling, submaps), modes, and
the framing bit. `barrel vorbis-scan SeriesNNNN.fpl`
reads every clip of a bank: all 18,000 voice clips are mono, 48 kHz, block sizes 256 and 2048,
with 39 codebooks (a few more in some), and every setup header reads to its framing bit; so do
the 247 music files (`Sound/BGM/*.kvs`, KOVS like the voices; `vorbis-scan DIR`).

The audio packets decode too (`vorbis::Decoder`, `decode`): floor 1 (its Y values, then the
curve through the used points and the inverse-dB table), residues 0, 1 and 2 (the classbook's
classifications per partition, eight passes), the inverse coupling, floor × residue, the
inverse MDCT (a DCT-IV through an n/4-point FFT, checked against the definition), the
power-sine windows with their short/long slopes, and overlap-add between the windows' centres;
the stream is cut to its last page's granule position. Floor 0 is not supported (no file
uses it so far). `barrel voice-wav SeriesNNNN.fpl CLIP OUT.wav [OUT.ogg]` writes a clip;
against ffmpeg's decoder the clips tried (banks 0, 3, 10, 14, 17) come out the same length and
within 3 of 32,767 at every sample (rounding), and a 5.8-second clip takes 0.05 s.

## Music

`Data/x64/Sound/BGM/` holds 248 KOVS files (the same wrapper as the voices, decoded the same
way): 221 named (`ev_usual01.kvs`, `bt_boss02.kvs`, `sy_item.kvs`) and 27 numbered
(`11547.kvs` ...). The executable lists the tracks by number at `0x1407e5ef0` (English Escha
executable): 245 records of 0x38 bytes, each the PC file (`DISC:Sound/BGM/<name>.kvs`), the
PlayStation (`.at9`) and Vita (`.kno`) files, a volume (a float at +0x2c: 0.5 for most event
music, 0.7 to 1.0 for the rest) and the Japanese title at +0x30 (track 1 "タイトル画面　ピアノ
8ch", 104 "日常1", 115 "トラブル発生a", 244 "MARIA(イグドラシル戦BGM)"). Events name tracks by
these numbers: `ECT_SOUND_BG_PLAY` `{track, volume %, 0, 0}` (805 uses, 116 tracks),
`ECT_SOUND_BG_STOP` and `ECT_SOUND_BG_FADEOUT` `{hundredths of a second, 0}` (1,220 uses,
mostly 1 or 1.5 s); `EVENT_MM08_AS4_130` stops the music, plays 105 ("日常2") and then the
item jingle (6). `barrel_formats::bgm::tracks` reads the list and `barrel play-event` prints the
music steps with their files and titles.

A music file's KOVS header carries its loop after the Ogg size: +0x08 the sample the loop goes
back to, +0x0c the loop's end (0 in every file: the stream's end), and +0x10 1 for a piece
played once. Of the 247 tracks 57 play once (the jingles: `sy_item`, the orchestral title), 184
loop back to a point partway in (`ev_usual01` to sample 177,664 of 3,007,104, `bt_boss02` to
314,838), and 6 loop whole; no loop point lies past its track's end. This is read from the
files, not from the player's code (`voice::looping`; `barrel_audio::Mixer::play` takes the
frame to loop back to).

## Sound effect banks (`Sound/SE/*.wh2` + `.wb2`)

Koei Tecmo's KWB2: a header file (`.wh2`: `_HBW0000`, its size, `KWB2`, a u16, the sound count,
the data size, a 1.0, and at +0x24 one u32 offset per sound) and a data file (`.wb2`:
`_DBW0000`, its size, then the samples from 0xc). Each sound holds one or more blocks
`{u16 rate, u8 0, u8 channels, u32 codec, u32 0, u32 samples, u32 offset, u32 size}`, found
by that shape in its span (`barrel_formats::kwb`). `A15_bank_se`, the only bank on disk, has
127 sounds in 226 blocks, all codec 2 (16-bit PCM), mono, 48 kHz, 284 seconds in all; most
sounds have two blocks with the same samples (what the second is for is not read).
`Sound/SE/env/` (`00ENV_CAVE01.kvs` ...) and `Sound/SE/stream/` are KOVS like the music.
`A15_bank_se` is the only bank: the executable names just `DISC:Sound/SE/A15_bank_se.wh2` (the
sound library's `KTSL_SE_NUM_BANK_MAX` allows more), and no container holds one (`barrel scan`:
no sound entries in the 1,580 containers). The first entry starts with the offsets of the last
three (what for is not read). The executable also has 38 `SE_*` tags (`SE_JUMP01`,
`SE_HIT01`, ...). `barrel se-scan BANK.wh2` lists a bank.

**Effect numbers.** The player (`0x140160070`) takes a number up to 151 and looks up its group
(`0x1407e96a0`, one i32 per number) and its index within the group: 0 to 127 are group 0, the
bank's sounds by number (`0x140501330`); 128 to 151 are group 2, the 24 ambience loops by the
pointer list at `0x140d5fb10` (`RES:Sound/SE/env/00ENV_CAVE01.kvs` to `23ENV_RAIN01.kvs`,
`0x1405014e0`); group 1 (`0x140501540`) has no numbers. Above 151 it plays nothing.
`u8/0x10` (`0x75100008`, 234 uses; handler `0x14005210c`, case 6 of the `0x75100002` table at `0x140052e70`; `ECT_SOUND_SE_PLAY`, `0x75100001`, 5 uses, is dispatched elsewhere and taken to work alike) is `{effect, a, volume %, pan ×100,
d}`: volume and pan are divided by 100; the pan (0 left, 64 centre, 128 right, 64 by default)
becomes `(clamp(x, 0, 128) / 128 - 0.5) × 2` (`0x140160230`, the player's +0x19a0); `a` goes to
`0x1410c8428` and `d` to +0x199c (neither named). Its five uses (the `BSP_EV_SE_*` parts) name
113 (a door opening) and 236 to 244, which play nothing. `ECT_CHARA_SE_PLAY` (`s62`, 18 uses in
battle skill events) puts a character first; its numbers (151, 119, ...) look like another
numbering, and its handler is not read. `barrel_audio::Library::effect` follows the player.

## Shaders (`Data/x64/Shader/*.g1s`, `_S2G`)

80 shader packs, one per material kind: `CharaMT*` (characters: `CharaMTC`, `CharaMTCN`,
`CharaMTCNO`, `CharaMTSR`, ... and `CharaEYEL`/`EYER`, `CharaFraHAIR`/`BODY`/`SKIRT`),
`MapSTD*` and `MapWater*` (fields), `STD*`, `SM*_Psm` (shadow maps), `E3dS*` (effects),
`Ipu*`, `Contrast`, `Refraction*`, `VertexColor*`. A pack (`_S2G3000`, u32 size, a name such
as `CharaMT-CP`, a table not read yet) holds Direct3D 11 programs as DXBC containers one after
another (`barrel_formats::dxbc::find`): `CharaMTC.g1s` has 14, twelve vertex-shader variants
and two pixel shaders. Each container reads with `dxbc::program`: its constant buffers
(RDEF), bindings, inputs and outputs (ISGN/OSGN) and its code (SHEX, Shader Model 5).
`CharaMTC`'s pixel shaders sample `sDiffuse` and `sMask` and read `$Globals` `{vATest (the
alpha-tested variant only), atColor, HdrRangeInv, atCharaPointLightStrength,
atCharaLightColor, atCharaRimLightColor}`; their vertex shaders carry `atCharaLightDir`,
`atCharaRimLightDir`, the lights (`litPos`, `litCol`, `litDir`, `litAtt`, `litAmb`,
`nDirLit`) and the skinning matrices. `barrel g1s FILE` lists a pack.

## Interface art (`Data/x64/WinXls*/`, `Data/x64/Ipu*/`)

`WinXls` (and `_EN`, `_SC`, `_TC` for what has text) holds the interface textures as plain
g1t packs: `a15_common.g1t` (29 textures: button glyphs, menu words, frames, numbers, the
message window pieces in texture 28, item and character icons), `a15_menu`, `a15_battle`,
`a15_mix`, `a15_saveload`, the monster dictionary pages, help pages, posters, and
`a15_telop_<event>` title cards for a few events (`mm01_op1_111`, `mm01_op1_212`, ...).
`Ipu` holds full-screen images (event stills `ev_still_*`, tutorial stills, title
backgrounds, logos). Where each piece of a texture is placed on screen is in the executable's
interface code (not read).

## The font (`Data/x64/Font_EN/MainFont_x64_0.g1t` and a table in the executable)

Each language has one atlas (`Font`, `Font_EN`, `Font_SC`, `Font_TC`): a 2048×2048 BC3 texture
with the glyphs white in its alpha, packed in no grid. Where each glyph is comes from the
executable: just before the file name (`0x14079ee58` in the English one) a descriptor ends
with a pointer to the name and the glyph count (2,947); after the name's 0x18 bytes come the
glyphs, 28 bytes each: `{u32 code, u16 x, u16 y, u16 width, u16 height, i32 x offset, i32 y
offset (from the line's top to the glyph's), u32 advance, u32 0}`, in rising code order. Codes
0 to 126 are ASCII by number (0 to 31 are 3×3 blanks); the rest are the character's UTF-8 bytes
read as a big-endian number (`0xc2a2` ¢, `0xe280a6` …). The Latin glyphs reach 53 pixels below
the line's top; a space advances 10, `A` 23 (`barrel_formats::font`; `barrel font-text EXE
FONT.g1t OUT.png TEXT` lays out a line from both). The other languages' tables are not read yet.

The font also holds the 33 button icons that `<IMnn>` puts in a line: codes `0xd090` to
`0xd0b0` (which would read as Cyrillic А to а), 50×50 at the atlas's top. The colours that
`<CLxx>` names are a table at `0x1407b49b0` in the English executable, 16-byte records
`{u64 name, u8 r, g, b, 0}`: `NR` ffffff, `BK` 000000, `RE` c80000, `GR` 00aa00, `BL` 5050f0,
`R1` ff5050, `G1` 0af000, `P1` 882244, `YL` ffff00, `EG` 1eff1e, `BR` 3a1a08; the tag names
that follow them are `CR`, `CL`, `#`, `CS`, `IM` (what `#` and `CS` do is not read). The English
events use `<IMnn>` 23 times (tutorials) and `<CLGR>`/`<CLNR>` 5 times (accessories gained).

## Saves (`GAMEDATA<NN>.pcsave`)

The game saves to `Documents/KoeiTecmo/Atelier Escha and Logy DX/SAVEDATA/` (its
`ROOT:savedata/GAMEDATA%02d`). A `GAMEDATA` file starts with 0x100 plain bytes, the summary
the load screen shows: i32s from +0x28 (5, 28, 34, 2954, 12, 0, 1, 29, ... in one save; the
last two read as u16 pairs, 12 and 14, 12 and 15), -1 padding from +0x74, and at +0xa4 the time
of saving as a Windows `SYSTEMTIME` (year, month, day of the week, day, hour, minute, second,
milliseconds). The summary's words match the state: +0x28.. play time, presumably (5, 28,
34: hours, minutes, seconds), then the money (2954) and the alchemy level (12), and from the
eleventh word the squad as the leader's level (12) and (character, level) pairs (9 and 11, 7
and 15, 2 and 12, 1 and 12; the last two levels' high halves hold 14 and 15, not read).
The coded game state follows. `SYSDATA.pcsave`, the settings, is coded whole.

**The coding** (`0x1400219a0 (codec, buffer, length, &coded_length, encode)`, in place; the
settings writer `0x14015d380` calls it with encode 1 and `fwrite`s the result). Decoding
(`barrel_formats::save::decode`) undoes, in order:

1. trailing zero bytes are dropped and the rest padded to 16 (`P` bytes);
2. a bit shuffle over the last `min(P, 0x800)` bytes (`0x14001fe10`): the indices of their
   bits are drawn out one at a time from a stream, entry `(s & 0x7fff) % left` of those
   left, and each two drawn in a row swap bits (its own inverse);
3. each big-endian u16 word: `r = s & 0x7fff` drawn; when `r % 0xec2 ≥ 0x761` the word is
   xored with the next draw `& 0x7fff`; then `r` is taken off;
4. the 16-byte trailer: byte `P - 0x10` is 0xff, then big-endian u32s: `P - 0xc` the xor and
   `P - 8` the sum of the data's big-endian u32s, `P - 4` a seed;
5. the data bytes (`P - 0x10`) xored with the stream's low byte in runs: keys 0xa22f, 0xa129,
   0x394d in turn for 7, 13 and 23 bytes plus the round number, each key carrying on where its
   last run stopped; the sums are checked after;
6. an Exp-Golomb byte code (`0x140020a80`): a big-endian u32 count, then per byte, from the
   top bit down, `k` zeros and the `k + 1` bits from the next one give `v`, the byte `v - 1`
   (a lone one is 0, eight zeros 0xff);
7. a byte substitution (`0x140020740`): a count `n` (0 for 256), `n` table bytes, a big-endian
   u32 length and that many indices into the table.

`save::encode (state, seed)` codes a state the other way (the steps in reverse: the state's
trailing zeros dropped, a byte table of the distinct bytes most frequent first, the
Exp-Golomb packing, padding to 16, the seeded byte runs, the trailer, the word pass adding `r`
before the conditional xor, the bit swaps). Decoding gives the state back, for the settings
file and for a 300,868-byte game state, whose coding comes out 66,368 bytes, the size of the
game's own file (the game's table order is likely the same: most frequent first).

The stream is `b = a × b + 0x2f09` (u32), its value `b >> 16`, with `a` = 0x3b9a73c9 (plus the
seed for step 5) and `b` set to each step's key (0x3745d, 0xd727, the three run keys). When
encoding, the seed is `0x1403b7390`'s value (so two saves of one state differ).

The state is the save buffer the game reads (`[0x140e00f30]`, 0x5000 bytes for the
settings): a 0x20-byte header starting with the i32 20130322 (a date stamp, 2013-03-22),
then a u32 length and a name (`system_data`), then named sections, each a NUL-terminated name
and a little-endian u32 length. The settings hold `ipu`, `bgm`, `movie`, `buc`, `eventData`,
`eventFlag`, `item`, `recipe`, `book`, `monster`, `fieldMap`, ..., `costumeData`, `option`,
`saveData` (2,912 bytes coded, 19,137 decoded); a game save `dlc`, `event`
(`m_eventListSaveData`, `m_eventFlag`, ...), `knowledge`, `item`, `recipe`, `book`,
`fieldMap`, `monster`, `mixEsp`, `potential`, `money`, `party`, `m_squad`, ... (66,368 bytes
coded, 300,855 decoded). The coding was found from a memory read of a short test run: the
decoded settings sat in the save buffer at the title screen, and the code that fills it led
to the codec; the decoder reproduces that memory byte for byte. The sections' fields are not
read yet.

**Records** (`save::state_records`): after the 0x20-byte header, each record is a
little-endian u32 size counting itself and a NUL-terminated name, then most often a u32 length
and that many bytes (`Body::Data`; Shallie's saves, coded the same way, give that length big-endian), or child records filling it exactly (`Body::Group`:
`system_data`, `option`, `event`, `knowledge`, `item`, ...; or a u32 count and at least that many
records, `party`). Some are written by their own code
and kept raw (`Body::Raw`: `FLAG` is two bytes and a `CLEAR` record of two bytes; `m_recipeData`
a byte and a count of 265 entries; `costumeData` a count and length-value pairs; `money` 4
bytes). Encoding drops the state's trailing zeros, so `decode` pads the state
back until its top records are whole. A game save's top records: `dlc`, `event`
(`m_eventListSaveData`, `m_eventFlag`, `m_isDoneToday`), `knowledge` (`item`, `recipe`, `book`,
`fieldMap`, `monster`, `mixEsp`, `potential`, `eff`), `money`, `party`, `fieldmap`
(`m_userDataFieldMapData`, `...Gimmick`, `...Enemy`, `m_userDataItemBoxOpenNum`), `library`,
`item` (`m_unitKago` the basket, `m_unitContainer`, the `...Important` pair, `m_unitMaterial`,
`limitSize`, `ID`), `time`, `fieldmap_effect`, `event_check`, `date`, `global`, `mix_lv`
(`m_lv`, `m_exp`), `explore_equip`, `worldmap`, ... `barrel save FILE` prints the header and
the record tree (`BARREL_SAVE_BYTES=n` adds each record's first bytes).

Fields read so far (`save::summary`): `money` an i32 (the header's fourth summary value),
`mix_lv/m_lv` and `m_exp` the alchemy level (the header's fifth value) and experience, `date`
a u32 first, the game's day number (`calendar::Date::of`: 229 is 20 August of school year 1,
139 days after a new game's 1 April), `item/limitSize` the basket's and the
container's sizes (60, 999). The item lists (`m_unitKago` 120 slots, `m_unitContainer` 999,
`m_unitMaterial` 200, the `...Important` pair 100 each) are 36-byte units (`save::ItemUnit`):
a serial (+0; `item/ID` holds the next), the `itemData.xml` row (+2), the quality as a float
(+4), three trait rows (+8), a word the battle code checks (+0xe), four effect rows (+0x10),
a word (+0x18) and four counts (+0x1c ...: the battle code's uses left first); a serial of
0xffff marks an empty slot. One save's basket reads Paper Scraps, Glass Core, Antique Stone, ...
(`BARREL_ITEMS=itemData.xml` names them in `barrel save`). `party` is a u32 count (11) and that many `Party` records, then
`m_squad` (11 i32 slots: the squad's characters in order, -1 empty; 5 used in one save). A
`Party` record (698 bytes, `save::Member`) starts with the character (0 to 10), level,
experience, a word (392 in every one), HP and MP now and their maxima (Escha in one save:
level 12, 694 experience, HP 24 and MP 43 now, base max HP 30 and MP 47), then base attack,
defense and speed (8, 8, 13), five small values (resistances, presumably) and two constants
(5 and 25), the same block with equipment applied from +0x114 (max HP 35, attack 13), the
character's skills from +0x230 in command order (`barrel party-skills`), ten 12-byte entries
of the skill row, 0 and 1 (Escha: 147 Attack, 148 Ether Shoot, 149 Protection, 150 Ether
Spear, 156 Ether Mark, 151 Super Energetic, ...), then the Move skill (+0x2a8, 157), the level
again (+0x2ac) and the **friendship** float (+0x2b0): 0 for Escha, 15.02 and 14.69 for two
companions, 0.99 for others. That is the calendar's daily gain (`days::World::friendship`,
the runtime record's +0x2a4: the save's record is the runtime one with 12 bytes in front):
139 days at most 0.11 a day give at most 15.29, nine days 0.99, and the leader none.

The alchemists' battle items (`explore_equip/explore_item_escha`, `explore_item_logy`, 25
item units each; `Summary::battle_items`), with their uses left in the first count (2 of 3 for
a Bomb; 0 for a spent one). `barrel battle EXE MONSTER --from SAVE` fights with the save's
squad, its first three (each at its level's growth row with the saved equipped max HP, attack,
defense and speed, starting at its HP and MP now) and those items (quality, effects, traits and
uses as saved, target and wait from the battle item table).

The events (`event/m_eventListSaveData`, `save::EventRecord`): 1,165 records of 36 bytes, one
per `EVENT_*` name in the executable's enum order (`EVENT_AA01_010`, `EVENT_MM01_OP1_011`, ...),
the array at `0x1410a42d0` saved whole: seen (+0, 94 in one save), the day it was seen (+4,
-31 never; 90 for the new game's first events), how many times (+8), the day's roll (+0x10:
the calendar's daily rolls, `0x1410a42e0` + 0x24 × the event) and a value from 0 to 2
(+0x14). `m_eventFlag` (604 bytes) is the 151 event flags at `0x1410932d0`, as words
(`Summary::event_flags`; mostly 0 with a few 1s and a 5).

`global` (68 bytes) is the block of globals at `0x141098a10` copied as it is (its writer,
`0x14035e7b0`, found through the record-name table at `0x140753100`: name getter, write, read
per record; the reset `0x14035e8e0` sets +0 to -1 and the rest to 0). At +0x38 is the
protagonist the player chose, 0 Escha or 1 Logy: what "Master Character" (`Chara.xml` row
749) stands for in events (`docs/events.md`; `Summary::hero`). The user's save says Escha.

**The record handlers.** Each top-level record has a small object in the executable's data:
a vtable, a helper, the name getter, the write and read functions, then the name inline.
`tools/save_records.py` lists them from the English executable (the writers fill each record;
the readers are where to look for any field not yet mapped):

| record | write | read |
|---|---|---|
| `event` | `0x14035ebc0` | `0x14035ee90` |
| `global` | `0x14035e7b0` | `0x14035e840` |
| `dlc` | `0x140168680` | `0x1401686b0` |
| `knowledge` | `0x140369970` | `0x14036a250` |
| `money` | `0x14036a9a0` | `0x14036a9f0` |
| `party` | `0x1403625d0` | `0x140362700` |
| `fieldmap` | `0x1403682b0` | `0x140368740` |
| `fieldmap_effect` | `0x1403648f0` | `0x140364b50` |
| `library` | `0x140366870` | `0x140366c60` |
| `item` | `0x14035ccc0` | `0x14035d360` |
| `date` | `0x14035c180` | `0x14035c1f0` |
| `time` | `0x14036ad40` | `0x14036adb0` |
| `worldmap` | `0x14035dd00` | `0x14035e0c0` |
| `shop` | `0x14035c2e0` | `0x14035c420` |
| `explore_equip` | `0x140365640` | `0x140365be0` |
| `dlcbgm` | `0x14036cea0` | `0x14036cfd0` |
| `event_check` | `0x140368150` | `0x1403681b0` |
| `mix_lv` | `0x14036a730` | `0x14036a830` |
| `mix_recipe` | `0x140367e50` | `0x140367f10` |
| `subject` | `0x140360760` | `0x140361190` |
| `quest` | `0x140363cf0` | `0x140364130` |
| `bonus` | `0x14036b940` | `0x14036bce0` |
| `delivery` | `0x14036c4c0` | `0x14036c5f0` |
| `poster` | `0x14036b400` | `0x14036b4d0` |
| `monthrep` | `0x1403634b0` | `0x1403635f0` |
| `work` | `0x140366440` | `0x1403665f0` |
| `system_data` | `0x1401622d0` | `0x140162700` |
| `hom` | `0x1401688e0` | `0x140168930` |
| `option` | `0x140164190` | `0x1401641b0` |

`system` (the `GAMEDATA` stub; the real one is SYSDATA's `system_data`) has empty handlers.

`fieldmap/m_userDataFieldMapEnemy` and `m_userDataFieldMapGimmick` (47,340 bytes each,
written by `0x1403682b0`): a count of field maps (122), then per map a u32 384 and that map's
384 bytes copied from a global array (the enemies' at `0x141087bd0`, the gimmicks' at
`0x14107c4d0`, 0x180 bytes a map): 32 entries of 12 bytes. An enemy entry's reset value
(`0x140368cf0`) is `{-1, -1, 1}` (an 8-byte -1, then 1); a gimmick's in the user's save is
`{1, 0, 1}` everywhere. In the user's save every enemy entry is still the reset value, and a live run
(2026-11-12) showed why: beating an ordinary field enemy in the Southern Ruins and leaving to
the area map changed none of the 47,232 bytes at `0x141087bd0`, so ordinary enemies are not
recorded there (they come back). Nothing else does either: apart from the save's writer and
reader and the two resets (`0x140368cf0`, and the new game's `0x140369090`, which reaches it
as the image base + 0x1087bd8), no code refers to any address inside the array, directly or
from the image base, so in the shipped game it always holds the reset values and Barrel only
has to carry it through a save; the field's out-or-beaten check (`0x140075e10`) looks enemies up by area in a map kept
at the field (+0x228, +0x40).

**Knowledge** (`knowledge`, written by `0x140369970`): the library's levels, one i32 per
entry in eight lists that are one block in memory at `0x141079fe0` (0x24e4 bytes, zeroed by
`0x1403692d0`), each saved as its own record:

| record | entries | offset | what |
|---|---|---|---|
| `item` | 560 | 0 | `itemData` rows |
| `recipe` | 265 | 0x8c0 | recipes |
| `book` | 73 | 0xce4 | recipe books |
| `fieldMap` | 122 | 0xe08 | field maps |
| `monster` | 153 | 0xff0 | monsters |
| `mixEsp` | 90 | 0x1254 | synthesis skills |
| `potential` | 400 | 0x13bc | traits |
| `eff` | 698 | 0x19fc | effects |

Levels only rise: the setters (`0x1403692f0` items, `0x1403694f0` recipes, `0x140369560`
books) keep the higher value unless their third argument forces it, and an item reaching 2
or more posts a library notice (message 0xe). Taking an item (`0x14029f4f0`, by how it came:
kinds 0 to 4 at 10 and its recipe book at 2 via `0x140369520`, any other kind at 2) runs
`0x140369840`, which raises the item, its four effects, its three traits (not while the flag
at `0x1410782e3` is set) and the item row's two synthesis skills (`0x1402a35b0`) to that
level. So 10 is known in full (made or gathered) and 2 seen (bought or given). One save holds
items at 0, 2, 4 and 10, recipes at 0, 5 (known) and 6 (made at least once), books at 0, 4 and 10. The request board reads
the item levels: a synthesis request needs its item at 6 or more, a battle request an
entry at 2 or more (which list is not read yet). The engine's rule is `library::learn_item`; `barrel save` counts the
levels.

**Revealed recipe effects** (`mix_recipe`, written by `0x140367e50`, kept at
`0x1410993a0`): a u32 count (265, the recipe table's order), then per recipe a u32 length
(10) and ten flag bytes. Flag j belongs to the recipe's j-th effect, counted over its four
effect lines step by step, skipping steps with no effect (`0x1400e32d0`;
`recipe::Recipe::effect_list`). The recipe screen's name for it (`0x140367ba0`) is the
effect's name when the flag is set, "????" when not, and "None" past the last effect;
`0x140367d70` says whether all of a recipe's effects are revealed. One save at alchemy level
12 has flags on only four recipes (Craft's sixth effect; item 196's first and third, and
so on), so they track effects actually reached, not recipes known. The flags are set when an
item gets an effect (`0x14029d010` for one slot, `0x14029d080` for all four, through
`0x140367ad0`, which also raises the effect's knowledge to 10): its place in the item's
recipe's list, capped at 9 (`library::reveal_effect`). `BARREL_EXE=... barrel save FILE` prints
them.

**Bonuses** (`bonus`, written by `0x14036b940`): four blocks copied from memory as they are.
`skill`, 128 bytes at `0x141079780` (one byte a skill slot, 0 or 1); `mixvalue`, 544 u16 at
`0x1410799a0`; `value`, 128 i32 at `0x141079de0`; `collectTimeBonus`, 17 bytes at
`0x1410782e8`. A shorter saved block is zero-filled on load. `value[i]` is read through
`0x14002f2f0` and raised through `0x14002f930(i, amount)` (adds). Where the readers put
them: `value[1]` and `value[0x28]` as percentages on top of 100 (`0x1401d8600`, `0x140347f25`),
`value[2]` (and `value[2 + k]` at `0x14005fc80`) added to a base of 50 and capped at 99,
`value[3]` added as a float in the synthesis code (`0x14030f029`), `value[0x11]` taken off a
cost (`0x14032f2ce`), a run of them added to the characters' equipped stats (`0x14032df23`,
the +0x114 block of `Party`). One save has only `value[4]` = 5. These are the game's lasting
bonuses, granted by the rows of `bonus/Bonus.xml` through `0x14002f310` (docs/tables.md,
"Bonuses", has which action adds to which value). `save::Summary::bonus_values`.

**Battle items** (`explore_equip`, written by `0x140365640`): per alchemist,
`explore_item_<name>` (25 item units, 900 bytes; Escha's at `0x1410ae6b0`, Logy's 0x384
after), `explore_slot_<name>` (25 i32) and `explore_take_over_<name>` (25 item units, all
empty in the save read; what carries over is not read yet). The items sit on a row of 25
squares: an item's index in the list is the first square it covers, and it covers as many
squares as its own equip cost (item unit +0x18: `itemData.xml`'s `EquipCost` less its
`ITEM_SUB_EQUIP_COST` traits, 1 to 5, `synthesis::equip_cost`). `explore_slot` is that grid as saved: each
square holds the index of the item over it, -1 when free, 25 when locked (past the
capacity, 10 in the save read). Escha's there: a Bomb (cost 3) at 0, another at 5, a Healing
Salve (1) at 8, so `0 0 0 -1 -1 5 5 5 8 -1` and 25 after. `explore::grid` rebuilds it, and
`BARREL_ITEMS=itemData.xml barrel save` checks both alchemists' grids against the costs.

**World map** (`worldmap`, written by `0x14035dd00`, one block from `0x141098b70`):
`knowledgeRoute`, an i32 per area record (85, docs/tables.md "Field areas"), and
`knowledgePointer`, an i32 per place (96, "Places"), at +0x154: the map's knowledge levels,
0 nothing, 10 new, 15 all (the executable's "Knowledge: nothing / open wait / new / all"; -1
is checked for too, "open wait" presumably). `0x14035e3e0(place, level)` sets a place (a
notice, message 0x18, at 10 or more) and passes it on to the world-map place it belongs to
(`0x140171ad0`, `0x1401719d0`); `0x14035e3a0(area, level)` sets a route; the reset
(`0x14035e2c0`) zeroes the block and sets the town to 15. `chara` (two i32 at `0x141098e44`,
29 and 0 in the save read) is where the party stands, presumably. `pointerStatus`, 96 × 12
bytes at `0x141098e4c`: per place an i32 taken from the place record's +0x4c at reset (its
+0x48 decides), a byte flag and an i32; in the save read the field places hold 30 to 200 in
steps of 10 and flags of 1, which looks like a per-place amount, not read yet.
`save::Summary::route_knowledge`, `place_knowledge`, `world_position`.

**Research** (`work`, written by `0x140366440`): `work_data_status`, an i32 per row of
`work/WorkData.xml` (103: the research projects bought with money, `cost`, from a rank,
`exe_Rank`, after an event, `openEventTag`, and after another project, `WorkTag_exe`; each
grants a `Bonus.xml` row) at `0x141079800`, and `work_data_new`, one byte (`0x14107999c`).
The list (`0x140179170` and on) leaves out projects at -1 (not open) and 2 (done), shows a
project only when its `WorkTag_exe` project is at 2, and marks it new when its status is not
0: so -1 not open, 1 open and new, 0 open and seen, 2 done. The save read has four at 1 and
seven at 0, none done. `save::Summary::work_status`.

**Library marks** (`library`): bitsets, bit i (byte i / 8, bit i % 8 from the low end) for
entry i of a knowledge list: `new_item` (61 bytes, items 0 to 487), `new_eff` (88, the 698
effects), `new_potential` (50, the 400 traits), `new_property` (12, the 90 synthesis skills),
`new_book` (10, 73 books), `new_monster` (20, 153 monsters), and `new_help`, `new_memo`,
`notice_help`, `notice_memo` for the help and memo pages. A clear bit marks an entry as new
(known, not yet looked at in the library); the bits past each list's end are clear too. In
the save read every bit is set except four items (364, 369, 417, 438, all at knowledge 10:
recently made) and bit 487. Not checked against the library screen's code.

The calendar's other state: `quest/randseed` (a positive 31-bit value, the request board's
seed offset, `days::World::board_offset`), `quest/now` (the board's ten slots), `player`
(the requests taken) and `next`, ten 52-byte slots each (`save::BoardSlot`): the kind (0
synthesis, 1 monster, 2 gathering), the target as a number (an `itemData` row, the item enum's
order; or a monster counted from the first `MONSTER_` name), the state (0 empty, 1 posted, 2
taken), the day posted (220, the 11th, a redraw day) and a later day (329, the deadline
presumably), a count, a float (6 where the table's points are 5.12) and two prizes as item row
and rolled count. The slots hold rolled values, not request numbers: `quest::board_from_save`
finds each slot's request by kind, target and prize items (the first such; two Craft
requests differ only in a prize count and a condition). `quest/count`; `subject/job_rank` and `job_point` (rank 3, 1,565 points in
one save); `shop/shop_data`, the four shops' stock exactly as the game keeps it at
`0x141078e14` (a u32 and 50 wares of count, days and state each, 604 bytes a shop:
`shop::Stock`); `monthrep/data` (19,440 bytes: sixty 0x144-byte month reports, presumably,
twelve a year over five years); `party/m_squad`, the 11-slot list the daily friendship gain
reads (`0x141093530`). `save::Summary` reads the seed, the rank and the stock.

## Voices (`Sound/Voice/Series*.fp`)

`Data/x64/Sound/Voice` (and `Voice_EN`) hold 18 banks, `Series0000` to `Series17000`, each
a `.fp` of sound data and a `.fpl` index: u32 count (1,000), then that many
`{u32 offset, u32 size}` pairs into the `.fp`. Every bank fills 999 of its 1,000 slots, so
the voices are one list of 18,000 clips numbered from 0, bank n × 1000 holding clips n × 1000
to n × 1000 + 999. Some clips are named outright: events play them with `u31`
(`ECT_EVENT_VOICE_PLAY`, `{clip}`, 400 uses, nearly all in the battle skill events: 10246 in
`SKILL_COMMON_ITEM_AT_DREAM_BOOK_01`, 14235 ...) and `s102` (`ECT_CHARA_VOICE_PLAY`,
`{character, clip}`, 12 uses), and the tables have a `VoiceNo` column (`SubjectTalkData`:
11083, 11084 ...; `SysMess`, -1 for none). Only 16 story events use the two commands, and
the small script a dialogue line runs (`0x140184060`) plays no voice; how a story line picks
its clip was found by watching the game (below).
`barrel play-event` prints the voice commands with their bank and clip. By what names them, the banks
divide by use: 10 and 11 hold the system and field voices of every character (`SysMess`:
Escha and Logy 45 each in bank 10, 22 each in 11, and the others in 11; `SubjectTalkData`:
Marion's in 11), 14 and 15 the battle voices (the skill events' `u31`), and 17 a few more of
Nio's; nothing names a clip in banks 0 to 9, which leaves those 10,000 for the story lines.

**Story lines' voices** (an oracle run, 2026-11-12: a preload library, `tools/oracle/readlog.c`,
logged every read the game made from the voice banks while the opening played). In English
the first opening scene's lines are read from `Voice_EN/Series12000.fp`, one clip after another
as each line comes up: line k of `EVENT_MESSAGE_MM01_OP1_111` is clip 12900 + k (a message's
fifth field is its index k in the event, 0 up). Matching clip sizes to line lengths over the
opening's events (sizes follow lengths closely, correlation 0.9 and more) gives each event's
first clip, and they run on without gaps: 111 at 12900 (14 lines), 212 at 12914 (13), 121 at
12927 (19), 222 at 12946 (16), 131 at 12962 (44), 232 at 13006 (41), 300 at 13047 (32), 310
at 13079. The game itself read 12927 for 121's first line. Events whose lines are the same
scene for the other alchemist share the clips (021 with 121, 022 with 222, 040 with 300), and
some (011, 012, 031, 032) match nothing well, so they are unvoiced or voiced elsewhere. So a
story line's clip is its scene's first clip plus its index, the scenes laid end to end in a
fixed order that is not the files' and not the executable's event list; where that order
lives in the game is not found yet. Matching sizes to lengths finds it without the game for
any event with enough lines: `voice::story_start`, and `barrel voice-map INSTALL` runs it over
every English event (520 with 3 lines or more; 192 match at correlation 0.85 or more, the
opening's chain among them exactly as the game read it). The weaker matches need checking
before the engine relies on them. Chaining helps only a little: of the 192 strong ones, 99
start or end where another strong one does, and growing chains from them (the best-fitting
event at each chain's end, correlation 0.7 or more) places 13 more, 205 in all of the 521
events with three lines or more; the strong matches sit in banks 0 to 3 and 10 to 17. Many
of the rest are unvoiced, and a few placements look doubtful (`MM01_OP1_011` at 10804,
`_012` at 3153), so another read-log run over other scenes is the way to firm the table up. The viewer
uses the strong ones: in a window it plays each line's clip from the English banks as the
line comes up (`stage::Stage::story_voice`; `barrel-view` prints "story voices from clip
12900 (match 0.89)" for the first opening scene).

**Later games' models.** Ryza 3 adds vertex attribute type 7, four unsigned 16-bit values
(bone indices in models whose palettes pass 256 slots, and UVs). Escha's rule for bone
indices (index / 3 picks a slot of the submesh's palette) still holds for most submeshes
of the later games, every index a multiple of 3. The ones that break it (13 of 29 in Sophie
2's `PC01A_MODEL_default.g1m`; which parts of the body they are is not checked yet) carry
other numbers there: dense values from 0 up past their palette's size (80 values against a
palette of 5), even with zero weight counted out. These look like the cloth and physics
parts the containers' `.swg` and `.rigbin` files drive. The reader treats a submesh as
palette-skinned only when every index is a multiple of 3 and keeps the others rigid on
their palette's first joint for now.

## Morph targets (`*.g1h`)

A character's facial shapes, next to its model in the same container
(`PC20_MODEL_default.g1h` beside `PC20_MODEL_default.g1m`). `barrel_formats::g1h` reads
them; `barrel g1h FILE#N` lists one and `barrel g1h-scan DIR` checks them all.

```text
header:  "_H1G", version "2200", file size, u16 header size, u16 set count, u32 0,
         then a u32 offset per set
set:     "PH1G", version "0100", size, u16 header size (0x14), u16 layout, u32 n,
         then n - 1 u32 offsets from the set's start, one per target
target:  a whole g1m whose geometry has a single vertex buffer of 12-byte entries
```

Each set belongs to one **vertex layout** of the base model (the number submeshes name), and
each target holds an **offset** for every vertex of that layout: small values (under 2 units)
added to positions in the hundreds. Escha's model has three sets: layout 7 (288 vertices,
7 targets, every vertex moving), layout 3 (1,512 vertices, 10 targets) and layout 4 (1,446
vertices, 18 targets), each target moving a different part of the face. The facial
animations (`*_FACIAL` containers, `PC20_EYE_M_ANG_M_Brw_B_Def.g1a` and so on, by
expression: angry, default, fun, sad, serious) are the single-value `g1a` tracks above,
whose "bones" are these targets' weights. All 126 morph files in Escha & Logy DX read and
match their models: 239 sets, 1,858 targets.

**Which track drives which target.** A set's n counts its targets plus one, and the
facial animation for that part of the face has exactly n tracks: Escha's brow animations
(`*_M_Brw_*`) have 8 tracks for the 8 of layout 7, the eye animations 11 for layout 3 and the
mouth animations 19 for layout 4. Track (bone) 0 is the rest shape, which has no stored
target; bone k ≥ 1 weights target k − 1. The posed face is then base + Σ weight × offset
over the set's targets. `barrel g1a FILE#N` prints the weights over time: the default mouth
moves bones 1, 5 and 6 (up to 0.77) while she talks, the angry mouth holds bone 12 at 1.0 and
moves 6, 8 and 13.

## Node names (`*Oid.bin`)

Read by `barrel_formats::oid`. Each model container has one (`<name>Oid.bin`, the `.bin`
entries with no magic); all 298 in Escha read, 31,415 names in all. `barrel oid-scan DIR`
reads them.

The file is the model's node names in node order (`root`, `Bip001_Pelvis`, `Bip001_Spine`,
...), each as `{u8 length, ASCII bytes}`, ended by a single 0xff byte. They name the
skeleton's bones, which G1MS stores by number only.

## Fields

A field is a folder under `Data/x64/Field/` with two containers. `<AREA>.elixir.gz` holds
the terrain and the field's own data:

| Entry | What |
|---|---|
| `<AREA>.ktf` | terrain (below) |
| `<AREA>.khm` | heightmap (below) |
| `<AREA>.g1t`, `<AREA>_pat.g1t` | terrain textures; `_pat` is the light texture (cloud shadows outdoors, a 64-byte stub indoors; `docs/shading.md`) |
| `<AREA>.g1s` | the field's shaders |
| `<AREA>.opl2` | where objects and effects stand (below) |
| `<AREA>_col.g1co` | collision |

`<AREA>_Obj.elixir.gz` holds the object models, `obj-<name>_hi.g1m`, each followed by
four `dummy` entries.

### Collision (`*.g1co`)

Read by `barrel_formats::g1co` (`barrel g1co-scan DIR`): each field's `<AREA>_col.g1co`, and
breakable objects' own (`obj-<name>_coll.g1co` in the `_Obj` containers). Little-endian:

```text
"OC1G", u32 50 ("2"), u32 0x01010000, u32 size (the container's entry can run past it)
"HVB_" chunk: "HVB_", u32 50, u32 0, u32 chunk size,
  u32 node count, u32 48 (node size), u32 triangle count, u32 triangles' offset,
  u32 vertex count, u32 vertices' offset (offsets from the chunk's start)
nodes at +0x30: f32×4 max, f32×4 min, i32 kind, i32 link, u32 count, u32 first
triangles: u16×3 vertex numbers each (the block padded to 16)
vertices: f32×4 (x, y, z, 0), the chunk's last bytes
```

The nodes are a bounding-volume hierarchy ("HVB" is "BVH" backwards): a leaf (kind 0, link
65535) covers triangles `first..first + count`, an inner node (kind 2, 3 or 4, presumably the
axis it splits) covers its children's and links to its second child. Over Escha's 162 files
(31,354 triangles, 45,162 vertices) every index is within its vertices, every triangle is in
exactly one leaf and inside that leaf's bounds. The mesh is the walls (upright quads) and the
floors where the ground is not the heightmap (TOWN_10: the town's outline, the gate squares
and a stair), in field units, one vertex per corner (not shared). Characters walk on its
floors and stop at its walls (`game::walk`, `engine.md` "Walking").

### Heightmaps (`*.khm`)

Read by `barrel_formats::khm`.

```text
32-byte header: "_MHK", "0100", u32 0, u32 file size, u16 cells across, u16 cells down,
                f32 cell size, f32 lowest height, f32 highest height
(across + 1) × (down + 1) u32 vertices, row by row
```

A vertex's top 24 bits are its height, 0 to 2^24 - 1 spanning the lowest to the highest
height; its low byte is a flag (0, or 2 in 63 of AREA_01_01's vertices). AREA_01_01 is 64 ×
64 cells of 100 units, from x = 0 and z = 0, heights -15.7 to 193.4; most of it is the
value for height 0 (0x1338aa), the valley floor. Drawn under the field's objects it lines up
with them, the cliff ring standing around its edge. It is coarse, one vertex every 100
units, so it is most likely the height for walking and placing; the detailed ground is in
the `.ktf`.

### Terrain (`*.ktf`)

Read by `barrel_formats::ktf`; all 141 of Escha's read (`barrel ktf-scan DIR`). The ground
is the field's heightmap textured by blending ground layers, and the file bundles all of
it:

| Offset | What |
|---|---|
| 0x0c | u32 file size |
| 0x10 | u32 offset of the layer textures: a `g1t` of 512×512 BC3 tiles (sand, dirt, cracked earth, grass in AREA_01_01), 1 to 20 per field |
| 0x14 | u32 offset of the blend maps: a `g1t` of two 512×512 BC1 maps over the whole field, their RGB channels the layers' weights |
| 0x1c | u32 offset of the heightmap: a `khm`, the same as the field's own `.khm` |
| 0x84 | f32 cell size (100), f32 25, f32 1/256 |
| 0x90 | u16 cells across, u16 cells down (64 × 64) |

The layer tiles' alpha is 0 throughout, so alpha means something other than opacity there.

The settings block starts with layer records at 0x1c (file offset 0xb4), 32 bytes each and
one per layer texture: `{u16 layer, u16 0xffff, u16 0, u8 n, u8 n, f32 u scale, f32 v
scale, 16 bytes of 0}`. The scale is 1 / (100 × n): the layer repeats every n cells.
The blend maps do not pair with the layers as two maps for six: over the 141 terrains the
count of maps does not follow the count of layers (10 layers with 1 map, 4 layers with 5;
`barrel ktf-scan` prints the pairs). So which map and channel weights which layer is set per
patch, by the block between 0x98 and the layer textures. Its layout, as seen in `TOWN_04` (6
layers, 6 maps), is not decoded yet:

- **Header:** at 0x98, u32 0x310, u32 0, u32 7, then small u16 pairs.
- **Layer records:** at 0xb4, as above.
- **Records of about 48 bytes:** from about 0x16c, each with a small float (0.015, 0.016), a
  height-like float (487 to 534) and a 6.
- **A quadtree:** a root with bounds centred on (10000, ~508, 10000), half the field's 20,000
  units on a side, then nodes of half-size 5,000, 2,500 ... Each holds 6 and 0x0b, child
  indices (-1 where none), and lists like `2 3 4 5` (the layers a patch uses, presumably).

In `TOWN_01` (1 layer, 1 map, a 790-byte block) the parts are clear enough to name:

- **Layer records:** one 32-byte record at 0xb4.
- **Four 48-byte records:** `{u32 0, u32 1, f32 a, f32 a, 0, 0, f32 a, f32 a, 0 × 4}`, with `a`
  = 1/4000, and the field is 4,000 units across. So these are presumably texture-coordinate
  scales for the blend maps. `TOWN_04` has four as well.
- **A quadtree of 60-byte nodes:** each is a centre (3 floats), 1.0, a half-size (3 floats),
  a pad (32 bytes), then seven integers. The root is centred on (2000, 0, 2000), half-size
  2000, followed by its children's numbers 1 to 4. The four leaves are centred on (1000,
  1000), (3000, 1000), (1000, 3000) and (3000, 3000), half-size 1000. A leaf's integers are -1,
  a flags word `0x0401`, 0, then pairs like (1, 4) and (2, 3), presumably ranges into what
  follows.
- **Patch records:** eight of 16 bytes at 0x318 (`{u32 n | 0x80000000 flag, u32 0 or 1,
  0x80ffffff, 0x80ffffff}`), then a u16 list (`1 2 3 5 3 6 3 7 4`).

`AREA_08_02` (6 layers, 1 map, a 5,000 × 15,000-unit field) does not fit that count. After its
six layer records come six 48-byte records, not four, laid out differently: `{f32 0.018, f32 3000
/ 1800 / 600, 0, u32 1, f32 1/5000, f32 1/15000, 0, 0, the same two, 0, 0}`, so they hold the
blend coordinate scales for each axis of a non-square field. Its root node (at 0x294) is centred
on (2500, 1800, 7500) with that half-size. So the record count follows something other than the
layers, and the leaves' integers are still unexplained. Reading them needs the executable's
section readers below.

Over nine fields the block grows with the layers: 750 to 794 bytes for one layer, 8 KB for
`AREA_08_02` (6 layers, 1 map), 97 KB for `AREA_01_01` (12 layers, 2 maps) and 694 KB for
`AREA_03_01` (13, 3). `AREA_01_01`, the viewer's usual test field, is therefore one whose
blending the viewer gets wrong.

The executable's reader checks the magic (`KFTK` read as `0x4b54464b`) in `0x14066be00` and
`0x14066efe0`. Both go to `0x14066bed0`, which calls a section reader for each part in turn:
`0x14066f900`, `0x14066fc90`, `0x14066fc50`, `0x14066f940`, `0x14066f690`, `0x14066f6d0` (the
largest), `0x14066fb10`, `0x14066fad0`, `0x14066fba0`, `0x14066f250`, `0x14066f2b0` and four
more. Most are thin wrappers over deeper calls, not followed yet.

### Effects (`*.g1e`)

The game's 536 effects are in `Data/x64/Effect/000_All.elixir.gz`, one `.g1e` each named
`<number>_<NAME>.g1e` (the numbers of `Saves/effect/eff_pack_list.xml`, 0 to 715), with one
shared texture pack, `eff_texture.g1t`. All 536 begin the same way:

| Offset | What |
|---|---|
| 0 | `XF1G` |
| 4 | the version, `5200` in all |
| 8 | u32 file size (matches in all) |
| 0xc | u32 9 in all |
| 0x10 | u32 1 in all: what the loader sizes its object by (0x188 + 0xe8 × it), so the number of effects in the file, presumably |

Further in, the devil aura (`701_FE_DEVIL_AURA_01`, 5,008 bytes) has 4 × 4 matrices (identity,
then one with -1 on the axis) and, at 0x110, a list of increasing offsets (0x20, 0x24c, 0x478,
0x6e4, 0x950, 0xddc), presumably its parts. Colours and sizes follow (runs of 1.0, 0.1, 40.0).
Comparing the six same-size aura variants (701–706, 5,008 bytes each) shows where some
settings sit. An RGB colour is at 0x15c–0x164 and again at 0x18c–0x194: red (1, 0.1, 0.1) in 01
and 02, blue (0.2, 0.2, 1) in 03 and 04, yellow (1, 0.8, 0.2) in 05 and 06. A scale is at 0x2f0,
0x318, 0x520 and 0x548: 1 in the odd variants, 2 in the even ones, so each colour comes in a
small and a large aura. Only two other words differ, at 0x134 and 0xd0c. The parts' layout is
not decoded yet. The executable reads the 0x14-byte header
(`0x1406162f0`), checks it (`0x140618f10`: `XF1G`, version at least `5200`), allocates that
object and constructs it with `0x140615be0` (the effect object, vtable `0x14097f1c8`, its
parts at +0xa0, 0xe8 bytes each, built by `0x140611d60`, vtable `0x14097f140`). The data is
read by the object's own methods: `0x140617180` sets its flags (+0x9c) from the header and runs
`0x140612780` on each part. The classes are the framework's (`elixir::EXEffectObject`,
`EXEffectManager`) under the game's `EffectObject` and `EffectManager`.

### Placement lists (`*.opl2`)

Read by `barrel_formats::opl2`. All 141 of Escha's read: 20,283 objects and 150 effects.
`barrel opl2-scan DIR` reads them all.

Two lists of the same shape follow each other, the objects and then the effects:

```text
u32 type count
per type:     u32 name length, name with its 0, u32, 16 bytes (0xff in most), u32 instance count
per instance: position (3 floats + pad), rotation in radians (3 floats + pad),
              scale (3 floats + pad), u32 tag length, tags, 32 bytes
```

An object type's name is its model's name without `_hi.g1m`. Instance tags are 4-character
codes run together: none (10,115 instances), `xDS_` (3,105), `xS3_` (1,129), `xV1_` and
combinations; effects carry `eff`. The game knows 21 tags (a table at `0x14095a740`: `xNS_`, `xDS_`, `xEF_`, `xE2_`, `xEV_`,
`xCT_`, `xAM_`, `xN1_`, `xN2_`, `xV1_` to `xV5_`, `xBK_`, `xGL_`, `xS1_` to `xS3_`). As a placement
is made (`0x1404f0480` and on; the object holds position `+0x230`, rotation `+0x240`, scale
`+0x250`), its name and tags are searched for each (`strstr`), and a flag word at `+0x5a0` holds
them as bits in that order from 0 (`xDS_` 1, `xEV_` 4, `xCT_` 5, `xAM_` 6, `xN1_` 7, `xN2_` 8,
`xV1_` to `xV5_` 9 to 13; the same function writes the prefixes back from the bits). What each
does:

| tag | effect |
|---|---|
| `xDS_` | casts a shadow (`+0x561` = 1; `renderer.md`, shadow casters) |
| `xN1_`, `xN2_` | the wind's node shaker keyword (`<n_shaker>`, `shading.md`) |
| `xV1_` to `xV5_` | the wind's vertex shaker keyword (`<v_shaker>`, `shading.md`) |
| `xS1_`, `xS2_`, `xS3_` | raise the object's draw priority by 0, 1, 2 (the word at `+0x568`; `renderer.md`, "Draw priority") |
| `xCT_` | runs `0x1404f0ac0`: a box from the rotation and the model's bounds (`+0x468`), placed at the position, into `+0x600` (not read further) |
| `xAM_` | sets `+0x654..+0x65c` to (-1, -1, 1) and `+0x661` = 1 (not read further) |
| `xEF_` | effect objects (their files, `Obj/xEF_*/....elixir.gz`) |
| `xNS_`, `xE2_`, `xEV_`, `xBK_`, `xGL_` | not read |

## Event bytecode (`*.ebd`)

Read by `barrel_formats::ebd`. In Escha all 1,639 files read without error: 49,393 actions,
1.4 million words, 238 distinct commands. The files are under `Event/`:

| Directory | What |
|---|---|
| `event/<chapter>/` | story events, each beside its messages (`EVENT_MESSAGE_*.ebm`) |
| `skill/` | battle skills, per monster and shared |
| `parts/` | small shared pieces |

A file is a list of actions, the event editor's blocks. The editor's default name for
them, 新規アクション ("new action") with the description 説明 ("description"), is kept
in most files.

```text
u32 action count
per action:  u32 flags (0 in every file so far), string name, string description,
             u32 word count, then that many u32 words (the body), the last being 0x7f000000
tail:        u32 1, u32 1, 0x7f000000      (one more, empty body)
             u32 resource count, then that many strings
```

A string is `u32 length` (the terminating 0 counted) followed by its Shift-JIS bytes, with
no padding. Words after a string are therefore not 4-byte aligned in the file.

The resource strings name what the event loads: `ULTSET_*` settings, `ULTCAMERA_*`
cameras and `ING_*` items, each with an editor timestamp. The list often starts with an
empty string. In some files the last string is cut off by the end of the file, so the
reader stops there.

### Body words

| Word | Meaning |
|---|---|
| top byte 0x70..=0x7e | a command. The top byte is the group, written `p`..`~` (so 0x71 is `q`); bits 16..24 are flags (0, 0x10, 0x30 and 0x40 seen); the low 16 bits are the command number. |
| 0x7f000000 | end of the body |
| anything else | an operand |

Each command is followed by a fixed number of operand words; `docs/events.md` lists
the commands, their names and their operand counts. For example `q4`
(`ECT_FIELDMAP_CHARA_ADDITION_EX`) takes 14 words:

```text
q4  7 0  0 0  1000 0  800 0  -50 0  31 0  2 0
```

Here the operands read as `(value, type)` pairs, with type 0 an integer literal, but
other commands take odd counts (`ECT_CHARA_POS_SET` takes 7), so pairs are not the rule.

## Event messages (`EVENT_MESSAGE_*.ebm`)

Read by `barrel_formats::ebm`. All 4,660 of Escha's message files read: 57,084 lines across
Japanese (`Event/`) and the three translations (`Event_EN`, `Event_SC`, `Event_TC`), all
UTF-8. `barrel ebm FILE` prints a file; `barrel ebm-scan DIR` reads them all.

```text
u32 message count
per message: u32 kind (1 for 89% of lines; 2, 4 and 0 also occur)
             i32 speaker, a character number (-1 for system text)
             i32 ×6: 0; the speaker again; a number per line (0..~250); a small
                   code (0, 3 or 4); the line's index in the file; 0
             u32 length, then the UTF-8 text with its terminating 0
```

In `MM08_AS4_130` the third field runs 246, 91, 1, 1, 237, 248, ... (Escha's lines 246, 248,
235, 134), which looks like a voice clip or expression number per line, and the fourth is 0,
3 or 4; which they are is still open. They matter for faces: events hardly set expressions
with commands (`ECT_CHARA_FACIAL_MOTION_SET`, `s88`, occurs 17 times in all 1,639 events, and
the brow, eye and mouth variants never), so per-line expressions must come from these
fields or from the body motions. `barrel ebm` prints the fields.

Over all 14,271 Japanese lines: field 0 is always 0, field 1 equals the speaker in 13,928;
field 5 is 0 or 1. Field 3 is 0 (7,397), 4 (3,589), -1 (1,618), 2, 3, 10, 8 or 9. Field 2
is -1 for system text and otherwise a number up to 437 from one shared set: Escha's and
Logy's lines use about 80 values each (0, 1, 196, 198 and 246 the most), the others 40 to 50,
many the same across characters. It is not a `CharaBase` row (196, 198 and 246 there are
dummy camera and position markers) and not a voice clip on its own (see "Voices" below).

**What the game does with them** (English Escha executable). When a line is shown,
`0x1401837a0` skips kinds 3 and 4 and speaker -1, then `0x140184060` writes a small event
script for the speaker and runs it:

- **Field 2 is a gesture**: if it is at most 437, `0x140184280` turns it into a chain of
  body motions from a 438-entry table at `0x1407f2f60` (0x88 bytes each: two forms of 17
  i32, the second used by a flag and empty so far). A form is a first motion number, up to
  15 steps and an end type. The game emits `ECT_CHARA_MULTI_MOTION_LOOP_CLEAR` (`s83`) and
  `..._CHANGE_TYPE` (`s96`, the end type), then for each step: a value under 10,000 makes
  it the current motion, and 10,000 + n emits `..._LOOP_ADD` (`s82`: speaker, current
  motion, n, 10), playing it n more times. Gesture 0 is motion 31 once; 198 is motion 61
  for five loops, then 31; 1, 196 and 246 play their motion (56, 61, 68) twice. A motion number
  is the `MOTION_*` enum (553 tags at `0x140cee170`: `MOTION_WAIT` = 0, ...,
  `MOTION_F_WAIT` = 31, `MOTION_TALK` = 35, `MOTION_EV_ACT_1` = 56, ...); the tag without
  `MOTION_` is the `CharaBase` column, whose value names the animation for that character
  (`<FileHead>_<value>.g1a`). For Escha: 31 is `F_WAIT` = `ACT_DEF`, her idle; 56 `EV_ACT_1`
  = `STAND_NORMAL`; 61 `EV_ACT_6` = `STAND_THINK`; 62 `EV_ACT_7` = `STAND_YES`; 68 `EV_ACT_13`
  = `STAND_NORMAL_2`. So gesture 0 settles to idle, and 198 thinks for five loops before
  going back to idle. Not every enum tag has a column; the columns cover the ones a
  character uses.

  Where the animations are: the `ACT_*` ones (idle `ACT_DEF`, `ACT_DEF_2`, ...) in the
  character's `<Motion1>` container (`PC20_BASE_MOTION1`, 56 animations), the event ones in
  a container of their own beside it (`PC20/PC20_STAND_THINK.elixir.gz`). Each body motion
  comes with its own facial animations: `PC20_STAND_THINK.g1a` plus
  `PC20_STAND_THINK_M_Brw_B_Def.g1a`, `..._M_Eye_...` and `..._M_Mth_...`, so a gesture
  moves the face too unless an expression overrides it.
- **Field 3 is the expression**: if it is at most 10, it adds `ECT_CHARA_FACIAL_MOTION_SET`
  (`s88`): speaker, then a brow, eye and mouth facial motion from an 11-entry table at
  `0x1407f2e50` (-1 keeps the current one):

  | Field 3 | Brow | Eye | Mouth |
  |---|---|---|---|
  | 0 | keep | keep | 0 |
  | 1 | keep | keep | 8 |
  | 2 | keep | keep | 2 |
  | 3 | keep | keep | 4 |
  | 4 | keep | keep | 6 |
  | 5, 6, 7 | keep | keep | keep |
  | 8 | 4 | 4 | 8 |
  | 9 | 5 | 5 | 10 |
  | 10 | 6 | 6 | 12 |

  Each row has a second form whose mouth number is one higher (1, 9, 3, ...), chosen by a
  flag the caller passes; and -1 (1,618 lines) means no expression change.

This is why events hardly use `s88` themselves: every line carries its expression.

**From motion numbers to animations.** The executable keeps three string tables per
character naming its `_FACIAL` animations by number (Escha's: brows at `0x140cfe5e8`, eyes
at `0x140cff3e8`, mouths at `0x140cfcb70`). Brows and eyes, 0 to 6: DEF, FUN, ANG, SAD, SER,
FUN2, SAD (`PC20_EYE_M_ANG_M_Brw_B_Def`, ...). Mouths, 0 to 13: each of those moods twice,
first `PC20_MTH_M_<mood>_M_Mth_B_Def`, a 4-second loop that opens and closes the mouth (the
talking form, even numbers), then `PC20_MTH_DEF_<mood>_...`, a still pose (odd numbers).
So expression 10 is sad brows, sad eyes and a sad talking mouth. Other characters' tables
differ: Logy has no FUN2 eyes and a SAD2 instead, Wilbell (`PC27`) only four brows. They
are three lists of records, `{u64 CharaBase row, then 7 brow, 7 eye or 14 mouth name
pointers}`, a null pointer where the character lacks that motion (the part then keeps its
current one); the lists start after the `FACIAL_MOUTH_TALK_*` enum. Escha is row 0, her
costumes PC20B and PC20C rows 453 and 454 (0x1c5, 0x1c6), matching `CharaBase.xml`.
`barrel_game::face::Tables::read` finds them by that shape: 55 characters in Escha & Logy
DX, each with all three lists. `barrel faces EXE` lists them.

**Later games.** The message files of Sophie 2 and the Ryza games use the same records
with more values, some values after the text too, the count sometimes stored negative, and
then a table: a u32 count and that many records of k i32 (one record of
`1, -1, -1, -1, -1, -1, 0, 0, 0` in most of Sophie 2's; Ryza 3's `cm01_010` has 15):

| Layout | Count | Values before the text | After | Table records | Seen in |
|---|---|---|---|---|---|
| Escha | positive | 6 | 0 | none | Escha & Logy DX, Ryza DX |
| B | negative | 6 | 1 | 6 values | Ryza 2 DX |
| C | negative | 8 | 2 (-1, 300) | 9 values | Sophie 2 |
| D | positive | 13 | 2 (-1, 300) | 13 values | Ryza 3 DX |

The reader tries each and keeps the one that reads exactly the count's messages and ends
exactly at the table's end. In the later layouts the line's index sits among the values
(Sophie 2's seventh, 0, 1, 2, ...; Ryza 2's fifth and Ryza 3's twelfth, from 40000 and
60000) and the speaker numbers are the later games' own (Ryza 3's Clifford is 286). What the
table records are is still open. All message files read: Sophie 2's 3,607, Ryza DX's 8,415,
Ryza 2's 11,766 of 11,767 and Ryza 3's 9,629 of 9,630 (the two left are Japanese-only
`zz01_010` and `aa01_010`, test events by their names). Ryza 2's Japanese `zz01_010` (2,606
bytes, count -23) fits none of the four layouts: read as kind, speaker, values, text, its first
message is kind 2, speaker 0 with seven values before the text (0, 0, 3, -1, 0, -1, 2), the
second has one value after its text (-1) and six before the next; a work-in-progress mix, by
the look of it. Its translated copies are a two-message stub ("Stylish Weapon Skins - Ryza
available.") that reads as layout Escha.

Markup in the text: `<CR>` breaks the line (35,202 uses), `<IMnn>` puts an icon inline,
`<CLGR>` and `<CLNR>` change and reset the colour.

**Mangled lines.** Some lines in each translation were left in Japanese, and the tool that
made the translated files mangled them: it read their Shift-JIS bytes two at a time as
UTF-16 and wrote the result as UTF-8. エ (`83 47`) became U+4783, and markup was paired up
too (`<CR>` became U+433C U+3E52). Pairs that landed in the UTF-16 surrogate range could not
be written and became U+FFFD, so those characters (several kana, ま among them) are lost.

`Message::repaired_text` turns such lines back into Japanese and marks the lost characters
with 〓. The stored text is kept as it is. A line only counts as mangled when it has no
kana, at least half its characters are CJK, Hangul or private-use code points, every code
point's low byte could start a Shift-JIS character (or both bytes are printable ASCII), and
the result reads as Japanese. That repairs 567 English, 579 Simplified Chinese and 578
Traditional Chinese lines (the tutorial's furniture descriptions among them), and touches
none of the Japanese originals.

## Tables (`Saves*/**/*.xml`)

Read by `barrel_formats::table`. All 312 of Escha's XML files read without error. What the
tables hold is in `tables.md`.

The files are XML only loosely; the game's own reader accepts all of them, so this one
is lenient in the same ways:
- **Encoding.** The declaration is not to be trusted. The translated tables declare
  Shift-JIS but are UTF-8 with a byte-order mark, and some Shift-JIS files have UTF-8
  comments. The rule that works for every file: a BOM or valid UTF-8 means UTF-8;
  anything else is Shift-JIS (code page 932).
- **Attributes** may follow each other with no space (`pos_y="21.0"rot_x="0"`).
- **Entities.** `&nbsp;` is used, which XML does not define. Unknown entities are kept as
  written.
- **Unclosed elements.** `field/_def.xml` never closes `<wind_root>` before `</root>`. A
  closing tag that matches an element further out closes the ones inside it.

There is no text content; everything is in attributes. Comments are common (1,426 in
Escha), some of them holding commented-out rows.

## Voice packs (`Sound/Voice*/SeriesNNNN.fp`, `.fpl`)

`Data/x64/Sound/Voice` (and `Voice_EN`) holds 18 series, `Series0000` to `Series17000`, each a
`.fp` of up to 1,000 clips and a `.fpl` listing them: `u32 count` (1000), then per clip
`{u32 size, u32 offset}` into the `.fp` (size 0: no clip). Each clip is a KOVS file:
`"KOVS"`, `u32` Ogg size, 24 zero bytes, then an Ogg Vorbis stream whose first 0x100 bytes
are XORed with their index (`OfeP` for `OggS`). A clip's length is its last Ogg page's
granule position over the identification header's sample rate. `barrel_formats::voice`
reads them; `barrel voice-scan DIR` finds 18,000 clips, 21.7 hours, all decoding, in
Escha's Japanese voices. Clips are numbered within a series (`SeriesN` clip k is voice N + k,
by the look of it); which voice a message line plays is not in the line (its fields are
speaker, gesture, expression and line number) and is still open, as are
`ECT_EVENT_VOICE_PLAY` (`u31`, 400 uses, a voice number such as 14235) and
`ECT_CHARA_VOICE_PLAY`.

## Interface layouts (compiled into the executable)

The `WinXls` sheets ship as textures only (`Data/x64/WinXls_<lang>/a15_*.g1t`); the layouts
that place them are tables in the executable (English addresses; `crates/formats/src/layout.rs`):

- **Layouts** `0x140dd9080`, ids from 0x165: 16 bytes `{u64 nodes, i32 x, i32 y}`
  (`0x140373f40`, `0x140373ca0`). The nodes are 0x28 bytes, `{i32 part, i32 cut, i32 x, i32
  y, i32 kind, ...}`, ending with part -1, each placed from the layout's (x, y)
  (`0x140374260`; kind picks the widget, `0x14037ace0`, `0x14037a400`, ...).
- **Sprites**: a part below 0x164 is a sprite sheet, `u64 cuts = [0x140db9f70 + 8 part]`;
  each cut is 20 bytes `{i32 texture, u16 x0, y0, x1, y1, 8 bytes}`, the rectangle in the
  texture's pixels.
- **Textures**: ids from 583 name `WinXls` files, `[0x140db5c50 + 8 (id - 583)]` (303 names;
  `a15_telop_mm01_op1_111_01.g1t` is 736). Lower ids, and parts from 0x164 on, are other
  kinds (not read yet).
- Coordinates are on a 1920 × 1080 screen.

The narration cards (`u55`, `docs/events.md`) are layouts 960 to 1010: the opening's are
1536 × 72 strips of the telop sheets, one per line, at x 180 around y 462.

## Costumes (`Saves/costume/*.xml`)

An outfit is a body with a head and a face accessory. `CostumeBody.xml` (35 rows): its name,
whose it is (`costume_chara`), its model (`chara_base`, a `CharaBase` tag: Escha's Colseit Uni
is `PC20`, her Apple Pirate `PC20B` ...), a priority and the nodes the accessories hang from
(`head_attach`, `face_attach`: `H_4_Acc_Head_Attach1`, `H_4_Acc_Face_Attach1`).
`CostumeHead.xml` (38) and `CostumeFace.xml` (6): accessories, each a model name stem
(`filename_base`, `HEAD00`; none for the first, "nothing"). `AccessoryAdjust.xml` (1,292 rows):
for a body, part and accessory, the scale, rotation (degrees) and offset it is placed with.
`PartsInvisible.xml` (90): the body's own parts hidden while an accessory is worn (Escha's
default body hides `P_M_Acc_Hat1`, her hat, under any head accessory). `AccessoryChange.xml`
(28): accessories swapped for another (`src` → `dst`) on a body: each body's own hat for none.
Every body has one `PartsInvisible` row with `exclude="true"`, naming that same hat: its nodes
are hidden under every head accessory but that one, so wearing a body's own hat shows the hat
built into the body with nothing attached; rows without it hide their nodes only under the
accessory they name (the maid's `P_M_Acc_Hat2` under each of the hats). Read from the tables'
patterns (`costume::resolve`), not yet from the code. `CostumeChara.xml`: each
character's default body, head and face. The settings save (`SYSDATA`) keeps the choices:
`costumeData`, an i32 count (5: Escha, Logy, Wilbell, Linca, Awin by the bodies worn) then each
character's body, head and face rows, every value written as a length (4) and the i32
(`costume::choices`; the oracle save: Escha in the Colseit Uni with the Fluffy Hat, the others
in theirs); `cosBodyFlag` (34 bytes) and `cosHeadFlag` (38) mark the bodies and heads unlocked,
one byte each. `barrel_formats::costume`, `barrel costumes ROOT [SYSDATA]`. Not yet used by the renderer.

## The message window's names

An `ebm` line's second field (Escha) is the name the window shows: an index into the English
executable's speaker-name table at `0x1408117a0` (40-byte records `{u64 name, i32 3, i32 -1,
u32 0, f32 × 4 255}`; `layout::speaker_names`). Entries 0 to 17 are the cast as in `Chara.xml`;
the rest name those not yet introduced or never named: 59 "???" (Clone's first lines in
`EVENT_MM01_OP1_111`, as the game shows them), 60 "!!!", 62 "Everyone", 19 "Gloomy Man" and
the like. The field is usually the speaker's own number (Clone: 16 in 365 lines, 59 in 5).

## PSSG (Ayesha DX)

Ayesha DX keeps the PS3 game's PhyreEngine data: 1,768 files under `Res/x64` (`*.PSSG` and
`*.PSSG.gz`; the `.gz` ones are plain PSSG but for 731, mostly motions, that are gzip members).
`barrel_formats::pssg` reads them all (`barrel pssg-scan DIR`); big-endian throughout:

| Part | Layout |
|---|---|
| Header | `PSSG`, u32 the size of the rest, u32 the attribute kinds' count, u32 the node kinds' count |
| Schema | per node kind: u32 id, a name (u32 length, bytes), u32 attribute count, then per attribute u32 id and a name |
| Node | u32 kind, u32 size of the rest, u32 size of its attributes, the attributes (u32 kind, u32 length, bytes), then child nodes or raw data |

Whether a node's tail is children or data is not marked: it is children when it reads as nodes
to the node's end exactly. The root is `PSSGDATABASE`. Over all the files the commonest kinds
are the render streams (`RENDERSTREAM`, `RENDERINSTANCESTREAM`, `RISTREAM`), animation
channels and keys, data blocks (`DATABLOCK`, `DATABLOCKDATA`, `DATABLOCKSTREAM`), shader
inputs, index sources, skinning (`SKINJOINT`, `INVERSEBINDMATRIX`, `JOINTNODE`) and the scene
nodes (`NODE`, `RENDERNODE`, `TRANSFORM`, `BOUNDINGBOX`).

Attribute ids belong to the file, not to a node kind: a node carries kinds listed under others
(`id` among them), so a name is looked up across the whole schema. String attributes are a u32
length and the bytes.

**Meshes** (`Pssg::meshes`, `barrel pssg-meshes`, `barrel pssg-obj FILE OUT.obj`): each
`RENDERDATASOURCE` is one. Its `RENDERINDEXSOURCE` gives `primitive` (`triangles`, else a
strip), `format` (`ushort`, `uint` or `uchar`) and `count`, its `INDEXSOURCEDATA` the indices.
Its `RENDERSTREAM`s name a `DATABLOCK` (`dataBlock="#<id>"`) and a `subStream`; the block's
`DATABLOCKSTREAM`s give each stream's `renderType` (`Vertex`, `Normal`, `Color`, `ST`, and in a
skinned mesh `SkinnableVertex`, `SkinnableNormal`, `SkinIndices`, `SkinWeights`), `dataType`
(`float1` to `float4`, `uchar4`), `offset` and `stride`, over `elementCount` elements of its
`DATABLOCKDATA`. The vertex data is big-endian like the rest (a unit normal and a white vertex
colour read so). Over Ayesha's 1,768 files that is 45,941 meshes, 13,953,259 triangles, every
index within its mesh's vertices. Characters stand with z up (Ayesha's `PC00J_MODEL`: 164.5
units tall, arms out).

**Placement** (`Pssg::placements`): the scene nodes (`ROOTNODE`, `NODE`, `RENDERNODE`,
`JOINTNODE`, `SKINNODE`) each hold a `TRANSFORM` of 16 big-endian floats, taken as row vectors
(the translation in the last row) and multiplied from the root down; an instance under a node
(`RENDERSTREAMINSTANCE`, or `MODIFIERNETWORKINSTANCE` for the morphed and skinned ones) names
its mesh by `indices="#<data source>"`. `barrel pssg-obj` places each mesh so. A character's
meshes are already in place (their transforms are the identity: `PC00J_MODEL` comes out the
same); a field map's are spread over it (`L_POINT_DLC_02_D`: 201 render nodes, a ground grid
with its props gathered at the middle, the sky sphere (`LAYER_020SKY`) half a million units
out). The order is confirmed by the skins below.

**Textures** (`Pssg::textures`, `barrel pssg-textures FILE OUTDIR`): a `TEXTURE` node has
`width`, `height`, `texelFormat` and `numberMipMapLevels`, and a `TEXTUREIMAGEBLOCK` whose
`TEXTUREIMAGEBLOCKDATA` holds the levels largest first, in the usual DDS block layout (not
byte-swapped): a 128 × 128 `dxt1` with 7 more levels is 10,936 bytes, down to 1 × 1. The `id` is
the source file's name (`PC00J_Body.dds`). Over Ayesha's files: 4,647 `dxt1`, 1,462 `dxt5`, 533
`dxt3` (BC2: `bc::decode_bc2`) and 6 `ui8x4` (uncompressed, its byte order not read yet); every
compressed one decodes to its size, and Ayesha's body atlas and eyes come out as drawn.

**Skeletons** (`Pssg::joints`, `Pssg::skins`): the joints are `JOINTNODE`s in the scene tree
(`B_0_Base`, `B_0_Pelvis`, ...), placed by their `TRANSFORM`s like any node. A `SKINNODE`
(`skeleton="#<id>"`) lists its palette as `SKINJOINT joint="#<joint>"` children, in order, and
the `SKELETON` it names (in a `LIBRARY`) holds as many `INVERSEBINDMATRIX`es (16 big-endian
floats). The skinned mesh is drawn through the `SKINNODE`'s `MODIFIERNETWORKINSTANCE`
(`network="PSSGInternalDatabase#skinTransformLocal"`), its streams `SkinIndices` (palette
slots, `uchar4`) and `SkinWeights` (`float4`). Over Ayesha's 2,444 skins every joint is found
and every palette matches its skeleton's count; and inverse bind × the joint's world matrix is
the identity (median off by 0.00003) for 25,286 of 27,386 joints, which settles the matrix
order (row vectors, a parent's matrix applied after its child's). The rest are in 74 files,
effects (`E3D_*`) and a few monsters (`MOB03A_MODEL`), whose stored pose is not the bind pose.

**Motions** (`Pssg::motions`): an `ANIMATION` (`PC00_ACT_BYEBYE`: 464 channels, 565 constant
ones) holds `CHANNELREF channel="#<id>" targetName="<joint>"` children and `CONSTANTCHANNEL value
targetName keyType` ones (a fixed value, 16 bytes). Each `ANIMATIONCHANNEL` names a `timeBlock`
and a `valueBlock`, each an `ANIMATIONCHANNELDATABLOCK` (`keyCount`, `keyType`) whose `KEYS`
are big-endian floats: `Time` one a key, `Translation` and `Scale` three, `Rotation` four (a
quaternion: every one over Ayesha's files is of unit length), `MorphTargetWeight1` and
`ShaderInstanceFloatParameter` one (their targets `#<weights>#<n>` and
`#<shader instance>#<parameter>`, the face's morphs and the eyes' UV scroll). The
`constantChannelEndTime` spans the constant channels only (0 in a motion that has none), so a
motion lasts to its last key. Some keys come before time 0 (757 channels, the facial motions
start at -1/15 s), leading the curve in. Over the 1,960 motions (1,089,658 channels) every
channel's keys are in order with a value for each.

Materials are not read yet, nor how the game blends the keys (linear, presumably, with the
rotations slerped).
