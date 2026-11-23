#!/bin/bash
# Live-versus-cold check of Ryza's Settings screen: the graphics a player applies with Yes must
# draw exactly what a fresh start with the saved config draws.
#   tools/live-settings.sh ENV_FILE [CASE...]
# ENV_FILE is docs/ryza/checks.env's shape: it sets $G (the game's folder), $LSAVES (a saves
# folder for the title's list) and $OUT (scratch space, big files). Cases (default: all) are
# same (the settings applied again, unchanged, mid-run: BARREL_REAPPLY_AT), scale75, msaa4,
# low, fsr1, nativeaa (fsr3 and taa, the temporal ones, are named too).
#
# Run A starts from an empty config on the title's 3D backdrop, opens Settings, goes to the
# Graphics tab, changes the rows, leaves with B, answers Yes and writes a picture some frames
# after the title is back. Run B starts with the config A saved and presses the same keys at
# the same frames (each change taken one step further and back, so the dialog and its timing match; the
# saved values are the same ones) and writes the same frame. The two pictures are compared
# (PSNR; identical is "inf"). Spatial cases must be identical; temporal ones (TAA, FSR 3.1) start
# their history at different moments and need $TEMPORAL_DB (default 40) dB.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
. "${1:?usage: live-settings.sh ENV_FILE [CASE...]}"
shift
out=${OUT:?the env file sets OUT}/live
mkdir -p "$out"
export SDL_AUDIO_DRIVER=dummy
# The title's event draws its waits and actions from the clock, as the game does; both runs
# take the same draws instead.
BARREL_EVENT_DRAWS=$(awk 'BEGIN { for (i = 0; i < 64; i++) printf "%s%.3f", (i ? "," : ""), (i * 0.618034) % 1 }')
export BARREL_EVENT_DRAWS
size=${SIZE:-960x540}
temporal_db=${TEMPORAL_DB:-40}
field=s19_l_highway_03
tab=${TAB_PRESSES:-2}   # RB presses from the first tab to the Graphics tab

# Case table: name | changes ("row:direction:count" separated by spaces) | temporal (0/1).
# Rows of the Graphics tab: 2 Render Scale, 3 Upscaler, 7 Overall Quality, 12 Anti-Aliasing.
declare -A changes=(
    [scale75]="2:-1:2"
    [msaa4]="12:1:%AA_MSAA4%"
    [low]="7:-1:2"
    [fsr1]="2:-1:2 3:1:1"
    [nativeaa]="12:1:%AA_NATIVE%"
    [taa]="12:1:%AA_TAA%"
    [fsr3]="2:-1:2 3:1:4"
)
declare -A temporal=([taa]=1 [fsr3]=1 [nativeaa]=1)
# Anti-Aliasing steps from the default (the game's FXAA) to a choice: Off, FXAA, TAA, MSAA 2x/4x/8x,
# native AA.
aa_steps() { case $1 in %AA_TAA%) echo 1 ;; %AA_MSAA4%) echo 3 ;; %AA_NATIVE%) echo 5 ;; *) echo "$1" ;; esac; }

# Builds the key list: prints "KEYS LAST" for the changes with `$2` = a (apply) or b (cold).
keys_for() {
    local list=$1 mode=$2 f=1097 cur=0 k="857:down:6,887:choose" i
    for ((i = 0; i < tab; i++)); do k+=",$((957 + 60 * i)):rb"; done
    f=$((957 + 60 * (tab - 1) + 80))
    local ch
    for ch in $list; do
        IFS=: read -r row dir n <<<"$ch"
        n=$(aa_steps "$n")
        while ((cur < row)); do k+=",$f:ldown:6"; f=$((f + 20)); cur=$((cur + 1)); done
        while ((cur > row)); do k+=",$f:lup:6"; f=$((f + 20)); cur=$((cur - 1)); done
        local step opp=$((-dir))
        if [ "$mode" = a ]; then
            for ((i = 0; i < n; i++)); do k+=",$f:$([ "$dir" = 1 ] && echo right || echo left)"; f=$((f + 20)); done
            f=$((f + 20 * n)) # (B undoes and redoes: the same time)
        else
            # On past and back (back first would undo more: a scale of 100% clears the upscaler).
            for ((i = 0; i < n; i++)); do k+=",$f:$([ "$dir" = 1 ] && echo right || echo left)"; f=$((f + 20)); done
            for ((i = 0; i < n; i++)); do k+=",$f:$([ "$opp" = 1 ] && echo right || echo left)"; f=$((f + 20)); done
        fi
    done
    # B leaves, Yes (the box starts on No: one down) and A.
    k+=",$f:back"; f=$((f + 60))
    k+=",$f:ldown:6"; f=$((f + 40))
    k+=",$f:ok"
    echo "$k $f"
}

