# Escha & Logy DX: open problems

Everything Barrel is stuck on for Escha & Logy DX, has not worked out yet, or knows to be wrong, in one place, so
any of it can be picked up cold. Each entry says what is known, what was tried (addresses are in
the English Escha & Logy DX executable, `Atelier_Escha_and_Logy_EN.exe`), a suggested way in,
and how to check a fix. Entries leave this file when they are solved (the answer goes into the
topic docs) and are added as soon as something stalls. Problems in the shared engine, whatever
game shows them, are in `docs/open.md`.

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
- (2026-11-14) Battle command row: drawn from the game's pieces, but the input is still Barrel's (Up/Down through Attack and a text list of skills, Left/Right the target); Move, Wait, Change and Run are shown, not given; Item for the alchemists not in the row yet; kind-36 flipbooks at their first frame.
- (2026-11-14) Turn order bar: shows the order at the member turn waiting (`Turn::order`), not as the events play out.
- (2026-11-14) Battles don't really work yet: Barrel's battle is an auto-battle with a stand-in
  playback and a text command list (stage 17 in the plan).
- (2026-11-14) The lead is drawn 3.2 times on the maps by `0x140388820`, Barrel's fitted 1.75 (area)
  and 3.7 (world) say otherwise; which model and base scale the map uses.
- (2026-11-14) The maps (stage 15): the scroll and label plates and event points are the game's art now; left: the cursor's float height (within its bob range in the recordings).
- (2026-11-14) Title music: Barrel plays track 1 (`sy_title01_piano`, the piece the recordings
  have at the title) once, as its KOVS header says (+0x10 = 1, 58 s). Open: whether the PC plays
  track 1 or 168 (the same piece as `_2ch`: both correlate 0.91 to 0.92), and what follows when
  it ends (track 216 `sy_op_loop`, "タイトル画面ループ用", the title screen's loop, is the
  likely one); a recording of the boot and of the title left idle past a minute answers both
  (a live read of the current BGM number too).
- (2026-11-14) The boot recording is still to make: the first try (2026-11-14) captured another
  gamescope's stream (record.sh then took the first stream named gamescope; it now takes its own
  run's) and was deleted. Make it when no other game runs under gamescope.
- (2026-11-14) No intro movie on startup (movies: native decoders, chosen 2026-11-14).
- (2026-11-14) No company logo splash screens before the title. Found: stills 5 (Koei Tecmo) and 4 (Gust), docs/formats.md; shown by play/splash.rs, order and times confirmed by the escha-boot recording. Still open: whether a button skips them (Barrel: any button does).
- (2026-11-14) Title music onset: on escha-boot it is first heard at 15.1 s, 1.1 s before the title fades in at 16.25, inside the slot of the movie Proton could not play. Track 1 (8ch, 1.1 s of silence first) started at 14.06 s or track 168 (`_2ch`, 0.2 s of silence) at 14.93 s both fit; correlation leans to 168 (0.68 against 0.64 over 5 s) but not decisively. Barrel starts track 1 with the title. Settle both with a recording where the movie plays (or a hook on the BGM call) once stage 18 can play movies.
- (2026-11-14) The pre-title movie `Movie_EN/avantitle.wmv` (96 s, VC-1 1920 × 1080 with WMA Pro sound) plays after the logos and before the title; Barrel goes straight to the title. Needs a VC-1 / WMA Pro decoder (Proton has none either: its recording shows 4 s of colour bars). Same for `opening*.wmv`, `sequence_*.wmv` and `ending.wmv`.
- (2026-11-14) The title's fade in from white is in (title.rs `white_at`, every time the title screen starts in a window; the recording shows only the boot's). Whether a return to the title from play fades the same way is open.
- (2026-11-14) Layout nodes of kind 4 are scaled by word 5 (percent; values 30 to 185 across the table, 0 on 18 nodes, likely unscaled). Only the title's logo (80) uses it yet (hud `Element::Scaled`); the other kind-4 nodes drawn through `Element::stored` still ignore it: check each screen they appear on before applying it there.

## Saves

