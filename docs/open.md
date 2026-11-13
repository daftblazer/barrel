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

## Issues found during manual testing
- Escha's hat and puffballs don't move correctly with the character, they float above Escha
  - Fixed (2026-11-13): the hat's submeshes have a bone palette (joint 18, under the head) but no
    per-vertex weights, and were drawn unposed; such submeshes are now rigid on their palette's
    joint, so the hat turns with her head. To check in play whether it still sits a little high.
- English spoken dialogue during initial cutscene with Escha and Clone gets cut a little short. Some audio during scenes cuts short.
  - Likely the next line starting while the voice still played: lines ran on a reading time
    (1 s + 0.08 s a character) shorter than some clips. Lines now wait for A, so a voice plays
    to its end unless skipped; to check in play. Sound effects: fixed (2026-11-13) one more cause,
    the sound output was reopened at every scene change (one event into the next, or into free
    play), cutting off whatever still played; it is now kept for the whole run (only the music
    and ambience stop). To check in play.
- Scenes normally are progressed by pressing the A button on the controller
  - Done (2026-11-13): in a window each line now waits at its end for A / Enter, and A before
    then ends the line (and its voice) at once. Auto mode (Y / T) goes on by itself after the
    line's voice. The game's own auto-mode button and timing are not read yet.
- Some animation transitions don't transition smoothly
  - Partly fixed (2026-11-13): a motion switch during a running blend dropped the half-done
    blend (a 30-unit snap in the opening), and a gesture's next division of the same motion
    switched with no blend at all; both now blend out of the pose last shown, in events and
    for her walk / run / stand. If you still see a pop, note where (scene and moment).
- After each dialogue, the text box flickers. Likely disappearing for a frame then reappearing. 
  - Fixed (2026-11-13): pressing A moved the event a tick past the line's end, and the next
    line came a tick later, so the box was gone for a frame. The window now stays up, empty,
    for 0.6 s after a line (bridging that tick and the short pauses between some lines); a
    stand-in until the game's own window rule is read (`docs/engine.md`).
- Character select on new game looks wrong
  - Fixed (2026-11-13), checked against a screenshot of the game's confirm step: mirrored pieces
    (node kind 2: the bottom parchment's right half, the right rope) were drawn unmirrored;
    the confirm step now hides the cards and arrows and shows the chosen figure in colour,
    moved to the middle; the desk picture, the tags' lit cut, the question line and centred
    texts (kind 22) as the game has them. Left: the Yes tag's tilt, Logy's confirm figure place
    (no screenshot of it).

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
- **Followers on escort requests (unconfirmed).** On a plain field walk only the lead is shown
  (oracle run), but a character may follow the lead while an escort-style request or story
  quest is active (the user's recollection, not checked). Way in: look for request kinds or
  event ops that attach a character to the player; **live read** during such a request once
  one is reachable without spoilers past the opening.
- **Defeated field enemies.** Which enemies stay down after a battle (`0x141087bd0` per map):
  **live read** after one fight.

- **Field HUD details.** The time dial's ten segments (part 0x8d, layout 609) are drawn as the
  tenths of the day still free (the game's screenshot shows them dark in an area: likely the
  other way round, or time of day); the assignment badges (603), the "Free Time" tab (604),
  the arrows (602), the small count (part 0xe5), the gathering gauge and notices are not shown.
  The button list's place (the layout says y 792) and the text size are measured from one
  screenshot. Way in: the HUD's update code (the layout 625 owner; find who loads layout id
  625 = 0x271) or a **live read** with the day's time partly spent.

- **Which title layout.** Layouts 958 (`a15_title.g1t`) and 1056 (`a15_title64.g1t`) hold the
  same title menu at different places; Barrel uses 1056 (958's last entry ends past the
  screen's foot). The cursor's place and the menu's motion are ours. Way in: who loads layout
  0x420 / 0x3be, or a screenshot of the real title menu.

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

- **`TOWN_03` event scenes** (`EVENT_MM08_AS4_110`) render from inside the ground with nobody in
  view; the terrain is a flat placeholder. Next idea: each object's bounds against the
  characters; whether the event starts in another field before its `q12`.
  Parked: checks against the running game stay within the opening for now, and this event
  comes later.
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

- **Lighting gaps.** Checked against the reference recording of the opening (2026-11-13): the
  lamp glow matches (Colland's office, `EVENT_MM01_OP1_121` at 9.5 s: lamp shade 253 against
  243, cabinet, floor, books and Marion's face within 1 to 4 of 255); the ambient and the
  glow match since the glow's real threshold; the sky dome since its own-distance fog (within
  about 20 in green and blue at the top of `TOWN_09`'s sky). Left: the cloud shadows' phase
  (the drift starts at the field's load in the game; not reproducible), the terrain's
  specular term (not drawn), `cDiff`'s alpha.
- **Distant view too pale**: fixed (`TOWN_04`'s plaza and haze match the recording).
- **Terrain details.** The blend is read and drawn (docs/formats.md, "Patches and their
  draws"; checked by eye on AREA_01_01, AREA_03_01 and AREA_04_01, not yet against the
  game's pixels). Open: whether a draw's layer numbers count in a per-patch texture list, the
  draws' third byte, the stored normals, the tiles' alpha as specular strength, the coarse
  patches at a distance, and `battle`'s terrain (it does not read as a mesh).
- **Fog and shadows**: the conversion of the stored fog values (`0x1404d6df0`) into shader
  parameters was not found.
- **Draw priority of pieces** (`xS1`–`xS3`, the 1024-bucket sort `0x1403387e0`): the writer
  among 100+ candidate stores can't be told statically. **Live read** of the draw queue's +0x20
  buckets during a field frame.
- **Particle effects** (`eff_pack_list.xml`, `.g1e`): not decoded; character-bound effects
  (`0x1401f6cf0`) wait on it. Parked: the opening uses none (no `s38`–`s114` in the
  `MM01_OP1` events, and `barrel opl2` lists no placed effects in any `TOWN_*`; the first fields
  with some are `AREA_06_*` (smoke, eruption particles) and `AREA_08_*` (`287_FLAME_01`)), so
  there is no reference within the opening to check a decoder against.
- **Plant sway amplitude** (`v_shaker`) and tree springs (`n_shaker`): implemented, units not
  verified against the game.

## Sound

- **Sound against the reference recording**: voice and music levels match since
  `OPTION_VOLUME` and `VOICE_GAIN` (`docs/engine.md`, "Levels"; measured, the option table not
  read); the effects' level is not measured apart.
- **Sound effect id layer**: event sound ids (≤ 244) go through a group/cue layer at the bank's
  entry 0, not read.

## Other games

- **Sophie 2 `s40` op 3**: the motion object's +0x140 / +0x148 vtable +0x40 is unidentified.
- **Sophie 2 / Ryza PAK second-layer key**: open (the original queue's item 7).

## Waiting on the user

- **Movies**: whether and how to play them is the user's call; not implemented.
