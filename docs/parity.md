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
and text as the diff sees them. `barrel-parity screen <capture dir> <frame> [index.tsv]` lists a captured frame's draws. `barrel-parity quads <capture dir> <frame or a-b/step> [index.tsv]` prints each frame's 2D quads unrounded with their corners, a range in one pass (for measuring an animation frame by frame).
`barrel-parity shader <capture dir> <name>` disassembles a captured shader.

The game's 2D pixel shaders scale alpha by `fAlphaRate` (folded into the compared colour) and
colour by `HdrRangeInv` (not folded: the final pass undoes it). Fully transparent rectangles
and rectangles covered by a later opaque one (the index marks fully opaque textures) are
dropped. Font glyphs become text runs, read through the game's glyph table (the index's
`glyph` lines), so text compares as strings with their box.

The game's pixel shaders take constants the vertex colour does not show (`ps_cb`, printed with
each game-side difference). Reading what they do is part of matching a screen.

A quad of one plain colour compares by its place and colour alone: the game's screen fade (a
vertex shader with `rate`, `sColor` and `eColor` over the whole screen, no texture read) and a
Barrel quad that samples a single point of a texture both read as `(plain colour)`.

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
  frame's `view_proj`; a scene draw also its box, what it is part of (`name`), its
  draw-distance class, its level of detail and the placed object it is part of (the box is
  the draw's own, though the view test uses the whole object's). The trace is off by one branch per draw.
- `barrel-parity state <capture> <a-b/step> <index.tsv> <model folder>` prints, a frame a line,
  where the camera is and where a model's skinned draws are (docs/ryza/state.md).
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
  row0 . (p, 1). `game_world` expands it to a column-major 4x4. A skinned draw's
  `mL2W` is its whole bone palette (below, not a world), so it is compared by its bones.
- Barrel's render space mirrors x. `diff3d` compares each game world matrix both as it is and
  as S M S (S = diag(-1, 1, 1)) and keeps the smaller difference, reporting how many fit the
  mirrored one.
- The game's world units differ from Barrel's by whatever the field's import applies; the
  first comparison (the title backdrop against `s01_i_attic`, because `charatest` is not in
  Ryza's archives) has different scenes and cameras, so its numbers (13 of 995 matched, world
  differences in the tens of thousands) say nothing yet. The tool is the means for the next
  step: render the same field and character as the capture and drive the count to zero.
- Matching (after the title backdrop): by index count, Barrel's textures among the game's (the
  game binds maps Barrel has no slot for; 16 x 16 placeholders are left out), nearest world
  first; a Barrel draw kept at the identity (a static object, placed in its vertices) has no
  world to compare. Barrel's `terrain` draws are included. The view-projection is compared in
  Barrel's clip space: the game's `mW2P` with its world x and clip y negated. The unmatched
  game draws are also counted by vertex shader. Frame 1400 against `--title`: 557 of 995 matched
  (before: 13, with the wrong field), view-projection within 0.03.
- A skinned draw's constant buffer (vertex shader `sha0de8a`): 16 bytes zero, `mW2P` (64), then
  the bone palette, three registers per bone (rows `m0 m1 m2 t`, world.x = row0 . (p, 1)), 32
  bones in the order of the submesh's bone map.
- A skinned draw's `mL2W` is the palette: 1536 bytes, 32 bones of three registers (the reflection
  names the array `mL2W`; a rigid draw's is 48 bytes), then `bSkin`. `diff3d` reads it, takes
  Barrel's `bones` from its trace (the model matrix times each joint's skin matrix, in the
  slots of the submesh's bone map, render space), pairs a skinned draw with the Barrel draw of the
  same key whose palette is nearest, and prints per texture folder `pose ...: N skinned draws,
  bones worst translation T, rotation R` (the largest entry difference of the 3 x 3 part). Joints
  the game moves by cloth simulation (`simulated` in the trace: no motion moves them off the bind
  pose) and submeshes Barrel draws rigid (`palette_indexed` false) are not compared.
  `DIFF3D_POSES=1` lists each draw; `=fit` adds under each the rigid motion (yaw, translation, and the
  spread of the translation over the bones) that takes Barrel's palette to the game's, which is where
  she stands and faces when the pose is the same.
