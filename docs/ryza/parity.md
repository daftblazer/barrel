# Ryza parity scoreboard

Each row is a scenario. Its game capture comes from `../ryza-recomp/scenarios/<name>.sh`
(`tools/scenario.sh` there, on a 1920 × 1080 headless display), Barrel's trace from the matching
Barrel run, diffed with `barrel-parity` (docs/parity.md). Textures are named through
`../ryza-recomp/work/traces/textures.tsv`. Mismatches count the diff's lines, game frame
against Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1380-1420 | (no Ryza title yet) | not compared | the reference only: the title (a21_title.g1t, mainfont01_hd1080) over the charatest backdrop (pc00a, pc02a, pc03a), 1,924 draws into a 4-target G-buffer |
