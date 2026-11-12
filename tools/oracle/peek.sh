#!/bin/sh
# Reads LEN bytes at ADDR (hex) of the game's memory through the oracle host, to OUT.
#   tools/oracle/peek.sh ADDR LEN OUT
here=$(cd "$(dirname "$0")/../.." && pwd)
rm -f "$3"
echo "read $1 $2 $3" >"$here/work/oracle/cmd"
timeout 10 sh -c "until [ -e '$3' ] || tail -1 '$here/work/oracle/done' | grep -q '^err.*$3'; do sleep 0.1; done"
[ -e "$3" ] || { tail -1 "$here/work/oracle/done" >&2; exit 1; }
