# Polyglot Compute-Microbench Results (auto-generated)

**Runs per cell:** 11 · **Pinning:** macOS scheduler hint (taskpolicy -t 0 -l 0 — P-core preferred via throughput/latency tiers, NOT strict affinity)
**Hardware:** Darwin 25.4.0 arm64 on MacBookPro · **Date:** 2026-04-25
**Perry version:** v0.5.243

Headline = median wall-clock ms. Lower is better.

| Benchmark      | Perry |  Rust |   C++ |    Go | Swift |  Java |  Node |   Bun | Hermes |  Python |
|----------------|-------|-------|-------|-------|-------|-------|-------|-------|--------|---------|
| fibonacci      |   312 |   319 |   308 |   454 |   400 |   283 |  1016 |   518 |      - |   15814 |
| loop_overhead  |    12 |    96 |    97 |    98 |    97 |    99 |    56 |    41 |      - |    2986 |
| array_write    |     4 |     7 |     3 |     9 |     3 |     9 |     9 |     6 |      - |     396 |
| array_read     |     4 |     9 |     9 |    11 |     9 |    11 |    13 |    15 |      - |     342 |
| math_intensive |    14 |    48 |    50 |    51 |    49 |    51 |    49 |    50 |      - |    2244 |
| object_create  |     1 |     0 |     0 |     0 |     0 |     5 |     8 |     6 |      - |     163 |
| nested_loops   |    17 |     8 |     8 |    11 |     8 |    10 |    17 |    19 |      - |     485 |
| accumulate     |    34 |    95 |    95 |    98 |    98 |    98 |   598 |    98 |      - |    5052 |

## Per-cell full stats

Format: median (p95: X, σ: S, min: Y, max: Z) ms

