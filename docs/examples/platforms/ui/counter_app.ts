// demonstrates: counter App with reactive State — shared between watchOS and wasm/web docs
// docs: docs/src/platforms/watchos.md, docs/src/platforms/wasm.md
// platforms: macos, linux, windows
// targets: watchos-simulator, web

// Identical TypeScript drives the data-driven SwiftUI renderer on watchOS,
// the DOM bridge on web/wasm, and the AppKit/GTK4/Win32 backends on the
// host. The same anchor block is included from both platform pages — if the
// counter API drifts, both pages flag at once.

// ANCHOR: counter
import { App, Text, VStack, Button, State } from "perry/ui"

const count = State(0)

App({
    title: "Counter",
    width: 400,
    height: 300,
    body: VStack(16, [
        Text(`Count: ${count.value}`),
        Button("Increment", () => count.set(count.value + 1)),
    ]),
})
// ANCHOR_END: counter
