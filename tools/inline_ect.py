#!/usr/bin/env python3
"""Decode the event-command lists Escha's exe builds on the stack and runs (0x140296bc0).

    inline_ect.py EXE VA SIZE    disassemble SIZE bytes from VA (start on a function) and print,
                                 at each call to the runner, the words stored before it, named
                                 from crates/formats/src/ebd_names.rs; '?' is a value computed at
                                 run time. Run from the repository root.

The battle's built-in cameras are such lists (docs/events.md, "The battle's own camera lists").
"""
import sys,struct,re,subprocess
sys.path.insert(0,'tools')
from pe import PE
p=PE(sys.argv[1]); start=int(sys.argv[2],16); size=int(sys.argv[3],16)
names={}
for line in open('crates/formats/src/ebd_names.rs'):
    m=re.search(r'\(0x([0-9a-f]+), "(\w+)"\)',line)
    if m: names.setdefault(int(m.group(1),16),m.group(2))
from pe import disasm_fixed; out=disasm_fixed(p,start,size)
if not isinstance(out,str):
    out='\n'.join(out)
xmm={}; stack={}
def rd(va,n): o=p.off(va-p.base); return p.data[o:o+n]
for line in out.splitlines():
    m=re.search(r'^\s*([0-9a-f]+):.*?\t([a-z].*)$',line)
    if not m: continue
    addr,ins=int(m.group(1),16),m.group(2)
    if (mm:=re.match(r'movdq[au]\s+xmm(\d+),XMMWORD PTR \[rip\+0x[0-9a-f]+\]\s+# 0x([0-9a-f]+)',ins)): xmm[mm[1]]=rd(int(mm[2],16),16)
    elif (mm:=re.match(r'xorps\s+xmm(\d+),xmm\1',ins)): xmm[mm[1]]=bytes(16)
    elif (mm:=re.match(r'movdq[au]\s+XMMWORD PTR \[rsp\+0x([0-9a-f]+)\],xmm(\d+)',ins)):
        b=xmm.get(mm[2],bytes(16))
        for k in range(16): stack[int(mm[1],16)+k]=b[k]
    elif (mm:=re.match(r'mov\s+DWORD PTR \[rsp\+0x([0-9a-f]+)\],0x([0-9a-f]+)',ins)):
        b=struct.pack('<I',int(mm[2],16))
        for k in range(4): stack[int(mm[1],16)+k]=b[k]
    elif (mm:=re.match(r'movss\s+DWORD PTR \[rsp\+0x([0-9a-f]+)\],xmm(\d+)',ins)) or (mm:=re.match(r'mov\s+DWORD PTR \[rsp\+0x([0-9a-f]+)\],e',ins)):
        for k in range(4): stack[int(mm[1],16)+k]=None  # runtime value
    elif 'call   0x140296bc0' in ins:
        if not stack: print(f"-- call at {addr:#x}: (no list)"); continue
        lo=min(stack)
        words=[];o=lo
        while all(o+k in stack for k in range(4)):
            bs=[stack[o+k] for k in range(4)]
            words.append(None if None in bs else struct.unpack('<i',bytes(bs))[0]); o+=4
        txt=[]
        for w in words:
            if w is None: txt.append('?')
            elif (w&0xffffffff) in names: txt.append('\n   '+names[w&0xffffffff])
            elif w==0x7f000000: txt.append('\n   END')
            else: txt.append(str(w))
        print(f'-- call at {addr:#x}:',' '.join(txt)); stack={}
