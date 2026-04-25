#!/bin/bash
# Regression: `return` inside a `try { ... }` body skipped `js_try_end`,
# leaking one slot in the runtime's setjmp jump-buffer table per call.
# The cap is 128 — programs hitting the early-return-in-try path enough
# times panic with "Try block nesting too deep".
#
# Fix: `Stmt::Return` lowering now emits `ctx.try_depth` `js_try_end`
# calls before the `ret` so the runtime's TRY_DEPTH counter stays
# balanced.
#
# This test exercises the specific shape (try-body containing an
# early-return path) hundreds of times in a tight loop. Pre-fix this
# would panic before iteration 128; post-fix it runs to completion.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PERRY="$SCRIPT_DIR/../target/release/perry"
[ ! -f "$PERRY" ] && PERRY="$SCRIPT_DIR/../target/debug/perry"
if [ ! -f "$PERRY" ]; then
  echo "SKIP: perry binary not found (build with cargo build --release)"
  exit 0
fi

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

cat > "$TMPDIR/main.ts" << 'EOF'
// Each call to `step` enters a try, takes the early-return branch,
// and leaks one TRY_DEPTH slot pre-fix. With MAX_TRY_DEPTH = 128,
// the 128th call panics. Run 500 iterations so we'd be deep into
// the panic territory if the leak still existed.
function step(i: number): number {
  try {
    if (i >= 0) return i;  // early return inside try body
    return -1;
  } catch (e: any) {
    return -2;
  }
}

let sum = 0;
for (let i = 0; i < 500; i++) {
  sum += step(i);
}
console.log("sum=" + sum);
console.log("done");
EOF

cd "$TMPDIR"
"$PERRY" compile main.ts --output test_bin >/dev/null 2>&1
RUN_OUTPUT=$(./test_bin 2>&1)

# Sum of 0..499 = 124750
EXPECTED="sum=124750
done"

if [ "$RUN_OUTPUT" = "$EXPECTED" ]; then
  echo "PASS"
  exit 0
fi

echo "FAIL: try-block depth leaked or wrong sum"
echo "Expected:"
echo "$EXPECTED"
echo ""
echo "Got:"
echo "$RUN_OUTPUT"
exit 1
