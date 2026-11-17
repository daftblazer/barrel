# Ryza parity scoreboard

Each row is a scenario. Its game capture comes from `../ryza-recomp/scenarios/<name>.sh`
(`tools/scenario.sh` there, on a 1920 × 1080 headless display), Barrel's trace from the matching
Barrel run, diffed with `barrel-parity` (docs/parity.md). Textures are named through
`../ryza-recomp/work/traces/textures.tsv`. Mismatches count the diff's lines, game frame
against Barrel frame.

The captures are made on ryza-recomp's exact virtual clock and repeat frame for frame, so a
row's time is `(frame - start) / 60` with nothing fitted. Frame numbers in the other docs that
come from the first capture (the title at 1140, the notice at 278, the 3D frame 1400) are 265
higher than this one's (875, 11 after its two-frame earlier notice, 1135).

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1115-1155 | (no Ryza title yet) | not compared | the reference only: the title (a21_title.g1t, mainfont01_hd1080) over the charatest backdrop (pc00a, pc02a, pc03a), 1,924 draws into a 4-target G-buffer |
| title-cycle (title, no input) | 1135-1535 every 100 | `--title --title-at <(frame - 875) / 60>`, frame 0 | 0 at all five (93 quads) | logo, prompt and its blink, version line with its outline, copyright, from uil/uis_gen_a21_title.xml and uia_common.xml; left: the fade from black (a full-screen fill, not a 2D quad), the menu, the 3D backdrop, the logos / loading / autosave screens before it |
| title-menu (button at 1235, then down, down, up, down held) | 1232-1620 | `--title --title-at <(frame - 875) / 60> --title-press 360 --title-keys ...`, frame 0 | 0 at nine (1235 to 1580: 93 to 99 quads) outside the sparkles | the prompt's fade, the items' staggered slide-in, the cursor's spin in perspective, its props' turning, the selected item's pulse, moves |
| title-menu-wrap, title-menu-saves | 1298-1550, 1232-1430 | as above; with a save `--title-saves` and the title from frame 543 | 0 at 1540; 0 at four with a save (100 quads) | going round the ends, a held direction's repeat, five items with Load Game selected |
| title-cycle (boot sequence) | 35-825 (11 frames) | `--boot --boot-at <(frame - 11) / 60>`, `PARITY_TURNED_TIES=left` | 0 at ten; 825: 1 (one glow 2/255 off, docs/ryza/open.md) | "Creating system data..." notice, Koei Tecmo and Gust logos (0.5 s fades), the autosave window with its 8-orb spinner, then the title (1135: 0); from the uis/uil/uia XMLs and STR_UI_0896; fitted: the schedule's frames, `LOGO_HOLD`, `LINE_STEP`, `GLOW_DRIFT` (see docs/ryza/open.md: the capture's clock) |
