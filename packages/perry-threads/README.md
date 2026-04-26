# @perryts/threads

**Drop-in parallel `map` / `filter` / `spawn` for browsers, Bun, and Node.js.** One API, one import, real worker-thread parallelism on all three — no bundler config, no platform branching, no native addon.

```ts
import { parallelMap } from '@perryts/threads';

const squared = await parallelMap(bigArray, (n) => n * n);   // uses all your cores
```

## Why

| | Speedup vs `Array.map` | Setup |
|---|---|---|
| Browsers | ~N× (N = cores) | none |
| Bun | **3.5×** on 4 workers, 10-core M-series | none |
| Node.js (≥18) | **3.4×** on 4 workers, 10-core M-series | none |

Measured on N=200 000 CPU-heavy items (see `test.mjs` in the repo).

- **Zero-config cross-runtime.** Uses the browser/Bun global `Worker` where available, transparently falls back to Node's `worker_threads` otherwise. Same code, same result.
- **Pay-for-what-you-use.** Arrays under 1024 elements run inline — worker startup overhead isn't worth it for small inputs, so the library skips it automatically.
- **Pooled workers.** The first call allocates the pool; subsequent calls reuse it. No per-call thread-spawn cost.
- **Order preserved.** Chunks are reassembled in input order for both `parallelMap` and `parallelFilter`.

## Install

```bash
npm install @perryts/threads
```

Requires Node ≥ 18. No runtime dependencies.

## Usage

### `parallelMap(data, fn, options?)`

```ts
import { parallelMap } from '@perryts/threads';

const nums = Array.from({ length: 1_000_000 }, (_, i) => i);
const squared = await parallelMap(nums, (n) => n * n);
```

Pass context for values the worker function needs:

```ts
const factor = 7;
const out = await parallelMap(
  nums,
  (n, ctx) => n * ctx.factor,
  { context: { factor } },
);
```

### `parallelFilter(data, fn, options?)`

```ts
import { parallelFilter } from '@perryts/threads';

const evens = await parallelFilter(nums, (n) => n % 2 === 0);
```

### `spawn(fn, options?)`

Run a single function on a background worker:

```ts
import { spawn } from '@perryts/threads';

const result = await spawn(
  (ctx) => heavyCompute(ctx.input),
  { context: { input: bigPayload } },
);
```

## Options

```ts
interface ThreadOptions<C = unknown> {
  /** Passed as the second argument to the worker function. Structured-cloned to each worker. */
  context?: C;
  /** Number of workers. Defaults to navigator.hardwareConcurrency or os.cpus().length. */
  concurrency?: number;
}
```

## Important: function serialization

Worker functions are serialized via `fn.toString()` and re-parsed inside each worker — **they must be self-contained**. Closure captures don't survive. Pass anything the function needs through `context`:

```ts
// WRONG — `multiplier` is undefined inside the worker
const multiplier = 3;
await parallelMap(arr, (n) => n * multiplier);

// RIGHT — pass via context
await parallelMap(arr, (n, ctx) => n * ctx.m, { context: { m: 3 } });
```

## When NOT to use this

- **Small arrays.** Below ~1000 items, `.map` is faster. (The library detects this and runs inline — you don't have to branch yourself.)
- **I/O-bound work.** Workers help with CPU-bound code. For HTTP fetches, DB calls, etc., use `Promise.all` on the main thread.
- **Shared mutable state.** Workers communicate via message passing (structured clone). If you need `SharedArrayBuffer` or `Atomics`, this isn't the library.

## How it works

One public API, two backends picked at runtime:

- **Browsers & Bun** — global `Worker` + `Blob` URL
- **Node.js (≥18)** — `worker_threads.Worker` with an inline script shim that adapts `parentPort` to browser-style `self.onmessage` / `self.postMessage`, so the worker body is identical across backends

Browser bundlers may flag the `require('worker_threads')` fallback as an unresolved import. It's inside a `try { … } catch {}` and gated on `typeof require === 'function'`, so it's safe to mark `worker_threads` as external or ignore the warning.

## License

MIT
