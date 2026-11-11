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
              u8×4 0, 0, 0x10, 0x21 in every Escha texture
              u8   1 if an extended header follows
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

Every texture's data is exactly the size its format, size and mip count give, so there is
no padding or swizzling on PC. The renderer can upload BC1 and BC3 as they are;
`barrel_formats::bc` decodes them in software for tools.

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
| `<AREA>.ktf` | terrain (KTGL, magic `KFTK`), not read yet |
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
             i32 ×6, not decoded yet (they vary with portrait and expression)
             u32 length, then the UTF-8 text with its terminating 0
```

Markup in the text: `<CR>` breaks the line (35,202 uses), `<IMnn>` puts an icon inline,
`<CLGR>` and `<CLNR>` change and reset the colour.

**Mangled lines.** 169 lines in each translation (507 in all) were left in Japanese, and
their text was mangled by the tool that made the translated files: each Shift-JIS byte pair
was stored as one code point with its bytes swapped (エ, `83 47`, became U+4783), then
written as UTF-8. They are placeholders such as ダミー ("dummy").
`Message::repaired_text` turns them back into readable Japanese; the stored text is kept as
it is. The test cannot misfire on real Japanese text: kana and common kanji never pass the
byte-range check.

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
