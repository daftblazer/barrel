#!/bin/bash
# tools/deploy.sh [user@host]: build Barrel and install it as the Kintsugi engine for app 1152310
# (Escha & Logy DX): on this machine, or over SSH on an aarch64 device (cross-built: see
# tools/package.sh for BARREL_AARCH64_SYSROOT / BARREL_AARCH64_DEPS). Uses Kintsugi's
# tools/install-engine.sh ($KINTSUGI, else ../../tools/kintsugi). For a password login set
# SSH_ASKPASS (with SSH_ASKPASS_REQUIRE=force).
set -euo pipefail
cd "$(dirname "$0")/.."
HOST=${1:-}
K=${KINTSUGI:-../../tools/kintsugi}
if [ -z "$HOST" ]; then
    tools/package.sh x86_64 target/engine/x86_64
    "$K/tools/install-engine.sh" 1152310 target/engine/x86_64
else
    tools/package.sh aarch64 target/engine/aarch64
    "$K/tools/install-engine.sh" 1152310 target/engine/aarch64 "$HOST"
fi
