#!/bin/sh
# Ends an oracle run: every process in the session run.sh started (gamescope, Proton, wine,
# the game), and nothing else.
here=$(cd "$(dirname "$0")/../.." && pwd)
sid=$(cat "$here/work/oracle/session" 2>/dev/null) || exit 0
pkill -TERM -s "$sid" 2>/dev/null
sleep 3
pkill -KILL -s "$sid" 2>/dev/null
# Wine's device helpers (winedevice.exe) leave the session, so also end every process whose
# environment names this run's prefix.
prefix=$here/work/oracle-prefix
for d in /proc/[0-9]*; do
    grep -qz "^STEAM_COMPAT_DATA_PATH=$prefix\$" "$d/environ" 2>/dev/null && kill -KILL "${d#/proc/}" 2>/dev/null
done
rm -f "$here/work/oracle/session"
# The run's null sink (ORACLE_AUDIO), if it made one.
if [ -s "$here/work/oracle/sink-module" ]; then
    pactl unload-module "$(cat "$here/work/oracle/sink-module")" 2>/dev/null
    rm -f "$here/work/oracle/sink-module"
fi
