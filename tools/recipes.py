#!/usr/bin/env python3
"""Dump Atelier Escha & Logy DX's synthesis recipes from its executable.

    recipes.py EXE TABLE_VA ITEM_TAGS.txt CATEGORY_TAGS.txt

TABLE_VA is the first record (Craft's); the tag files are str_table.py dumps of the item
and category enums. Each record is 72 little-endian i32 (288 bytes); docs/tables.md has the
layout. Prints one line per recipe: the item, its header fields and its ingredients, where
(Category) is any item of that category.
"""
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE


def tags(path, prefix=None):
    out = []
    for line in open(path):
        if line.startswith('#'):
            continue
        name = line.split()[1]
        if prefix is None or name.startswith(prefix):
            out.append(name)
    return out


def main():
    exe, va, item_file, cat_file = sys.argv[1:5]
    pe = PE(exe)
    items = tags(item_file)
    cats = tags(cat_file, 'ITEM_CATEGORY_')
    base = pe.off(int(va, 16) - pe.base)
    n = 0
    while True:
        r = struct.unpack_from('<72i', pe.data, base + 288 * n)
        if not (0 <= r[0] < len(items)) or not items[r[0]].startswith('ITEM_'):
            break
        if r[1] not in range(0, 16):
            break
        slots = []
        for s in range(4):
            kind, ident, count, index = r[24 + 12 * s:28 + 12 * s]
            if ident < 0:
                continue
            if kind == 1:
                name = '(%s)' % cats[ident][len('ITEM_CATEGORY_'):]
            elif kind == 0:
                name = items[ident]
            else:
                name = '?%d:%d' % (kind, ident)
            slots.append('%s x%d' % (name, count))
        days = struct.unpack('<f', struct.pack('<i', r[6]))[0]
        print('%3d %-34s kind %d level? %d %d %d cost %d time %g | %s' % (
            r[0], items[r[0]], r[1], r[2], r[3], r[4], r[5], days, ', '.join(slots)))
        n += 1
    print('# %d recipes' % n)


if __name__ == '__main__':
    main()
