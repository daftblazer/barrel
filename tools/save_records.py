#!/usr/bin/env python3
"""Lists Escha & Logy DX's save record handlers from the English executable.

Each top-level save record has a small object in .data: a vtable pointer (0x140bc7xxx), a
helper, its name getter, its write and read functions, then its name inline (NUL-terminated,
padded to 8). Usage: save_records.py Atelier_Escha_and_Logy_EN.exe
"""
import struct
import sys

d = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', d, 0x3c)[0]
n = struct.unpack_from('<H', d, pe + 6)[0]
opt = struct.unpack_from('<H', d, pe + 20)[0]
base = struct.unpack_from('<Q', d, pe + 24 + 24)[0]
secs = []
for i in range(n):
    o = pe + 24 + opt + 40 * i
    name = d[o:o + 8].rstrip(b'\0').decode()
    vsize, va, rsize, raw = struct.unpack_from('<IIII', d, o + 8)
    secs.append((name, va, vsize, raw, rsize))
text = next(s for s in secs if s[0] == '.text')


def is_code(q):
    return base + text[1] <= q < base + text[1] + text[2]


found = []
for name, va, vsize, raw, rsize in secs:
    if name not in ('.data', '.rdata'):
        continue
    for o in range(raw, raw + rsize - 48, 8):
        q = struct.unpack_from('<6Q', d, o)
        if not (0x140b00000 <= q[0] < 0x140d00000 and all(is_code(x) for x in q[1:5])):
            continue
        s = d[o + 40:o + 40 + 32].split(b'\0')[0]
        # Record names are lower case with underscores (other strings can follow such objects).
        if not s or not all(c == 0x5f or 0x61 <= c <= 0x7a for c in s) or s == b'name':
            continue
        found.append((s.decode(), q[2], q[3], q[4], base + va + o - raw))
for name, getter, write, read, at in found:
    print(f'{name:<20} write {write:#x}  read {read:#x}  (object {at:#x})')
