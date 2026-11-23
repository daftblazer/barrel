#!/bin/bash
# Live check of Ryza's display rows: what a player applies with Yes on the Settings screen must
# reach the open window, with no restart.
#   tools/live-display.sh ENV_FILE [CASE...]
# ENV_FILE is docs/ryza/checks.env's shape: it sets $G (the game's folder), $LSAVES (a saves
# folder for the title's list) and $OUT (scratch space). Cases (default: all): resolution (the
# Screen Resolution row one step down), borderless (Window Settings to Borderless), windowed (from
# a fullscreen start, Window Settings back to Window), vsync (Vertical Sync off with the limit
# Unlimited), limit30 (Frame Rate Limit 30 with vsync off, so the limit alone paces).
#
# This opens a real window on the desktop (about 12 seconds a case). A window run takes its keys
# from the pad, so a scripted pad (BARREL_PAD_SCRIPT) opens Settings on the title, goes to the
# Graphics tab, changes the rows, leaves with B and answers Yes. The run then logs, 120 frames
# after the apply, `display: inner WxH, fullscreen B, swapchain WxH, present mode M, mean frame X ms`.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
. "${1:?usage: live-display.sh ENV_FILE [CASE...]}"
shift
out=${OUT:?the env file sets OUT}/live-display
mkdir -p "$out"
export SDL_AUDIO_DRIVER=dummy
BARREL_EVENT_DRAWS=$(awk 'BEGIN { for (i = 0; i < 64; i++) printf "%s%.3f", (i ? "," : ""), (i * 0.618034) % 1 }')
export BARREL_EVENT_DRAWS
field=s19_l_highway_03

# Graphics tab rows: 0 Screen Resolution, 1 Window Settings, 5 Frame Rate Limit, 6 Vertical Sync.
# Case table: name | start config (\n separated) | changes "row:direction:count" | expectation.
# The expectation is checked against the probe line: a regex on it, or `mean=LO-HI` (ms).
declare -A start=(
    [resolution]=""
    [borderless]=""
    [windowed]='window 1920x1080\nfullscreen on\noption_window_mode 1'
    [vsync]=""
    [limit30]=""
)
declare -A changes=(
    [resolution]="0:-1:1"
    [borderless]="1:1:1"
    [windowed]="1:-1:1"
    [vsync]="5:1:3 6:1:1"
    [limit30]="5:-1:1 6:1:1"
)
declare -A want=(
    [resolution]="inner 1600x900, fullscreen false, swapchain 1600x900"
    [borderless]="fullscreen true, swapchain ([0-9]+)x([0-9]+),"
    [windowed]="inner 1280x720, fullscreen false, swapchain 1280x720"
    [vsync]="present mode (MAILBOX|IMMEDIATE),"
    [limit30]="mean frame (3[0-6]\.[0-9]+) ms"
)

# Writes the pad script: Settings opened on the title, the Graphics tab, the changes, B, Yes (the
# box starts on No: one down) and A. Prints the last frame used.
pad_script() { # file changes
    local f=857 cur=0 ch row dir n i file=$1
    : >"$1"
    press() { echo "$f $1 6" >>"$file"; f=$((f + ${2:-20})); }
    press DOWN 30
    press A 70
    f=957
    press RB 60
    press RB 60
    for ch in $2; do
        IFS=: read -r row dir n <<<"$ch"
        while ((cur < row)); do press DOWN; cur=$((cur + 1)); done
        for ((i = 0; i < n; i++)); do press "$([ "$dir" = 1 ] && echo RIGHT || echo LEFT)"; done
    done
    press B 60
    press DOWN 40
    press A 40
    echo $f
}

status=0
for name in "${@:-resolution borderless windowed vsync limit30}"; do
    for c in $name; do
        [ -n "${changes[$c]:-}" ] || { echo "FAIL $c: unknown case"; status=1; continue; }
        d=$out/$c
        rm -rf "$d"
        mkdir -p "$d"
        printf "${start[$c]}" >"$d/config.toml"
        [ -s "$d/config.toml" ] && echo >>"$d/config.toml"
        last=$(pad_script "$d/pad" "${changes[$c]}")
        # (the apply, then 120 frames at the limit and a margin, in the run's 60 Hz clock)
        frames=$((last + 600))
        BARREL_PAD_SCRIPT=$d/pad BARREL_CONFIG=$d/config.toml timeout 90 "$here/tools/capped.sh" "$here/target/release/barrel" "$G" $field "$d/x.png" --window --title --title-at 0 --title-press 692 --title-saves --saves "$LSAVES" --frames $frames >"$d/log" 2>&1
        probe=$(grep '^display: inner' "$d/log" | tail -1)
        if [ -z "$probe" ]; then echo "FAIL $c: no display line after the apply ($d/log)"; status=1; continue; fi
        if [[ $probe =~ ${want[$c]} ]]; then echo "PASS $c: $probe"; else echo "FAIL $c: $probe (wanted ${want[$c]})"; status=1; fi
    done
done
exit $status
