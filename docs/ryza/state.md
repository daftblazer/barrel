# Ryza DX: reading the game's state from the answer key

What the original is doing at a frame, and where each part is read from a ryza-recomp run
(`docs/parity.md` has the tools). Nothing here needs an address inside the game's heap: the
runs repeat frame for frame, so a value read from a run's draws or calls is the value every
run has at that frame.

| State | Read from | How |
|---|---|---|
| Camera (per frame) | a dump | the view at heap `0x7fff6b9ae070` with `RYZA_NO_ASLR=1` (the address repeats only without randomisation): eye at +0x100, forward axis at +0xf0 negated; ryza-recomp `tools/walkstate.py` prints both with her state. The dump at Present f holds frame f - 1's camera (the capture's eye to 0.008) |
| Camera | the capture | `barrel-parity state <draws> <a-b/step> <index> <model>` prints the camera's position per frame, from the first G-buffer draw's `mW2P` (the point whose clip x, y and w are 0); `barrel-parity draws --vars mW2P` gives the whole matrix |
| Player | a dump | her feet at `0x141b7a590` (three floats, game units: 100 a metre) and her facing at `0x141b7a5a0` (a unit vector x, y, z; yaw = atan2(x, z)), a static copy the game updates every frame (`DUMP_AT="a-b;0x141b7a580+0x40"`). Copies in metres at `0x141a66228` and `0x141ac283c` (not followed). The capture's mean bone translation (`barrel-parity state`) agrees to the motion's sway |
| Event and step | the command log | a scenario with `HOOKS=scenarios/hooks/commands.hooks`, then `tools/cmdlog.py`: every command each runner takes and its frames; the event's own runner's `w1 N` words are its step marks |
| Field | the command log and file reads | the `q` runner's loads (`q5` a character, `q4`/`q9` a map) and `barrel-tool pak-reads` on a run with `RYZA_TRACE_FILES` |
| Input | the scenario | its `PAD` lines: the run has no other |
| Frame time | fixed | 1/60 s on the virtual clock (`docs/ryza/open.md`, "The answer key's runs repeat exactly") |

Scenarios that give them (ryza-recomp `scenarios/`): `title-commands` (the title's event),
`new-game` (the opening's first event, commands and a capture), `load-rasen` (the field from a
save) and `walk-rasen` (the same, then the left stick up for two seconds: the camera moves
about 10 units a frame and stops at frame 2020), `walk-30` (P3's 30 s walk: runs, a walk, turns,
the camera turned and pitched; her state and the camera every frame, `RYZA_NO_ASLR=1`).

Not read yet:

- **Who writes the player's copy.** `RYZA_WATCH` reports only a page's first write each frame
  (here `0x141b7a850`'s); the movement code is not found from it.
- **The player's position in memory.** The character runner's own object (its first 2 KB) does
  not hold the position `s2` sets, in metres or in the script's hundredths; the character it
  drives is behind a pointer not followed. The capture gives the position without it.
- **The player alone.** The model's folder also matches other draws that share its textures
  (62 draws at the save's spot, 28 after walking away from them), so the mean moves when they
  are culled. To do: keep the draws of one index-count set, or the root joint only.
- **The world clock.** The time of day and weather in force are read (a hook on
  `0x14071e970` and `0x14071e9d0`, the environment's setters: the save's Rasenboden loads at
  weather 0, time 1, noon); the clock that moves them is not.
