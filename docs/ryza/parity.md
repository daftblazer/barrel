# Ryza parity scoreboard

Each row is a scenario. Its game capture comes from `../ryza-recomp/scenarios/<name>.sh`
(`tools/scenario.sh` there, on a 1920 × 1080 headless display), Barrel's trace from the matching
Barrel run, diffed with `barrel-parity` (docs/parity.md). Textures are named through
`../ryza-recomp/work/traces/textures.tsv`. Mismatches count the diff's lines, game frame
against Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1380-1420 | (no Ryza title yet) | not compared | the reference only: the title (a21_title.g1t, mainfont01_hd1080) over the charatest backdrop (pc00a, pc02a, pc03a), 1,924 draws into a 4-target G-buffer |
| title-cycle (title, no input) | 1400-1800 every 100 | `--title --title-at <(frame - 1140) / 60>` (1700: 9.30, 1800: 10.9667: the capture's clock runs a frame or two off 60 Hz there, the prompt's pulse shows it), frame 0 | 0 at all five (93 quads) | logo, prompt and its blink, version line with its outline, copyright, from uil/uis_gen_a21_title.xml and uia_common.xml; left: the fade from black (a full-screen fill, not a 2D quad), the menu, the 3D backdrop, the logos / loading / autosave screens before it |
