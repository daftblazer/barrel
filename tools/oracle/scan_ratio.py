#!/usr/bin/env python3
"""Finds float triples in the game's writable memory with given ratios (y/x, z/x), through the
oracle host: e.g. a fog colour derived from the field's rayleighRGB.
    tools/oracle/scan_ratio.py RY RZ [MAX_MB]"""
import os, struct, subprocess, sys
import array
here = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
work = os.path.join(here, "work", "oracle")
pid = open(os.path.join(work, "pid")).read().strip()
ry, rz = float(sys.argv[1]), float(sys.argv[2])
limit = int(sys.argv[3]) if len(sys.argv) > 3 else 4096
tmp = os.path.join(work, "scan.bin")
total = 0
for line in open(f"/proc/{pid}/maps"):
    rng, perms = line.split()[:2]
    if not perms.startswith("rw"):
        continue
    lo, hi = (int(x, 16) for x in rng.split("-"))
    a = lo
    while a < hi and total < (limit << 20):
        n = min(16 << 20, hi - a)
        total += n
        r = subprocess.run([os.path.join(here, "tools/oracle/peek.sh"), f"{a:x}", str(n), tmp], capture_output=True)
        if r.returncode == 0:
            f = array.array("f"); f.frombytes(open(tmp, "rb").read()[: n // 4 * 4])
            for i in range(len(f) - 3):
                x = f[i]
                if 1e-6 < abs(x) < 1e6 and abs(f[i + 1] - ry * x) < 1e-4 * abs(x) and abs(f[i + 2] - rz * x) < 1e-4 * abs(x):
                    print(f"{a + 4 * i:x} {f[i]:.6g} {f[i+1]:.6g} {f[i+2]:.6g} {f[i+3]:.6g}", flush=True)
        a += n
print(f"scanned {total >> 20} MB", file=sys.stderr)
