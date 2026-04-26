#!/usr/bin/env bash
# Top-level driver: build all implementations, run all workloads, write
# results/results.json and results/metadata.json. Exits non-zero if any build
# fails; workload run failures are recorded as exit_code in the JSON but
# don't abort the suite.
#
# Env var overrides (all optional):
#   HONEST_BENCH_WARMUP=5
#   HONEST_BENCH_MEASURED=20
#   HONEST_BENCH_SKIP_BUILD=1     — skip (re)building the three toolchains
#   HONEST_BENCH_ONLY=3           — comma-separated workload ids to run
#                                   (1=json, 3=image_convolution; 2 is TBD)
#
# Layout assumes cwd = benchmarks/honest_bench/.

set -euo pipefail

cd "$(dirname "$0")"
ROOT="$(pwd)"
PERRY_ROOT="$(cd ../.. && pwd)"
RESULTS_DIR="$ROOT/results"
mkdir -p "$RESULTS_DIR"

# ------------------------------ 1. metadata -----------------------------------
echo "--- capturing metadata"
python3 - <<PY > "$RESULTS_DIR/metadata.json"
import json, subprocess, datetime, platform, os
def run(cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=10).stdout.strip()
    except Exception as e:
        return f"error: {e}"
meta = {
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "host": {
        "os_version": run(["sw_vers", "-productVersion"]),
        "kernel":     run(["uname", "-a"]),
        "arch":       platform.machine(),
        "cpu":        run(["sysctl", "-n", "machdep.cpu.brand_string"]),
        "ncpu":       run(["sysctl", "-n", "hw.ncpu"]),
        "ram_gb":     round(int(run(["sysctl", "-n", "hw.memsize"]) or 0) / (1024**3), 2),
    },
    "toolchains": {
        "rustc": run(["rustc", "--version"]),
        "cargo": run(["cargo", "--version"]),
        "zig":   run(["zig", "version"]),
        "python": run(["python3", "--version"]),
        "perry": run([os.path.join("$PERRY_ROOT", "target/release/perry"), "--version"]) or "(local build)",
    },
    "harness": {
        "warmup":   int(os.environ.get("HONEST_BENCH_WARMUP", 5)),
        "measured": int(os.environ.get("HONEST_BENCH_MEASURED", 20)),
    },
}
print(json.dumps(meta, indent=2))
PY

# ------------------------------ 2. build --------------------------------------
if [[ -z "${HONEST_BENCH_SKIP_BUILD:-}" ]]; then
  echo "--- building Rust image_conv"
  (cd "$ROOT/workloads/3_image_convolution/rust" && cargo build --release >/dev/null)
  echo "--- building Zig image_conv"
  (cd "$ROOT/workloads/3_image_convolution/zig" && ./build.sh >/dev/null)
  echo "--- building Perry image_conv"
  (cd "$PERRY_ROOT" && target/release/perry "$ROOT/workloads/3_image_convolution/perry/image_conv.ts" \
        -o "$ROOT/workloads/3_image_convolution/perry/image_conv" 2>&1 | tail -2)

  echo "--- building Rust json_pipeline"
  (cd "$ROOT/workloads/1_json_pipeline/rust" && cargo build --release >/dev/null)
  echo "--- building Zig json_pipeline"
  (cd "$ROOT/workloads/1_json_pipeline/zig" && ./build.sh >/dev/null)
  echo "--- building Perry json_pipeline"
  (cd "$PERRY_ROOT" && target/release/perry "$ROOT/workloads/1_json_pipeline/perry/json_pipeline.ts" \
        -o "$ROOT/workloads/1_json_pipeline/perry/json_pipeline" 2>&1 | tail -2)
fi

# ------------------------------ 3. fixtures -----------------------------------
if [[ ! -f "$ROOT/assets/input.json" ]]; then
  echo "--- generating JSON fixture (one-time)"
  python3 scripts/gen_json.py
fi
if [[ ! -f "$ROOT/assets/input_small.json" ]]; then
  # 100-record cut of the full fixture — all three languages run this
  python3 -c "
