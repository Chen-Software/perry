# JSON Polyglot Benchmark Results

**Workload:** parse + stringify a 10,000-record (~1 MB) JSON array, 50 iterations, best-of-5.
**Hardware:** Darwin 25.4.0 arm64 on MacBookPro.
**Date:** 2026-04-25.

Each language listed twice — *idiomatic* (default release-mode flags most projects use) and *optimized* (aggressive tuning). Lower is better; sorted by time.

| Implementation | Profile | Time (ms) | Peak RSS (MB) |
|---|---|---:|---:|
| perry (gen-gc + lazy tape) | optimized | 65 | 85 |
| rust serde_json (LTO+1cgu) | optimized | 180 | 11 |
| rust serde_json | idiomatic | 192 | 11 |
| bun (default) | idiomatic | 242 | 80 |
| perry (mark-sweep, no lazy) | idiomatic | 351 | 102 |
| node (default) | idiomatic | 359 | 182 |
| node --max-old=4096 | optimized | 362 | 181 |
| c++ -O3 -flto (nlohmann/json) | optimized | 778 | 25 |
| go -ldflags="-s -w" -trimpath | optimized | 785 | 22 |
| go (encoding/json) | idiomatic | 785 | 24 |
| c++ -O2 (nlohmann/json) | idiomatic | 843 | 25 |
| swift -O -wmo (Foundation) | optimized | 3706 | 33 |
| swift -O (Foundation) | idiomatic | 3710 | 34 |
