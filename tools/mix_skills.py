#!/usr/bin/env python3
"""Dump Atelier Escha & Logy DX's synthesis skills from its executable.

    mix_skills.py EXE [TABLE_VA]

TABLE_VA defaults to 0x1407b5f40 (English Escha executable): 68 records of 56 bytes, read
by the getter at 0x1400fd480. docs/tables.md has the layout. Prints one line per skill:
index, user (Escha/Logy), type, the two parameters, the level it is learnt at, the
Fire/Water/Wind/Earth point costs, the name and the description.
"""
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE

COUNT = 68
SIZE = 56


def main():
    exe = sys.argv[1]
    va = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0x1407b5f40
    pe = PE(exe)
    base = pe.off(va - pe.base)

    def text(ptr):
        o = pe.off(ptr - pe.base) if ptr >= pe.base else None
        if o is None:
            return '?'
        end = pe.data.find(b'\0', o, o + 400)
        return pe.data[o:end].decode('utf-8', 'replace')

    for i in range(COUNT):
        r = pe.data[base + SIZE * i:base + SIZE * (i + 1)]
        name, = struct.unpack_from('<Q', r, 0)
        kind, p1, p2, _, user, level = struct.unpack_from('<6i', r, 8)
        cost = struct.unpack_from('<4i', r, 0x20)
        desc, = struct.unpack_from('<Q', r, 0x30)
        print('%2d %-5s type %2d params %3d %3d level %2d cost F%d W%d A%d E%d  %s: %s' % (
            i, ('Escha', 'Logy')[user] if user in (0, 1) else user, kind, p1, p2, level,
            *cost, text(name), text(desc)))


if __name__ == '__main__':
    main()