import json
with open('$ROOT/assets/input.json') as f: full = json.load(f)
with open('$ROOT/assets/input_small.json', 'w') as f:
    json.dump(full[:100], f, separators=(',',':'))
"
fi

# ------------------------------ 4. run ----------------------------------------
ONLY="${HONEST_BENCH_ONLY:-1,3}"
RESULTS="$RESULTS_DIR/results.json"
# Clear & start a JSON array — we append raw rows and close it at the end.
: > "$RESULTS.rows"

run_one() {
  local workload="$1" lang="$2" bin="$3"; shift 3
  echo "--- running $workload / $lang"
  bash "$ROOT/harness/run_bench.sh" "$workload" "$lang" "$bin" "$@" >> "$RESULTS.rows"
}

NODE_FLAGS="--experimental-strip-types --disable-warning=MODULE_TYPELESS_PACKAGE_JSON"
NODE_IMG="$ROOT/workloads/3_image_convolution/node/image_conv.ts"
NODE_JSON="$ROOT/workloads/1_json_pipeline/node/json_pipeline.ts"

if [[ ",$ONLY," == *,3,* ]]; then
  echo "=== workload 3: image convolution ==="
  run_one image_convolution rust  "$ROOT/workloads/3_image_convolution/rust/target/release/image_conv"
  run_one image_convolution zig   "$ROOT/workloads/3_image_convolution/zig/zig-out/bin/image_conv"
  run_one image_convolution perry "$ROOT/workloads/3_image_convolution/perry/image_conv"
  run_one image_convolution node  "node" $NODE_FLAGS "$NODE_IMG"
  run_one image_convolution bun   "bun" "run" "$NODE_IMG"
fi

if [[ ",$ONLY," == *,1,* ]]; then
  echo "=== workload 1 (small): json pipeline @ 100 records ==="
  SMALL_IN="$ROOT/assets/input_small.json"
  run_one json_pipeline_small rust  "$ROOT/workloads/1_json_pipeline/rust/target/release/json_pipeline"  "$SMALL_IN" "/tmp/out_rust.json"
  run_one json_pipeline_small zig   "$ROOT/workloads/1_json_pipeline/zig/zig-out/bin/json_pipeline"       "$SMALL_IN" "/tmp/out_zig.json"
  run_one json_pipeline_small perry "$ROOT/workloads/1_json_pipeline/perry/json_pipeline"                 "$SMALL_IN" "/tmp/out_perry.json"
  run_one json_pipeline_small node  "node" $NODE_FLAGS "$NODE_JSON" "$SMALL_IN" "/tmp/out_node.json"
  run_one json_pipeline_small bun   "bun" "run" "$NODE_JSON" "$SMALL_IN" "/tmp/out_bun.json"

  echo "=== workload 1 (full): json pipeline @ 500k records ==="
  FULL_IN="$ROOT/assets/input.json"
  run_one json_pipeline_full rust  "$ROOT/workloads/1_json_pipeline/rust/target/release/json_pipeline"  "$FULL_IN" "/tmp/out_rust.json"
  run_one json_pipeline_full zig   "$ROOT/workloads/1_json_pipeline/zig/zig-out/bin/json_pipeline"       "$FULL_IN" "/tmp/out_zig.json"
  run_one json_pipeline_full perry "$ROOT/workloads/1_json_pipeline/perry/json_pipeline"                 "$FULL_IN" "/tmp/out_perry.json"
  run_one json_pipeline_full node  "node" $NODE_FLAGS "$NODE_JSON" "$FULL_IN" "/tmp/out_node.json"
  run_one json_pipeline_full bun   "bun" "run" "$NODE_JSON" "$FULL_IN" "/tmp/out_bun.json"
fi

# ------------------------------ 5. finalize -----------------------------------
python3 - <<PY > "$RESULTS"
import json
rows = []
with open("$RESULTS.rows") as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try: rows.append(json.loads(line))
        except Exception: pass
print(json.dumps({"rows": rows}, indent=2))
PY
rm -f "$RESULTS.rows"

N=$(python3 -c "import json; print(len(json.load(open('$RESULTS'))['rows']))")
echo "--- wrote $RESULTS with $N rows"
echo "--- metadata: $RESULTS_DIR/metadata.json"
