#!/bin/sh
# Records the oracle run's screen (gamescope's PipeWire stream) losslessly to OUT.mkv for
# SECONDS (H.264 at quantizer 0, 4:4:4, and the sound as FLAC with ORACLE_AUDIO=1; each frame stamped as it arrives, so dropped or doubled
# frames show in the timestamps). Start the game with ORACLE_HZ=60 for 60 frames a second.
#   tools/oracle/record.sh OUT.mkv SECONDS
set -eu
out=$1
seconds=$2
# The stream of this run's gamescope (run.sh's session pid), not another gamescope that may be
# running on the machine: a node's client carries the process id.
here=$(cd "$(dirname "$0")/../.." && pwd)
session=$(cat "$here/work/oracle/session" 2>/dev/null || true)
[ -n "$session" ] || { echo "record.sh: no run (work/oracle/session)" >&2; exit 1; }
node=$(pw-dump 2>/dev/null | python3 -I -c '
import json, sys
objs = json.load(sys.stdin)
pid = sys.argv[1]
clients = {o["id"] for o in objs if o.get("type") == "PipeWire:Interface:Client"
           and str(((o.get("info") or {}).get("props") or {}).get("application.process.id")) == pid}
for o in objs:
    p = (o.get("info") or {}).get("props") or {}
    if o.get("type") == "PipeWire:Interface:Node" and p.get("node.name") == "gamescope" and p.get("client.id") in clients:
        print(o["id"]); break' "$session")
[ -n "$node" ] || { echo "record.sh: no PipeWire stream from this run's gamescope (pid $session)" >&2; exit 1; }
# With the run's null sink (ORACLE_AUDIO=1), its sound too, as FLAC in the same file.
if pactl list short sinks 2>/dev/null | grep -q barrel_oracle; then
    timeout -s INT "$seconds" gst-launch-1.0 -e -q matroskamux name=mux ! filesink location="$out" \
        pipewiresrc path="$node" do-timestamp=true always-copy=true ! videoconvert ! video/x-raw,format=Y444 \
        ! x264enc pass=quant quantizer=0 speed-preset=ultrafast ! queue ! mux. \
        pulsesrc device=barrel_oracle.monitor do-timestamp=true ! audioconvert ! audio/x-raw,rate=48000,channels=2 \
        ! flacenc ! queue ! mux. || true
else
    timeout -s INT "$seconds" gst-launch-1.0 -e -q \
        pipewiresrc path="$node" do-timestamp=true always-copy=true ! videoconvert ! video/x-raw,format=Y444 \
        ! x264enc pass=quant quantizer=0 speed-preset=ultrafast ! matroskamux ! filesink location="$out" || true
fi
[ -s "$out" ] || { echo "record.sh: nothing recorded" >&2; exit 1; }
