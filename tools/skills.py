#!/usr/bin/env python3
"""Dump Atelier Escha & Logy DX's battle skills from its executable.

    skills.py EXE [TABLE_VA]

TABLE_VA defaults to 0x140d5ab10 (English Escha executable): 96-byte records of a name
pointer, a category pointer, 18 i32 and a description pointer; docs/tables.md has the
layout. Prints one line per skill: index, name, category, the values, description.
"""
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE

SIZE = 96


def main():
    pe = PE(sys.argv[1])
    va = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0x140d5ab10

    def text(ptr):
        if not pe.base <= ptr < pe.base + 0x4000000:
            return None
        o = pe.off(ptr - pe.base)
        if o is None:
            return None
        end = pe.data.find(b'\0', o, o + 300)
        try:
            return pe.data[o:end].decode('utf-8')
        except UnicodeDecodeError:
            return None

    o = pe.off(va - pe.base)
    i = 0
    while True:
        r = pe.data[o + SIZE * i:o + SIZE * (i + 1)]
        name = text(struct.unpack_from('<Q', r, 0)[0])
        if name is None:
            break
        category = text(struct.unpack_from('<Q', r, 8)[0])
        values = struct.unpack_from('<18i', r, 16)
        desc = text(struct.unpack_from('<Q', r, 88)[0]) or ''
        print('%3d %-28s %-36s %s %s' % (i, name, category, values, desc.replace('<CR>', ' ')))
        i += 1


if __name__ == '__main__':
    main()
