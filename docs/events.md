# Event commands

What each event bytecode command is and how many operand words it takes. The body format
itself is in `formats.md`, "Event bytecode".

## Where the names come from

The executable registers every event command word with its Squirrel VM as a constant named
`ECT_*`, so that scripts can refer to them. Each registration is
`mov r8d, WORD; lea rdx, [name]; call add_constant`. `tools/ect_names.py` finds the
add-constant function from the `ECT_END` registration (`0x7f000000`) and lists every word
and name passed to it:

```sh
objdump -d -M intel --no-show-raw-insn GAME.exe > work/game.asm
tools/ect_names.py GAME.exe work/game.asm > work/ect.txt
tools/cmd_sites.py GAME.exe work/game.asm work/classes.txt > work/sites.txt
tools/ebd_gaps.py "<install>/Event" work/ect.txt work/sites.txt   # the table below
```

Escha & Logy DX registers 219 names. By group:

| Group | Top byte | Names | What |
|---|---|---|---|
| `p` | 0x70 | 6 | event start and the macro waits |
| `q` | 0x71 | 48 | field map: characters, gimmicks, fog, colour |
| `r` | 0x72 | 1 | battle |
| `s` | 0x73 | 103 | characters: position, motion, rotation, colour, effects, sound |
| `t` | 0x74 | 36 | camera and variable-index settings |
| `u` | 0x75 | 23 | event-wide: post effects, contrast, sound (`0x751.....`) |
| `x` | 0x78 | 1 | `ECT_SET_DEFCHARA_BCID` |
| | 0x7f | 1 | `ECT_END` |

The flags byte is part of the command's identity: `0x75100005` (`ECT_SOUND_BG_FADEOUT`) and
`0x75000005` are different commands. The executable compares whole words when it
dispatches them.

Of the 238 command words the events use, 132 are named; they account for 66% of all
command uses. The unnamed ones are not exposed to Squirrel. They are mostly in `r` (battle),
`u` and `w`, and their names have to come from their handlers.

`w1` (`0x77000001`) is the commonest word in the events (47,448 uses). It always takes
one operand, a number that counts up through an event (1, 2, 3, ...), and it often comes in
runs (`w1 1 w1 2 w1 3`) between other commands. It **starts an action**: `Event`'s handler
(`0x1400523bf` in the English executable) queues its operand, an action number, among the
actions to start (`0x140210d30`), and the action then runs alongside the one that started it.
Action 0 is the event's main script and starts the others: in 934 of the 1,021 story events
its `w1` numbers are exactly 1 to the last action, each once and in order; the other 87 skip a
few actions, which never run.

## Decoding

`barrel_formats::ebd::decode` splits a body into commands using an operand table for the
game (`OperandTable::escha()`, generated from the table below). A command with a fixed count
takes exactly that many words; the six whose count varies run to the next command word.
The decoder checks that every command lands on another command word or `END`, so a table
that is wrong for a game fails at once. `barrel ebd-decode DIR` runs it over every event:
all 49,393 of Escha's bodies decode into 238,443 commands.

## Playing an event

`barrel_game::event::Player` (crate `crates/game`) runs an event and yields what happens,
step by step; `barrel play-event FILE.ebd [EN|SC|TC]` prints it, finding the event's
message file on its own (`Event_<LANG>/event/<chapter>/EVENT_MESSAGE_<name>.ebm`).

