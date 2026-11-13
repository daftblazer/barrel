# Open problems

Everything Barrel is stuck on, has not worked out yet, or knows to be wrong, in one place, so
any of it can be picked up cold. Each entry says what is known, what was tried (addresses are in
the English Escha & Logy DX executable, `Atelier_Escha_and_Logy_EN.exe`), a suggested way in,
and how to check a fix. Entries leave this file when they are solved (the answer goes into the
topic docs) and are added as soon as something stalls.

Oracle runs (the real game under Proton, with memory reads and a virtual pad:
`tools/oracle/README.md`) are allowed: brief, the game stopped right after each, never with the
player's own save folder. Many entries below are marked **live read**: one short oracle run would
likely settle them where static reading failed.

## Saves

- **Writing the game's own `GAMEDATA` saves.** Reading works (`save::decode`, the record tree,
  `save::summary`), and `save::encode` reproduces the coding. Missing: a record writer (the
  parser loses `party`'s count prefix and whether a length was little- or big-endian), and a new
  game's full state as a template for fields Barrel does not model. Barrel saves its own text
  format meanwhile (`session::Session::save`). Check: a written save loads in the real game.

## Field play

- **Arrival point from the world map.** Barrel puts the player at the area's own world map exit
  (`WORLDMAP_CHANGE_EX`), a stand-in (`docs/engine.md`). Tried three times: the change-field
  state `clsFMStateChangeFieldMap` (vtable `0x1407729d8`, fade machine `0x14007a090`/`0x14007a1a0`),
  `0x140079400` → `0x140214f20` → the field load `0x140076760` / `0x140081080`; candidates
  `0x1402b6d43`, `0x1402ecad1` were bounds checks. Way in: read `0x140076760` whole for where it
  places the player, or **live read** the player's position right after arriving in
  `AREA_01_01` from the Town Entrance.
- **Play camera follow curves.** `play_camera::Follow` is rebuilt from `_def.xml`'s comments, not
  the code (`docs/engine.md`). The setting's tweens sit at `CameraCFieldMap` +0x150 (bumper
  speed), +0x168 (max distance), +0x1b0 (zoom time), +0x1c8 (zoom distance), written at
  `0x1401e7321`; their readers were not found (two tries; `0x140234570` is the jump timers, not
  the camera). Also not done: blending between camera areas' settings (Barrel snaps). Way in:
  **live read** the camera's eye and target each frame while the player runs and stops.
- **Field NPC spawns in story events (characters 10048 and up).** Third lookup `0x140270230`
  (list at manager +0x50 by +0x78); leads `0x14017f13e`, `0x1403386af` unchecked. Parked after
  three tries. The stage camera of some story events depends on it.
- **Town people.** Their source is found (`clsFMDataNpc`, placements `0x1408c2cd0`, 841 × 0x98;
  conditions `0x1408aab70`); the 1,494 × 0x40 records at `0x1408ab750` are unread, and the
  viewer does not yet place them in free play.
- **Gathering details.** How many picks one gathering makes (passed in by `0x14023c210`'s caller,
  not read; Barrel uses 1); the reach at which a point can be used (Barrel uses its size); the
  two "table level" item units `0x140dfc8b0` / `0x140dfc8d4` and the field gathering state
  `0x1410783b0`'s setter (`0x140283d16` copies it from an effect object).
- **Ambience volume units**: the map record's ambience volume (0.05 to 0.2) is taken as a linear
  gain, unverified.
- **Gimmick +0x38.** Its reader was not found (not the constructor `0x1402712d0` nor the list
  walk `0x140276190`).
- **Defeated field enemies.** Which enemies stay down after a battle (`0x141087bd0` per map):
  **live read** after one fight.

## Battle

- **Formation / battle areas.** Where the party and enemies stand: battle areas decoded, but the
  start formation needs the battle setup the field encounter builds (who writes the scene
  request +0x40). Scans for the party's home positions (±692.8) found nothing; the formation
  writes through `[fighter+0x10]+0x3d8` (a model's vector pointers). **Live read** during a
  battle: `[f+0x10]+0x3d8`.
- **Monster sizes.** Size +0x358 = `[0x141077ed0]` + id × 0x80 + 0x64; its filler is not found.
  **Live read** `0x141077ed0`'s records once a battle has loaded.
- **Item power.** The builder of an effect entry's +0x18 for action kind 1 was not found (three
  tries; itemData `effect` is row +0x40).
