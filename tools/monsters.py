#!/usr/bin/env python3
"""Dump Atelier Escha & Logy DX's monsters from its executable.

    monsters.py EXE TABLE_VA ITEM_TAGS.txt

TABLE_VA is the monster table (one 48-byte record per monster number, 153 of them);
ITEM_TAGS.txt is a str_table.py dump of the item enum. docs/tables.md has the layout.
"""
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE


def main():
    exe, va, item_file = sys.argv[1:4]
    pe = PE(exe)
    d = pe.data
    items = [l.split()[1] for l in open(item_file) if not l.startswith('#')]

    def at(p):
        o = pe.off(p - pe.base) if p >= pe.base else None
        return o

    def text(p):
        o = at(p)
        if o is None:
            return '?'
        e = d.find(b'\0', o, o + 128)
        return d[o:e].decode('utf-8', 'replace')

    base = at(int(va, 16))
    for k in range(153):
        name_p, family, boss, exp, money, _, _, drops_p, stats_p = struct.unpack_from('<QIIiiiiQQ', d, base + 48 * k)
        o = at(stats_p)
        blocks = []
        while o is not None and len(blocks) < 16:
            b = struct.unpack_from('<20i', d, o)
            if b[0] <= 0:
                break
            blocks.append('Lv%d HP%d ATK%d DEF%d SPD%d [%d] res %s' % (b[0], b[1], b[2], b[3], b[4], b[5], '/'.join(map(str, b[6:10]))))
            o += 80
        drops = []
        o = at(drops_p)
        if o is not None:
            for i in range(4):
                item, chance, count = struct.unpack_from('<3i', d, o + 12 * i)
                if 0 < chance <= 100 and 0 <= item < len(items):
                    drops.append('%s %d%% x%d' % (items[item], chance, count))
        print('%3d %-24s family %d boss %d exp %d money %d | %s | drops: %s' % (
            k, text(name_p), family, boss, exp, money, '; '.join(blocks) or '-', ', '.join(drops) or '-'))


if __name__ == '__main__':
    main()
