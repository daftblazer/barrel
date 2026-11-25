#!/usr/bin/env python3
"""The script `barrel-tool ryza-lockstep` runs for ryza-recomp's `gather-rasen2` replay, from one dump of the generator.

usage: lockstep_script.py SCENARIO_DIR

Writes the run to stdout: the hand gathering of the first comb (2041), the voice chance after it (2042), the second comb (2501) and
its chance (2502), the staff's two swings (3166, 3266) and its gathering (3184), the field's free play between them from the hook
log (`tools/free_play.py`), and the generator's index at the frames the capture dumped it (`dumps/F.1.bin`'s first word).
"""
import os
import struct
import subprocess
import sys

COMB = "S06_T_RASEN_COMB_HAND"
BOX = "S06_T_RASEN_BOX_ROD"


def index_at(scenario, frame):
    with open(f"{scenario}/dumps/{frame}.1.bin", "rb") as f:
        return struct.unpack("<I", f.read(4))[0]


def main(scenario):
    here = os.path.dirname(os.path.abspath(__file__))
    lines = [(2041, f"gather {COMB}"), (2042, "clip"), (2501, f"gather {COMB}"), (2502, "clip"), (3166, "swing"), (3184, f"gather {BOX}"), (3266, "swing")]
    for frame in (2380, 2500, 3165, 3400):
        lines.append((frame, f"index {index_at(scenario, frame):x}"))
    free = subprocess.run([sys.executable, "-I", f"{here}/free_play.py", f"{scenario}/hooks.log", "2043", "3500"], check=True, capture_output=True, text=True).stdout
    for line in free.splitlines():
        frame, rest = line.split(" ", 1)
        lines.append((int(frame), f"free {rest}"))
    # An index line goes before the frame's other draws (the dump is the state before them).
    lines.sort(key=lambda l: (l[0], not l[1].startswith("index")))
    for frame, what in lines:
        print(frame, what)


if __name__ == "__main__":
    main(sys.argv[1])
