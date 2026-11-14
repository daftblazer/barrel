# Oracle runs

Tools for running the original Escha & Logy DX (English) under Proton as a reference: what a
scene really looks like, and values the game only computes while running.

- `host.py INSTALL PROTON` (in the background): starts a run as its own descendant and serves
  reads of the game's memory through `work/oracle/cmd` (`read ADDR LEN OUT`, `pid`, `quit`).
  Memory reads need the reader to be the game's ancestor (Yama `ptrace_scope` 1).
- `run.sh`: the run itself: headless gamescope (1280 × 720, 30 Hz), a Proton prefix of its own
  under `work/oracle-prefix` (never the player's), Proton's log in `work/oracle`, and a
  silent sound device (ALSA's null device through a private config). The game needs a sound
  device: without one its effect player reads a sound bank that was never made and crashes.
- `stop.sh`: ends exactly the run's session.
- `shot.sh OUT.png`: a screenshot through `gamescopectl` (absolute paths only reach it).
- `pad.py` (in the background, started before the game) and `pad.sh "tap A" "dpad LEFT" "stick L 0 -1 800" ...`:
  a virtual Xbox 360 pad through uinput (python-evdev), named "Barrel oracle pad"; the game
  takes it as its controller. It is visible system-wide while it exists.
  Start the pad in its own session (`setsid python3 tools/oracle/pad.py ... &`): a pad started
  inside a command that is later stopped dies with it. `pad.sh` waits for the pad's log of finished commands
  (`work/oracle/paddone`) and gives up on a command that gets no answer within a minute.
  Remove that log before starting the pad: the pad empties it as it starts, and a helper that
  counted the last run's lines first waits for a line count it never reaches. A pad plugged in
  after the game started works.
- `xkey.c`: key taps through XTest (`cc -O2 -o work/xkey xkey.c -lX11 -lXtst`), to the run's
  X display (`DISPLAY=:2`).
- `readlog.c`: a preload library that logs the game's file reads (path, offset, length, time)
  for files matching `READLOG_MATCH` (the voice banks by default):
  `LD_PRELOAD=$PWD/work/readlog.so READLOG_FILE=$PWD/work/oracle/reads.log tools/oracle/run.sh ...`.
- `peek.sh ADDR LEN OUT`, `scan.py VALUE [MAX_MB]`: read memory, find a pointer.

To the opening, with the pad: A at the title, then New Game (once the run's prefix holds a
save the menu starts on Load Game: D-pad up first), A on Escha's card, A, D-pad left (Yes), A; the opening movie shows Proton's colour bars and A skips it, and
`EVENT_MM01_OP1_111` begins. On the keyboard: `k` confirms, the arrow keys move.

**Record every run** (`record.sh`, started right after the game and stopped before it): runs are
costly and a recording answers later questions without another. Start the game with
`ORACLE_HZ=60 ORACLE_AUDIO=1` for 60 frames a second and its sound.

**With a copied save** (the player's own progress point, in this prefix only): stay where the
player has already been. On the save used so far (Southern Ruins, Sector One), the area map's
Sector Two starts a story scene on entry: do not enter it; Sector One and the Ruins Entrance are
fine. Going back to Colseit (world map, 4 days to 8/24) starts a scene in the Atelier on arrival:
do not go back to town either (so no shop visits on this save). Stop at the first line of any scene that starts unexpectedly.

**Without Steam:** tried (2026-11-12) and the game will not run: under Proton's own Wine (its
Wine Mono for the English executable's .NET import, its DXVK copied into a prefix of its own),
with no Steam variables, the executable exits with 0 after a second, the Steam API's restart
check asking Steam to start it instead. Getting around that would mean replacing or patching
the game's `steam_api64.dll`, which this project does not do. So oracle runs go through Steam:
keep them short and stop them right after (`stop.sh`), as they count as the player's playtime.

## Atelier Ryza DX

`ORACLE_EXE=Atelier_Ryza_DX.exe ORACLE_APPID=3365030 tools/oracle/run.sh ...` (prefix
`work/oracle-prefix-3365030`). Without Steam (`ORACLE_NO_STEAM=1`) it exits within seconds, as
Escha does. It does not take the virtual pad; keyboard and mouse through `xkey` work: any key
past the title, W/A/S/D to move (`down:w` ... `up:w`), the left mouse button (`click:1`)
confirms and advances text, Space asks to skip an event (W/S pick Yes/No), F auto-plays, R
fast-forwards. A new game starts on the opening monologue, then the attic event, then free
play in the Rasen District.

## Recording

`ORACLE_HZ=60 tools/oracle/run.sh ...` runs the game at 60 Hz (30 by default). With
`ORACLE_AUDIO=1` its sound goes to a PipeWire null sink of its own (`barrel_oracle`, connected
to no device, so nothing is heard; `stop.sh` removes it) and `record.sh` adds the sink's monitor
to the recording as FLAC (48 kHz stereo). Check with `pactl list sink-inputs` that the game's
stream is on that sink before anything plays; without the option Wine's PulseAudio driver is
off and the sound goes to ALSA's null device.
`tools/oracle/record.sh OUT.mkv SECONDS` records gamescope's PipeWire stream as it is shown:
H.264 at quantizer 0 in 4:4:4 (lossless), each frame stamped as it arrives (a 5-second test at
the title: 298 frames, none more than 25 ms apart). `tools/oracle/frames.sh VIDEO FROM TO DIR`
writes every frame in a time range as PNG with the timestamps in `DIR/times.txt`, so a frame
shown twice or skipped can be told from the game's own animation. Recordings are references
kept outside the repository (they are the game's pictures), on a large disk.

`tools/oracle/poke.sh ADDR HEX` writes bytes to the run's memory through the host's `write`
command (only ever the oracle prefix's copy of a save; e.g. a party fighter's current HP, stat
1 at +0x8c, set to 1 to see a lost battle).
