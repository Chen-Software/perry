#!/usr/bin/env python3
"""Generate a deterministic 4K (3840x2160) PPM P6 test image.

Vectorized with bytearray and chunked PRNG so ~8M pixels generate in a few
seconds on a modern laptop. Deterministic: same bytes on every run.
"""
import sys
import struct
from pathlib import Path

W, H = 3840, 2160
SEED = 0x9E3779B9

def main():
    out = Path(__file__).resolve().parent.parent / "assets" / "input.ppm"
    out.parent.mkdir(parents=True, exist_ok=True)
    header = f"P6\n{W} {H}\n255\n".encode("ascii")
    total = W * H * 3
    # xorshift32 stream expanded into a big bytes blob in C
    state = SEED
    chunk_size = 1 << 16  # words per chunk
    pixels = bytearray(total)
    pos = 0
    # Deterministic gradient component
    grad = bytearray(total)
    for y in range(H):
        row_base = (y * 255) // H
        off = y * W * 3
        for x in range(W):
            grad[off] = (x * 255 // W) & 0xFF
            grad[off + 1] = row_base & 0xFF
            grad[off + 2] = ((x + y) * 255 // (W + H)) & 0xFF
            off += 3
    # xorshift32 noise
    noise = bytearray(total)
    s = state
    i = 0
    while i + 4 <= total:
        s ^= (s << 13) & 0xFFFFFFFF
        s ^= (s >> 17)
        s ^= (s << 5) & 0xFFFFFFFF
        s &= 0xFFFFFFFF
        noise[i]     = s & 0xFF
        noise[i + 1] = (s >> 8) & 0xFF
        noise[i + 2] = (s >> 16) & 0xFF
        noise[i + 3] = (s >> 24) & 0xFF
        i += 4
    while i < total:
        s ^= (s << 13) & 0xFFFFFFFF
        s ^= (s >> 17)
        s ^= (s << 5) & 0xFFFFFFFF
        s &= 0xFFFFFFFF
        noise[i] = s & 0xFF
        i += 1
    # sum grad + noise mod 256
    for j in range(total):
        pixels[j] = (grad[j] + noise[j]) & 0xFF
    with open(out, "wb") as f:
        f.write(header)
        f.write(pixels)
    print(f"wrote {out} ({out.stat().st_size} bytes, {W}x{H})")

if __name__ == "__main__":
    main()
