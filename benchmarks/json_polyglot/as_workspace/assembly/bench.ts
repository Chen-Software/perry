// JSON validate-and-roundtrip polyglot benchmark — AssemblyScript
// (compiled to wasm, run via wasmtime).
//
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → stringify → discard.
//
// AssemblyScript is a TypeScript-like language that compiles to
// WebAssembly. For JSON we use json-as (https://github.com/JairusSW/json-as),
// which generates type-specialized (de)serializers at compile time
// via a transform — same conceptual approach as Rust's serde and
// Kotlin's kotlinx.serialization, no runtime reflection.
//
// Workload caveat: AssemblyScript is strictly typed (no `any`). The
// JS reference benchmarks use `items: any[]` with heterogeneous
// member types; AS requires a concrete `Item` class. The ITEM SHAPE
// is the same — same fields, same types — but the bench is closer
// in shape to the Rust serde_json / Kotlin kotlinx.serialization
// rows than to the dynamic-typing JS rows. This is documented as a
// scope note in benchmarks/README.md's "Honest disclaimers" section.

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
    JSON.stringify(parsed);
}

const ITERATIONS = 50;
const start: i64 = Date.now();

let checksum: i64 = 0;
for (let iter = 0; iter < ITERATIONS; iter++) {
    const parsed = JSON.parse<Item[]>(blob);
    checksum += parsed.length;
    const reStringified = JSON.stringify(parsed);
    checksum += reStringified.length;
}

const elapsed: i64 = Date.now() - start;
console.log("ms:" + elapsed.toString());
console.log("checksum:" + checksum.toString());
