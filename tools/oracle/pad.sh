#!/bin/sh
# Sends commands to the oracle pad (tools/oracle/pad.py) and waits until they are done.
#   tools/oracle/pad.sh "tap A" "wait 2000" "dpad LEFT" ...
here=$(cd "$(dirname "$0")/../.." && pwd)
done_file=$here/work/oracle/paddone
# The pad is a system-wide device: every game reading pads gets its presses. Press only while
# this run's game is up (another project's test game may be running too), or, for a Barrel test
# run, while PAD_TARGET_PID (its pid) is alive.
if [ -n "${PAD_TARGET_PID:-}" ]; then
    kill -0 "$PAD_TARGET_PID" 2>/dev/null || { echo "pad.sh: PAD_TARGET_PID $PAD_TARGET_PID is gone, nothing pressed" >&2; exit 1; }
else
    "$here/tools/oracle/display.sh" >/dev/null || { echo "pad.sh: no oracle run alive, nothing pressed" >&2; exit 1; }
fi
# The pad opens its log of finished commands as it starts; wait for it rather than miscount.
timeout 10 sh -c "until [ -e '$done_file' ]; do sleep 0.1; done" || { echo "pad.sh: the pad is not running" >&2; exit 1; }
for c in "$@"; do
    n=$(wc -l <"$done_file")
    echo "$c" >"$here/work/oracle/padcmd"
    timeout 60 sh -c "until [ \$(wc -l <'$done_file') -gt ${n:-0} ]; do sleep 0.05; done" || { echo "pad.sh: no answer to '$c'" >&2; exit 1; }
done
