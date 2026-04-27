# Other Modules

Additional npm packages and Node.js APIs supported by Perry.

## sharp (Image Processing)

The `sharp` runtime functions are declared (`js_sharp_resize`, `js_sharp_blur`,
`js_sharp_to_buffer`, etc.) but the user-facing dispatch from
`import sharp from "sharp"; sharp(input).resize(...)` is not wired into the
LLVM backend yet. Track the follow-up at issue #200.

```text
import sharp from "sharp";

await sharp("input.jpg")
  .resize(300, 200)
  .toFile("output.png");
```

## cheerio (HTML Parsing)

The `cheerio` runtime exists (see `crates/perry-stdlib/src/cheerio.rs`) but
the dispatch path is not wired yet — track at issue #200.

```text
import cheerio from "cheerio";

const html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
const $ = cheerio.load(html);
console.log($("h1").text()); // "Hello"
```

## nodemailer (Email)

```typescript
{{#include ../../examples/stdlib/other/snippets.ts:nodemailer}}
```

## zlib (Compression)

The `zlib` runtime exists but dispatch from `import zlib from "zlib"` is not
wired yet — track at issue #200.

```text
import zlib from "zlib";

const compressed = zlib.gzipSync("Hello, World!");
const decompressed = zlib.gunzipSync(compressed);
```

## cron (Job Scheduling)

The `cron` runtime exists but dispatch from `import { CronJob } from "cron"`
is not wired yet — track at issue #200.

```text
import { CronJob } from "cron";

const job = new CronJob("*/5 * * * *", () => {
  console.log("Runs every 5 minutes");
});
job.start();
```

## worker_threads

The `worker_threads` API is partially recognized at HIR-lowering time
(`parentPort` / `Worker` shapes) but full dispatch is incomplete. For
data-parallel work today, prefer `parallelMap` / `parallelFilter` /
`spawn` from `perry/thread` (see [Threading](../threading/overview.md)).

```text
import { Worker, parentPort, workerData } from "worker_threads";

if (parentPort) {
  // Worker thread
  const data = workerData;
  parentPort.postMessage({ result: data.value * 2 });
} else {
  // Main thread
  const worker = new Worker("./worker.ts", {
    workerData: { value: 21 },
  });
  worker.on("message", (msg) => {
    console.log(msg.result); // 42
  });
}
```

## commander (CLI Parsing)

```typescript
{{#include ../../examples/stdlib/other/snippets.ts:commander}}
```

## decimal.js (Arbitrary Precision)

```typescript
{{#include ../../examples/stdlib/other/snippets.ts:decimal}}
```

## lru-cache

The wired constructor takes the npm v7+ options-object shape
(`new LRUCache({ max: 100 })`) — the older positional form
`new LRUCache(100)` falls through to a `max=100` default.

```typescript
{{#include ../../examples/stdlib/other/snippets.ts:lru-cache}}
```

## child_process

```typescript
{{#include ../../examples/stdlib/other/snippets.ts:child-process}}
```

## Next Steps

- [Overview](overview.md) — All stdlib modules
- [File System](fs.md) — fs and path APIs
