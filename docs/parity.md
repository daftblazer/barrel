# Parity checks

Barrel is checked against what the original game draws and does, as recorded by a recomp
of the game. For Escha that is `../escha-recomp`, for Ryza `../ryza-recomp`: both run headless
with a null Direct3D 11 and the same draw capture (Ryza's variables are `RYZA_*`, its scenarios
run on a 1920 × 1080 headless display). A diff of the two says where Barrel differs, without
pixels and without judging by eye.

## Draw parity (2D)

1. Capture the game. escha-recomp's `ESCHA_TRACE_DRAWS=dir` (see its docs/DESIGN.md, "Draw
   capture") writes every draw with its state, textures named by content hash. Run at 60 fps,
   bounded with `ESCHA_FRAMES`, optionally scripted with `ESCHA_PAD` / `ESCHA_KEYS`.
2. Name the textures. `barrel-parity textures <game folder> <index.tsv>` hashes every g1t
   texture in the game's files (loose, in elixir containers or in Ryza's PAK archives) the same
   way, so a captured texture reads as `Data/x64/WinXls_EN/a15_title.g1t#0` (Escha) or
   `\data\x64\res_en\ui\a21_title.g1t#0` (Ryza: the archives' own names, lower case).
   Ryza's index goes to `../ryza-recomp/work/traces/textures.tsv` (`PARITY_INDEX` for
   tools/parity.sh).
3. Trace Barrel. `BARREL_TRACE_DRAWS=file` makes every 2D layer (`barrel_render::text::Layer`)
   write its quads when they change, with textures named the same way, on a 1920 × 1080 screen.
4. Diff. `barrel-parity diff <capture dir> <game frame> <barrel trace> <barrel frame>
   [index.tsv]` turns the game's 2D draws (4-vertex strips placed by their vertex shader's
   `mL2P` matrix) into rectangles. It matches them in order against Barrel's and lists each
   difference:
   - rect beyond 0.5 px;
   - uv beyond 0.001;
   - colour beyond 1.5/255;
   - what only one side draws.

   A rotated rectangle (Barrel's `Quad::spin`, traced with its `corners`) is compared by its four
   corners within 0.5 px when both sides draw it rotated; otherwise as above.

   The exit status is 0 when nothing differs. Draws that read a render target (the copy to
   the back buffer, post effects) and 3D draws are counted, not compared.

`tools/parity.sh <capture dir> <game frame> <barrel arguments...>` does steps 3 and 4 for
one offscreen Barrel frame (exit status 0 when nothing differs).

Animation phase: `barrel-parity align <trace dir> <a-b/step> <barrel trace> <a-b> [index.tsv]`
diffs every captured game frame of the first range against each Barrel frame of the second,
prints the best Barrel frame per game frame and the single offset (game frame - Barrel frame)
that minimises the total mismatches, and its count on the last line. It tells layout parity from
animation phase: a cycling sprite that matches at some frame but not at the same one is phase.
`PARITY_ALIGN=a-b/step tools/parity.sh <capture> <frame> <barrel args> --frames N` runs Barrel
once for N frames and aligns frames 0..N-1 (frames with nothing on screen are left out; a Barrel
that draws the same quads in all of them is reported). `diff` is unchanged. The 2D screens' clock starts at `BARREL_CLOCK_AT` (a tick, default 0;
docs/escha/parity.md, "Time alignment"): a plain check against game frame F uses F + 204 for the
Escha screens. A page's own animations (the picker's row frame, the Equip. pages' pulse) start when the page opens: `BARREL_PAGE_AGE=<ticks since the page opened>` is the age at the first frame (docs/escha/parity.md, "Page clocks").

`PARITY_SKIP=prefix,...` leaves textures whose game file name starts with a prefix out of the
diff on both sides, for what a check does not cover yet (`Data/x64/Effect/`: particles).

`barrel-parity quads <capture dir> <frame> [index.tsv]` lists a captured frame's 2D rectangles
and text as the diff sees them. `barrel-parity screen <capture dir> <frame> [index.tsv]` lists a captured frame's draws.
`barrel-parity shader <capture dir> <name>` disassembles a captured shader.

