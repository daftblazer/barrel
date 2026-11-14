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
| 0x10001 | lighting materials: a u32 (0x100), then `count` 64-byte entries: diffuse (four floats), ambient (three), a word of nibble flags (`0x111`, `0x133`, `0x1133`), three floats, a float and three floats (field models: 1, 1, 1, 20 and 0, 0, 0; characters 0, 0, 0, then 10 to 25.5 and a grey: by `MapSTD`'s constants specular, power and emissive, not read) and a mask (`0xffff`, `0xffffff`). The diffuse and ambient are `MapSTD`'s `cDiff` and `cAmb`. Field models have one entry, all ones in 4,501 of 4,590 (every field's `_Obj` models); the rest are greys: `TOWN_10`'s `obj-a15_06house_body` diffuse 0.7 and ambient 0.3, ground pieces 0.588, a few 0.97 to 0.99, crystals ambient 0, some diffuse alpha 0.5 to 0.62. Characters have several (12 in `PC20_MODEL`), matching neither the materials nor the mesh groups: which one a submesh takes is not known. `barrel g1m-materials` prints them |
| 0x10002 | materials: `{u32, u32 texture count, u32, u32}`, then per texture `{u16 index into the model's g1t, u16 layer, u16 type, u16 subtype, u16 tile u, u16 tile v}` |
| 0x10003 | shader parameter sets (one per submesh's "shader parameters" number): per set `{u32 count}`, then per parameter `{u32 size, u32 name length, u32, u16 value count, u16 kind}`, the name, its float values (`Geometry::shader_params`; `shading.md`, "The materials' own parameters") |
| 0x10004 | vertex buffers: `{u32, u32 stride, u32 vertex count, u32}`, then the vertices |
| 0x10005 | vertex layouts: `{u32 buffer count, u32 buffer numbers[], u32 attribute count}`, then per attribute `{u16 buffer (position in this layout's list), u16 offset, u8 type, u8, u8 semantic, u8 layer}` |
| 0x10006 | bone palettes: `{u32 count}`, then per entry `{u32, u32 flags (0x80000000), u32 joint}`; a submesh with a palette but no bone indices or weights (Escha's `_P_M_Acc_Hat1`, on joint 18 under the head) is rigid on its palette's first joint |
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

Motions (`.g1a`) name bones by id, and the base skeleton's index map turns an id into a joint
(`Skeleton::joint`). Escha's maps send every animated id to itself, so using the id as the
joint worked there; Ryza's do not: `pc00a` (179 joints) maps ids 0-24 to themselves, leaves
25-49 unused (0xffff) and sends 50 on to joints 25 on, and her stand motion animates ids 0-24,
50-81, 100-125, 130-144. Read as joints, those tracks turn the wrong ones (her head faced
backwards).

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
item jingle (6). The boot's logo splashes are stills (the still table at `0x1408f3d00`, 32-byte records): 4
`Ipu/logo_gust.g1t` (1920 × 1080, the Gust brush logo on light grey) and 5 `Ipu/logo_kt.g1t`
(the Koei Tecmo Games logo, 1920 × 1080 in the top-left of a 2048 × 2048 texture), each with
`_EN` / `_SC` / `_TC` copies. `Ipu/logo_havok.g1t` and `logo_dolby.g1t` are on the disc but no
still names them. Track 8 is the Gust jingle (`_gust_ondo.kvs`, volume 0.5), but the PC install
has no such file. Layout nodes of kind 36 are flipbooks: word 5 cuts from the node's, in turn (the battle
command row's diamond, 0xe8 cuts 2 to 5, and its arrows, 6 to 9 and 10 to 13; the rate is not
read yet). The battle's commands (layout 887's row of 859) take icon 0xe7 and label 0xec cut by
command: 0 Attack, 1 Item, 2 Draw, 3 Skill, 4 Special, 5 Move, 6 Wait, 7 Change, 8 Run.
Layout nodes of kind 4 draw their sprite scaled by word 5, in percent, about the node: the
title's logo (layout 1056's first node, part 0x15c) at 80 (escha-boot at 17 s; RMSE of the
title 0.202 at full size, 0.030 at 80%). The title's "Press any button" (0x15c cut 1) pulses: a
triangle wave in opacity, 1 down to 0.2 and back over 2.4 s, from 1 as the title starts.
The boot (escha-boot recording): Loading plate, Koei Tecmo then Gust (each
0.5 s fade from black, 2 s held, 0.5 s fade out, back to back), the pre-title movie
`avantitle.wmv`, the title fading in from white over 0.3 s.
The title screen's music is track 1 (`sy_title01_piano`, played once: its KOVS +0x10 is 1;
168 is the same piece as `_2ch`, 2 the orchestral title, 216 `sy_op_loop` the title's loop).
`barrel_formats::bgm::tracks` reads the list and `barrel play-event` prints the
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
with a pointer to the name and the glyph count (2,947); the glyphs start at the first 16-byte
boundary after the name's 0 (0x18 bytes on, here), 28 bytes each: `{u32 code, u16 x, u16 y,
u16 width, u16 height, i32 x offset, i32 y offset (from the line's top to the glyph's), u32 advance, u32 0}`, in rising code order. Codes
0 to 126 are ASCII by number (0 to 31 are 3×3 blanks); the rest are the character's UTF-8 bytes
read as a big-endian number (`0xc2a2` ¢, `0xe280a6` …). The Latin glyphs reach 53 pixels below
the line's top; a space advances 10, `A` 23 (`barrel_formats::font`; `barrel font-text EXE
FONT.g1t OUT.png TEXT` lays out a line from both). The multi-language executable (`Atelier_Escha_and_Logy.exe`) holds a table per font folder,
in the folders' order: Japanese (`0x1407d4db8`, 3,007 glyphs), English (`0x1407e96d0`, 2,947,
the English executable's table exactly), Simplified Chinese (`0x1407fd960`, 4,483) and
Traditional Chinese (`0x14081c3f0`, 3,772), in the same layout (their glyphs start 0x20 bytes
after the name, the boundary falling there; `font::FONTS_MULTI`). Every table has the kana and
Latin; only the Simplified one has 这, the Traditional one 這.

The font also holds the 33 button icons that `<IMnn>` puts in a line: codes `0xd090` to
`0xd0b0` (which would read as Cyrillic А to а), 50×50 at the atlas's top. The glyphs run B, B, A, A, X,
X, Y, Y, then the pad, LB, LT, RB, RT and so on; the texts name the first four by role, the
Japanese way (`Menu_Line_Help.xml`: "<IM02>Return <IM00>Confirm"), and with the confirm button on
A (the English release, and Barrel) 00/01 draw the A glyphs and 02/03 the B ones
(`message::western`): the maps' Return row shows B, as recorded. The colours that
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
entry at 2 or more: the item list's, read by the monster's number (`tables.md`, "Requests"). The engine's rule is `library::learn_item`; `barrel save` counts the
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
order; or a monster counted from the first `MONSTER_` name), how many are wanted (0 an
empty slot) and done so far, the day posted (220, the 11th, a redraw day) and the assignment's
deadline (329), a flag set to 1 on posting, the points (6 where the table's are 5.12) and two
prizes as item row and count, laid out as the game's slots at `0x1410985d8` (`tables.md`,
"Requests": the rolls). The slots hold rolled values, not request numbers: `quest::board_from_save`
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
some (011, 012, 031, 032) match nothing well, so they are unvoiced or voiced elsewhere. So a story line's clip is its scene's first
clip plus its index. The first clips are a table in the executable: an i32 per event in the
`EVENT_*` order (1,165) at `0x14082d710`, read by `0x140213420 (event)`, -1 for an event without
voices (`voice::first_clips`). 521 events have voices; the opening's are as the read log found
(111 at 12900, 121 at 12927, 131 at 12962, 212 at 12914 ...), and 011, 012, 021, 022, 031, 032 and
040 have none (021, 022 and 040 do not share the other alchemist's clips after all). The
multi-language executable holds the same table, so the Japanese and English banks share the
numbering. The viewer plays each line's clip from it as the line comes up
(`stage::Stage::story_voice`; `barrel-escha` prints "story voices from clip 12900" for the first
opening scene). Matching clip sizes to line lengths (`voice::story_start`) guessed the starts
before the table was found; `barrel voice-map INSTALL` now compares the two: of its 520 guesses
135 are the table's, and 111 of the 192 strong ones (correlation 0.85 or more), so it is only a
cross-check.

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

**A heightmap alone.** Nine of Ryza DX's fields (`S35_S_COVE_03`, `S40`/`S41_M_TOWER`,
`S45_M_UNDERGROUND_01`, `S53_M_STARBOTTLE_01`, battle points `S76`, `S78`, `S80`, `S82`) have a
4.9 KB terrain whose layer and blend offsets are 0, with no layers and no patches: only the
heightmap (at 0x1f0). Their ground is drawn by their models; the terrain is read with empty
texture packs and not drawn.

The settings block starts with layer records at 0x1c (file offset 0xb4), 32 bytes each, as
many as the header's u32 at 0x28 says: `{u16 colour texture, u16 normal map texture (0xffff:
none), u16 0, u8 n, u8 n, f32 u scale, f32 v scale, 16 bytes of 0}` (`ktf::Layer`). The scale
is 1 / (cell size × n): the layer repeats every n cells. Escha's layers are texture k for layer k
with no normal map, one per texture (all 140 readable terrains; the battle terrain's records
sit elsewhere). **Ryza DX** pairs each colour tile with a normal map: Rasenboden's 6 textures
are 3 layers (grass, sand, cobblestone; records `{0, 1}`, `{2, 3}`, `{4, 5}`, the third u16
0x101, n = 1 at 1024×1024), so the draws' layer numbers count records, not textures.
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
- **A quadtree** (its node layout is read below, "Read order"): the root centred on (2000, 0,
  2000), half-size 2000, its children 1 to 4; the four leaves centred on (1000, 1000), (3000,
  1000), (1000, 3000) and (3000, 3000), half-size 1000.
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

**Read order (from the executable).** `0x14066bed0` reads the file as one stream, each
section by its own reader, each checking that it read the length the header gives:

| File offset (TOWN_01) | Reader | What |
|---|---|---|
| 0x00 | | `KFTK`, version `0820`, 10, the file size, the section offsets (0x10 layer textures, 0x14 blend maps, 0x1c heightmap), then counts from 0x24 |
| 0x48 | `0x14066f900` | 0x50 bytes: the terrain's lighting material as in g1m section 0x10001 (diffuse 1, 1, 1, 1 = `cDiff`; ambient 1, 1, 1 = `cAmb`; flags `0x133`; 0.3 three times and 4.0: specular and power), then the cell size and others (TOWN_09: 100, 25, 1/256) |
| 0x98 | `0x14066fc90` | 0x1c bytes, the same in every file (`0x310, 0, 7`, then u16 pairs: a vertex format, by the look) |
| 0xb4 | `0x14066fc50` | the layer records, 32 bytes each, count at 0x28 |
| | `0x14066f940` | 8 bytes a layer, only when a flag passed in is set (not in the files read) |
| 0xd4 | `0x14066f690` | the 48-byte records, count at 0x2c (4 in TOWN_01, 6 in AREA_08_02) |
| 0x194 | `0x14066f6d0` | the node tree, count at 0x30 |
| 0x2d0 | `0x14066fb10`, `0x14066fad0`, `0x14066fba0`, `0x14066f250`, `0x14066f2b0`, `0x14066f150`, `0x14066f1d0`, `0x14066c3b0`, `0x14066c3f0`, `0x14066f110`, `0x14066f210` | the rest, in that order: `fb10` reads 16-byte records into 24-byte slots, `fad0` the same, `fba0` 8-byte ones into 24-byte slots |

The counts (u32 from 0x24, as the dispatcher passes them): 0, layers (0x28), 48-byte records
(0x2c), nodes (0x30), 4 / 6 / 4 (TOWN_01, AREA_08_02, AREA_01_01; 0x34, not passed to these
readers), the patches (0x38: 1 / 4 / 4; `fb10`'s and `fba0`'s count), `fad0`'s count (0x3c: 2 /
15 / 51), `f2b0`'s (0x40: 128 / 4320 / 51552) and 22 / 2538 / 43246 (0x44). So after the nodes:
a 16-byte record per patch (`fb10`), 16-byte records by 0x3c (`fad0`), and, when a flag of the
terrain object (+0x8a, a byte that also divides) is set, 8 bytes per patch (`fba0`: a word kept
at the patch's +0x10 and a size divided by that byte, by the look an index range: offset and
count). `f250` builds something from the 0x1c-byte block (the vertex format), and `f2b0` hands
0x40's count to a virtual reader (the vertex data, presumably). The patches are therefore draw
chunks (vertex and index ranges per leaf), not blend descriptions: which layer weighs where must
ride on the vertices or come later in the file.

**A node** is a 0x28-byte record, then 5 integers, or 9 when bit 0 of its flags is set (it has
children): centre (3 floats), 1.0, half-size (3 floats), 12 bytes of 0, the patch it holds at
+0x20 (an index counted up through the leaves, -1 where none), its depth at +0x24 (0 the root),
its flags at +0x25 (0x01 children, 0x04 on the root and on leaves without a patch). The five
integers are the parent and the neighbouring nodes (-1 for none); the reader drops them. The
four after them, on a node with children, are the children's numbers (-1 for none), turned into
pointers. TOWN_01: the root and four leaves, one holding patch 0; AREA_08_02 (non-square, three
levels): nine nodes, leaves 2, 3, 5, 8 holding patches 0 to 3. What follows the nodes is not
fitted yet byte for byte: in TOWN_01 one patch record (16 bytes) and two `fad0` records (32)
from 0x2d0 leave 0x300 to 0x318 for `fba0` and what `f250` reads before the records with
`0x80ffffff` at 0x318.

**How the game blends (from the field's own shader pack, `<AREA>.g1s`).** AREA_01_01's pack
(16 programs) holds three tiny pixel programs (5, 6, 7) that each sample one layer tile
(`sLand0`, at TEXCOORD0.zw) and one blend map (`sBlendMap`, at TEXCOORD0.xy), and output the
tile times one channel of the map (5 red, 6 green, 7 blue) with alpha half that weight. Their
vertex program (1) takes integer positions (`itof`), scales them by `scale` and lifts them by
`gMrpRng`, and gives the tile coordinate as position × `texsize0` and the map coordinate as
position × `bmsize` + `uvOffset0`. So the ground is painted pass by pass, one (layer, map,
channel) per pass, added together, and the combinations are what the file's per-patch data
lists, not a fixed two-maps-for-six rule. The lit ground programs (8 to 15) come with one tile
(8), two tiles and a map (9, 10, 11: again one per channel) and more; TOWN_01's pack (1 layer)
has only the one-tile kind. The real ground vertex is POSITION, NORMAL, COLOR (programs 2, 3).

**Patches and their draws.** After the nodes come a 16-byte record per patch (count at 0x38):
`{u32 first draw, u16 draws, u16 4 or 5 (1 in TOWN_01), u16 draws, u16 draws, u16 patch, u8,
u8}`; the draws of all patches add up to the count at 0x3c. Then the draws, 24 bytes each (the
executable reads them as a 16-byte and an 8-byte part):

| Byte | What |
|---|---|
| 0 | the program within the pass kind (below) |
| 1 | the pass kind: 0x00 the depth pass (key `00 ff ff ff`), 0x10 one layer painted alone (the bake programs 5–7: byte 0 is 0, 1, 2), 0x20 the lit ground |
| 2 | 4 or 6 in the lit draws (6 with programs 13–15), 4 in the bakes; meaning not settled |
| 3 | the blend map (0 in AREA_01_01's lit pairs, 1 in its bakes) |
| 4 | the number of layers drawn (0, 1 or 2) |
| 5, 6 | those layers (0xff where none) |
| 8–23 | first vertex, vertex count, first index, index count |

**Atelier Ryza DX** sets two more bits on every lit and one-layer draw: bit 0 of byte 1 (kinds
0x11 and 0x21) and bit 4 of byte 0 (programs 0x10 to 0x17; the depth draws stay 0x00). So the
kind is byte 1's high nibble, the program byte 0's low one, and the rest flags (`ktf::Draw::flags`;
over Escha's 8,512 terrain draws, `barrel ktf-scan`, none is set). What the bits change is not
read yet; S06_T_RASEN (Rasenboden): 16 patches, 86 draws, programs 0–7 as in Escha.

In the lit kind byte 0 + 8 is the shader pack's program: 0 is one tile (program 8), 1–3 two
tiles under a map's channel pairs (programs 9–11), 4 one tile plus the bake (12) and 5–7 two
tiles plus the bake (13–15). **The bake:** the one-layer draws (kind 0x10, programs 5–7 by
byte 0: red, green, blue) paint each of their layer's tile times its map channel, alpha half
the weight, added together into a screen-sized target (`sRtt`, read at the projected
position). Program 13 then takes its pair's two weights plus twice the bake's alpha as the
whole weight, adds the bake's colour to its two tiles and divides by that whole; program 12
weighs its one tile by 1 − 2 × the bake's alpha (at least 0) the same way. So a patch with
more layers than one map holds blends three of them through a map's pairs and the rest (under
the second map) through the bake. Program 9 reads the map's red and green, divides each by their sum (plus 1/1024) and
mixes its two tiles by them; the tile's alpha (0 in the files read) scales the specular term,
and the result is halved. The pairs follow one channel per layer: in AREA_01_01's first patch
the draws `0 2` (program 9: red, green), `2 1` (10: green, blue) and `1 0` (11: blue, red:
it reads red and blue and swaps them) give the first map's red to layer 0, green to layer 2
and blue to layer 1. The layer numbers in the bakes (`0 1 2` in patch 0, `0 1 3` in patch 1)
do not fit the same layers under a second map, so a draw's layer numbers presumably count in a
per-patch texture list (the patch record's 4 or 5), which is not found yet. So a patch is drawn
as the overlaps of its layers two at a time, each overlap its own index range. TOWN_01 (one
layer): a depth draw and one lit draw of one tile.

**Vertices and indices.** After the draws: 8 bytes per patch (`fba0`: the patch's first
vertex and first index as byte offsets into the two lists below; the second is divided by the
index size, a byte of the terrain object, as it is read), then the vertices
(the byte count at 0x40, 16 bytes each) and the indices (u16, the byte count at 0x44); they
end exactly at the layer textures' offset (0x10) in TOWN_01, AREA_08_02 and AREA_01_01, so the
block is read whole. A vertex is `{s16 x, s16 height, s16 z, s16 0, u8 × 4 colour, u8 × 4
morph colour}`: x and z count cells (times the cell size at 0x84: 100 in AREA_01_01 over 0 to 64,
250 in AREA_08_02 over 4 to 20 and 0 to 60). By the terrain's vertex program (the field's
`.g1s` program 2), the two colours' rgb are the vertex colour near and toward the coarser level,
blended by the LOD morph (`gMrpRng`), and their fourth bytes are the normal's x and z (unorm
× 2.007874 − 1.007874; y = √(1 − x² − z²)). The colour is baked shade, white in the fields
first read (AREA_*: `ff ff ff 80`), not in the towns: means 0.61 in TOWN_09, 0.88 in TOWN_04,
0.96 in TOWN_10, darkest under walls and in corners. The light is `MapSTD`'s on that colour:
ambient × colour × `cAmb` + the second light's diffuse × colour × `cDiff`, and the sun's
diffuse × colour × `cDiff` apart (for the shadow and the light texture). The patches' vertices follow each other
(a patch's count is where its farthest-reaching draw ends; they add up to the whole). A draw's
first index counts from its patch's first index and its indices from its patch's first vertex
(its vertex range is only the lowest and the span its indices use). The indices are triangle strips, repeated indices joining the runs. The height spreads the
s16 range over a node's height bounds: low + (stored + 32768) / 65535 × (high − low), the
bounds being those of the topmost ancestor that holds a patch too (fields with levels of
detail, such as AREA_04_01, have coarse patches whose children are finer patches over the same
ground, the patch record's byte 0xc giving the level: the finer ones store their heights
against the coarse one's bounds). Checked by `barrel ktf-scan`: 581,713 of 658,935 vertices
over the 140 meshes land within 0.5 of the heightmap; most of the rest sit exactly 10 lower
(skirts along the patch edges, by the look). `battle`'s terrain does not read as a mesh. The vertex program turns the integers to floats and scales them by `scale`. So the
terrain is its own mesh, not the heightmap: TOWN_01's is one 500 × 500 square (cells 3 to 4,
0 to 1) of 8 vertices, the rest of its ground being models.

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

### Ryza field lights (`<FIELD>_light.bin`)

In the field container of Atelier Ryza DX and Ryza 2 DX (Ryza 3 DX's fields have none; they
carry `_version.bin` and `_commonInfo.bin` instead). Read by `barrel_ryza::light`; all of Ryza
DX's fields read, with 0 to 55 lights each (`barrel ryza-field INSTALL`). A u32 count, then 40
bytes a light:

| Offset | What |
|---|---|
| 0 | f32 x, y, z: where it stands, in field units (Rasenboden's 24 at lamp height through the village; the attic's 2 near its floor) |
| 12 | f32: 5 in every light read |
| 16, 20 | f32 × 2: 2.55 and 1.5 in the attic, 4.9 and 0.157 in Rasenboden: strength and falloff, by the look |
| 24 | f32: the range (700, 800, 1000) |
| 28, 32 | f32 × 2: 0.5 / 0.3 in the attic, 0.45 / 0.2 in Rasenboden |
| 36 | u32 kind: 0 in the attic, 2 in Rasenboden |

There is no colour, so these are not the whole light: see "Ryza field environment".

### Ryza's character files

`\data\x64\character\pc00a\`: `pc00a_model.elixir.gz` (the skeleton `PC00A_MODEL.g1m`, the
mesh `PC00A_MODEL_default.g1m`, `PC00A_MODELOid.bin` (the bones' names, "Swing settings") and
`PC00A_MODEL.swg` (the swing settings)), `pc00a_model.g1t` (her textures),
the motions (`pc00a_<NNNNN>`, "Ryza's motions") and `face\` with 30 small containers: the
expressions (`pc00a_face_expression_<name>`: default, smile, anger, sadness, surprise, wink, ...),
the eyes' directions (`pc00a_face_eye_<dir>`), blinks (`closeeye_up/down`) and
`pc00a_face_pronunciation`. Each holds one `.g1a` of one frame (0.017 s) with 67 tracks: bone 0
and the face's bones 300 to 365, which her field motions (96 tracks, none at 300 or above) do not
move. `barrel_ryza::face` lays `expression_default` over her pose and blinks toward
`closeeye_down`; the rest are not used yet. Her mesh groups name their shaders by a 32-bit hash
(`@CB0965E5`, `@4196223C`, ... in `barrel g1m-materials`; submeshes 0 to 7 and 20, 21 also
carry the numbers 20000 to 20006); the hashes match none of the 212 shader packs' names
(`\data\x64\shader\*.g1s`, CRC32 or FNV-1/1a or djb2 or sdbm, any case), nor any string in
`saves\shader\shader.xml` or in the packs' headers, and their bytes (either order) and text
are in no pack and not in the executable, which embeds no character shaders (one `_S2G` pack
of stage names, 113 bare DXBC blobs) and compiles none at run time (no `D3DCompile`).

`saves\shader\shader.xml` (`<shader Name Type Autoload Id OutlineFlg Scm String File>`, 274
rows) maps the names models use (`String`) to packs (`File`) by kind (`Type`): 8 the G-buffer
programs (205: `STD`, `PBB`, `PBB_Gb`, `PBBA2`, ... mostly onto `PBB_Cll.g1s` and its `_Gb`
twins), 5 the shadow maps (14: `SM`, `SM-C`, `SM-B0`, ...), 14 the deferred lighting passes
(`DS-Cll`, `DS-EmCll`, `DSChara-Cll`, `DS-CllRr`, ...), 26 the eyes (`EYET`, `EYET_DSGb`), 10
grass, 11 rivers, 7 plain colour programs, and one each for the sky, clouds, stars, water,
decals and effects. `chara_shader_param.xml` names the character passes' shared textures:
`sEnv` (`Shader/texture/defaultref.g1t`), `sOutline` (`outline.g1t`), `sToon` (`toon.g1t`),
`sReflection` (`defaultmask.g1t`). Each pack's header names its variant after the magic
(`DSChara-Cll.g1s`: `PBDS-CllEEmIra3lnhIriLr`). `DSChara-Cll`'s pixel program lights the
G-buffer (six targets `sGBuf0`..`5`, depth) with a ramp (`sRamp`, `rampCoordScaler`, ...), a
rim table (`cRimTbl`) and the clustered lights. The parts tables
(`charapartsdefaultvisible.xml`, `charapartschange.xml`, `charapartsmodel.xml`) have no rows for
`PC00A`: all her parts show.

### Ryza's speeds

No table gives how fast she walks or runs (`charabase.xml`, `charamotionset.xml`,
`basemotion.xml` and the field tables have no speeds; `basemotion.xml` only lists the base
motions' numbers: 0, 11, 21, 131, 30, 40, 70 to 72, 200 to 250, 1000 to 1020). Her loops play in
place, so `barrel_ryza::motion::stride_speed` takes the speed at which a planted foot
(`SK_L_Foot` 11, `SK_R_Foot` 12, within 1.5 units of its lowest) does not slide: walk loop
(11) 68.4, run loop (21) 415.7 units a second, 71 and 432 at her model's scale (1.04). A
step-length check agrees (the walk's foot swings 47 units over a 1.35 s cycle of two steps).

### Swing settings (`*.swg`)

Next to a Ryza character's model, `<MODEL>Oid.bin` names its bones (`barrel_formats::oid`):
after `HeaderCharaOid`, `ObjectID:<MODEL>` and a version, each entry is `id,name`
(`0,root`, `6,SK_Head`, `300,F_Head_Down`, `952,SWING500_Hair000`; `oid::by_id`). The ids are
the skeleton's bone ids: `SK_*` the body, 300 and up the face (`F_*`), 500 to 1151 the
swinging parts (652 bones in Ryza's: `SWING000` the bust, `SWING100` the head, `SWING150`,
`SWING200`, `SWING300`, `SWING400` the skirt's bottom, `SWING500` the hair, `SWING600` the
options, most in left and right chains of 25).

`<MODEL>.swg` (`barrel_formats::swg`, `barrel swg FILE#N OID#N`): magic `SWGQ`, `{u32 header
size 0x24, u32 count, 8-byte name, f32 1.0, 12 bytes 0}`, then records of 0x48 bytes `{u16
group, u16 bone id, 15 floats, 4 u16}`. Ryza's (`def014`) has 10, on the bust (500, 501), the
skirt's bottom (902, 927) and the hair (952, 953 and the side locks 1052, 1053, 1077, 1078):

| group | floats | words |
|---|---|---|
| 0 bust | 60 20 25 25 0 0 3 1 2 2 0 0 0 1 0 | 10 3 25 25 |
| 400 skirt | 90 90 90 90, then 0 | 25 4 25 100 |
| 500 hair | 45 and 0 to 45 (four), 0 ..., 0.8 (-69 on the side locks' roots), 5 | 25 4 15 or 25 100 |

The first four floats look like angle limits in degrees. The other swing bones (the head,
150 to 300, the options, the rest of each chain) have no record: their settings, and how the
game moves the chains (its own physics), are not known. Her field motions do not move the
swing bones, so they keep the bind pose.

### Ryza's motions

A character's motion set is named in `saves\chara\charamotionset.xml` (`MotionsetTag`,
`Directory`, `BaseName`; Ryza `PC00A`, the townspeople's sets with a `SkeletonBase` height),
and each motion is its own container `\data\x64\character\<set>\<set>_<NNNNN>.elixir.gz`
holding one `.g1a` (Ryza: 107 of them, 00000 to 09120). `saves\chara\basemotion.xml` lists the
ones every set has: 0, 11, 21, 131, 30, 40, 70 to 72, 200 to 250, 1000 to 1020. The field's
come in threes, start / loop / stop: 10, 11, 12 walking (0.63, 1.35, 0.67 s), 20, 21, 22
running (0.25, 0.75, 0.65 s); 0 is the idle stand (2.52 s). Posed side on, 11 is a walk and 21
a run. They stay in place: the root bone (joint 1, the hips at 86 to 89 units) does not
travel, so the speed she moves at is the game's. Motion 131 lowers the hips to 77 (a crouch);
30, 40 and the rest are not identified yet. The tracks name bone ids, mapped to joints through
the skeleton's index map ("Skeletons" above).

### Ryza field walls (`<field>_le.koc`)

A loose file beside the field's containers (36 fields in Ryza DX; Ryza 2 DX has 47, Ryza 3 DX
85). Header: `_COK0100`, u32 file size, u32 quad count; then each quad's four corners as
(x, y, z, 1) floats in game space. Nearly all are upright: (a, top), (a, bottom), (b, bottom),
(b, top), the line a to b on the ground spanning bottom to top (Rasenboden: 99 quads, 96
upright, 300 to 14,000 units long, the field's edges, cliffs and banks). Two there are sloped
and one flat (at y 5843), use not known. `barrel_ryza::koc` reads them and gives the upright
ones to the shared walker as walls. Beside it: `_coll.bin` in the main container (Rasenboden:
4 records of a position, a rotation and a size, likely boxes), `.g1pf` and `.elixir.g1pf`
(`FP1G`, over a 128,000-unit square: path-finding, not read) and `.trmd` (`TRMD`, 1024 × 1024
cells of 5 bytes: likely the ground's material, not read). In Rasenboden's `.trmd` (header
`TRMD`, u32 1024, 1024, 0) only each cell's first byte varies (the other four are 0xff): three
values, 4 (579,687 cells), 36 (434,751) and 13 (34,138). Drawn as an image, 13 is one long
band (river-like), 36 thin branching lines (path-like) and a large region, 4 the rest. Where
the grid lies over the field is not settled: from the origin or centred, in either axis
order and either mirroring, the gate's dirt path and the grass tufts beside it fall in the
same class, so the cell size is likely not 125 units either. Likely the footstep and
gathering material (`SE_FLD_057_FOOT_GRASS01`...); not the terrain's holes.

**`_grass.bin` and `_grass.g1t`** (Ryza DX, in the main container): the field's plants.
`_grass.bin` (Rasenboden: 263,116 bytes) begins with the area's size (f32 128000, 128000), a
u32 30000 (likely the draw distance), an f32 0.01, the grid's size (u32 256, 256) and a u32
count of plant kinds (19). Then 5 floats per kind (zero-padded to 0x3cc): Rasenboden's first
(50, 100, 60, 40, 0.1), (110, 30, 60, 60, 0.1), (70, 30, 60, 20, 3): sizes in units and,
last, a density (0.01 to 3); which float is which is not settled. The rest of the file is the
grid, 256 × 256 u32 cells of 500 units over the area from the origin, x fastest: the kind in
each cell, 0xff for none. Rasenboden fills 1,446 cells (x cells 0 to 100, z 37 to 131), with
kinds 7 (785 cells: the large yellow fields), 6 (483, strung along the paths), 5, 8, 9, 10
and 0. `_grass.g1t` holds three 2048 × 1024 textures: an atlas of 8 plant cards (4 × 2,
512 × 512 each: weeds, flowering plants, a grass tuft, wheat, BC3 with alpha), one filled with
red (BC3; a placeholder or a mask) and the cards' normal maps (BC1). Which card a kind draws
is not in its record: the game builds the plants' quads on the CPU (`ObjectGrassPBS.g1s`'s
vertex programs take position, normal, colour, UV, tangent and a billboard size, with
`g_fWidthScale`, `g_fHeightScale`, `g_fFarClipBegin`/`End`, `g_fFarScaleBegin`/`End` and a
sway phase `g_fPhase`, `g_fOscPhase`), so the choice is in the executable. The grass quality
and range presets (`saves/graphicssetting/grassqualitypreset.xml`, `grassrangepreset.xml`):
density 1 and fading from 8000 to 15000 units at Middle, the highest. `barrel_ryza::grass`
reads the file; `barrel-ryza` draws the plants (docs/ryza/open.md). `_coll.bin` is a u32 count of boxes
(position, rotation in radians, three sizes, then two words: 2 and 15 in all of Rasenboden's;
44 bytes each), then a u32 count of cylinders (position, rotation, radius, height, the same two
words; 40 bytes). Rasenboden's four boxes are tall and thin (38 × 715 × 37 by its first, near
the path below Ryza's house) and its cylinder wide (704 by 547): invisible blockers, read by
`barrel_ryza::solids::field_shapes` (taken as half-sizes about the position, the cylinder's
height upward: not checked).

### Ryza field cameras (`saves\fieldmap\cameraparam\<field>.xml`)

One per field (100 files; Shift-JIS XML). `free_camera/base_setting` is the player's camera:
`fov_default` (Rasenboden 45), `yaw_default` (180: behind the player, against her facing),
`pitch_min/max/default` (-30, 45, 8, degrees), `length_min/max/default` (200, 550, 400 units)
and `at_diff_y` (0.1, not placed). `camera_func/look_up` (`max_at_diff_add`,
`min/max_length_by_pitch`, `look_up_start_pitch`, `eye_down_end_pitch`) changes the look point
and distance as the camera pitches up, not read. Then `landmark_camera`s (indexed 1, 2, ... in
file order, chosen by gimmicks), `disp_range` (draw distances in metres for characters, effects,
gimmicks, near/middle/far/landmark objects, shadows: Rasenboden's PC objects 70, 200, 400 and
3000, its Switch twin `disp_rangeNX` 100, 150, 200 and 3000 with console variants
`*_console`; read by `camera::DrawRanges`), `lod` (`use_lod`, `hi_mid`, `mid_low`: off on the PC,
on for the Switch's `lod_NX` at 60 and 80), `light_map` (marked unused),
`RLR` (screen-space reflection steps) and `ambient_effect` (effects per time and weather).
`barrel_ryza::camera` reads the free camera. With it (vertical field of view taken as 45°)
and a look point 110 units above her feet, Rasenboden's gate frames as in the game, the
player a little larger.

### Ryza model collision (`<model>_c*.g1co` in `<field>_obj`)

Beside each model's `<model>_hi.g1m` (entry names without the placement's `obj-`), its collision
in the model's space, in four kinds by suffix: `_cbvh[N]` is a triangle mesh in Escha's field
collision form (`OC1G`, one `HVB_` chunk, `barrel_formats::g1co`; Rasenboden: all 37 read, on
cliffs, rocks, trees, the floors and stairs), `_cplh[N]` one convex hull in an `HLP_` chunk
(offsets from the chunk's start: at 16 the triangle count, the triangles' offset and the vertex
count, u32 each, then the vertices' and the adjacency's offsets, u16 each; at 0x20 eight
(offset, count) pairs into the adjacency; at 0x40 the centre; the triangles as u16 triples; the
vertices as (x, y, z, 0) floats from the centre, the cliff's lowest at the mesh's lowest; then
each vertex's neighbours),
`_cbox[N]` boxes and `_ccap[N]` capsules; two stumps carry `_Group<N>.g1co` meshes. A box or
capsule file is `OC1G` (16 bytes, the version and size as the others), a kind byte (0 a box, 3 a
capsule), then floats with no chunk: a box's centre, its three axes and its half-sizes, each four
floats (the axes identity in all of Rasenboden's 47); a capsule's centre and axis (four floats
each, the axis (0, 1, 0) in all 26), its length and its radius (tree trunks: radius 46 to 284).
Most are in the model's y-up space and taller than the mesh (the signpost's box is 416 high over
a 140-unit post); a few lie along z instead (the light pole, the clothes pole), which nothing in
the file marks, and the bags' boxes fit their mesh in neither way.
`barrel_ryza::solids` places each mesh as its placement puts the model
(`field::placement_transform`): in Rasenboden 956 placed meshes, 282,000 triangles, giving the
walk its walls (upright triangles) and floors (the rest) beside the `_le.koc` quads.

### Ryza's character tables

`saves\chara\chara.xml` has a `<Chara BaseTag=...>` row per character: `Size` (the model's
scale; Ryza `PC00A` 1.04), `Height` and `Radius` (154, 44), `CamH` and `CamR` (160, 50), `Foot`,
`Body`, `Face`, `Head`, `Extra`, `bbScale`, `EncHeight` and others. `saves\chara\charabase.xml`
has a `<CharaBase BaseTag=...>` row: the model and texture paths, `FMrad`, `FMhig`, `FMslp` (the
field collision's radius and height in metres and its steepest slope: Ryza 0.28, 1.5, 75, as
Escha's), `clothWindScale`, `localGravity`, `wetMax` and the motion sets (`motset0`,
`motset1`). `barrel_ryza::chara` reads the scale and the collision; `CamH` is read, but taken as
the camera's look height it puts Rasenboden's gate view inside a tree where the game's is
clear, so its meaning is open.

### Ryza field environment (`<field>_env.elixir.gz`)

Each Ryza DX field has a second container beside its objects': `<FIELD>_env.bin` (97,920 bytes
in the attic) and two cube maps, `<FIELD>_ad.g1t` (256 × 256 BC6H, 6 faces, 1 level) and
`<FIELD>_as.g1t` (the same with 6 levels). So the fields are lit from HDR cube maps, a diffuse
irradiance map and a pre-filtered specular one, unlike Escha's per-field light table.
The cube maps are not the field's own: the attic's and Rasenboden's `_ad.g1t` are the same
file byte for byte (their `_env.bin` differ), and the env containers come in a few sizes
only. The diffuse map's faces are in the game's axes, y up: +y is the sky's blue (0.19, 0.26,
0.42), -y a warm ground bounce (0.47, 0.36, 0.28), the sides about 0.3. `barrel-ryza` turns it
into nine spherical-harmonic coefficients (`barrel_ryza::environment`) for the renderer's
ambient term (model.vert's `sh`), scaled and tinted by the set's `indirectStage_` block (0x54c:
scale, specular scale, colour, ..., `AmbientLightScale` at +0x30; `StageLight`), the characters'
by `indirectChara_` (0x5d0, `CharaLight`).

**`_env.bin`** is 12 sets of 0x1fe0 bytes (97,920 in every field read), one per weather and
time of day: set = weather × 4 + time, times morning, noon, evening, night (0–3), weathers
sunny, cloudy, foggy (`saves/fieldmap/cameraparam/env/envtemplate.xml` names both; it also
lists an after-rain weather the file has no sets for). The field HUD shows the current time of day as
one of four clock icons in the same order (`gen_a21_field01_clock_icon__xLIST01` to `04` in
`ui/a21_field.g1t`, texture 3, 180-pixel cells in a 4096 space over the 2048 texture); the
game's 10 AM shows the second, noon. Only about 150 of a set's 2,040 words
differ between sets, and about 2,000 words between the attic and Rasenboden, mostly before
0x800. The parameters are named in `saves/fieldmap/override_env/env_*.xml` (250 attributes,
`<block>_<member>`: trueHDR, ambientLight, parallelLight00/01, indirectStage, heightFog, skyFog,
fog, cloud, cloudParticle, gust, shadowMap, ssao, rlr, ir, outline, ...), and the executable
holds a reflection table of the classes (`EnvDataParallelLight`, `EnvDataHemiLight`,
`EnvDataLightShaft`, ...) with their members in order, from about 0x13ce900 in the file. The
binary keeps the blocks in another order than the XML. Placed so far (set 0): 0x00 `fLumiMax`
(4.2), 0x1c `fMiddleGray` (0.18), 0x20 `vExposureRange` (0.9, 1.2), FXAA from about 0x28,
the height fog layers from 0x78 to 0x12c, the clouds from 0x1a4 to 0x1ec, the cloud particles
from 0x210 to 0x244, the reflections (`rlr_`) from 0x254. **From the executable** (the XML reader, `0x14071d000`, hands each block's reader the
structure's address plus an offset, and each reader names its members): `trueHDR_` from 0
(`fLumiThreshold` 0x18, `fMiddleGray` 0x1c, `fLightAdaptedScale` 0x28, `fDarkAdaptedScale`
0x2c, the FXAA four from 0x30, `fGamma` 0x40, `fBloomWeight` 0x5c ...), the height fog layers
0x7c, the sky plane glow 0x180, clouds 0x1a4, then 0x21c, 0x260, `shadowMap_` 0x284, `ssao_`
0x310, `gust_` 0x34c, `lightShaft_` 0x380, `lpv_` 0x3a0, `SkyDome_` 0x41c, `mainAmbientLight_`
0x4c8 (`{rgb, extension}`), `parallelLight%02d_` 0x4d8 (two of 0x18 bytes: `{rgb, elevation,
azimuth, extension}`), `fog_` 0xd4c, `stageAmbientLight_` 0xd78, `indirectStage_` 0xda0,
`patternMap_` 0x15e4, `skyFog_` 0x1604, `indirectChara_` 0x1630, `albedoRetouch_` 0x1edc,
`twinkle_` 0x1f40. The file matches those offsets up to the HDR block at least, but holds the
ambient and the lights 0x44 bytes earlier: `mainAmbientLight_` at 0x484, the sun at 0x494, the
second light at 0x4ac (`barrel_ryza::environment::Light`). Where the 0x44 starts is not placed,
and later blocks shift again (`indirectStage_` is not at 0xd5c). The HDR block in the file, at the
executable's offsets (Rasenboden's sunny sets morning / noon / evening / night): 0x00
`fLumiMax` (4.2 / 4.3 / 2.5 / 4.2), 0x18 `fLumiThreshold` (0.3, evening 0.8), 0x1c
`fMiddleGray` 0.18, 0x20 `vExposureRange` (0.7 / 0.9 / 0.9 / 0.9 to 2.4: not the default
override's 0.9 to 1.2), 0x28 `fLightAdaptedScale` 0.5, 0x2c `fDarkAdaptedScale` (0.5,
evening 0.3), the FXAA four 0x30 (0.5, 0.5, 8, 2), 0x40 `fGamma` (2.2, noon 1.8), 0x44
`vSaturationScale` per channel (placed by its values only: laid on as saturation it worsens
the noon still's match, as a colour scale it does no better; not applied) ((1.2, 1.2, 1.0) / (1.27, 1.3, 1.2) / (1.2, 1.1, 1.0) /
(1.2, 1.2, 1.2)), 0x50 `vColorScale` (1, 1, 1; placed by the override's values), 0x5c
`fBloomWeight` (0.4, evening 0.6); `barrel_ryza::environment::Light` reads them. The same
three saturation words recur at 0x2f4. Comparing the four sets word by word, the others that
differ: 0x08 and 0x0c (1, 1000; noon 0, 10000), the height fog's colours (0x8c on, 0x110 the
second layer's (0.543, 1.41, 2) as the override's), the sky plane glow from 0x178, the clouds
from 0x1ac, 0x274 (200, night 600), 0x310 to 0x34f, 0x460 to 0x48f (0x478 a grey triple 2 /
3 / 3 / 7 before the ambient), the lights, and blocks from 0x4f8 to 0x7a0, 0xff0, 0x17f0.
Read that way, Rasenboden's
sunny sets give: morning sun (5, 4.62, 3.5) at 30° elevation, 65° azimuth, ambient (21.6, 30,
27.2); noon ambient (3, 3, 4); evening sun (5, 2.07, 1) at 30°, -30°, ambient (2, 1.4, 0.8);
night ambient (0.39, 0.53, 0.77); the second light a dim blue (0.12, 0.14, 0.3) at 10°, -164.5°.
The attic's morning sun is (3, 2.92, 2.72) at 15°, 52.5°.

The default XML gives the shape of the lighting: the sun (`parallelLight00`: colour 8, 8, 8, elevation 22°, azimuth -20°), a second
directional light (1, 1, 1 at 7°, 45°), an ambient colour, and the diffuse cube map scaled by
`indirectStage_IDLScalerOfAmbientDiffuseMap` (2.5) times an RGB (1.64, 1.98, 4.0). Their
offsets are not placed yet. The first words of set 0 for comparison: 4.2, 15, 1, 1000 (fog
on, 1000; noon has 0 and 10,000).

**The sky.** A set names its sky dome at 0x3d8 (64 bytes, zero-ended; `SkyDome_`'s
`SkyDomeDisplaySetFactory` in the executable, whose reader also reads `SkyDomeLightGroupID`,
`SkyDomeWorldPosition`, `SkyDomeWorldScale` and two more): a container
`\data\x64\field\skydome\<name lower-cased>.elixir.gz`. Rasenboden: sunny morning and evening
`MORNING_SUNNY_02`, noon `MORNING_SUNNY`, night `obj-A21-TEST-NIGHTSKY-01_hi`, the other
weathers none; the attic and the living room only noon (`obj-A21-TEST-SKYCLOUD-ANIMATION-01_hi`).
`MORNING_SUNNY_02` is one model (two submeshes, shader `PBSkyDome-NT0Tn`: colour, normal map,
alpha-cut) of clouds alone, 148 km across and 29 km high around the origin: the blue behind
them is the sky shader's (`skyplane.g1s`, the sky glow block at 0x180), not a texture.

**Fog and sky scattering.** Two blocks of the same shape (the executable's `EnvDataFog` reader,
for `fog_` at structure +0xd4c and `skyFog_` at +0x1604; member names from the environment
XMLs): `{f32 start, f32 end, f32 mie[3], f32 g, f32 rayleigh[3], f32 power, f32 ambient}`, in
the file at 0x4fc (`fog_`) and 0x5a4 (`skyFog_`). Rasenboden's sunny morning sky: start 1454,
end 400,000, mie (4.0, 3.41, 2.54), g 0.984, rayleigh (1.5, 1.98, 3.0), power 10; noon: 1000,
300,000, mie 0.71 each, g 0.974, rayleigh (0.02, 0.12, 1.0), power 40. The default XML's: fog
1000, 500,000, g 0.85, power 20, mie (2, 0.95, 0.5), rayleigh (0.075, 0.183, 0.75); sky 1000,
1,000,000, g 0.9, power 30, mie (3, 1.95, 1.5), rayleigh (0.4, 0.76, 4.0). How they become the
shaders' `Scat` constants is not read yet (docs/shading.md, "Ryza DX: the sky").


### Placement lists (`*.opl2`)

Read by `barrel_formats::opl2`. All 141 of Escha's read: 20,283 objects and 150 effects.
`barrel opl2-scan DIR` reads them all (`barrel opl2-scan PAK...` those in archives).

**Ryza DX and Ryza 2 DX** keep the same instance records but a longer type header, and no
effects list: the file ends after the objects. A type is `u32 name length`, the name without
a 0, a word (0, 13, 26, ... in `s01_i_attic`: 13 apart from type to type), 48 bytes of 0xff,
288 of 0, then the instance count. Their 32 bytes after an instance's tags hold 16 zero bytes,
8 of 0xff and a word (`opl2::Instance::word`). Its bits 0-1 look like the draw-distance class
of the field camera file's `disp_range` (0 near, 1 middle, 2 far, 3 landmark: in Rasenboden bags
and pots are 0, grapes and fallen trees 1, stumps and fences 2, cliffs, mountains and the far-end
trees 3; 102, 222, 225 and 646 placements). Bits 0xc000 are on floors and stairs and 0x12000 on
stairs and bridges; in Rasenboden the 0xc000 ones are exactly the placements whose collision
meshes do not lie on their models (`barrel_ryza::solids::NO_COLLISION` leaves them out); the rest (0x08, 0x20, 0x40, 0x80, 0x100) not read. All read: Ryza DX 96 lists
(6,741 types, 53,698 objects), Ryza 2 DX 105 (8,037 types, 78,588 objects). **Ryza 3 DX's**
differ again (116 of 117 fail at the first instance count): not read yet.

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
235, 134), the speaker's gesture for the line, and the fourth (0, 3 or 4) its expression (both read
below, "What the game does with them"). They matter for faces: events hardly set expressions
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
by the look of it). Which clip a story line plays, and the two voice commands, are under
"Voices" above.

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
- **Textures** `0x140db6800`, 887 records of 16 bytes `{u64 dds name, i32 index, i32 file}`
  (`0x1403740a0`): one picture of a `WinXls` pack, `index` its place in the pack's list and
  `file` the pack, named by `[0x140db5c50 + 8 file]` (303 names). Texture 0 is
  `a15_menu_parts01` (`a15_menu.g1t` #0), 7 to 35 `a15_common.g1t` #0 to #28 (`pad_button`,
  `a15_common_parts01` ...), 36 to 42 `a15_mix.g1t`, 43 to 570 `a15_itempic.g1t` (the item
  pictures, then small icons), 571 to 583 `a15_battle.g1t`, 736 `a15_telop_mm01_op1_111_01`.
  Every sprite of the 701 layouts resolves (`barrel layouts EXE`; `barrel layout-png ROOT
  OUT.png ID...` draws layouts).
- Coordinates are on a 1920 × 1080 screen.
- **Nesting**: a node part above 0x164 is another layout, by id, placed at the node (its own
  place added): 105 layouts are not nested in any other, the screens. Part 0x164 (kinds 21, 22, 23) is a text (kind 23 ends at its node, right-aligned: the save list's numbers at x 1800; kind 22 is centred on it: "Select your character." at x 954, the field HUD's days left; kinds
  21 and 22 start there). A kind 29 node is a number's place: the HUD date's month and day
  (part 0x57, 36 pixels a digit) are each centred on their node ("8 /20": the "8" half a glyph
  left of its node, the "20" a glyph left of its; measured on the colseit-arrival recording's
  area map by patch search). Nested layouts sit at their node (776 four times in 788), but the
  field HUD's assignment panel 605 rests at its own place (0, -32), 20 right and 8 down of its
  node in 625 (-20, -40) (same recording; the node is likely its slide-in's start). Its two
  texts stay at their nodes: "Left" (kind 22) starts at its node (x 78 at 1080p, node 80),
  the days left are centred on theirs (x 138 to 183, node 160), about 15% larger than "Left"
  though both nodes say size 27. No word of the node tells the two apart (words 5 to 9 alike). Under
  the date, layout 603 (at the panel's ten 0x25b nodes, five a row, the second staggered) is
  one card per request slot taken (`quest/player`): part 0x8b cut 3 (a scroll) while open, cut
  5 (pink) with the red tick (cut 1) once done, cut 6 (blank) for an empty slot, each pinned
  (cut 12); only six slots show on the user's save (rank 3), the rule not read. Below them, the
  field effect's gauge is layout 614 (node 0x266 in 625 at (32, 246), drawn 4 right of it):
  the box (part 0x69 cut 2), the fill (cut 6, 48 × 108, red at the top to green at the foot)
  shown from its foot up to the gauge over 200 (45 shows its bottom 22.5%), the tube over it
  (cut 17, notched at 100), the percentage (small digits 0xe5, kind 28, ending at its node, 18
  a digit) with its sign (0xe5 cut 13); from 200 "MAX" (0xf0) in their place (`0x1402823fa`).
  The gauge is `fieldmap/m_userDataFieldMapData`'s second word (`0x1410783ac`, kept within 0 and
  200 by `0x140283920`, which also notes crossing 100); its mode (`0x1410783b0`) picks the
  notice beside it (`0x140283a60`): modes 3 to 8 show part 0x6e cuts 0 to 5 ("Collection
  Quantity Changed", "Rare Material Changed", "Relic", "Battle", "Land", "Report Change in
  Effect") on the plate (0x69 cut 4), any other none. `0x140283070` clears both (mode -1,
  values 0, gauge 0). Cut 5 under the plate with layouts 611 to 613 (three choice icons) is
  the effect's choice, shown in the field at 100; 0x6f (kind 36) are glows. The area map's scroll is part 0x92 cut 8 (462 × 750, `a15_common.g1t`
  picture 13), drawn at its own size with its top-left at (1446, 150), found in the
  recording by its roll; part 0x280 in layout 641 is layout 640 nested (the tab), not a sheet. A
  neighbour's label is layout 623: plate part 0x27 (kind 19; cut 0 the left end and body 192
  wide, cut 1 a 48-wide middle repeated, cut 2 the 48-wide right end, 72 high) at a fixed 352
  wide, its kind 22 text at (176, 18) in dark ink; layouts 633 / 634 add the gilt ornament on
  the right / left (parts 0x90, 0x91 through layouts 627 to 630, kind 7), 631 / 632 above /
  below (631: 0x90 upright at (102, 0) over the plate at (0, 64), text (176, 82); 632: the plate
  first, 0x91 at (102, 44) under it). A route going up or down the screen gets those, centred
  on it: the world map's Colseit label north of the Southern Ruins (colseit-arrival 1:12) stands
  11 under its route's third dot and 6 right of it. Layout node kind 7 turns its sprite a quarter clockwise about the node (the
  ornaments: 0x90 points up, turned it points right; 0x91 down, turned left; each has four
  cuts, likely its bob). In the recording the plate's centre stands 52 over its route's dot,
  ~25 nearer the
  screen's middle. On the scroll, 641's rules (part 0xbe cut 5) sit 7 below their nodes and
  the texts are in the layouts' ink (58, 26, 8) drawn opaque (the recording's darkest text
  pixels (57, 27, 6), though the stored alpha byte is 0xe1). The maps' header is layout 652 at the
  screen's top-left: the feather (part 0x92 cut 2) at (1392, -2), layout 651 at (1260, 114)
  (the dark band, cut 11, and the gold line, cut 3) and the region's name (637) at (1440, 66).
  The world map's scroll is the same picture upside down: its flat foot with the folded
  corners on top and its roll at the foot, 9 higher than the area map's, and no button
  (colseit-arrival 1:12, by RMSE of the top edge and the roll); the list's rules under the roll
  are not drawn.
  The area map's scroll is layout 645: the scroll (0x92 cut 8), the item panel 641 at its
  top-left (which is why the rules sit 7 below where Barrel's texts put 641) and the button
  646 at (248, 678): the arrow plate (part 0x39 cut 14) and a pad button (part 0x41 cut 4 at
  (8, 6)). Part 0x41's cuts run in pairs (B, A, Y, X, each with a dim
  twin) and name roles in the Japanese layout: the English release swaps A/B and Y/X there
  (cut 4 shows X in the recording). The font's `<IMnn>` icons swap only A/B: glyph 4 is
  already X (`<IM04> Area Info` shows X on the world map in the recording):
  +0x18 i32 its size in hundredths (27.00 for the HUD's "Left"; unit not read), +0x1c its colour
  as bytes R, G, B, A, i.e. 0xAABBGGRR (0: the widget's own; the HUD's "Left" (184, 146, 69),
  tan, as recorded; the panels' ink (58, 26, 8), the `BR` text colour), +0x20 a pointer to its UTF-8 string (+0x14 is 0 or a
  number such as 19213, unread). Fixed labels are the string itself, with `<IMnn>` button
  icons ("<IM24> Search Equip"); texts the game fills in hold filler kana ("あいうえお…", as
  long as the room it has) (`layout::texts`, `Text::filled_in`). The node `kind` values seen:
  0 (2,623), 21, 28, 4, 22, 36, 15, 19, 29, 7, ... (widget, not read).
- **Screens found** by drawing the top layouts (`barrel layout-png`): 625 the field HUD (top
  left, layout 605: "Left" and the assignment's days "0/0", the year disc, the time-of-day
  dial, the "Free Time" tab, the collection badges; bottom left, 620: the button list "<IM24>
  Search Equip", "<IM19> Requests", "<IM11> Assignment", "<IM15> Leave Map", "<IM29> Running
  OFF", "<IM20> Message Log"; at the right, 618 ×5: the gathered-item notices; it also nests the request poster and other pop-ups), 958 the title
  menu on `a15_title.g1t` and 1056 the same pieces on `a15_title64.g1t` (logo part 0x15c cut 0
  at (310, 30); "Press any button" 0x15c cut 1 at (570, 660); the entries New Game, Load Game,
  Dressing Room, Extras, Options, Exit Game, parts 0x15d, 0x15e, 0x162, 0x15f, 0x160, 0x161
  from y 538 every 78, cut 0 plain and cut 1 lit; the cursor, layout 1057: bar 0x15c cut 2,
  ornaments part 0x163 cuts 0 and 4 at (-60, -16) and (642, -16); 958's own entries start at
  616, the last ending past the screen's foot). The title's backgrounds are stills 9 to 11
  (`Ipu/title_bg01` to `03`), 1015 and 1023 the hero choice (on `a15_selection.g1t` over the menu
  background, layout 357: the figures, layouts 1021 and 1022; the profile cards, Escha's 1024 and
  Logy's 1025, part 0x141 cut 0 frame, cut 1 or 2 the profile, cut 3 a "Cleared" stamp; the
  arrows, parts 0x142 and 0x143; the Yes and No tags, 1013 and 1014; "Select your character.";
  1023 has the cards side by side, Logy's at the left, 1015 the figures alone with the tags), 1035 the save list (`a15_saveload.g1t`: four slot plates,
  layout 1040 every 200 from y 200, the lit plate part 0x152 and the plain 0x153; the stamp part
  0x151, cut 0 "NEW DATA", 1 Escha's and 2 Logy's "DATA" picture; the slot texts, layout 1042:
  "New SaveData" or "Game Data 999", the date line and "9999/99/99 00:00:00"; the heading part
  0x157, cut 0 "Save", 1 "Load", 2 "Delete"; the details panel 1044 with "Play Time",
  "Protagoist" (sic), "LV", "Alchemy LV", "Money" and eleven party rows, layout 1046), 916 the battle result, 920 party info, 926 field info.

A sprite node of kind 2 is drawn mirrored left to right (25 nodes: the right halves of
symmetric pieces, as the hero choice's bottom parchment, layout 695, one half at [750, 0] as
stored and at [0, 0] mirrored, and its right-hand rope, layout 1017 at x 1284, the left one,
1016, mirrored). Kind 7 (the hero choice's Yes tag) is likely a tilt: the game draws that tag
turned a few degrees. Some pictures are stored at half alpha (at most 128: the hero choice's
grey figure sketches, cut 0 of parts 0x138 and 0x139, whose cut 1 is the same in colour at
full alpha). The desk under a screen is chosen at run time: the hero choice's layout names
part 0 cut 0 with the overlay 357 (cuts 4 and 5), but the game shows part 0 cut 1 there. A mirrored node's word 5 (the
word after the kind) is the axis: 0 left to right, 1 top to bottom, 2 both. Layout 649 draws
one corner of a dark red vignette (a15_common #25, 964 × 548) four times with 0, 0, 1 and 2,
and 650 a diamond border the same way; together they frame the world and area maps. The
maps' button lists are layout 636 (Info, Assignment, System at the left; Requests, Message Log
at the right), nested in no other layout: the game places it at run time, at (6, 854) by the
recording, and words the first row by the map, `<IM04> Area Info` on the world map and
`<IM02> Return` on an area map (strings side by side in the executable); with the world map's cursor on the town (Colseit) there is no first row: the left plate (part 0x8f cut 0, three 64-pixel rows in one) shows from its second row down, Assignment and System where they always stand (colseit-arrival 1:25). Above it the maps
show the field HUD's date panel (625's branch 605) without the time dial.

The main menu opened in play is layouts 383 (the desk background, `a15_menu.g1t` picture 0),
361 (the lead's figure: nested 957 on `a15_menu_chara01.g1t`) and 374 (placed at [0, 90]: a dark
strip, eight command buttons, layout 373, from [36, 184] stepping about 81 down and 14 right,
and five Option rows, layout 378, from [138, 340] stepping 56 to 58 down and 12 right). A
command button: plate part 0x1d (cut 0 green, 1 yellow; cuts 2 and 3 are the Option rows' white
and tan plates), name part 0x2f and icon part 0x2b on `a15_common.g1t` picture 2 (cuts 0 to 9 in
gold, 10 to 19 the same in white: Container, Basket, Adventure Equip., Battle Equip., Formation,
Status, Assignment, Request, Journal, Option), a glowing frame part 0x1e (cuts 0 to 4: 0, 1, 2,
1, 0, a pulse; the Option rows' is part 0x1f) and a "NEW" badge part 0x4a. The Option rows' texts
are filled in at run time; `SysInfo.xml` describes five (save, load, change the music, volume and
display, quit to the title), and the executable's strings at 0x140794000 name Save, Load, Erase
Save Data, Rest, Change BGM, Dressing Room, Options, Return to Title. Layout 365 is the same
figure with six party plates (layout 362, from [78, 152] stepping 136 down and 45 right) and the
list arrows (364).

The Status page is layout 400: the open notebook, layout 399 (at [62, 128]), holds on its right
page layout 431: the name (a kind 22 text, size 45), the labels "LV", "Next", "HP", "MP",
"ATK", "DEF", "SPD", four "RES" rows with their element icons (part 0x67 cuts 0 to 3) and
"Friend", and fourteen numbers in that order (HP and MP as current / max), each a node of kind
28 holding digit sprites (part 0x4e on `a15_common.g1t` picture 7: cut = digit, 20 pixels wide;
cut 12 the slash), and the bookmark with the figure (layout 393, kind 7). Its other branches:
398 the equipment effects list, 388 the skill list, 387 a skill's details.

The item grid is layout 456 (placed at [100, 252]; the Container screen, layout 458, shows it
twice, at x 44 and 480, under the tab layout 457 with "Name" between the LB / RB marks): 25 cells
in five rows of five, 98 to 104 pixels apart. The plain cell is layout 451: the lit frame (part
0x17 cut 5 at [-16, -14]), a strip (part 0x18, kind 17), the item's picture (part 0xd7, kind 4,
at [18, 16]) and a number (part 0x54 on `a15_common.g1t` picture 7, kind 28, at [84, 52]). The
first column of rows 2 to 5 holds layouts 452 to 455 instead: the same pieces with wider frames
(240 to 546 pixels), likely the cursor spanning several cells. An item's picture is its
`itemData.xml` `ImageNo` as the picture index in `WinXls/a15_itempic.g1t` (528 pictures of
96 × 96; the layouts' placeholder is 519). The detail panels for one item (picture, name,
category, traits) are layouts 465, 467 and 500 to 505.

The Assignment page is layout 430: the book, layout 428 (placed at [114, 150]: the Tasks and
Bonus tabs, layouts 410 and 411, each label there twice, dark and plain), the card, layout 425
(25 squares, layout 419 each, at x 72 to 606 and y 72 to 592: columns A to E, rows 1 to 5, as
`SubjectData.xml`'s `Pos`; the centre emblem part 0xf cut 12; the lines overlay layout 424),
the Bonus page, layout 426 (six rows "Bonus1" … "Line"), the details panel, layout 418 (at
[836, 0]: the name, a kind 22 text, the icon part 0xe, the description where the layout has
"By pressing <IM20>here…", "Reward" with points, completion stars), the past cards list, layout
429, and the rank box, layout 527. A square: plate part 0xf cut 6, the task icon part 0xe (cut 5
"?", 6 a sword, 8 a basket, 10 a pot; 7, 9, 11 the same in orange; 12 to 15 the same in white),
a glow (part 0xd, kind 36), stamps (layouts 949 "OK" and 704) and counters (parts 0x56 and
0xaf, kind 28). The tasks' `Type`s: `SUBJECT_CATEGORY_BTL` (118), `ADV` (169), `MIX` (110),
`OTHER` (60, the centre square's main assignment, several rows sharing it).

The narration cards (`u55`, `docs/events.md`) are layouts 960 to 1010: the opening's are
1536 × 72 strips of the telop sheets, one per line, at x 180 around y 462.

**The Options screen** (main menu → Option → Options) is layout 511: the desk (part 0x0,
`a15_menu.g1t` #0), the title bar (0x30 cut 1 with heading 0x2e cut 14 and icon 0x2c cut 14)
and the page, part 0x198, which is layout 408 placed at (584, 150): seven volume rows (BGM,
SE, Env. SE, Sys. SE, Voice, B. Voice, Movie; their names at x 50 every 74 pixels from y 70,
a bar 0x3c behind each, the slider 0x197 at x 304), two two-way rows ("Fast Forward Event":
Read / All, "Voice Language": English / Japanese, each choice in a 0x3c box at x 300 and 500,
its text twice: dark `1a285a` chosen, grey `787881` not), the rules 0x28 between rows, the
cursor 0x32 at x -72, and "Default" / "Confirm" (0x3d buttons at y 746 and 800). 511 also
holds the hero's outfit choice (texts "Escha", "Logy", "Ok"; layout 512). Barrel's graphics
page (`docs/renderer.md`, "Graphics settings") is to be drawn with these same pieces.

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
`dxt3` (BC2: `bc::decode_bc2`) and 6 `ui8x4` (uncompressed, four bytes a texel in DDS order: blue, green, red, alpha; the
alpha byte is 255 throughout the opaque ones and `waves2`'s normal map has its 248-average Z
in the first byte); every one decodes to its size, and Ayesha's body atlas and eyes come out as drawn.

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

**The battle HUD** is layout 886 on `a15_battle.g1t` (the battle's layouts are 854 to 929):
the turn order along the top (layout 864: fifteen face frames and "Action"), the party's
plates (layout 878: three of layout 870 at x 424, 798 and 1168, y 796; the reserve row, three
of layout 868, at y 966), the battle cries' speech bubbles over each (layout 882, kind 22
"…!!" texts), the action banner (885), the Support gauge with "Fast Forward(×1.5)" (884) and
layout 877 (glints over the party plates' bars, at y 916 to 952: not the target panel, as
once thought). The target plate is layout 920 on its own: the frame (part 0x102 cut 3, 336 × 96),
the red HP bar (cut 6, 246 × 20 at [40, 48]), the name (kind 22 at [160, 6]), the state icon
rows (nested 917 to 919: 0x395 to 0x397) and a strip under them (cut 4). The game shows it
while a member picks a target (the ruins-entrance-battle-stats recording at 322 and 354 s): its
top at y 186 on the 1080 screen for every target, its left edge near the target's place on
screen (8 to the left, within about 20). How the game places it is not read yet. A plate (870): its frame (part 0xee cut
5), the portrait (layout 869: part 0xf2, cut = the member's number, 0 Escha to 10 Nio, under a
ring, part 0xee cut 4), the HP bar (part 0xee cut 11, 124 × 16 at [126, 54]) and the MP bar
(cut 10 at [126, 104]), numbers (parts 0xde and 0xdf, kind 28) and state icons (layouts 873,
875). Layout 910 is the party's formation screen (six plates, layout 905).

**The battle result** is layout 904 (not 916, which is the "Field Info" cards): the desk (901,
`a15_menu.g1t`), the "Result" heading (part 0x37c, kind 7) and the party list (898): six rows
(part 0x37f at y 128 + 124 × row, each a portrait plate with "Lv" and "Next" at x 244, the
member's numbers and a "Skill Get!" mark), then the battle's totals, "EXP" (part 0x37d at y 868)
and "Cole" (the money, part 0x37e at y 956, with a coin). Layout 900 is one member's level-up
panel over it (a row, "HP" to "SPD" with old and new numbers, layout 899: two number columns at
x 356 and 476, part 0x4c digits; a skill name and "learned!", part 0x37b); 903 is the confirm
button (`<IM00>`). A plain render (`barrel layout-png ROOT OUT 904`) shows every state at once.
The running game (recording `ruins-entrance-battle-result-60hz.mkv`, 260 to 263 s) shows it
without the desk: over the battle scene, the lead in a victory pose at the right, the "Result"
heading and one row per party member in the battle (five there: portrait, "Lv 12", "Next 72";
"Level Up!" in red beside a member that went up), then "EXP 39" and "Cole 13" with the coin.
Its drops follow on a screen of their own over the desk: the item grid (the battle's items,
each with a check mark), the chosen item's card ("Slag Antenna": Alchemy LV, EFF bar, Stats,
the four element icons, Trait, Effect, Property, "1/1") and "Possessed" (basket 6, container
0), with an "OK" under the grid. The "Possessed" box is layout 706 (part 0x61 parchment, the
basket's and the container's icons, part 0x2a cuts 13 and 12, each with a part 0x4c number);
no layout holds it, nor a grid of the drops screen's shape (five by six, a scroll bar, no "Use"
headings; layouts 404, 445, 685, 791 and 815 checked), so the screen is put together at run
time.

**The equipment screen** is layout 404 (`a15_menu.g1t`; not seen in the running game): the
Status notebook (399, with 431 inside, as layout 400 has it), the slot panel (403: a heading,
401, four slot lines, part 0x28, and four rows, layout 390 each, at y 206, 262, 320 and 376:
an item frame, part 0xb cut 4, a name bar, part 0x3d, and the name at x 206), the new values
beside the notebook (406 / 405: nine rows, HP and MP at y 354 and 398, ATK, DEF and SPD at 480,
530 and 578, the four RES from 706 to 846; each an arrow, part 0x68 on `a15_common.g1t`
picture 5, cut 0 a red up arrow, cut 2 a blue down one, cut 4 blank, and a part 0x4e number
ending at x 1678), the basket grid (568) and the item card (752).

**The shop screen** is layout 664 (its pieces fit a shop throughout; not seen in the running
game). Its branches: 371 the title bar (a text at 570, 40), 372 the help bar (a kind 23 text
ending at 1856, 1012), 370 the money box (coin, a part 0x4d number ending at 628, 168), 662 the
product list (657: the panel, its heading sprite "Product Name / Inventory / Possession /
Price", eleven ruled rows from y 354, 52 apart; 661: one row at y 318, the lit bar part 0x94,
its dark backing part 0x95 cut 2, the "New" tag part 0x95 cut 4, the name at 116, 340, the
stock and the count held as part 0x4e numbers ending at x 418 and 492, the price as a part
0x50 number ending at 602 beside the coin, part 0x116 cut 8), 659 the Price / Total panel
(two part 0x4d numbers ending at 592, at y 864 and 918), 568 the selling grid, 752 the item
card, 446 the party's "E" marks (who can equip it), 660 the Amount dialog and 530 a notice at
the top left. Before this was read it was described as an item
grid with "Empty" slots for the basket and the container, the chosen item's card (its name,
"Alchemy LV", "Equip. Cost", "EFF", "Stats", "Effect", "Item Info" with trait and effect lines),
twelve slots of part 0x1be at the right (x 1200 and 1340, every 110 or so from y 280) and the
amount dialog, layout 660 ("Amount" with two arrows and a count, part 0xcd digits; "Total:" and
"Pay:" each with a number, part 0x4d digits, and the coin, part 0x101 cut 4). The four shops'
wares are in `ShopData.xml` (`SHOP_NIO`, `SHOP_NOTIONS`, `SHOP_LIQUOR`, `SHOP_HOM`, 50 rows
each, gated by `EvStart` / `EvEnd`).

**The synthesis screens** are layouts 720 to 851 on `a15_mix.g1t`; the largest stand alone
(no layout nests them): 753 (the item's card: picture, "Stats", "Effect", "Property"), 774
(the recipe and ingredient pick: "Recipe" tabs, "Use" columns, the ingredient list), 789 (the
ingredient grid, 25 cells, with the effect lines' bars by element), 815 (the cauldron: seven
skill rows with SP, "Information", the element bars), 821/822 and 830/831 (more of the
cauldron and result). They show every state at once in a plain render (all branches placed).


## Ryza DX: the graphics presets (`saves/graphicssetting/*.xml`)

The DX release's graphics options are tables of presets, one element per `Type`, `Field`
`"Default"` (a field may have its own: `FIELDMAP_S21_L_VALLEY_01` draws everything to 10000).
`generalpreset.xml` maps the overall quality to each table's type:

| General | AA | Draw range | LOD | Shadow dist | Grass range / quality | Others |
|---|---|---|---|---|---|---|
| High | Taa | High | High | High | Middle / Middle | bloom, light shafts, SSAO on; DOF Middle; point lights Middle; RLR High; water High; anisotropic |
| Middle | Fxaa | Middle | Middle | Middle | Low / Low | light shafts and DOF off; point lights Low |
| Low | Off | Low | Low | Low | VeryLow / VeryLow | bloom, SSAO off; linear filtering |

- `disprangepreset.xml` (units from the eye): High ObjNear 8000, ObjMiddle 20000, ObjFar 40000,
  Chara / Enemy / Gimmick / Effect 10000; Middle 7000 / 15000 / 30000 / 7000; Low 5000 / 10000 /
  20000 / 5000. The field's own `cameraparam` `disp_range` (in metres: `obj_middle` 200.0) is
  the same High range, a hundredth of it.
- `loddistpreset.xml`: `Lod_hi_dist` / `Lod_mid_dist` High 10000 / 30000, Middle 8000 / 25000,
  Low 6000 / 20000: a model's `_hi` up to the first, `_mid` to the second, `_low` past it. The
  field's `cameraparam` `lod` (`use_lod` false on PC, `hi_mid` 400 and `mid_low` 800 m; its
  `lod_NX` on and 60 / 80) is the Switch version's.
- `shadowdistpreset.xml`: `Distance` 1, 0.7, 0.5 (a scale of the shadow cascades' reach).
- `grassqualitypreset.xml` / `grassrangepreset.xml`: see `_grass.bin` above.
- Read by `barrel_ryza::presets::Graphics::read`: the general preset's `GrassQualityType`,
  `GrassRangeType` and `LODDistType`, then those tables' `Field="Default"` rows.
- `texturepreset.xml`: `TextureMipBias` High 0, Middle 1, Low 2 (Barrel leaves out that many
  of each texture's largest levels).
- `shadowpreset.xml`: the three cascades' map sizes, High 4096² each; Middle 2000 × 2400,
  2000 × 2400, 1600²; Low 1200 × 1600, 1200 × 1600, 1024².
- `pointlightpreset.xml`: `ClusterFarLimitZ` Middle 100000, Low 50000, VeryLow 10000 (how far
  point lights are gathered), `LightFadeOutRange` 2000 for all.
- `rlrpreset.xml` (screen-space reflections): `MaxRaySamples` 256 / 128 / 64, `PixelsPerStep`
  1 / 2 / 4.
- `effectpreset.xml`: `ReducedBufferLevel` High 2, Middle 1, Low 0.
- `waterqualitypreset.xml`: `EnableShoreBubble` on High only, `EnableCaustics` off on Low.
- `aapreset` (Off 0, Fxaa 1, Taa 2), `texturefilterpreset` (Linear 0, Anisotropic 1) and
  `bloompreset`, `dofpreset`, `lightshaftpreset`, `ssaopreset` (`Enable` 0 / 1): switches.

## Ryza DX: `saves/envoverride/env_override.xml`

40 rows of `Name`, `Weather`, `Time`, `SaturationScaleEx` over 20 light set names (`Dummy`,
`LGT01_DEFAULT_01` to `LGT09_TOWER_01`, `S999_CHARA_LIGHT`, `EVENT_C29`, ...); -1 for any
weather or time. `LGT01_DEFAULT_01`: 1.2 for any, then clear morning 1.4, noon 1.6, evening
1.6, night 1.5. The names are the executable's (with `LGT07_INROOM_01` first); no table found
says which a field uses. The executable also names five time zones: `TIME_ZONE_MORNING`,
`_NOON`, `_EVENING`, `_NIGHT`, `_MIDNIGHT` (and `_NONE`), against the environment's four sets.
`saves/fieldmap/sky/sky_default.xml` gives each weather and time (0 to 3) a sky tag
(`FM_SKY_MORNING_CLEAR`, `FM_SKY_DAY_RAIN`, ...) and its particle effects (rain 1754 / 1756
every 0.5 s, thunder 1755 / 1757, mist 1759, snow at scale 1 every 0.75 s).

## Ryza DX: events (`\event\...`)

As Escha's: an event is a script (`.ebd`, the commands and their named actions) and its
messages per language (`.ebm`); `barrel ebd` and `barrel ebm` read Ryza's as they are. The
scripts are only under `\event\event_jp\` (2,333, one for every language); the messages are
under each `event_<lang>` (8,415 in all). The folders are the story chapters and kinds:
`mm00` to `mm09`, `mm20` (main story), `ms01` to `ms20`, `qc*` / `qn*` (quests), `xs*`, `xf*`,
`xg*`, `xq*`, `xx*`, `zz*`, `aa01`, `am00`, `dx00` (the DX additions), `tt01`, and
`event\skill` (650). The new game's first event is `event_mm00_010` (the opening monologue
over the island, "On an ordinary island called Kurken..."); its actions are named in
Japanese as Escha's (`初期設定` set-up, `風景カメラ` landscape camera, `ライザ初期設定` Ryza's
set-up, `波エフェクト` wave effect, `歩き始め` start walking, `停止` stop).

Their command words have their own operand counts: `ebd_tables::RYZA` (`OperandTable::ryza`,
measured by `barrel ebd-measure` from the 2,333 scripts as Sophie 2's were: 299 commands, 14
with varying counts) decodes all 52,464 action bodies into 281,196 commands; Escha's table
fails on 45,701 of them. Ryza's counts are Sophie 2's for 239 of the 258 commands both use, and
Escha's for only 60 of 144: the newer games' command set. `barrel ebd-decode` takes Ryza's table
under an install holding `Atelier_Ryza_DX.exe`. The opening's commonest commands: `w1` (140),
`s40` (116), `p1` (79), `s2` (47), `s36` (41), `t25`, `s105` (34 each), `u17` (30), `u66` (29).
Ryza's executable names a word through a compiled switch rather than one Squirrel constant
each (`tools/ect_switch_names.py` walks it): it names only the macros (`p1`
`ECT_MACRO_TIME_WAIT`, `p2`, `p3`, `p5`, `p6`, `w1` `ECT_MACRO_INDEX_CHANGE`) and
`ECT_CALL_SHADOW_ACTION`; every other word comes back `(unknown)` (the names were stripped:
114 `ECT_` strings in all). Where it names one, Sophie 2's word agrees, so the table takes
Sophie 2's name for a word with the same count (156 of 299 named, in `ebd_tables::RYZA`'s
comments): the opening's `s40` `ECT_CHARA_MOTION_SET_LOOP`, `s2` `ECT_CHARA_POS_SET`, `s36`
`ECT_CHARA_MOTION_WAIT`, `s105` `ECT_CHARA_FACIAL_MOTION_SET`, `s13` `ECT_CHARA_ROT_SET`, `s17`
`ECT_CHARA_ROT_MOVE_SET_TARGET`, `t18` `ECT_CAMERA_ROT_MOVE_SET`, `q10`
`ECT_FIELDMAP_CHARA_COLLISION_FLAG`, `q5` `ECT_FIELDMAP_CHARA_ADDITION_EX`, `u44`
`ECT_EVENT_SET_POST_EFFECT`; unnamed in both: `u17`, `u66`, `u65`, `t25` (Ryza only).
