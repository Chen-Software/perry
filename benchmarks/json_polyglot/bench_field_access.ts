// JSON parse-and-iterate polyglot benchmark — Perry / Bun / Node.
//
// 10k records (same generator as bench.ts), 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
//
// This is the honest "real-world JSON" companion to bench.ts. Where
// bench.ts is a parse+stringify roundtrip that Perry's lazy tape can
// memcpy without materializing, this one TOUCHES EVERY ELEMENT, which
// forces full materialization. The point is to show what happens when
// you can't avoid the work.

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

// Warmup — keeps JIT runtimes from charging us startup.
for (let i = 0; i < 3; i++) {
  const parsed = JSON.parse(blob);
  let warmSum = 0;
  for (let j = 0; j < parsed.length; j++) {
    warmSum += parsed[j].nested.x;
  }
  JSON.stringify(parsed);
}

const ITERATIONS = 50;
const start = Date.now();

let checksum = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
  const parsed = JSON.parse(blob);
  let sum = 0;
  for (let i = 0; i < parsed.length; i++) {
    sum += parsed[i].nested.x;
  }
  checksum += sum;
  const reStringified = JSON.stringify(parsed);
  checksum += reStringified.length;
}

const elapsed = Date.now() - start;
console.log("ms:" + elapsed);
console.log("checksum:" + checksum);
