#!/usr/bin/env python3
"""Compares two cue logs (`<frame> <kind> <hash>`: the game's, from tools/ryza_cues.py, and Barrel's
BARREL_TRACE_CUES) within a frame.
  tools/cuediff.py KEY BARREL [--from FRAME] [--to FRAME] [--skip KIND,...]
Prints how many of the key's cues Barrel has (same kind and hash, at most one frame apart), then
the key's cues it lacks and the cues it has that the key does not."""
import sys


def load(path, lo, hi, skip):
    out = []
    for line in open(path):
        f = line.split()
        if len(f) == 3 and lo <= int(f[0]) <= hi and f[1] not in skip:
            out.append((int(f[0]), f[1], f[2]))
    return out


def main(argv):
    lo, hi, skip = 0, 1 << 60, set()
    args = []
    it = iter(argv)
    for a in it:
        if a == "--from":
            lo = int(next(it))
        elif a == "--to":
            hi = int(next(it))
        elif a == "--skip":
            skip = set(next(it).split(","))
        else:
            args.append(a)
    key, mine = load(args[0], lo, hi, skip), load(args[1], lo, hi, skip)
    used = set()
    missing = []
    for k in key:
        hit = next((i for i, m in enumerate(mine) if i not in used and m[1:] == k[1:] and abs(m[0] - k[0]) <= 1), None)
        if hit is None:
            missing.append(k)
        else:
            used.add(hit)
    extra = [m for i, m in enumerate(mine) if i not in used]
    print(f"cues: {len(key) - len(missing)} of {len(key)} within a frame, {len(extra)} extra")
    for k in missing:
        print("  missing", *k)
    for m in extra:
        print("  extra  ", *m)


main(sys.argv[1:])
