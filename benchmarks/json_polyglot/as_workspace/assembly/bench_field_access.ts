// JSON parse-and-iterate polyglot benchmark — AssemblyScript
// (compiled to wasm, run via wasmtime).
//
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// IDENTICAL workload to bench_field_access.{ts,go,rs,swift,cpp,kt}
// in the parent directory. See bench.ts for the typed-vs-`any` scope
// caveat.

import { JSON } from "json-as";

@json
class Nested {
    x: i32 = 0;
    y: i32 = 0;
}

@json
class Item {
    id: i32 = 0;
    name: string = "";
    value: f64 = 0.0;
    tags: string[] = [];
    nested: Nested = new Nested();
}

const items: Item[] = [];
for (let i = 0; i < 10000; i++) {
    const it = new Item();
    it.id = i;
    it.name = "item_" + i.toString();
    it.value = (i as f64) * 3.14159;
    it.tags = ["tag_" + (i % 10).toString(), "tag_" + (i % 5).toString()];
    const n = new Nested();
    n.x = i;
    n.y = i * 2;
    it.nested = n;
    items.push(it);
}
const blob: string = JSON.stringify(items);

// Warmup
for (let i = 0; i < 3; i++) {
    const parsed = JSON.parse<Item[]>(blob);
    let warmSum: i64 = 0;
    for (let j = 0; j < parsed.length; j++) {
        warmSum += parsed[j].nested.x;
    }
    JSON.stringify(parsed);
}

const ITERATIONS = 50;
const start: i64 = Date.now();

let checksum: i64 = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
    const parsed = JSON.parse<Item[]>(blob);
    let sum: i64 = 0;
    for (let i = 0; i < parsed.length; i++) {
        sum += parsed[i].nested.x;
    }
    checksum += sum;
    const reStringified = JSON.stringify(parsed);
    checksum += reStringified.length;
}

const elapsed: i64 = Date.now() - start;
console.log("ms:" + elapsed.toString());
console.log("checksum:" + checksum.toString());