It knows **`u18` (0x75000012), which shows the event's messages from its first operand to
its second, inclusive**, and the waits (below). In `EVENT_MM08_AS4_130` its ranges (0–0, 1–3,
4–11, ... 25–25) cover the 26 messages exactly once, in order. The player runs action 0 and starts
another action wherever a `w1` says (the game runs them at once; the player puts a started
action's steps right where it starts, each tagged with its action); every other command is
reported with its name and operands, for the systems that will handle it. `Player::schedule`
(`play-event FILE [LANG] --timed`) runs the event on a clock instead: every started action
at once, each held only by its own waits, and prints each step with its time. That each
action has its own timer is an assumption, and a doubtful one: the timer is at +8 of the
`Event` object (RTTI `Event`, vtable `0x140758188`, built by `0x140050560`), which holds the
event's `EventData` at +0x70 (vtable `0x140847128`; `w1` queues action numbers in it), so
there is one timer per event. The setter (`0x14005b310`) only takes a time while the timer is
armed (+0x40), so two actions waiting at once may take turns rather than overlap; how the
update loop (`Event` slot +0x48, `0x140052ee0`) shares it among running actions is still to
be read.

What is known of that loop: every listener (`Event`, `EventExecChara` and the others) is an
executor with a queue of commands of its own (`0x140029190` returns it) and a timer of its own
at +8. Its update (`0x1402117e0`, the shared base) looks at the command at the front of its
queue each frame, asks itself whether it can start it (slot +0x40) and whether it is done
(slot +0x48, `Event`'s being `0x140052ee0`, where the waits are checked), and moves on when it
is (`0x14005b270`). So a wait holds the executor that took the command (the event's own for
`p1`, a character's for `s3`), not an action. How an action's commands reach the queues, and
whether an action waits for each before sending the next (the 0.1 s waits between character
placements in `EVENT_MM08_AS4_130` only make sense if it does), is the next thing to read.

Two helpers tie the listeners together (read, but not fully): `0x140296a60` asks each
listener in turn through its slot +0x28 until one says yes (`0x140293db0`); `Event`'s
handlers call it, 6 times, with commands they build. `EventExecChara`'s +0x28 says yes to a
`p3` naming its character, so a `p3` most likely waits through that question. The other,
`0x140296a90`, is what `Event`'s wait checks call (6 times); it goes through `0x140293ef0`,
which calls a method (+0xe0) of another object, not the listener chain; what it asks is not
read yet. (An earlier version of this paragraph had the two the wrong way round.) Messages take no time
there (the game waits for the reader), nor do the waits for characters and the camera. Run that way, that
event fades the music out
(`ECT_SOUND_BG_FADEOUT`), starts track 105 (`ECT_SOUND_BG_PLAY`), shifts the contrast,
waits, and talks the dragon problem through in the right order.

Only an event's first action ever shows messages: across all 1,021 story events no other
action has a `u18`. In 894 of them the message ranges run 0, 1, 2, ... in file order. The
other 127 are still linear scripts: their messages are shown out of numeric order (the
prologue tutorial shows 43, 45, 44, then 40-41 as it walks past the furniture), or some are
never shown at all, the lines the writers deleted (their text is 【削除】, "deleted"). No
branching has been found inside an event.

**Speakers** are character numbers: the rows of `Saves/chara/Chara.xml` (or
`Saves_<LANG>/chara/Chara.xml` for a translation), 0 Escha, 1 Logy, 2 Awin, 3 Threia, 4
Reyfer and so on. `ECT_FIELDMAP_CHARA_ADDITION_EX` takes the same number as its first
operand. `play-event` prints the names.

**Fields.** `ECT_FIELDMAP_CHANGE` and `ECT_FIELDMAP_CHANGE_NOT_BGM` take a field number:
its position in a table of 24-byte field tags in the executable, 0 `TOWN_01` to 11
`TOWN_12`, then 12 `AREA_01_01`, 13 `AREA_01_02`, ... (137 tags in Escha). The tags are
the folder names under `Data/x64/Field/`.

**Placing characters.** `ECT_FIELDMAP_CHARA_ADDITION_EX` (`q4`) is `{character, 0, 0, 0, x,
y, z, 0, angle in degrees, ...}` in field coordinates (all 14 operands below, "Adding and
removing characters"); `ECT_CHARA_POS_SET` (`s4`) is
`{character, 0, 0, 0, x, y, z}`. A character's model is found through its `Chara.xml` row's
`BaseTag` and that tag's `Model` in `CharaBase.xml` (`PC20/PC20_MODEL`, under
`Data/x64/Character/`). `barrel-view --field <TOWN_04> --event FILE.ebd` draws an event's
opening: the field and each character at its first placement. `EVENT_MM08_AS4_130` (field
3, `TOWN_04`) puts Escha and Logy on the grass by a curved stone wall and Threia and Reyfer a
few metres off, all standing at ground height.

**Idle poses.** Each character stands in its field wait motion: the `CharaBase.xml` row's
`F_WAIT` names it (`ACT_DEF` for Escha), the file is `<FileHead>_<F_WAIT>.g1a` in the
`<Motion1>` container (`PC20_BASE_MOTION1`), on the skeleton in the base model. In
`EVENT_MM08_AS4_130` that gives Escha and Logy facing Threia and Reyfer across the town
plaza, by the stone planter, all four standing naturally.

**The event camera.** The field camera's commands are handled by `CameraCFieldMap`
(`0x1401e7a10` in the English executable, a jump table over `0x74100003..0x74100015`).
Every operand is an integer divided by 100, and the angles are then turned from degrees to
radians. `t20/0x10` (0x74100014) sets the whole camera; in `EVENT_MM08_AS4_130` it opens
with `3791 0 2250 286 448 9983 383 11369 0 150 0`:

`t19/0x10` (`ECT_FIELDMAP_CAMERA_DATA_EFFECT_SET`, `{on}`, case `0x1401e81b1`) reloads the
field's camera data (virtual +0xd8, `0x1401e6e10`, by the id at +0x13c, into +0x130) and sets
+0x138 (virtual +0x88, `0x1401ec650`). While it is on, the camera's update passes its target
through that data (`0x140066f80`) and takes its angles from the result (`0x1401e7171`,
`0x1401e748c`): the field's own camera rules, as in play. Every opening event turns it off
first and on again as its last camera command (594 `t19 0`, 548 `t19 1` in all), handing the
camera back to play, so a scene never draws with it; the stage keeps the flag
(`FieldCamera::data_effect`) and the camera data itself is not read.

| Operand | Example | Use (from the handler) |
|---|---|---|
| 1 | 37.91° | yaw, through the same setter as `t3` (0x74100003; the setter `0x1401ec9e0` keeps it at +0x24/+0x28) |
| 2 | 0° | an angle, stored where `t5` (0x74100005) stores |
| 3 | 22.50° | the field of view (see below), through the same setter as `t7` (0x74100007) |
| 4 | 2.86° | the pitch (see below), through `0x1401ec8f0` (the immediate form of `t9`'s setter) |
| 5 | 448 | distance (divided by 100, then multiplied back) |
| 6..8 | (9983, 383, 11369) | the target point, in field units |
| 9..11 | (0, 150, 0) | an offset added to the target: the look-at point sits 150 units up, about head height |

The camera looks at target + offset from `distance` away, turned by yaw and pitch. In
`EVENT_MM08_AS4_130` that opens on Threia and Reyfer by the plaza's planter, with a market
stall and flowerbeds behind them and Escha coming into frame. `barrel-view --event` uses the
event's camera; `--free-camera` places the view on the characters instead. The field of
view is not in the command (40° is used for now).

`t18/0x10` (`ECT_FIELDMAP_CAMERA_SET_FOLLOW_CHARA`, handler `0x1401e816d`) is `{character,
on}`: the camera follows the character only when `on` is not 0, and otherwise follows nothing
(`virtual +0x78(-1)`); the opening's `t18 0 0` turns following off, so `t20`'s target stands.

**Where the eye is** (`0x1401e68f0`, settled from the code). The point looked at is the
target plus the offset (+0xd0 and virtual +0x70). The eye is `(0, 0, distance)` (virtual +0x68)
turned by the angles `{x: -pitch, y: yaw}` (virtual +0x60 and +0x58; `0x14004fbd0`, then
`0x140029050`, row vectors) and added to it: target + (sin yaw · cos pitch, sin pitch,
cos yaw · cos pitch) × distance, in game space (the renderer then mirrors x like the
vertices). The field of view is `t20`'s third operand: the camera hands it to the view
(`0x140415ba0`, +0x140), which halves it and takes its tangent for the projection
(`0x1404142d5`). The pitch is the fourth. With these, `EVENT_MM01_OP1_111` shows Clone and
Escha over the shoulder in the workshop, then Escha close up and walking through town.

Two guesses were made before the code was read, by checking which way of placing the eye
put the event's characters nearest the centre of its first shot over the 168 story events
(taking operand 3 as the pitch, and the yaw's sign in x only); both were wrong in ways that
measure could not see, since the view always passes through the target.

**The field camera over time** (`barrel_game::field_camera`). Each of its values is held as
`{start, goal, now, seconds}` (yaw at +0x20 to +0x2c, pitch +0x50, a fourth angle +0x80,
distance +0x98; setters `0x1401eca20`, `0x1401ec7a0`, `0x1401ec940`, `0x1401ec890`): a
command heads it from its value now to the goal over the seconds, at once for 0. The odd words
set the goal, the even ones add to the value now (`t4` after `t3`'s `0x1401e7b01` branch):

| Word | Changes | Operands |
|---|---|---|
| `t3`/`t4` | yaw | degrees, seconds |
| `t5`/`t6` | an angle (`t20`'s second; not followed) | |
| `t7`/`t8` | field of view | degrees, seconds |
| `t9`/`t10` | pitch (`t20`'s fourth) | degrees, seconds |
| `t11`/`t12` | distance | units, seconds |
| `t13`/`t14` | the target | x, y, z, seconds |
| `t20` | everything at once | see above |

The update (`0x1401e6ee0`, each frame) moves every value now by `(goal - now) × dt / time
left` and counts the time left down, landing on the goal when it runs out: linear in time.
The stage holds the field camera (`Stage::field_camera`) and its snapshot gives the eye and
target of whichever camera was set last, the field camera or the manual one (`t3` ... without
`/0x10`); the window follows it every frame. `EVENT_MM01_OP1_111`'s action 7 turns the camera
from -176.96° to -163.97° over five seconds while it pulls toward Escha.

**Screen effects** (`u13`, `ECT_EVENT_SET_POST_EFFECT`, 2,180 uses; `u14`,
`ECT_EVENT_WAIT_POST_EFFECT`, waits for it). The handler (`0x140331bf0`) passes the second
operand, the third divided by 100 (seconds) and the fourth (the kind) to `0x140331220`, which
makes one of these by the kind (a 17-way jump table; class names from the constructors'
vtables): 0 `PostEffectFade`, 1 and 12 `PostEffectBlur`, 2 `PostEffectSepiaOld`, 3
`PostEffectCrossFade`, 4 `PostEffectBattleEnter`, 5 `PostEffectNegative`, 6 `PostEffectGlow`,
7 `PostEffectRefraction`, 9 `PostEffectHue`, 10 `PostEffectGamma`, 11 `PostEffectColor`, 13
`PostEffectSepia`, 14 `PostEffectMonotone`, 15 `PostEffectTexFilter`, 16 `PostEffectWave` (8
none). Their programs are the `.kps` packs in `Data/x64/PostEffect/pe_pack.elixir.gz`.
`EVENT_MM01_OP1_111`'s main action changes shots with `u13 -1 0 50 3` ... `u14`: a half-second
cross-fade from the last shot to the new one. The scheduler makes `u14` wait until the latest effect's time
has run out (one effect at a time, as the game keeps one).

**Labels and jumps.** `u3 {n}` marks label `n` in its action (an action's labels are found
by scanning its words, `0x14020fcd6`); `u5 {n}` goes on from label `n` at once
(`0x140210be0` → `0x14020fd80` sets the running action's next command, +0x8), `u6 {a, b}`
from label `a + b`. `EVENT_MM01_OP1_111` ends with a fade to black, everyone removed, then
`u5 99`, which jumps over starting actions 64 and 65 (Escha placed in town, the camera handed
back to play, a fade in) to `u3 99` at the end: that part is reached some other way
(skipping the event, presumably). `u9`/`u10/0x40` (3,628 uses each, no operands) bracket each
event's setup action (`u9`, start the setup with its preload `u2`, `u10`): taken to hold the
event while it loads, which the viewer does not need (not read).

**Telops** (narration cards: `u55`, `u56`, class `EventTelop`, handler `0x1402196f0`). The
voiced narration of the openings (`EVENT_MM01_OP1_111` cards 38 to 44 with voices 12907 to
12913, `_212` cards 45 to 50) and the ending is drawn as pre-rendered text, the textures
`WinXls_EN/a15_telop_<event>_NN.g1t` (texture indices 151 to 163 in the UI table at
`0x140db9520`, 16-byte `{name, index << 32}` records). `u55 {card}` picks the card: layout id
`0x140d85ce0[card]` (51 entries, 960 to 1010; the opening's 986 to 998). `u56 {from, to,
hundredths}` (all hundredths) moves its opacity (+0x2d8 start, +0x2dc goal, +0x2e0 now, +0x2e4
time left; `0x140219240` moves it linearly each frame); events fade a card in over 1 s with
the voice and out over 0.5 s.

The layout ids index `0x140dd9080` (16 bytes from id 0x165 on: `{node pointer, x, y}`;
`0x140373f40`), the cards at (180, 390). The nodes are the UI layouts compiled into the
executable (the `WinXls` sheets ship no data files): 0x28-byte records starting `{u32 part,
u32 cut, ...}` (part 0x128 on for the telops): see `docs/formats.md`, "Interface layouts".

**Fades** (`u22`, `ECT_EVENT_CONTRAST_SET_MIX`, 3,159 uses). The command goes to the `Contrast`
objects (vtable `0x1407275f8`; virtual +0x20 `0x14003d3f0`): `{id, preset, hundredths of a
second}`, the id matching the object's +0x58 (1 in the story events). `0x14003ce20` takes
preset `preset` (0 to 20) from the table at `0x1407272b0`, 32 bytes each: two colours with
opacity, the overlay's start and end, which it moves between over the time (0: at once).
Preset 2 goes from clear to black (fade out), 3 from black to clear (fade in), 0 and 1 the
same with white, 4 to 11 and 16 to 20 partial darkenings (0.5, 0.8), 12 to 15 coloured
tints. `EVENT_MM01_OP1_111` starts black (2, at once) and fades in over a second (3, 100)
on the workshop floor before the camera finds Escha and Clone.

**Waits.** `p1` (`ECT_MACRO_TIME_WAIT`, 8,664 uses) waits its operand in hundredths of a
second (the unit is read from the values below; that the frame time it counts down by is in
seconds is not yet confirmed from the code): `Event`'s handler (`0x140050ff8` in the English executable) divides it by 100 and
sets the event's timer (`0x14005b310`, the float at +0x44), which counts down by each frame's
elapsed time (`0x14005b330`) and lets the event go on when it reaches 0. The values are short
pauses: 50 (half a second) 3,806 times, 10 2,647 times, then 100, 150, 200 and up to a few
seconds. `u1` (0x75000001) waits 0.3 s, a constant in the same handler. `play-event` prints
both as waits.

`barrel_game::event::Player` also turns `s3`, `s4` and `s10` into steps of their own (a
character's wait, place and looping motion, each with the action it belongs to), and
`play-event` prints them with the character's name and, when it finds the install's
executable next to `Saves`, the motion's name from its `MOTION_*` enum (`Escha plays TURN_L`,
then `Escha plays WALK (blend 0.40 s)` in `EVENT_MM01_OP1_111`, after a turn command).

The other `p` commands (`p2` to `p5`, and a few others such as `0x71000023`, `0x72000013`,
`0x7200000f`) wait until a listener says a thing is done (`0x140052f35` in the event
player's update). `p3` (`ECT_MACRO_CHARA_WAIT`) takes a character number (1 and 0, Logy and
Escha, most often): `0x1400710b0` finds the character by number (its +0x78) through
`0x1400760c0` and asks it, and `WMResMgr` (`0x1403874e0`) matches it against its characters'
+0x81c. What it waits for (a move or motion to end, or the character's files to load; the
resource manager's part hints at the latter) is still open. `p5` (`ECT_MACRO_CAMERA_WAIT`)
is answered by `EventExecCamera`.

**Characters' own commands.** Each character in an event has a listener of its own
(`0x14026b110` decides, `0x14026b3c0` runs): it takes a command when the command's first
operand is its character number (the character's +0x78), for `q7`, `q9`, `s3`, `s4`, `s18`
to `s21`, `s66` and a few more. So these commands' first operand is always a character, and
each character runs them on its own:

- `q7` (`ECT_FIELDMAP_CHARA_SET_EVENT_MODE`, `{character}`) sets the character's mode
  (+0x138, through `0x14026d040`) to 1, the event mode; `q9` (0x71000009) sets it to its
  second operand (5 when there is none);
- `s3` (`ECT_CHARA_TIME_WAIT`, `{character, hundredths}`) starts a timer on the character
  (the same timer as the event's, `0x14005b310`), so the character waits while the rest of
  the event goes on;
- `s4` (`ECT_CHARA_POS_SET`) places it (see "Placing characters").

That a `p3` (`ECT_MACRO_CHARA_WAIT`) waits until a character has done what it was told (its
timer and queued commands) would fit, but is not read yet.

Events also add invisible markers as characters: `Chara.xml` rows 590 to 603 are
`ID_DUMMY_POS_000` to `_013`; rows 591 to 602 (`_001` to `_012`) are added (`q3`) and set up
(`q7`, `q8`) 281 times each in Escha's events, and row 749, "Master Character", often too, places for the camera and the characters to aim at, by the look of it.
`q3` (`ECT_FIELDMAP_CHARA_ADDITION`) is `{character, 0}`; `q8`
(`ECT_FIELDMAP_CHARA_COLLISION_FLAG`) is `{character, collides, b, c, d}`, most often
`0 0 0 2` (no collision) or `1 1 1 2`. The field's character listener (`0x140071160`, which
takes `q1`, `q3` to `q5`, `q8`, `q13`, `q14`, `q17` to `q19`, `q29`, `q32`, `q37` and `q39`,
as its filter `0x140071030` lists) finds the character by number and sets its +0x128 from
`collides` (when that changes, the character's collision body is added to the field's
collision world, `0x140506fc0`, or taken out, `0x140507070`) and its +0x129 flag from `b` (what
reads it is not followed yet). `c` and `d` are not read there.

**Adding and removing characters.** In the field character listener (`0x140071160`):

- `q3` (`ECT_FIELDMAP_CHARA_ADDITION`, `{character, v}`) creates the character if the field
  has none of that number (`0x140075720`) and sets its +0x148 to `v` (0 in the events);
- `q5` (`ECT_FIELDMAP_CHARA_DELETE`, `{character}`) removes it (`0x1400758e0`);
- `q4` (`ECT_FIELDMAP_CHARA_ADDITION_EX`) creates it if needed, sets +0x148 from operand 12,
  and hands the whole command to the character's own command player (`0x14026cf70`, which
  queues it at character +0x10 through `0x140211970`), which places it;
- `0x71000027` does the same as `q4` with a 13th operand, passed when creating the character
  (when not above 0).

`q4`'s operands over Escha's 2,398 uses:

| Operand | Values | What |
|---|---|---|
| 1 | character | `Chara.xml` row |
| 2, 4, 8, 10, 12, 14 | always 0 | |
| 3 | 0, or 709 in 90 | not known |
| 5, 6, 7 | x, y, z | position in field units |
| 9 | angle in degrees | facing |
| 11 | a motion number: 31 (`F_WAIT`) in 2,353, 32 `WALK`, 34 `SIT_DOWN`, 33 `RUN`, 67 `EV_ACT_12` | the motion it starts in (the `MOTION_*` enum) |
| 13 | 1 in 2,166, 2 in 230 | not known |

**`s116` and `s117`.** The character's own command runner (`EventExecChara`, vtable
`0x14081b140`; its filter takes commands whose first operand is the character's +0x81c, and
its slot +0x28 is what a `p3` on that character asks) runs both, in its switch at
`0x1401eff04`:

- `s116` (0x73000074, `{character, segment, rate, weight}`, 1,604 uses) plays a **head
  gesture**: one segment of the character's `<FileHead>_M_HEAD.g1a` (in the container its
  `CharaBase` row's `Neck` column names, e.g. `PC20_NECK`; 12.03 s, 361 frames), on the
  character's second motion player (+0x38, `0x1404ee950`). `0x140037a10` turns the segment
  (0 to 7) into a frame range at 30 frames a second: 0-39, 54-88, 103-137, 147-181, 192-223,
  237-267, 282-312 or 327-360 (tables `0x140726730`, `0x140726750`); -1 (or anything else)
  ends it. Rate and weight are hundredths (`0x140202920`): the player's time moves by
  dt × rate (+0x4c) from the segment's start and stops at its end (+0x44 clamped to +0x48,
  `0x1404ed1e0`), so the events' rate 999 jumps to the end pose and holds it, 1 plays it in
  real time; the pose is laid over the body motion's at the weight (+0x50; `0x1404177a0`
  lerps the joints' transforms, `(1 - w) base + w gesture`), the events using 0.2 to 0.4. Only
  the joints the player's list (+0x110) marks take it; `M_HEAD` turns `B_0_Neck` (up to 18°),
  `B_0_Head` (51°) and the side hair, and the renderer takes the joints it turns (a guess
  where the list is not read). The events mostly hold segments 4 and 6.
- `s117` (0x73000075, `{character}`, 1,632 uses) clears the character's +0x8d, ending the head
  gesture.

**Motions and moves.** In the same runner switch:

- **Move and turn waits.** `s21` (`ECT_CHARA_MOVE_PRE_WAIT`, `{character, hundredths}`,
  case `0x1401f08af`) gives the character's mover (+0x608) a time (`0x1402f20d0`: +0x30, flag
  +0x34); the mover's update (`0x1402f2894`) clears the flag once its time left (+0x28) is no
  more than that, which ends the wait: the action goes on that long before the character
  arrives (`s21 16 20` in `EVENT_MM01_OP1_111`: Clone's idle pose starts 0.2 s before the end
  of her 3.09 s walk). `s29` (`ECT_CHARA_ROT_MOVE_WAIT`, `{character}`) waits for its timed
  turn (`s26`). The scheduler runs the steps on a scratch stage to know where each move
  starts, and so its length.

- **Divisions.** The motion starter `0x1401f4c50(character, motion, division, blend)` plays
  one division of the motion: the records at the character's +0x110 + 8 × motion, 12 bytes
  `{f32 start, f32 end, u8 loops}`, filled from `Saves/chara/CharaDiv.xml` and
  `SubCharaDiv.xml` (rows `{BaseTag = FileHead, MotionTag = the MOTION_* name, DivN = "start,
  end, loops"}` in seconds, end -1 the clip's end; `crates/formats/src/chara_div.rs`); without
  one it plays `{-1, -1, 1}`, the whole clip. `s5` (`{character, motion, division, blend}`,
  case `0x1401eff27`) and `s11` (`ECT_CHARA_MOTION_SET_AUTO_LOOP`, `{character, motion,
  division, flag, blend}`, case `0x1401f0171`; +0x5c = division == flag) pass theirs; `s10`
  passes 0 and then clears the range (`0x1402036d0` with -1, -1), so it plays the whole clip.
  So a seated townsperson's `s11 … SIT_DOWN 1` loops 1.0 to 3.97 s, the sitting, and an
  `EV_ACT_1` division 1 its 0.67 to 4.67 s loop, not the whole clip. `s13`
  (`ECT_CHARA_MOTION_SET_OFFSET_ONLY`, `{character, division, flag}`, case `0x1401f0211`)
  moves the motion playing (+0x824) to another of its divisions (`0x1401f6830`: the player's
  range set from the division, its time to the start, no blend; +0x828 keeps the division,
  +0x5c = flag ≠ 0). `EVENT_MM01_OP1_121` starts Escha's `EV_ACT_21` at division 0 (`s11`)
  and steps it through 2, 1, 2, 1, 3, an `s6` (`ECT_CHARA_MOTION_WAIT`) after each one that
  does not loop. `s6` (`{character}`) holds the action until the character's motion
  (its division, when one was given) has played out; a looping one is not waited for. The
  scheduler (`Player::schedule`) does this when given the lengths
  (`Player::motion_length`): the viewer measures every motion and division the event plays
  from the clips and `CharaDiv` before scheduling; the CLI has no clips, so there `s6` takes
  no time.

- `s10` (`ECT_CHARA_MOTION_SET_LOOP`, `{character, motion, blend, flag}`) plays the motion
  (a `MOTION_*` number, e.g. 32 `WALK`) through `0x1401f4c50`, blending over `blend` / 100
  seconds, and sets the runner's +0x5c from `flag` (not zero: what it switches is not read).
- `s18` (`ECT_CHARA_MOVE_SET`, `{character, position, extra, type, ...}`) hands the command
  to the character's move controller (`0x1402f25f0` at runner +0x600, with the character's
  position at +0x230), which builds a move from it ("Event positions and moves" above): in
  `EVENT_MM01_OP1_111` Escha walks to (1036, 9, 1587) at 150 units a second. The extra
  operand (1 in 745 uses, 0 in 736, 2 in 236) goes to the move's +0x38, which the moves'
  updates check: 1 turns the character to face the way it moved that frame, on the ground
  (yaw = atan2(dx, dz), 0 towards +z; not when it moved less than 0.01 along both x and z),
  and 2 also tilts it to the slope (`0x1402f21a0`). `barrel_game::camera::Mover` runs type 4
  and type 0 moves frame by frame this way.
  `play-event` prints these moves.

**Event positions and moves.** Many commands give a point as six operands
`{kind, id, point, x, y, z}` (`0x1402f13f0` reads them; `s4`'s `{character, 0, 0, 0, x, y, z}`
is a character and one of these). `0x1402f0fa0` finds what `kind` and `id` name:

| Kind | What (from the code) |
|---|---|
| 0 | nothing: the offset is a point in the field |
| 2 | a thing whose +0x160 object gives the point (`0x1402f0de0`; not named yet) |
| 3 | a field character by number (`0x1402f0c60` asks the listeners with `0x7600000d {id}`; the field-character manager answers, `0x1400718c0` → `0x1400760c0`, which looks the number up in its two character lists by +0x78, then `0x140270230`, then the player). The camera's `t3`/`t6` use ids 10048 to 10057: the field's own NPCs (`EVENT_QC08_MIC_050` puts them in event mode with `q7 10048` ...), placed by the field, not the event |
| 4 | a thing (`0x1402f0d20`) with a place at +0x90 and a facing at +0xa0 |
| 6 | one of four saved points (`point`, clamped to 0..3, at `0x141078288`) |
| any other | a character by kind and number (`0x1402f0ba0` asks with `0x76000002 {kind, id}`): kind 5 with the numbers the `s` commands use (10000 and up in skill events) |

`point` picks one of 27 points on a character (`0x1401f3b80`, a switch over the
character's measures, its `Chara.xml` row: `Height` 151 and `Radius` 27 for Escha, and
`Foot`, `Body`, `Face`, `Head` 0, 0.7, 0.91, 1 and `Extra` 0.7, 1, 1 as fractions of the
height):

| Point | Where |
|---|---|
| 1 to 7 (and 17 to 23) | up by `Height` × `Foot`, `Body`, `Face`, `Head`, `Extra` x, y, z (the runtime row's +0x70..+0x7c and +0x48..+0x50, times +0x60) |
| 8, 24 | its place at height 0 |
| 10, 12 | at height 0, `Radius` (+0x64) in front of it or behind it (its facing, +0x244) |
| 9, 11 | the same along the camera's direction (`0x1402f0ec0`) |
| 0, 26 | the character's +0x3d8 (not read) |
| 13 to 16, 25 | other cases (14: fixed points by model, below or above 0x12) |

1 (feet), 2 (body) and 8 are the camera's commonest; the skill camera's target on point 2 sits
at the body. Which of the runtime row's columns is which comes from matching the schema's
offsets (`0x1401ed1c0`, each 4 on in the row) and the values' ranges. `0x1402f1250` adds the offset, turned first by the
anchor's facing unless the kind is 0 or the point is 14 to 26 (the game's Y rotation, from
`0x140028d50`'s sine and cosine: x' = x·cos + z·sin, z' = z·cos − x·sin). The coordinates are
whole field units (the code divides by 100 and multiplies back).

A move (`0x1402f1a90`, used by the camera's `t4`/`t7` and by the characters' `s18`) is a
position, then (for `s18`, `0x73000014` and `0x73000029` only) one more operand, then a type:

| Type | Operands after it | Uses in `t4` / `t7` / `s18` | Built by |
|---|---|---|---|
| 4 | seconds ×100, ease | 1,876 / 1,735 / 571 | `0x1402f3480` |
| 0 | speed, then x, y, z (mostly 0) | 27 / 19 / 950 | `0x1402f3720` |
| 3 | two: 800 to 2,500, then a height (50 to 300) | a few / 13 / 131 | `0x1402f2680` |
| 1 | eight more (a point and angles, by the look of them) | 129 / 5 / 63 | `0x1402f1cd9` |
| 2 | two | 1 / 1 / 2 | `0x1402f18a0` |

Type 0 heads for the position at `speed` field units a second (150, a walk, is the
commonest in `s18`; 120 to 140 for the slower walkers): its velocity is the unit direction
times the speed, plus the three extra values turned to the direction of travel (a push or
acceleration; mostly zero). Its time is the distance over the speed; the extra vector is taken
off the velocity every second, and half of it times the time is added at the start, so
the move bows (up, or to a side) and still ends on the point (`0x1402f3720`, update
`0x1402f3a70`). Every move's update takes the frame time off the time left first, then
moves the point, and puts it on the end once the time is used up. Type 3 is a jump: the move keeps a gravity of 9.8 (+0x8c) and
`0x1402f28f0` works out the velocity from the height; the first operand is not named yet.

Type 4 goes straight from where the thing is when the command runs to the position, in the
given time (`0x1402f3530` updates it). With u the part of the time gone, the distance covered
is u · (1 + ease/100 · (1 − u)): ease 100 (1,487 of the `t4`s) slows to a stop (u·(2 − u)), −100
(295) starts slowly (u²), 0 (207) is even.

**The skill camera.** `CameraCManual` (`0x1401eb170`, the camera of the battle skill events,
`t1 1`) keeps the eye and the target as two moving points (+0x18 and +0x28):

- `t3` (`ECT_CAMERA_EYE_POS_SET`) and `t6` (`ECT_CAMERA_AT_POS_SET`) put the eye or the target
  at a position.
- `t4` and `t7` (`…_MOVE_SET`) move them (a move, as above), from where they are.
- `t9` (`ECT_CAMERA_EYEAT_POS_SET`) sets both: the *target* first, then the eye (the first
  position goes where `t6` puts it; the eye's offsets are the larger ones, 2 to 10 m).
- `t10` (`ECT_CAMERA_ROT_MOVE_SET`, `{a, b, seconds ×100, ease}`) turns the view by the two
  angles in degrees (a to +0x38) over time, and `t12` (`ECT_CAMERA_LENGTH_MOVE_SET`,
  `{length, seconds ×100, ease}`) changes the eye's distance from the target; both use the
  mover at `0x140025890`, not read yet.
- `t29` (`ECT_CAMERA_MOVE_KILL`) ends both moves where they are; `t5`/`t8` wait for them.

`barrel_game::camera` reads these (`Position`, `Move`, `eased`), and `play-event` prints them:
in `SKILL_COMMON_MOB25_APPEAR_00` the camera starts on the monster (character 10002) from
4 m out and pulls back to 9 m in a second, easing to a stop.

**Which field.** An event names its field itself: `q12` (`ECT_FIELDMAP_CHANGE_NOT_BGM`,
`{map}`, 621 uses) and `q2` (`ECT_FIELDMAP_CHANGE`) change to field map `map`, a number in the
executable's `FIELDMAP_*` enum (122 names, found by `FIELDMAP_TOWN_01`): `TOWN_01` to
`TOWN_12` are 0 to 11, then `AREA_01_01` (12) and on, then the extra areas and
`A15_WMAP`. Without the prefix the names are the field directories under `Data/x64/Field`.
`EVENT_MM08_AS4_130` changes to 3, `TOWN_04` (the plaza it is drawn in), `_110` to 2
(`TOWN_03`) and `_020` to 8 (`TOWN_09`). `play-event` prints the change with its name, and
`barrel-view --field auto --event FILE` draws the event in its own field
(`event::field_maps`, `event::first_field`). 512 of the 1,021 story events change field; the
others play wherever the player already is (their field is not in the event).

**Frame time.** The game's updates take the frame's measured time, not a fixed tick:
`GameModeManager`'s update (vtable slot 1) passes it to the event manager (`0x1402931b0`),
which hands it to every event and executor. When the manager's +0x4b is set it uses
exactly 1/60 s instead (`0x140853f24`), so 60 Hz is the design rate. (`0x14077ba78`, 1/30,
is only a threshold inside the speed move's other update, not a frame time, as an earlier
note here had it.) Moves are not quite step-independent (a bowed walk adds its velocity
before taking off its push each frame), so replays and lockstep checks need the same steps;
Barrel runs its logic on a fixed 60 Hz tick and blends ticks for the screen (`docs/engine.md`, "Timing"); `Stage::frame` takes any length. Where the measured time
comes from (the main loop, and whether it is clamped) is still to be found.

**Facings.** `s23` (`ECT_CHARA_ROT_SET`, `{character, x, y, z}`, 4,668 uses; `0x1401f097c`
in the runner switch) sets the character's rotation (+0x240) to the three angles in degrees;
the middle one is the facing (0 towards +z, 90 towards +x), the same angle a move that
turns its character writes, and the other two are nearly always 0. `s24`
(`ECT_CHARA_ROT_SET_TARGET`, `{character, position}`, 6,809 uses) turns it to face a
position on the ground (`0x1401f4f50`: the yaw of the direction, 0 if the position is where
it stands). `s25` (`…_TARGET_ALL`) does the same through `0x1401f50c0`, not read.
`s26` (`ECT_CHARA_ROT_MOVE_SET`, `{character, x, y, z, seconds ×100, curve}`, 1,305 uses;
`0x1401f42e0`) turns it there over time: each angle changes the short way round (2π added or
taken off when that makes the change smaller; nothing happens when it is already there), and
the runner's interpolator at +0x638 (`0x1402034c0`) follows curve number `curve` (0 in 1,260
uses, 2 in 43) from `Saves/curve/curve_data.xml`, stretched from its 60 frames to the turn's
time (`0x1404e1770` fetches it; see "Curves" in `docs/tables.md`). Curves 0 and 2 have two
keys and so go evenly. Most `s26`s take 0 s; 0.5 s is the commonest time.

**The stage.** `barrel_game::stage::Stage` plays the timed steps on a clock of 1/60 s
frames (see "Frame time" above): `s4` places a character, `s23`/`s24`/`s26` turn it, `s18` starts a
mover from where it is, and the camera's eye and target are set or moved, positions measured
from characters taking the character's place and facing. `barrel play-event FILE --stage`
prints where everyone is each time the clock moves on: in `EVENT_MM01_OP1_111` character 16
walks from (506, 0, 1061) to (891, 0, 1320) between 3.5 and about 7.5 s, ending up facing
56°. Each line shown sets its speaker talking (the others fall quiet) and, when the line gives
one, the speaker's expression from then on (the message's fourth field; its third is a
gesture); `--stage` prints them (`face 4, talking`). `q4` (adding a field character) places it and turns it to its angle there too, `q5`
removes it, and `s5`/`s10` set the motion it plays. `s5` (`ECT_CHARA_MOTION_SET`,
`{character, motion, variant, blend ×100}`, 4,435 uses; `0x1401eff27`) goes through the same
`0x1401f4c50` as `s10` with `variant` (0 to 7) as its third argument, kept at the runner's
+0x828 (not read); `s10` passes 0 there and then clears the motion's play range
(`0x1402036d0` with -1, -1), so it loops from the start. With `--at` the viewer poses each
character in its stage motion that far in (looping ones wrap).
`barrel-view --field <DIR> --event FILE.ebd --at SECONDS` draws the stage as it stands at that
time: every character where it has walked to and facing where it turned, seen from the
stage's camera when the event has set its eye and target (the skill camera), otherwise from
the event's field camera. Positions on kinds 2 to 4 are not placed yet; the stage counts what it skips. Story events
number their characters by `Chara.xml` row, so `--stage` takes the measures from there.

Not yet: move types 1 and 2 and `s18`'s extra operand, `t10`/`t12`'s mover, the field camera's two
unidentified angles, characters' motions during the event.

## Who handles them

The event player hands each command to a chain of listener objects, each asked in turn
through a virtual method until one takes the command (`0x140293db0` in the English Escha
executable, calling slot `+0x28` on each). The listeners are classes derived from
`EventExec`. `tools/cmd_sites.py` follows the switch code that compares command words
(cmp/je chains, sub chains, range checks) and names the class whose method each comparison
is in; `classes.txt` is the executable's RTTI class names, one per line.

That gives a place for 61 of the 106 unnamed commands. The ones that matter most:

| Command | Uses | Handled by | So probably |
|---|---|---|---|
| `u18` | 4,990 | `B3Balloon` | show a speech balloon (a message line) |
| `t20/0x10` and the other `t…/0x10` | 4,484 | `CameraCFieldMap`, `EventExecCamera` | field-map camera |
| `s116`, `s117` | 1,604, 1,632 | `EventExecChara` | a head gesture (a segment of `M_HEAD` laid over the body), and its end ("`s116` and `s117`" below) |
| `u9/0x40`, `u10/0x40` | 3,628 each | `Event` | event flow; always together, with no operands |
| `u67`, `u68`, `u16` | 294, 194, 98 | `B3Balloon` | balloon variants |
| `u1/0x30`, `u2/0x30` | 243, 84 | `IpuGallery` | still pictures (Ipu are the CG images) |
| `u56`, `u97`, `u55` | 126, 65, 63 | `EventTelop` | on-screen captions |
| `u26` | 93 | `BalloonSelectMode` | a choice between balloons (dialogue choice) |
| `u3/0x40` | 88 | `MessageItemGp` | item-received message |

`w1` and `w4` are tested only by `Event`'s own code and the helpers around
`0x140050d30`: the event player starts actions itself (`w1`, above).

## Operand counts


"Words after it" counts the words between a command and the next command word in the same
body, over every event in Escha & Logy DX. 232 of the 238 commands are followed by the same
number of words every time, which gives their operand count. The 6 that vary are listed
with each count seen.

| Word | Short | Name | Handled by | Uses | Words after it (count) |
|---|---|---|---|---|---|
| `70000001` | `p1` | ECT_MACRO_TIME_WAIT | Event | 8664 | 1 (8664) |
| `70000002` | `p2` | ECT_MACRO_FIELDMAP_CAMERA_MOVE_WAIT | Event, EventExecCamera | 12 | 0 (12) |
| `70000003` | `p3` | ECT_MACRO_CHARA_WAIT | Event, WMResMgr | 3539 | 1 (3539) |
| `70000004` | `p4` | ECT_MACRO_CHARA_MARKER_WAIT | Event | 23 | 1 (23) |
| `70000005` | `p5` | ECT_MACRO_CAMERA_WAIT | Event, EventExecCamera | 313 | 0 (313) |
| `71000003` | `q3` | ECT_FIELDMAP_CHARA_ADDITION | WMResMgr | 5895 | 2 (5895) |
| `71000004` | `q4` | ECT_FIELDMAP_CHARA_ADDITION_EX | EventExecChara | 2398 | 14 (2398) |
| `71000005` | `q5` | ECT_FIELDMAP_CHARA_DELETE | WMResMgr | 41 | 1 (41) |
| `71000006` | `q6` | ECT_FIELDMAP_CHARA_DELETE_ALL |  | 1109 | 0 (1109) |
| `71000007` | `q7` | ECT_FIELDMAP_CHARA_SET_EVENT_MODE |  | 5895 | 1 (5895) |
| `71000008` | `q8` | ECT_FIELDMAP_CHARA_COLLISION_FLAG |  | 8081 | 5 (8081) |
| `7100000a` | `q10` | ECT_FIELDMAP_EVENT_OBJECT_VISIBLE_SET | (no handler found: not in `0x140071030`'s, `0x14007119f`'s or `0x1401ddb80`'s switches) | 771 | 1 (771) |
| `7100000b` | `q11` | ECT_FIELDMAP_NPC_ENEMY_RESET |  | 526 | 0 (526) |
| `7100000c` | `q12` | ECT_FIELDMAP_CHANGE_NOT_BGM |  | 621 | 1 (621) |
| `7100000d` | `q13` | ECT_FIELDMAP_GIMMICK_RESET |  | 17 | 0 (17) |
| `7100000f` | `q15` | ECT_FIELDMAP_MAPVISIBLE_SET |  | 53 | 3 (53) |
| `71000010` | `q16` | ECT_FIELDMAP_MAPANIMATION_EXEC |  | 2 | 6 (2) |
| `71000020` | `q32` | ECT_FIELDMAP_CHARA_TRANSPARENT_FOCUS |  | 249 | 5 (249) |
| `71000021` | `q33` | ECT_FIELDMAP_SET_FIELD_COLOR |  | 1 | 4 (1) |
| `71000024` | `q36` |  |  | 147 | 1 (147) |
| `7100002a` | `q42` | ECT_FIELDMAP_LIGHT_PARALLEL_SET |  | 15 | 6 (15) |
| `7100002b` | `q43` | ECT_FIELDMAP_LIGHT_PARALLEL_ORG |  | 18 | 1 (18) |
| `7100002c` | `q44` | ECT_FIELDMAP_LIGHT_AMBIENT_SET |  | 50 | 3 (50) |
| `7100002d` | `q45` | ECT_FIELDMAP_LIGHT_AMBIENT_ORG |  | 57 | 0 (57) |
| `71000032` | `q50` | ECT_FIELDMAP_CHARA_LIGHT_COLOR_SET |  | 34 | 4 (34) |
| `71000033` | `q51` | ECT_FIELDMAP_CHARA_LIGHT_COLOR_ORG |  | 38 | 0 (38) |
| `71000034` | `q52` | ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_SET |  | 16 | 4 (16) |
| `71000035` | `q53` | ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_ORG |  | 23 | 0 (23) |
| `71000036` | `q54` |  |  | 108 | 14 (108) |
| `71000037` | `q55` |  |  | 112 | 1 (112) |
| `71000038` | `q56` |  |  | 44 | 1 (44) |
| `71000039` | `q57` |  |  | 42 | 0 (42) |
| `72000004` | `r4` | ECT_BATTLE_ADD_EFFECT_ID |  | 1117 | 2 (1117) |
| `72000008` | `r8` |  |  | 1095 | 1 (1095) |
| `7200000d` | `r13` |  |  | 340 | 4 (340) |
| `7200000e` | `r14` |  |  | 681 | 6 (681) |
| `7200000f` | `r15` |  | Event, GameModeBattle | 191 | 0 (191) |
| `72000010` | `r16` |  | Event | 60 | 14 (60) |
| `72000011` | `r17` |  | Event | 76 | 2 (76) |
| `72000012` | `r18` |  | Event | 90 | 7 (90) |
| `72000013` | `r19` |  | Event, GameModeBattle | 6 | 1 (6) |
| `72000014` | `r20` |  | Event | 693 | 2 (693) |
| `72000016` | `r22` |  | Event | 30 | 6 (30) |
| `72000018` | `r24` |  | Event | 12 | 4 (12) |
| `7200001b` | `r27` |  |  | 3 | 1 (3) |
| `7200001c` | `r28` |  |  | 8 | 3 (8) |
| `7200001d` | `r29` |  |  | 3 | 1 (3) |
| `7200001e` | `r30` |  |  | 6 | 3 (6) |
| `7200001f` | `r31` |  |  | 1 | 4 (1) |
| `72000020` | `r32` |  |  | 13 | 6 (13) |
| `72000021` | `r33` |  |  | 1 | 1 (1) |
| `72000022` | `r34` |  |  | 2 | 3 (2) |
| `72000023` | `r35` |  |  | 41 | 2 (41) |
| `73000001` | `s1` | ECT_CHARA_SET_MARKER | EventExecBtlChara | 998 | 5 (998) |
| `73000002` | `s2` | ECT_CHARA_CAMERA_QUAKE_SET |  | 665 | 3 (665) |
| `73000003` | `s3` | ECT_CHARA_TIME_WAIT | EventExecChara | 3849 | 2 (3849) |
| `73000004` | `s4` | ECT_CHARA_POS_SET |  | 20753 | 7 (20753) |
| `73000005` | `s5` | ECT_CHARA_MOTION_SET |  | 4435 | 4 (4435) |
| `73000006` | `s6` | ECT_CHARA_MOTION_WAIT |  | 2966 | 1 (2966) |
| `73000007` | `s7` | ECT_CHARA_MOTION_SET_DIRECT |  | 24 | 2 (24) |
| `73000008` | `s8` | ECT_CHARA_MOTION_SET_DIV |  | 858 | 4 (858) |
| `73000009` | `s9` | ECT_CHARA_MOTION_SET_SPEED |  | 1504 | 2 (1504) |
| `7300000a` | `s10` | ECT_CHARA_MOTION_SET_LOOP |  | 1604 | 4 (1604) |
| `7300000b` | `s11` | ECT_CHARA_MOTION_SET_AUTO_LOOP |  | 133 | 5 (133) |
| `7300000c` | `s12` | ECT_CHARA_MOTION_SPEED_MOVE_SET |  | 31 | 4 (31) |
| `7300000d` | `s13` | ECT_CHARA_MOTION_SET_OFFSET_ONLY |  | 68 | 3 (68) |
| `7300000e` | `s14` | ECT_CHARA_MOTION_SET_OFFSET_AUTO_LOOP |  | 44 | 3 (44) |
| `7300000f` | `s15` | ECT_CHARA_COLOR_SET |  | 908 | 5 (908) |
| `73000010` | `s16` | ECT_CHARA_COLOR_MOVE_SET |  | 533 | 7 (533) |
| `73000011` | `s17` | ECT_CHARA_COLOR_MOVE_WAIT |  | 83 | 1 (83) |
| `73000012` | `s18` | ECT_CHARA_MOVE_SET |  | 1717 | 13 (950), 11 (704), 19 (63) |
| `73000013` | `s19` | ECT_CHARA_MOVE_WAIT |  | 717 | 1 (717) |
| `73000014` | `s20` | ECT_CHARA_MOVE_SET_WAIT |  | 48 | 13 (48) |
| `73000015` | `s21` | ECT_CHARA_MOVE_PRE_WAIT |  | 741 | 2 (741) |
| `73000017` | `s23` | ECT_CHARA_ROT_SET |  | 4668 | 4 (4668) |
| `73000018` | `s24` | ECT_CHARA_ROT_SET_TARGET |  | 6809 | 7 (6809) |
| `73000019` | `s25` | ECT_CHARA_ROT_SET_TARGET_ALL |  | 100 | 7 (100) |
| `7300001a` | `s26` | ECT_CHARA_ROT_MOVE_SET |  | 1305 | 6 (1305) |
| `7300001b` | `s27` | ECT_CHARA_ROT_MOVE_SET_TARGET |  | 21 | 9 (21) |
| `7300001c` | `s28` | ECT_CHARA_ROT_MOVE_SET_TARGET_ALL |  | 8 | 9 (8) |
| `7300001d` | `s29` | ECT_CHARA_ROT_MOVE_WAIT |  | 174 | 1 (174) |
| `73000021` | `s33` | ECT_CHARA_ROT_STOP |  | 25 | 1 (25) |
| `73000022` | `s34` | ECT_CHARA_OFFSET_ROT_MOVE |  | 39 | 6 (39) |
| `73000023` | `s35` | ECT_CHARA_SCALE_SET |  | 762 | 4 (762) |
| `73000024` | `s36` | ECT_CHARA_SCALE_MOVE_SET |  | 246 | 6 (246) |
| `73000025` | `s37` | ECT_CHARA_SCALE_MOVE_WAIT |  | 55 | 1 (55) |
| `73000026` | `s38` | ECT_CHARA_EFF_ADD |  | 12013 | 25 (12013) |
| `73000027` | `s39` | ECT_CHARA_EFF_WAIT |  | 15 | 2 (15) |
| `73000029` | `s41` | ECT_CHARA_EFF_MOVE_SET |  | 245 | 14 (132), 12 (100), 20 (13) |
| `7300002a` | `s42` | ECT_CHARA_EFF_MOVE_WAIT |  | 158 | 2 (158) |
| `7300002c` | `s44` | ECT_CHARA_EFF_PRE_WAIT | EventExecBtlChara | 130 | 3 (130) |
| `7300002d` | `s45` | ECT_CHARA_EFF_STOP |  | 228 | 2 (228) |
| `7300002e` | `s46` | ECT_CHARA_EFF_KILL |  | 1406 | 2 (1406) |
| `7300002f` | `s47` |  |  | 19 | 17 (19) |
| `73000030` | `s48` | ECT_CHARA_NODEEFF_STOP |  | 2018 | 2 (2018) |
| `73000031` | `s49` | ECT_CHARA_NODEEFF_KILL |  | 1980 | 2 (1980) |
| `73000033` | `s51` | ECT_CHARA_LOOK_SET |  | 4 | 4 (4) |
| `73000035` | `s53` | ECT_CHARA_LOOK_MOVE_SET |  | 2 | 6 (2) |
| `73000039` | `s57` | ECT_CHARA_LOOK_CLEAR |  | 41 | 1 (41) |
| `7300003e` | `s62` | ECT_CHARA_SE_PLAY |  | 18 | 6 (18) |
| `73000040` | `s64` | ECT_CHARA_CHANGE_PARTS |  | 49 | 3 (49) |
| `73000042` | `s66` | ECT_CHARA_CHANGE_ADD_OBJ |  | 1 | 4 (1) |
| `73000044` | `s68` | ECT_CHARA_NODECHARA_ADD |  | 269 | 12 (269) |
| `73000045` | `s69` | ECT_CHARA_NODECHARA_DETACHCHILD |  | 8 | 2 (8) |
| `73000046` | `s70` | ECT_CHARA_NODECHARA_DETACH |  | 259 | 1 (259) |
| `73000047` | `s71` | ECT_CHARA_SET_DISP | EventExecChara | 31 | 2 (31) |
| `73000050` | `s80` | ECT_CHARA_SET_FLAG |  | 6 | 3 (6) |
| `73000051` | `s81` | ECT_CHARA_SET_PROGRAM_MOTION_WAIT |  | 41 | 1 (41) |
| `73000053` | `s83` | ECT_CHARA_MULTI_MOTION_LOOP_CLEAR |  | 1 | 1 (1) |
| `73000054` | `s84` |  |  | 405 | 7 (405) |
| `73000055` | `s85` | ECT_CHARA_ROT_SET_COPY_TARGET_ALL |  | 11 | 7 (11) |
| `73000056` | `s86` | ECT_CHARA_ROT_MOVE_SET_COPY_TARGET |  | 2 | 9 (2) |
| `73000058` | `s88` | ECT_CHARA_FACIAL_MOTION_SET |  | 17 | 4 (17) |
| `7300005f` | `s95` | ECT_CHARA_LOAD_ONE_MOTION |  | 2 | 2 (2) |
| `73000062` | `s98` | ECT_CHARA_NODEEFF_ADD2 |  | 3896 | 19 (3896) |
| `73000063` | `s99` | ECT_CHARA_SE_NON_STREAM_PLAY |  | 4344 | 6 (4344) |
| `73000066` | `s102` | ECT_CHARA_VOICE_PLAY |  | 12 | 2 (12) |
| `73000070` | `s112` |  | EventExecChara | 1 | 1 (1) |
| `73000071` | `s113` | ECT_CHARA_EFF_FADEOUT | EventExecChara | 10 | 2 (10) |
| `73000072` | `s114` | ECT_CHARA_NODEEFF_FADEOUT | EventExecChara | 115 | 2 (115) |
| `73000073` | `s115` | ECT_CHARA_OFFSET_ROT_MOVE_WITHOUT_SORTROT | EventExecChara | 77 | 6 (77) |
| `73000074` | `s116` |  | EventExecChara | 1604 | 4 (1604) |
| `73000075` | `s117` |  | EventExecChara | 1632 | 1 (1632) |
| `73000076` | `s118` |  | EventExecChara | 1 | 5 (1) |
| `73000077` | `s119` |  | EventExecChara | 8 | 7 (8) |
| `73000079` | `s121` |  | EventExecBtlChara | 11 | 3 (11) |
| `7300007a` | `s122` |  | EventExecChara | 3 | 14 (3) |
| `7300007b` | `s123` |  | EventExecChara | 4 | 2 (4) |
| `74000001` | `t1` | ECT_CAMERA_C_TYPE_SET | EventExecCamera | 2785 | 1 (2781), 11 (4) |
| `74000002` | `t2` | ECT_CAMERA_TIME_WAIT | EventExecCamera | 127 | 1 (127) |
| `74000003` | `t3` | ECT_CAMERA_EYE_POS_SET | CameraCFollowDir, CameraCManual, EventExecCamera | 931 | 6 (931) |
| `74000004` | `t4` | ECT_CAMERA_EYE_MOVE_SET | EventExecCamera | 2006 | 9 (1877), 17 (129) |
| `74000005` | `t5` | ECT_CAMERA_EYE_MOVE_WAIT | CameraCManual, EventExecCamera | 1291 | 0 (1291) |
| `74000006` | `t6` | ECT_CAMERA_AT_POS_SET | EventExecCamera | 1004 | 6 (1004) |
| `74000007` | `t7` | ECT_CAMERA_AT_MOVE_SET | EventExecCamera | 1754 | 9 (1749), 17 (5) |
| `74000008` | `t8` | ECT_CAMERA_AT_MOVE_WAIT | EventExecCamera | 192 | 0 (192) |
| `74000009` | `t9` | ECT_CAMERA_EYEAT_POS_SET | CameraCBattlePlayer, CameraCFollow, EventExecCamera | 1613 | 12 (1613) |
| `7400000a` | `t10` | ECT_CAMERA_ROT_MOVE_SET | EventExecCamera | 895 | 4 (895) |
| `7400000b` | `t11` | ECT_CAMERA_ROT_MOVE_WAIT | EventExecCamera | 18 | 0 (18) |
| `7400000c` | `t12` | ECT_CAMERA_LENGTH_MOVE_SET | EventExecCamera | 41 | 3 (41) |
| `7400000d` | `t13` | ECT_CAMERA_LENGTH_MOVE_WAIT | EventExecCamera | 5 | 0 (5) |
| `7400000e` | `t14` | ECT_CAMERA_QUAKE_SET | EventExecCamera | 268 | 2 (268) |
| `74000010` | `t16` | ECT_CAMERA_QUAKE_STOP | EventExecCamera | 226 | 0 (226) |
| `74000018` | `t24` | ECT_CAMERA_FOV_SET | EventExecCamera | 18 | 1 (18) |
| `7400001a` | `t26` | ECT_CAMERA_FOV_MOVE | EventExecCamera | 12 | 3 (12) |
| `7400001d` | `t29` | ECT_CAMERA_MOVE_KILL | EventExecCamera | 1389 | 0 (1389) |
| `74100002` | `t2/0x10` |  | CameraCFieldMap, EventExecCamera | 11 | 0 (11) |
| `74100003` | `t3/0x10` |  | CameraCFieldMap, EventExecCamera | 431 | 2 (431) |
| `74100005` | `t5/0x10` |  | CameraCFieldMap, EventExecCamera | 7 | 2 (7) |
| `74100007` | `t7/0x10` |  | CameraCFieldMap, EventExecCamera | 428 | 2 (428) |
| `74100009` | `t9/0x10` |  | CameraCFieldMap, EventExecCamera | 428 | 2 (428) |
| `7410000b` | `t11/0x10` |  | CameraCFieldMap, EventExecCamera | 428 | 2 (428) |
| `7410000d` | `t13/0x10` |  | CameraCFieldMap, EventExecCamera | 431 | 4 (431) |
| `74100012` | `t18/0x10` | ECT_FIELDMAP_CAMERA_SET_FOLLOW_CHARA | CameraCFieldMap, EventExecCamera | 1144 | 2 (1144) |
| `74100013` | `t19/0x10` | ECT_FIELDMAP_CAMERA_DATA_EFFECT_SET | CameraCFieldMap, EventExecCamera | 1142 | 1 (1142) |
| `74100014` | `t20/0x10` |  | CameraCFieldMap, EventExecCamera | 4484 | 11 (4484) |
| `74100015` | `t21/0x10` |  | CameraCFieldMap, EventExecCamera | 17 | 12 (17) |
| `7420000c` | `t12/0x20` |  | BucManager | 1 | 0 (1) |
| `74200011` | `t17/0x20` |  | BucFrameGp | 2 | 1 (2) |
| `74200012` | `t18/0x20` |  | BucFrameGp | 3 | 1 (3) |
| `74200013` | `t19/0x20` |  | BucFrameGp | 5 | 0 (5) |
| `74300001` | `t1/0x30` | ECT_VARIINDEX_INDEX_START |  | 195 | 0 (195) |
| `74300002` | `t2/0x30` | ECT_VARIINDEX_INDEX_SETTING |  | 6736 | 2 (6736) |
| `75000001` | `u1` |  | Event | 596 | 0 (596) |
| `75000002` | `u2` |  |  | 741 | 1 (741) |
| `75000003` | `u3` |  |  | 1297 | 1 (1297) |
| `75000005` | `u5` |  |  | 806 | 1 (806) |
| `75000007` | `u7` |  |  | 199 | 1 (199) |
| `75000008` | `u8` |  |  | 99 | 3 (99) |
| `7500000b` | `u11` | ECT_EVENT_SET_TIME_EFFECT |  | 339 | 6 (339) |
| `7500000c` | `u12` | ECT_EVENT_WAIT_TIME_EFFECT | Event | 66 | 1 (66) |
| `7500000d` | `u13` | ECT_EVENT_SET_POST_EFFECT |  | 2180 | 8 (1319), 4 (861) |
| `7500000e` | `u14` | ECT_EVENT_WAIT_POST_EFFECT |  | 637 | 0 (637) |
| `7500000f` | `u15` |  |  | 81 | 0 (81) |
| `75000010` | `u16` |  | B3Balloon | 98 | 2 (98) |
| `75000011` | `u17` |  | B3Balloon | 2 | 0 (2) |
| `75000012` | `u18` |  | B3Balloon | 4990 | 2 (4990) |
| `75000015` | `u21` | ECT_EVENT_CONTRAST_WAIT | Contrast | 138 | 1 (138) |
| `75000016` | `u22` | ECT_EVENT_CONTRAST_SET_MIX | Contrast | 3159 | 3 (3159) |
| `75000019` | `u25` |  | B3Balloon | 2 | 3 (2) |
| `7500001a` | `u26` |  | BalloonSelectMode | 93 | 4 (93) |
| `7500001b` | `u27` |  |  | 2 | 5 (2) |
| `7500001d` | `u29` |  |  | 24 | 5 (24) |
| `7500001e` | `u30` |  |  | 2 | 1 (2) |
| `7500001f` | `u31` | ECT_EVENT_VOICE_PLAY |  | 400 | 1 (400) |
| `75000022` | `u34` |  |  | 52 | 2 (52) |
| `75000023` | `u35` |  |  | 79 | 2 (79) |
| `75000024` | `u36` |  |  | 76 | 2 (76) |
| `75000026` | `u38` |  |  | 50 | 1 (50) |
| `7500002e` | `u46` |  |  | 18 | 3 (18) |
| `7500002f` | `u47` |  | Movie | 18 | 0 (18) |
| `75000030` | `u48` |  |  | 18 | 0 (18) |
| `75000033` | `u51` |  | ClearSaveGameMode | 2 | 1 (2) |
| `75000034` | `u52` |  | ClearSaveGameMode | 7 | 0 (7) |
| `75000035` | `u53` |  |  | 2 | 0 (2) |
| `75000037` | `u55` |  | EventTelop | 63 | 1 (63) |
| `75000038` | `u56` |  | EventTelop | 126 | 3 (126) |
| `75000043` | `u67` |  | B3Balloon | 294 | 1 (294) |
| `75000044` | `u68` |  | B3Balloon | 194 | 0 (194) |
| `7500004f` | `u79` |  |  | 3 | 0 (3) |
| `75000061` | `u97` |  | EventTelop | 65 | 3 (65) |
| `75000067` | `u103` | ECT_EVENT_POST_EFFECT_BLUR_SET_TARGET |  | 862 | 6 (862) |
| `75000069` | `u105` |  |  | 2 | 0 (2) |
| `7500006a` | `u106` |  |  | 2 | 0 (2) |
| `7500006b` | `u107` |  |  | 9 | 1 (9) |
| `7500006c` | `u108` |  |  | 9 | 0 (9) |
| `75000082` | `u130` |  |  | 2 | 2 (2) |
| `75000086` | `u134` |  |  | 22 | 1 (22) |
| `75000087` | `u135` |  |  | 5 | 0 (5) |
| `75000088` | `u136` |  |  | 5 | 0 (5) |
| `7500008b` | `u139` | ECT_EVENT_DEL_POST_EFFECT_ALL |  | 148 | 0 (148) |
| `75000091` | `u145` |  | BalloonSelectMode, Event | 4 | 7 (4) |
| `75000092` | `u146` | ECT_EVENT_VOICE_PLAY_TRACK |  | 2 | 2 (2) |
| `75100001` | `u1/0x10` | ECT_SOUND_SE_PLAY |  | 5 | 5 (5) |
| `75100003` | `u3/0x10` | ECT_SOUND_BG_PLAY |  | 805 | 4 (805) |
| `75100004` | `u4/0x10` | ECT_SOUND_BG_STOP |  | 19 | 1 (19) |
| `75100005` | `u5/0x10` | ECT_SOUND_BG_FADEOUT |  | 1220 | 2 (1220) |
| `75100008` | `u8/0x10` | (sound effect, `0x14005210c`) |  | 234 | 5 (234) |
| `75200001` | `u1/0x20` | ECT_CONTRAST_SET | Contrast | 515 | 3 (515) |
| `75200003` | `u3/0x20` | ECT_CONTRAST_WAIT | Contrast | 141 | 1 (141) |
| `75200004` | `u4/0x20` | ECT_CONTRAST_SET_MIX | Contrast | 8 | 3 (8) |
| `75300001` | `u1/0x30` |  | IpuGallery | 243 | 1 (243) |
| `75300002` | `u2/0x30` |  |  | 84 | 2 (84) |
| `75300004` | `u4/0x30` |  |  | 3 | 1 (3) |
| `75300007` | `u7/0x30` |  |  | 14 | 7 (14) |
| `75350001` | `u1/0x35` |  | Event, WMResMgr | 17 | 1 (17) |
| `75400001` | `u1/0x40` |  | MessageItemGp | 29 | 4 (29) |
| `75400002` | `u2/0x40` |  | MessageItemGp | 29 | 3 (29) |
| `75400003` | `u3/0x40` |  | MessageItemGp | 88 | 2 (88) |
| `75400004` | `u4/0x40` |  | EventItemSelectGameMode | 3 | 2 (3) |
| `75400005` | `u5/0x40` |  | EventItemSelectGameMode | 7 | 2 (7) |
| `75400006` | `u6/0x40` |  |  | 3 | 0 (3) |
| `75400007` | `u7/0x40` |  | EventItemSelectGameMode | 3 | 0 (3) |
| `75400008` | `u8/0x40` |  | Event | 134 | 0 (134) |
| `75400009` | `u9/0x40` |  | Event | 3628 | 0 (3628) |
| `7540000a` | `u10/0x40` |  | Event | 3628 | 0 (3628) |
| `77000001` | `w1` |  | Event | 47448 | 1 (47448) |
| `77000004` | `w4` |  | Event | 206 | 1 (206) |
