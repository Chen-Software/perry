#!/bin/bash
# Regression: cross-module class getters returned `undefined`.
#
# Bug history:
#   - `ImportedClass` carried `method_names` but no `getter_names`, so the
#     dispatch site at `expr.rs::PropertyGet` never registered a
#     `(class, "__get_<prop>")` entry for imported classes. Reading
#     `obj.prop` for an imported getter fell through to a runtime helper
#     that doesn't know about cross-module accessor descriptors and
#     silently returned `undefined`.
#   - Fixed by adding `getter_names`/`setter_names` to `ImportedClass` and
#     a registration loop in the Phase F method-symbol pass.
#
# This test covers the single-hop case: `import { Box } from './lib';
# const b = new Box(); console.log(b.prop)`. The chained case is covered
# by `test_chained_cross_module_getter.sh`.

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

cat > "$TMPDIR/lib.ts" << 'EOF'
export class Box {
  field: number = 100;
  method(): number { return 200; }
  get prop(): number { return 300; }
}
EOF

cat > "$TMPDIR/main.ts" << 'EOF'
import { Box } from './lib';
const b = new Box();
console.log("field=" + b.field);
console.log("method=" + b.method());
console.log("prop=" + b.prop);
EOF

cd "$TMPDIR"
"$PERRY" compile main.ts --output test_bin >/dev/null 2>&1
RUN_OUTPUT=$(./test_bin 2>&1)

EXPECTED="field=100
method=200
prop=300"

if [ "$RUN_OUTPUT" = "$EXPECTED" ]; then
  echo "PASS"
  exit 0
fi

echo "FAIL: cross-module getter returned wrong value"
echo "Expected:"
echo "$EXPECTED"
echo ""
echo "Got:"
echo "$RUN_OUTPUT"
exit 1
