// demonstrates: Issue #185 Phase C steps 2+3 — inline style: { ... } object
// on the Button constructor. Codegen destructures the trailing arg into
// a sequence of setter calls at HIR time. Mirrors React-style ergonomics
// while compiling to the same FFI as the verbose imperative pattern.
// docs: docs/src/ui/styling.md
// platforms: macos, linux, windows
// targets: ios-simulator, tvos-simulator, watchos-simulator, web, wasm, android

import { App, VStack, Button } from "perry/ui"

// Step 2 scalar props (number / string / boolean): borderRadius,
// borderWidth, opacity, fontSize, tooltip, hidden, enabled.
// Step 3 multi-arg props: backgroundColor / color / borderColor
// (PerryColor object literal), padding (single number OR per-side
// object), shadow ({color, blur, offsetX, offsetY}), textDecoration
// (string-literal). String colors and gradient land in step 4.
// ANCHOR: button-inline-full
const card = Button("Save", () => {
    console.log("saved")
}, {
    backgroundColor: { r: 0.231, g: 0.510, b: 0.965, a: 1.0 },
    borderColor: { r: 0.0, g: 0.0, b: 0.0, a: 0.1 },
    borderWidth: 1,
    borderRadius: 8,
    padding: 12,
    opacity: 0.95,
    shadow: {
        color: { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
        blur: 12,
        offsetX: 0,
        offsetY: 4,
    },
    tooltip: "Save the current document",
    enabled: true,
})
// ANCHOR_END: button-inline-full

App({
    title: "Inline Style Demo",
    width: 320,
    height: 240,
    body: VStack(16, [card]),
})
