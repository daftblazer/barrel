# Open problems

Each game keeps its own list of what Barrel is stuck on, has not worked out yet, or knows to be
wrong; this file holds the problems of the shared engine (whatever game shows them) and of the
games not started yet. Each entry says what is known, what was tried, a suggested way in, and
how to check a fix. Entries leave when they are solved (the answer goes into the topic docs) and
are added as soon as something stalls.

| Game | Open problems | Also |
|---|---|---|
| Escha & Logy DX | `docs/escha/open.md` | `docs/escha/remaining.md`: what is left for the whole game |
| Ryza DX | `docs/ryza/open.md` | |

## Engine

- **A GPU hang on the RP6** (the driver's hangcheck, `VK_ERROR_DEVICE_LOST`), once, in Escha's
  opening about 0.7 s after it moved from the talk with Clone to `TOWN_10`, right after three
  camera cuts in a row; not reproduced offscreen or in play since. Way in: a camera log
  (`BARREL_SHOTS=1`) and the kernel's GPU dump (`/sys/class/devcoredump`, readable for 5 minutes
  after the hang) the next time it happens.
- **Per-draw name checks** (hiding, sway groups, event objects) run on every draw every frame;
  they could be decided once a scene.

## Data formats

- **Sophie 2 and Ryza PAK second-layer key**: open (the original queue's item 7).

## Games not started

- **Sophie 2 `s40` op 3**: the motion object's +0x140 / +0x148 vtable +0x40 is unidentified.
