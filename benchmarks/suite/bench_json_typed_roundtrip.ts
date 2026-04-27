// Issue #179 Step 1b benchmark. Mirrors bench_json_roundtrip but with
// a typed JSON.parse<Item[]>(blob) call so Perry can route through
// the schema-directed fast path. Node and Bun see the <Item[]> type
// argument erased — same workload, identical runtime behavior.

interface Item {
  id: number;
  name: string;
  value: number;
  tags: string[];
  nested: { x: number; y: number };
}

const items: Item[] = [];
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
for (let i = 0; i < 3; i++) {
  const parsed = JSON.parse<Item[]>(blob);
  JSON.stringify(parsed);
}

const ITERATIONS = 50;
const start = Date.now();
let checksum = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
  const parsed = JSON.parse<Item[]>(blob);
  checksum += parsed.length;
  const reStringified = JSON.stringify(parsed);
  checksum += reStringified.length;
}
const elapsed = Date.now() - start;
console.log("json_typed_roundtrip:" + elapsed);
console.log("checksum:" + checksum);
