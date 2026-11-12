#!/usr/bin/env python3
"""Lists the integer constants Escha & Logy DX registers with Squirrel besides ECT_*.

    sq_consts.py EXE ASM

ASM is `objdump -d -M intel --no-show-raw-insn EXE` (the English executable). The game
registers them as `mov r8d, VALUE` (or `xor r8d, r8d`), `lea rdx, [name]`, `lea rcx, [...]`,
`call 0x14012d910`. Prints `VALUE NAME`, one per line.
"""
import re
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE

ADD_CONST = '0x14012d910'


def main():
    exe, asm = sys.argv[1:3]
    pe = PE(exe)

    def cstr(va):
        o = pe.off(va - pe.base)
        if o is None:
            return None
        e = pe.data.find(b'\0', o, o + 128)
        s = pe.data[o:e]
        return s.decode() if s and all(32 < c < 127 for c in s) else None

    value, name = None, None
    for line in open(asm, errors='replace'):
        if 'r8d' in line:
            m = re.search(r'mov\s+r8d,(0x[0-9a-f]+)$', line.strip())
            if m:
                value = int(m.group(1), 16)
            elif re.search(r'xor\s+r8d,r8d', line):
                value = 0
        m = re.search(r'lea\s+rdx,\[rip\+0x[0-9a-f]+\]\s+# (0x[0-9a-f]+)', line)
        if m:
            name = cstr(int(m.group(1), 16))
        if ('call   ' + ADD_CONST) in line:
            if value is not None and name:
                v = value - (1 << 32) if value >= 1 << 31 else value
                print(v, name)
            value, name = None, None


if __name__ == '__main__':
    main()
