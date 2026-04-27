// JSON parse + stringify polyglot benchmark — Perry / TypeScript / Bun / Node.
// 10k records, ~1 MB blob, 50 iterations, best-of-5 reporting.
// IDENTICAL workload across every language in this directory.

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

// Warmup — keeps JIT-y runtimes from charging us startup.
for (let i = 0; i < 3; i++) {
  const parsed = JSON.parse(blob);
  JSON.stringify(parsed);
}

const ITERATIONS = 50;
const start = Date.now();

let checksum = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
  const parsed = JSON.parse(blob);
  checksum += parsed.length;
  const reStringified = JSON.stringify(parsed);
  checksum += reStringified.length;
}

const elapsed = Date.now() - start;
console.log("ms:" + elapsed);
console.log("checksum:" + checksum);
