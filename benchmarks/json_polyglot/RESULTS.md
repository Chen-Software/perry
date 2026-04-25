# JSON Polyglot Benchmark Results

**Runs per cell:** 11 · **Pinning:** macOS scheduler hint (taskpolicy -t 0 -l 0 — P-core preferred via throughput/latency tiers, NOT strict affinity)
**Hardware:** Darwin 25.4.0 arm64 on MacBookPro.
**Date:** 2026-04-25.

Two workloads, each language listed twice (idiomatic / optimized flag profile).
Median wall-clock time is the headline number; p95, σ (population stddev),
min, and max are reported per cell so noise is visible. Lower is better.

## JSON validate-and-roundtrip

Per iteration: parse → stringify → discard. The unmutated parse lets
Perry's lazy tape (v0.5.204+) memcpy the original blob bytes for
stringify, which is why Perry's headline number on this workload is so
low — the lazy path can avoid materializing the parse tree entirely.
10k records, ~1 MB blob, 50 iterations per run.

| Implementation | Profile | Median (ms) | p95 (ms) | σ | Min | Max | Peak RSS (MB) |
|---|---|---:|---:|---:|---:|---:|---:|
| c++ -O3 -flto (simdjson) | optimized | 28 | 48 | 6.9 | 25 | 48 | 9 |
| c++ -O2 (simdjson) | idiomatic | 32 | 80 | 13.9 | 30 | 80 | 9 |
| perry (gen-gc + lazy tape) | optimized | 89 | 114 | 12.7 | 74 | 114 | 85 |
| rust serde_json (LTO+1cgu) | optimized | 186 | 188 | 1.1 | 184 | 188 | 11 |
| rust serde_json | idiomatic | 200 | 202 | 1.0 | 198 | 202 | 11 |
| bun (default) | idiomatic | 252 | 257 | 3.2 | 248 | 257 | 84 |
| perry (mark-sweep, no lazy) | idiomatic | 368 | 381 | 7.0 | 360 | 381 | 102 |
| node (default) | idiomatic | 381 | 399 | 7.1 | 377 | 399 | 182 |
| node --max-old=4096 | optimized | 386 | 389 | 3.7 | 377 | 389 | 182 |
| kotlin -server -Xmx512m | optimized | 461 | 466 | 6.4 | 443 | 466 | 421 |
| kotlin (kotlinx.serialization) | idiomatic | 473 | 484 | 7.4 | 457 | 484 | 603 |
| assemblyscript+json-as (wasmtime) | idiomatic | 669 | 694 | 10.7 | 657 | 694 | 58 |
| c++ -O3 -flto (nlohmann/json) | optimized | 798 | 803 | 3.5 | 792 | 803 | 25 |
| go (encoding/json) | idiomatic | 805 | 818 | 5.1 | 797 | 818 | 23 |
| go -ldflags="-s -w" -trimpath | optimized | 806 | 814 | 3.2 | 802 | 814 | 23 |
| c++ -O2 (nlohmann/json) | idiomatic | 871 | 930 | 19.7 | 858 | 930 | 26 |
| swift -O (Foundation) | idiomatic | 3748 | 3778 | 17.8 | 3716 | 3778 | 34 |
| swift -O -wmo (Foundation) | optimized | 3782 | 4196 | 120.3 | 3758 | 4196 | 34 |

## JSON parse-and-iterate

Per iteration: parse → sum every record's nested.x (touches every element)
→ stringify. The full-tree iteration FORCES Perry's lazy tape to
materialize, so this is the honest comparison for workloads that touch
JSON content. 10k records, ~1 MB blob, 50 iterations per run.

| Implementation | Profile | Median (ms) | p95 (ms) | σ | Min | Max | Peak RSS (MB) |
|---|---|---:|---:|---:|---:|---:|---:|
| c++ -O3 -flto (simdjson) | optimized | 25 | 62 | 10.6 | 25 | 62 | 9 |
| c++ -O2 (simdjson) | idiomatic | 26 | 30 | 1.5 | 25 | 30 | 9 |
| rust serde_json (LTO+1cgu) | optimized | 187 | 190 | 1.5 | 185 | 190 | 11 |
| rust serde_json | idiomatic | 205 | 244 | 11.5 | 202 | 244 | 12 |
| bun (default) | idiomatic | 257 | 260 | 2.0 | 252 | 260 | 87 |
| node (default) | idiomatic | 360 | 368 | 4.1 | 352 | 368 | 119 |
| node --max-old=4096 | optimized | 365 | 378 | 5.1 | 358 | 378 | 119 |
| perry (mark-sweep, no lazy) | idiomatic | 373 | 376 | 2.7 | 367 | 376 | 102 |
| kotlin -server -Xmx512m | optimized | 462 | 470 | 4.6 | 454 | 470 | 423 |
| perry (gen-gc + lazy tape) | optimized | 476 | 482 | 3.8 | 470 | 482 | 100 |
| kotlin (kotlinx.serialization) | idiomatic | 484 | 494 | 5.4 | 476 | 494 | 606 |
| assemblyscript+json-as (wasmtime) | idiomatic | 660 | 746 | 26.7 | 648 | 746 | 58 |
| c++ -O3 -flto (nlohmann/json) | optimized | 814 | 1379 | 163.7 | 810 | 1379 | 27 |
| c++ -O2 (nlohmann/json) | idiomatic | 900 | 1004 | 31.2 | 893 | 1004 | 25 |
| go (encoding/json) | idiomatic | 944 | 1618 | 316.2 | 800 | 1618 | 22 |
| go -ldflags="-s -w" -trimpath | optimized | 1332 | 1565 | 159.5 | 990 | 1565 | 22 |
| swift -O (Foundation) | idiomatic | 3714 | 7079 | 1000.8 | 3690 | 7079 | 36 |
| swift -O -wmo (Foundation) | optimized | 3746 | 3850 | 33.3 | 3725 | 3850 | 34 |
