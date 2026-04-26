// demonstrates: Text widget styling with the real free-function API
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, textSetFontSize, textSetFontWeight, textSetColor, textSetFontFamily } from "perry/ui"

const label = Text("Hello, World!")
textSetFontSize(label, 18)
textSetColor(label, 0.2, 0.2, 0.2, 1.0) // RGBA in [0, 1]
textSetFontFamily(label, "Menlo")

const bold = Text("Bold")
textSetFontWeight(bold, 20, 1.0)

App({
    title: "Text",
    width: 400,
    height: 200,
    body: VStack(8, [label, bold]),
})