- **Item level in battle** (fighter +0x248, from a command copied whole): setter not found.
- **Battle time spent in the field** (`BtlReturn` +0x18, vtable `0x1408048c0`): its writer is not
  found.
- **Reaction queue timing.** When a queued boss reaction acts, against the battle state machine
  (`0x1401d4308`..`0x1401d5d4f`).
- **Missing rules for a full battle** (stage 4): traits, states, support attacks, items in the
  loop; see `crates/game/src/duel.rs`'s header for what the partial battle leaves out.

## Synthesis

- **Slot initial values** (+0x420 / +0x42c of the 0x4e0-byte state, set once at synthesis
  start): five static tries found only UI. **Live read** at the start of a synthesis.
- **Property point spending** (+0x3f8): only UI reads it; a trait's `point` is likely its PP
  cost. Needs a synthesis with 2 PP or more in a run.
- **Prices** (`q = trunc(quality / 3 × 2) + Σ trait +0x28 + Σ effect +0x24`, columns unsettled);
  buy and sell not told apart.

## Events and story

- **Event camera yaw sign.** `EVENT_MM01_OP1_111` needs the sign flipped (Escha 2° off centre)
  while `EVENT_MM08_AS4_130` looks right as is. Suspects: `t19` field camera data, operand 4's
  angle (`0x1401ec8f0`), a per-field base yaw; read `CameraCFieldMap`'s `t20` path to the eye
  (`0x1401e7a10`, jump index 0x11).
- **`TOWN_03` event scenes** (`EVENT_MM08_AS4_110`) render from inside the ground with nobody in
  view; the terrain is a flat placeholder. Next idea: each object's bounds against the
  characters; whether the event starts in another field before its `q12`.
- **Event start order among equal priorities**: the game sorts with MSVC's introsort, which does
  not keep order, so a random pick may differ from Barrel's.
- **Story voice line mapping**: which voice clip goes with which message line when not given by
  the first-clip table; voice paths sit only in a string pool. Parked twice.
- **`u79`** (in `MM08_AS4_*` events) unread; **`u145`** (endings) in a jump table, skipped.
- **Assignment counters** per condition kind (manager +0x348, built by `0x14002d4d0` /
  `0x14002dad0`): the per-condition factory is not found.
- **Request board points**: a monster slot paid 2 where 5 was expected; no other writer of slot
  +0x20 found.

## Rendering

- **Lighting gaps.** Sky dome too dark (lighting it as a field model made it darker: the MapSTD
  pixel program's extra terms are missing); outdoor ambient; lamp glow (no emissive parameters,
  the glow must come from a bright pass); cloud shadows; the glow combine. Parked four times.
- **Distant view too pale** (buildings and bushes far off): the real fog rule for the distant
  view is unknown.
- **A house front too bright** (`TOWN_10`): parked three times; next is per-object vertex colour
  checks (`BARREL_HIDE` + `g1m-materials`) against the game's pixels.
- **Terrain blending** (`.ktf` patch tree): which blend map and channel weights which layer per
  patch. Four tries; the leaf integers and layer → channel mapping need the section readers
  (`0x14066f6d0` and deeper). The viewer blends the first two maps over the first six layers,
  wrong on most areas.
- **Fog and shadows**: the conversion of the stored fog values (`0x1404d6df0`) into shader
  parameters was not found.
- **Draw priority of pieces** (`xS1`–`xS3`, the 1024-bucket sort `0x1403387e0`): the writer
  among 100+ candidate stores can't be told statically. **Live read** of the draw queue's +0x20
  buckets during a field frame.
- **Particle effects** (`eff_pack_list.xml`, `.g1e`): not decoded; character-bound effects
  (`0x1401f6cf0`) wait on it.
- **Plant sway amplitude** (`v_shaker`) and tree springs (`n_shaker`): implemented, units not
  verified against the game.

## Sound

- **Sound effect id layer**: event sound ids (≤ 244) go through a group/cue layer at the bank's
  entry 0, not read.

## Other games

- **Sophie 2 `s40` op 3**: the motion object's +0x140 / +0x148 vtable +0x40 is unidentified.
- **Sophie 2 / Ryza PAK second-layer key**: open (the original queue's item 7).

## Waiting on the user

- **Movies**: whether and how to play them is the user's call; not implemented.
