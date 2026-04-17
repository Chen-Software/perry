# Perry vs Rust vs Zig — honest benchmark suite

Three workloads that stress different memory patterns, implemented idiomatically
in Perry, Rust, and Zig. Results include peak RSS, latency percentiles, and
binary size — **including where Perry loses**.

## Workloads

| # | Name | Stresses | Expectation |
|---|------|----------|-------------|
| 1 | JSON pipeline | allocation + GC | Perry loses on RSS, competitive on wall time |
| 2 | HTTP echo+transform | tail latency | GC pauses visible in p99/p999 |
| 3 | Image convolution | tight compute loop | Perry competitive |

## Reproducing

```bash
cd benchmarks/honest_bench
./run.sh                  # build + run everything, write results/results.json
python3 scripts/plot.py   # render charts into charts/
python3 scripts/report.py # render REPORT.md from results.json
```

## Layout

```
honest_bench/
├── workloads/
│   ├── 1_json_pipeline/{perry,rust,zig}/
│   ├── 2_http_server/{perry,rust,zig}/
│   └── 3_image_convolution/{perry,rust,zig}/
├── harness/
│   └── run_bench.sh      # per-binary runner: 5 warmup + 20 measured
├── scripts/
│   ├── gen_image.py      # deterministic 4K PPM test fixture
│   ├── gen_json.py       # deterministic 100MB JSON test fixture
│   ├── plot.py           # matplotlib charts -> charts/*.png
│   └── report.py         # render REPORT.md from results.json
├── assets/               # generated test fixtures (gitignored)
├── results/results.json
└── run.sh                # top-level driver
```

## Rules

- Release/optimized builds only (`--release`, `ReleaseFast`, Perry's native path).
- Same algorithm, same data structures, no SIMD intrinsics unless all three have them.
- 5 warmup + 20 measured runs per binary. Median + stddev reported.
- Same machine, same data, same order.
- Test fixtures are deterministic (seeded RNG) so all three languages process
  identical bytes.
