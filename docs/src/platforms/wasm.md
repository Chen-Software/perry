# WebAssembly

Perry can compile TypeScript to WebAssembly using `--target wasm`.

## Building

```bash
# Self-contained HTML (default)
perry app.ts -o app --target wasm
open app.html

# Raw .wasm binary
perry app.ts -o app.wasm --target wasm
```

The default output is a single `.html` file containing a base64-embedded WASM binary and a JavaScript runtime bridge. If the output path ends with `.wasm`, a raw WASM binary is produced instead.

## How It Works

The `perry-codegen-wasm` crate compiles HIR directly to WASM bytecode using `wasm-encoder`. Unlike `--target web` (which emits JavaScript), this target produces real WebAssembly with a thin JS bridge for host APIs like `console.log` and string operations.

The NaN-boxing scheme matches the native perry-runtime — f64 values with STRING_TAG/POINTER_TAG — so the same value representation is used across native and WASM targets.

## Supported Features

- **Functions**: definitions, calls, parameters, return values
- **Control flow**: `if`/`else`, `while`, `for`, `switch`, `break`, `continue`, `try`/`catch`/`finally`
- **Data types**: numbers (f64), strings, booleans, `undefined`, `null`
- **Operators**: arithmetic, comparison, logical, unary, update (`++`/`--`)
- **String operations**: literals, concatenation, `charAt`, `substring`, `indexOf`, `slice`, `toLowerCase`, `toUpperCase`, `trim`, `includes`, `startsWith`, `endsWith`, `replace`, `split`, `.length`
- **Math**: `Math.floor`, `Math.ceil`, `Math.round`, `Math.abs`, `Math.sqrt`, `Math.pow`, `Math.min`, `Math.max`, `Math.log`, `Math.random`
- **Console**: `console.log()`, `console.warn()`, `console.error()`
- **Type operations**: `typeof`, `parseInt`, `parseFloat`
- **Other**: template literals, conditional expressions, `Date.now()`

## JavaScript Runtime Bridge

The WASM binary imports ~25 JavaScript functions for host interop:

- **Strings**: creation, concatenation, comparison, method dispatch
- **Console**: output formatting with NaN-boxed value conversion
- **Math**: delegation to `Math.*` built-ins
- **Memory**: access via `WebAssembly.Memory` buffer

Strings are managed via a global string table in JavaScript, with IDs passed as NaN-boxed values to and from WASM.

## FFI Support

The WASM target supports external FFI functions declared with `declare function` (no body). These are compiled as WASM imports under the `"ffi"` namespace, allowing native libraries like [Bloom Engine](https://bloomengine.dev) to provide GPU rendering, audio, and other platform APIs to WASM code.

```typescript
// These become WASM imports under the "ffi" namespace
declare function bloom_init_window(w: number, h: number, title: number, fs: number): void;
declare function bloom_draw_rect(x: number, y: number, w: number, h: number,
                                  r: number, g: number, b: number, a: number): void;
```

The host provides these imports when instantiating the WASM module:

```javascript
// Via __ffiImports global (set before boot)
globalThis.__ffiImports = { bloom_init_window: ..., bloom_draw_rect: ... };

// Or via bootPerryWasm second argument
await bootPerryWasm(wasmBase64, { bloom_init_window: ..., bloom_draw_rect: ... });
```

Void FFI functions automatically push `TAG_UNDEFINED` onto the WASM stack to satisfy expression contexts.

## Limitations

Current limitations:

- No UI widgets (`perry/ui` is not available)
- Switch statements use cascading if/else (no WASM table jumps)

## Minification

Use `--minify` to minify the JavaScript runtime bridge in the HTML output:

```bash
perry app.ts -o app --target wasm --minify
```

## Example

```typescript
function fibonacci(n: number): number {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

console.log(fibonacci(10)); // 55
```

```bash
perry fib.ts -o fib --target wasm
# Produces fib.html — open in any browser
```

## Next Steps

- [Web](web.md) — JavaScript target (full UI support)
- [Platform Overview](overview.md) — All platforms
