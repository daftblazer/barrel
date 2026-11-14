#!/bin/bash
# tools/package.sh <x86_64|aarch64> <out folder>: build Barrel and make its Kintsugi engine folder
# (`run`, the stripped barrel-escha and, for aarch64, the arm64 SDL3 it links in lib/).
#
# aarch64 is cross-built: set BARREL_AARCH64_SYSROOT (an arm64 root filesystem with the Vulkan
# loader and libwayland-client) and BARREL_AARCH64_DEPS (a prefix with arm64 SDL3: lib/libSDL3.so.0,
# lib/pkgconfig/sdl3.pc). The x86_64 build uses the system's SDL3.
set -euo pipefail
cd "$(dirname "$0")/.."
ARCH=${1:?usage: package.sh <x86_64|aarch64> <out folder>}
OUT=${2:?usage: package.sh <x86_64|aarch64> <out folder>}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
case "$ARCH" in
x86_64)
    tools/capped.sh cargo build --offline --release -p barrel-escha
    BIN=target/release/barrel-escha ;;
aarch64)
    : "${BARREL_AARCH64_SYSROOT:?set BARREL_AARCH64_SYSROOT (arm64 root filesystem)}"
    : "${BARREL_AARCH64_DEPS:?set BARREL_AARCH64_DEPS (prefix with arm64 SDL3)}"
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$PWD/tools/aarch64-cc.sh"
    export PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=
    export PKG_CONFIG_LIBDIR="$BARREL_AARCH64_DEPS/lib/pkgconfig:$BARREL_AARCH64_SYSROOT/usr/lib/aarch64-linux-gnu/pkgconfig:$BARREL_AARCH64_SYSROOT/usr/share/pkgconfig"
    tools/capped.sh cargo build --offline --release --target aarch64-unknown-linux-gnu -p barrel-escha
    BIN=target/aarch64-unknown-linux-gnu/release/barrel-escha ;;
*) echo "unknown architecture $ARCH" >&2; exit 1 ;;
esac
rm -rf "$OUT" && mkdir -p "$OUT"
llvm-strip --strip-debug -o "$OUT/barrel-escha" "$BIN"
if [ "$ARCH" = aarch64 ]; then
    mkdir -p "$OUT/lib"
    cp -L "$BARREL_AARCH64_DEPS/lib/libSDL3.so.0" "$OUT/lib/"
fi
cat > "$OUT/run" <<'RUN'
#!/bin/bash
# Kintsugi engine for Atelier Escha & Logy: Alchemists of the Dusk Sky DX (Steam app 1152310):
# Barrel, starting at its title screen. Kintsugi starts it as
#   run <game folder> <the game's own command line, ignored>
# Graphics: ~/.config/barrel/graphics.conf (the in-game Options page writes it; on an aarch64
# handheld the first start is fullscreen with the handheld preset). Saves: ~/.local/share/barrel/saves.
# Log: ~/.local/state/barrel/barrel.log.
here=$(cd "$(dirname "$0")" && pwd)
game=${1:-${KINTSUGI_GAME_DIR:?no game folder}}
unset LD_PRELOAD   # Steam's overlay libraries are for its own processes
[ -d "$here/lib" ] && export LD_LIBRARY_PATH="$here/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
if [ "$(uname -m)" = aarch64 ]; then
    export BARREL_DEFAULT_PRESET=${BARREL_DEFAULT_PRESET:-handheld} BARREL_DEFAULT_FULLSCREEN=${BARREL_DEFAULT_FULLSCREEN:-1}
fi
state=${XDG_STATE_HOME:-$HOME/.local/state}/barrel
mkdir -p "$state"
cd "$here"
exec ./barrel-escha --field auto /dev/null --title "$game" --window 0 >"$state/barrel.log" 2>&1
RUN
chmod +x "$OUT/run"
echo "engine folder: $OUT"
