#!/usr/bin/env python3
"""Finds a little-endian u64 in the game's writable memory through the oracle host.
    tools/oracle/scan.py VALUE_HEX [MAX_MB]   → prints each address holding it"""
import os, struct, subprocess, sys
here = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
work = os.path.join(here, "work", "oracle")
pid = open(os.path.join(work, "pid")).read().strip()
value = struct.pack("<Q", int(sys.argv[1], 16))
limit = int(sys.argv[2]) if len(sys.argv) > 2 else 4096
chunk = 16 << 20
total = 0
tmp = os.path.join(work, "scan.bin")
for line in open(f"/proc/{pid}/maps"):
    rng, perms = line.split()[:2]
    if not perms.startswith("rw"):
        continue
    lo, hi = (int(x, 16) for x in rng.split("-"))
    a = lo
    while a < hi and total < (limit << 20):
        n = min(chunk, hi - a)
        r = subprocess.run([os.path.join(here, "tools/oracle/peek.sh"), f"{a:x}", str(n), tmp], capture_output=True)
        total += n
        if r.returncode == 0:
            data = open(tmp, "rb").read()
            i = data.find(value)
            while i >= 0:
                if (a + i) % 8 == 0:
                    print(f"{a + i:x}", flush=True)
                i = data.find(value, i + 1)
        a += n
print(f"scanned {total >> 20} MB", file=sys.stderr)
