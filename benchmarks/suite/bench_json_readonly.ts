// Issue #179 Step 2 tier-1 / Phase 2+3+4 read-only benchmark:
// parse a large blob, read a few values, do NOT mutate, do NOT
// stringify. The lazy path should be dramatically faster + lighter
// than the direct path here because the tree is never materialized
// (force-materialize fires on `parsed[i]` but only for the touched
// indices; most of the tape is untouched — and actually, since
// Perry's current force_materialize_lazy materializes the WHOLE
// tree on first indexed access, this bench shows the upper-bound
// cost of partial-read patterns).
//
// Comparison target: Bun + Node on the same workload. Under lazy,
// Perry should win on time (no tree build for .length reads) and
// RSS (no tree allocation if we only touch .length).

const items: any[] = [];
for (let i = 0; i < 10000; i++) {
  items.push({
    id: i,
    name: "item_" + i,
    value: i * 3.14159,
    tags: ["tag_" + (i % 10), "tag_" + (i % 5)],
    nested: { x: i, y: i * 2 }
  });
}
const blob = JSON.stringify(items);

// Warmup
for (let w = 0; w < 3; w++) {
  const p = JSON.parse(blob);
  // just reads — no indexed access that would materialize
  const _ = p.length;
}

const ITERATIONS = 50;
const start = Date.now();
let checksum = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
  const parsed = JSON.parse(blob);
  checksum += parsed.length;
  // Read a handful of specific indices + fields. With lazy + force-
  // materialize-on-index, this triggers ONE full materialization
  // per parse, so expected to be slower than pure .length-only but
  // still much lighter than stringify-every-time (which the other
  // bench exercises).
  // Actually: we only read .length here to isolate the pure lazy
  // win. A sibling bench that adds indexed reads is separate.
}
const elapsed = Date.now() - start;
console.log("json_readonly:" + elapsed);
console.log("checksum:" + checksum);
