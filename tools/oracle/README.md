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
- `xkey.c`: key taps through XTest (`cc -O2 -o work/xkey xkey.c -lX11 -lXtst`), to the run's
  X display (`DISPLAY=:2`).
- `peek.sh ADDR LEN OUT`, `scan.py VALUE [MAX_MB]`: read memory, find a pointer.

To the opening, with the pad: A at the title, A (New Game), A on Escha's card, A, D-pad left
(Yes), A; the opening movie shows Proton's colour bars and A skips it, and
`EVENT_MM01_OP1_111` begins. On the keyboard: `k` confirms, the arrow keys move.
