#!/usr/bin/env python3
"""Unscramble an encrypted table (`.xml.e`, Sophie 2 and the Ryza games): prototype.

    xml_e.py FILE.xml.e

Runs the stages read from the Ryza DX executable (docs/survey.md, "Encrypted tables") up
to the decompression, which is still to be read, and prints the stored checksums next to
the computed ones.
"""
import struct,sys
MULT=0x3b9a73c9
class Rng:
    def __init__(s,state): s.state=state&0xffffffff
    def next(s):
        s.state=(s.state*MULT+0x2f09)&0xffffffff
        v=s.state
        if v&0x80000000: v-=1<<32
        return (v>>16)&0x7fff
def bit_shuffle(buf,start,length,seed,chunk):
    rng=Rng(seed); pos=start
    while length>0:
        n=min(length,chunk); nbits=n*8
        lst=list(range(nbits)); perm=[]
        for i in range(nbits):
            idx=rng.next()%len(lst); perm.append(lst.pop(idx))
        for k in range(nbits//2):
            a,b=perm[2*k],perm[2*k+1]
            ba=(buf[pos+(a>>3)]>>(a&7))&1; bb=(buf[pos+(b>>3)]>>(b&7))&1
            buf[pos+(a>>3)]=(buf[pos+(a>>3)]&~(1<<(a&7)))|(bb<<(a&7))
            buf[pos+(b>>3)]=(buf[pos+(b>>3)]&~(1<<(b&7)))|(ba<<(b&7))
        pos+=n; length-=n
def words(buf,seed):
    rng=Rng(seed)
    for i in range(0,len(buf)//2*2,2):
        r=rng.next(); w=(buf[i]<<8)|buf[i+1]
        if r%0x1462>=0xa31: w^=r
        w=(w-r)&0xffff
        buf[i]=w>>8; buf[i+1]=w&0xff
def decode(raw):
    ver,size=struct.unpack_from('>II',raw,0)
    d=bytearray(raw[16:])
    n=len(d); last=min(n,0x800)
    bit_shuffle(d,n-last,last,0x6d3f,0x100)
    words(d,0xcb53)
    return ver,size,d

def stage3(d):
    n=len(d)
    e=n-12
    while e>0 and d[e-1]==0: e-=1
    edi=e-1
    assert d[edi]==0xff, 'no end marker'
    mult=(struct.unpack_from('>I',d,n-4)[0]+MULT)&0xffffffff
    states=[0xa83b,0xb11d,0x88a5]; lens=[0x1d,0x13,0xb]
    pos=0; g=0; extra=0
    while pos<edi:
        st=states[g]; run=lens[g]+extra
        for _ in range(run):
            if pos>=edi: break
            st=(st*mult+0x2f09)&0xffffffff
            v=st if st<0x80000000 else st-(1<<32)
            d[pos]^=(v>>16)&0xff; pos+=1
        states[g]=st
        if g==2: extra+=1
        g=(g+1)%3
    d[edi]=0
    s_sub=0; s_xor=0
    for i in range(0,edi//4*4,4):
        v=struct.unpack_from('>I',d,i)[0]; s_sub=(s_sub-v)&0xffffffff; s_xor^=(~v)&0xffffffff
    want_xor=struct.unpack_from('>I',d,n-8)[0]; want_sub=struct.unpack_from('>I',d,n-12)[0]
    print('checksums', hex(s_xor),hex(want_xor), hex(s_sub),hex(want_sub))
    out=d[:edi]
    bit_shuffle(out,0,min(edi,0x800),0x74b9,0x80)
    return out
if __name__ == '__main__':
    ver, size, d = decode(open(sys.argv[1], 'rb').read())
    out = stage3(d)
    print('version', ver, 'plain size', size, 'compressed', len(out))
    print(out[:64].hex())
