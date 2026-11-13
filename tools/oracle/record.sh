#!/bin/sh
# Records the oracle run's screen (gamescope's PipeWire stream) losslessly to OUT.mkv for
# SECONDS (H.264 at quantizer 0, 4:4:4; each frame stamped as it arrives, so dropped or doubled
# frames show in the timestamps). Start the game with ORACLE_HZ=60 for 60 frames a second.
#   tools/oracle/record.sh OUT.mkv SECONDS
set -eu
out=$1
seconds=$2
node=$(pw-dump 2>/dev/null | python3 -I -c '
import json, sys
for o in json.load(sys.stdin):
    p = (o.get("info") or {}).get("props") or {}
    if o.get("type") == "PipeWire:Interface:Node" and p.get("node.name") == "gamescope":
        print(o["id"]); break')
[ -n "$node" ] || { echo "record.sh: no gamescope PipeWire stream" >&2; exit 1; }
timeout -s INT "$seconds" gst-launch-1.0 -e -q \
    pipewiresrc path="$node" do-timestamp=true always-copy=true ! videoconvert ! video/x-raw,format=Y444 \
    ! x264enc pass=quant quantizer=0 speed-preset=ultrafast ! matroskamux ! filesink location="$out" || true
[ -s "$out" ] || { echo "record.sh: nothing recorded" >&2; exit 1; }
