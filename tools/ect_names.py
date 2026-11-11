#!/usr/bin/env python3
"""List the event command names an Atelier executable registers with Squirrel.

    ect_names.py EXE ASM

ASM is `objdump -d -M intel --no-show-raw-insn EXE`. The game registers each event command
word as a Squirrel constant named ECT_*: `mov r8d, WORD; lea rdx, [name]; call add_const`.
This finds the add-constant function from the ECT_END registration (word 0x7f000000) and
prints every `WORD NAME` pair passed to it, one per line.
"""
import re
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE


def main():
    exe, asm = sys.argv[1:3]
    pe = PE(exe)

    def cstr(va):
        o = pe.off(va - pe.base)
        if o is None:
            return None
        e = pe.data.find(b'\0', o, o + 256)
        s = pe.data[o:e]
        return s.decode() if s and all(32 <= c < 127 for c in s) else None

    lines = []
    for line in open(asm):
        m = re.match(r'\s+([0-9a-f]+):\t(\S+)\s*(.*)', line)
        if m:
            lines.append(m.groups())
    # Each registration: mov r8d,imm ; lea rdx,[rip+..] # str ; lea rcx,... ; call F
    calls = {}
    pairs = []
    for i in range(len(lines) - 3):
        a, op, args = lines[i]
        if op != 'mov' or not args.startswith('r8d,0x'):
            continue
        word = int(args[6:], 16)
        if not 0x70 <= word >> 24 <= 0x7f:
            continue
        for j in range(i + 1, min(i + 5, len(lines))):
            _, op2, args2 = lines[j]
            m = re.match(r'rdx,\[rip\+0x[0-9a-f]+\]\s+# 0x([0-9a-f]+)', args2)
            if op2 == 'lea' and m:
                name = cstr(int(m.group(1), 16))
                for k in range(j + 1, min(j + 4, len(lines))):
                    if lines[k][1] == 'call':
                        if name and name.startswith('ECT_'):
                            pairs.append((word, name, lines[k][2]))
                        break
                break
    targets = {t for w, n, t in pairs if n == 'ECT_END'}
    seen = set()
    for word, name, target in pairs:
        if target in targets and (word, name) not in seen:
            seen.add((word, name))
            print('%08x %s' % (word, name))


if __name__ == '__main__':
    main()