| Benchmark | Runtime | Stats (ms) |
|---|---|---|
| fibonacci | perry | 312 (p95: 394, σ: 28.1, min: 308, max: 394) |
| fibonacci | rust | 319 (p95: 338, σ: 6.1, min: 315, max: 338) |
| fibonacci | cpp | 308 (p95: 314, σ: 3.5, min: 302, max: 314) |
| fibonacci | go | 454 (p95: 488, σ: 11.0, min: 448, max: 488) |
| fibonacci | swift | 400 (p95: 416, σ: 5.4, min: 396, max: 416) |
| fibonacci | java | 283 (p95: 301, σ: 5.5, min: 281, max: 301) |
| fibonacci | node | 1016 (p95: 1882, σ: 248.1, min: 998, max: 1882) |
| fibonacci | bun | 518 (p95: 525, σ: 3.4, min: 513, max: 525) |
| fibonacci | hermes | - |
| fibonacci | python | 15814 (p95: 16099, σ: 118.2, min: 15755, max: 16099) |
| loop_overhead | perry | 12 (p95: 16, σ: 1.2, min: 12, max: 16) |
| loop_overhead | rust | 96 (p95: 97, σ: 1.1, min: 94, max: 97) |
| loop_overhead | cpp | 97 (p95: 98, σ: 1.4, min: 94, max: 98) |
| loop_overhead | go | 98 (p95: 102, σ: 1.4, min: 97, max: 102) |
| loop_overhead | swift | 97 (p95: 101, σ: 1.5, min: 96, max: 101) |
| loop_overhead | java | 99 (p95: 101, σ: 1.0, min: 98, max: 101) |
| loop_overhead | node | 56 (p95: 69, σ: 4.3, min: 54, max: 69) |
| loop_overhead | bun | 41 (p95: 41, σ: 0.5, min: 40, max: 41) |
| loop_overhead | hermes | - |
| loop_overhead | python | 2986 (p95: 3077, σ: 35.7, min: 2947, max: 3077) |
| array_write | perry | 4 (p95: 5, σ: 0.7, min: 3, max: 5) |
| array_write | rust | 7 (p95: 8, σ: 0.4, min: 6, max: 8) |
| array_write | cpp | 3 (p95: 6, σ: 1.2, min: 1, max: 6) |
| array_write | go | 9 (p95: 13, σ: 1.2, min: 9, max: 13) |
| array_write | swift | 3 (p95: 5, σ: 1.2, min: 1, max: 5) |
| array_write | java | 9 (p95: 17, σ: 2.7, min: 7, max: 17) |
| array_write | node | 9 (p95: 10, σ: 0.6, min: 8, max: 10) |
| array_write | bun | 6 (p95: 8, σ: 0.9, min: 5, max: 8) |
| array_write | hermes | - |
| array_write | python | 396 (p95: 404, σ: 4.3, min: 389, max: 404) |
| array_read | perry | 4 (p95: 5, σ: 0.3, min: 4, max: 5) |
| array_read | rust | 9 (p95: 9, σ: 0.0, min: 9, max: 9) |
| array_read | cpp | 9 (p95: 10, σ: 0.4, min: 9, max: 10) |
| array_read | go | 11 (p95: 17, σ: 2.4, min: 11, max: 17) |
| array_read | swift | 9 (p95: 10, σ: 0.3, min: 9, max: 10) |
| array_read | java | 11 (p95: 12, σ: 0.4, min: 11, max: 12) |
| array_read | node | 13 (p95: 14, σ: 0.6, min: 12, max: 14) |
| array_read | bun | 15 (p95: 18, σ: 0.9, min: 15, max: 18) |
| array_read | hermes | - |
| array_read | python | 342 (p95: 350, σ: 4.7, min: 336, max: 350) |
| math_intensive | perry | 14 (p95: 17, σ: 0.9, min: 14, max: 17) |
| math_intensive | rust | 48 (p95: 49, σ: 0.6, min: 47, max: 49) |
| math_intensive | cpp | 50 (p95: 89, σ: 11.2, min: 49, max: 89) |
| math_intensive | go | 51 (p95: 74, σ: 6.7, min: 50, max: 74) |
| math_intensive | swift | 49 (p95: 50, σ: 0.6, min: 48, max: 50) |
| math_intensive | java | 51 (p95: 52, σ: 0.6, min: 50, max: 52) |
| math_intensive | node | 49 (p95: 51, σ: 0.7, min: 49, max: 51) |
| math_intensive | bun | 50 (p95: 51, σ: 0.5, min: 50, max: 51) |
| math_intensive | hermes | - |
| math_intensive | python | 2244 (p95: 4091, σ: 531.8, min: 2215, max: 4091) |
| object_create | perry | 1 (p95: 1, σ: 0.5, min: 0, max: 1) |
| object_create | rust | 0 (p95: 1, σ: 0.3, min: 0, max: 1) |
| object_create | cpp | 0 (p95: 0, σ: 0.0, min: 0, max: 0) |
| object_create | go | 0 (p95: 0, σ: 0.0, min: 0, max: 0) |
| object_create | swift | 0 (p95: 0, σ: 0.0, min: 0, max: 0) |
| object_create | java | 5 (p95: 5, σ: 0.4, min: 4, max: 5) |
| object_create | node | 8 (p95: 9, σ: 0.5, min: 8, max: 9) |
| object_create | bun | 6 (p95: 8, σ: 0.7, min: 5, max: 8) |
| object_create | hermes | - |
| object_create | python | 163 (p95: 165, σ: 1.5, min: 160, max: 165) |
| nested_loops | perry | 17 (p95: 20, σ: 0.9, min: 17, max: 20) |
| nested_loops | rust | 8 (p95: 9, σ: 0.3, min: 8, max: 9) |
| nested_loops | cpp | 8 (p95: 9, σ: 0.3, min: 8, max: 9) |
| nested_loops | go | 11 (p95: 13, σ: 1.6, min: 8, max: 13) |
| nested_loops | swift | 8 (p95: 9, σ: 0.5, min: 8, max: 9) |
| nested_loops | java | 10 (p95: 11, σ: 0.5, min: 10, max: 11) |
| nested_loops | node | 17 (p95: 18, σ: 0.5, min: 16, max: 18) |
| nested_loops | bun | 19 (p95: 20, σ: 0.5, min: 19, max: 20) |
| nested_loops | hermes | - |
| nested_loops | python | 485 (p95: 510, σ: 9.1, min: 477, max: 510) |
| accumulate | perry | 34 (p95: 35, σ: 0.4, min: 34, max: 35) |
| accumulate | rust | 95 (p95: 96, σ: 0.7, min: 94, max: 96) |
| accumulate | cpp | 95 (p95: 97, σ: 1.1, min: 94, max: 97) |
| accumulate | go | 98 (p95: 99, σ: 0.9, min: 96, max: 99) |
| accumulate | swift | 98 (p95: 99, σ: 1.0, min: 96, max: 99) |
| accumulate | java | 98 (p95: 99, σ: 0.8, min: 97, max: 99) |
| accumulate | node | 598 (p95: 604, σ: 3.8, min: 590, max: 604) |
| accumulate | bun | 98 (p95: 100, σ: 0.9, min: 97, max: 100) |
| accumulate | hermes | - |
| accumulate | python | 5052 (p95: 9388, σ: 1454.0, min: 4979, max: 9388) |
