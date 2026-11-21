#!/bin/sh
# Pictures of Ryza's head and hair on the `walk-rasen` run (tools/ryza_cloth.sh): Barrel walks her
# from the save with the same pad script and writes the frame asked for, the camera held on her
# head at a fixed angle (`--look-at` moves with her).
#
#   tools/ryza_cloth_view.sh NAME FRAME [YAW [PITCH [DISTANCE]]]    -> $OUT/NAME-FRAME.png
#
# Needs docs/ryza/checks.env as the checks do. FRAME is the game's frame (1879 on is the walk).
. "${CHECKS_ENV:-docs/ryza/checks.env}"
SAVE=$(ls "$LSAVES"/*/AutoSaveData/data.dat | head -1)
NAME=$1; FRAME=$2; YAW=${3:-150}; PITCH=${4:-10}; DIST=${5:-130}
# Her head is about 150 above her feet at the start (game units, centimetres).
printf '1900 LY=1.0 120\n' > "$OUT/walk-rasen.pad"
BARREL_PAD_SCRIPT="$OUT/walk-rasen.pad" tools/capped.sh target/release/barrel "$G" s06_t_rasen "$OUT/$NAME-$FRAME.png" \
  --size 800x800 --save "$SAVE" --ryza 32520,6180.01,30334.02 --turn -1.0 \
  --look-at 32520,6330,30334.02 --yaw "$YAW" --pitch "$PITCH" --distance "$DIST" --fov 30 \
  --walk-from 1879 --frames $((FRAME - 1879 + 1)) ${BARREL_ARGS} >/dev/null 2>&1
