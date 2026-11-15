#!/bin/bash
# Packages Barrel's one program, `barrel`, which runs every supported game.
#
#   tools/package.sh <x86_64|aarch64> <out folder> [escha|ryza]
#       The Kintsugi engine folder for one game's Steam app: `run` (starts `barrel` with the game
#       folder Kintsugi passes), the stripped `barrel` and, for aarch64, the arm64 SDL3 in lib/.
#       The game is Escha & Logy DX unless given. tools/deploy.sh installs it.
#
#   tools/package.sh release [out folder]
#       The release packages, in target/release-packages unless given:
#         barrel-<version>-linux-x86_64.tar.gz    always (uses the system's SDL3)
#         barrel-<version>-linux-aarch64.tar.gz   when BARREL_AARCH64_SYSROOT and BARREL_AARCH64_DEPS are set
#         barrel-<version>-windows-x86_64.zip     when a Windows target can be built: the Rust target
#             x86_64-pc-windows-gnu installed, x86_64-w64-mingw32-gcc on the PATH and
#             BARREL_WINDOWS_SDL3 set to a prefix with the Windows SDL3 (bin/SDL3.dll,
#             lib/libSDL3.dll.a, lib/pkgconfig/sdl3.pc); otherwise it is skipped with a line saying why.
#       Each holds `barrel` (`barrel.exe`), SDL3 where it is not the system's, and README.txt.
#
# aarch64 is cross-built: set BARREL_AARCH64_SYSROOT (an arm64 root filesystem with the Vulkan
# loader and libwayland-client) and BARREL_AARCH64_DEPS (a prefix with arm64 SDL3: lib/libSDL3.so.0,
# lib/pkgconfig/sdl3.pc). The arm64 binary finds its SDL3 in lib/ beside it (rpath $ORIGIN/lib).
set -euo pipefail
cd "$(dirname "$0")/.."
USAGE="usage: package.sh <x86_64|aarch64> <out folder> [escha|ryza]  |  package.sh release [out folder]"
MODE=${1:?$USAGE}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}

# build_linux <x86_64|aarch64>: builds `barrel`, prints the path of the binary.
build_linux() {
    case "$1" in
    x86_64)
        tools/capped.sh cargo build --offline --release -p barrel >&2
        echo "${CARGO_TARGET_DIR:-target}/release/barrel" ;;
    aarch64)
        : "${BARREL_AARCH64_SYSROOT:?set BARREL_AARCH64_SYSROOT (arm64 root filesystem)}"
        : "${BARREL_AARCH64_DEPS:?set BARREL_AARCH64_DEPS (prefix with arm64 SDL3)}"
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$PWD/tools/aarch64-cc.sh"
        export PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=
        export PKG_CONFIG_LIBDIR="$BARREL_AARCH64_DEPS/lib/pkgconfig:$BARREL_AARCH64_SYSROOT/usr/lib/aarch64-linux-gnu/pkgconfig:$BARREL_AARCH64_SYSROOT/usr/share/pkgconfig"
        tools/capped.sh cargo build --offline --release --target aarch64-unknown-linux-gnu -p barrel >&2
        echo "${CARGO_TARGET_DIR:-target}/aarch64-unknown-linux-gnu/release/barrel" ;;
    *) echo "unknown architecture $1" >&2; exit 1 ;;
    esac
}

# stage_linux <arch> <binary> <folder>: the binary, stripped, and the arm64 SDL3 beside it.
stage_linux() {
    mkdir -p "$3"
    llvm-strip --strip-debug -o "$3/barrel" "$2"
    if [ "$1" = aarch64 ]; then
        mkdir -p "$3/lib"
        cp -L "$BARREL_AARCH64_DEPS/lib/libSDL3.so.0" "$3/lib/"
    fi
}

if [ "$MODE" != release ]; then
    ARCH=$MODE
    OUT=${2:?$USAGE}
    GAME=${3:-escha}
    case "$GAME" in
    escha) NAME="Atelier Escha & Logy: Alchemists of the Dusk Sky DX"; APP=1152310; LOG=barrel.log ;;
    ryza) NAME="Atelier Ryza: Ever Darkness & the Secret Hideout DX"; APP=3365030; LOG=barrel-ryza.log ;;
    *) echo "unknown game $GAME" >&2; exit 1 ;;
    esac
    BIN=$(build_linux "$ARCH")
    rm -rf "$OUT"
    stage_linux "$ARCH" "$BIN" "$OUT"
    cat > "$OUT/run" <<RUN
