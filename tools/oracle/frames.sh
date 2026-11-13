#!/bin/sh
# Extracts every frame of a recording between FROM and TO seconds to DIR/f_NNNN.png, and
# prints each frame's timestamp (DIR/times.txt), so doubled or dropped frames can be told
# from the game's own animation.
#   tools/oracle/frames.sh VIDEO.mkv FROM TO DIR
set -eu
mkdir -p "$4"
ffmpeg -v error -ss "$2" -to "$3" -i "$1" -fps_mode passthrough -frame_pts 1 "$4/f_%06d.png"
ffprobe -v error -select_streams v -read_intervals "$2%$3" -show_entries frame=pts_time -of csv=p=0 "$1" >"$4/times.txt"
