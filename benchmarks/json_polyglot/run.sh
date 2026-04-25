#!/usr/bin/env bash
# JSON polyglot benchmark runner.
#
# Each language is listed TWICE — once with idiomatic / default flags,
# once with aggressive / optimized flags — so a skeptical reader can
# see both the "what most projects ship with" number and the "what's
# achievable with full tuning" number.
#
# Workload: 10k records, ~1 MB blob, parse + stringify, 50 iterations.
#
# Methodology (v0.5.243+):
#   RUNS=11 (default; configurable via $RUNS env var). For each cell
#   we collect every per-run wall-clock ms and emit median, p95,
#   stddev (σ), min, and max — not "best-of-N" — so noise and outlier
#   sensitivity are visible. RSS is captured per-run via
#   /usr/bin/time -l (peak RSS, not average); we report the max
#   observed peak across runs.
#
# CPU pinning:
#   macOS: taskpolicy -t 0 -l 0 (sets throughput-tier 0 + latency-tier 0
#          — a scheduler HINT toward P-cores on Apple Silicon, NOT
#          strict pinning. Apple does not expose unprivileged hard
#          affinity. -c user-interactive doesn't exist; that flag only
#          accepts downgrade clamps utility/background/maintenance.)
#   Linux: taskset -c 0 (strict pinning to CPU 0).
#   Otherwise: no pinning, with a caveat banner at run start.
#
# Toolchains expected on PATH:
#   perry  (built locally — falls back to ../../target/release/perry)
#   bun, node
#   go, cargo, swiftc, clang++
#   nlohmann/json (brew install nlohmann-json on macOS)
#   kotlinc + gradle (brew install kotlin gradle)
#
# Outputs:
#   RESULTS.md — markdown table sorted by median time

set -uo pipefail
cd "$(dirname "$0")"

PERRY=${PERRY:-../../target/release/perry}
RUNS=${RUNS:-11}
TMPDIR=$(mktemp -d)
KEEP=${KEEP:-0}
[[ "${1:-}" == "--keep" ]] && KEEP=1
[[ "$KEEP" -eq 0 ]] && trap 'rm -rf "$TMPDIR"' EXIT

NLOHMANN_INCLUDE=$(brew --prefix nlohmann-json 2>/dev/null || echo "")/include

have() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------------------
# CPU pinning detection. Sets PIN_CMD (array, may be empty) and PIN_NOTE.
# ---------------------------------------------------------------------------
PIN_CMD=()
PIN_NOTE=""
case "$(uname)" in
    Darwin)
        if have taskpolicy; then
            PIN_CMD=(taskpolicy -t 0 -l 0)
            PIN_NOTE="macOS scheduler hint (taskpolicy -t 0 -l 0 — P-core preferred via throughput/latency tiers, NOT strict affinity)"
        else
            PIN_NOTE="macOS without taskpolicy — no pinning available"
        fi
        ;;
    Linux)
        if have taskset; then
            PIN_CMD=(taskset -c 0)
            PIN_NOTE="Linux strict (taskset -c 0)"
        else
            PIN_NOTE="Linux without taskset — no pinning available"
        fi
        ;;
    *)
        PIN_NOTE="Unknown platform $(uname) — no pinning attempted"
        ;;
esac

echo "==============================================================="
echo "JSON polyglot benchmark — Perry v$(grep '^version' ../../Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
echo "Hardware: $(uname -srm) on $(hostname -s)"
echo "Runs per cell: $RUNS (median + p95 + σ + min + max reported)"
echo "Pinning strategy: $PIN_NOTE"
echo "==============================================================="
echo

# Results accumulator — TSV: median|label|profile|p95|stddev|min|max|rss_mb
RESULTS_FILE="$TMPDIR/results.tsv"
: > "$RESULTS_FILE"