The game's 2D pixel shaders scale alpha by `fAlphaRate` (folded into the compared colour) and
colour by `HdrRangeInv` (not folded: the final pass undoes it). Fully transparent rectangles
and rectangles covered by a later opaque one (the index marks fully opaque textures) are
dropped. Font glyphs become text runs, read through the game's glyph table (the index's
`glyph` lines), so text compares as strings with their box.

The game's pixel shaders take constants the vertex colour does not show (`ps_cb`, printed with
each game-side difference). Reading what they do is part of matching a screen.

## Draw parity (3D)

The same capture holds the scene's mesh draws (frame 1400 of Ryza's boot-title scenario: the
title's 3D backdrop, about 1,900 draws: shadow maps, a four-target G-buffer, lighting, post).

- `barrel-parity draws <capture> <frame> [index.tsv] [--vars A,B] [--pass N] [--summary]`
  groups a frame's draws into passes (a run of draws with the same colour targets and depth
  target) and prints per draw its counts, shaders, textures (by archive name where the index
  knows them) and constant variables. Variables are read from the draw's constant-buffer blobs
  at the offsets of the shader's own reflection (RDEF, the same parser `shader` prints). By
  default the matrices (a variable of 64 or 48 bytes: the reflection keeps no type) and the
  variables named for View, Proj, W2P, L2W, Eye or Cam show; `--vars` names exactly which.
- With `BARREL_TRACE_DRAWS=file` the renderer also writes `{"frame":f,"draws3d":[...]}` per
  frame (format in `crates/render/src/trace.rs`): pass (`shadow`, `scene`, `sky`, `outline`,
  `terrain`), index count, textures by the same content hash, the world matrix, and the
  frame's `view_proj`. The trace is off by one branch per draw.
- `barrel-parity diff3d <capture> <frame> <barrel trace> <frame> [index.tsv]` takes the game's
  G-buffer draws (four or more colour targets and a depth target) and Barrel's `scene` and
  `sky` draws, and matches them by (set of texture names the index knows, index count),
  nearest world matrix first. It lists the game's draws with no match and Barrel's, grouped by
  the texture's archive folder (draws with no indexed texture group as "no named texture" and
  match on the index count alone), and ends with
  `N game draws, M Barrel draws, K matched, max world diff X`.

Conventions found in the capture (checked on the title's characters, `sha0de8a` skinned and
`sh1df3f7` rigid vertex shaders):

- `mW2P` is a float4x4 used row-vector style, clip = (world, 1) . M; the four registers are M's
  rows, with the translation in the fourth. Read as column-major floats it is the usual
  column-vector world-to-clip matrix, so it compares to Barrel's `view_proj` directly. Its z
  and w columns are nearly equal (a reversed or near-plane projection at the title's camera).
- `mL2W` is 48 bytes: three registers, each one output row `(m0 m1 m2 t)`: world.x =
  row0 . (p, 1). `game_world` expands it to a column-major 4x4. Skinned draws carry no
  `mL2W` (their buffer holds `mW2P` and a palette of the same 3-register rows, with the model
  at the identity), so they match by textures and counts only.
- Barrel's render space mirrors x. `diff3d` compares each game world matrix both as it is and
  as S M S (S = diag(-1, 1, 1)) and keeps the smaller difference, reporting how many fit the
  mirrored one.
- The game's world units differ from Barrel's by whatever the field's import applies; the
  first comparison (the title backdrop against `s01_i_attic`, because `charatest` is not in
  Ryza's archives) has different scenes and cameras, so its numbers (13 of 995 matched, world
  differences in the tens of thousands) say nothing yet. The tool is the means for the next
  step: render the same field and character as the capture and drive the count to zero.
- Not covered yet: shadow passes (game and Barrel both list them; `diff3d` compares colour
  passes only), instanced draws (Barrel writes `inst` 1), view and projection separately.
