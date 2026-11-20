#!/usr/bin/env python3
"""One line a captured frame of a draw capture: draws, draws into the back buffer (the 2D
layer), and the UI files (res_*/ui) whose textures the frame samples. Shows a run's screens
(title, load screen, loading, field) at a glance.

usage: tools/phases.py DRAWS.jsonl TEXTURES.tsv
"""
import json, re, sys

names = {}
for line in open(sys.argv[2]):
    f = line.rstrip('\n').split('\t')
    if len(f) > 1:
        names['tex' + f[0]] = f[1]

def ui(name):
    m = re.search(r'\\ui\\([^\\#]+)', name)
    return m.group(1) if m else None

frame = None
def flush():
    if frame is not None:
        print(f"{frame:5} draws {draws:5} back {back:4}  {' '.join(sorted(files))}")

for line in open(sys.argv[1]):
    d = json.loads(line)
    if 'frame' in d and len(d) == 1:
        flush()
        frame, draws, back, files = d['frame'], 0, 0, set()
    elif 'draw' in d:
        draws += 1
        if d.get('rt') == ['backbuffer']:
            back += 1
        for t in d.get('ps_tex', {}).values():
            u = ui(names.get(t, ''))
            if u:
                files.add(u.removesuffix('.g1t'))
flush()
