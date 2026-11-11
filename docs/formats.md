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
| 0x10009 | LOD groups, not decoded yet |

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

Every joint's parent comes before it, so world transforms compose in one pass:
position = parent position + parent rotation × local position, rotation = parent rotation ×
local rotation. Escha's skeleton (`Character/PC20/PC20_MODEL.g1m`, 345 joints) comes out
standing in its bind pose: pelvis at 88 units, knees at 51, toes near 0, head at 157.

Characters split the skeleton from the meshes. The base model (`PC20_MODEL.g1m`) holds the
skeleton and almost no geometry; the `_default` model holds the meshes and a G1MS with one
placeholder joint and the index map alone. The Oid.bin names are the scene's nodes, not the
joints one for one (helpers such as the thigh-twist nodes sit in between), so naming the
joints needs the index map; that is still to be worked out.

### Skinning

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

## Voices (`Sound/Voice/Series*.fp`)

`Data/x64/Sound/Voice` (and `Voice_EN`) hold 18 banks, `Series0000` to `Series17000`, each
a `.fp` of sound data and a `.fpl` index: u32 count (1,000), then that many
`{u32 offset, u32 size}` pairs into the `.fp`. Every bank fills 999 of its 1,000 slots, so
the voices are one list of 18,000 clips numbered from 0, bank n × 1000 holding clips n × 1000
to n × 1000 + 999. How an event line picks its clip is still open.

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
| `<AREA>.g1t`, `<AREA>_pat.g1t` | terrain textures |
| `<AREA>.g1s` | the field's shaders |
| `<AREA>.opl2` | where objects and effects stand (below) |
| `<AREA>_col.g1co` | collision |

`<AREA>_Obj.elixir.gz` holds the object models, `obj-<name>_hi.g1m`, each followed by
four `dummy` entries.

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
Fields with more layers than the two maps' six channels can weight need the block between
0x98 and the layer textures (layer settings and a tree of patches with their bounds),
which is not decoded yet.

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
combinations; effects carry `eff`. What the tags mean is still open.

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