- Not covered yet: shadow passes (game and Barrel both list them; `diff3d` compares colour
  passes only), instanced draws (Barrel writes `inst` 1), view and projection separately.

## Constants parity (Ryza fields)

The lighting and post-processing constants of a captured frame against what Barrel uses for the
same view, constant by constant (frame 2550 of the load-rasen capture, the gate at Rasenboden at
noon, against the view of the scoreboard row `rasen-3d-2550`).

- `BARREL_TRACE_CONSTANTS=<file>` on a Ryza field render writes one JSON object: role -> constant
  name -> floats, with `null` for a component Barrel has no counterpart for. A role is one draw of
  the game's frame (`ambient.stage`, `lighting.chara`, `sky.plane`, `fog.stage`, `specular.chara`,
  `water.first`, `meter.adapt`, `bloom.bright`, `tone`...), and the names and layouts are the
  game's constants for it (`litDir` and `litCol` as two lights of four floats, `Scat` as 16, ...).
  The dump (`crates/ryza/src/play/constants.rs`) is built where the frame is: from the look slots
  the frame feeds, from the set's readers for what Barrel keeps in another form, and from the
  frame's tone, glow and metering values. It undoes the two differences from the game's values:
  the exposure (the scene is lit at the bottom of the set's range, folded into every light colour:
  divided out) and the mirror (render space has x negated: negated back). It costs nothing when
  the variable is unset.
- `barrel-parity constants <capture> <frame> <barrel constants json> [index.tsv]` finds each
  role's draw in the capture (`ROLES` in `crates/parity/src/constants.rs`: pass and draw, the
  passes counted as `draws` counts them), reads the game's value of each named constant and
  prints a mismatch beyond a relative 1e-3 (absolute 1e-5 near zero) as `pass P draw D program
  name[i]: game X barrel Y`. A name in both shader stages' buffers is compared with the copy that
  agrees best. Then it lists what Barrel does not dump: every named constant of the lighting
  and post passes (the ambient-and-shadow pass to the last pass before the interface, and of the
  forward pass only the specular ambient's two draws), minus the camera's matrices and the 2D
  passes' viewport plumbing, one line a pass with identical draws once. It ends with `N constants
  compared, M differ, K of the game's lighting/post constants not dumped (names)`, K counting
  (pass, name) pairs.
- Left out of the comparison as time-dependent: the cloud pattern's offsets (`mW2Pt` 12 and 13) and the sky
  strip's layer offsets (`vTexOf` 4 to 7), which are the wind's travel since the field came up.
- The board row is `rasen-constants` in the local checks file: it passes while the differ count is
  at or below the number reached. The not-dumped list is the map of what Barrel has no
  counterpart for (ambient occlusion, reflections, height fog, light shafts, the star filter,
  the depth of field, the temporal anti-aliasing, the shadow cascades, the water's own
  light).

## The pixel board (Ryza fields)

The game's render targets for the captured frame (frame 2550 of the load-rasen capture, the
gate at Rasenboden at noon) against Barrel's own at the stages they share. The game's pixels
come from the reference run's pixel mirror (ryza-recomp's `docs/DESIGN.md`, "Pixel mirror":
`RYZA_PIXELS=<dir>` with the capture's frames, one folder `f<frame>` of `<first draw>-<last
draw>_<target>` files, `.f32` with a `.txt` beside it or `.png`); the folder is read only here.

- A **view file** (`docs/ryza/views/<name>.txt`: numbers and names only, no paths) says which
  frame the board reads: per stage the game's dump file stem (`stage tone 2078-2078_rt1920x1080.90#7`),
  `masks` (the depth and stencil before the sky), `normal` (the G-buffer's normal target), `strip` (the two
  targets round the cloud strip, when the dump has them), `view` (the capture's `mWV` rotation, nine
  numbers, for the `normal` line) and the `region` lines (name and a box in the 1920 by 1080 picture).
  A stage the file does not name is skipped. The game's draw numbers differ between frames (the dump names
  follow the capture's draws), so each view names its own; the folder of dumps is the first
  argument after the view. Two views: `gate.txt` (frame 2550 of the load-rasen capture) and
  `turn.txt` (frame 2150 of the pixels-turn capture: the camera turned toward the windmill tower and
  the house, her a few steps on; regions: the tower's sunlit and shaded sides, the crates, the rock,
  grass, path, the house's white and blue wall, the planter, the left tree).
- The camera of a view comes from the capture's `mW2P` and `mWV` (`barrel-parity draws --vars mW2P,mWV`
  with `PARITY_FULL=1`): the eye from `barrel-parity state`, the look-at the eye less 1000 times
  `mWV`'s third row, the pitch `asin` of that row's y, the yaw 360 less its `atan2(x, z)` in degrees,
  and the vertical field of view from `mW2P`'s y scale over the pitch's cosine (2.41421: 45.0
  degrees at 16:9, for both views; the board's first rows used 45.4, 1% too wide). Compared with
  `view_proj` in Barrel's trace (`BARREL_TRACE_DRAWS`, the game's matrix with x and y flipped) the
  clip matrix equals the game's to 5 digits, so the game adds no jitter at these frames. The sails'
  and the sky's clock is the same number (`BARREL_SKY_CLOCK`).
