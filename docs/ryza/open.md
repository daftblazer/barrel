# Ryza DX: open problems

Everything Barrel is stuck on for Atelier Ryza DX, has not worked out yet, or knows to be wrong
(addresses are in `Atelier_Ryza_DX.exe`, which is not DRM-wrapped). Same rules as
`docs/escha/open.md`; problems in the shared engine are in `docs/open.md`.

The original Ryza DX is never run through Steam without asking first (its playtime and
achievements are the player's own, and the DX version has not been played there).

## Most pressing (worked first; each entry stands on its own)

Ranked by what it blocks. Each says what is known, what is ruled out, where to start, and when it
is solved. ryza-recomp is ../ryza-recomp (its CLAUDE.md has the build and run notes); Barrel's
checks are `runchecks docs/ryza/checks.txt` (44 rows; frames are the exact capture's) and
`docs/escha/checks.txt` (17).

1. **The title backdrop's cast: what is left** (P1). Ryza, Lent and Tao and the three
   weapons lying by them are drawn from the event `tt01_010` (docs/renderer.md, "The
   backdrop"; docs/events.md, "Timing and loops"): diff3d at frame 1400 matches 615 of 995
   draws, all three characters' bones within 0.012 units (rows `title-3d-poses-1400`,
   `title-3d-props-1400`), the event's commands pair with the game's log. Left: the cloth
   submeshes of the title's characters hang at rest, carried by their parent joint (built from the
   model's control points, docs/formats.md "Cloth pieces"; the field's Ryza runs the game's
   cloth solver, `barrel_engine::cloth`, and the title's cast could take it the same way; the pieces of sections 0x30002 and 0x30005, in 8 of the 37
   character models with cloth, are still drawn rigid on one joint); a head turn
   (`s88`) takes its first angle only, at once (the title's `s88 0 -7 0 0` is 7 degrees about
   the model's x axis in the head's bind frame; the opening's `s88 0 0 56 -4` and the timed
   `s90` are the way to the other two); `npc34a`'s two draws have no Barrel bones to compare.
   Of the 995 game draws at frame 1135, 962 have a Barrel match (row `title-3d-1135`). The 32
   without are the terrain, triangle strips in 32 patches (the pass's first draws; 24 by
   `sh0cf28a`, 8 by `sh499eb4`): the game draws every draw record of a terrain patch (`.ktf`:
   a layer or a pair of layers each, as a strip) on its own, where Barrel draws a patch once,
   its layers blended in one pass (16 patches in view; the strips are not checked against
   the file's records index by index). The Barrel draws
   without a game match (row `title-3d-1135` counts the matched): those 16, 2 plant cells
   (below, "Grass"), 5 of the characters' (the cloth submeshes), and 46 of the field's models
   that the game leaves out because they stand behind other scenery (docs/renderer.md, "View
   culling"; how the game tests what is hidden is not read, and nothing shows of it). One
   game draw more has no match: a bush just outside the view, out of sight. The title is
   drawn without draw ranges, a stand-in: with the field's own ranges 400 game draws go
   missing, so the title's ranges are not the field camera file's.

2. **The title's sparkles** (P1). The menu's own quads match the game's on every frame checked
   (rows `menu-*`), but the game also draws particles Barrel does not: a burst at the button
   (frames P + 1 to P + 31, up to 96 quads) and a trail for about a second after each move of
   the cursor (20 to 105 quads). They come from the effect player (`Saves/ez_eff_player/title.xml`,
   effect 1663 for the burst, started in `0x1400d7640`), drawn with unnamed textures between
   the prompt and the cursor. Barrel reads no `.g1e` effect yet (docs/formats.md, "Effects"), and
   the particles are scattered by the game's random numbers. Start: effect 1663's `.g1e` and the
   effect player's update. Solved
   when frames P + 10 and a move's frame + 3 diff to 0.
3. **The title's menu: what is not read** (P1). Stand-ins in `title_menu.rs`: the cursor's
   six-frame travel and the held direction's 37 / 15 frame repeat (the list control's own
   values), the perspective's eye distance (fitted 1340 to 0.015 pixels; a 30 degree view over
   720 lines gives 1343.5), the fade's two steps ahead, and Episodes always showing
   (`0x1405a2350`, the ownership check). Not captured: confirming an item, whether a direction
   counts before the cursor shows, and the sequencer's state 0xc (60 s without a button at the
   prompt, `0x140de7824`).

## Event commands against the game's own log

The game runs every command through per-subject queues (`0x140489410` steps a runner's queue:
the event's own `u`/`w`/`p` words, characters' `s`, the camera's `t`, the `q` loads), and
ryza-recomp logs them with their frames (`scenarios/hooks/commands.hooks`,
`tools/cmdlog.py`). `barrel-tool ryza-commands INSTALL EVENT [DRAWS]` prints what Barrel's
player takes and `tools/cmddiff.py` pairs the two per group (docs/parity.md, "Event commands").
Read from the game and now Barrel's (docs/events.md, "Atelier Ryza DX's events"): waits
count down in 32-bit floats (some a frame short), the title's loop (a label, a jump, a number
drawn from the clock, a jump by it), the random wait, and an event waiting on its fades.
With the game's two draws given (`18.1,2`) the title's event pairs 66 of 69 commands and the
opening's first 132 of 135, each group at one offset. Left:

- **Loading is not timed.** The game's event runs its first commands, then loads its
  characters behind `p1 10` polls (title: 874 to 917; opening: 1601 to 1667, its clock from
  1699), and only then places them. Barrel places them at once, so its early commands sit 40
  (title) and 64 to 95 (opening) frames from the game's, a constant after that
  (`actors::EVENT_START` is fitted to it).
