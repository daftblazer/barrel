#!/bin/sh
# Sends commands to the oracle pad (tools/oracle/pad.py) and waits until they are done.
#   tools/oracle/pad.sh "tap A" "wait 2000" "dpad LEFT" ...
here=$(cd "$(dirname "$0")/../.." && pwd)
for c in "$@"; do
    n=$(wc -l <"$here/work/oracle/paddone" 2>/dev/null || echo 0)
    echo "$c" >"$here/work/oracle/padcmd"
    timeout 60 sh -c "until [ \$(wc -l <'$here/work/oracle/paddone') -gt $n ]; do sleep 0.05; done"
done
