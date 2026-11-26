#!/usr/bin/env python3
"""The synthesis rules' answer key: the game's `setq` / `ringeval` hook lines (ryza-recomp scenarios/synth-rules/hooks.log) as the rows
`barrel-tool ryza-rules` prints. Usage: synth_rules_key.py HOOKS.log [rings]

setq words: rdx (the quality, hex), xmm6 (mean + bonus), xmm7 (clamped), count, sum, bonus 0x157, other 0x15b, 16 totals (word 0 = the rings'
quality bonus), B, mode, then 4 effect slots of {tier, id, ptr lo, ptr hi}. ringeval words: def, state, 32 state words (16 rings of value, lit).
"""
import sys

def signed(t):
    v = int(t, 16)
    return v - (1 << 32) if v >= 1 << 31 else v

def num(t):
    f = float(t)
    return str(int(f)) if f == int(f) else repr(f)

def main():
    path, mode = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else "")
    seen = set()
    for line in open(path):
        w = line.split()
        if len(w) < 3 or w[0] != "[hook]":
            continue
        frame, kind, a = w[1], w[2], w[3:]
        if kind == "setq" and mode != "rings":
            q, mean, _clamp, count, total = int(a[0], 16), a[1], a[2], a[3], a[4]
            bonus_a = a[7]
            slots = a[25:41]
            tiers = [signed(slots[i * 4]) for i in range(4)]
            row = f"{frame} q={q} mean={num(mean)} sum={num(total)} count={count} a={bonus_a} tiers={','.join(map(str, tiers))}"
        elif kind == "ringeval" and mode == "rings":
            row = f"{frame} rings {' '.join(str(signed(t)) for t in a[2:34])}"
        else:
            continue
        if row not in seen:
            seen.add(row)
            print(row)

main()