- The turn view's run (row `turn-pixels` in the local checks file): the clock constants read from the
  capture's draws as for the gate (`PARITY_FULL=1 barrel-parity draws <draws> 2150 <index> --vars
  vTexOf,vThreshold,mW2Pt`): the strip's `vTexOf` offset 0.0198282 over 0.0028125 a second gives
  `BARREL_SKY_CLOCK=7.0500` (the second layer's -0.44062734 over 0.0625 gives 7.0500), `mW2Pt`'s
  translation `BARREL_PATTERN_PHASE=-0.10466671` and `vThreshold` `BARREL_CLOUD_THRESHOLD=0.64210135`.
  Her place is read by `DIFF3D_POSES=fit DIFF3D_FIT_AT=<her x,y,z> barrel-parity diff3d` (a trace of
  Barrel's draws of the same view): per skinned draw the rigid motion that takes Barrel's palette to the
  game's, as her facing's difference (yaw) and how far her point moves, so iterating
  `--ryza` and `--turn` until the yaw reads 0 and the move under a unit places her from the capture's
  matrices, not from a bone's mean, and `--motion-time` (her idle's phase) until the bones'
  worst translation is at its least: the gate `--ryza 32520,6180,30334.02 --turn -1.0 --motion-time 1.15`
  (bones within 0.14 units), the turn view `--ryza 32200.55,6193.3,30166.94 --turn -126.33
  --motion-time 2.2` (0.19). The height comes from the terrain under her, read on its cells'
  triangles as the game does (`Heightmap::at_faces`: the fit had her 1.36 units low in the turn
  view on the bilinear height, and 0.02 on the triangles).
- `tools/pixels.sh gate|turn|left` renders one view of the board with its clocks, camera and
  her place and scores it (the views' files are `docs/ryza/views/`; usage in its header): the
  three views are the gate, the windmill past the house (the turn view) and the cliffs and
  tall grass with the river far off (the left view).
- `BARREL_DUMP_STAGES=<dir>` on a Ryza field render (offscreen) writes Barrel's targets in the
  same layout (`<name>.f32`, `<name>.txt`: `width height channels format`, RGBA floats, rows
  top to bottom) and `tone.png`, in the game's units: the scene is stored at half
  (`HDR_RANGE_INV`) and lit at the bottom of the set's range, so a dumped value is the stored
  one times 2 over that exposure (0.9 at noon), the glow's targets times 2. The frame is drawn
  again up to each stage (`render::dump::Stop`) and the scene copied out: `lit` (the terrain,
  the models and the water: the game's lit scene before the sky, but with the scene fog and the
  specular ambient in each fragment, which the game adds in its fog and forward passes),
  `dome` (and the sky plane and dome, without the cloud strip), `sky` (and the strip: the game's
  sky stage, and with Barrel's fog already in, its stage after the fog), `scene` (the finished
  HDR scene, with the outlines), `cloud_map` (the strip's 1024 by 1024 map, 0 to 1), `bright`
  (the bloom's bright pass at a quarter) and `bloom` (its result at an eighth). Barrel has no
  G-buffer, ambient pass or lighting pass of its own, so those have no dump; the occlusion has
  one (`ao`, the pass's 8-bit result at half size, 0 to 1, in all four channels). Nothing is
  made while the variable is unset.
- A `shadow` stage (`Stop::Shadow`, `look` slot 31's w) draws the terrain and the models as the
  sun's share per pixel (the cascades' light times the cloud pattern, the same meaning as the
  alpha of the game's ambient-and-shadow pass, `1892-1894_rt1920x1080.9#15`), paired by
  `barrel-parity pixels` with that alpha. The stages marked in `pixels.rs` print the mean of
  eleven regions of the picture (canopy, trunk, grass, sand, far hills, the dark canopy, the stone
  wall, the shaded path, and `ryza shadow` (880, 880, 1250, 1060): her shadow on the sand), the game's and Barrel's,
  over the pixels the game drew before the sky.
