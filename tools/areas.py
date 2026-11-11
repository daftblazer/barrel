#!/usr/bin/env python3
"""Dump Atelier Escha & Logy DX's field area records from its executable.

    areas.py EXE [TABLE_VA]

TABLE_VA defaults to 0x140d67fb0 (English Escha executable): 85 records of 0x58 bytes, one
per FIELDMAP_AREA_* number (docs/tables.md, "Field areas"). Prints the index, the short id,
the i32 fields, the float at +0x24 and the developers' name.
"""
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE

SIZE = 0x58
COUNT = 85


def main():
    pe = PE(sys.argv[1])
    va = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0x140d67fb0

    def text(ptr):
        if not pe.base <= ptr < pe.base + 0x4000000:
            return ''
        o = pe.off(ptr - pe.base)
        if o is None:
            return ''
        end = pe.data.find(b'\0', o, o + 200)
        return pe.data[o:end].decode('utf-8', 'replace')

    o = pe.off(va - pe.base)
    for i in range(COUNT):
        r = pe.data[o + SIZE * i:o + SIZE * (i + 1)]
        w = struct.unpack_from('<18i', r, 16)
        days = struct.unpack_from('<f', r, 0x24)[0]
        print(i, text(struct.unpack_from('<Q', r, 8)[0]), w[:4], w[4], days, w[6:12],
              text(struct.unpack_from('<Q', r, 0)[0]))


if __name__ == '__main__':
    main()
