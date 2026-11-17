# Ryza DX: reading the game's state from the answer key

What the original is doing at a frame, and where each part is read from a ryza-recomp run
(`docs/parity.md` has the tools). Nothing here needs an address inside the game's heap: the
runs repeat frame for frame, so a value read from a run's draws or calls is the value every
run has at that frame.

| State | Read from | How |
|---|---|---|
| Camera | the capture | `barrel-parity state <draws> <a-b/step> <index> <model>` prints the camera's position per frame, from the first G-buffer draw's `mW2P` (the point whose clip x, y and w are 0); `barrel-parity draws --vars mW2P` gives the whole matrix |
| Player | the capture | the same command: the mean bone translation of the skinned draws whose textures are in the model's folder (`pc00a` is Ryza). World units are the game's (100 a metre) |
| Event and step | the command log | a scenario with `HOOKS=scenarios/hooks/commands.hooks`, then `tools/cmdlog.py`: every command each runner takes and its frames; the event's own runner's `w1 N` words are its step marks |
| Field | the command log and file reads | the `q` runner's loads (`q5` a character, `q4`/`q9` a map) and `barrel-tool pak-reads` on a run with `RYZA_TRACE_FILES` |
| Input | the scenario | its `PAD` lines: the run has no other |
| Frame time | fixed | 1/60 s on the virtual clock (`docs/ryza/open.md`, "The answer key's runs repeat exactly") |

Scenarios that give them (ryza-recomp `scenarios/`): `title-commands` (the title's event),
`new-game` (the opening's first event, commands and a capture), `load-rasen` (the field from a
save) and `walk-rasen` (the same, then the left stick up for two seconds: the camera moves
about 10 units a frame and stops at frame 2020).

Not read yet:

- **The player's position in memory.** The character runner's own object (its first 2 KB) does
  not hold the position `s2` sets, in metres or in the script's hundredths; the character it
  drives is behind a pointer not followed. The capture gives the position without it.
- **The player alone.** The model's folder also matches other draws that share its textures
  (62 draws at the save's spot, 28 after walking away from them), so the mean moves when they
  are culled. To do: keep the draws of one index-count set, or the root joint only.
- **The world clock and weather.** Not looked for; the field's lighting constants in the
  capture follow from them.
- **Facing.** In the bone matrices' rotation; not printed.
