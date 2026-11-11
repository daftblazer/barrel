#!/usr/bin/env python3
"""Dump Atelier Escha & Logy DX's field area records from its executable.

    areas.py EXE [TABLE_VA]
    areas.py --places EXE

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


PLACES = 0x140d65bb0
PLACE_SIZE = 0x60
PLACE_COUNT = 96


def main():
    places = sys.argv[1] == '--places'
    args = sys.argv[2:] if places else sys.argv[1:]
    pe = PE(args[0])
    va = int(args[1], 16) if len(args) > 1 else 0x140d67fb0

    def text(ptr):
        if not pe.base <= ptr < pe.base + 0x4000000:
            return ''
        o = pe.off(ptr - pe.base)
        if o is None:
            return ''
        end = pe.data.find(b'\0', o, o + 200)
        return pe.data[o:end].decode('utf-8', 'replace')

    if places:
        o = pe.off(PLACES - pe.base)
        for i in range(PLACE_COUNT):
            r = pe.data[o + PLACE_SIZE * i:o + PLACE_SIZE * (i + 1)]
            name, pid, desc = (text(struct.unpack_from('<Q', r, k)[0]) for k in (0, 0x18, 0x38))
            kind = struct.unpack_from('<i', r, 0x10)[0]
            links = [a for a in struct.unpack_from('<6i', r, 0x20) if a >= 0]
            print(i, pid, kind, name, 'areas', links, '|', desc)
        return
    o = pe.off(va - pe.base)
    for i in range(COUNT):
        r = pe.data[o + SIZE * i:o + SIZE * (i + 1)]
        w = struct.unpack_from('<18i', r, 16)
        days = struct.unpack_from('<f', r, 0x24)[0]
        print(i, text(struct.unpack_from('<Q', r, 8)[0]), w[:4], w[4], days, w[6:12],
              text(struct.unpack_from('<Q', r, 0)[0]))


if __name__ == '__main__':
    main()
