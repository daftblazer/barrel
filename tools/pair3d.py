# pair3d.py CAPTURE FRAME G3D.txt B3D.jsonl [OPL2.txt [HIDE.txt]]: pairs Barrel's rigid scene draws
# (a BARREL_TRACE_DRAWS line, with boxes) with the game's (the capture's vertex buffers through
# each draw's mL2W, as `barrel-parity draws CAPTURE FRAME INDEX.tsv` prints it) by index count and box centre. Prints
# Barrel's unpaired draws (depth, place on screen, box diagonal, how far outside the view), the
# paired ones wholly outside the view, and each side's distance outside against its box. With
# a placement listing (`BARREL_VERBOSE=1 barrel-tool opl2 FILE#N`) also the placement words of
# both; HIDE.txt gets the unpaired draws in view (index count and box corner).
import json,sys,re,struct,math,collections
C,frame,g3d,b3d=sys.argv[1],int(sys.argv[2]),sys.argv[3],sys.argv[4]
l2w={}; cur=None
for l in open(g3d):
    m=re.match(r'\s+#(\d+) count',l)
    if m: cur=int(m.group(1)); continue
    m=re.match(r'\s+mL2W = \[(.*)\]',l)
    if m and cur is not None:
        v=[float(x) for x in m.group(1).split()]
        if len(v)==12: l2w[cur]=v
