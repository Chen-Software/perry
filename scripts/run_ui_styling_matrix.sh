#!/usr/bin/env bash
# perry/ui styling-matrix CI gate (Phase A of issue #185).
#
# Three checks:
#   1. The matrix in `crates/perry-ui/src/styling_matrix.rs` matches the
#      actual `perry_ui_*` exports in every backend's `lib.rs`. Cells
#      claiming Wired/Stub for a missing symbol — or claiming Missing
#      while the symbol is exported — both fail.
#   2. `docs/src/ui/styling-matrix.md` is up-to-date relative to the
#      source-of-truth. `--gen` rewrites it; CI runs `git diff --exit-code`
#      after this script to catch uncommitted regenerations.
#   3. `cargo test -p perry-ui` passes (matrix invariants).
#
# Usage:
#   scripts/run_ui_styling_matrix.sh

set -uo pipefail

cd "$(dirname "$0")/.."

echo "[1/3] Building styling-matrix binary"
cargo build --quiet -p perry-ui --bin styling-matrix
status=$?
if [[ $status -ne 0 ]]; then
    echo "FAIL: cargo build -p perry-ui --bin styling-matrix failed"
    exit 1
fi

echo "[2/3] Verifying matrix vs lib.rs reality (--check)"
./target/debug/styling-matrix --check
status=$?
if [[ $status -ne 0 ]]; then
    echo
    echo "FAIL: matrix drift detected. Either:"
    echo "  - update crates/perry-ui/src/styling_matrix.rs to match the lib.rs files, or"
    echo "  - update the affected backend's lib.rs to match the matrix's promise."
    exit 1
fi

echo "[3/3] Regenerating docs/src/ui/styling-matrix.md (--gen)"
./target/debug/styling-matrix --gen
status=$?
if [[ $status -ne 0 ]]; then
    echo "FAIL: matrix generation errored"
    exit 1
fi

echo "[4/4] Running matrix unit tests"
cargo test --quiet -p perry-ui
status=$?
if [[ $status -ne 0 ]]; then
    echo "FAIL: cargo test -p perry-ui"
    exit 1
fi

echo "OK: styling matrix in sync with all backends"
