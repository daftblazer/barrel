#!/usr/bin/env python3
"""A virtual Xbox 360 pad for oracle runs (uinput through python-evdev), driven through
work/oracle/padcmd. Start it before the game so the game finds it at start-up.

    tools/oracle/pad.py            (run in the background; ends on "quit")

Commands, one per line:
    tap BUTTON [MS]         press and release (default 100 ms held)
    hold BUTTON MS
    stick L|R X Y MS        hold a stick at X, Y (-1..1, y down positive) for MS, then centre
    dpad UP|DOWN|LEFT|RIGHT [MS]
    wait MS
    quit
Buttons: A B X Y LB RB BACK START GUIDE LS RS LT RT.
Each command's completion is appended to work/oracle/paddone.
"""
import os
import time
from evdev import AbsInfo, UInput, ecodes as e

here = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
work = os.path.join(here, "work", "oracle")
os.makedirs(work, exist_ok=True)
fifo = os.path.join(work, "padcmd")
if os.path.exists(fifo):
    os.remove(fifo)
os.mkfifo(fifo)
done = open(os.path.join(work, "paddone"), "a", buffering=1)

BUTTONS = {
    # As the kernel's xpad driver reports an Xbox 360 pad: X is BTN_X and Y is BTN_Y (whose
    # aliases, BTN_NORTH and BTN_WEST, read the other way round; SDL and the game take X as the
    # west button). Before 2026-11-14 X and Y were swapped here.
    "A": e.BTN_SOUTH, "B": e.BTN_EAST, "X": e.BTN_X, "Y": e.BTN_Y,
    "LB": e.BTN_TL, "RB": e.BTN_TR, "BACK": e.BTN_SELECT, "START": e.BTN_START,
    "GUIDE": e.BTN_MODE, "LS": e.BTN_THUMBL, "RS": e.BTN_THUMBR,
}
stick = AbsInfo(value=0, min=-32768, max=32767, fuzz=16, flat=128, resolution=0)
trigger = AbsInfo(value=0, min=0, max=255, fuzz=0, flat=0, resolution=0)
hat = AbsInfo(value=0, min=-1, max=1, fuzz=0, flat=0, resolution=0)
caps = {
    e.EV_KEY: list(BUTTONS.values()),
    e.EV_ABS: [(e.ABS_X, stick), (e.ABS_Y, stick), (e.ABS_RX, stick), (e.ABS_RY, stick),
               (e.ABS_Z, trigger), (e.ABS_RZ, trigger), (e.ABS_HAT0X, hat), (e.ABS_HAT0Y, hat)],
}
pad = UInput(caps, name="Barrel oracle pad (Xbox 360)", vendor=0x045E, product=0x028E, version=0x110, bustype=e.BUS_USB)


def emit(kind, code, value):
    pad.write(kind, code, value)
    pad.syn()


def ms(words, i, default):
    return (int(words[i]) if len(words) > i else default) / 1000


while True:
    with open(fifo) as f:
        for line in f:
            w = line.split()
            if not w:
                continue
            try:
                if w[0] == "quit":
                    done.write("ok quit\n")
                    pad.close()
                    raise SystemExit
                if w[0] in ("tap", "hold"):
                    name = w[1].upper()
                    t = ms(w, 2, 100)
                    if name in ("LT", "RT"):
                        axis = e.ABS_Z if name == "LT" else e.ABS_RZ
                        emit(e.EV_ABS, axis, 255); time.sleep(t); emit(e.EV_ABS, axis, 0)
                    else:
                        emit(e.EV_KEY, BUTTONS[name], 1); time.sleep(t); emit(e.EV_KEY, BUTTONS[name], 0)
                elif w[0] == "stick":
                    ax, ay = (e.ABS_X, e.ABS_Y) if w[1].upper() == "L" else (e.ABS_RX, e.ABS_RY)
                    emit(e.EV_ABS, ax, int(float(w[2]) * 32767)); emit(e.EV_ABS, ay, int(float(w[3]) * 32767))
                    time.sleep(ms(w, 4, 500))
                    emit(e.EV_ABS, ax, 0); emit(e.EV_ABS, ay, 0)
                elif w[0] == "dpad":
                    axis, v = {"UP": (e.ABS_HAT0Y, -1), "DOWN": (e.ABS_HAT0Y, 1), "LEFT": (e.ABS_HAT0X, -1), "RIGHT": (e.ABS_HAT0X, 1)}[w[1].upper()]
                    emit(e.EV_ABS, axis, v); time.sleep(ms(w, 2, 100)); emit(e.EV_ABS, axis, 0)
                elif w[0] == "wait":
                    time.sleep(ms(w, 1, 0))
                done.write(f"ok {line.strip()}\n")
            except SystemExit:
                raise
            except Exception as x:
                done.write(f"err {x}: {line.strip()}\n")