run() { # name config-file keys last-key-frame picture
    local pic=$5 last=$4
    local frames=$((last + 163 + 100 + 1)) # (the screen is left 163 frames after Yes, a hundred more)
    BARREL_DUMP_STAGES=${DUMP:+$pic.stages} BARREL_CONFIG=$2 "$here/tools/capped.sh" "$here/target/release/barrel" "$G" $field "$pic" --size "$size" --title --title-at 0 --title-press 692 --title-saves --saves "$LSAVES" --title-keys "$3" --frames $frames >"$pic.log" 2>&1
}

status=0
for c in "${@:-same scale75 msaa4 low fsr1 nativeaa}"; do
    for name in $c; do
        if [ "$name" = same ]; then
            # The same settings applied again mid-run (BARREL_REAPPLY_AT) against a plain run.
            d=$out/same
            mkdir -p "$d"
            BARREL_CONFIG=/dev/null "$here/tools/capped.sh" "$here/target/release/barrel" "$G" $field "$d/plain.png" --size "$size" --frames 60 >"$d/plain.log" 2>&1
            BARREL_REAPPLY_AT=30 BARREL_CONFIG=/dev/null "$here/tools/capped.sh" "$here/target/release/barrel" "$G" $field "$d/again.png" --size "$size" --frames 60 >"$d/again.log" 2>&1
            grep -q "applied again" "$d/again.log" || { echo "FAIL same: nothing was applied again"; status=1; continue; }
            ae=$(magick compare -metric AE "$d/plain.png" "$d/again.png" null: 2>&1 | head -1)
            if [ "${ae%% *}" = 0 ]; then echo "PASS same: applied again vs plain, 0 pixels differ"; else echo "FAIL same: applied again vs plain, $ae pixels differ"; status=1; fi
            continue
        fi
        [ -n "${changes[$name]:-}" ] || { echo "FAIL $name: unknown case"; status=1; continue; }
        d=$out/$name
        rm -rf "$d"
        mkdir -p "$d"
        : >"$d/a.toml"
        read -r ka last <<<"$(keys_for "${changes[$name]}" a)"
        run "$name" "$d/a.toml" "$ka" "$last" "$d/a.png" || { echo "FAIL $name: run A failed ($d/a.png.log)"; status=1; continue; }
        grep -q "applied live" "$d/a.png.log" || { echo "FAIL $name: nothing was applied live (no change saved?)"; status=1; continue; }
        cp "$d/a.toml" "$d/b.toml"
        read -r kb last <<<"$(keys_for "${changes[$name]}" b)"
        run "$name" "$d/b.toml" "$kb" "$last" "$d/b.png" || { echo "FAIL $name: run B failed ($d/b.png.log)"; status=1; continue; }
        psnr=$(magick compare -metric PSNR "$d/a.png" "$d/b.png" null: 2>&1 | head -1)
        ae=$(magick compare -metric AE "$d/a.png" "$d/b.png" null: 2>&1 | head -1)
        want=inf
        if [ -n "${temporal[$name]:-}" ]; then
            ok=$(awk -v p="$psnr" -v t="$temporal_db" 'BEGIN { print (p == "inf" || p + 0 >= t) ? 1 : 0 }')
        else
            ok=$([ "${ae%% *}" = 0 ] && echo 1 || echo 0)
        fi
        if [ "$ok" = 1 ]; then echo "PASS $name: live vs cold PSNR $psnr dB, $ae pixels differ"; else echo "FAIL $name: live vs cold PSNR $psnr dB, $ae pixels differ"; status=1; fi
    done
done
exit $status
