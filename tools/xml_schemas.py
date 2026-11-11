#!/usr/bin/env python3
"""List the XML tables' typed schemas compiled into Escha & Logy DX's executable.

    xml_schemas.py EXE ASM

ASM is an objdump -d -M intel listing of EXE. Each table's loader registers its columns with
0x1404de950 (schema, type, record offset, name) (docs/tables.md, "Column types"); this finds
those calls and prints, per loader, each column's name, type and offset.
"""
import re
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE

REGISTER = '1404de950'


def main():
    pe = PE(sys.argv[1])
    lines = open(sys.argv[2], encoding='utf-8', errors='replace').read().split('\n')

    def text(va):
        o = pe.off(va - pe.base)
        if o is None:
            return '?'
        e = pe.data.find(b'\0', o, o + 80)
        return pe.data[o:e].decode('latin1')

    loaders = {}
    func = None
    regs = {}
    for line in lines:
        if 'int3' in line:
            func = None
            regs = {}
            continue
        m = re.match(r'\s+([0-9a-f]+):\t(.*)', line)
        if not m:
            continue
        addr, ins = m.groups()
        if func is None:
            func = addr
        mm = re.search(r'lea\s+r9,\[rip\+0x[0-9a-f]+\]\s+# 0x([0-9a-f]+)', ins)
        if mm:
            regs['name'] = int(mm.group(1), 16)
        mm = re.search(r'(?:lea|mov)\s+edx,(?:\[r[a-z0-9]+\+0x([0-9a-f]+)\]|0x([0-9a-f]+))$', ins)
        if mm:
            regs['type'] = int(mm.group(1) or mm.group(2), 16)
        mm = re.search(r'(?:lea|mov)\s+r8d,(?:\[r[a-z0-9]+\+0x([0-9a-f]+)\]|0x([0-9a-f]+))$', ins)
        if mm:
            regs['off'] = int(mm.group(1) or mm.group(2), 16)
        if re.search(r'xor\s+r8d,r8d', ins):
            regs['off'] = 0
        if ins.startswith('call') and REGISTER in ins:
            loaders.setdefault(func, []).append((text(regs.get('name', 0)), regs.get('type'), regs.get('off')))
    for f, cols in loaders.items():
        print('loader', f, len(cols), 'columns')
        for name, t, off in cols:
            print('   %-28s type %s  +0x%x' % (name, t, off if off is not None else -1))


if __name__ == '__main__':
    main()
