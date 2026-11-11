#!/usr/bin/env python3
"""Recover the fixed second scramble over PAK entry names (Atelier Sophie 2, Ryza 1-3 DX).

    pak_key.py FILE.PAK [FILE.PAK ...]

Each entry's name is XORed with its own 32-byte key and with a fixed 32-byte key for the
game, both repeating every 32 bytes (docs/survey.md, "PAK archives"). With the entry's key
removed, this guesses the fixed bytes one position at a time: the names are assumed to
start with \\Data\\x64\\, and at each later position the byte that turns the most names
still being read into path characters wins (a 0 counts only right after an extension).
Prints the bytes found, in hex, and a sample of the names they give. The fixed key repeats
every 32 bytes, so the later blocks of the result, where the names vary most, show it
best; the first block can be off where every name has the same letters (Sophie 2's names
start with \\data, not \\Data). This is how Sophie 2's key was found; it is the way to
find each Ryza game's.
"""
import struct,sys
files=sys.argv[1:]
names=[]
for f in files:
    with open(f,'rb') as fh:
        h=fh.read(16); cnt=struct.unpack_from('<I',h,4)[0]
        d=fh.read(184*cnt)
    for i in range(cnt):
        e=d[184*i:184*(i+1)]
        names.append(bytes(e[j]^e[132+j%32] for j in range(128)))
print(len(names), file=sys.stderr)
ok=set(b'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_\\./-')
seed=b'\\Data\\x64\\'
K=[names[0][j]^seed[j] for j in range(len(seed))]
dec=[bytearray(n[j]^K[j] for j in range(len(seed))) for n in names]
alive=list(range(len(names)))
lower=set(b'abcdefghijklmnopqrstuvwxyz0123456789_')
def sc(i,p,k):
    c=names[i][p]^k
    if c==0: return 3 if b'.' in dec[i][-5:] else -5
    if c in lower: return 2
    if c in ok: return 1
    return -3
for p in range(len(seed),128):
    if not alive: break
    best=max(range(256), key=lambda k: sum(sc(i,p,k) for i in alive))
    score=sum(sc(i,p,best) for i in alive)
    K.append(best)
    for i in alive: dec[i].append(names[i][p]^best)
    alive=[i for i in alive if names[i][p]^best!=0]
    print(p, hex(best), score, len(alive), file=sys.stderr)
key=bytes(K)
print(key.hex())
for i in range(0,len(names),max(1,len(names)//15)):
    n=bytes(a^b for a,b in zip(names[i],key)); print(n.split(b'\0')[0])
