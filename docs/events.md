# Event commands

What each event bytecode command is and how many operand words it takes. The body format
itself is in `formats.md`, "Event bytecode".


**Event objects (`q10`, `q36`).** `ECT_FIELDMAP_EVENT_OBJECT_VISIBLE_SET` and `q36` share one
case in the field command switch (`0x14028a700`, jump table at RVA `0x28b5a4`, case
`0x14028a7a6`): `{shown}` (operand ≠ 0) goes to `0x140291860` with the name fragment `xEV_`,
showing or hiding every field object whose name holds it (TOWN_03's `obj-xEV_door_r`,
`_door_l`, `obj-xEV_kabe_etc`; TOWN_11 has one). The openings show them as they start
(`q10 1`) and hide them at the end, so outside events they are hidden. `q15` is the next case
(`0x14028a80c`): `{version, shown, flag}` to `0x140291600`.

**Other field and screen commands.** `q11` (`ECT_FIELDMAP_NPC_ENEMY_RESET`, no operands) is case
`0x14028a7fa` of the field switch: it clears a global list (`0x141077eb0`, `0x1401fa830`) and
rebuilds the field's people and monsters (`0x14028d190`): nothing an event's frames show.
`u68` (no operands, `B3Balloon` slots 4 and 8) follows `u67 0` at the end of events 515 and
516, before their fade: the balloon choice screen (`Event/balloonsel/balloonseldata.bsb`),
interactive, not played by the viewer.

**Field object versions (`q15`).** `ECT_FIELDMAP_MAPVISIBLE_SET {version, shown, ...}` shows
or hides the field objects whose placement tags carry that version (`xAM_balloon_ver<N>`:
TOWN_07's airship dock is built in three versions, `obj-map08b_balloon_body` ver1,
`obj-a15_05balloon_b_low_b` ver2, `obj-map08townparts_a_tw7` ver3). Logy's opening
(`EVENT_MM01_OP1_212`) runs `q15 2 1 1` as it starts and `q15 2 0 1` later; in the oracle
frames the version 2 airship shows and the version 3 dock parts (a banner and a balloon behind
Awin and Logy) do not, so objects of a version the event has not shown are hidden. Read from
the frames, not the handler; how the story picks a version outside events is not known yet.

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
at once, each held only by its own waits, and prints each step with its time, in time order
(`BARREL_READ=1` holds each line its unattended reading time, as the viewer does; a thread
that holds lines runs ahead of the others, so the steps are sorted afterwards: before that,
event 212's camera cut at its third line came out after lines 3 to 5, and the viewer, which
applies steps in order up to its clock, never reached it while they showed). That each
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
(`virtual +0x78(-1)`); the opening's `t18 0 0` turns following off, so `t20`'s target stands. When on,
the stage takes the followed character's position as the target (`t20`'s offset still added;
`Stage::camera_follow`, `FieldCamera::eye_target_from`). Of 1,144 uses 589 turn following off;
most of the others come as an event ends, following character 749, "Master Character",
handing the camera back to play; `EVENT_MM02_OP2_141` follows Escha from its first frame.

**The master character** (`Chara.xml` row 749) stands for the protagonist the player chose. As
the executable reads a command's operands (`0x1401ddcbe` and the like, after the slots are
resolved), each that equals 749 becomes what `0x14035ebb0` returns: the global `0x141098a48`,
which the new game's choice screen sets (`0x1400ce4f3`: 1 if the choice is Logy, else 0). So
749 is row 0, Escha, or row 1, Logy. `Player::hero` sets the choice (Escha by default;
`barrel-view --hero logy`) and `Player::slot` swaps 749 for it, the camera's follow (`t18`)
included. The save keeps the choice in its `global` record (+0x38; `docs/formats.md`, Saves).

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
| `t5`/`t6` | an angle (`t20`'s second; not followed: it is 0 in 4,481 of `t20`'s 4,484 uses, and the 7 `t5`s set 0) | |
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

**Still pictures** (`u1/0x30`, `IpuGallery`, 243 uses; case `0x14029b95f` → `0x14029b1f0`):
`{picture}` shows still `picture` over the scene from the table at `0x1408f3d00`
(`crates/formats/src/layout.rs`, `layout::still`: `{width, height, scale x, scale y, folder,
file}`; files in `Data/x64/Ipu_EN`, else `Ipu`). 0 and 1 show none (most uses: 105 of
`u1/0x30 1`), 2 is a black screen (63 uses), 3 white; the rest are logos, title screens and
the `ev_still_*` event pictures. `EVENT_MM01_OP1_111` holds the black one from 0 to 1.5 s,
under its first fade. The viewer draws the still under the window and the text, scaled to the
height and centred; `u2/0x30` (`{picture, hundredths}`, 84
uses, case `0x14029b6f2` → `0x14029b2f0` with the seconds) changes to `picture` fading over
the time (0.5 s between event stills); the viewer fades the new one in over the one before
(how the game blends them inside `0x14029b2f0` is not read).

**Labels and jumps.** `u3 {n}` marks label `n` in its action (an action's labels are found
by scanning its words, `0x14020fcd6`); `u5 {n}` goes on from label `n` at once
(`0x140210be0` → `0x14020fd80` sets the running action's next command, +0x8), `u6 {a, b}`
from label `a + b`. `EVENT_MM01_OP1_111` ends with a fade to black, everyone removed, then
`u5 99`, which jumps over starting actions 64 and 65 (Escha placed in town, the camera handed
back to play, a fade in) to `u3 99` at the end: that part is reached some other way
(skipping the event, presumably). `u9`/`u10/0x40` (3,628 uses each, no operands) bracket each
event's setup action (`u9`, start the setup with its preload `u2`, `u10`): taken to hold the
event while it loads. They do nothing when run: the event handler's switch (`0x1400513b7` and on) has no case for group 0x40's `u8` to `u10` and its ready check (`0x140050d30`) passes them at once, so they only mark the script; `event::MARKERS` drops them (with `u8/0x40`, 134 uses).

**Slots.** `t1/0x30` (`ECT_VARIINDEX_INDEX_START`) and `t2/0x30`
(`ECT_VARIINDEX_INDEX_SETTING {slot, row}`) bind variable character slots (10028 to 10061) to
`Chara.xml` rows, most often the position markers 590 to 603 (6,736 bindings over the events:
34 slots, 54 rows); an event's setup binds them before anything names them, and later commands
name the character by the slot or by the row, so one marker can appear under two numbers.
`event::Player` reads every binding up front and gives the row for the slot wherever a command
names a character: the one acting, the thing it faces or goes to, and the camera's anchors (in
the openings, Escha and Logy face marker 590 where the script says 10028, and the camera aims
at 611 and 616 for 10049 and 10054). With the slots read as rows, the stage looks kind-3 anchors up as characters too
(before, their ids were taken for something else): `EVENT_MM01_OP1_310`, whose camera and
moves aim at markers through slots, went from 21 positions it could not place to 1 (an
anchor on Logy, who is never placed in it); the other openings were at 0 and stay there.

