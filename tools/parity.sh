#!/bin/sh
# Renders one Barrel frame offscreen with the 2D draw trace and diffs it against a game capture.
#   tools/parity.sh <capture dir> <game frame> <barrel-escha arguments...>
# The capture is a scenario's draws/ folder (escha-recomp tools/scenario.sh); the texture index
# is $PARITY_INDEX (default ../escha-recomp/work/traces/textures.tsv). Barrel's frame is
# written to $PARITY_OUT (default a temporary folder) as frame.png and trace.jsonl.
# With `--frames N` among the arguments the diff takes Barrel's last frame (N - 1), else its
# first: a screen that needs time to come up (a battle's command row) is compared where it stands.
# Exit status 0 when nothing differs.
here=$(cd "$(dirname "$0")/.." && pwd)
capture=$1 frame=$2; shift 2
last=0
prev=
for a in "$@"; do
    [ "$prev" = --frames ] && last=$((a - 1))
    prev=$a
done
# Only the last frame is written to a file.
[ "$last" -gt 0 ] && set -- "$@" --save-every $((last + 1))
index=${PARITY_INDEX:-$here/../escha-recomp/work/traces/textures.tsv}
out=${PARITY_OUT:-$(mktemp -d)}
mkdir -p "$out"
rm -f "$out/trace.jsonl"
SDL_AUDIO_DRIVER=dummy BARREL_TRACE_DRAWS=$out/trace.jsonl timeout 300 "$here/tools/capped.sh" "$here/target/release/barrel-escha" "$@" > "$out/run.log" 2>&1 || { echo "parity: barrel-escha failed (see $out/run.log)" >&2; exit 2; }
"$here/target/release/barrel-parity" diff "$capture" "$frame" "$out/trace.jsonl" "$last" "$index"