- **Writing the game's own `GAMEDATA` saves.** Reading works (`save::decode`, the record tree,
  `save::summary`), and `save::encode` reproduces the coding. Missing: a record writer (the
  parser loses `party`'s count prefix and whether a length was little- or big-endian), and a new
  game's full state as a template for fields Barrel does not model. Barrel saves its own text
  format meanwhile (`session::Session::save`). Check: a written save loads in the real game.

## Field play

- **Play camera follow.** Read live (`docs/engine.md`, "Play camera follow"): the look point
  sits on the player. Left: what `bumper_follow_speed` / `bumper_max_distance` change, the
  eye (look outside `CameraCFieldMap`'s first 4 KB, or follow its pointers at +0x130 and
  +0x260..+0x288), the zoom blend's way back, blending between camera areas (Barrel snaps).
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

- **Field HUD details.** The time dial is fixed (2026-11-13, the running game: its segments
  light as field time is spent, clockwise from the top; `crates/render/src/hud.rs`). Left: the
  dial's hand (layout 610, a turned sprite), the assignment badges (603), the "Free Time" tab
  (604), the arrows (602), the small count (part 0xe5); the field effect gauge's choice panel
  (614's cut 5 and 611 to 613, open in the field at 100: what it offers and how one is picked),
  its glows (0x6f), and when the game clears it (`0x140283070`: the world map shows 0% with no
  notice after the Southern Ruins at 45; callers `0x14016ec80` for scene kinds 12, 13, 94, 95,
  `0x14028cc34`, `0x140387980`, `0x1403a1313`; Barrel shows 0 on the world map and keeps the
  session's value) and whether a town shows it (Barrel: no); the gathering's time is
  confirmed (a live read of the carried tenths, `0x1410782e4`: 0 on arrival, 6 after a battle
  and a gathering, 11 → a new day with 1 carried after a second gathering, as `days` has it).
  The button list's place (the layout says y 792) and the text size are measured from one
  screenshot; so are the assignment panel's (+20, +8) rest and its texts' alignment ("Left"
  starts at its node, the days left are centred and ~15% larger: no node word says so; Barrel
  keeps their size equal). The request cards under the date: how many slots show (six on the
  user's save at rank 3; Barrel fixes six) and the cards of a failed request or of other
  kinds (cuts 2 and 4 unseen). Way in: the board's take check (who writes `quest/player`
  slots, near `0x14033ab30`) for the slot limit. The area map shows the assignment panel, the badge row and the gathering gauge but
  no dial. Way in for the rest: the HUD's update code (the layout 625 owner; who loads layout
  id 625 = 0x271).

- **Which title layout.** Layouts 958 (`a15_title.g1t`) and 1056 (`a15_title64.g1t`) hold the
  same title menu at different places; Barrel uses 1056 (958's last entry ends past the
  screen's foot). The cursor's place and the menu's motion are ours. Way in: who loads layout
  0x420 / 0x3be, or a screenshot of the real title menu.

## Battle

- **Item power.** The builder of an effect entry's +0x18 for action kind 1 was not found (three
  tries; itemData `effect` is row +0x40).
- **Item level in battle** (fighter +0x248, from a command copied whole): setter not found.
- **Battle time spent in the field** (`BtlReturn` +0x18, vtable `0x1408048c0`): its writer is not
  found.
- **Reaction queue timing.** When a queued boss reaction acts, against the battle state machine
  (`0x1401d4308`..`0x1401d5d4f`).
- **Run's escape value: top speed or first monster.** Read live as 19 with monster speeds 19,
  19, 15, 15, so "the fastest monster" and "the first monster" both fit; a battle whose first
  monster is not the fastest decides it. Where `tries` (`0x141077dcc`) is reset is not read.
- **Missing rules for a full battle** (stage 4): traits, states, support attacks, items in the
  loop; see `crates/game/src/duel.rs`'s header for what the partial battle leaves out.

## Synthesis

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
  Parked (2026-11-13, stage 7): a live read first needs the queue object. At the queuing call
  (`0x1404d8869`) it is `[r13]` with r13 = rbp + 0x40, rbp the argument of the function at
  `0x1404d8525`: follow that function's callers to a global, then read the queue's buckets
  (+0x20, total +0x1020) and sorted entries during a field frame.
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

## Waiting on the user

- **Movies**: to be played by native Rust decoders (ASF, VC-1, WMA Pro; the user's choice,
  2026-11-14); not implemented yet.
- World map colour grading: our world map is warmer and lighter than the recording (colseit-arrival 1:25: the Old Highway's green `_off` mark draws olive-yellow, the clouds cream); the map shader's light or tone, not the marks' choice.

- Battle turn bar spacing (stage 17; DONE in Barrel 2026-11-14 for the recorded case, the
  start order by speed an inference; READ: the gaps are the timeline's free slots, 16 entries
  in all, docs/tables.md "Turn order"; to do: Barrel's Timeline keeps the free slots, and how
  the battle start sets the waits 0, 50, … 750 (a strike-opened battle; a plain one not read).
  This is battle logic, not only the bar: the game's fighters start 100 apart where
  `fight::Battle::new` starts all at 500, so later turn orders can differ. Read the join's
  callers (`0x14018cf20`, its `from` argument) and the start of a plain battle live): the game places each portrait by the time until that
  turn (ruins-entrance-battle-stats at 322 s: about two slots, 128 px, apart), Barrel packs
  the order into neighbouring slots. Needs the timeline's wait values mapped to the bar (read
  how the bar's code places a face: its slot or a pixel offset).

- Move command: who sets the mover's new place (+0x394) before `BtlCharaActionMove` runs, how
  the player chooses it (which sector, the +0x3c direction) and whether a member already there
  swaps. Barrel moves to the next sector round and moves nobody else.

- Battle items: where Item sits in an alchemist's command row (Barrel puts it after Attack, by
  command number; check a recording of Escha's or Logy's turn), choosing a party member for a
  heal item (Barrel uses it on the user), and when spent items fill up again (Barrel keeps them
  at 0 uses; the game refills them at the atelier, not read). A new game's starting battle
  items are not read.

- Equipping battle items from the basket (the atelier's preparation, capacity 5 at the start): the basket's items carry no uses or effects yet (`gather::Got`), which a `carry` line needs.

- Battle numbers: the code that draws them is not found (the parts' ids are not loaded as
  constants where the battle draws); so not read: their anchor on the fighter (Barrel: 100
  units over its place), how long they stay (the recording's camera cuts away after ~0.85 s),
  the heal and MP colours (part 0xe2's pink row, the purple rows of picture 1) and when the
  big grey italic row (0xe1, layout 883) and layout 927 (a number on a panel with a word,
  likely the chain count) are used.

- The action banner (layout 885): the code that places and sizes it (Barrel uses the
  recording's place and size: full size, centred at x 960, top 180, name + 306 wide, name size
  33), when it comes up and goes within an action, and whether Run, Move, Wait or items show
  it (Barrel: skills, attacks and items only).

- `--walk` does not move the player in a window (offscreen only).

- The damage rate panel is drawn level at its layout's places; the game tilts it about 7°
  (rising to the right), its labels lower and further left, the numbers higher (the
  ruins-entrance recording at 104 s). Its drawing code, and how long it stays, are not read.

- The target cursor: while a target is chosen the game shows a cyan 3D mark over the target's
  head (the ruins-entrance-battle-result recording at 102.8 s). It is not in a15_battle.g1t, so
  likely an effect model; not found or drawn yet. The game's camera also moves behind the actor
  facing the target while choosing; Barrel's stays put.

- The Support gauge's gain: the action kind 1 (× 4), the loop's hit flag, the globals
  `0x141077e78` / `0x141077e7c` that stop it and the carry +0x1458 are not modelled; the start's
  300 case (setup +0x38 = 1) neither; the support attacks and guards that spend it
  (`0x140192879`) are not in yet.

- Support costs: Escha and Katla 100, Logy 50 for a Support Attack (live, 2026-11-15); where the
  cost comes from (the request's +0x5c through the timeline entry `0x14018d890`) and why Logy's
  is half (a passive or his equipment?) are not read; Support Guard's cost (50?) not confirmed.

- Support Attacks in Barrel's battle: the supporter's own turn is left where it was (not
  checked against the game: the recording's turn bar after a support would tell); members
  other than Escha, Logy and Katla take 100. In play LB / Y / RB (1 / 2 / 3) give the left,
  centre and right plate's member's support while a party hit's banner shows, its bubble
  (layout 882) over the plate; the game also frames the plate in a grey box then (not found),
  and the text is a little browner than the game's. X (the reserve, the bubbles' second row)
  and Support Guard are not in, nor supports by the stand-in.

- Support Guard: its cost (50, from one live reading) and whether it covers area attacks, its
  +0x2b8's clearing (`0x1401b4a3f`) and the guard's own gauge gain are not read. In play a
  monster's action on a member is held 1 s (Barrel's length, not measured) with "Support Guard"
  bubbles over the others' plates when the gauge holds 50; the game holds it as the monster's
  motion starts, and moves the guard in front of its target (not animated here).
