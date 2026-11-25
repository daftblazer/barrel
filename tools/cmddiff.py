#!/usr/bin/env python3
# Compares the commands an event runs in the original and in Barrel, group by group (the
# word's letter: p, q, s, t, u, v, w).
#   tools/cmddiff.py GAME BARREL [--from FRAME] [--to FRAME] [--groups stuq] [--all]
# GAME    the answer key's log, hex: `first-last runner word operands...`
#         (ryza-recomp: tools/cmdlog.py --hex hooks.log)
# BARREL  `barrel-tool ryza-commands INSTALL EVENT`: `frame aACTION word operands...`
# --from/--to keep the game's commands that start in those frames (the log holds everything
# the game ran, not one event). A command repeated back to back counts once (a script's wait
# loop). Per group: how many commands each side has and how many pair up in order, then the
# commands only one side has (the first 12; --all for every one), then the timing of the
# pairs: the game's frame less Barrel's, as the commonest offset and the pairs off it.
import collections, difflib, struct, sys

def operand(v):
    f = struct.unpack('<f', struct.pack('<I', v))[0]
    if 0x01000000 <= v < 0xff000000 and 1e-6 < abs(f) < 1e7:
        return '%.6gf' % f
    return str(v - (1 << 32) if v >= 0x80000000 else v)

def name(words):
    w = words[0]
    flag = '/%x' % (w >> 16 & 0xff) if w >> 16 & 0xff else ''
    return ' '.join(['%c%d%s' % (w >> 24, w & 0xffff, flag)] + [operand(v) for v in words[1:]])

def load(path, game, lo, hi):
    out = collections.defaultdict(list)
    for line in open(path):
        f = line.split()
        if len(f) < 3:
            continue
        frame = int(f[0].split('-')[0])
        if game and not lo <= frame <= hi:
            continue
        words = tuple(int(x, 16) for x in f[2:])
        if not 0x70 <= words[0] >> 24 <= 0x7e:
            continue
        seq = out[chr(words[0] >> 24)]
        if seq and seq[-1][1] == words:
            continue
        seq.append((frame, words, f[1]))
    return out

args = sys.argv[1:]
def option(flag, default):
    if flag in args:
        i = args.index(flag)
        v = args[i + 1]
        del args[i:i + 2]
        return v
    return default
lo, hi = int(option('--from', 0)), int(option('--to', 1 << 60))
groups = option('--groups', 'pqstuvw')
every = '--all' in args
if every:
    args.remove('--all')
game, barrel = load(args[0], True, lo, hi), load(args[1], False, 0, 0)
total = [0, 0, 0]
for g in groups:
    a, b = game.get(g, []), barrel.get(g, [])
    if not a and not b:
        continue
    blocks = difflib.SequenceMatcher(None, [w for _, w, _ in a], [w for _, w, _ in b], autojunk=False).get_matching_blocks()
    pairs = [(a[i + k], b[j + k]) for i, j, n in blocks for k in range(n)]
    for k, v in enumerate((len(a), len(b), len(pairs))):
        total[k] += v
    print('%s: game %d, Barrel %d, paired %d' % (g, len(a), len(b), len(pairs)))
    ga, gb = {id(p[0]) for p in pairs}, {id(p[1]) for p in pairs}
    only = [('game only  ', x) for x in a if id(x) not in ga] + [('Barrel only', x) for x in b if id(x) not in gb]
    for side, (frame, words, who) in only if every else only[:12]:
        print('  %s %6d %-4s %s' % (side, frame, who, name(words)))
    if len(only) > 12 and not every:
        print('  ... and %d more' % (len(only) - 12))
    if pairs:
        offs = collections.Counter(x[0] - y[0] for x, y in pairs)
        common = offs.most_common(1)[0][0]
        off = [(x, y) for x, y in pairs if abs(x[0] - y[0] - common) > 1]
        print('  timing: game - Barrel = %d frames for %d of %d pairs; %d off it' % (common, len(pairs) - len(off), len(pairs), len(off)))
        print('  exact: %d of %d pairs at that offset to the frame' % (sum(1 for x, y in pairs if x[0] - y[0] == common), len(pairs)))
        for x, y in off if every else off[:8]:
            print('    game %d, Barrel %d (%+d): %s' % (x[0], y[0], x[0] - y[0] - common, name(x[1])))
print('total: game %d, Barrel %d, paired %d' % tuple(total))
