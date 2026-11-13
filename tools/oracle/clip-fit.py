#!/usr/bin/env python3
"""Finds clips (a voice line from `barrel voice-wav`, an effect) in a mix and the gain each plays at.

    tools/oracle/clip-fit.py MIX.wav CLIP.wav [CLIP.wav ...]

MIX is a recording of the game or a Barrel capture (`barrel-view --audio`). Each clip is found by
cross-correlation (normalised by the mix's local energy) and its gain fitted by least squares; what
else plays under it (music) does not correlate with it. Both are taken to mono with FFmpeg's
downmix, so compare gains between mixes, not with 1. Needs numpy and ffmpeg.
"""
import subprocess
import sys

import numpy as np

R = 16000


def mono(p):
    raw = subprocess.run(["ffmpeg","-v","error","-i",p,"-ac","1","-ar",str(R),"-f","f32le","-"],check=True,capture_output=True).stdout
    return np.frombuffer(raw, np.float32).astype(np.float64)
g = mono(sys.argv[1])
for clip in sys.argv[2:]:
    v = mono(clip)
    n = 1 << int(np.ceil(np.log2(len(g) + len(v))))
    c = np.fft.irfft(np.fft.rfft(g, n) * np.conj(np.fft.rfft(v, n)), n)[:len(g) - len(v)]
    # normalise by the game's local energy so loud music doesn't win
    e = np.sqrt(np.convolve(g * g, np.ones(len(v)), 'valid')[:len(c)] + 1e-9)
    k = int(np.argmax(c / e))
    seg = g[k:k + len(v)]
    gain = seg.dot(v) / v.dot(v)
    resid = seg - gain * v
    print("%s at %.3f s, gain %.3f (%.1f dB), match %.2f, residual %.1f dB" % (clip.split('/')[-1], k / R, gain, 20*np.log10(abs(gain)), (c/e)[k] / np.sqrt(v.dot(v)), 20*np.log10(np.sqrt((resid**2).mean()))))