# Compute median, p95, stddev, min, max from a list of integer ms values.
# Reads values one per line on stdin. Outputs single line:
#   "median|p95|stddev|min|max"
# stddev is population stddev (n divisor), one decimal.
compute_stats() {
    awk '
    { v[NR] = $1 + 0 }
    END {
        n = NR
        if (n == 0) { print "0|0|0|0|0"; exit }
        # sort ascending (insertion sort — fine for n=11..30)
        for (i = 2; i <= n; i++) {
            x = v[i]; j = i - 1
            while (j >= 1 && v[j] > x) { v[j+1] = v[j]; j-- }
            v[j+1] = x
        }
        # median
        if (n % 2 == 1) { median = v[(n+1)/2] }
        else { median = (v[n/2] + v[n/2+1]) / 2 }
        # p95: index = ceil(0.95 * n) (1-based)
        p95_idx = int(0.95 * n + 0.99999)
        if (p95_idx > n) p95_idx = n
        if (p95_idx < 1) p95_idx = 1
        p95 = v[p95_idx]
        # mean + stddev (population)
        sum = 0
        for (i = 1; i <= n; i++) sum += v[i]
        mean = sum / n
        ss = 0
        for (i = 1; i <= n; i++) ss += (v[i] - mean) ^ 2
        stddev = sqrt(ss / n)
        printf "%d|%d|%.1f|%d|%d", median, p95, stddev, v[1], v[n]
    }'
}

# Run a binary RUNS times under the pinning prefix, collect all ms values
# and the worst-case RSS, write a row to RESULTS_FILE plus a stdout line.
#
# Usage: run_bench <profile> <label> <env-string> <command...>
run_bench() {
    local profile="$1"; shift
    local label="$1"; shift
    local env_str="$1"; shift
    local samples_file="$TMPDIR/samples.$$"
    : > "$samples_file"
    local worst_rss=0
    local i
    for i in $(seq 1 "$RUNS"); do
        local stderr_file="$TMPDIR/stderr.$$.$i"
        local out
        if [[ -n "$env_str" ]]; then
            out=$(env $env_str "${PIN_CMD[@]}" /usr/bin/time -l "$@" 2>"$stderr_file" || true)
        else
            out=$("${PIN_CMD[@]}" /usr/bin/time -l "$@" 2>"$stderr_file" || true)
        fi
        local ms
        ms=$(printf '%s\n' "$out" | sed -n 's/^ms:\([0-9]*\)$/\1/p' | head -1)
        local rss_bytes
        rss_bytes=$(grep "maximum resident set size" "$stderr_file" 2>/dev/null | awk '{print $1}')
        rm -f "$stderr_file"
        [[ -z "$ms" ]] && continue
        [[ -z "$rss_bytes" ]] && rss_bytes=0
        echo "$ms" >> "$samples_file"
        if [[ "$rss_bytes" -gt "$worst_rss" ]]; then worst_rss="$rss_bytes"; fi
    done
    local sample_count
    sample_count=$(wc -l < "$samples_file" | awk '{print $1}')
    if [[ "$sample_count" -eq 0 ]]; then
        printf "  %-44s  FAILED (no successful runs)\n" "$label"
        rm -f "$samples_file"
        return
    fi
    local stats
    stats=$(compute_stats < "$samples_file")
    rm -f "$samples_file"
    local median p95 stddev min max
    IFS='|' read -r median p95 stddev min max <<< "$stats"
    local rss_mb
    rss_mb=$(awk -v b="$worst_rss" 'BEGIN { printf "%d", b/1048576 }')
    printf "  %-44s  median=%-4s p95=%-4s σ=%-4s [%s..%s] ms · %s MB\n" \
        "$label ($profile)" "$median" "$p95" "$stddev" "$min" "$max" "$rss_mb"
    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
        "$median" "$label" "$profile" "$p95" "$stddev" "$min" "$max" "$rss_mb" \
        >> "$RESULTS_FILE"
}

