# Workload 1 — JSON pipeline

Read a JSON array, apply a filter + add 2 derived fields, serialize back,
write output. Designed to stress allocation and (for Perry) the GC.

## Pipeline

```
read input.json
  -> JSON.parse
  -> filter: active === true
  -> build fresh object per kept record with all source fields PLUS:
       display_name: name.toUpperCase()
       age_group:    age<30?"young":age<50?"mid":"senior"
  -> JSON.stringify
  -> write output.json
```

Each implementation prints:

```
input_bytes=<N> records_in=<N> records_out=<N> output_bytes=<N> hash=<FNV-1a-32 hex>
```

## Two scales

| Fixture | Records | Input | Runs on |
|---------|---------|-------|---------|
| `assets/input_small.json` | 100 | 21 KB | Rust, Zig, Perry |
| `assets/input.json` | 500,000 | 108 MB | Rust, Zig only |

Perry is run on the small fixture only. On the 108 MB fixture it hits a
cluster of size-dependent bugs in its JSON implementation / GC:

1. **Iteration drops records.** Iterating over a 500-record+ `JSON.parse`
   result and touching `.fields` on each element causes records to silently
   vanish mid-iteration — the GC-scan for parser-allocated records isn't
   covering everything the user code can reach.
2. **`JSON.stringify` panics at ~1000+ records** with
   `"byte index N is not a char boundary"` inside
   `perry-runtime/src/json.rs:427` — reading already-corrupted string
   payloads.
3. **Variable-sized `Buffer.alloc(N)` in function params** triggers a
   GC-root-scan gap; bulk-filling routines need their buffers hoisted to
   module-level globals. (This one was worked around in the image
   convolution; it's orthogonal to the JSON bugs.)

On the 100-record fixture Perry produces byte-identical output to Rust and
Zig. The report tables record this explicitly.

## Libraries

- **Rust**: `serde_json` with derive'd `Deserialize` / `Serialize` structs.
- **Zig**: `std.json.parseFromSlice` + `std.json.Stringify.valueAlloc`.
- **Perry**: `JSON.parse` / `JSON.stringify` + `fs.readFileSync(path,
  'utf8')` / `fs.writeFileSync`.

## What we measure

- Wall time (parse + transform + serialize + file I/O)
- Peak RSS
- Binary size
- Correctness: Rust + Zig must produce byte-identical output (FNV-1a-32
  hashes match). Perry must match on the small fixture.
