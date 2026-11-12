#!/bin/sh
# A screenshot of the oracle run (gamescope-0) to OUT.png, shrunk to 640 wide as OUT-small.png.
#   tools/oracle/shot.sh OUT.png
out=$(realpath -m "$1")
rm -f "$out"
GAMESCOPE_WAYLAND_DISPLAY=gamescope-0 gamescopectl screenshot "$out" >/dev/null 2>&1
timeout 10 sh -c "until [ -s '$out' ]; do sleep 0.3; done" && magick "$out" -resize 640x "${out%.png}-small.png"