G=collections.defaultdict(list); cur=None; cache={}
for l in open(C+'/draws.jsonl'):
    if l.startswith('{"frame"'): cur=json.loads(l)['frame']; continue
    if cur!=frame or '"draw"' not in l: continue
    e=json.loads(l)
    if len(e['rt'])<4 or e.get('ds','-')=='-' or e['draw'] not in l2w or 'indices' not in e: continue
    vb=e['vb'][0]; k=vb['data']
    if k not in cache:
        b=open(C+'/blobs/'+vb['data']+'.bin','rb').read(); st=vb['stride']
        ps=[struct.unpack_from('<3f',b,i*st) for i in range(len(b)//st)]
        cache[k]=ps
    ps=cache[k]
    if not ps: continue
    m=l2w[e['draw']]
    w=[(m[0]*x+m[1]*y+m[2]*z+m[3], m[4]*x+m[5]*y+m[6]*z+m[7], m[8]*x+m[9]*y+m[10]*z+m[11]) for x,y,z in ps]
    lo=[min(p[i] for p in w) for i in range(3)]; hi=[max(p[i] for p in w) for i in range(3)]
    G[e['count']].append((e['draw'],lo,hi))
d=json.loads(open(b3d).readline()); vp=d['view_proj']
def clip(p):
    x,y,z=p; return [vp[r]*x+vp[4+r]*y+vp[8+r]*z+vp[12+r] for r in range(4)]
out=[]; paired=0; gleft=0; pairs=[]
B=collections.defaultdict(list)
for x in d['draws3d']:
    if x['pass']=='scene' and 'box' in x and 'bones' not in x:
        b=x['box']; w=x['world']
        # corners through world, then game space (x mirrored)
        cs=[(cx,cy,cz) for cx in (b[0],b[3]) for cy in (b[1],b[4]) for cz in (b[2],b[5])]
        ws=[(w[0]*cx+w[4]*cy+w[8]*cz+w[12], w[1]*cx+w[5]*cy+w[9]*cz+w[13], w[2]*cx+w[6]*cy+w[10]*cz+w[14]) for cx,cy,cz in cs]
        lo=[min(p[i] for p in ws) for i in range(3)]; hi=[max(p[i] for p in ws) for i in range(3)]
        B[x['idx']].append((x,lo,hi))
for idx,bs in B.items():
    gs=list(G.get(idx,[]))
    for x,lo,hi in bs:
        c=[(lo[i]+hi[i])/2 for i in range(3)]
        best=None
        for j,(n,glo,ghi) in enumerate(gs):
            gc=[-(glo[0]+ghi[0])/2,(glo[1]+ghi[1])/2,(glo[2]+ghi[2])/2]
            dd=math.dist(c,gc)
            if best is None or dd<best[1]: best=(j,dd)
        if best and best[1]<150: gs.pop(best[0]); paired+=1; pairs.append((idx,x,lo,hi,c,best[1]))
        else: out.append((idx,x,lo,hi,c,best[1] if best else None))
    gleft+=len(gs)
print('paired',paired,'barrel only',len(out),'game left (same counts)',gleft)
def outside(lo,hi):
    # how far the box is outside the view: the largest over planes of the smallest corner excess (in w units)
    cs=[clip((x,y,z)) for x in (lo[0],hi[0]) for y in (lo[1],hi[1]) for z in (lo[2],hi[2])]
    worst=-1e9; which=''
    for name,f in (('left',lambda c:-c[0]-c[3]),('right',lambda c:c[0]-c[3]),('bottom',lambda c:-c[1]-c[3]),('top',lambda c:c[1]-c[3]),('near',lambda c:-c[3])):
        m=min(f(c) for c in cs)
        if m>worst: worst=m; which=name
    return worst,which
print('paired draws wholly outside the view:',sum(1 for o in pairs if outside(o[2],o[3])[0]>0), [ (o[0],round(outside(o[2],o[3])[0]),outside(o[2],o[3])[1]) for o in pairs if outside(o[2],o[3])[0]>0][:40])
for idx,x,lo,hi,c,nd in sorted(out,key=lambda o:clip(o[4])[3]):
    if nd is None: continue
    cl=clip(c); size=math.dist(lo,hi); o=outside(lo,hi)
    t=(x['tex'][0] if x['tex'] else '-').split(':')[-1]
    print('%6d w %7.0f ndc %6.2f %6.2f diag %6.0f out %7.0f %-6s nearest %.0f %s'%(idx,cl[3],cl[0]/cl[3],cl[1]/cl[3],size,o[0],o[1],nd,t[:20]))
# placements: which word the paired and the unpaired carry
if len(sys.argv)>5:
    P=[]; name=None
    for l in open(sys.argv[5]):
        m=re.match(r'\s+at \[([^\]]*)\] turned \[[^\]]*\] scaled \[[^\]]*\] (\S*)\s*(0x[0-9a-f]+)',l)
        if m: P.append(([float(v) for v in m.group(1).split(',')],name,m.group(2),int(m.group(3),16)))
        elif not l.startswith('       '):
            w=l.split(); name=w[1] if len(w)>1 else None
    def near(c,lo,hi):
        g=(-c[0],c[1],c[2]); best=None
        for p,n,t,w in P:
            if -hi[0]-50<=p[0]<=-lo[0]+50 and lo[2]-50<=p[2]<=hi[2]+50:
                d=math.dist(g,p)
                if best is None or d<best[0]: best=(d,n,t,w)
        return best
    tab=collections.Counter(); detail=collections.Counter()
    for kind,lst in (('paired',pairs),('only',[o for o in out if o[5] is not None and outside(o[2],o[3])[0]<=0])):
        for o in lst:
            b=near(o[4],o[2],o[3])
            tab[(kind,'%#x'%b[3] if b else '-')]+=1
            if kind=='only': detail[(b[1] if b else '-','%#x'%b[3] if b else '-',o[0])]+=1
    for k in sorted(tab): print(k,tab[k])
    for k,v in sorted(detail.items()): print(v,k)
if len(sys.argv)>6:
    with open(sys.argv[6],'w') as f:
        for o in out:
            if o[5] is not None and outside(o[2],o[3])[0]<=0:
                b=o[1]['box']; f.write('%d %r %r %r\n'%(o[0],b[0],b[1],b[2]))
# side planes in world units: the centre's distance outside each plane against the box's half diagonal
def planes():
    r=lambda i:[vp[i],vp[4+i],vp[8+i],vp[12+i]]
    x,y,w=r(0),r(1),r(3); ps=[]
    for name,a,sg in (('left',x,1),('right',x,-1),('bottom',y,1),('top',y,-1)):
        p=[w[i]+sg*a[i] for i in range(4)]; n=math.sqrt(p[0]**2+p[1]**2+p[2]**2)
        ps.append((name,[v/n for v in p]))
    return ps
PL=planes()
def cdist(c):  # largest distance of the centre outside a plane
    return max((-(p[0]*c[0]+p[1]*c[1]+p[2]*c[2]+p[3]),n) for n,p in PL)
print('--- outside the box test: centre distance outside, half diagonal, half extents')
for kind,lst in (('game draws',[o for o in pairs if outside(o[2],o[3])[0]>0]),('game hides',[o for o in out if o[5] is not None and outside(o[2],o[3])[0]>0 and clip(o[4])[3]>0])):
    for o in sorted(lst,key=lambda o:cdist(o[4])[0]-math.dist(o[2],o[3])/2):
        d,n=cdist(o[4]); r=math.dist(o[2],o[3])/2
        print('%-10s %6d %-6s centre out %6.0f  r %6.0f  d-r %6.0f  d/r %.2f'%(kind,o[0],n,d,r,d-r,d/r))
