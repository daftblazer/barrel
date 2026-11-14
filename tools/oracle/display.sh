#!/bin/sh
# Prints this oracle run's gamescope Wayland display (gamescope-N), read from the environment of a
# process in run.sh's session; fails when the run is not alive. Never guess gamescope-0: another
# gamescope (another project's test run) may hold it.
here=$(cd "$(dirname "$0")/../.." && pwd)
sid=$(cat "$here/work/oracle/session" 2>/dev/null) || { echo "oracle: no run (work/oracle/session)" >&2; exit 1; }
for p in $(pgrep -s "$sid" 2>/dev/null); do
    d=$(tr '\0' '\n' <"/proc/$p/environ" 2>/dev/null | sed -n 's/^GAMESCOPE_WAYLAND_DISPLAY=//p' | head -1)
    [ -n "$d" ] && { echo "$d"; exit 0; }
done
echo "oracle: this run's gamescope is not up (session $sid)" >&2
exit 1
