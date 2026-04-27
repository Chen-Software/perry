#!/bin/bash
# Regression: chained cross-module getter access returned `undefined`.
#
# Bug: `ImportedClass.field_names` carried only names, not types, so the
# stub class registered into `class_table` had every field typed
# `Type::Any`. `receiver_class_name`'s `PropertyGet` recursion at
# `type_analysis.rs:1030` walks `class.fields[i].ty` to find the receiver
# class for the next hop in a chain — `Type::Any` collapses that walk.
# Compounding the issue, only directly-imported classes ended up in
# `imported_classes`; transitively-referenced ones (`vm.viewport.scroll`
# where Viewport and Scroll are imported by the EditorViewModel module
# but not by the importer) weren't in `class_table` at all.
#
# Fix:
#   - `ImportedClass.field_types: Vec<Type>` populated from the source class.
#   - Fixed-point closure in compile.rs pulls in transitively-referenced
#     classes whose names appear in `field_types` and exist in
#     `exported_classes`.
#
# This test exercises a 3-deep chain (vm → viewport → scroll → getter)
# split across the importer + imported library so the closure has work
# to do (Viewport and Scroll are not in the importer's direct import list).

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
export class Scroll {
  private _top: number = 42;
  get scrollTop(): number { return this._top; }
}
export class Viewport {
  readonly scroll: Scroll;
  constructor() { this.scroll = new Scroll(); }
}
export class VM {
  readonly viewport: Viewport;
  constructor() { this.viewport = new Viewport(); }
}
EOF

cat > "$TMPDIR/main.ts" << 'EOF'
import { VM } from './lib';
const vm = new VM();
// Chain through three modules (well, one library file but three classes)
// where only `VM` is directly named in the importer.
console.log("chain=" + vm.viewport.scroll.scrollTop);
// Same chain via local — exercises the type-flow path (the local must
// be inferred as Scroll for the getter dispatch to find it).
const s = vm.viewport.scroll;
console.log("local=" + s.scrollTop);
EOF

cd "$TMPDIR"
"$PERRY" compile main.ts --output test_bin >/dev/null 2>&1
RUN_OUTPUT=$(./test_bin 2>&1)

EXPECTED="chain=42
local=42"

if [ "$RUN_OUTPUT" = "$EXPECTED" ]; then
  echo "PASS"
  exit 0
fi

echo "FAIL: chained cross-module getter returned wrong value"
echo "Expected:"
echo "$EXPECTED"
echo ""
echo "Got:"
echo "$RUN_OUTPUT"
exit 1