#!/bin/bash
# Kintsugi engine for $NAME (Steam app $APP):
# Barrel, which plays the game whose folder Kintsugi passes. Kintsugi starts it as
#   run <game folder> <the game's own command line, ignored>
# Settings and saves are per game: ~/.config/barrel/<game>/ (the in-game Options pages write
# graphics.conf; on an aarch64 handheld the first start is fullscreen with the handheld preset),
# ~/.local/share/barrel/<game>/saves. Log: ~/.local/state/barrel/$LOG.
here=\$(cd "\$(dirname "\$0")" && pwd)
game=\${1:-\${KINTSUGI_GAME_DIR:?no game folder}}
unset LD_PRELOAD   # Steam's overlay libraries are for its own processes
[ -d "\$here/lib" ] && export LD_LIBRARY_PATH="\$here/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
if [ "\$(uname -m)" = aarch64 ]; then
    export BARREL_DEFAULT_PRESET=\${BARREL_DEFAULT_PRESET:-handheld} BARREL_DEFAULT_FULLSCREEN=\${BARREL_DEFAULT_FULLSCREEN:-1}
fi
state=\${XDG_STATE_HOME:-\$HOME/.local/state}/barrel
mkdir -p "\$state"
cd "\$here"
exec ./barrel --game "\$game" >"\$state/$LOG" 2>&1
RUN
    chmod +x "$OUT/run"
    echo "engine folder: $OUT"
    exit 0
fi

# ---- release packages ----
# Released programs carry no build machine's paths: the crate sources' paths (kept for panic
# messages) are rewritten to neutral ones. A separate target folder keeps the everyday build's
# cache (other flags would rebuild it).
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME/.cargo/registry/src=/cargo --remap-path-prefix=$HOME/.cargo/git/checkouts=/cargo-git --remap-path-prefix=$PWD=/barrel --remap-path-prefix=$HOME=/home"
export CARGO_TARGET_DIR="$PWD/target/dist"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DEST=$(realpath -m "${2:-target/release-packages}")
mkdir -p "$DEST"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/barrel-package.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT

readme() {   # readme <platform: linux|windows> <folder>
    local doc=tools/README.$1.txt
    sed "s/@VERSION@/$VERSION/g" "$doc" > "$2/README.txt"
}

for ARCH in x86_64 aarch64; do
    if [ "$ARCH" = aarch64 ] && { [ -z "${BARREL_AARCH64_SYSROOT:-}" ] || [ -z "${BARREL_AARCH64_DEPS:-}" ]; }; then
        echo "skipping linux-aarch64: BARREL_AARCH64_SYSROOT / BARREL_AARCH64_DEPS not set" >&2
        continue
    fi
    BIN=$(build_linux "$ARCH")
    DIR="$STAGE/barrel-$VERSION-linux-$ARCH"
    stage_linux "$ARCH" "$BIN" "$DIR"
    readme linux "$DIR"
    cp LICENSE "$DIR/"
    tar -C "$STAGE" -czf "$DEST/barrel-$VERSION-linux-$ARCH.tar.gz" "barrel-$VERSION-linux-$ARCH"
    echo "package: $DEST/barrel-$VERSION-linux-$ARCH.tar.gz"
done

WIN=x86_64-pc-windows-gnu
if ! rustup target list --installed 2>/dev/null | grep -qx "$WIN"; then
    echo "skipping windows-x86_64: Rust target $WIN is not installed (rustup target add $WIN)" >&2
elif ! command -v x86_64-w64-mingw32-gcc >/dev/null; then
    echo "skipping windows-x86_64: x86_64-w64-mingw32-gcc (the mingw-w64 linker) is not on the PATH" >&2
elif [ ! -f "${BARREL_WINDOWS_SDL3:-/nonexistent}/bin/SDL3.dll" ]; then
    echo "skipping windows-x86_64: BARREL_WINDOWS_SDL3 must name a prefix with bin/SDL3.dll, lib/libSDL3.dll.a and lib/pkgconfig/sdl3.pc" >&2
else
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
    export PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$BARREL_WINDOWS_SDL3/lib/pkgconfig"
    tools/capped.sh cargo build --offline --release --target "$WIN" -p barrel
    DIR="$STAGE/barrel-$VERSION-windows-x86_64"
    mkdir -p "$DIR"
    x86_64-w64-mingw32-strip -o "$DIR/barrel.exe" "${CARGO_TARGET_DIR:-target}/$WIN/release/barrel.exe"
    cp "$BARREL_WINDOWS_SDL3/bin/SDL3.dll" "$DIR/"
    readme windows "$DIR"
    cp LICENSE "$DIR/"
    (cd "$STAGE" && rm -f "$DEST/barrel-$VERSION-windows-x86_64.zip" && zip -qr "$DEST/barrel-$VERSION-windows-x86_64.zip" "barrel-$VERSION-windows-x86_64")
    echo "package: $DEST/barrel-$VERSION-windows-x86_64.zip"
fi
