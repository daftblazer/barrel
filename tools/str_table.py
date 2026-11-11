#!/usr/bin/env python3
"""Dump an array of string pointers from an x64 executable, given one string in it.

    str_table.py EXE STRING [OUT.txt]

Finds the pointer to STRING in the data sections, walks back and forward while the
neighbouring 8-byte values also point at printable strings, and prints `index name` for the
whole array. The executable keeps its enum names this way (item tags, item categories,
...), so an entry's index is the number the game uses for it. Several enums can sit back to
back in one array; split them by prefix.
"""
import re
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE


def main():
    exe, name = sys.argv[1:3]
    pe = PE(exe)
    d = pe.data

    def s_at(va):
        o = pe.off(va - pe.base) if va >= pe.base else None
        if o is None:
            return None
        e = d.find(b'\0', o, o + 96)
        s = d[o:e]
        return s.decode() if len(s) >= 2 and all(32 <= c < 127 for c in s) else None

    def q(va):
        o = pe.off(va - pe.base)
        return struct.unpack_from('<Q', d, o)[0] if o is not None else 0

    m = re.search(b'\0' + re.escape(name.encode()) + b'\0', d)
    if not m:
        sys.exit('string not found')
    target = pe.base + pe.rva_of_off(m.start() + 1)
    p = d.find(struct.pack('<Q', target))
    if p < 0:
        sys.exit('no pointer to the string')
    va = pe.base + pe.rva_of_off(p)
    start, end = va, va
    while s_at(q(start - 8)):
        start -= 8
    while s_at(q(end)):
        end += 8
    names = [s_at(q(start + 8 * i)) for i in range((end - start) // 8)]
    out = open(sys.argv[3], 'w') if len(sys.argv) > 3 else sys.stdout
    print('# array at %#x, %d entries' % (start, len(names)), file=out)
    for i, n in enumerate(names):
        print(i, n, file=out)


if __name__ == '__main__':
    main()
