#!/usr/bin/env python3
"""The field's free-play draws from a hook log: a schedule for `barrel-tool ryza-lockstep`.

usage: free_play.py HOOKS.LOG [FROM [TO]]

Prints one `FRAME TIMERS SPAWN` line for each frame (FROM to TO) on which the ambient particles' emitters drew from the
generator (`barrel_ryza::free_play`): TIMERS the `frand 0 15` calls of `0x14008e290`, SPAWN 1 if the spread's `frand -250 250`
(`0x14008e3cd`) followed. The hook log writes `[hook] FRAME frand A B CALLER` (numbers in hex, floats in decimal).
"""
import sys


def main(path, first=0, last=10**9):
    frames = {}
    for line in open(path):
        p = line.split()
        if len(p) < 6 or p[0] != "[hook]" or p[2] != "frand" or not p[1].isdigit():
            continue
        frame = int(p[1])
        if not first <= frame <= last:
            continue
        timers, spawn = frames.get(frame, (0, 0))
        if p[5].endswith("14008e290") and p[3:5] == ["0", "15"]:
            timers += 1
        elif p[5].endswith("14008e3cd"):
            spawn = 1
        else:
            continue
        frames[frame] = (timers, spawn)
    for frame in sorted(frames):
        print(frame, *frames[frame])


if __name__ == "__main__":
    a = sys.argv[1:]
    main(a[0], *(int(v) for v in a[1:3]))
