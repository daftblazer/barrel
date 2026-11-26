#!/bin/bash
# Runs a command inside a headless gamescope, muted, so no window or sound reaches the desktop.
#   tools/headless.sh [-W WIDTH -H HEIGHT] COMMAND...
# Everything the command starts (Barrel windows, the recomp, a whole runchecks batch) gets
# gamescope's own Xwayland display; the host's Wayland socket is hidden from it.
set -u
w=1920 h=1080
while [ $# -gt 0 ]; do
    case $1 in
        -W) w=$2; shift 2 ;;
        -H) h=$2; shift 2 ;;
        *) break ;;
    esac
done
[ $# -gt 0 ] || { echo "usage: headless.sh [-W W -H H] COMMAND..." >&2; exit 2; }
exec env -u WAYLAND_DISPLAY SDL_AUDIO_DRIVER=dummy \
    gamescope --backend headless -W "$w" -H "$h" -w "$w" -h "$h" -- \
    env -u WAYLAND_DISPLAY SDL_VIDEODRIVER=x11 SDL_AUDIO_DRIVER=dummy "$@"
