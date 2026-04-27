// demonstrates: drop shadow on a Button via widgetSetShadow (issue #185 Phase B)
// docs: docs/src/ui/styling.md
// platforms: macos, linux, windows
// targets: ios-simulator, tvos-simulator, watchos-simulator, web, wasm, android

// Phase B shadow closure complete across all 9 platforms (v0.5.296 →
// v0.5.298): Apple via CALayer.shadow*, Web via CSS `box-shadow`, GTK4
// via CSS `box-shadow` (per-handle CssProvider), Android via Material
// `setElevation` (+ API 28+ outline-shadow color tinting). Windows
// stores shadow params but rendering is deferred to a follow-up
// (DirectComposition or custom WM_PAINT pass) — see the matrix entry
// for Windows status.

import { App, VStack, Button, widgetSetShadow, widgetSetBackgroundColor, setCornerRadius, setPadding } from "perry/ui"

const card = Button("Tap me", () => {
    console.log("tapped")
})
widgetSetBackgroundColor(card, 0.95, 0.95, 0.97, 1.0)
setCornerRadius(card, 12)
setPadding(card, 16, 24, 16, 24)
// (r, g, b, a, blur, offset_x, offset_y) — same shape as HTML
// `box-shadow: <offset_x> <offset_y> <blur> rgba(...)`. Black at 25%
// opacity, 12pt blur, 4pt down — a stock Material-style elevation.
widgetSetShadow(card, 0.0, 0.0, 0.0, 0.25, 12.0, 0.0, 4.0)

App({
    title: "Shadow Demo",
    width: 320,
    height: 240,
    body: VStack(16, [card]),
})
