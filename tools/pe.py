#!/usr/bin/env python3
"""Look things up in an x64 PE executable: sections, RVAs, MSVC RTTI and vtables.

    pe.py EXE vtables CLASS        the vtables of a class named in RTTI (e.g. EventAnalyzer)
    pe.py EXE disasm VA [COUNT]    disassemble COUNT bytes (default 0x200) from VA with objdump
    pe.py EXE xrefs VA             code addresses whose RIP-relative operand points at VA
    pe.py EXE find-string TEXT     VAs of an ASCII string

Reads at most the executable itself; prints virtual addresses (image base included).
"""
import re
import struct
import subprocess
import sys


class PE:
    def __init__(self, path):
        self.path = path
        self.data = open(path, 'rb').read()
        d = self.data
        pe = struct.unpack_from('<I', d, 0x3c)[0]
        assert d[pe:pe + 4] == b'PE\0\0'
        nsec = struct.unpack_from('<H', d, pe + 6)[0]
        opt_size = struct.unpack_from('<H', d, pe + 20)[0]
        opt = pe + 24
        assert struct.unpack_from('<H', d, opt)[0] == 0x20b, 'not PE32+'
        self.base = struct.unpack_from('<Q', d, opt + 24)[0]
        self.sections = []
        s = opt + opt_size
        for i in range(nsec):
            name = d[s:s + 8].rstrip(b'\0').decode()
            vsize, va, rsize, raw = struct.unpack_from('<IIII', d, s + 8)
            chars = struct.unpack_from('<I', d, s + 36)[0]
            self.sections.append((name, va, max(vsize, rsize), raw, rsize, chars))
            s += 40

    def section(self, name):
        return next(x for x in self.sections if x[0] == name)

    def off(self, rva):
        for name, va, size, raw, rsize, _ in self.sections:
            if va <= rva < va + rsize:
                return raw + rva - va
        return None

    def rva_of_off(self, off):
        for name, va, size, raw, rsize, _ in self.sections:
            if raw <= off < raw + rsize:
                return va + off - raw
        return None

    def q(self, rva):
        o = self.off(rva)
        return struct.unpack_from('<Q', self.data, o)[0]

    def code_sections(self):
        return [x for x in self.sections if x[5] & 0x20]

    def vtables(self, cls):
        name = ('.?AV%s@@' % cls).encode()
        out = []
        for m in re.finditer(re.escape(name) + b'\0', self.data):
            td = self.rva_of_off(m.start() - 16)
            if td is None:
                continue
            # complete object locators: signature 1, offset, cd offset, td rva, chd rva, self rva
            pat = struct.pack('<I', td)
            for c in re.finditer(re.escape(pat), self.data):
                col_off = c.start() - 12
                if col_off < 0:
                    continue
                sig, offset, cd, tdr, chd, selfr = struct.unpack_from('<IIIIII', self.data, col_off)
                col = self.rva_of_off(col_off)
                if sig != 1 or selfr != col:
                    continue
                ptr = struct.pack('<Q', self.base + col)
                for p in re.finditer(re.escape(ptr), self.data):
                    vt = self.rva_of_off(p.start() + 8)
                    if vt is None:
                        continue
                    funcs = []
                    r = vt
                    while True:
                        v = self.q(r)
                        rv = v - self.base
                        if not any(va <= rv < va + size for _, va, size, _, _, _ in self.code_sections()):
                            break
                        funcs.append(v)
                        r += 8
                    out.append((offset, self.base + vt, funcs))
        return out

    def disasm(self, va, count=0x200):
        rva = va - self.base
        o = self.off(rva)
        return subprocess.run(
            ['objdump', '-D', '-b', 'binary', '-m', 'i386:x86-64', '-M', 'intel',
             '--adjust-vma=%#x' % (va - (o - o)), '--start-address=%#x' % va,
             '--stop-address=%#x' % (va + count), self.path_slice(o, count, va)],
            capture_output=True, text=True).stdout

    def path_slice(self, off, count, va):
        import os
        import tempfile
        fd, p = tempfile.mkstemp(suffix='.bin', dir=os.environ.get('PE_TMP'))
        os.write(fd, self.data[off:off + count])
        os.close(fd)
        self._tmp = p
        return p

    def xrefs(self, target):
        hits = []
        for name, va, size, raw, rsize, _ in self.code_sections():
            d = self.data[raw:raw + rsize]
            # any disp32 at position i whose RIP (end of instruction) + disp == target.
            # The instruction end is unknown, so try ends of i+4 .. i+8 (imm8/imm32 tails).
            for i in range(0, len(d) - 4):
                disp = struct.unpack_from('<i', d, i)[0]
                for tail in (4, 5, 8):
                    if self.base + va + i + tail + disp == target:
                        hits.append(self.base + va + i)
        return hits


def disasm_fixed(pe, va, count):
    import os
    out = pe.disasm(va, count)
    try:
        os.unlink(pe._tmp)
    except OSError:
        pass
    # objdump with --adjust-vma on a slice: relabel addresses
    lines = []
    for line in out.splitlines():
        m = re.match(r'\s*([0-9a-f]+):\t(.*)', line)
        if m:
            lines.append('%x:\t%s' % (int(m.group(1), 16), m.group(2)))
    return '\n'.join(lines)


def main():
    exe, cmd, *rest = sys.argv[1:]
    pe = PE(exe)
    if cmd == 'vtables':
        for offset, vt, funcs in pe.vtables(rest[0]):
            print('vtable %#x (subobject offset %d), %d entries' % (vt, offset, len(funcs)))
            for i, f in enumerate(funcs):
                print('  [%2d] %#x' % (i, f))
    elif cmd == 'disasm':
        va = int(rest[0], 16)
        count = int(rest[1], 16) if len(rest) > 1 else 0x200
        print(disasm_fixed(pe, va, count))
    elif cmd == 'xrefs':
        for h in pe.xrefs(int(rest[0], 16)):
            print('%#x' % h)
    elif cmd == 'find-string':
        for m in re.finditer(re.escape(rest[0].encode()) + b'\0', pe.data):
            print('%#x' % (pe.base + pe.rva_of_off(m.start())))
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