- **How long a fade holds the event** is fitted to three fades (`event::fade_hold`: 3, 32 and
  94 frames for 0, 0.5 and 1.5 s; the picture's own fade is 30 and 90). The game's test is
  behind `0x14050fdb0` (to `0x140509f30`), not read. Rules tried that give 93 for 1.5 s: the
  fade's frames plus a fixed 2 or 3, and a float sum against the length plus 1/30 s.
- **The prompt's slide-in starts a frame late, for a reason not read.** Frame by frame the
  game's prompt is the layout's curve a frame after its `delay` (`title::SLIDE_LATE`, rows
  `title-prompt-in-1000` and `1010` at 0, with the fade's rows `title-fade-900`, `930`, `950`).
  What makes the layout's animations start a frame after the title's first frame is not found.
- **The draws are the game's only when given.** The pick is seeded by the clock's seconds, as
  the game's; the wait's generator is the game's shared one (`0x1406ae6b0`), which Barrel
  does not follow, so a parity run gives the numbers (`BARREL_EVENT_DRAWS`).
- **`s69`** carries -4300 in its 14th operand in the game's queue where the script has 0, and
  `s36` (wait for the motion) takes no time in `ryza-commands`, which gives the player no
  motion lengths (the play does).

## File bytes Barrel never uses

`barrel-tool unread` over a sample of the archives (`tools/unread-survey.sh`, docs/parity.md)
and one field's files (`s01_i_attic`):

- **The environment file (`<field>_env.bin`): 97% unused** (95,232 of 97,920 bytes, over
  every time of day and weather Barrel reads). Barrel takes the fog, the stage light, the
  character light and the sun; the rest holds what several stand-ins guess at (exposure,
  saturation, the sky's scales, emission by time of day, the wind: `tools/standins.sh`). The
  largest known gap between the game's data and what Barrel reads. Nearly all of the file is
  now placed against the game's structure (docs/formats.md, "Ryza field environment": the
  running game's 12 sets read by a hook and matched to the file's by value), so each block
  the XML reader names has its place in the file. The wind is read (`gust_`, `environment::Gust`:
  docs/shading.md). Left: the members' names and types inside
  each block (the XML reader's per-block functions name them, from `0x14071d000`), then
  reading them in `environment.rs` in place of the stand-ins; and the pieces the values could
  not place (file 0x614 to 0x674, the shadow block's single words).
- **The placement list (`.opl2`): 68% unused**, nearly all empty slots (the 8 collision records
  of a type with no collision files); really unread are 28 of the 32 bytes after an instance's
  tags and 3 floats a collision record (docs/formats.md).
- **The field's shapes (`_coll.bin`): 18%** (32 of 180 bytes in 4 ranges, not looked at).
- Animations (`.g1a`, 45 files): 12% unused, all of it padding to 16 bytes and header words.
  Models, textures, events, tables, the terrain (`.ktf` 1.3%, `.khm` 1.3%): under 2%.
- No parser to ask: `.g1s`, `.g1pf`, `.trmd`, `Oid.bin`, and the collision primitives
  (`_cbox`, `_ccap`, `_cplh.g1co`: the shared `g1co` reader takes only their header).

## The answer key's runs repeat exactly

ryza-recomp's scenarios run on virtual clocks by default (RYZA_DETERMINISTIC): each frame is
1/60 s and every read a thread makes within a frame returns the frame's time (only past 64
reads in one frame does a thread's clock creep, 1 µs a read, so a loop waiting on the clock
still ends); the null XAudio2's passes follow the frames presented; and Present waits until no
other game thread is at work, so what a frame set going (a load) is finished before the next
frame. A thread in a blocking wait is not at work, nor is one spinning on SwitchToThread: the
game's wait for a resource (0x140728dc0, 83 callers) loops on a yield until another thread,
often the presenting one, has done its part. Two title-cycle runs and two opening probes give
the same captures frame for frame, paced or not, so scenarios run unpaced: the 14,000-frame
opening in about 2.5 minutes for 4.5, a boot to the title in seconds. The boot is 265 frames
shorter than in the first captures (its loading no longer runs across frames in real time).
Timeouts (Sleep, timed waits) end on a clock of their own (ryza-recomp d155a88): in Present
up to the next display refresh, else when every thread is idle, the earliest first and one
thread at a time; a thread woken (an event, a lock, a futex, a start) counts as at work until
it runs. On real time they ended wherever the host got to them, which moved loads by a frame
between runs. Load Game (scenarios/load-rasen.sh, from a save in Rasenboden) and the opening
now repeat frame for frame too. The recapture starts the boot a frame later (the notice at
12) with the title where it was; it also ended the one glow quad at frame 825 that no time
could match (the game's value there came with the old timing, not from a curve Barrel lacks).
Left: the runtime's timer and thread-pool threads still fire in real time (none seen to matter
yet), and a thread that computes for a minute without a wait makes the frames stop waiting
(a line in the log names it).

## Loops that take 50 or 51 frames

The game adds `speed x frame time` to a looping animation's time each frame, in 32-bit floats,
and restarts it once the sum reaches the length (`boot::loop_time`). For the boot spinner's
glow (0.6667 s at speed 0.8, 50 frames' worth) the sum after 50 frames sits within a few
units in the last place of the length: at a frame time of 16,666,670 ns or less the loop
takes 51 frames, at 16,666,671 or more it takes 50 (the same for the title prompt's 0.75 s
blink: 46 or 45). The game's frame time is `now - last` of QueryPerformanceCounter (read at
0x1406b94ab through 0x140b8c9f0, stored at +0x80 and +0x84 of the object in rbx, capped at
+0x88). ryza-recomp's old virtual clock added 1 µs for each read by any thread, so the frame
time wandered by a microsecond either way (16,665.7 to 16,667.7 µs over frames 900-1105,
`RYZA_TRACE_CLOCK`) and the loops took 50 or 51 frames by turns: the "one frame ahead each 105"
of `boot::GLOW_DRIFT` and the title rows' times 9.30 and 10.9667 s (for frames 1700 and 1800
after a start at 1140) are fits to that. On real hardware the same comparison goes either way
with the display's timing; the exact clock gives 51 every time, and 91 for the prompt's whole
1.5 s pulse. The reference capture is made on the exact clock now and `GLOW_DRIFT` is gone:
`boot::loop_time` adds the game's float frame time as the game does.

## Fields

- **The post constants Barrel does not dump** (P2; `rasen-constants`: 665 compared, 0 differ,
  188 (pass, name) pairs not dumped at the gate; `barrel-parity constants ... consts.json`
  lists them by pass). They split three ways:
  - Passes Barrel draws, names not yet dumped (passes 11 to 22 and the tone pass 69): the
    shadow terms of the ambient-and-shadow pass (`mW2S`, `mSemiStaticW2S`, `mSubW2S`, `vSMDF`,
    `sdplane`...), the lighting pass's other draws (`litDir`/`litCol` 1 and 2, `emScl`,
    `vAmbOccRat`), the water's own ambient and lighting (passes 14 to 16), the sky's and fog's
    height fog (`cHFog`, `hFogParam`, `hFogFlucPrms`), the reflections (pass 21).
  - Off at the gate (weight or switch 0; only their source to read): the depth of field
    (`fKIDSDOFType` 0, `fHexScale` 0, passes 31 to 37), the star filter's result
    (`fStarWeight` 0 in the tone pass, though its chain 49 to 60 runs), the light shafts
    (`vLightShaftPower` 0, 63 to 68), the lens flare (`fLensFlareWeight` 0), the vignette
    (`fLimbDarkeningWeight` 0). Their values sit in the set's `trueHDR_` block and its
    neighbours (`docs/formats.md`, the environment table).
  - On at the gate, missing from Barrel: the depth of field's softening. The tone pass
    (`sh54b7a1dac8c7a203`) has three branches on `fKIDSDOFType`; the distant blur
    (`fDistantBlurIntensity` past `fDistantBlurZThreshold`) is only type 2's, and the gate is
    type 0: it mixes in pass 31's quarter-size blurs by the weight in alpha, a ramp
    `saturate((1 / max(1 - 2|w - 0.5|, 0) - 1) / 4)` of `w`, the front blur (`rt480x270 #4`)
    where `w` (`rt1920x1080 #17`'s alpha) is 0.5 or more, the back (`#3`, its own alpha) over
    that. The chain: pass 29 (`sh7b41f1dd6f2b7f36`) writes the scene with `w` = a 512 × 1
    curve looked up by the raw depth (`smplWeight`, `rt512x1 #5`); pass 30 makes the two
    quarter-size layers, pass 31 blurs them in 8 taps; 32 to 37 (the hexagonal blur) feed only
    type 1. The game's pixels at the gate (pixel mirror, scratch scenario `pix-gate`): the
    back weight is nil, the front reaches 0.078 and averages 0.05 to 0.07 over the upper half
    of the screen (the far view), 0 below the middle. The curve is written by the CPU at load
    (no draw writes it in frames 1700 to 2600; the capture logs no texture uploads): what
    builds it (the field's or camera's focus settings) is not read.
  - Also on: the sun flare and paraffin wash (pass 73 reads the cascade into a 1 × 1 sun visibility, pass 74
    draws two screen quads with it: `fFlareAmount` 0.6, `fParaffinAmount` 0.2, `fIntensity`
    20 and 15, `vFlareColor`, `vParaffinColor`, `mRotation`; the set's lens-flare block holds
    them), the game's TAA (pass 72, `g_cbTemporalAAParam` 960, 0.1, 0.05, 0.2, `jitterScale`
    0.5; Barrel's TAA is an option of its own) and two textured screen quads (pass 76,
    `materialColor` 1, not identified).

- **Water** (P2). Barrel draws Rasenboden's river and sea (`docs/shading.md`, "Ryza DX: the
  water"; the file `_river.bin`, `docs/formats.md`): the ribbons of its two bodies from the file's
  points and widths, lit by the field's sun, mixed with the sky's colour by the Fresnel term, fogged
  as the models; the sea runs to the horizon. Left: the game's wave volume normals, the refraction
  and the depth fade (the sea is opaque), shore bubbles, the planar reflection, and the cubic the
  game makes of each piece from the points (Catmull-Rom stands in: the draw's own stream is not in
  the capture). Barrel's draws still do not match the game's 10 water draws in `diff3d` (they are
  not in Barrel's trace); at the gate the sea lies behind the camera, so the horizon's white haze
  there is the sky's, not the water's.
- **Rasenboden against the game's draws** (P2). From the game's own camera at the gate after
  loading a save (ryza-recomp's `load-rasen`, frame 2550: eye 32526.8, 6363.3, 29944.7;
  `barrel FIELD --look-at 32509.512,6224.147,30934.779 --distance 1000 --pitch 8 --yaw 180.99981
  --fov 45 --ryza 32517,6180,30336 --done MM00_010`, row `rasen-3d-2550`), 856 of the game's 913
  draws have a Barrel match. The 55 without: the terrain's 42 strips (Barrel draws a patch at
  once), the water's 10 (below), one `npc23a` draw and 2 of the field's models. Barrel
  draws 113 the game does not: 62 of the field's models and some 20 townsfolk draws, in view
  and hidden behind the town's buildings and ground (the game's own test of what is hidden
  is not read; Barrel's stand-in, the ground's skyline, leaves out 211 draws the game also
  leaves out and none it draws: `docs/renderer.md`, "Ryza DX: objects behind the ground").
  The "title backdrop" draws are Rasenboden's own cliffs and rocks (their textures share
  names with the backdrop's in the index; `PARITY_LIST=1` now names each extra draw's model,
  object number and place). Of the 47 field objects still drawn only by Barrel at the gate,
  34 are on screen and 13 straddle the eye, at all distances (157 to 44,000 units) and sizes,
  in all four classes. Ruled out as the game's rule: the view (none outside it), distance and
  size (the drawn ones span the same), the GPU's queries (none), and the finished picture's
  depth: against the game's own depth target (`1 - 50 / view depth`), 75 objects it draws are
  wholly behind it and some it hides are mostly in front (a fence's box 89%). Where to start:
  the code, not the data: hook the field object's per-frame draw (find it from a hidden
  object's record by RYZA_FIND on its position, `+0x230`, then RYZA_WATCH the flag words near
  `+0x5a0` over a frame), and read what sets the skip. Tried since: the placement word's
  unread bits (0x08 to 0x10000) do not split hidden from drawn (both carry 0x28, 0x68, 0xa8,
  0x120a8); a trace object's number is its placement's index plus one. Searching the running
  game's memory at frame 2550 for the objects' positions (RYZA_FIND) finds their records; a
  byte 0x727 past the position was 1 on two hidden rocks and 0 on two drawn ones, but over
  107 records it is 1 on 52 drawn and 0 on 10 hidden: not the flag. (RYZA_FIND stops at 2000
  hits; leave out an object placed at the origin, whose zeros match everywhere.) **The rule
  is occluders**: the field's `_le.koc` quads (docs/formats.md, "Ryza field walls"). Each frame
  (`0x140a3cac0`, called from `0x140725b50`) the game keeps the quads in view (each as two
  triangles against the frustum, `0x14099aa20`; a quad's byte `+0x45` turns it on), sorts them
  by their nearest corner's depth (`0x140a3b7d0`), gives each five planes (its face turned
  away from the eye and its four edges through the eye, `0x140985080`), drops quads wholly
  behind a nearer one (flag `+0x5d` of the 0x60-byte records at `+0x30`, count `+0xc8`), then
  measures the kept ones' corners in a 2D camera basis (from `0x140a3d183`; use not read).
  Tested with Barrel's draw boxes at the gate (scratch `koc2.py`), per model part, as `diff3d`
  pairs repeated baked models by count and not by place: a box wholly inside one quad's
  five planes accounts for 45 of the 53 parts the game hides, but 3 the game draws would be
  hidden (`S06-HOUSE-01`, `COMMON-STUMP-01`, `S09-STAIR-02`) and 11 it hides are not
  (cliff blocks, `CLIFF-15M-01`, `CLIFF-30M-02`, a mountain). A bounding sphere fits worse (12).
  The per-object query (`0x140a39fc0`, from `0x1407282ce`) and Barrel's version of it:
  docs/renderer.md, "Ryza DX: objects behind the field's occluders" (99fe8e5): at the gate
  Barrel's draws 975 -> 934, field parts the game does not draw 78 -> 38, matched 859 -> 858.
  The stump was a false lead: the game hides stump 1147 too (its own query on its own kept
  records, a hook `flt*20:rdx u32:rcx+0xc8 flt*768:[rcx+0x30]` at frame 2550, scratch
  `occr.py`: 12 records, no zero planes, 97 of the 364 boxes hidden, 1147 among them). The
  game's fourth stump draw is stump 1145, which it queries and draws and Barrel leaves out
  by the view: its centre is at x -1.11 in Barrel's view (box to -1.06), the paired draw by count
  made 1147 look wrong. The view test's planes are the camera's (logged at `[r14]`: 72.7 by
  45 degrees, near 50, far 300,000, inside negative), but an object takes one of two tests
  by its word `+0x250`: 0, its oriented box (`0x14099a3d0` on `+0x1d0`); else the world box
  around that box (`0x14099a3b0`, a jump to `0x14099c050`, on `+0x220`: max then min corner).
  At the gate 463 take the first and 760 the second (scratch `views.log`); 1145's world box
  reaches 2 units inside the left plane where Barrel's (its vertices' box, 45 to 187 units
  smaller a side) is outside. What sets `+0x250` is not read: not the model (both tests
  have deserts, cliffs, trees, fences), not the tilt, nor size or distance alone (the second
  test's objects are smaller, median half diagonal 194 against 1,495, but overlap). Left:
  (1) the `+0x250` flag (Barrel would test those objects on the box around their oriented
  box; one draw at the gate). (2) The rest of the extra parts
  (20 objects at the gate) are not the occluders' doing. The game keeps quads 98, 8, 3, 6,
  2, 1, 0, 4, 9, 5, 11, 14 (its records matched to the file's by plane); the dropped ones
  in view are mostly inside a nearer kept quad as Barrel's rule has it, but 15, 16, 92, 95,
  96 and 97 are not. Both programs take the same 26 quads in view, in the same order (the
  update `0x140a3cac0`'s stale records past its count hold all 26, depths Barrel's less the
  near distance 50), and the game's count is already 12 right after its covered-quad drop
  (hook on its `sqrt` call, `0x140d70164` from `0x140a3d284`, reading `rdi+0xc8`). That drop
  as read (`0x140a3d090`..`0x140a3d17d`: a later quad goes when its four corners lie strictly
  inside all five planes of a nearer kept one) keeps those six: none of the 12 holds one by
  85 to 2,300 units. So the drop does more than read (2 tries; the view test `0x14099c7f0`,
  the enable byte `+0x45` (all 1), the frustum (the camera's) and a count cap are ruled out).
  Barrel keeping quad 15 hides 5 objects the game draws (899 to 901, 958, 1137).
  The mountain (8), the cliffs 78, 95, 96, 586, 587 and the kudzu 972 reach the game's query
  and its own records leave them visible (the best quad misses each by 397 to 3,108 units),
  yet it draws none of them: something after the query leaves them out (per part, or a
  draw-stage test; not read). The far trees (785 to 1183, `-FAR`/`-FAREND`) and object 168
  never reach the game's view test: left out before it (a far-tree switch, not read). (3) Occluders are off on the title (`a.title`): the title's field
  has a `_le.koc` too; whether the game uses it there is not checked. What got it here from 809: objects are
  in or out of view whole, by a sphere (`docs/renderer.md`, "Ryza DX: which field models are
  drawn"), and have no draw ranges or lower levels of detail at the top settings, as the
  game's code and capture say ("Ryza DX: object draw ranges"). The story's progress decides
  who and what a field places (`docs/tables.md`, "Ryza DX: story spans"; this save has only
  `MM00_010` of the main story, which leaves the same 22 people and 4 boards as before).
  The townsfolk stand where `npcmarker.xml` puts them, turned as the capture's root matrices
  (`npc25c`: within 6 units and 0.01 of the game's). The walkers are where the game has them only at the
  game's own field time: row `rasen-3d-2550` runs with `BARREL_SKY_CLOCK=13.7166` (the pixel
  board's clock for that frame; at 0 the two walkers in view stood at their paths' starts,
  1,200 and 3,600 units off). At that clock the townsfolk's bones are within 55 units of the
  game's (`npc29a` running on `s06_move_data_003`, 3 m/s), 41 (`npc22a/b` walking
  `s06_move_data_005`) and under 9 for those standing; walkers now stand on the
  ground under their path (the field's heightmap and floors, as Ryza's; `WALKER_STEP`, a
  stand-in, though 50 to 1,000 give the same): `npc22a/b` 41 and 40 -> 11 and 6. Left:
  `npc29a` stays 55 off (not height: `DIFF3D_POSES=root` has its root 28 and 50 off in x
  and z, along its path: at clock 13.5 instead of 13.7166 it is 7.6 off, but `npc22a` is
  then 40; no one clock fits all three, so its pace, its pauses' lengths (random between
  `interval_min` and `interval_max`) or its start differ; the capture's frames 1750 to 2550
  (every 50) settle it: the game runs it at 301 units a second (3 m/s, as Barrel) and covers
  140 fewer units between 1750 and 1850 than its pace gives: a pause at a point, whose length
  the game draws at random between the row's bounds; Barrel cannot match it without the
  game's random numbers, so this one stays), the game's own
  rule for its walkers' ground (not read), and walkers on the title (no ground without
  Ryza's rig), and the pace of their motions. The story comes from a save with `--save` (`docs/formats.md`,
  "Ryza DX saves"). Left: the object ranges' own measure and classes below
  the top setting (the placement word's bits 0-1 as the class is still a guess: nothing at
  High tells), the game's switch for levels of detail, the walkers' pauses, motions and
  ground between points (`docs/tables.md`), the people's drop to the ground when
  `no_gravity` is 0 (ten of Rasenboden's rows have it 0, with motion 0 and no turning), the
  people's and gimmicks' own ranges below the top setting, and the bits of `timezone` and
  `weather` (stand-in
  order). The windmills' sails turn and the boat rocks by the motions the object
  container has under their names (`<model>.g1a`, looped: the game turns the sails 0.5° a
  frame, all windmills together); where in the motion the game is when a field appears is
  not read (Barrel starts it with the field; the board's `BARREL_SKY_CLOCK` sets it, and the capture's
  sail joints sit where the sky's clock puts them: 7.05 s at frame 2150). The game's projection is a
  vertical field of 45.0 degrees at 16:9 (`mW2P`: y scale 2.4142 over the view's pitch cosine, x
  1.3578): the 45.4 the gate's row used was 1% too wide (read off the culling, not the matrix).
  With the exact camera (eye from `mW2P`'s zero point, the yaw and pitch from `mWV`'s third row,
  the field of view from its y scale) the clip matrix equals the game's to 5 digits and the
  game does not jitter its projection at these frames (the temporal anti-aliasing's constants
  are not dumped, but `mW2P` has no sub-pixel offset). The cull stand-in (docs/renderer.md, "which
  field models are drawn") was measured under the 45.4 view: 858 pairs then, 856 with the exact
  one (2 parts of objects at the view's edge).
- **Ryza 3 DX's placement lists** read neither as Escha's nor as Ryza DX's (they fail at the
  first type's instance count): for when Ryza 3 starts (`docs/formats.md`, "Placement lists").
- **Field lighting.** `barrel --time T --weather W` lights a field with the game's own
  constants where they are read (docs/shading.md, "Ryza DX: the frame as the game draws it":
  every pass, its formula and its constants at Rasenboden's noon, from the answer key's
  capture): colour textures sampled as sRGB, the sun at the stage's key scale (3 at noon) and
  the second light from the set, the cloud pattern on the sun (strength 0.7), the stage's
  ambient from the diffuse light map (7 × (0.1, 0.2602, 1)), the exposure metered (0.9 at
  noon), the colour scale (1.27, 1.3, 1.2), the filmic curve with the set's
  `fLumiMax` and `fGamma`, and `SaturationScaleEx` (1.6) after the gamma. Against the game's
  noon still at the gate (`work/ryza-ref/rasen-gate-noon.png`, fourteen spots): sunlit sand
  237, 217, 144 against 246, 232, 175, stone 226, 227, 209 against 236, 236, 210, a path in
  shade 25, 57, 117 against 25, 57, 115, a rock in shade 46, 73, 119 against 56, 82, 118;
  mean error 12.8 levels a channel (14.5 before these). Left, largest first:
  (1) **dark surfaces in shade** come out dimmer and less blue than the still's (a trunk 32,
  32, 31 against 44, 53, 63, dark leaves 23, 65, 74 against 44, 81, 78): the game adds the
  specular ambient in a pass of its own and Barrel's models do it as the pass does (the `_as`
  cube map by the reflected view, from the set's scale and tint, fogged; docs/shading.md),
  but the bark's specular map is dull (`F0` 0.02, gloss 0.2), so it adds a few levels;
  what is left is the screen-space reflections (the pass mixes the cube map toward them),
  and the terrain's, the plants' and
  the characters' specular ambient (not drawn; the terrain's tiles are as dull). (2) **sunlit grass**
  lacks red and blue (150, 194, 47 against 171, 210, 64; 133, 190, 2 before the direct
  specular and the terrain's normal maps): the plants' own ambient (the pass's
  second kind, a flat (3, 6, 4), is a guess) and the bloom. Sunlit sand is 193, 195, 94
  against 217, 219, 114: a gap of about a tenth in every channel that the ground's
  light does not explain yet. (3) **the ambient occlusion**
  and its tint: drawn (docs/shading.md, "The ambient occlusion"), in the direct and ambient
  specular too. The direct specular is drawn for
  the models with a specular map and for the terrain (docs/shading.md, "What Barrel does of
  the lighting pass"); the characters' and the plants' are not. (4) **the exposure** is metered by the
  game's rule (docs/shading.md, "The exposure"): daylit views and the attic's backdrop stay at
  the set's floor (0.9), as the game's noon still's own brightness says it does, so the
  sunlit sand's tenth is not the exposure; the night view rises to 1.14. Left of it: the depth
  of field's weighting of the metered layer, and the adaptation's base rate and time (0.95
  and 3 s, from the capture: no lighting set carries them). (5) **the bloom**: the game's bright pass and chain are done, its star
  filter is not. (6) **the cloud pattern's
  phase** starts with the field (the game's is not read), and the pattern's switch for the
  other times of day is a reading of one word (stand-in). (7) **the sky**: the plane and the dome take the game's own constants (below). Evening,
  morning and night have no reference frame. Ryza takes the characters' light (`indirectChara_`:
  the sun at 0.8, the set's ambient at 0.2 plus the light map's mean at 1.5 × (0.8, 1, 1),
  flat rather than by her normals), her second light at 0.487 (`KeyLight1Scale`); the game also adds
  the light volume, which Barrel does not.
  (8) **The constants board** (`barrel-parity constants`, docs/parity.md, "Constants parity"):
  397 constants of the lighting and post draws at the gate against what Barrel uses, 0 differ
  (17 before the board's own conventions and Barrel's two wrong values were put right: the
  scene fog's in-scattering was the sun at 1 where the game's fog pass takes it at the stage's key
  scale 3, and the characters' second light was at 1 and 0.267 where the game takes 0.487).
  So the lighting set's values are not what separates the pictures at the gate (the pale
  distant hills, the white horizon band, the sand's tenth): the cause is in what Barrel has no
  counterpart for, the 240 constants the board lists as not dumped (height fog parameters read as
  zero at noon, the ambient occlusion, the reflections, the cascades' matrices, the light
  volume's clusters, the depth of field, the star filter, the light shafts, the temporal
  anti-aliasing), or in a formula. The board's values that are stand-ins (the dome's and the
  strip's wind offsets are left out: time-dependent) are marked in `play/constants.rs`.
  (9) **The halo and the gate's picture against the game's pixels.** Two views are on the board
  (`docs/ryza/views`): the gate and the turn view (frame 2150, toward the windmill tower and a
  house). The camera first: the gate's row had `--fov 45.4` and a yaw of 181, 1% wider than the
  game's projection and 0.0002 degrees off; with the matrix's own 45.0 and the exact yaw the gate's
  tone error went from 15.87 to 9.02 (stage 15.2 to 8.0, the `normal` median from 4.3 to 1.5
  degrees) and the turn view's from 19.5 to 10.9, all of the "thin edge offset". The turn view
  then lost its windmill-tower band when the field clock turned the sails to the capture's phase
  (7.05 s; the sail shadow had fallen on the tower's other side): tone 10.12 (stage 9.11, sky
  16.4, characters 26.4 over her pose). With the dome's layers slid as the game's
  (`BARREL_DOME_CLOCK`) the gate reads 8.12 (sky 3.7) and the turn view 9.05 (sky 4.9); with
  Ryza where the capture has her, at her idle's phase (docs/parity.md), 6.79 (characters 9.5 from
  36.9, stage 7.05) and 8.69 (characters 13.9 from 26.4, stage 8.94); with the field models'
  metal darkened as the G-buffer program writes it (colour times 0.2 times the specular map's
  green: the mailbox's ornament read 0.74 0.54 0.16 against the game's 0.25 0.15 0.06 lit, now
  0.29 0.19 0.11) 6.77 and 8.53; with the cloud pattern where the game's matrix puts it (it
  stood 500 and 866 units off: docs/shading.md) 6.19 and 5.48, the sun's share on open ground
  0.456 against 0.458 and 0.446 against 0.448 (so the shares below, measured before, are
  mostly gone). Shares left on the turn view (lit, game against Barrel):
  the tower's sunlit side 1.70 against 1.86 and shaded 0.86 against 0.93 (the sun's share 0.43
  against 0.47 and 0.45 against 0.48: the cloud pattern's level; the occlusion and normals
  agree, the normals within 3 to 4 degrees), the house's white wall 2.41 against 2.77 (share
  0.425 against 0.492, flat in the game and banded in Barrel's: the pattern's lookup on a
  vertical wall), the planter 0.20 against 0.26 (share 0.45 against 0.56), the rock 1.04
  against 0.84 (Barrel darker, share 0.54 against 0.51, normals 3.5 degrees), the crates and the
  grass and the path within 3% (grass 3%). The pattern's level is the open term at both views:
  the gate's ground has Barrel 7% under the game's share, the walls 16% over.
  Earlier measures, before the camera was exact: The game's own render
  targets for the gate (frame 2550; the pixel board, docs/parity.md) replaced the still as the
  measure. The halo was Barrel's, not the game's: the cloud strip's colour was not stored at
  half like the rest of the scene, so the clouds were twice as bright (fixed:
  docs/shading.md, "Ryza DX: the sky, the clouds"), and the bloom's bright pass three times the
  game's. Tone error against the game's tone pass (mean absolute, 0 to 255): 24.63 at the
  start, 24.13 with the strip's fix, 23.85 with the clouds given the capture's phases
  (`BARREL_SKY_CLOCK`, `BARREL_PATTERN_PHASE`, `BARREL_CLOUD_THRESHOLD`; the game's cloud map is
  noise from a lattice it shuffles each run, so its clouds' places cannot be reproduced). The
  camera is within a pixel or two (the best shift of the picture is (-1, 2) px: 22.97 against
  23.34). What the 23.85 is made of at the gate (shares of the summed error; the picture's
  error is mostly broad, not fine: still 16.4 after a 32 by 32 box blur of both):
  - **Leaves missing in the trees' crowns: solved.** The alpha-tested field materials
    (`PBBA2-N`: the trees' leaf cards, house trims, signs) are drawn by the game with no face
    culling, and Barrel culled their back faces: from below, the cards that face up were gone
    (docs/shading.md, "Two-sided materials"). Sky is 9.10% of Barrel's half-size depth against
    the game's 9.15% (10.40% before; barrel-only 0.51%, game-only 0.56%), the crowns'
    cells (2,0) and (3,0) hold 1.4% and 3.9% Barrel-only sky (12.1% and 14.3% before), and the
    tone error is 17.36 (20.44 before). What is left of the sky's edge is leaf detail (the
    game-only share is as large). The sway's phase and the alpha the G-buffer program reads
    are the game's, so nothing there was checked further. Not found: what the game keys the
    single-sided `PBBA2-N` draws on (flowerbeds, ivy: lighting-material flags `0x133`).
  - **The ambient occlusion: about 13%** (the pixels with an occlusion below 0.85 are 35% of
    the picture at a mean error of 27.4 against 15.0 elsewhere; the game's own occlusion laid
    over Barrel's scene took the tone error from 23.2 to 20.1). The feature is built from the
    game's program (the tone error is 20.6 on the board: 22.8 before) with a stand-in noise
    texture, so the occlusion's pattern differs (mean abs 0.128 against the half size picture, its
    regions' means 0.02 to 0.08 above the game's: the left canopy 0.741 against 0.657); the tree
    regions of the lit stage are still 1.5 to 2 times the game's brightness (item below).
    **The far hills** (the strip (1500, 330, 1800, 350), lit and fogged 0.150, 0.387, 0.535
    against the game's fogged 0.128, 0.318, 0.427): two causes found and fixed. Their models
    stand at 0.47 of their height and the game turns normals by the placement's own matrix
    (Barrel turned them by the inverse transpose), and the normal map's green runs against the
    texture's v (Barrel's frame from the screen derivatives had it along v: over the stage the
    median angle to the game's normal target went from 14.6 to 11.8 degrees, the stone wall
    from 24.5 to 16.3). The meshes' own tangents (the game's frame, `B = cross(T, N) w`) take the
    median angle over the stage (the board's `normal` line, half size) from 5.7 to 5.6 degrees,
    the far hills' from 24.0 to 22.8, the wall's from 7.6 to 7.0, the trunk's from 10.9 to 10.7: the
    derived frame was nearly the same. The grass is the biggest region left (the game's normals
    there vary with a standard deviation of 0.17 in y, Barrel's plant cards' 0.01). What is left
    on the hills is about 20%.
  - **The sky's pixels: 9.5%** (9% of the picture, mean abs 24.9; with the game's own cloud map
    (`BARREL_CLOUD_MAP`, docs/parity.md) 16.9 and the tone error 15.76 against 17.01 without it). Measured on
    the board's stages (sky pixels, game units): the plane 0.091, the plane and the dome's first
    layer 0.106, both layers 0.416 (the dome's second layer is the clouds on the dome: its
    contribution to the sky is a quarter weaker than the game's in the rows 0 to 180, 0.47, 1.0 against 0.63, 1.33 in
    green and blue; the game's sun term there is about 0.37 stronger; the mesh's own tangent, now in
    Barrel's vertices, changed the `dome` stage by less than 0.001, so that is not the cause),
    the strip's contribution 0.106 (0.241 with the grid cells split along the other diagonal; the rest is in
    the rows 0 to 60), the finished sky 0.51. Those were measured with the dome's layers standing still: with their
    scroll (`txTrs01`, docs/shading.md) the second layer's weakness is gone (it was the clouds
    standing elsewhere), the `dome` stage's sky reads 0.079 and the finished picture's sky 3.7
    of 255 at the gate and 4.9 in the turn view (12.5 and 16.4 before). Not drawn are the cloud particles: the game's draw `1916` leaves the picture as it is at
    this frame (its target equals the strip's, every pixel exactly), so they are not built.
  - **The characters: 3.7%** (2.2% of the picture, mean abs 39.4): Ryza's pose differs by
    10 to 15 pixels at her feet (the idle's phase), her shaded side and outlines. The
    characters' own shadow map (docs/shading.md, "The characters' map": her draws are in it and
    not in the cascades; 1024², its box around her) is built: her shadow on the ground has the
    game's crisp shape and is cut at the box's edge, and the `shadow` stage's characters mask is
    0.099 mean abs (0.114 before; the game's sun share on her pixels is 0.086, Barrel's 0.092
    against 0.110). The tone error did not follow: 15.81 to 15.85 over the picture, the `ryza
    shadow` region's tone 29.96 to 31.03, because the crisp shadow now lies where her pose differs
    from the game's (a blurred one overlapped more). Left: the box's rule (the game's grows with
    her pose; Barrel's is her box on the starting joints plus a constant, a stand-in), which
    shaders take the second bias set (a stand-in list).
  - **The rest, about a third**, spread over the stage at 15 a pixel: the shadows' edges (the
    sun's share against the game's alpha: 0.072 mean abs over the stage, a third of it from the
    stage past 10000 units, where the game reads its static map and the clouds' pattern,
    random per run; the characters' own shadow map is built, below),
    the reflections, the depth of field's softness, the temporal anti-aliasing, the lantern's
    glass (lit white in the game, dark in Barrel).
  Where the game's occlusion is 1 and the sun's share 0, Barrel's scene is within 3 to 5% of
  the game's, the stage's ambient equals the game's ambient pass to 1%, and the sunlit sand and
  stone are within 6% (docs/shading.md, "Against the game's own pixels"): the lighting set's
  values and the exposure are right; the picture's error is coverage and occlusion.
  Ryza herself, seen from behind at the gate (her shaded side), with the toon ramp sampled
  as the game's pass does (docs/shading.md, "Ramp"): coat 161, 146, 69 against the still's
  162, 150, 75, thigh 154, 138, 142 against 156, 143, 102, hat 149, 142, 138 against 162,
  160, 161; left: the frill under the coat (142, 119, 61 against 90, 75, 48: the occlusion,
  which also drives the ramp), the boot calves (95, 60, 27 against 60, 40, 25: her forward outline pass, docs/shading.md), the
  still's dark creases on the coat and red sheen on the boots (occlusion and specular).
  The ramp's constants are the ones captured at Rasenboden's noon, taken for every field.
- **Model lighting material.** Ryza's models' G1MG section 0x10001 reads as in Escha but holds
  diffuse 0.8 and ambient 0 (`barrel g1m-materials`): their shaders (`PBB-N`, `PBB-EmN`, ...)
  take the ambient from the environment, not `cAmb`. `barrel` gives every draw the full
  material until those shaders are read.
- **Emissive and odd materials.** `PBB-EmN` submeshes (the attic wall's windows; at
  Rasenboden three houses' windows, a bridge pillar and the light poles) have an emission map
  (texture kind 19) and flicker parameters (`emFrq` 1, `emRange` (0, 1) or (1, 1), `emStart`
  0 in Ryza's house). The lighting pass adds `sEmi × |emScl|`: to the light (times the colour)
  when `emScl` ≥ 0, to the finished colour when negative; `emScl` is not among the material's
  parameters (set at run time). `barrel` draws the map as light (`model.frag` flag
  32768, the map bound as the draw's set 3) at a stand-in strength by time of day
  (`EMISSION_BY_TIME`: none by day, 2 at evening, 3 at night): the house's window glows at
  night. The flicker is not done: no pass of `PBB-EmN` (`_gb`, `_gba2`, `_cll`) reads
  `emFrq`, `emRange` or `emStart`; all three take only `emScl` (+16 in the G-buffer passes'
  block, +96 in the lighting pass's), so the game works the flicker out on the CPU into
  `emScl` each frame. Its curve (a wave between `emRange`'s two ends at `emFrq`, offset by
  `emStart`, would fit the names; `emRange` (1, 1) would then hold still) is a guess until a
  run reads `emScl` over a few seconds (tools/oracle/README.md). Not drawn yet from the G-buffer model
  (docs/shading.md, "Ryza DX: the deferred G-buffer"; normal maps are drawn now, their
  green's sign unchecked): the specular map (metal flag, specular amount, roughness), and the lighting pass (`pbb-*_cll.g1s`). Attic view inside: `--look-at 3905,2250,3914 --distance 300 --pitch 10`.
- **Terrain against the models.** Rasenboden's terrain (grass, dirt paths, cobbled road; its
  heights match the heightmap) lies at grass level over the sandy canyon path the cliff models
  make in the village close-up (`--look-at 31261,5466,30776 --distance 6000 --pitch 25`). Every
  Ryza lit terrain draw has kind bit 0, so that bit hides nothing; program bit 4 (on some
  draws) is unread. Needs a frame of the game at that spot, or the field shader pack's terrain
  programs. The layers' normal maps are not drawn yet.
- **`obj-A21-S06-FLOOR-01`** (placed at the origin, its geometry in field coordinates, x 1210
  to 56805, z 33342 to 53823) is the stone paving along the canal and its bridge: hidden
  (`--hide FLOOR-01`), the overview shows a hole there. It draws where it belongs; only its
  collision meshes are off (below). Not a stray "sand square", as first noted.
- **Other field files** not in Escha: `.trmd` (`TRMD`, 5 MB, 1024 × 1024 header words then
  runs of `24 ff ff ff ff`), `.g1pf` (`FP1G`, a grid over 128,000 units, likely navigation),
  `.koc` (read: the field walls) next to the containers.

## Against the game

A recorded run of the original (a new game: the opening, the attic event at dusk, free play
from the path outside Ryza's house; kept outside the repository) is the reference. Ryza's house
is `obj-A21-S06-HOUSE-01` (32538, 6194, 28804); free play starts on the path down from its gate,
past the signpost (32148, 6003, 31151) toward the light poles. Close to the game's first view:
`barrel INSTALL s06_t_rasen OUT --look-at 32400,6350,30600 --distance 800 --pitch 12 --yaw 150
--time morning`. What differs there, largest first:

- **Shadows.** The game's path and rocks lie in the trees' shade, lit blue by the sky.
  `barrel` casts the sun's shadow from every placed model and Ryza (`--no-shadows` to
  compare) as the game's three cascades (boxes round the view's slices to 1600, 3600 and
  10000 units, with the game's depth bias and filter) and a static layer over the whole field
  past them (docs/shading.md, "The cascades"; docs/renderer.md, "Shadows"). Measured against
  the game's sun share at the gate (`barrel-parity pixels`, the `shadow` stage): the stage 0.0785
  mean abs, the middle tree 0.112 (the game's 0.117), the left canopy 0.010 (the game's 0.010,
  0.034 before the leaf cards were two-sided: the leaks were the cards' missing back faces,
  not the sun), the dark canopy 0.012 against 0.009, the trunk 0.002 against 0.001, the grass
  0.402 against 0.428, the sand 0.345 against 0.380. Left: (1) where the game takes its cascades' ends
  (1600, 3600 and 10000 and the fade's 3500 are one frame's constants, `CASCADE_ENDS`), the
  depth plane 36755 before the camera and the third cascade's box (11672 wide, no slice from
  3600 fits it); (2) which draws cast (a placed model casts here; Barrel's cascades hold 160,
  235 and 687 draws against the game's 104, 177 and 669: the choice is not found); (3) the
  characters' map (`mSubW2S`, 25 draws: built, with a stand-in box); (4) the static map's contents (drawn once at load in the game: it is
  not in the capture; its box and its reading are the game's, docs/shading.md "The cascades",
  and its bias is the far cascade's as a stand-in; where its top 5,000 and its depth 173,439
  come from is not found). The lit canopy's brightness (0.05 against 0.022) is not the sun's share
  but the occlusion: regressing the lit stage on the share per pixel, Barrel's ambient part is
  0.061 against the game's 0.039 in the canopy and 0.070 against 0.031 on the middle tree, the
  game's sun slope half Barrel's. With the occlusion built (docs/shading.md, "What is left in
  the lit scene") the canopy's lit colour is 0.044 against the game's 0.022: the sun's share
  was the part left (0.034 against 0.010): two-sided leaf cards close it (lit stage: left canopy
  0.023, 0.044, 0.034 against the game's 0.022, 0.042, 0.032; dark canopy 0.032, 0.059, 0.045
  against 0.030, 0.053, 0.032; trunk 0.024, 0.034, 0.038 against 0.019, 0.028, 0.031). The shadow pass
  is 1.9 ms (6.9 before the geometry moved into device memory, docs/renderer.md); which draws
  cast is still the open part (160, 235 and 687 against the game's 104, 177 and 669).
- **The sky.** `barrel` draws the sky plane with the game's own constants (the set's
  `skyFog_`, the sun times its power, the path through the atmosphere, the horizon haze of
  the glow block) and the set's cloud dome where the set places it, lit by the set's ambient
  and sun for it and fogged with the sky's scattering (docs/shading.md, "the sky"). The game's large white clouds
  are drawn too (a strip showing a 1024 × 1024 cloud map ray-marched from the set's `cloud_`
  block, docs/shading.md "the clouds"; where they fall differs from the game's, whose noise
  table is reshuffled each run, so the gate's sky mean runs 130 to 149 in red over three tables
  tried, the game's still 153). Left, largest first: **the cloud particles** (`cloudParticle_`: a few dozen
  billboards from a CPU system whose rules are not read), the clouds' stand-ins (their
  threshold moves with a height noise and the sharpness follows it, `3 / (1 - threshold)`; the grid's height and fall are read from
  one frame, so other eye heights and sets are unchecked), how long the dome's layers have
  slid when the sky's clock starts (the game's dome runs some 14.7 seconds ahead in the captured runs; Barrel's starts with the sky's).
  At the gate's noon a gap in the leaves at the top of the view shows bright sky where the
  still shows leaves: the tree's foliage there, not the sky. Morning, evening and night have
  no reference frame.
- **Which set at which hour.** The game's first view of free play
  (`work/ryza-ref/rasen-path-start.png`, the clock at 10 AM) looks like our noon set there:
  deep shade, blue-lit path and rocks, a clear blue sky. Our morning set comes out pale and
  hazy with hardly any shade: its ambient is ten times noon's (19.7, 29.6, 35.6) and its
  `skyFog_` a thick grey haze (mie 4, 3.4, 2.5; rayleigh 1.5, 2, 3). The HUD's clock shows which:
  its icon is one of four (`gen_a21_field01_clock_icon` in `ui/a21_field.g1t`: rising sun,
  full sun, setting sun, moon, in the set order morning, noon, evening, night), and at 10 AM
  in every still it is the full sun, so 10 AM is already the noon set. The hours where the
  others begin are not read yet (the executable names `time_zone`, `EVENT_SET_TIMEZONE`). Night has no reference
  frame yet, so its colour (too bright and green here) cannot be checked.
- **Grass.** The plants (flowers, weeds, tufts, wheat) come from `_grass.bin` / `_grass.g1t`
  (docs/formats.md): which kinds grow in a cell, how many, their cards and their sizes' ranges
  are the file's, and `barrel` draws them as the game does, one draw a 500-unit cell of upright
  cards turning about the vertical (look slots 37 to 39), on the heightmap: the title's 348
  plant draws all have their match by card atlas and plant count. Which cells are drawn is
  the game's own test (docs/renderer.md, "Ryza DX: which plant cells are drawn"): read from
  `0x140a78dd0` and its planes hooked at `0x1409990c0` (the call from `0x140a79304`), it gives
  the game's cells but for 1 on the title and 2 at the gate; Barrel's left-over plant draws
  are 1 on the title and 3 at the gate (they were 2 and 15 with the measured limits it
  replaced). Not modelled: the test box's top uses the tallest plant of the cell's kind
  (`+0x48` of the kind), where Barrel takes the cell's tallest plant. Where a plant stands in its
  cell and where in its ranges its size falls are the game's own numbers
  (`barrel_ryza::grass::Scatter`, from the game's cell builder `0x140a77060`, which fills a
  cell's vertices the first time the cell is drawn: a generator `s × 69069 + 1` seeded from
  the cell's grid index, each member's run seeded from the cell's, four numbers a plant):
  all 574 plant cells of the pixel board's three captures come out plant for plant, place
  and size. The title's 348 cells' first plants stand where the rule puts them too. Not read in the builder: a second run of numbers that can drop plants (none is
  dropped in these captures), and a branch for cells at the edge of a neighbour with
  another setting (flags at the cell's +0x100, +0x104, +0x124). The row length in the seed
  (256) is the builder's on Rasenboden; whether it follows the grid's size is not checked on
  another field. Stand-ins: how a lower grass quality thins the plants, a sway of the
  cards' tops by stand-in wind values (`PLANT_SWAY`: the game's `g_vAttCache0` / `g_fPhase`
  are set at run time; sway slots 0 and 1, free in Ryza's fields), no shadows cast, lit as
  the field's models with an upward normal, normal-mapped by the cards' own normal maps
  (`_grass.g1t`'s third texture, flag 8192, as the game's plant program does). In the shade
  they come out teal where the game's are green: the cards are green (the grass tuft's opaque
  pixels average 60, 83, 25) but the sky light from above is blue (noon's 2.7, 3.0, 5.2 ×
  the cube map); the game's pigment mix most likely carries the green. They shrink away
  over the game's grass preset's range as the game's do (`model.vert` flag 16384, look slot
  93); the near clip and the far scale-up are not done. The preset follows the graphics'
  `foliage` (full: Middle, density 1, 8000 to 15000; reduced, the handheld preset: Low, 0.75,
  3000 to 3100; least: VeryLow, 0.5, 2000 to 2100; `barrel_ryza::grass::Preset`). Rasenboden
  at the gate: 120,884 plants in 1,432 draws, 4.5 ms a frame on the PC at 1280 × 720. The
  game's plant program mixes the vertex colour with a field-wide pigment map by
  `g_vPigmentMapScale` (docs/shading.md, "Ryza DX: the plants"); which texture is the pigment
  and the scale's values are set at run time, not read. What is left of the grass on the pixel board is fine detail: the
  left view's grass strip (y 480 to 600) is 12.3 of tone error a pixel but 3.6 with both pictures
  blurred by 6 pixels, the two line up to the pixel (no half-pixel shift does better), and their
  sharpness (mean Laplacian 63.1 against 64.1) and spread (standard deviation 54.3 against 54.0)
  agree: the blades differ one by one, not in shade. Over the whole board half the error is
  such detail (4.08 against 2.02 blurred on the left view). The title's capture has no
  other grass draws: whatever grass shows on the ground itself is the terrain's own shading.
- **Things in front of the camera.** The game leaves a placed object that stands right in
  front of the camera out of the view, while it still casts its shadow: in the left view of
  the pixel board (`docs/ryza/views/left.txt`) the roofed post `obj-A21-S06-POST`, 125 units
  from the eye over the ground and under the eye's height, has no draw in the G-buffer pass
  and three in the nearest shadow cascade; in the turn view, 355 units away, it is drawn.
  `barrel` draws it (it fills a quarter of the picture), so the left view's run leaves it
  out of the view by name (`--unseen`). The rule is not read: the camera file has no setting for it (the
  program knows no `camera_ct` name; the field template's note calls it "things in front of
  the camera turn see-through", with a speed and a radius), and whether it fades or cuts is
  not known (one frame, 300 frames after the camera stopped).
  Read so far (pass `0x140728150`, per object `rsi`): drawn (`+0x247`) needs `+0x245` set,
  the float `+0x1cc` (and its parent's at `+0x2c8`) non-zero, byte `+0x249` clear, the
  frustum test (`0x14099a3d0` on the box at `+0x1d0`) and the occluders; the shadow flag
  (`+0x248`) is worked out after, apart from all that (`+0x246`, then the shadow view's planes
  at `[r14+0x8]`). In the left view's frame 2150 (hook on the frustum test) 463 objects reach
  that test, none with `+0x249` set, and the post is not among them: it is stopped before,
  by `+0x245` clear or `+0x1cc` zero, which leaves its shadow. Who sets it is not found: the
  thirteen functions that store to `+0x1cc` and the small setter `0x140a67960` do not touch
  it in that frame (it was set earlier, or it is `+0x245`). Next: a hook over several frames
  as the camera turns toward the post (frames 1850 to 2150), or the stores to `+0x245`.
- **What moves in the wind.** The trees' leaves are not physics: each tree is a field object
  that plays a looping motion (docs/formats.md, "Shared field motions"), 4 or 8 s long,
  turning its leaf joints by up to about a degree; the leaf meshes are skinned one joint a
  vertex and the trunks are not. Barrel plays them (`field::Moving`), and the joints agree
  with the capture's (rotation 0.0000, translation within 0.1). All the field's motions,
  the windmills' too, run on one clock that starts with the field and is a frame ahead of
  the sky's. With them the pixel board went from 5.82 / 4.71 / 7.61 to 3.53 / 3.89 / 5.08.
  How the game does it, for the next thing that moves: the objects are `EXFieldObjectMotion`
  (update 0x1407509e0, player advance 0x1406c2870, evaluate 0x1406c4280 into the animation
  evaluator 0x140ac01b0, joints to world in 0x1406eb430). The engine's swing class with
  named parameters (0x14086f2e0: Gravity, SpringCoef, Damping, WindResistance, ...) never
  runs in the field. The plants lean as the game's now that their cards are anchored at the
  foot (docs/shading.md, "the plants"): with a capture's wind (`BARREL_PLANT_WIND`) the
  left view's grass strip went from 23.5 to 12.3 and no other sway phase comes near (24),
  the board to 3.30 / 3.75 / 4.38. Ryza's cloth pieces (NUNO: skirt, jacket hem, hair) run the
  game's solver, read and matched stage by stage against a capture (docs/formats.md "Cloth
  pieces"): the board to 3.12 / 3.63 / 4.21. The hanging lamps do not move in the game: between the
  capture's frames 2500 and 2550 only the four skinned field objects (Barrel's within 0.1),
  the grass and the water change their draws (scratch `moving.py`).
- **Brightness and bloom.** The brightness and saturation are the game's now (above,
  "Field lighting": the sun's key scale, the colour scale, `SaturationScaleEx`). The bloom
  is the game's bright pass and chain (docs/shading.md, "the frame as the game draws it")
  with the set's `fLumiThreshold` and `fBloomWeight`; its star filter is not done. Only
  `s02_l_hap_main` and `s04_l_sad_main` have their own overrides (`env_<field>.xml`): every
  field takes `env_default.xml` here.

## Ryza in the field

- **Ground and walls.** `--ryza` stands her on the field's heightmap (`<FIELD>.khm`, through the
  shared `walk::Ground::of_heightmap`): at the garden gate she stands on the stepping stones as
  in the game. The fields have no `_col.g1co` as Escha's do; their collision is likely in
  `<FIELD>_coll.bin` (224 bytes in Rasenboden), `.g1pf` or `_le.koc`. The `.koc` walls are
  read (`barrel_ryza::koc`, docs/formats.md) and stop her in a window (run from the garden
  gate toward the house, she stops at the quad across its front, z 28966, her radius short of
  it). Some quads there cross each other in an X behind that one, so not every quad may be a
  plain wall (volumes outlined by quads?). Its sloped and flat quads are not used; `_coll.bin`'s boxes and cylinder are
  (half-sizes or full ones, and where a cylinder starts, not checked).
  The placed models' collision meshes (`_cbvh`, docs/formats.md) are: walls and floors on
  models, and she steps over a wall whose top is within a step (60) of her feet. The boxes and capsules are
  used too (boxes as boxes, capsules as eight-sided prisms). Every shape is put where the
  placement list says within its model (`opl2::CollisionFile`: a rotation and an offset for each
  collision file, docs/formats.md) before the model's placement, as the game does. The hulls (`_cplh`) are read
  and used the same way (Rasenboden: 449 of them; with the meshes, boxes and capsules 1,617
  placed shapes, 293,000 triangles; the game builds 1,628, the 11 more being kinds not read). The camera keeps out of
  the ground behind her (`camera::clear_distance`: pulled in where terrain or a floor comes
  within 30 below to 400 above its line, no nearer than 60, easing back out): banks no longer
  swallow it; it also stops short of the first wall its line passes through
  (`walk::Walls::first_hit`). It has no size: a rock just beside the line can still fill the
  view's side. The game's own camera collision and how it pulls in (pitching up?) are not read. Heights look only at the floors in
  their 256-unit cell (`walk::Ground`): 0.6 µs a height in Rasenboden on the PC, from 1.8 ms
  searching all 282,000 triangles.
- **Walking her** (`barrel ... --ryza X,Y,Z --window`, `barrel_ryza::player`). Stand-ins,
  not read from the game: the lean that starts a run (0.6), how fast she turns and the step she climbs (60). The camera looks 128 above her feet
  (measured: the game's eye and her position at the gate with the field camera's 8° and 400;
  about 0.8 of `chara.xml`'s `CamH` 160, the rule not read). With it
  and the field camera's 45°, 400 and 8° (vertical field of view), the scene at the garden gate
  frames as in the game's still (the fence, the path and its steps, the light pole, the trees in
  the same places); she stands nearer the camera in the save than at the spot used here. Her speeds (walk 71, run 432 units a second) are those her
  feet keep still at (docs/formats.md, "Ryza's speeds"): whether the game moves her that fast
  is unchecked. Her size against the walls (28 by 150)
  and her model's scale (1.04) are the character tables' (`barrel_ryza::chara`). The camera's field of view, distance and pitch are
  the field's (`barrel_ryza::camera`); `at_diff_y` and the look-up adjustment are not placed,
  and the field of view taken as vertical frames the gate as the game does. Setting off plays the gait's
  start motion (10, 20) then its loop, letting go its stop motion (12, 22) then the stand;
  walk and run change loop to loop, each change mixing the last motion out over 0.2 s. How
  the game moves her during the start and stop (here eased in and out) and its mixing time
  are not read.
- **Collision shapes outside their models** (small). With the placement list's per-file
  rotation and offset the stairs, floors, tunnel rocks and light poles lie on their models
  (down the garden stairs' middle the floors are within 8 of the terrain; before, the stair
  mesh lay 105 too high). Left: 26 of Rasenboden's 1,617 placed shapes, 1 of the attic's 9 and
  1 of the title field's 1,212 still lie mostly outside their model's bounds, all boxes or
  capsules on props from older games (`a13-case`, `a14-box2`, `a17_...`, `02bag`, a trash can;
  `BARREL_VERBOSE=1` lists them). Each record's last three floats are not read (zero where
  looked). Not checked by a driven walk: Barrel's Ryza has no scripted input yet.
- **Her shading.** Drawn as a field model with the characters' lighting pass as the program
  has it (`model.frag` flag 128; docs/shading.md, "Ryza DX: the character lighting pass": two
  lights with the rim weight, the occlusion's and the shadow's `min`, the ramp's `u` from the
  two lights' half-Lambert terms), the pass's constants the capture's at Rasenboden's noon
  (a stand-in on the constants board: where the game keeps them is not found). Over her
  pixels the game's lit target is reproduced from its own G-buffer to 2.8%; Barrel's differs
  by its inputs (the board's `chara` lines: mean colour within 1%, the sorted luminance
  distributions 2.8% apart). Her forward passes are built (docs/shading.md, "Her forward
  passes"): the specular ambient with her own block and her metal's diffuse (the red sheen
  on her stockings and boots, which the previous reading took for the hull's), and the hull
  (a dark line on her silhouette and seams). Left: the half-size screen-space reflections
  the specular ambient mixes toward (a stand-in: the boots and the thigh come out a fifth
  too bright, 0.049 against 0.042 and 0.094 against 0.079 in red), the hull's own
  fog (nothing at her distance), the face's and the townsfolk's faces' forward programs
  (draws 2025 to 2031, not read), the normal map in her lighting (her G-buffer program reads it as `rgb x 2 - 1`, normalised, in the
  mesh's tangent frame, not the field models' red x alpha; her draws have no free texture slot for it: the
  mask slot holds the ramp, set 3 her occlusion map, set 4 her specular map; the vertex normal is
  used), the direct specular term (0.4% of her light), the back-light amount, the second light on
  fields where it differs, the hair highlight is laid on fixed (the game slides it with the
  view's pitch). Which override a field uses (most have none: `env_default`) and the point
  lights' share of the pass are not done. A field's gimmicks are drawn as the stage's models,
  as the game does (the notice board at the gate's left; `actors::add_character` without the ramp
  rows). Her own G-buffer programs are not read: her mesh names them by hashes that match no
  shader pack's name (docs/formats.md, "Ryza's character files"); her materials' ramp rows
  (`rampIndex` 3 for her coat's cloth, 1 elsewhere) are read. Her face takes its default expression
  and blinks (every 4 s, a stand-in timing); the other expressions and the eyes' directions
  are not used. Her skirt, jacket hem and hair move by the game's cloth solver (the model's NUNO
  pieces, docs/formats.md "Cloth pieces"). Not done there: the parent's rotation limit and the
  game's smoothing of the parent's acceleration (plain differences stand in), and the other
  characters' cloth. The `.swg` settings (10 records over 652 swing bones, docs/formats.md
  "Swing settings") are a separate system the field does not run for her.

## On handhelds

- **Geometry, not pixels.** Rasenboden from the garden gate draws 2,087 draws and 1.85 million
  triangles, every placed object at full detail. On the PC (RX 9070 XT, 1920x1080, offscreen,
  60 frames, median) the frame is 5.97 ms at the desktop preset and 5.89 ms at the handheld one
  (75 % scale): the scene pass barely changes with the resolution, so it is bound by vertices
  and draws. The field's draw distances count only below the top object-distance setting,
  as in the PC game (`docs/renderer.md`, "Ryza DX: object draw ranges"; `barrel --draw-ranges
  pc|nx` forces them on, and these timings were taken with them on). Choosing the Switch's
  ranges belongs with the handheld preset (a graphics setting, not yet). The timings below
  are from before objects were culled whole and before levels of detail were tied to the
  setting: to be taken again.
  Where the frame goes at the gate (1920x1080, desktop preset, 57 frames, median 5.83 ms): the
  shadow map 1.70 ms, the scene 4.04, the rest 0.08. After view culling, by placement class
  (draws / triangles):

  | pass | near | middle | far | landmark |
  |---|---|---|---|---|
  | scene | 39 / 51k (with Ryza) | 62 / 67k | 146 / 67k | 472 / 430k |
  | shadow map | 72 / 54k | 27 / 21k | 53 / 20k | 142 / 160k |

  The shadow map (3000 units square around her, ±8000 along the sun) draws 255k triangles,
  the scene 615k: about the same cost a triangle, and two thirds of both are landmarks (the
  canyon's cliffs). The game's own shadow range (`disp_range` `shadow`, 300 m) cuts nothing
  here. For the RP6: the landmarks' triangles, then the shadow map's casters. The scene's
  landmark triangles at the gate by model: `a21-common-cliff-block-30m-02` 110k (80 draws),
  `a21-common-desert-cliff-06` 52k, `a21-s06-windmill-01` 35k (95 draws), the houses
  (`a21-s06-house-01`, `-03`, `-04`) 20k to 26k each, `a21-common-mountain-02` 18k, the rest
  under 15k. None of these has a `_mid` or `_low` model, and a `_hi` model holds one level (its
  mesh groups are all LOD 0), so the game's LOD would not lighten them; whether 615k triangles
  and 717 draws are too many for the RP6 is for its timings. The placements'
  classes (docs/formats.md, "Placement lists") put the cost in the landmark class: leaving each
  class out in turn (1080p, 37 frames), near saves 0.36 ms, middle 0.39, far 0.51, landmark 3.9
  of 5.9 (646 cliffs and mountains, 1.26 of the 1.85 million triangles). Landmarks are drawn to
  3000 (the farthest), so distance culling saves little; lower-detail models exist for few
  objects (26 `_mid`, 14 `_low` in Rasenboden's objects, mostly flowers, carts and the woods; the
  cliffs have only `_hi`), and the PC game runs with `use_lod` off. Whether the RP6 needs more
  than the culling we have is for its own timings.
- **On the RP6 itself** (Adreno 740, Turnip, Mesa 26.2.4; 2026-11-14; the aarch64 package
  from `tools/package.sh aarch64 … ryza`, the game read from the device's own install on its SD
  card; offscreen, 1920x1080, the garden gate at noon, 240 frames, median of 237). It runs
  unchanged and draws as on the PC. Desktop preset: 18.8 ms (shadow map 2.67, scene 15.73, glow
  and AA 0.24, post 0.18), 225 MB of textures, 44,015 plants: short of 60 fps. Handheld preset
  (75 % scale, Middle tables, mip bias 1): 14.5 ms (shadow map 1.25, scene 11.83, glow, AA and
  upscale 1.20, post 0.19), 59 MB of textures, 32,980 plants: 60 fps with about 2 ms to spare
  at the gate. The scene pass is three times the PC's and still most of the frame; the
  landmarks' triangles (above) are where it goes next, and a walk through the village (not
  only the gate) is still to be timed.
- **Floors on models**: read, see "Ryza in the field".
- **The files.** A Rasenboden run (with her) reads 22 files from four archives (2026-11-14):
  `PACK00_01.PAK` (3.06 GB: only `shader\texture\chara_rmp.g1t`), `PACK00_02.PAK` (2.04 GB:
  her model, face and motions), `PACK00_03.PAK` (13.69 GB: the field, its objects, environment
  and walls, the sky dome) and `PACK02.PAK` (0.05 GB: the tables under `saves`), 18.8 GB of
  the install's 21. The voice archives `PACK00_04_*` (3.6 GB) and `Data/x64` (6.1 GB: sound
  and UI resources) are not read yet.

## Running the original

- **Without the Steam client**: the executable imports `steam_api64.dll` (callbacks,
  achievements, Steam saves) but no restart-through-Steam check was found among its strings;
  whether it starts without the client is unchecked.
- **Levels of detail.** Rasenboden's object container holds 138 `_hi` models, 26 `_mid` and
  14 `_low`. `barrel` loads all three (`Placement::lods`, `Draw::lod`) and shows each by
  the distance from the eye to its box's centre at the game's High LOD preset (10000, 30000;
  docs/formats.md, "the graphics presets"). In a far view of the village it saves about
  0.08 ms of 7.2 on the PC with 12 pixels changed; the landmarks (cliffs, windmills) have no
  lower levels. The graphics' `foliage` picks the game's general preset
  (`barrel_ryza::presets::Graphics`: full High, reduced Middle, least Low), and the plants and
  these distances come from the tables it names (Middle 8000 / 25000 on the handheld preset).
  The general preset's object draw ranges (`DispRangeType`) take the place of the field's
  own below the top setting and are not used at it (`docs/renderer.md`, "Ryza DX: object
  draw ranges"): the handheld's Middle (7000, 15000, 30000) drops far small objects. The shadow distance (`ShadowDistType`: 1, 0.7, 0.5) scales the
  cascades' ends (1600, 3600, 10000 on High; 0.7 times them on the handheld's Middle): a
  guess at what it scales, as the game's own use of it is not read.
  In the far view 0.5% of the pixels change and the time does not (the far layer is redrawn
  only when it moves). The texture mip bias (`TextureType`) leaves out each texture's largest
  levels at upload (`g1t::Texture::without_top_levels`, kept at 4 × 4 or more): Rasenboden's
  526 textures take 224.9 MB on the desktop preset and 59.3 MB on the handheld's (bias 1), at
  the gate 5.73 against 5.60 ms (PC); the picture softens as the game's Middle does.
- **`SaturationScaleEx`** (`env_override.xml`, docs/formats.md): if Rasenboden is
  `LGT01_DEFAULT_01`, a saturation of 1.6 at noon on top of (or instead of) the environment's
  `vSaturationScale` (1.27, 1.3, 1.2), which matched the noon still worse when laid on as
  saturation (so Barrel applies none). Which set a field uses and where the game lays the
  scale (everything, or the characters: `S999_CHARA_LIGHT`) are unknown; a reference frame of
  a saturated object at noon against Barrel's with each guess would tell. Also the fifth time
  zone, `TIME_ZONE_MIDNIGHT`, and which of the four sets it shows.
- **Plants against the measured constants** (docs/shading.md, "the plants' constants as the
  game sets them"): the sway is now the game's own formula with its measured `g_vAttCache0` and
  `g_fOscPhase` speed; only the gusts' curve over time is not read (the strength holds 9.6, the
  middle of the 8.7 to 10.5 seen), and the plants' own phases differ from the game's since
  Barrel places its plants itself. The plants sample no pigment map at the gate, so their
  teal is not a missing pigment: the pixel side (`cDiff`, `fPtn`, the ambient terms of
  `objectgrasspbs`'s 416-byte block) is not in the saved ring where looked: the lit blocks
  beside the plants' vertex blocks (sun (−0.354, −0.707, −0.612), `litCol` (4.78, 5.0, 3.5),
  `ambLit` (3, 3, 4), `cDiff` 1 or 0.8, `fPtn` (1, 1, 1, 20)) end in matrices, not the ambient
  terms, so they are other programs'. Against the matched still (`rasen-gate-noon.png`) the
  game's plants at the gate are dark green (about 12, 44, 36 in a clean patch) where Barrel's
  are lighter and bluer: too much of the bluish ambient, or a darker card texture. A run that
  reads the block by its own layout (the 416 bytes after `vEye` at +80 with `vATest` at +176)
  would settle it. `emScl` by evening and its
  flicker need a run at evening.
