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
- `xkey.c`: key taps through XTest (`cc -O2 -o work/xkey xkey.c -lX11 -lXtst`), to the run's
  X display (`DISPLAY=:2`).
- `peek.sh ADDR LEN OUT`, `scan.py VALUE [MAX_MB]`: read memory, find a pointer.

Keys: any key at the title; `k` confirms, the arrow keys move. New Game, `k` on the
character card, Left (Yes), `k`; the opening movie shows Proton's colour bars and `k` skips
it, and `EVENT_MM01_OP1_111` begins.
