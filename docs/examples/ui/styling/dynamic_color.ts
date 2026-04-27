// demonstrates: Issue #185 Phase C step 7 — runtime parseColor for
// non-literal color values. Compile-time string parsing handles the
// common case (`backgroundColor: "#3B82F6"`); this exercises the
// runtime fallback for `backgroundColor: someStringVar` patterns
// where the color comes from a variable resolved at runtime.
// docs: docs/src/ui/styling.md
// platforms: macos, linux, windows
// targets: ios-simulator, tvos-simulator, watchos-simulator, web, wasm, android

import { App, VStack, Button } from "perry/ui"

// Color held in a runtime variable — codegen can't constant-fold it.
const themeColor = "#3B82F6"
const dangerColor = "red"

const primary = Button("Primary", () => {}, {
    backgroundColor: themeColor,    // runtime fallback path
    color: "white",                  // compile-time literal still works
    borderRadius: 8,
    padding: 12,
})

const danger = Button("Delete", () => {}, {
    backgroundColor: dangerColor,    // runtime fallback path
    color: "white",
    borderRadius: 8,
    padding: 12,
})

App({
    title: "Dynamic color demo",
    width: 320,
    height: 240,
    body: VStack(16, [primary, danger]),
})