- A `depth` stage (the occlusion's normal and depth target at half size: the view depth in
  the fourth channel, a large clear value where nothing is drawn, the distant view's solid pieces
  included) gives the coverage line, printed after the stages: `coverage: game sky X%, barrel sky
  Y%, barrel-only Z%, game-only W%` (the game's from its depth at the far plane, a half-size pixel
  the game's when two of its four are) and the two crown cells of the 6 by 4 grid of 320 by 270
  pixels, `cell (2,0)` and `cell (3,0)`: their Barrel-only and game-only shares.
- A `chara` stage (`Stop::Chara`, `look` slot 31's w 2) draws the terrain and the models as
  coverage: 1 where a draw of the character path (flag 128) lands, 0 elsewhere, the water
  skipped. `barrel-parity pixels` takes the connected pieces of it that touch the game's
  characters (stencil 8) as Barrel's own character pixels (a field's gimmicks are drawn as
  the stage's models, as the game lights them, so they are not in it), and under the `lit`,
  `scene` and `tone` stages prints a `chara <stage>` line: the mean colour of the game's
  characters and of Barrel's, each over its own pixels (with the pixel counts), the mean
  luminance's difference in percent, the mean absolute difference between the two sorted
  luminance distributions (the sorted lists resampled to the shorter one's length, so the pose
  does not count: in the stage's units, and over the game's mean luminance as `rel`) and the
  same for the saturation `(max - min) / max`. The `lit` stage has Barrel's fog in it and the
  game's not (the game adds it before the sky, with the forward draws), so the `scene` line is
  the one to read. At the gate: mean colour within 0.6% (0.4109 0.3126 0.2245 against 0.4134
  0.3126 0.2230), the luminance distributions 0.0091 apart (2.8% of the mean; 0.0112 before her
  forward passes, 0.0164 before the character lighting followed the program, docs/shading.md),
  the saturation distributions 0.0067 (0.0354), the tone stage's 2.43 of 255 (3.14, 3.53). Two
  lines under it, `chara <stage> boots` and `thighs`, give the mean colour of two bands of her
  body, each in its own picture's coordinates (the largest piece of the character mask: the
  lowest 15% of its height, and 62% to 80% of it): the game's and Barrel's (boots 0.0415 0.0167
  0.0145 against 0.0492 0.0193 0.0158, thighs 0.0794 0.0319 0.0281 against 0.0936 0.0369
  0.0316: the sheen is a fifth too bright, the screen-space reflections left out).
  The picture's tone error went from 15.85 to 15.87 with them: her lines and sheen are now where
  the game's are but her pose differs by 10 to 15 pixels, so each line is a pair of errors.
  The direct specular of her lighting pass (the same highlight rule as the field models') was
  tried on top and left out: it moved the mean the wrong way (0.4147 against 0.4134, the tone
  distribution 2.65 against 2.43) while the specular ambient is still too strong.
- The `board` region (0, 522, 52, 556) is the notice board at the gate's left, a field gimmick
  (a stage model: the game draws it in the G-buffer pass with stencil 2 and the field models'
  program, and into the cascades), printed under the `lit`, `scene` and `tone` stages like the
  others. Drawn through the character path (before) the `lit` mean was 0.323 0.150 0.216
  against the game's 0.245 0.136 0.240; drawn as the stage's models 0.263 0.161 0.300 (0.221
  0.101 0.251 without the specular ambient, which comes out about twice the game's here, as it
  does on the stage's models: the screen-space reflections are left out). The tone stage's
  0.486 0.314 0.430 became 0.418 0.335 0.522 against the game's 0.399 0.277 0.399 (the water
  beside it and the glow's bleed into the blue).
- The clouds' and the cloud shadows' phases are the field's clock, which the game's run does
  not repeat: `BARREL_SKY_CLOCK=<seconds>` (the sky's clock: the strip's wind, the water),
  `BARREL_DOME_CLOCK=<seconds>` (how long the dome's layers have slid: the first layer's
  `txTrs01.x` over 0.002, `28.40` at the gate's frame and `21.73` at the turn view's; the
  dome's clock starts some 14.7 seconds before the sky's in these runs),
  `BARREL_PATTERN_PHASE=<offset>` (the cloud shadows' pattern, both axes, in repeats) and
  `BARREL_CLOUD_THRESHOLD=<t>` (the cloud map's threshold; the sharpness follows as `3 / (1 -
  t)`) give them the capture's values. Frame 2550: `13.7166` (the strip's `vTexOf` offset
  0.03857833 over its 0.0028125 a second; the second layer's `vTexOf` offsets, 1000 and 500
  over the grid a second, give the same 13.7), `-0.07133281` (`mW2Pt`'s translation) and `0.6538959` (`vThreshold`). The
  game's cloud map also comes from a noise lattice it shuffles each run, so where its clouds
  stand is not reproducible by Barrel's own passes (the share of the sky error that is
  placement is in `docs/ryza/open.md`).
- `BARREL_CLOUD_MAP=<png>` fills the renderer's cloud map (1024 by 1024, 8 bits a channel) from
  the game's own map (`1911-1911_rt1024x1024.27#4.png` in the dump folder) in place of the
  density and map passes (`Maps::load_map`), so the clouds stand where the game's do and the
  `cloud_map` stage reads 0. The strip's wind still comes from `BARREL_SKY_CLOCK`. The cloud
  shadows on the ground are not from this map (they are the shadow pass's own pattern,
  `BARREL_PATTERN_PHASE`), so the `shadow` stage does not move with it.
- Three stages are for the sky's draws one at a time: `plane` (the game's `1912`), `dome1` (`1913`, the first dome layer) and `dome` (`1914`, after both layers)
  against Barrel's stages of the same names, and after the stage lines `strip`, the cloud strip's own
  contribution over the sky pixels (the game's `1915` less `1914`, Barrel's `sky` less `dome`):
  the mean abs of the two contributions and, by bands of 60 rows, each one's mean light, over the
  pixels that are sky in both pictures.
- A `normal` line after the coverage: the angle between Barrel's mapped normals (the `depth`
  target's xyz, half size, render space) and the game's normal target (`9-911_rt1920x1080.9#12`:
  the view-space normal as an octahedral pair, turned back to the world by the capture's `mWV`
  and mirrored in x), at each half-size pixel's top-left full-size pixel, over the stage's pixels
  (stencil 2 before the sky, drawn in both): the median and mean in degrees and the same in
  each region of the list above (halved); `normal.png` shows the angle, 0 to 45 degrees black to
  white. Two samples of a normal map half a pixel apart already differ by 4 to 5 degrees, so the
  median does not go below that.
- `barrel-parity pixels <view file> <game dump dir> <barrel dump dir> [<picture dir>]` pairs the stages
  (`cloud_map`, `ao` (the game's `914-914_rt960x540.27#2`: the occlusion, half size, with the
  regions' means taken over the half size picture), `shadow`, `lit`, `lit_fogged` (Barrel's `sky` against the game's stage after both fogs),
  `sky`, `scene`, `bright` (the game's 482 by 272 cropped to Barrel's 480 by 270), `bloom`,
  `tone`) and prints one line each: `stage: mean abs X, rel Y, worst region (c,r) Z`, `X` the
  mean absolute difference over the three colour channels (in the game's units, in 0 to 255
  for the tone pass), `Y` that over the game's own light (`sum |g - b| / sum g`), the worst of
  an 8 by 6 grid of regions with its mean absolute error; below it, for the 1920 by 1080
  stages, the same for the masks from the game's depth and stencil before the sky (`sky`: depth
  at the far plane, `characters`: stencil 8, `stage`: the rest). The last line is `pixels: tone
  mean abs N`. With a picture folder it writes `<stage>.png`, the game's, Barrel's and their
  difference (red: Barrel brighter, blue: darker, four times as strong) side by side, 960 wide
  at most (HDR light shown as `x / (1 + x)` with a gamma).
- The row is `rasen-pixels` in the local checks file: it passes while the tone error is at or
  below the number reached.

## Stand-ins

A stand-in is anything the code uses that is not the game's own rule or data: a value fitted
from a capture, a placeholder, a simplification, something skipped. Each one carries a comment
line with `STAND-IN:` and one sentence saying what is stood in and what the real thing would
be, on the line above the item. `tools/standins.sh` lists every tag under `crates/` as
`file:line: text`, sorted, with a last line `N stand-ins`; a stand-in that is replaced by the
real thing loses its tag.

## Unused file bytes

`barrel-tool unread FILE [--gran N] [--budget SECONDS]` shows which byte ranges of one game
data file Barrel's parser never uses, so ignored fields are visible. The parser is chosen by the
file's magic or name (g1m, g1t, g1a, g1h, g1co, ktf, khm, swg, pssg, kwb, elixir, xml, xml.e,
ebd, oid, opl2). A range is used when inverting its bytes changes the parse result (its `Debug`
text, an error, or a panic); the tool bisects the file down to `--gran` bytes (default 4) until
`--budget` seconds (default 60) run out, and the blocks it did not get to split are reported as
"used (not refined)". Output is one `offset..end (len) bytes` line per unused range, then
`FILE: N bytes, U unused in R ranges (P%), parser X`. It also knows Ryza DX's own field files
(`_env.bin`, `_light.bin`, `_grass.bin`, `_coll.bin`, `.koc`). `tools/unread-survey.sh DATA_DIR
WORK_DIR` runs it over a sample of a game's archives and prints a table by kind of file.
A parser that allocates what a mutated
length field asks for can exhaust memory; run it under the memory cap. The elixir container's
entries are listed, not parsed, so their bytes are reported unused.


## Event commands

The answer key logs the commands the game's queues run, each with the frames it ran over
(ryza-recomp: a scenario with `HOOKS=scenarios/hooks/commands.hooks`, then `tools/cmdlog.py
--hex hooks.log`). `barrel-tool ryza-commands INSTALL EVENT` prints the commands Barrel's
player takes from the same event, as `frame aACTION word operands...` in hex, frames at 60 a
second from the event's start. `tools/cmddiff.py GAME BARREL --from FRAME` pairs the two in
order within each group of commands (the word's letter) and prints, per group, the counts, the
commands only one side has, and the pairs whose frames are off the commonest offset. An event
is at parity when every command pairs and none is off the offset.
