#!/bin/sh
# The linker for aarch64 Linux builds (cargo's target linker): clang with lld against an arm64
# root filesystem ($BARREL_AARCH64_SYSROOT: glibc no newer than the device's, the Vulkan loader,
# libwayland-client) and a prefix with arm64 SDL3 ($BARREL_AARCH64_DEPS/lib). tools/package.sh
# sets both.
exec clang --target=aarch64-linux-gnu --sysroot="${BARREL_AARCH64_SYSROOT:?}" -fuse-ld=lld -L"${BARREL_AARCH64_DEPS:?}/lib" -Wl,-rpath,'$ORIGIN/lib' "$@"
