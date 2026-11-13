#!/bin/sh
# Starts Escha & Logy DX for an oracle run: headless gamescope (1280x720, 30 Hz or ORACLE_HZ), its own
# Proton prefix under work/ (never the player's), no audio drivers (or, with ORACLE_AUDIO=1, sound to a
# null sink nobody hears). Prints gamescope's pid.
#   tools/oracle/run.sh INSTALL_DIR PROTON_DIR
# Stop it with tools/oracle/stop.sh.
set -eu
install=$1
proton=$2
here=$(cd "$(dirname "$0")/../.." && pwd)
prefix=$here/work/oracle-prefix
mkdir -p "$prefix" "$here/work/oracle"
export STEAM_COMPAT_DATA_PATH=$prefix
export STEAM_COMPAT_CLIENT_INSTALL_PATH=${STEAM_COMPAT_CLIENT_INSTALL_PATH:-$HOME/.local/share/Steam}
export SteamAppId=1152310 SteamGameId=1152310
export PROTON_LOG=${PROTON_LOG:-1} PROTON_LOG_DIR=$here/work/oracle
# Silent but with a working sound device (the game crashes without one: its effect player
# reads a sound bank that is never made): Wine's ALSA driver on ALSA's null device, set for
# this process only through its own config file; the PulseAudio driver off.
printf 'pcm.!default { type null }\nctl.!default { type hw card 0 }\n' >"$here/work/oracle/asound.conf"
export ALSA_CONFIG_PATH=/usr/share/alsa/alsa.conf:$here/work/oracle/asound.conf
# ORACLE_AUDIO=1: the game's sound goes to a null sink of its own (`barrel_oracle`, connected
# to no device: nothing is heard) for record.sh to take; the ALSA null device stays the fallback.
if [ -n "${ORACLE_AUDIO:-}" ]; then
    pactl list short sinks | grep -q barrel_oracle ||
        pactl load-module module-null-sink sink_name=barrel_oracle sink_properties=device.description=BarrelOracle >"$here/work/oracle/sink-module"
    export PULSE_SINK=barrel_oracle
else
    export WINEDLLOVERRIDES="winepulse.drv=d"
fi
cd "$install"
# Its own session, so stop.sh ends exactly this run's processes (not a game the player has open).
setsid gamescope --backend headless -W 1280 -H 720 -r "${ORACLE_HZ:-30}" -- \
    "$proton/proton" run ./Atelier_Escha_and_Logy_EN.exe \
    >"$here/work/oracle/game.log" 2>&1 &
echo $! >"$here/work/oracle/session"
echo $!
