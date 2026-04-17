# Workload 3 — image convolution

5×5 Gaussian blur over a 3840×2160 RGB image (~8.3M pixels, ~25M bytes).
Tight compute loop with minimal heap pressure; expected to be Perry's best case.

## Why in-memory instead of file I/O

The original spec called for "load 4K PPM, write output". While building this
I hit two gaps in Perry's `fs` module that prevent reading or writing binary
files reliably:

1. **`fs.readFileSync(path)` (no encoding)** — silently returns an empty string
   for non-UTF-8 bytes. The codegen routes `FsReadFileBinary` through
   `js_fs_read_file_sync` (the string-returning path), which uses
   `fs::read_to_string()` internally — that fails hard on any non-UTF-8 byte
   sequence (a 4K random-color PPM has millions of them).
2. **`fs.writeFileSync(path, buffer)`** — writes the right number of bytes but
   they're all zero. The runtime's `js_fs_write_file_sync` reads a
   `StringHeader` regardless of whether the value is actually a `BufferHeader`
   (different layout: 12 vs 8 bytes of pre-data).

Both gaps were filed as follow-up tickets. For this benchmark we want to
measure the **convolution loop**, not whether the host language can read a
file, so all three implementations generate the input in-memory with the same
deterministic PRNG (xorshift32, seed `0x9E3779B9`) and print an output
checksum. The checksum is the 64-bit FNV-1a hash of the output bytes —
identical across the three languages proves the convolution is computed
identically. Wall time and peak RSS are still measured externally.

## Algorithm

```text
kernel K = [[ 1,  4,  7,  4, 1],
            [ 4, 16, 26, 16, 4],
            [ 7, 26, 41, 26, 7],
            [ 4, 16, 26, 16, 4],
            [ 1,  4,  7,  4, 1]]  / 273
edges: clamp-to-edge
pixel layout: interleaved u8 R,G,B
```

Per-pixel: 25 kernel samples × 3 channels = 75 u8 reads + 3 u8 writes. Integer
arithmetic throughout; one `i32 /= 273` per channel at the end. All three
implementations use the same structure.

## Inputs / outputs

- Input: 3840×2160 RGB, generated on the fly per run (not a measured cost).
- Output: same dimensions; only the 64-bit FNV-1a checksum of the pixel bytes
  is printed. Binary file I/O is intentionally excluded from this workload.

## Expected checksum

All three implementations must print the same hex checksum. The harness greps
it out of stdout and diffs across languages.
