#!/usr/bin/env python3
"""Finds two floats a given distance apart (in floats) in the game's writable memory.
    tools/oracle/scan_pair.py A B GAP [MAX_MB]   → address of A, then the 8 floats before A"""
import os, subprocess, sys, array
here = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
work = os.path.join(here, "work", "oracle")
pid = open(os.path.join(work, "pid")).read().strip()
A, B, gap = float(sys.argv[1]), float(sys.argv[2]), int(sys.argv[3])
limit = int(sys.argv[4]) if len(sys.argv) > 4 else 4096
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
            for i in range(8, len(f) - gap):
                if f[i] == A and f[i + gap] == B:
                    print(f"{a + 4 * i:x}", " ".join(f"{v:.6g}" for v in f[i - 7:i + gap + 1]), flush=True)
        a += n
print(f"scanned {total >> 20} MB", file=sys.stderr)
