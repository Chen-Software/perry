# JSON Polyglot Benchmark Results

**Workload:** parse + stringify a 10,000-record (~1 MB) JSON array, 50 iterations per run.
**Runs per cell:** 11 · **Pinning:** macOS scheduler hint (taskpolicy -t 0 -l 0 — P-core preferred via throughput/latency tiers, NOT strict affinity)
**Hardware:** Darwin 25.4.0 arm64 on MacBookPro.
**Date:** 2026-04-25.

Each language listed twice — *idiomatic* (default release-mode flags most projects use) and *optimized* (aggressive tuning). Median wall-clock time is the headline number; p95 (worst-of-best-95%), σ (population stddev), min, and max are reported per cell so noise is visible. Lower is better; sorted by median time.

| Implementation | Profile | Median (ms) | p95 (ms) | σ | Min | Max | Peak RSS (MB) |
|---|---|---:|---:|---:|---:|---:|---:|
| perry (gen-gc + lazy tape) | optimized | 74 | 85 | 4.9 | 69 | 85 | 85 |
| rust serde_json (LTO+1cgu) | optimized | 183 | 186 | 1.3 | 181 | 186 | 11 |
| rust serde_json | idiomatic | 199 | 201 | 1.2 | 197 | 201 | 11 |
| bun (default) | idiomatic | 276 | 294 | 7.3 | 267 | 294 | 84 |
| node --max-old=4096 | optimized | 381 | 421 | 16.3 | 374 | 421 | 182 |
| perry (mark-sweep, no lazy) | idiomatic | 384 | 459 | 22.9 | 372 | 459 | 102 |
| node (default) | idiomatic | 385 | 484 | 37.8 | 370 | 484 | 182 |
| kotlin -server -Xmx512m | optimized | 457 | 472 | 5.6 | 449 | 472 | 424 |
| kotlin (kotlinx.serialization) | idiomatic | 475 | 485 | 9.0 | 452 | 485 | 607 |
| c++ -O3 -flto (nlohmann/json) | optimized | 783 | 791 | 2.7 | 781 | 791 | 25 |
| go (encoding/json) | idiomatic | 807 | 873 | 22.5 | 800 | 873 | 23 |
| go -ldflags="-s -w" -trimpath | optimized | 812 | 1118 | 91.2 | 802 | 1118 | 23 |
| c++ -O2 (nlohmann/json) | idiomatic | 855 | 858 | 2.2 | 852 | 858 | 25 |
| swift -O (Foundation) | idiomatic | 3747 | 3990 | 72.9 | 3721 | 3990 | 34 |
| swift -O -wmo (Foundation) | optimized | 3879 | 5309 | 426.8 | 3750 | 5309 | 35 |
