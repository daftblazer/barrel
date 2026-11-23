#!/bin/bash
# Ryza's Settings values at work: each case starts the game with a config holding the saved
# values and reads the effect from the run's log.
#   tools/options-effects.sh ENV_FILE
# ENV_FILE is docs/ryza/checks.env's shape (sets $G the game's folder, $LSAVES a saves folder and
# $OUT scratch space). Needs target/release/barrel built. Prints PASS or FAIL per case.
#   camera-horizontal  a field walk with the right stick held right: the camera turns the other way
#                      with the row on Inverted, by the same amount
#   camera-speed       the horizontal speed row at 10 turns it twice as far as the default
#   camera-vertical    the right stick held down: the pitch moves the other way with the row inverted
#   volume-bgm         the title's log names the music's gain: 0 at volume 0, 1 at the default
#   volume-voice       the voice's gain at 10 is above the default's
# (The confirm button's swap is a unit test, `preferences::tests`: the offscreen title is not
# driven by the pad's buttons.)
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
. "${1:?usage: options-effects.sh ENV_FILE}"
out=${OUT:?the env file sets OUT}/options-effects
mkdir -p "$out"
export SDL_AUDIO_DRIVER=dummy
status=0
bin="$here/target/release/barrel"
save=$(ls "$LSAVES"/*/AutoSaveData/data.dat | head -1)

# walk NAME CONFIG PAD: a field run of 60 frames, the camera traced to $out/NAME.cam (last line printed).
walk() {
    printf '%s\n' "$2" >"$out/$1.config"
    printf '%s\n' "$3" >"$out/$1.pad"
    rm -f "$out/$1.cam"
    BARREL_CONFIG="$out/$1.config" BARREL_PAD_SCRIPT="$out/$1.pad" BARREL_TRACE_CAMERA="$out/$1.cam" \
        "$here/tools/capped.sh" "$bin" "$G" s06_t_rasen "$out/$1.png" --size 320x180 --save "$save" --ryza 32520,6180.01,30334.02 --turn -1.0 \
        --walk-from 1879 --frames 60 >"$out/$1.log" 2>&1
    tail -1 "$out/$1.cam"
}
# value LINE KEY: the number after KEY in LINE.
value() { awk -v k="$2" '{ for (i = 1; i < NF; i++) if ($i == k) print $(i + 1) }' <<<"$1"; }
check() { # NAME OK DETAIL
    if [ "$2" = 1 ]; then echo "PASS $1: $3"; else echo "FAIL $1: $3"; status=1; fi
}

right='1890 RX=1.0 60'
down='1890 RY=-1.0 60'
base=$(walk base "" "$right")
flip=$(walk flip "option_camera_horizontal 1" "$right")
fast=$(walk fast "option_camera_horizontal_speed 10" "$right")
# (the turns are the yaw's change from where the camera started, the trace's first line)
y_start=$(head -1 "$out/base.cam" | awk '{ print $3 }')
y0=$(value "$base" yaw); y1=$(value "$flip" yaw); y2=$(value "$fast" yaw)
y0=$(awk -v a="$y0" -v s="$y_start" 'BEGIN { print a - s }'); y1=$(awk -v a="$y1" -v s="$y_start" 'BEGIN { print a - s }'); y2=$(awk -v a="$y2" -v s="$y_start" 'BEGIN { print a - s }')
check camera-horizontal "$(awk -v a="$y0" -v b="$y1" 'BEGIN { print (a * b < 0 && a + b < 0.01 && a + b > -0.01) ? 1 : 0 }')" "turn $y0 degrees by default, $y1 inverted"
check camera-speed "$(awk -v a="$y0" -v b="$y2" 'BEGIN { print (a != 0 && b / a > 1.99 && b / a < 2.01) ? 1 : 0 }')" "turn $y0 degrees by default, $y2 at speed 10"
vbase=$(walk vbase "" "$down")
vflip=$(walk vflip "option_camera_vertical 1" "$down")
p0=$(value "$vbase" pitch); p1=$(value "$vflip" pitch)
# (the pitch is compared with where the camera started, the trace's first line)
s0=$(head -1 "$out/vbase.cam" | awk '{ print $5 }')
check camera-vertical "$(awk -v s="$s0" -v a="$p0" -v b="$p1" 'BEGIN { print ((a - s) * (b - s) < 0) ? 1 : 0 }')" "pitch $s0 at the start, $p0 by default, $p1 inverted"

title() { # NAME CONFIG: the title's first frames; the log's volumes line
    printf '%s\n' "$2" >"$out/$1.config"
    BARREL_CONFIG="$out/$1.config" "$here/tools/capped.sh" "$bin" "$G" s01_i_attic "$out/$1.png" --size 320x180 --title --title-saves --saves "$LSAVES" --frames 30 >"$out/$1.log" 2>&1
    grep '^sound: volumes' "$out/$1.log" | tail -1
}
v0=$(title vol-default "")
v1=$(title vol-bgm0 "option_volume_bgm 0")
v2=$(title vol-voice10 "option_volume_voice 10")
check volume-bgm "$([ "$(value "$v1" bgm)" = 0.000 ] && [ "$(value "$v0" bgm)" = 1.000 ] && [ "$(value "$v1" sfx)" = 1.000 ] && echo 1 || echo 0)" "$v1"
check volume-voice "$(awk -v a="$(value "$v2" voice)" -v b="$(value "$v0" voice)" 'BEGIN { print (a > 1.9 * b) ? 1 : 0 }')" "$v2"
exit $status
