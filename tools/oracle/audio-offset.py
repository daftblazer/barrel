#!/usr/bin/env python3
"""Lines a Barrel sound capture up against the game's recording.

    tools/oracle/audio-offset.py GAME.wav BARREL.wav

Both are taken to mono at 8 kHz (through ffmpeg); their loudness envelopes (10 ms steps) are
cross-correlated for the offset at which BARREL best fits inside GAME, then the two levels are
printed every two seconds from there. Needs numpy and ffmpeg.
"""
import subprocess
import sys

import numpy as np

RATE, STEP = 8000, 80


def mono(path):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-ac", "1", "-ar", str(RATE), "-f", "f32le", "-"],
                         check=True, capture_output=True).stdout
    return np.frombuffer(raw, np.float32)


def envelope(x):
    e = np.sqrt(np.convolve(x * x, np.ones(STEP) / STEP, "valid")[::STEP])
    return (e - e.mean()) / (e.std() + 1e-9)


def db(x):
    return 20 * np.log10(np.sqrt((x * x).mean()) + 1e-9)


game, barrel = mono(sys.argv[1]), mono(sys.argv[2])
if len(barrel) > len(game):
    sys.exit("the Barrel capture must be the shorter one")
c = np.correlate(envelope(game), envelope(barrel), "valid") / (len(barrel) // STEP)
k = int(np.argmax(c))
print("best offset: game = barrel + %.2f s, correlation %.2f" % (k * STEP / RATE, c[k]))
aligned = game[k * STEP:k * STEP + len(barrel)]
for t in range(0, len(barrel) // RATE, 2):
    a, g = barrel[t * RATE:(t + 1) * RATE], aligned[t * RATE:(t + 1) * RATE]
    print("%4d s  barrel %6.1f dB  game %6.1f dB" % (t, db(a), db(g)))
