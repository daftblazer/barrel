#!/bin/bash
# tools/deploy.sh [user@host]: build Barrel and install it as the Kintsugi engine for app 1152310
# (Escha & Logy DX), or with BARREL_GAME=ryza for app 3365030 (Ryza DX): on this machine, or over SSH on an aarch64 device (cross-built: see
# tools/package.sh for BARREL_AARCH64_SYSROOT / BARREL_AARCH64_DEPS). Uses Kintsugi's
# tools/install-engine.sh ($KINTSUGI, else ../../tools/kintsugi). For a password login set
# SSH_ASKPASS (with SSH_ASKPASS_REQUIRE=force).
set -euo pipefail
cd "$(dirname "$0")/.."
HOST=${1:-}
GAME=${BARREL_GAME:-escha}
case "$GAME" in escha) APP=1152310 ;; ryza) APP=3365030 ;; *) echo "unknown game $GAME" >&2; exit 1 ;; esac
K=${KINTSUGI:-../../tools/kintsugi}
if [ -z "$HOST" ]; then
    tools/package.sh x86_64 target/engine/$GAME-x86_64 "$GAME"
    "$K/tools/install-engine.sh" "$APP" target/engine/$GAME-x86_64
else
    tools/package.sh aarch64 target/engine/$GAME-aarch64 "$GAME"
    "$K/tools/install-engine.sh" "$APP" target/engine/$GAME-aarch64 "$HOST"
fi
