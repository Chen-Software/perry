#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p zig-out/bin
zig build-exe src/main.zig \
  -O ReleaseFast \
  -target aarch64-macos.14.0 \
  -lc \
  --name json_pipeline \
  -femit-bin=zig-out/bin/json_pipeline
echo "built: $(du -h zig-out/bin/json_pipeline | cut -f1) zig-out/bin/json_pipeline"
