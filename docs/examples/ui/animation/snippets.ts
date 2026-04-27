// demonstrates: per-API animation snippets shown in docs/src/ui/animation.md
// docs: docs/src/ui/animation.md
// platforms: macos, linux, windows

import { App, Text, Button, VStack } from "perry/ui"

// ANCHOR: opacity
const fading = Text("Fading text")
// Animate from the widget's current opacity to `target` over `durationSecs`.
fading.animateOpacity(1.0, 0.3) // target, durationSeconds
// ANCHOR_END: opacity

// ANCHOR: position
const moving = Button("Moving", () => {})
// Animate by a delta (dx, dy) relative to the widget's current position.
moving.animatePosition(100, 200, 0.5) // dx, dy, durationSeconds
// ANCHOR_END: position

App({
    title: "animation-snippets",
    width: 400,
    height: 300,
    body: VStack(16, [fading, moving]),
})
