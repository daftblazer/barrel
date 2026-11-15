# Escha parity scoreboard

Each row is a scenario. Its game capture comes from `../escha-recomp/scenarios/<name>.sh`
(`tools/scenario.sh`), and Barrel's trace from the matching Barrel run, diffed with
`barrel-parity diff` (docs/parity.md). Mismatches count the diff's lines, game frame against
Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1400 | `--title`, frame 0 | 0 (+1 phase) | the one difference is the "press" pulse's phase (Barrel at 0 s, the game 15 s in); its curve already matches (2.4 s, low 0.2). Animated items need time alignment |
| load-field (area map) | 3000 | `--load barrel-user.save --area-map`, frame 0 | 1 | the field gauge reads 45 (the save) where the game draws 44; rest at parity |
