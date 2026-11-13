#!/bin/sh
# Writes the bytes HEX (hex digits) to ADDR (hex) of the oracle run's memory through the host.
#   tools/oracle/poke.sh ADDR HEX
here=$(cd "$(dirname "$0")/../.." && pwd)
n=$(wc -l <"$here/work/oracle/done" 2>/dev/null || echo 0)
echo "write $1 $2" >"$here/work/oracle/cmd"
timeout 10 sh -c "until [ \$(wc -l <'$here/work/oracle/done') -gt $n ]; do sleep 0.1; done"
tail -1 "$here/work/oracle/done"
