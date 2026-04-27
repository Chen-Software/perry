# `04_array_read` — why 211 MB for 4 ms

`benchmarks/suite/04_array_read.ts` measures sequential read of a 10M-element
`number[]`. The timed loop is fast (4 ms) but the headline `Peak RSS` cell in
the README's "Other Perry benches" table reads 211 MB. This is a working-set
question, not a leak. The math:

## Source

```ts
const SIZE = 10000000;
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
    arr[i] = i;          // forces the array to grow incrementally
}
let sum = 0;
const start = Date.now();
for (let i = 0; i < arr.length; i++) sum = sum + arr[i];
const elapsed = Date.now() - start;
```

The fill loop is what allocates — the timed read loop touches nothing new.

## Working-set math

- Element type is `number` (f64) → **8 bytes per slot, raw**.
- `10M × 8 B = 80 MB` of *actual data*.
- Perry's array starts at `MIN_ARRAY_CAPACITY = 16` and **doubles** on grow
  (`crates/perry-runtime/src/array.rs:528-549`). Old buffer is **abandoned in
  the arena**, not freed — bump-allocator semantics; reclaim happens at the
  next GC.
- Doubling sequence reaching 10M: `16, 32, …, 8_388_608, 16_777_216`. Final
  capacity = `16M × 8 B = 128 MB`.
- During the *last* grow (`8M-cap → 16M-cap`), both buffers coexist in the
  arena until the next GC: `64 MB + 128 MB = 192 MB peak arena`.
- Plus ~6 MB binary/libc baseline + ~13 MB amortized intermediate-growth
  buffers + arena block headers + longlived arena.
- Expected peak RSS ≈ **~210 MB**. Measured: **211 MB**. Match.

## Diagnostic confirmation

`PERRY_GC_DIAG=1 /tmp/array_read_bench`:

```
[gc] blocks: general=8 (2 live), longlived=2 (0 live), freed_bytes=33554592
[gc-step] pre_in_use=67109112 post_in_use=65536216 sweep_freed=33554592 …
[gc] blocks: general=9 (2 live), longlived=2 (0 live), freed_bytes=65536144
[gc-dealloc] freed 2 blocks (3145728 bytes) back to OS
[gc-step] pre_in_use=132645096 post_in_use=130547928 sweep_freed=65536144 …
array_read:4
sum:49999995000000
```

`/usr/bin/time -l`:

```
221937664  maximum resident set size       (211.6 MB)
217465528  peak memory footprint           (207.4 MB)
```

The two GC cycles match the predicted abandon-old-on-grow pattern:

- First minor GC: `sweep_freed = 33,554,592 B ≈ 32 MB` — the abandoned
  `4M-cap` buffer (4M × 8 = 32 MB).
- Second minor GC: `sweep_freed = 65,536,144 B ≈ 62.5 MB` — the abandoned
  `8M-cap` buffer (8M × 8 = 64 MB; the 1.5 MB delta is the freshly-emptied
  arena blocks the buffer was straddling).
- `pre_in_use = 132,645,096 B ≈ 126 MB` is the snapshot *after* the
  `16M-cap` allocation but *before* the `8M-cap` is reclaimed — the live
  side, not the simultaneous-coexistence peak.

## Conclusion

**211 MB is legitimate working-set, not a leak or fragmentation bug.** It is
dominated by:

1. Index-based grow (`arr[i] = i`) triggering doubling reallocation, with the
   last grow holding both `8M` and `16M` capacity buffers simultaneously.
2. Final container capacity rounded up to `16M × 8 B = 128 MB` — bench
   stores 80 MB of meaningful data; the rest is reserved-but-empty slack.

A user pre-allocating with `new Array(SIZE)` would skip the doubling
sequence and land near ~135 MB peak. The bench intentionally uses the
`arr[i] = i` idiom because that matches what hot JS code looks like and
matches what the comparison-language benches do; switching to
preallocation would understate the real-world cost of the doubling
strategy.

If you want to drive this number lower at the runtime level, options are:

- Aggressively GC mid-grow (currently the runtime relies on the post-grow
  cycle that fires shortly after) — has a throughput cost.
- Switch to 1.5× growth instead of 2× — reduces peak coexistence to
  `(2/3)·N + N = 1.67·N` instead of `0.5·N + N = 1.5·N`. Wait, that's
  actually a *worse* peak; doubling is already optimal for transient peak
  among integer-multiple growth factors. The 1.5× win is amortized
  reclaim, not peak.
- Realloc-in-place when the trailing arena block has room — a real win,
  but requires the arena to track per-block free space in a way it
  currently doesn't.

None of those would change the bench's *time* number; they'd only chip at
the headline RSS. Filed as a follow-up: not blocking, not a regression.