# ---------------------------------------------------------------------------
# Perry — idiomatic = default flags (gen-GC, lazy JSON tape).
#         optimized = same default (Perry doesn't have separate flag tiers
#                     for compiled output; the lazy JSON tape IS the
#                     optimization. Direct path is shown for honesty.)
# ---------------------------------------------------------------------------
echo "=== Perry ==="
if [[ -x "$PERRY" ]]; then
    "$PERRY" compile bench.ts -o "$TMPDIR/perry_bin" >/dev/null 2>&1
    if [[ -x "$TMPDIR/perry_bin" ]]; then
        run_bench "optimized"  "perry (gen-gc + lazy tape)"   ""                      "$TMPDIR/perry_bin"
        run_bench "idiomatic"  "perry (mark-sweep, no lazy)"  "PERRY_GEN_GC=0 PERRY_JSON_TAPE=0" "$TMPDIR/perry_bin"
    fi
else
    echo "  Perry binary not found at $PERRY — run 'cargo build --release -p perry' first"
fi

# ---------------------------------------------------------------------------
# Bun — JavaScriptCore JIT.
# ---------------------------------------------------------------------------
echo "=== Bun ==="
if have bun; then
    run_bench "idiomatic" "bun (default)" "" bun bench.ts
fi

# ---------------------------------------------------------------------------
# Node.js — V8.
# ---------------------------------------------------------------------------
echo "=== Node.js ==="
if have node; then
    run_bench "idiomatic" "node (default)"        ""  node --experimental-strip-types bench.ts
    run_bench "optimized" "node --max-old=4096"   ""  node --experimental-strip-types --max-old-space-size=4096 bench.ts
fi

# ---------------------------------------------------------------------------
# Go — encoding/json.
# ---------------------------------------------------------------------------
echo "=== Go ==="
if have go; then
    go build -o "$TMPDIR/bench_go_idio" bench.go 2>&1 | tail -3
    if [[ -x "$TMPDIR/bench_go_idio" ]]; then
        run_bench "idiomatic" "go (encoding/json)" "" "$TMPDIR/bench_go_idio"
    fi
    go build -ldflags="-s -w" -trimpath -o "$TMPDIR/bench_go_opt" bench.go 2>&1 | tail -3
    if [[ -x "$TMPDIR/bench_go_opt" ]]; then
        run_bench "optimized" "go -ldflags=\"-s -w\" -trimpath" "" "$TMPDIR/bench_go_opt"
    fi
fi

# ---------------------------------------------------------------------------
# Rust serde_json
# ---------------------------------------------------------------------------
echo "=== Rust ==="
if have cargo; then
    cargo build --release --target-dir "$TMPDIR/cargo_target" --quiet 2>&1 | tail -5
    if [[ -x "$TMPDIR/cargo_target/release/bench" ]]; then
        run_bench "idiomatic" "rust serde_json" "" "$TMPDIR/cargo_target/release/bench"
    fi
    cargo build --profile release-aggressive --target-dir "$TMPDIR/cargo_target" --quiet 2>&1 | tail -5
    if [[ -x "$TMPDIR/cargo_target/release-aggressive/bench" ]]; then
        run_bench "optimized" "rust serde_json (LTO+1cgu)" "" "$TMPDIR/cargo_target/release-aggressive/bench"
    fi
fi

# ---------------------------------------------------------------------------
# Swift
# ---------------------------------------------------------------------------
echo "=== Swift ==="
if have swiftc; then
    swiftc -O bench.swift -o "$TMPDIR/bench_swift_idio" 2>&1 | tail -3
    if [[ -x "$TMPDIR/bench_swift_idio" ]]; then
        run_bench "idiomatic" "swift -O (Foundation)" "" "$TMPDIR/bench_swift_idio"
    fi
    swiftc -O -wmo bench.swift -o "$TMPDIR/bench_swift_opt" 2>&1 | tail -3
    if [[ -x "$TMPDIR/bench_swift_opt" ]]; then
        run_bench "optimized" "swift -O -wmo (Foundation)" "" "$TMPDIR/bench_swift_opt"
    fi
fi

