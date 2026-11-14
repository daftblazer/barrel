#!/usr/bin/env python3
"""Names event command words through a game's word-to-name switch (Atelier Ryza DX).

    ect_switch_names.py EXE ASM WORDS_FILE

ASM is `objdump -d -M intel --no-show-raw-insn EXE`. Ryza DX does not register its event
commands as Squirrel constants one by one (`ect_names.py`); it names a word with a compiled
switch: compares that halve the range, then a subtraction and a jump table into blocks of
`mov r8d, LENGTH; lea rdx, [NAME]`. The switch is found from the `ECT_END` string's block;
each word in WORDS_FILE (hex, the first on each line; e.g. `barrel ebd-measure` output) is
walked through it by a small interpreter of the instruction forms the switch uses, and
`WORD NAME` is printed (`-` where the switch has no name for it).
"""
import re
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from pe import PE

LINE = re.compile(r'^\s*([0-9a-f]+):\s+(\S+)\s*(.*?)\s*$')
HEXNUM = re.compile(r'^(0x[0-9a-f]+|\d+)$')


def main():
    exe, asm, words_file = sys.argv[1:4]
    pe = PE(exe)

    def cstr(va):
        o = pe.off(va - pe.base)
        e = pe.data.find(b'\0', o, o + 256)
        return pe.data[o:e].decode('ascii', 'replace')

    # The instructions, by address.
    code = {}
    order = []
    for line in open(asm):
        m = LINE.match(line)
        if not m:
            continue
        a = int(m.group(1), 16)
        code[a] = (m.group(2), m.group(3))
        order.append(a)
    index = {a: i for i, a in enumerate(order)}

    # The block naming ECT_END, and from it back to the function's first instruction.
    end_va = next(pe.base + pe.rva_of_off(o) for o in [pe.data.find(b'ECT_END\0')] if o >= 0)
    use = next(a for a in order if code[a][0] == 'lea' and f'# {end_va:#x}' in code[a][1])
    i = index[use]
    while code[order[i - 1]][0] not in ('ret', 'int3'):
        i -= 1
    start = order[i]

    def run(word):
        regs = {'edi': word, 'eax': 0, 'ecx': 0, 'rax': 0, 'rdx': 0}
        flags = (0, 0)  # the compare's two operands
        pc = start
        for _ in range(400):
            op, args = code[pc]
            nxt = order[index[pc] + 1]
            parts = [p.strip() for p in args.split('#')[0].split(',')] if args else []
            def val(x):
                if x in regs:
                    return regs[x]
                return int(x, 16) if x.startswith('0x') else int(x)
            def u(x):
                return x & 0xffffffff
            def s(x):
                x = u(x)
                return x - (1 << 32) if x >> 31 else x
            if op == 'cmp':
                flags = (val(parts[0]), val(parts[1]))
            elif op in ('je', 'jne', 'jg', 'jge', 'jl', 'jle', 'ja', 'jae', 'jb', 'jbe', 'jmp'):
                target = args.split()[0]
                if target in regs:
                    pc = regs[target] & 0xffffffffffffffff
                    continue
                a, b = flags
                take = {
                    'jmp': True,
                    'je': u(a) == u(b), 'jne': u(a) != u(b),
                    'jg': s(a) > s(b), 'jge': s(a) >= s(b), 'jl': s(a) < s(b), 'jle': s(a) <= s(b),
                    'ja': u(a) > u(b), 'jae': u(a) >= u(b), 'jb': u(a) < u(b), 'jbe': u(a) <= u(b),
                }[op]
                pc = int(target, 16) if take else nxt
                continue
            elif op == 'lea' and parts[0] in ('eax', 'ecx') and parts[1].startswith('[rdi'):
                m = re.match(r'\[rdi([+-])(0x[0-9a-f]+)\]', parts[1])
                k = int(m.group(2), 16)
                regs[parts[0]] = u(word + k if m.group(1) == '+' else word - k)
            elif op == 'lea' and parts[0] == 'rdx' and '#' in args:
                target = int(args.split('#')[1].strip(), 16)
                if target == pe.base:
                    regs['rdx'] = pe.base
                else:
                    return cstr(target)
            elif op in ('add', 'sub') and parts[0] in ('edi', 'ecx', 'eax') and HEXNUM.match(parts[1]):
                k = val(parts[1])
                regs[parts[0]] = u(regs[parts[0]] + k if op == 'add' else regs[parts[0]] - k)
                flags = (regs[parts[0]], 0)
            elif op == 'add' and parts == ['rax', 'rdx']:
                regs['rax'] = regs['rax'] + regs['rdx']
            elif op == 'mov' and parts[0] in ('ecx', 'eax') and parts[1] in regs:
                regs[parts[0]] = u(regs[parts[1]])
            elif op in ('cdqe',):
                regs['rax'] = s(regs['eax'])
            elif op == 'movsxd' and parts[0] == 'rax':
                regs['rax'] = s(regs[parts[1]])
            elif op == 'mov' and parts[0] == 'eax' and 'DWORD PTR [rdx+rax*4+' in parts[1]:
                off = int(re.search(r'\+(0x[0-9a-f]+)\]', parts[1]).group(1), 16)
                rva = regs['rdx'] - pe.base + off + 4 * regs['rax']
                regs['eax'] = struct.unpack_from('<I', pe.data, pe.off(rva))[0]
                regs['rax'] = regs['eax']
            elif op == 'movzx' and parts[0] == 'eax' and 'BYTE PTR [rdx+rax*1+' in parts[1]:
                off = int(re.search(r'\+(0x[0-9a-f]+)\]', parts[1]).group(1), 16)
                rva = regs['rdx'] - pe.base + off + regs['rax']
                regs['eax'] = pe.data[pe.off(rva)]
                regs['rax'] = regs['eax']
            elif op in ('xorps', 'ret'):
                return None
            pc = nxt
        return None

    for line in open(words_file):
        m = re.search(r'0x([0-9a-f]{8})', line)
        if m:
            w = int(m.group(1), 16)
            print(f'{w:08x} {run(w) or "-"}')


if __name__ == '__main__':
    main()