**Waits and marks.** `u15` (no operands) waits 2 seconds (`0x140051412`, the constant at
`0x14071c964`), as `u1` waits 0.3. `u36 {event, seen}` marks an event (its `EVENT_*` number)
seen or not through `0x14035f480`, the record the save keeps (`docs/formats.md`, the events):
the flag, and when seen the count raised, the day stamped (`0x1410a42cc`) and a word copied
from `0x141098a2c` (+0xc); the openings mark the event that has just played. `u34 {flag, value}` sets one of 151 event flags (`0x14035f660`,
the words at `0x1410932d0`; the save's `event/m_eventFlag`). `u38 {trophy}` unlocks a Steam
achievement once (`0x140168390`: a byte per trophy at `0x140e05408`, the `TROPHY_*` names in
0x198-byte rows at `0x140d5fcf8`, then the user-stats interface's +0x38 and +0x50,
`SetAchievement` and `StoreStats`); the openings unlock 1 and 2. Barrel leaves that out on
purpose (it never touches the player's Steam account; listed under `docs/engine.md`'s deliberate differences).

**What else unlocks achievements.** The 53 `TROPHY_*` names (in this order: `PLT_COMPLETE`,
`SYSTEM_001`–`004`, `SUBJECT_001`–`009`, `EVENT_001`–`018`, `ENDING_001`–`009`,
`SYSTEM_005`–`010`, `BATTLE_001`–`006`) are unlocked by number from five places besides
`u38`: the bonus action `Get_Trophy` (`0x14002f3ed`; no row of `Bonus.xml` uses it), a
character reaching level 99 (`0x14032d815`: 45, `SYSTEM_009`), the alchemy level (`0x14107c4c8`)
reaching 50 (`0x14036a6d9`: 46, `SYSTEM_010`), and the battle code (`0x140188390`, from
`0x1401d7b70`): when a battle value at +0x14 of its argument is 0x4a to 0x56 it unlocks
`BATTLE_001` to `006` (0x4a: 48, 0x4c: 47, 0x4e: 49, 0x50: 50, 0x52: 51, 0x53 to 0x56: 51
and 52), the other values none. By the monster list's order those numbers are the
breakers, eyes, cubes and sky monsters (74 to 86), which suggests boss fights, not checked.
The `SUBJECT`, `ENDING`, most `SYSTEM` and `EVENT` ones come from event scripts (`u38`).
`u26 {n, ...}` stores `n` in the action list's +0xe8 and broadcasts (`0x140296a60`; what reads
it is not known), `u46 {id, flag}` sends the event's interface object (+0x1a0) a message of type
0xd with them (not read further). `u16`, `u47`,
`u48`, `u67` and `u68` fall to the end of the event handler's switch (`u67`/`u68` are the
balloon choice screen's, handled elsewhere). The waits on characters (`s6`, `s21`, `s29`) and
`u14` (`ECT_EVENT_WAIT_POST_EFFECT`) are honoured by the schedule though `play-event` still
lists them.

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
timer and queued commands) would fit, but is not read yet. The oracle agrees: in
`EVENT_MM01_OP1_131` the main action starts Escha's walk (`w1 12`: `s18` to the desk, `s21 0
30`), then `p3 0`, then the first line, and the game shows the line with Escha already at
Marion's side; in 212 Logy's first line comes once he has stopped. The scheduler holds a `p3 c`
while any other running action still has character commands (`0x73…`) for `c` to run, so the
line waits for the walk (131: 14.0 → 17.5 s; 212: 8.0 → 11.5 s; 111 and 222 unchanged).

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
| 2 | always 0 | operands 2 to 7 are an event position `{kind, id, point, x, y, z}` (below), read by `0x1402f13f0` in the character's command player `0x14026b230`; kind 0 is a point in the field |
| 4, 14 | always 0 | (4 is the position's `point`) |
| 3 | 0, or 709 in 90 | the position's `id`, unused with kind 0, so 709 changes nothing |
| 5, 6, 7 | x, y, z | the position's offset, in field units |
| 8, 10 | always 0 | the x and z turns, in degrees, beside operand 9's |
| 9 | angle in degrees | facing (the y turn; operands 8 to 10 become the character's +0x240 in radians) |
| 11 | a motion number (played when below 96, through the motion starter `0x1401f4c50`): 31 (`F_WAIT`) in 2,353, 32 `WALK`, 34 `SIT_DOWN`, 33 `RUN`, 67 `EV_ACT_12` | the motion it starts in (the `MOTION_*` enum) |
| 12 | always 0 | the motion's blend in hundredths of a second, and the field character's +0x148 (the listener `0x140071160`) |
| 13 | 1 in 2,166, 2 in 230 | sets the character's +0x5c flag when not 0 (always, in Escha) |

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
| 3 | two: a speed (800 to 2,500), then a height (50 to 300) | a few / 13 / 131 | `0x1402f2680` |
| 1 | ten: a circle (below) | 129 / 5 / 63 | `0x1402f1cd9` |
| 2 | two: a starting and an ending speed | 1 / 1 / 2 | `0x1402f18a0` |

Type 1 (`MdCircle`, vtable `0x140931560`, built by `0x1402f2e90`, updated by `0x1402f3090`)
circles the position: `{ax, ay, az, r0, r1, rise, from, to, speed0, speed1}`. The circle's
plane is turned by `ax`, `ay`, `az` (hundredths of a degree; the facing of what the position is
anchored to is added to `ay`); the angle runs from `from` to `to` (degrees; `to` defaults to 360) at a turning speed
that goes evenly from `speed0` to `speed1` (hundredths of a degree a second; 5 when missing),
so the move takes (to − from) / ((speed0 + speed1) / 2). The radius goes from `r0` to `r1`
and the height by `rise` (field units) in step with the angle's progress. The time is not above 0 when the
speeds' sign is against the way to go, and the game's circle then never runs. Around the hero
(`s18`, kind 5 / 10000 at its feet): `{0, 0, 0, 800, 800, −2000, 90, 270, 15000, 30000}`, a half
turn at 800 units, falling 2000, in 0.8 s; the camera's eye circles slowly (3 degrees a second
and slowing) around a dummy marker. The point is (radius, height, 0) turned about y by the
angle, then about x, z and y by the plane's turns, from the position (row vectors through the
game's matrices `0x1400c2930`, `0x1402f4a70`, `0x140028ce0`); each frame the speed changes
first, then the angle moves on, stopping at `to`. `camera::Mover` plays it.

Type 0 heads for the position at `speed` field units a second (150, a walk, is the
commonest in `s18`; 120 to 140 for the slower walkers): its velocity is the unit direction
times the speed, plus the three extra values turned to the direction of travel (a push or
acceleration; mostly zero). Its time is the distance over the speed; the extra vector is taken
off the velocity every second, and half of it times the time is added at the start, so
the move bows (up, or to a side) and still ends on the point (`0x1402f3720`, update
`0x1402f3a70`). Every move's update takes the frame time off the time left first, then
moves the point, and puts it on the end once the time is used up. Type 3 is a jump `{speed, height}` (`0x1402f2680`, update `0x1402f2760`): `0x1402f28f0` launches
it at `speed` (field units a second) at the angle atan(2 (√H·√(H − dy) + H) / D), with D the
distance across the ground, dy the rise to the point and H the height, and sets the gravity
(+0x8c, 9.8 until then) so that the climb stops after √H / (√H + √(H − dy)) of the time, D over
the speed across the ground: the jump peaks H above where it starts and lands on the point. A
peak below the point, or no distance across, and the jump never runs. Each frame the gravity
comes off the climb, then the point moves.

Type 2 (`MdAccel`, `0x1402f2af0`, update `0x1402f2bb0`; four uses) goes straight to the point
with the speed going evenly from the first operand to the second (field units a second): the
time is the distance over their mean (`0x1402f2e00`).

The extra operand of `s18` (the move's +0x38) is how the mover faces while it goes
(`0x1402f21a0`, once it has moved more than 0.01 across the ground): 0 keeps its facing, 1 turns it
to the way it goes on the ground, 2 along the way in all three directions, tilting it up or down
(the jumps and flights around the hero): it turns +z onto the way it goes, which is the facing
and then a turn about the mover's own x axis by minus the climb's angle. The stage plays both
(`Body::tilt`, the snapshot's `tilts`); `s23` sets the tilt back with the rest of the rotation.

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
- `t10` (`ECT_CAMERA_ROT_MOVE_SET`, `{from, to, seconds ×100, curve}`) rolls the camera about
  its line of sight from `from` to `to` degrees (+0x38, from +0x78 to +0x7c): the update
  (`0x1401ead70`) turns the up vector (0, 1, 0) about the line from the target to the eye by it
  (`0x1405c83c4`, a rotation about an axis). `t12` (`ECT_CAMERA_LENGTH_MOVE_SET`, `{length,
  seconds ×100, curve}`) slides the eye along its line to the target, from its distance when
  the command runs to `length`, each frame after the points have moved. Both follow a curve of
  `curve_data.xml` (`0x140025890(mover, curve, from, to, seconds)`; curves 0, 1, 3 and 13 in the
  events, as in "Curves"), and the stage plays both (`Stage::roll`, the snapshot's `roll`).
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
gesture: an entry of the executable's gesture table, a chain of motion divisions, each step
10000 + n playing division n of the current motion from `SubCharaDiv`/`CharaDiv`, a looping
division or the last holding the chain: Logy's 196 raises his hand to his chin and holds it,
198 lowers it and returns to `F_WAIT`, as the oracle frames show; `barrel gestures EXE [N...]`
prints them); `--stage` prints them (`face 4, talking`). `q4` (adding a field character) places it and turns it to its angle there too, `q5`
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

Not yet: the field camera's unidentified angle (nearly always 0). The characters' motions
during an event are played by the viewer (`docs/renderer.md`).

**Lighting.** The field map's event object (`nspFM::clsFMWorldEvent`, slot 8 at
`0x14028a700`) changes the field's lights for a scene and puts them back. Each `_SET` has an
`_ORG` that restores what the field had (kept at the object's +0x258 to +0x2c8). The stage plays them all: `Stage::lights` holds what the event has set, and the renderer lays it over the
field's own lights. The sun's direction is the field look's own convention (pitch, then yaw), so the event's angles go
straight in.

| Command | Operands | What |
|---|---|---|
| `q42` `ECT_FIELDMAP_LIGHT_PARALLEL_SET` | `{light, r, g, b, pitch, yaw}` | a sun light (0 or 1): colour in 128ths (50, 30, 30 most often), and its direction (0, 0, 1) turned about x by `pitch` and then about y by `yaw`, both in hundredths of a degree (2000 and 23000 most often); handed to the scene's lights (+0x150, `0x1404d72b0`) |
| `q43` `…_PARALLEL_ORG` | `{light}` | that light back as the field had it |
| `q44` `ECT_FIELDMAP_LIGHT_AMBIENT_SET` | `{r, g, b}` | the ambient colour in 128ths (`0x1404d7270`); 80, 90, 85 most often |
| `q45` `…_AMBIENT_ORG` | | the field's ambient back |
| `q50` `ECT_FIELDMAP_CHARA_LIGHT_COLOR_SET` | `{r, g, b, w}` | the characters' shader colour `atCharaLightColor`, all four in hundredths (60, 30, 10, 35 most often) |
| `q51` | | its field value back |
| `q52` `ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_SET` | `{r, g, b, w}` | the same for the rim light, `atCharaRimLightColor` |
| `q53` | | its field value back |
| `q54` | `{light, x, y, z, r, g, b, ar, ag, ab, att0, att1, fall, range}` | one of four point lights (0 to 3) as the scene's lights hold a field's `POINT` (`0x1404d7300`): its place and range in field units, its colour and ambient in 128ths, the constant and linear attenuation in thousandths, and the quadratic term made from `fall` (thousandths) so that, the linear term left out, the light is down to `fall` at the range: (1 − fall·att0) / (fall·range²) |
| `q55` | `{light}` | that point light back |
| `q56` | `{strength}` | `atCharaPointLightStrength` in hundredths (30 most often) |
| `q57` | | back to 0.3 |

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

The `s` commands' handler is mostly `EventExecChara`: its runner (`0x1401efea0`) switches on the word through a jump table over `0x73000003` to `0x7300007b` (at RVA `0x1f2540`), and every word whose entry is not the default case is listed as its. `s80` and `s81` are in no switch found. The other groups' blanks were filled the same way, from every jump-table switch over command words in the executable (a range check, then a table of 4-byte offsets): the `q` commands' handlers are the field character listener and the field map's event object (`nspFM::clsFMWorldEvent`, vtable `0x1408f1390`, its slot 8 at `0x14028a700`, the lighting commands `q42` to `q57`), the `r` commands' `GameModeBattle` (`0x1401a0e90`, called from its slots 5 and 6), the `u` commands' `Event` itself (slot 8, `0x140050fa0`), `B3Balloon` and `IpuGallery`. Only `t1/0x30`, `t2/0x30` (operand prefixes), `u1/0x10` and `u6/0x40` are left without one.

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
| `71000006` | `q6` | ECT_FIELDMAP_CHARA_DELETE_ALL | field characters (`0x140071160`), nspFM::clsFMWorldEvent | 1109 | 0 (1109) |
| `71000007` | `q7` | ECT_FIELDMAP_CHARA_SET_EVENT_MODE | field characters (`0x140071160`) | 5895 | 1 (5895) |
| `71000008` | `q8` | ECT_FIELDMAP_CHARA_COLLISION_FLAG | (field character in or out of the physics world, `0x140071348`) | 8081 | 5 (8081) |
| `7100000a` | `q10` | ECT_FIELDMAP_EVENT_OBJECT_VISIBLE_SET | (no handler found: not in `0x140071030`'s, `0x14007119f`'s or `0x1401ddb80`'s switches) | 771 | 1 (771) |
| `7100000b` | `q11` | ECT_FIELDMAP_NPC_ENEMY_RESET | (no handler found, like `q10`) | 526 | 0 (526) |
| `7100000c` | `q12` | ECT_FIELDMAP_CHANGE_NOT_BGM | field characters (`0x140071160`), nspFM::clsFMWorldEvent | 621 | 1 (621) |
| `7100000d` | `q13` | ECT_FIELDMAP_GIMMICK_RESET | field characters (`0x140071160`) | 17 | 0 (17) |
| `7100000f` | `q15` | ECT_FIELDMAP_MAPVISIBLE_SET | field characters (`0x140071160`), nspFM::clsFMWorldEvent | 53 | 3 (53) |
| `71000010` | `q16` | ECT_FIELDMAP_MAPANIMATION_EXEC | nspFM::clsFMWorldEvent | 2 | 6 (2) |
| `71000020` | `q32` | ECT_FIELDMAP_CHARA_TRANSPARENT_FOCUS | field characters (`0x140071160`) | 249 | 5 (249) |
| `71000021` | `q33` | ECT_FIELDMAP_SET_FIELD_COLOR | field characters (`0x140071160`), nspFM::clsFMWorldEvent | 1 | 4 (1) |
| `71000024` | `q36` |  | field characters (`0x140071160`), nspFM::clsFMWorldEvent | 147 | 1 (147) |
| `7100002a` | `q42` | ECT_FIELDMAP_LIGHT_PARALLEL_SET | nspFM::clsFMWorldEvent | 15 | 6 (15) |
| `7100002b` | `q43` | ECT_FIELDMAP_LIGHT_PARALLEL_ORG | nspFM::clsFMWorldEvent | 18 | 1 (18) |
| `7100002c` | `q44` | ECT_FIELDMAP_LIGHT_AMBIENT_SET | nspFM::clsFMWorldEvent | 50 | 3 (50) |
| `7100002d` | `q45` | ECT_FIELDMAP_LIGHT_AMBIENT_ORG | nspFM::clsFMWorldEvent | 57 | 0 (57) |
| `71000032` | `q50` | ECT_FIELDMAP_CHARA_LIGHT_COLOR_SET | nspFM::clsFMWorldEvent | 34 | 4 (34) |
| `71000033` | `q51` | ECT_FIELDMAP_CHARA_LIGHT_COLOR_ORG | nspFM::clsFMWorldEvent | 38 | 0 (38) |
| `71000034` | `q52` | ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_SET | nspFM::clsFMWorldEvent | 16 | 4 (16) |
| `71000035` | `q53` | ECT_FIELDMAP_CHARA_LIM_LIGHT_COLOR_ORG | nspFM::clsFMWorldEvent | 23 | 0 (23) |
| `71000036` | `q54` |  | nspFM::clsFMWorldEvent | 108 | 14 (108) |
| `71000037` | `q55` |  | nspFM::clsFMWorldEvent | 112 | 1 (112) |
| `71000038` | `q56` |  | nspFM::clsFMWorldEvent | 44 | 1 (44) |
| `71000039` | `q57` |  | nspFM::clsFMWorldEvent | 42 | 0 (42) |
| `72000004` | `r4` | ECT_BATTLE_ADD_EFFECT_ID | GameModeBattle (`0x1401a0e90`) | 1117 | 2 (1117) |
| `72000008` | `r8` |  | GameModeBattle (`0x1401a0e90`) | 1095 | 1 (1095) |
| `7200000d` | `r13` |  | GameModeBattle (`0x1401a0e90`) | 340 | 4 (340) |
| `7200000e` | `r14` |  | GameModeBattle (`0x1401a0e90`) | 681 | 6 (681) |
| `7200000f` | `r15` |  | Event, GameModeBattle | 191 | 0 (191) |
| `72000010` | `r16` |  | Event | 60 | 14 (60) |
| `72000011` | `r17` |  | Event | 76 | 2 (76) |
| `72000012` | `r18` |  | Event | 90 | 7 (90) |
| `72000013` | `r19` |  | Event, GameModeBattle | 6 | 1 (6) |
| `72000014` | `r20` |  | Event | 693 | 2 (693) |
| `72000016` | `r22` |  | Event | 30 | 6 (30) |
| `72000018` | `r24` |  | Event | 12 | 4 (12) |
| `7200001b` | `r27` |  | GameModeBattle (`0x1401a0e90`) | 3 | 1 (3) |
| `7200001c` | `r28` |  | GameModeBattle (`0x1401a0e90`) | 8 | 3 (8) |
| `7200001d` | `r29` |  | GameModeBattle (`0x1401a0e90`) | 3 | 1 (3) |
| `7200001e` | `r30` |  | GameModeBattle (`0x1401a0e90`) | 6 | 3 (6) |
| `7200001f` | `r31` |  | GameModeBattle (`0x1401a0e90`) | 1 | 4 (1) |
| `72000020` | `r32` |  | GameModeBattle (`0x1401a0e90`) | 13 | 6 (13) |
| `72000021` | `r33` |  | GameModeBattle (`0x1401a0e90`) | 1 | 1 (1) |
| `72000022` | `r34` |  | GameModeBattle (`0x1401a0e90`) | 2 | 3 (2) |
| `72000023` | `r35` |  | GameModeBattle (`0x1401a0e90`) | 41 | 2 (41) |
| `73000001` | `s1` | ECT_CHARA_SET_MARKER | EventExecBtlChara | 998 | 5 (998) |
| `73000002` | `s2` | ECT_CHARA_CAMERA_QUAKE_SET | battle character code (`0x1401ae66a`) | 665 | 3 (665) |
| `73000003` | `s3` | ECT_CHARA_TIME_WAIT | EventExecChara | 3849 | 2 (3849) |
| `73000004` | `s4` | ECT_CHARA_POS_SET | EventExecChara | 20753 | 7 (20753) |
| `73000005` | `s5` | ECT_CHARA_MOTION_SET | EventExecChara | 4435 | 4 (4435) |
| `73000006` | `s6` | ECT_CHARA_MOTION_WAIT | EventExecChara | 2966 | 1 (2966) |
| `73000007` | `s7` | ECT_CHARA_MOTION_SET_DIRECT | EventExecChara | 24 | 2 (24) |
| `73000008` | `s8` | ECT_CHARA_MOTION_SET_DIV | EventExecChara | 858 | 4 (858) |
| `73000009` | `s9` | ECT_CHARA_MOTION_SET_SPEED | EventExecChara | 1504 | 2 (1504) |
| `7300000a` | `s10` | ECT_CHARA_MOTION_SET_LOOP | EventExecChara | 1604 | 4 (1604) |
| `7300000b` | `s11` | ECT_CHARA_MOTION_SET_AUTO_LOOP | EventExecChara | 133 | 5 (133) |
| `7300000c` | `s12` | ECT_CHARA_MOTION_SPEED_MOVE_SET | EventExecChara | 31 | 4 (31) |
| `7300000d` | `s13` | ECT_CHARA_MOTION_SET_OFFSET_ONLY | EventExecChara | 68 | 3 (68) |
| `7300000e` | `s14` | ECT_CHARA_MOTION_SET_OFFSET_AUTO_LOOP | EventExecChara | 44 | 3 (44) |
| `7300000f` | `s15` | ECT_CHARA_COLOR_SET | EventExecChara | 908 | 5 (908) |
| `73000010` | `s16` | ECT_CHARA_COLOR_MOVE_SET | EventExecChara | 533 | 7 (533) |
| `73000011` | `s17` | ECT_CHARA_COLOR_MOVE_WAIT | EventExecChara | 83 | 1 (83) |
| `73000012` | `s18` | ECT_CHARA_MOVE_SET | EventExecChara | 1717 | 13 (950), 11 (704), 19 (63) |
| `73000013` | `s19` | ECT_CHARA_MOVE_WAIT | EventExecChara | 717 | 1 (717) |
| `73000014` | `s20` | ECT_CHARA_MOVE_SET_WAIT | EventExecChara | 48 | 13 (48) |
| `73000015` | `s21` | ECT_CHARA_MOVE_PRE_WAIT | EventExecChara | 741 | 2 (741) |
| `73000017` | `s23` | ECT_CHARA_ROT_SET | EventExecChara | 4668 | 4 (4668) |
| `73000018` | `s24` | ECT_CHARA_ROT_SET_TARGET | EventExecChara | 6809 | 7 (6809) |
| `73000019` | `s25` | ECT_CHARA_ROT_SET_TARGET_ALL | EventExecChara | 100 | 7 (100) |
| `7300001a` | `s26` | ECT_CHARA_ROT_MOVE_SET | EventExecChara | 1305 | 6 (1305) |
| `7300001b` | `s27` | ECT_CHARA_ROT_MOVE_SET_TARGET | EventExecChara | 21 | 9 (21) |
| `7300001c` | `s28` | ECT_CHARA_ROT_MOVE_SET_TARGET_ALL | EventExecChara | 8 | 9 (8) |
| `7300001d` | `s29` | ECT_CHARA_ROT_MOVE_WAIT | EventExecChara | 174 | 1 (174) |
| `73000021` | `s33` | ECT_CHARA_ROT_STOP | EventExecChara | 25 | 1 (25) |
| `73000022` | `s34` | ECT_CHARA_OFFSET_ROT_MOVE | EventExecChara | 39 | 6 (39) |
| `73000023` | `s35` | ECT_CHARA_SCALE_SET | EventExecChara | 762 | 4 (762) |
| `73000024` | `s36` | ECT_CHARA_SCALE_MOVE_SET | EventExecChara | 246 | 6 (246) |
| `73000025` | `s37` | ECT_CHARA_SCALE_MOVE_WAIT | EventExecChara | 55 | 1 (55) |
| `73000026` | `s38` | ECT_CHARA_EFF_ADD | EventExecChara | 12013 | 25 (12013) |
| `73000027` | `s39` | ECT_CHARA_EFF_WAIT | EventExecChara | 15 | 2 (15) |
| `73000029` | `s41` | ECT_CHARA_EFF_MOVE_SET | EventExecChara | 245 | 14 (132), 12 (100), 20 (13) |
| `7300002a` | `s42` | ECT_CHARA_EFF_MOVE_WAIT | EventExecChara | 158 | 2 (158) |
| `7300002c` | `s44` | ECT_CHARA_EFF_PRE_WAIT | EventExecBtlChara | 130 | 3 (130) |
| `7300002d` | `s45` | ECT_CHARA_EFF_STOP | EventExecChara | 228 | 2 (228) |
| `7300002e` | `s46` | ECT_CHARA_EFF_KILL | EventExecChara | 1406 | 2 (1406) |
| `7300002f` | `s47` |  | EventExecChara | 19 | 17 (19) |
| `73000030` | `s48` | ECT_CHARA_NODEEFF_STOP | EventExecChara | 2018 | 2 (2018) |
| `73000031` | `s49` | ECT_CHARA_NODEEFF_KILL | EventExecChara | 1980 | 2 (1980) |
| `73000033` | `s51` | ECT_CHARA_LOOK_SET | EventExecChara | 4 | 4 (4) |
| `73000035` | `s53` | ECT_CHARA_LOOK_MOVE_SET | EventExecChara | 2 | 6 (2) |
| `73000039` | `s57` | ECT_CHARA_LOOK_CLEAR | EventExecChara | 41 | 1 (41) |
| `7300003e` | `s62` | ECT_CHARA_SE_PLAY | EventExecChara | 18 | 6 (18) |
| `73000040` | `s64` | ECT_CHARA_CHANGE_PARTS | EventExecChara | 49 | 3 (49) |
| `73000042` | `s66` | ECT_CHARA_CHANGE_ADD_OBJ | EventExecChara | 1 | 4 (1) |
| `73000044` | `s68` | ECT_CHARA_NODECHARA_ADD | EventExecChara | 269 | 12 (269) |
| `73000045` | `s69` | ECT_CHARA_NODECHARA_DETACHCHILD | EventExecChara | 8 | 2 (8) |
| `73000046` | `s70` | ECT_CHARA_NODECHARA_DETACH | EventExecChara | 259 | 1 (259) |
| `73000047` | `s71` | ECT_CHARA_SET_DISP | EventExecChara | 31 | 2 (31) |
| `73000050` | `s80` | ECT_CHARA_SET_FLAG | (no handler found) | 6 | 3 (6) |
| `73000051` | `s81` | ECT_CHARA_SET_PROGRAM_MOTION_WAIT | (no handler found) | 41 | 1 (41) |
| `73000053` | `s83` | ECT_CHARA_MULTI_MOTION_LOOP_CLEAR | EventExecChara | 1 | 1 (1) |
| `73000054` | `s84` |  | EventExecChara | 405 | 7 (405) |
| `73000055` | `s85` | ECT_CHARA_ROT_SET_COPY_TARGET_ALL | EventExecChara | 11 | 7 (11) |
| `73000056` | `s86` | ECT_CHARA_ROT_MOVE_SET_COPY_TARGET | EventExecChara | 2 | 9 (2) |
| `73000058` | `s88` | ECT_CHARA_FACIAL_MOTION_SET | EventExecChara | 17 | 4 (17) |
| `7300005f` | `s95` | ECT_CHARA_LOAD_ONE_MOTION | EventExecChara | 2 | 2 (2) |
| `73000062` | `s98` | ECT_CHARA_NODEEFF_ADD2 | EventExecChara | 3896 | 19 (3896) |
| `73000063` | `s99` | ECT_CHARA_SE_NON_STREAM_PLAY | EventExecChara | 4344 | 6 (4344) |
| `73000066` | `s102` | ECT_CHARA_VOICE_PLAY | EventExecChara | 12 | 2 (12) |
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
| `74300002` | `t2/0x30` | ECT_VARIINDEX_INDEX_SETTING (slot to row, see Slots) |  | 6736 | 2 (6736) |
| `75000001` | `u1` |  | Event | 596 | 0 (596) |
| `75000002` | `u2` |  | Event | 741 | 1 (741) |
| `75000003` | `u3` |  | Event | 1297 | 1 (1297) |
| `75000005` | `u5` |  | Event | 806 | 1 (806) |
| `75000007` | `u7` |  | Event | 199 | 1 (199) |
| `75000008` | `u8` |  | Event | 99 | 3 (99) |
| `7500000b` | `u11` | ECT_EVENT_SET_TIME_EFFECT | Event | 339 | 6 (339) |
| `7500000c` | `u12` | ECT_EVENT_WAIT_TIME_EFFECT | Event | 66 | 1 (66) |
| `7500000d` | `u13` | ECT_EVENT_SET_POST_EFFECT | Event | 2180 | 8 (1319), 4 (861) |
| `7500000e` | `u14` | ECT_EVENT_WAIT_POST_EFFECT | Event | 637 | 0 (637) |
| `7500000f` | `u15` |  | Event | 81 | 0 (81) |
| `75000010` | `u16` |  | B3Balloon | 98 | 2 (98) |
| `75000011` | `u17` |  | B3Balloon | 2 | 0 (2) |
| `75000012` | `u18` |  | B3Balloon | 4990 | 2 (4990) |
| `75000015` | `u21` | ECT_EVENT_CONTRAST_WAIT | Contrast | 138 | 1 (138) |
| `75000016` | `u22` | ECT_EVENT_CONTRAST_SET_MIX | Contrast | 3159 | 3 (3159) |
| `75000019` | `u25` |  | B3Balloon | 2 | 3 (2) |
| `7500001a` | `u26` |  | BalloonSelectMode | 93 | 4 (93) |
| `7500001b` | `u27` |  | B3Balloon, Event | 2 | 5 (2) |
| `7500001d` | `u29` |  | B3Balloon, Event | 24 | 5 (24) |
| `7500001e` | `u30` |  | B3Balloon, Event | 2 | 1 (2) |
| `7500001f` | `u31` | ECT_EVENT_VOICE_PLAY | B3Balloon, Event | 400 | 1 (400) |
| `75000022` | `u34` |  | B3Balloon, Event | 52 | 2 (52) |
| `75000023` | `u35` |  | B3Balloon, Event | 79 | 2 (79) |
| `75000024` | `u36` | (marks an event seen: `{event, seen}`, `0x14035f480`) | B3Balloon, Event | 76 | 2 (76) |
| `75000026` | `u38` |  | B3Balloon, Event | 50 | 1 (50) |
| `7500002e` | `u46` |  | B3Balloon, Event | 18 | 3 (18) |
| `7500002f` | `u47` |  | Movie | 18 | 0 (18) |
| `75000030` | `u48` |  | B3Balloon, Event | 18 | 0 (18) |
| `75000033` | `u51` |  | ClearSaveGameMode | 2 | 1 (2) |
| `75000034` | `u52` |  | ClearSaveGameMode | 7 | 0 (7) |
| `75000035` | `u53` |  | B3Balloon, Event | 2 | 0 (2) |
| `75000037` | `u55` |  | EventTelop | 63 | 1 (63) |
| `75000038` | `u56` |  | EventTelop | 126 | 3 (126) |
| `75000043` | `u67` |  | B3Balloon | 294 | 1 (294) |
| `75000044` | `u68` |  | B3Balloon | 194 | 0 (194) |
| `7500004f` | `u79` |  | Event | 3 | 0 (3) |
| `75000061` | `u97` |  | EventTelop | 65 | 3 (65) |
| `75000067` | `u103` | ECT_EVENT_POST_EFFECT_BLUR_SET_TARGET | Event | 862 | 6 (862) |
| `75000069` | `u105` |  | Event | 2 | 0 (2) |
| `7500006a` | `u106` |  | Event | 2 | 0 (2) |
| `7500006b` | `u107` |  | Event | 9 | 1 (9) |
| `7500006c` | `u108` |  | Event | 9 | 0 (9) |
| `75000082` | `u130` |  | Event | 2 | 2 (2) |
| `75000086` | `u134` |  | Event | 22 | 1 (22) |
| `75000087` | `u135` |  | Event | 5 | 0 (5) |
| `75000088` | `u136` |  | Event | 5 | 0 (5) |
| `7500008b` | `u139` | ECT_EVENT_DEL_POST_EFFECT_ALL | Event | 148 | 0 (148) |
| `75000091` | `u145` |  | BalloonSelectMode, Event | 4 | 7 (4) |
| `75000092` | `u146` | ECT_EVENT_VOICE_PLAY_TRACK | Event | 2 | 2 (2) |
| `75100001` | `u1/0x10` | ECT_SOUND_SE_PLAY |  | 5 | 5 (5) |
| `75100003` | `u3/0x10` | ECT_SOUND_BG_PLAY | Event | 805 | 4 (805) |
| `75100004` | `u4/0x10` | ECT_SOUND_BG_STOP | Event | 19 | 1 (19) |
| `75100005` | `u5/0x10` | ECT_SOUND_BG_FADEOUT | Event | 1220 | 2 (1220) |
| `75100008` | `u8/0x10` | (sound effect, `0x14005210c`) | Event | 234 | 5 (234) |
| `75200001` | `u1/0x20` | ECT_CONTRAST_SET | Contrast | 515 | 3 (515) |
| `75200003` | `u3/0x20` | ECT_CONTRAST_WAIT | Contrast | 141 | 1 (141) |
| `75200004` | `u4/0x20` | ECT_CONTRAST_SET_MIX | Contrast | 8 | 3 (8) |
| `75300001` | `u1/0x30` |  | IpuGallery | 243 | 1 (243) |
| `75300002` | `u2/0x30` |  | IpuGallery | 84 | 2 (84) |
| `75300004` | `u4/0x30` |  | IpuGallery | 3 | 1 (3) |
| `75300007` | `u7/0x30` |  | IpuGallery | 14 | 7 (14) |
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

### Camera quakes (`t14`, `t16`)

`t14` (`ECT_CAMERA_QUAKE_SET`, 268 uses) is `{quake, seconds × 100}`: `EventExecCamera`'s
case (`0x1401e102d`) takes the number and the second operand over 100 (`0x14071a9b0`) and
starts quake `cq_obj_<quake>` on the camera's quake player (+0x1c8, set up by
`0x140035ba0`: the seconds at +8, six envelope players built). The events use 99900 (999 s,
the "until stopped" value); `t16` (`ECT_CAMERA_QUAKE_STOP`) clears the player (+0x1d0,
+0x1d8). The quakes are `Saves/cameraQuake/camera_quake_data.xml` (`barrel_formats::quake`,
`barrel quakes INSTALL`): 61, each six tracks of up to 16 keys `{value, frame}` at 60 frames
a second, linear between keys, held or looped. The tracks come in opposite-signed pairs
(`cq_obj_001`: 0 to -0.3 and 0 to +0.3 over two seconds; `cq_obj_031`: ±0.01 on two pairs),
each axis's bounds: the player's offset function (`0x140035920`) takes tracks 2k and 2k + 1
at the current time, ×100 to whole numbers, draws a random whole number between them with
the game's range draw (`0x1404ca660`, `barrel_game::rng::Rng::range`), and returns the
three over 100, ×100 again: an offset in field units, one unit steps, a new one every frame.
Its caller (`0x1401e16fd`) hands it to the camera (slot 0xd0); the renderer adds it to the eye
and the target alike (taken: what the camera does with it is not read). The update
(`0x140035880`) adds the frame time to the elapsed time (+0xc) and stops the quake past its
seconds (or wraps, with +0x10 set); the envelopes advance by the frame time over the seconds
(not read further: the renderer runs them at 60 frames a second). `Step::Quake` and
`Step::QuakeStop` carry them; the stage keeps the running one.

Deliberate difference: the viewer's shake draws from its own generator (the game's kind,
seeded apart), not the game's shared one, so the jitter differs frame by frame.

## Atelier Sophie 2's events

Sophie 2 keeps its events in `PACK01.PAK` (`event/event_jp/<group>/event_*.ebd`, plus `parts`
and `skill`; `barrel pak-extract PACK01.PAK .ebd DIR` writes them out): 2,780 files, 45,074
action bodies, 1.3 million words, and the container is Escha's (every file reads). The
command words are the same kind (top byte 0x70 to 0x7f, a group, flags and a number) but the
set is larger: 361 commands against Escha's 238, the commonest `w1` (the step marks, as in
Escha), `s40`, `p1`, `t12`, `s36`, `u17`.

`barrel ebd-measure DIR` measures an operand table from a game's own bodies (the words up to
the next command word; `None` where that varies) and prints it: over Escha's events it gives
exactly the hand-checked table, less the five commands only the script macros use, which is
what makes it trustworthy as a first draft. Over Sophie 2's it gives 339 fixed counts and 22
varying, and all 45,074 bodies decode with it (`OperandTable::sophie2`,
`ebd_tables::SOPHIE2`).

Sophie 2's executable registers 280 `ECT_*` names the same way
as Escha's (`tools/ect_names.py` → `work/sophie2_ect.txt`), naming 182 of the 361 words its
events use (the table's comments carry them). The two games share 192 names, but only 30 keep
their word: the groups were renumbered (`ECT_CAMERA_AT_POS_SET` is `74000006` in Escha,
`74000007` in Sophie 2; `ECT_CAMERA_EYE_MOVE_PRE_WAIT` `7400001e` against `74000006`), so a
player for both has to go through the names, not the words. Of the shared names both games
use, 102 take the same operand count and 10 changed: `ECT_FIELDMAP_CHARA_ADDITION` 2 → 1,
`_ADDITION_EX` 14 → 13, `ECT_FIELDMAP_LIGHT_PARALLEL_SET` 6 → 7, `ECT_CHARA_MOTION_SET` 4 →
varying, `_SET_LOOP` 4 → 5, `ECT_CHARA_VOICE_PLAY` 2 → 3, `ECT_CHARA_SET_MARKER` 5 → 11,
`ECT_CHARA_CAMERA_QUAKE_SET` 3 → varying, `ECT_CHARA_SET_FLAG` 3 → 2, and
`ECT_CAMERA_C_TYPE_SET` varying → 1.

`event::Player::translated` plays another game's event with Escha's player: it decodes with
that game's table, then carries each command over to Escha's word by name where both games
give the name the same operand count; a command named in neither game keeps its word when
both tables agree on its count (`w1`, the step marks; the still-picture and fade commands).
Every other command gets a flag bit no Escha word has (`event::UNMAPPED`), so a renumbered
word is never read as the Escha command that happens to sit on it. `BARREL_GAME=sophie2
barrel play-event FILE.ebd` runs a Sophie 2 event so: its first, `event_mm01_010`, plays its
fades, stills, waits and the music stop; the commands left unmapped are what to read next.

Most of what is left unmapped does not carry over by name at all. Sophie 2's commonest,
`s40` (`ECT_CHARA_MOTION_SET_LOOP`, 15,125 uses, five operands against Escha's four), shows
why: Escha's `{character, motion, blend, loop}` holds `Chara.xml`-style characters (0, 1, 11,
749), motions from the executable's small motion table (31, 32, 40) and blends in hundredths
(0 to 100); Sophie 2's holds characters numbered from 10,028 up, five- and six-digit motion numbers (12501, 172501), blends of 1,500 to 8,000 (likely
milliseconds) and a fifth word (0, or -1 in 612). So Sophie 2's character, motion and timing
commands need its own tables (its characters, its motions) rather than Escha's, and the
translation is right to leave them out. The characters at least work as in Escha: they are
slots, bound by `t2/0x30` (`{slot, row}`, Escha's `ECT_VARIINDEX_INDEX_SETTING`, same word)
in the event's setup (`event_mm01_*`: 10029 → row 0, 10030 → 1, 10032 → 3, 10033 → 4, 10028 →
420), which the player already reads through its slots. The motions are simpler than Escha's
too: no motion table, the number names an animation file. Each character has a motion
container (`data/x64/character/<model>/<model>_motion.elixir.gz`, in `PACK00_02.PAK` and the
DLC packs) of `g1a` files named by six-digit numbers (`PC01A_000900.g1a`, `PC01A_001000.g1a`
… 77 for `pc01a`), and the shared ones by body type hold the common motions (`cf00` 510,
`cm00`, …: `CF00_172501.g1a`), so `s40`'s motion 1000 is `<model>_001000.g1a` and 172501 is
`CF00_172501.g1a`. The third operand (0, 1,500, 3,000, …) is still to be read; the fifth is
0 or -1.

Which containers a character's motions are in comes from `saves/chara` (`PACK02.PAK`):
`chara.xml` gives the row's `BaseTag` (`PC01A`), `charabase.xml` the base's two motion sets
(`motset0="CHARA_MOTIONSET_PC01A"`, `motset1="CHARA_MOTIONSET_CF01"`: its own, then the shared
one of its body type), `charamotionset.xml` each set's `Directory` and `BaseName`, and
`charamotionconvert.xml` motion numbers a base swaps for others (`PC00A`: 0 → 800000). A motion
is looked for in the first set's container, then the second's (`sophie2::Motions::files`).
Over the `mm01` events every `s40` on a slot bound to row 1 (`PC01A`, 45 uses) finds its file
in `pc01a_motion` or `cf01_motion`. (`BARREL_GAME=sophie2 barrel ebd-ops DIR WORD` reads
operand values with Sophie 2's table.)

## Script macros (`Script/*.nut`)

Escha's `Script` folder is 36 plain Squirrel sources (Shift-JIS, CRLF, Japanese comments). Some
are command macros written with the event commands themselves: a function fills an array with
`ECT_*` words and operands, copies it word by word (`SQ.copyProgramMacro(value, index)`) and
runs it (`SQ.execProgramMacro(name)`, or `execProgramMacroFast`). So they play through the same
commands as the `.ebd` events.

- `macro_encount.nut`: the field's forced encounters (`forceEncount` and one per scripted boss,
  e.g. `forceEncountSkySlug`, `forceEncountDragon`, `forceEncountBeast`): the
  player and target taken out of play (`ECT_FIELDMAP_CHARA_SET_EVENT_MODE`, collisions off),
  the camera on the target, its roar motion (`MOTION_EX10`), effects, voice, then collisions
  back on. They play the lead-in only; the battle and its formation come from the code
  (`docs/tables.md`, battle notes).
- `macro_battle.nut`: battle camera and movement helpers (`selectMoveCamera`,
  `setTargetCameraCraneShot`, `setCommandMove`, `setEvadeChara`/`setEvadeCamera`,
  `setProgramPosACEvade(ranks)`, `setTornadoRolling`/`Falling`), naming fighters by
  `CHARA_EX_DEF`/`CHARA_EX_CUR` (`ECT_SET_DEFCHARA_BCID` binds the default to a battle
  character).
- `ai_*.nut`: the field creatures' behaviour (`_in`/`_out` pairs per kind: bear, cow, dog,
  slags, dragon, spirit...), calling the host through `SQ.getPlayer`, `SQ.getActiveChara` and
  the like.
  Each is a bare block run per tick: a state machine over `pcOwn.getState()` (the `Enum.STATE_*`
  values of `utility.nut`'s `Enum` class: 0 select, 1 wait, 2 waiting, 3 to 7 the move steps,
  100 and up free states), setting motions (`setMotionTag`), timers, movement and emotion
  icons on the creature, with `Val`'s constants (`MOTION_BLEND_FRAME` 0.2, `CHARA_ROT_LIFE`
  0.35, ...). `barrel_game::squirrel` parses the subset the 36 scripts use (locals, constants,
  `if`/`switch`/`for`/`foreach`/`while`, functions with `...`, classes of static members,
  arrays, calls and members): `barrel nut-parse ROOT` parses all 36. `squirrel::Interp` runs
  them as a tree walker (switches fall through to the next case until a `break`; a function sees
  its own scope and the root table, not its caller's locals), the game's side behind a `Host`
  trait (its objects' methods and the named constants). `barrel nut-run ROOT FILE TICKS` runs one
  on a stand-in creature (state and timer kept, every other question answered with 0 or false)
  after `utility.nut`: `ai_dog_out.nut`, the dog not yet aware of the player, goes from select to
  wait (motion 31, `F_WAIT`, blend 0.35), then to waiting until its timer runs out, then draws one
  of its four patterns (`SQ.randRange(0, 3)`; the first, state 102, barks with motion 26).
  All 29 `ai_*` scripts run 30 ticks that way without an error. What they ask of the game:
  the creature's `setState` (544 calls in the sources), `setMotionTag` (238), `isEndMotin`
  (sic, 169), `moveRotation`, `stopMovePosition`, `isMovePosition`, `getPosition`,
  `getMoveSpeed`, `movePosition`, emotion icons, `isHitCollision`, a timer, custom position,
  rotation and parameter slots, range checks (`setIsJudgeRange`, `isMaxRangeIn`, `isRangeIn`)
  and spots near the player (`getPositionToPlayerBehind`, `getPositionToPlayerEscape`,
  `getRandomRangePosition`); and of `SQ`: `getPlayer`, `getActiveChara`, `randRange`/`randRangeF`
  and the macro calls. The executable binds 175 names in all (`tools/sq_binds.py EXE ASM`: each
  name, its native function and the stub that converts its arguments; `setMotionTag` is
  `0x140231fa0`, `isEndMotin` `0x140232360`, `movePosition` `0x140232000`, `setState`
  `0x140131f60`), which is where each method's meaning is to be read.
  Read so far: `setMotionTag (tag, division, blend, speed, a, b)` starts the motion on the
  character's motion object (+0xc0, virtual +0x48) and then sets its speed when it differs;
  `isEndMotin` is true once no motion is queued (+0x90 equals +0x98) or the done flag (+0x5e)
  is set; `movePosition (to, speed)` builds a straight mover from where the character stands
  (+0x230, `0x1402f1710`) and starts it (`0x1402f25b0`); `getWaitTime` and `getMoveSpeed` are
  virtuals of the creature's class; positions travel as host `Vector3`s (`SQ.getVector3`,
  `SQ.getXByVector3`). `barrel_game::creature::Creature` plays a script's side of those (a
  timer counting down, motions that end after a set time, straight moves, emotion icons, the
  custom slots, and from the creature's field enemy record its speeds, wait, sight cone and
  reach; the script object is its "brain", `nspFM::clsFMBrainEnemy` (vtable `0x1407cba38`) or
  `clsFMBrainNpc`, whose +0x118 is the record. Its spots: `getRandomRangePosition` (`0x140232f30`)
  moves where it stands on x and z each by a draw of -32768 to 32767 over 32767 times its sight,
  keeping the first spot within its reach of home on both axes (up to 64 draws);
  `getPositionToPlayerBehind` (`0x1402330d0`) is the player's place plus the direction from the
  creature through the player times a float draw of 1 to 3 (the function mixes the creature's
  metres with the player's field units, so the scale is not settled); float draws use a
  generator of their own (`0x1404ca350`, an xorshift at `0x1410c2fd8`); `getPositionToPlayerEscape (angle)`
  (`0x140233240`) goes from where the creature is in a direction within `angle` degrees centred
  on the way the player faces (the `ai_*_in` scripts pass 90), up to its sight away, kept within
  its reach (the distance draw's lower bound is not settled). Whether the player is in sight is
  the brain's +0xd4 (`setIsRange`/`getIsRange`): `utility.nut`'s `isRangeInOut` tests the sight
  cone each tick and, when the answer changes while range judging is on, sets it, zeroes the
  timer and returns true, so the script starts over; the brain then plays the record's first
  script (`ai_dog_in2`, the chase or flight) while it is set and the second (`ai_dog_out`, the
  idle wandering) while it is not (`0x140233a27`), and `getMoveSpeed` gives the running speed)
  A new brain starts with that flag clear and range judging on (`0x140235b74` writes 0x100 over
  +0xd4 and +0xd5). `barrel_game::field_ai::Mob` is one field enemy so: its `Creature`, its own
  script world with `utility.nut` loaded, and its two scripts, one of them run each frame by the
  flag. `barrel field-sim ROOT AREA FRAMES` runs an area's enemies while a stand-in player walks
  west through them: at `AREA_01_01` the Ore Rats (`ai_move` idle, `ai_tracking` aware) walk off
  to wander and wait, the Stray Dog (`ai_dog_out`) waits, barks and runs about, and the Ore Rat
  the player passes behind notices it there (its 180° view), then follows it, turning toward it
  again every tenth of a second or so (`Val.CHARA_TRACKING_TIME`, 0.1)., and
  `barrel nut-run ROOT FILE FRAMES` drives one 600 units from the player: the unaware dog
  waits (`F_WAIT`), plays one of its idle actions (`EX9`, `EX8`), and runs off (`B_RUN`) to a
  spot and waits again.

`macros::read` reads them without a Squirrel VM (the arrays are literal): each function's name,
parameters and array; `Macro::words` resolves the array into an event action's words, with the
call's arguments for the parameters, the command names (`names.rs`), the other constants the
executable registers (`sq_consts.rs`, 453 of them, from `tools/sq_consts.py`: `mov r8d, value;
lea rdx, name; call 0x14012d910`) and the motions by name (`MOTION_` + the executable's motion
table). `barrel macros ROOT` checks them all: 32 of the 36 macros read and decode, all eleven
encounters among them. Four compute words in the function body, which a literal reader cannot
(`setCommandMove`'s local `s32XAccel`, `setTargetCameraCraneShot`'s `s32IsUp ? 800 : 200`,
`setTornadoRolling`, `fieldEventRaidEnd`). `barrel play-macro ROOT/Script/FILE.nut NAME [--timed] ARGS...` plays one
through the event player as a one-action event, its parameters given as `Chara.xml` rows:
`forceEncount` with Escha (0) and a stand-in target (1) takes both out of play, moves the
camera onto the target and back to 700, waits half a second, plays the target's roar (`EX10`)
and turns it to the player, plays Escha's `MIX_ERU`, then gives both their collisions back.
Five commands appear only in the macros, never in the
events; their operand counts come from every use there (`ebd_tables.rs`):
`FIELDMAP_CHARA_SET_BRAIN_MODE` 2, `SET_DEFCHARA_BCID` 1, `CHARA_SET_PROGRAM_ROT` 1,
`CHARA_LOOK_SET_TARGET` 7, `CHARA_LOOK_MOVE_WAIT` 1.

## When events play (`Event/eventCheck/EventCheckDataAll.ecd`)

Read by `0x1400548f0` into a map at `0x140df9a10` keyed by event number
(`barrel_formats::event_check`): an i32 count (1,165, one per `EVENT_*`), then per event 14 i32
(the record's +0x28 to +0x5c; word 1 is 10, 100 or 500 in the events looked at, a priority by
the look of it; words 8 to 13 are -1 in most), a group count (613 events have 2, 309 none, 160
have 3) and, per group, a word count and that many words (kept per group at the record's
+0x60). The groups hold commands: a code `0x70000000 + n` and its operands, sized by a 167-entry
table at `0x140cf9640` (16 bytes each: word count with the code, trigger group or -1, name;
`barrel_formats::event_check_codes`). Codes 0x00 to 0x7d are `EVENT_CONDITION_*` (event seen or
not, event flag compares, items, friendship, dates, reclaim, DLC ...), 0x7e to 0xa6
`EVENT_TRIGGER_*` (map change before/after, standing in a field map, talking to someone, mixing,
sleeping, event end ...), each in one of 34 trigger groups (`EVENT_CHECK_TRIGGER_GROUP_*`), and
`0x1400555d0` files each event under the groups of its triggers (map at `0x140df9a18`). Group 0
holds conditions only; triggers sit in the later groups. Every group in the file splits exactly
by the table. Most used: `EVENT_OCCURRENCE` (1,006), `EVENT_NOT_OCCURRENCE` (767),
`EVENTFLAG_EQ` (391), `EVENTFLAG_MT` (279), `EVENT_PASSAGE_MT` (268), `TRIGGER_MAP_FROM_TO_AFTER`
(253). Event 30, for one: `EVENT_OCCURRENCE 15`, `EVENT_NOT_OCCURRENCE 31`, `EVENTFLAG_MT 4 1`,
then `TRIGGER_MAP_FROM_TO_AFTER 0 4 0 5`. The file's 121,876 bytes read exactly so. `Event/eventprocess/EventProcessDataAll.epd`
(21,708 bytes, the same count first) is not read yet.

### Which event starts (`0x140214d70`, `barrel_game::event_start`)

Called from 31 places with a trigger group. Nothing starts while `0x141077ef0` or `0x141077ef1`
is set or `0x1402969b0` says no. Otherwise it walks the group's list. Each entry is 16 bytes:
event, the group holding the trigger, 1, and priority (word 1). The list is built by
`0x1400555d0`: a group goes under its first trigger code, found by a word-by-word scan for
0x7000007e..0xa6. It is sorted highest priority first by the library sort `0x140057570`,
which does not keep order among equals.

An entry passes when `0x140214c00` passes for group 0 and for its own group:

- **Gate:**
  - Word 0 = 1 fails while `0x1410782e2` is set (cleared at day end).
  - A seen event (`0x14035f270`) needs word 2 (repeatable).
  - Word 4 set needs event flag 1.
  - Words 8–10 and 11–13 are checked against the date by `0x14035c140` and `0x14035c160`,
    a range going by the names.
- **Group:** every command must pass, in order (`0x140214920` stops at the first failure;
  `0x140213670` evaluates one).

Passing entries are kept unless an earlier kept one has a different priority or the same
event. So only the top passing priority counts. One is picked with `0x1404ca7a0 %` the count;
none gives event -1. Barrel keeps file order among equal priorities. Matching the game's pick
among equals needs its sort's order, which is not reproduced yet.

## What events change (`Event/eventprocess/EventProcessDataAll.epd`)

Read by `0x140059c60` into a map at `0x140df9eb0` (`barrel_formats::event_process`). The file
is an i32 count (1,165, one per event), then per event a group count and, per group, a word
count and that many words. 851 events have 2 groups, 309 none; the file reads exactly. The
words are ordinary event commands handled by `0x140052391`, the same handler as `w1`, and
every group ends with `ECT_END`. Ten commands occur:

| Code | Operands | What it does |
|---|---|---|
| `0x77100002` | 1 | money down by the operand (`0x14036a8e0` of its negative) |
| `0x77100004` | 2 | an item set (`0x1400d2ab0`) into the basket (where = 0) or the container (1) |
| `0x77110001` | 1 | a character joins the 11-slot companion list (`0x140362030` → `0x140330220`) |
| `0x7711000a` | 2 | not read yet |
| `0x77120001` | 2 | an event marked seen or not (`0x14035f480`) |
| `0x77130002` | 2 | event flag change (`0x14035f6b0`) |
| `0x77130004` | 2 | event flag set (`0x14035f660`), the commonest command (41 uses) |
| `0x77140002` | 2 | book knowledge level (`0x140369560`) |
| `0x77140005` | 2 | a knowledge level in the table at `0x14107a8a0` (`0x140369460`) |
| `0x77160002` | 1 | days pass (`0x14035bf10`) |

The handler names more of the family:

- `0x77100001` raises money.
- `0x77100003` goes through `0x14036a990` and `0x14036a960`.
- `0x77110002` takes a character off the companion list.
- `0x77110003`–`6` join or leave depending on whether the event in the second operand has
  been seen.
- `0x77130005` raises an item's knowledge (`0x1403692f0`).
- `0x77140003` and `4` set map knowledge.
- `0x77141001` sets a place and `0x77141002` a route.

When the groups run (all three readers go through `0x14005a070 (event, group, out)`, which
copies the group or gives a bare `ECT_END` if there is none):

- **Group 0, when the event starts.** `0x1402178d0` builds the event runner and loads the
  script by the name table at `0x140d809b0`. It then hands group 0 to the runner
  (`0x140210710`) alongside the script.
- **Group 1, when the script reaches `ECT_END`.** The handler (`0x140052c0b`) runs group
  `[runner+0x60]` of event `[runner+0x58]`. Nothing runs if the runner has already ended,
  if `0x141077ef0` is set, or if the runner's +0x68 or +0x6a is set. The runner's +0x58 holds
  the 16-byte pick result (`event_start`): event, trigger group, 1, priority. So +0x60 is
  the 1 every trigger list entry carries, which makes this group 1.
- **Both groups, without a script.** `0x1402130c0` (from `0x140212c60`) marks the event seen
  (`0x14035f480`) and runs group 0, then group 1, in a scriptless runner.

Most events keep their changes in group 0; 40 have any in group 1.

#### One command (`0x140213670`, `event_start::command`)

A trigger code fails unless its trigger group is the one that fired. Then a byte-indexed jump
table picks the case: index bytes at `0x14021486c` (one per code), targets at `0x140214724`
(82 cases). Read so far:

| Code | Passes when |
|---|---|
| `EVENT_OCCURRENCE e` / `EVENT_NOT_OCCURRENCE e` | event `e` seen / not seen (`0x14035f270`) |
| `EVENT_PASSAGE_MT/LT/EQ e n` | `e` seen and today − the day it was seen (`0x14035c040` − `0x14035f3a0`) ≥ / ≤ / = `n` |
| `EVENTFLAG_MT/LT/EQ f v` | flag `f` ≥ / ≤ / = `v` (MT and LT include equality) |
| `TRIGGER_MAP_FROM_TO_BEFORE/AFTER a b c d` | all four equal the fired values (from kind, from map, to kind, to map by event 30's `0 4 0 5`) |
| `TRIGGER_MAP_FIELDMAP_BE m` | the fired map is `m` |

| the 14 one-value triggers (`MAP_COLLECT`, `ITEM_GET`, `MIX_SUCCESS`, `MIX_FAILED`, `EVENT_END`, `OTHER_TALK_BEFORE/AFTER_IS`, `OTHER_PLAYER_PUSH_BUTTON`, `GIMMICK_ACCESS_BEFORE/AFTER`, `ENEMY_KILL_IS`, `RECLAIM_TARGET_CLEAR`, `RECLAIM_FACILITY_BUILT`, `SUBJECT_CLEAR`) | the first operand equals the fired value |
| `MIX_HP_EMPTY`, `MIX_ACCESS`, `BATTLE_ANNIHILATION(_ATELIER)`, `RECLAIM_KINGDOM_LV_UP`, `ATELIER_RETURN`, `ATELIER_SLEEP_MIX`, `MAP_WORLDMAP_ROUTE_MOVE`, `MEMORIES_LV_UP` | always, once their trigger group fired |
| 41 codes on the switch's default (`CHIMU_*`, `MAP_TRAVERSAL_RITU_*`, `LICENSE_RANK_*`, `QUEST_ACCEPT_NUM_*`, the `RECLAIM_*` conditions, the `DLC_*` ones but `DLC`, `CHARA_WMPOINTER_BE_WITH_PLAYER`, `QUEST_(NOT_)OCCURRENCE`, `TRIGGER_ATELIER_ITEM_RETURN`) | never; the check data uses none of them |

Not read yet, by uses in the data: `CONDITION_MAP_FIELDMAP_BE` (176), `CHARA_FRIEND_POINT_MT` (36),
`ITEM_QUALITY_HAVE_MT` (33), `MAP_FIELDAREA_BE` (27), the date triggers and `DATE_AFTER` (20
each), `TRIGGER_PROGRAM_EXEC` (16), `EVENTFLAG_LT`'s siblings are done, `PLAYER_PARAM_MT` and
`CHARA_ADVENTURE_LV_MT` (10), `TRIGGER_MAP_FIELDMAP_RECT` and `TRIGGER_MAP_FIELDAREA_BE` (6),
`CHARA_PARTY_IN` (6), and a few single uses.

The trigger compare is `0x140213540 (ctx, cmd, count, mode)` against the fired values at
`ctx+8`:

- `0x60000002` matches anything.
- `0x60000001` matches a map for which `0x140236070` is false. That function tests the first
  word of the map's runtime record, `[0x140df9a08]+0x10e8+0x40×map`, which is the region for a
  field area, for 0.
- In the from-to triggers, `0x60000001` counts only at operands 1 and 3 after a kind of 0.

