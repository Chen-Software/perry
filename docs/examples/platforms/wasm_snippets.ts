// demonstrates: per-API wasm/web snippets shown in docs/src/platforms/wasm.md
// docs: docs/src/platforms/wasm.md
// platforms: macos, linux, windows
// run: false

// `run: false` because the snippets here either declare external FFI
// functions whose host implementations live only in the browser bridge,
// or call `fetch()` against a non-existent telemetry endpoint. The harness
// still compiles + links the file on every PR, which catches API drift in
// `declare function`, `fetch()`'s options shape, and `parallelMap` (whose
// FFI lives in perry-runtime + perry-stdlib).

import { parallelMap } from "perry/thread"

// ANCHOR: ffi-declares
declare function bloom_init_window(w: number, h: number, title: number, fs: number): void
declare function bloom_draw_rect(x: number, y: number, w: number, h: number,
                                  r: number, g: number, b: number, a: number): void
// ANCHOR_END: ffi-declares

// Reference the declares so the codegen emits import lines into the WASM
// (otherwise the linker would dead-strip them on the native target).
function exerciseFfi(): void {
    bloom_init_window(800, 600, 0, 0)
    bloom_draw_rect(0, 0, 100, 100, 1.0, 0.0, 0.0, 1.0)
}

// ANCHOR: module-const
// telemetry.ts
const CHIRP_URL = 'https://api.chirp247.com/api/v1/event'
const API_KEY   = 'my-key'

export function trackEvent(event: string): void {
    fetch(CHIRP_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-Chirp-Key': API_KEY },
        body: JSON.stringify({ event }),
    })
}
// ANCHOR_END: module-const

// ANCHOR: web-worker-thread
function workerThreadDemo(): void {
    const numbers = [1, 2, 3, 4, 5, 6, 7, 8]
    const squares = parallelMap(numbers, (n: number) => n * n)
    console.log(`squares len=${squares.length}`)
}
// ANCHOR_END: web-worker-thread

// Drive the helpers so they aren't dead-stripped before the linker runs.
function _entrypoint(): void {
    if (false as boolean) exerciseFfi()
    if (false as boolean) trackEvent("hi")
    if (false as boolean) workerThreadDemo()
}
_entrypoint()
