#!/bin/sh
# Ends an oracle run: every process in the session run.sh started (gamescope, Proton, wine,
# the game), and nothing else.
here=$(cd "$(dirname "$0")/../.." && pwd)
sid=$(cat "$here/work/oracle/session" 2>/dev/null) || exit 0
pkill -TERM -s "$sid" 2>/dev/null
sleep 3
pkill -KILL -s "$sid" 2>/dev/null
rm -f "$here/work/oracle/session"
