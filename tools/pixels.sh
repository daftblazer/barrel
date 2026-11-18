#!/bin/sh
# Scores one view of the Ryza pixel board: renders Barrel's frame of the view with its stages
# dumped and compares them with the game's (docs/parity.md, "The pixel board").
#   tools/pixels.sh gate|turn|left [barrel arguments...]
# $PIXELS_GAME is the game's install folder; $PIXELS_KEY the answer key's scenarios folder
# (default ../ryza-recomp/work/scenarios); $PIXELS_OUT where the stages, the frame and the
# comparison pictures go (default a temporary folder). Run from the repository's root after a
# release build. Prints the comparison; its last line is the board's score.
set -e
view=${1:?gate, turn or left}
shift
key=${PIXELS_KEY:-../ryza-recomp/work/scenarios}
out=${PIXELS_OUT:-$(mktemp -d)}
bin=${CARGO_TARGET_DIR:-target}/release
at="--ryza 32520,6180,30334.02 --turn -1.0"
case $view in
gate)
    dump=$key/pixels-rasen/pixels/f2550 map=1911-1911
    clocks="BARREL_SKY_CLOCK=13.7166 BARREL_DOME_CLOCK=28.40 BARREL_PATTERN_PHASE=-0.07133281 BARREL_CLOUD_THRESHOLD=0.6538959"
    camera="--look-at 32509.512,6224.147,30934.779 --yaw 180.99981 $at --motion-time 1.15" ;;
turn)
    dump=$key/pixels-turn/pixels/f2150 map=1870-1870
    clocks="BARREL_SKY_CLOCK=7.0500 BARREL_DOME_CLOCK=21.73 BARREL_PATTERN_PHASE=-0.10466671 BARREL_CLOUD_THRESHOLD=0.64210135"
    camera="--look-at 31721.86,6238.42,29814.96 --yaw 306.32707 --ryza 32200.55,6193.3,30166.94 --turn -126.33 --motion-time 2.2" ;;
left)
    # The post in front of the camera is hidden as the game hides it (docs/ryza/open.md).
    dump=$key/pixels-left/pixels/f2150 map=1643-1643
    clocks="BARREL_SKY_CLOCK=7.0500 BARREL_DOME_CLOCK=21.75 BARREL_PATTERN_PHASE=-0.10466671 BARREL_CLOUD_THRESHOLD=0.64210135"
    camera="--look-at 33029.292,6225.087,30640.031 --yaw 121.0000952 $at --motion-time 2.043 --hide obj-A21-S06-POST" ;;
*) echo "no view $view" >&2; exit 2 ;;
esac
rm -rf "${out:?}/$view" "${out:?}/$view-pictures"
# shellcheck disable=SC2086
env $clocks BARREL_CLOUD_MAP="$dump/${map}_rt1024x1024.27#4.png" BARREL_DUMP_STAGES="$out/$view" SDL_AUDIO_DRIVER=dummy \
    tools/capped.sh "$bin/barrel" "${PIXELS_GAME:?the game's install folder}" s06_t_rasen "$out/$view.png" --size 1920x1080 \
    --distance 1000 --pitch 8.0 --fov 45 $camera --done MM00_010 "$@" >"$out/$view.log" 2>&1 || { tail -5 "$out/$view.log" >&2; exit 1; }
"$bin/barrel-parity" pixels "docs/ryza/views/$view.txt" "$dump" "$out/$view" "$out/$view-pictures"
