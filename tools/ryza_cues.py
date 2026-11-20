#!/usr/bin/env python3
"""Ryza's cue log from a ryza-recomp hooks.log (scenarios/hooks/sound.hooks), in the line format
Barrel's BARREL_TRACE_CUES writes: `<frame> <kind> <hash>` (kind music, voice, effect, stop).

The sound manager's play (`play`) takes a request whose first word is the sound's hash and
third word a delay in seconds; music goes through the music request (`v28` right before it).
Effects are the hashes sfx.xml names; the rest not in stream.xml are voice clips (the voice
container's streams have no table). `v38` with a first argument below 0x100 stops a channel:
logged as one `stop` for a burst of them (within 5 frames), once a sound has played.

usage: ryza_cues.py HOOKS_LOG STREAM_XML SFX_XML
"""
import re
import struct
import sys


def tags(path):
    out = {}
    for m in re.finditer(r'hash="0x([0-9A-Fa-f]+)" tag="([^"]+)" file="([^"]+)"', open(path, encoding="shift_jis", errors="replace").read()):
        out[int(m.group(1), 16)] = (m.group(2), m.group(3))
    return out


def main(log, stream_xml, sfx_xml):
    streams, sfx = tags(stream_xml), tags(sfx_xml)
    music_at = None
    last_stop, played = -100, False
    for line in open(log):
        w = line.split()
        if len(w) < 4 or w[0] != "[hook]":
            continue
        frame, label, args = int(w[1]), w[2], w[3:]
        if label == "v28":
            music_at = frame
        elif label == "v38" and int(args[0], 16) < 0x100 and played:
            if frame - last_stop > 5:
                print(f"{frame} stop -")
            last_stop = frame
        elif label == "play":
            h = int(args[0], 16)
            delay = struct.unpack("<f", struct.pack("<I", int(args[2], 16)))[0]
            at = frame + round(delay * 60)
            if music_at == frame and h in streams and streams[h][1] == "BGM":
                kind = "music"
            elif h in sfx:
                kind = "effect"
            else:
                kind = "voice"
            played = True
            print(f"{at} {kind} {h:08x}")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(*sys.argv[1:])
