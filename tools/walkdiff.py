#!/usr/bin/env python3
"""Diff a headless walk (`barrel-tool walk`) against the game's per-frame state.

usage: walkdiff.py OURS KEY [PAD]

OURS and KEY have a line a frame, `frame x y z yaw | ex ey ez camyaw pitch` (feet and facing
in degrees, the camera's eye, its yaw and pitch). PAD (default: KEY with .pad for .state) is the
pad script; the frames are split into segments where a press starts or ends, and each segment
prints the largest error of: her position over the ground (xz) and in height (y), her facing,
the eye, the camera's yaw and pitch (degrees). The last line gives the first frame each passes
1 unit (positions, eye) or 0.1 degree.
"""
import math
import sys


def load(path):
    rows = {}
    for line in open(path):
        v = line.replace("|", " ").split()
        if len(v) >= 10:
            rows[int(v[0])] = [float(x) for x in v[1:10]]
    return rows


def angle(a, b):
    return abs((a - b + 180.0) % 360.0 - 180.0)


def errors(o, k):
    return [
        math.hypot(o[0] - k[0], o[2] - k[2]),
        abs(o[1] - k[1]),
        angle(o[3], k[3]),
        math.dist(o[4:7], k[4:7]),
        angle(o[7], k[7]),
        abs(o[8] - k[8]),
    ]


NAMES = ["xz", "y", "yaw", "eye", "camyaw", "pitch"]
LIMITS = [1.0, 1.0, 0.1, 1.0, 0.1, 0.1]


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    ours, key = load(sys.argv[1]), load(sys.argv[2])
    pad = sys.argv[3] if len(sys.argv) > 3 else sys.argv[2].rsplit(".", 1)[0] + ".pad"
    cuts = set()
    try:
        for line in open(pad):
            w = line.split()
            if len(w) >= 2 and w[0].isdigit() and int(w[0]) >= 1879:
                f = int(w[0])
                n = int(w[2]) if len(w) > 2 and w[2].isdigit() else 1
                cuts.update([f, f + n])
    except OSError:
        pass
    frames = sorted(f for f in key if f in ours)
    if not frames:
        sys.exit("no frames in common")
    bounds = sorted(c for c in cuts if frames[0] < c <= frames[-1])
    starts = [frames[0]] + bounds
    first = [None] * 6
    print("frames        " + " ".join(f"{n:>7}" for n in NAMES))
    for i, s in enumerate(starts):
        e = (starts[i + 1] if i + 1 < len(starts) else frames[-1] + 1)
        worst = [0.0] * 6
        for f in frames:
            if s <= f < e:
                for j, v in enumerate(errors(ours[f], key[f])):
                    worst[j] = max(worst[j], v)
        for f in frames:
            for j, v in enumerate(errors(ours[f], key[f])):
                if first[j] is None and v > LIMITS[j]:
                    first[j] = f
        print(f"{s:>5}-{e - 1:<5}   " + " ".join(f"{v:7.2f}" for v in worst))
    total = [max(errors(ours[f], key[f])[j] for f in frames) for j in range(6)]
    print("max           " + " ".join(f"{v:7.2f}" for v in total))
    print("first over    " + " ".join(f"{(str(f) if f else '-'):>7}" for f in first) + "   (1 unit, 0.1 degree)")


main()
