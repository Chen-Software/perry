#!/bin/bash
# Zig 0.15.2 doesn't recognize macOS 26 as a valid target; pin to 14.0.
# Using `zig build-exe` directly because `zig build` bootstraps the build
# script against the host target, which has the same version mismatch.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p zig-out/bin
zig build-exe src/main.zig \
  -O ReleaseFast \
  -target aarch64-macos.14.0 \
  -lc \
  --name image_conv \
  -femit-bin=zig-out/bin/image_conv
echo "built: $(du -h zig-out/bin/image_conv | cut -f1) zig-out/bin/image_conv"
