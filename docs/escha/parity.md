# Escha parity scoreboard

Each row is a scenario. Its game capture comes from `../escha-recomp/scenarios/<name>.sh`
(`tools/scenario.sh`), and Barrel's trace from the matching Barrel run, diffed with
`barrel-parity diff` (docs/parity.md). Mismatches count the diff's lines, game frame against
Barrel frame.

| Scenario | Game frame | Barrel | Mismatches | Notes |
|---|---|---|---|---|
| boot-title | 1400 | `--title`, frame 0 | 1 | the "press" plate's blink (alpha over time) |
| load-field | – | – | – | the recomp traps entering the world map after loading (missed vtable functions; being fixed) |
