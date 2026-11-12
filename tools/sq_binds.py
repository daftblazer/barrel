#!/usr/bin/env python3
"""List the native functions bound to Squirrel names in Escha & Logy DX (English executable).

    sq_binds.py EXE ASM     → one line per binding: name, native function, argument stub

A binding loads the member-function pointer (`lea rax, [rip+fn]`), then the name
(`lea rdx, [rip+name]`, `call 0x14051f050`), then the stub that converts the arguments
(`lea rdx, [rip+stub]`). ASM is objdump's disassembly of EXE.
"""
import re
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE


def main():
    exe, asm = sys.argv[1:3]
    pe = PE(exe)
    lines = open(asm).read().split('\n')
    for i, l in enumerate(lines):
        m = re.match(r'\s+(1401[12][0-9a-f]{4}):\s+lea\s+rdx,\[rip\+0x[0-9a-f]+\]\s+# (0x1407c[0-9a-f]{4})$', l)
        if not m or not any('call   0x14051f050' in lines[i + k] for k in (1, 2)):
            continue
        va = int(m.group(2), 16)
        o = pe.off(va - 0x140000000)
        name = pe.data[o:o + 40].split(b'\0')[0].decode('latin1')
        fn = next((mm.group(1) for k in range(i - 1, i - 18, -1)
                   if (mm := re.search(r'lea\s+rax,\[rip\+0x[0-9a-f]+\]\s+# (0x14[0-9a-f]+)$', lines[k]))), None)
        stub = next((mm.group(1) for k in range(i + 1, i + 14)
                     if (mm := re.search(r'lea\s+rdx,\[rip\+0x[0-9a-f]+\]\s+# (0x14012[0-9a-f]+)$', lines[k]))), None)
        print(name, fn, stub)


if __name__ == '__main__':
    main()
