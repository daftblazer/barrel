# Parity checks

Barrel is checked against what the original game draws and does, as recorded by a recomp
of the game. For Escha that is `../escha-recomp`, which runs headless with a null Direct3D 11.
A diff of the two says where Barrel differs, without pixels and without judging by eye.

## Draw parity (2D)

1. Capture the game. escha-recomp's `ESCHA_TRACE_DRAWS=dir` (see its docs/DESIGN.md, "Draw
   capture") writes every draw with its state, textures named by content hash. Run at 60 fps,
   bounded with `ESCHA_FRAMES`, optionally scripted with `ESCHA_PAD` / `ESCHA_KEYS`.
2. Name the textures. `barrel-parity textures <game folder> <index.tsv>` hashes every g1t
   texture in the game's files (loose or in elixir containers) the same way, so a captured
   texture reads as `Data/x64/WinXls_EN/a15_title.g1t#0`.
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
