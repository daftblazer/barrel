#!/usr/bin/env python3
"""How far a headless walk is from the game's feet at some frames.

usage: walkat.py OURS DUMPS_DIR FRAME...

OURS is a `barrel-tool walk` state file (`frame x y z yaw | ...`); DUMPS_DIR holds the capture's
per-frame player dumps (`<frame>.0.bin`: her feet at +0x10, three f32). The walk's state at frame F
is the game's dump of frame F + 1 (the pad's press at frame F acts on the next game frame: the
dump of 1901 is the first to move for a stick at 1900). Prints `FRAME ERR` in game units
(centimetres) over the ground, one line a frame, and `max ERR`.
"""
import math
import struct
import sys


def main(ours_path, dumps, frames):
    ours = {}
    for line in open(ours_path):
        v = line.replace("|", " ").split()
        if len(v) >= 5:
            ours[int(v[0])] = (float(v[1]), float(v[3]))
    worst = 0.0
    for f in frames:
        try:
            d = open(f"{dumps}/{f + 1}.0.bin", "rb").read()
        except OSError:
            print(f"{f} no dump")
            continue
        x, _, z = struct.unpack_from("<3f", d, 0x10)
        o = ours.get(f)
        if o is None:
            print(f"{f} not walked")
            continue
        err = math.hypot(x - o[0], z - o[1])
        worst = max(worst, err)
        print(f"{f} {err:.1f}")
    print(f"max {worst:.1f}")


if __name__ == "__main__":
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2], [int(a) for a in sys.argv[3:]])
