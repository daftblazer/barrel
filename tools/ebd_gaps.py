#!/usr/bin/env python3
"""Tabulate event commands: name (from ect_names.py output), uses, and how many words
follow each command before the next command word, across every .ebd under a directory.

    ebd_gaps.py EVENT_DIR NAMES.txt [SITES.txt]  > table.md

SITES is cmd_sites.py output; it adds the classes whose methods test each word.
"""
import collections
import os
import struct
import sys


def bodies(root):
    for dp, _, fs in os.walk(root):
        for f in sorted(fs):
            if not f.endswith('.ebd'):
                continue
            d = open(os.path.join(dp, f), 'rb').read()
            o = 4
            for _ in range(struct.unpack_from('<I', d, 0)[0]):
                o += 4
                for _ in range(2):
                    o += 4 + struct.unpack_from('<I', d, o)[0]
                nw = struct.unpack_from('<I', d, o)[0]
                o += 4
                yield struct.unpack_from('<%dI' % nw, d, o)
                o += nw * 4


def main():
    root, names_file = sys.argv[1:3]
    names = {}
    for line in open(names_file):
        w, n = line.split()
        names[int(w, 16)] = n
    owners = collections.defaultdict(set)
    if len(sys.argv) > 3:
        for line in open(sys.argv[3]):
            p = line.split()
            if len(p) > 2:
                owners[int(p[0], 16)].add(p[2].split('::')[0])
    gaps = collections.defaultdict(collections.Counter)
    for ws in bodies(root):
        idx = [k for k, w in enumerate(ws) if 0x70 <= w >> 24 <= 0x7f]
        for a, b in zip(idx, idx[1:]):
            gaps[ws[a]][b - a - 1] += 1
    print('| Word | Short | Name | Handled by | Uses | Words after it (count) |')
    print('|---|---|---|---|---|---|')
    for w in sorted(gaps):
        c = gaps[w]
        short = '%s%d' % (chr(w >> 24), w & 0xffff) + ('/%#x' % ((w >> 16) & 0xff) if (w >> 16) & 0xff else '')
        dist = ', '.join('%d (%d)' % kv for kv in sorted(c.items(), key=lambda kv: -kv[1])[:4])
        print('| `%08x` | `%s` | %s | %s | %d | %s |' % (
            w, short, names.get(w, ''), ', '.join(sorted(owners[w])), sum(c.values()), dist))


if __name__ == '__main__':
    main()
