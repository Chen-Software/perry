// demonstrates: Issue #185 Phase C step 5 — inline style: { ... } on
// VStack and HStack containers. Style arg lands AFTER the children
// array (so the variadic-children parser doesn't have to disambiguate).
// Both `VStack(children, style)` and `VStack(spacing, children, style)`
// shapes work — codegen detects whether args[0] is an array vs number
// and offsets the style position accordingly.
// docs: docs/src/ui/styling.md
// platforms: macos, linux, windows
// targets: ios-simulator, tvos-simulator, watchos-simulator, web, wasm, android

import { App, VStack, HStack, Text, Button } from "perry/ui"

// ANCHOR: stack-inline-full
// VStack with explicit spacing AND inline style — children + style.
const card = VStack(8, [
    Text("Heading"),
    Text("Subtitle"),
    Button("Action", () => { console.log("clicked") }),
], {
    backgroundColor: { r: 0.96, g: 0.97, b: 0.99, a: 1.0 },
    borderRadius: 12,
    padding: 16,
    shadow: {
        color: { r: 0.0, g: 0.0, b: 0.0, a: 0.1 },
        blur: 8,
        offsetY: 2,
    },
})

// HStack with no explicit spacing (children-array first form) + style.
const toolbar = HStack([
    Text("Left"),
    Text("Right"),
], {
    backgroundColor: { r: 0.2, g: 0.2, b: 0.2, a: 1.0 },
    padding: { top: 8, right: 16, bottom: 8, left: 16 },
    borderRadius: 6,
})
// ANCHOR_END: stack-inline-full

App({
    title: "Stack inline style",
    width: 400,
    height: 320,
    body: VStack(16, [card, toolbar]),
})
