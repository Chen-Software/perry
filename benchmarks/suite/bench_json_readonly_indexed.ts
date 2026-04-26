// Companion to bench_json_readonly.ts — adds indexed reads to
// exercise Phase 3's force-materialize-on-IndexGet path. Lazy
// materialize-on-first-access means the cost per parse is:
//   - .length: O(1) from cached_length
//   - .[i].field: forces full materialize ONCE, then reads are
//     O(1) on the tree.
// This is still a lazy win on memory (less blob+tape overhead
// dropped after each iteration's scope ends) but the per-iter time
// is dominated by the eager materialize — so closer to the direct
// path. Useful for showing the shape where lazy ISN'T a magic
// speedup: partial reads force full materialize.

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

for (let w = 0; w < 3; w++) {
  const p = JSON.parse(blob);
  let _x = p.length + p[0].id + p[p.length - 1].id;
}

const ITERATIONS = 50;
const start = Date.now();
let checksum = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
  const parsed = JSON.parse(blob);
  checksum += parsed.length;
  // Read 3 specific records' ids — forces full materialize on
  // first access, subsequent reads are O(1) on the tree.
  checksum += parsed[0].id;
  checksum += parsed[5000].id;
  checksum += parsed[9999].id;
}
const elapsed = Date.now() - start;
console.log("json_readonly_indexed:" + elapsed);
console.log("checksum:" + checksum);