# ---------------------------------------------------------------------------
# Kotlin (kotlinx.serialization on JVM)
# ---------------------------------------------------------------------------
echo "=== Kotlin ==="
GRADLE_LIB=$(brew --prefix gradle 2>/dev/null)/libexec/lib
KOTLINC_LIB=$(brew --prefix kotlin 2>/dev/null)/libexec/lib
if have kotlinc && [[ -d "$GRADLE_LIB" ]] && [[ -d "$KOTLINC_LIB" ]]; then
    SERIALIZATION_CP="$GRADLE_LIB/kotlinx-serialization-core-jvm-1.9.0.jar:$GRADLE_LIB/kotlinx-serialization-json-jvm-1.9.0.jar"
    PLUGIN="$KOTLINC_LIB/kotlinx-serialization-compiler-plugin.jar"
    kotlinc -Xplugin="$PLUGIN" -classpath "$SERIALIZATION_CP" -include-runtime -d "$TMPDIR/bench_kt.jar" bench.kt 2>&1 | tail -3
    if [[ -f "$TMPDIR/bench_kt.jar" ]]; then
        run_bench "idiomatic" "kotlin (kotlinx.serialization)" "" \
            java -cp "$TMPDIR/bench_kt.jar:$SERIALIZATION_CP" BenchKt
        run_bench "optimized" "kotlin -server -Xmx512m" "" \
            java -server -Xmx512m -cp "$TMPDIR/bench_kt.jar:$SERIALIZATION_CP" BenchKt
    fi
else
    echo "  kotlinc / gradle JARs not found — skipping (brew install kotlin gradle)"
fi

# ---------------------------------------------------------------------------
# C++ (nlohmann/json)
# ---------------------------------------------------------------------------
echo "=== C++ ==="
if have clang++ && [[ -d "$NLOHMANN_INCLUDE" ]]; then
    clang++ -std=c++17 -O2 -I"$NLOHMANN_INCLUDE" bench.cpp -o "$TMPDIR/bench_cpp_idio" 2>&1 | tail -3
    if [[ -x "$TMPDIR/bench_cpp_idio" ]]; then
        run_bench "idiomatic" "c++ -O2 (nlohmann/json)" "" "$TMPDIR/bench_cpp_idio"
    fi
    clang++ -std=c++17 -O3 -flto -I"$NLOHMANN_INCLUDE" bench.cpp -o "$TMPDIR/bench_cpp_opt" 2>&1 | tail -3
    if [[ -x "$TMPDIR/bench_cpp_opt" ]]; then
        run_bench "optimized" "c++ -O3 -flto (nlohmann/json)" "" "$TMPDIR/bench_cpp_opt"
    fi
fi

# ---------------------------------------------------------------------------
# Write RESULTS.md (sorted by median time, ascending)
# ---------------------------------------------------------------------------
{
    echo "# JSON Polyglot Benchmark Results"
    echo
    echo "**Workload:** parse + stringify a 10,000-record (~1 MB) JSON array, 50 iterations per run."
    echo "**Runs per cell:** $RUNS · **Pinning:** $PIN_NOTE"
    echo "**Hardware:** $(uname -srm) on $(hostname -s)."
    echo "**Date:** $(date -u +%Y-%m-%d)."
    echo
    echo "Each language listed twice — *idiomatic* (default release-mode flags most projects use) and *optimized* (aggressive tuning). Median wall-clock time is the headline number; p95 (worst-of-best-95%), σ (population stddev), min, and max are reported per cell so noise is visible. Lower is better; sorted by median time."
    echo
    echo "| Implementation | Profile | Median (ms) | p95 (ms) | σ | Min | Max | Peak RSS (MB) |"
    echo "|---|---|---:|---:|---:|---:|---:|---:|"
    sort -t$'\t' -k1 -n "$RESULTS_FILE" | while IFS=$'\t' read -r median label profile p95 stddev mn mx rss; do
        printf "| %s | %s | %s | %s | %s | %s | %s | %s |\n" \
            "$label" "$profile" "$median" "$p95" "$stddev" "$mn" "$mx" "$rss"
    done
} > RESULTS.md

echo
echo "Wrote $(pwd)/RESULTS.md"
cat RESULTS.md
