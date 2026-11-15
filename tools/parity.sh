#!/bin/sh
# Renders one Barrel frame offscreen with the 2D draw trace and diffs it against a game capture.
#   tools/parity.sh <capture dir> <game frame> <barrel arguments...>
# The capture is a scenario's draws/ folder (escha-recomp tools/scenario.sh); the texture index
# is $PARITY_INDEX (default ../escha-recomp/work/traces/textures.tsv). Barrel's frame is
# written to $PARITY_OUT (default a temporary folder) as frame.png and trace.jsonl.
# With `--frames N` among the arguments the diff takes Barrel's last frame (N - 1), else its
# first: a screen that needs time to come up (a battle's command row) is compared where it stands.
# PARITY_ALIGN=a-b/step (the game's frames, e.g. 2200-2260/20) with `--frames N` aligns time
# instead: every captured game frame of the range is diffed against each of Barrel's frames
# 0..N-1 and the last line gives the one offset (game frame - Barrel frame) with the fewest
# total mismatches, for screens with animation (a cycling glow shows another cut by time).
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
[ "$last" -gt 0 ] && [ -z "$PARITY_ALIGN" ] && set -- "$@" --save-every $((last + 1))
index=${PARITY_INDEX:-$here/../escha-recomp/work/traces/textures.tsv}
out=${PARITY_OUT:-$(mktemp -d)}
mkdir -p "$out"
rm -f "$out/trace.jsonl"
SDL_AUDIO_DRIVER=dummy BARREL_TRACE_DRAWS=$out/trace.jsonl timeout 300 "$here/tools/capped.sh" "$here/target/release/barrel" "$@" > "$out/run.log" 2>&1 || { echo "parity: barrel failed (see $out/run.log)" >&2; exit 2; }
if [ -n "$PARITY_ALIGN" ]; then
    [ "$last" -gt 0 ] || { echo "parity: PARITY_ALIGN needs --frames N" >&2; exit 2; }
    exec "$here/target/release/barrel-parity" align "$capture" "$PARITY_ALIGN" "$out/trace.jsonl" "0-$last" "$index"
fi
"$here/target/release/barrel-parity" diff "$capture" "$frame" "$out/trace.jsonl" "$last" "$index"
