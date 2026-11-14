#!/bin/sh
# A screenshot of this oracle run's gamescope (display.sh) to OUT.png, shrunk to 640 wide as
# OUT-small.png; fails rather than shoot another gamescope.
#   tools/oracle/shot.sh OUT.png
out=$(realpath -m "$1")
rm -f "$out"
display=$("$(dirname "$0")/display.sh") || exit 1
GAMESCOPE_WAYLAND_DISPLAY=$display gamescopectl screenshot "$out" >/dev/null 2>&1
timeout 10 sh -c "until [ -s '$out' ]; do sleep 0.3; done" && magick "$out" -resize 640x "${out%.png}-small.png"
