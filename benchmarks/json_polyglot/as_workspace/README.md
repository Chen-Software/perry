# AssemblyScript + json-as workspace

This subdirectory holds the AssemblyScript implementation of the JSON
polyglot benchmarks. AS is a TypeScript-like language that compiles to
WebAssembly; it's the closest "TS-to-native peer" we could find that's
production-stable on the bench's workload.

## Setup

```bash
cd benchmarks/json_polyglot/as_workspace
npm install --save-dev assemblyscript @assemblyscript/wasi-shim json-as
brew install wasmtime  # the wasm runtime we run the .wasm on
```

## What's here

- `assembly/bench.ts` — JSON validate-and-roundtrip (10k records,
  50 iterations, parse → stringify → discard).
- `assembly/bench_field_access.ts` — JSON parse-and-iterate (parse
  → sum every record's nested.x → stringify).
- `asconfig.json` — extends `@assemblyscript/wasi-shim/asconfig.json`
  for `Date.now()` + `console.log` via WASI; pulls in
  `json-as/transform` for compile-time (de)serializer generation.

## Build + run

```bash
npx asc assembly/bench.ts              --target release \
    --outFile build/bench_rt.wasm
npx asc assembly/bench_field_access.ts --target release \
    --outFile build/bench_fa.wasm

wasmtime build/bench_rt.wasm
wasmtime build/bench_fa.wasm
```

The parent `run.sh` does this automatically.

## Scope caveat

AS is strictly typed — no `any`. The JS reference benchmarks
(`bench.ts` and `bench_field_access.ts` in the parent directory) use
`items: any[]` with heterogeneous member types. AS requires concrete
`Item` / `Nested` classes with `@json` decorators. The data shape is
identical (same fields, same types); the bench is closer in shape
to the Rust serde_json / Kotlin kotlinx.serialization rows than to
the dynamic-typing JS rows. This is documented in
`benchmarks/README.md`'s "Honest disclaimers" section.

## Why json-as

`json-as` (https://github.com/JairusSW/json-as) is the de facto
performant JSON library for AssemblyScript. It generates type-
specialized (de)serializers at compile time via a transform — same
conceptual approach as Rust's serde and Kotlin's kotlinx.serialization,
no runtime reflection. README claims SIMD support; we don't enable
the `--enable simd` flag so the numbers reflect the default build.
