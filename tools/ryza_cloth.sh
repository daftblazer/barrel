#!/bin/sh
# Ryza's cloth against the game's, over ryza-recomp's `walk-rasen` capture (the left stick up from
# game frame 1900 for 120 frames): Barrel walks her from the save for the same frames, traces
# every piece's control points (BARREL_TRACE_CLOTH), and `barrel-parity cloth` compares them with
# the capture's `vCtrl` every 5th frame. Prints a line per piece and the totals
# (docs/parity.md, "Cloth").
#
#   tools/ryza_cloth.sh [FROM-TO/STEP]      default 1880-2100/5
#
# Needs docs/ryza/checks.env (game folder, saves, $OUT) as the checks do, and $WALK_RASEN (the capture's folder). Cloth numbers are in
# game units (centimetres), in each piece's parent joint's space.
. "${CHECKS_ENV:-docs/ryza/checks.env}"
SAVE=$(ls "$LSAVES"/*/AutoSaveData/data.dat | head -1)
RANGE=${1:-1880-2100/5}
CAP_RUN=${CAP_RUN:-tools/capped.sh}
W=${WALK_RASEN:?set WALK_RASEN to the walk-rasen capture folder of ryza-recomp}
IDX=${PARITY_INDEX}
LAST=${RANGE#*-}; LAST=${LAST%/*}
printf '1900 LY=1.0 120\n' > "$OUT/walk-rasen.pad"
rm -f "$OUT/cloth.jsonl"
# Frame 0 of the run is the game's frame 1879 (the first the walk check starts from); the last one drawn is $LAST.
BARREL_PAD_SCRIPT="$OUT/walk-rasen.pad" BARREL_TRACE_CLOTH="$OUT/cloth.jsonl" $CAP_RUN target/release/barrel "$G" s06_t_rasen "$OUT/cloth-walk.png" \
  --size 640x360 --save "$SAVE" --ryza 32520,6180.01,30334.02 --turn -1.0 \
  --walk-from 1879 --frames $((LAST - 1879 + 1)) ${BARREL_ARGS} >/dev/null 2>&1 || exit 1
target/release/barrel-parity cloth "$W/draws" "$RANGE" "$OUT/cloth.jsonl" "$IDX" pc00a
