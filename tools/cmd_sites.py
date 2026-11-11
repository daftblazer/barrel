#!/usr/bin/env python3
"""Find where an executable tests event command words, and which class method does it.

    cmd_sites.py EXE ASM CLASSES.txt > sites.txt

Follows the compiler's switch idioms over 32-bit words (cmp/je chains, sub/add chains,
lea reg,[r-K] range checks with bit tests or jump tables, as far as they can be followed
without data flow) and prints `WORD FUNCTION CLASS::slot` for each word found. CLASSES is a
list of RTTI class names whose vtables give the function owners.
"""
import collections
import re
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE


def is_cmd(w):
    return 0x70 <= w >> 24 <= 0x7f


def main():
    exe, asm, classes = sys.argv[1:4]
    pe = PE(exe)
    owner = {}
    for cls in open(classes).read().split():
        for off, vt, funcs in pe.vtables(cls):
            for i, f in enumerate(funcs):
                owner.setdefault(f, '%s::%d' % (cls, i))
    func = None
    prev_int3 = True
    acc = {}       # register -> amount subtracted from the loaded word
    rng = {}       # register -> base of a lea range check
    sites = collections.defaultdict(set)
    regs32 = r'(e[a-z]{2}|r\d+d)'
    for line in open(asm):
        m = re.match(r'\s+([0-9a-f]+):\t(\S+)\s*(.*)', line)
        if not m:
            continue
        addr, op, args = m.groups()
        if op == 'int3':
            prev_int3 = True
            continue
        if prev_int3:
            func = int(addr, 16)
            acc.clear()
            rng.clear()
            prev_int3 = False
        mm = re.match(regs32 + r',0x([0-9a-f]+)$', args)
        if op in ('sub', 'add') and mm:
            r, k = mm.group(1), int(mm.group(2), 16)
            if op == 'add':
                k = (-k) & 0xffffffff
            acc[r] = (acc.get(r, 0) + k) & 0xffffffff
            if is_cmd(acc[r]):
                sites[acc[r]].add(func)
            continue
        if op == 'cmp' and mm:
            r, k = mm.group(1), int(mm.group(2), 16)
            if r in rng and k <= 0x400:
                base = rng[r]
                for j in range(k + 1):
                    if is_cmd(base + j):
                        sites[base + j].add(func)
            elif r not in rng:
                w = (k + acc.get(r, 0)) & 0xffffffff
                if is_cmd(w):
                    sites[w].add(func)
            continue
        ml = re.match(regs32 + r',\[r[a-z0-9]+([+-])0x([0-9a-f]+)\]$', args)
        if op == 'lea' and ml:
            r = ml.group(1)
            k = int(ml.group(3), 16)
            base = k if ml.group(2) == '-' else (-k) & 0xffffffff
            if is_cmd(base):
                rng[r] = base
            continue
        mr = re.match(regs32 + r',', args)
        if mr and op in ('mov', 'movzx', 'movsxd', 'xor'):
            acc.pop(mr.group(1), None)
            rng.pop(mr.group(1), None)
    for w in sorted(sites):
        for f in sorted(sites[w]):
            print('%08x %x %s' % (w, f, owner.get(f, '')))


if __name__ == '__main__':
    main()
