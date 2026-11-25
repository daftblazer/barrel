#!/usr/bin/env python3
"""The script `barrel-tool ryza-lockstep` runs for ryza-recomp's `synth-atelier` replay.

usage: synth_script.py SCENARIO_DIR [FROM TO]

Writes to stdout one `FRAME draw CALL ARGS` line for each draw of the game's hook log in FROM..TO (default 2250..4900): `irand A B`,
`frand A B` (equal bounds draw nothing and are left out), `r640 N` and `r680 N` (N 0 draws nothing), in the hook log's own spelling
(hex integers), and `rand15` for a draw of `around` or `chance` (the hooks do not give their arguments), then `index HEX` lines with the
generator's index at every 100th frame the capture dumped (`dumps/F.1.bin`: the state before that frame's draws).

The draws are the synthesis's world around it, not its own rules: a voice pick per screen (`irand 2k 2k+1`, `0x1402de19b`), the effect's
48 `frand(0.05, 0.2)` (`0x1401e2fad`) and, when the synthesis's hours go by (3536), a pass over the world's stock (`0x14059f9b0` to
`0x14059fdaf`) that draws `r640`, `r680` and `irand(2, 4)` for each random potential (`0x14059f210`). docs/ryza/open.md, "Synthesis (P6)".
"""
import struct
import sys


# The callers of a bare draw: `around` (0x1406ae2e0) and `chance` (0x1406ae500); their arguments are not hooked.
BARE = ("1406ae2fd", "1406ae532")


def main(scenario, lo, hi):
    out = []
    for line in open(f"{scenario}/hooks.log"):
        w = line.split()
        if len(w) < 4 or w[0] != "[hook]" or not w[1].isnumeric():
            continue
        frame, call = int(w[1]), w[2]
        if not lo <= frame <= hi:
            continue
        if call in ("irand", "frand") and w[3] != w[4]:
            out.append(f"{frame} draw {call} {w[3]} {w[4]}")
        elif call in ("r640", "r680") and w[3] != "0":
            out.append(f"{frame} draw {call} {w[3]}")
        elif call == "rand15" and w[4] in BARE:
            out.append(f"{frame} draw rand15")
    for frame in range(lo - lo % 100 + 100, hi + 1, 100):
        try:
            with open(f"{scenario}/dumps/{frame}.1.bin", "rb") as f:
                out.append(f"{frame} index {struct.unpack('<I', f.read(4))[0]:x}")
        except OSError:
            pass
    # The dump of a frame is the generator before that frame's draws.
    out.sort(key=lambda l: (int(l.split()[0]), l.split()[1] != "index"))
    print("\n".join(out))


if __name__ == "__main__":
    a = sys.argv[1:]
    main(a[0], int(a[1]) if len(a) > 1 else 2250, int(a[2]) if len(a) > 2 else 4900)
