// demonstrates: counter from getting-started/first-app.md with styled widgets
// docs: docs/src/getting-started/first-app.md
// platforms: macos, linux, windows
// targets: ios-simulator, tvos-simulator, watchos-simulator, web, wasm
// run: false

import {
    App, VStack, Text, Button, State,
    textSetFontSize, textSetColor,
    setCornerRadius, setPadding,
    widgetSetBackgroundColor,
} from "perry/ui"

const count = State(0)

const label = Text(`Count: ${count.value}`)
textSetFontSize(label, 24)
textSetColor(label, 0.2, 0.2, 0.2, 1.0)        // RGBA in [0,1] — same as #333333

const btn = Button("Increment", () => count.set(count.value + 1))
setCornerRadius(btn, 8)
widgetSetBackgroundColor(btn, 0.0, 0.478, 1.0, 1.0)  // system blue

const stack = VStack(20, [label, btn])
setPadding(stack, 20, 20, 20, 20)

App({
    title: "Styled Counter",
    width: 400,
    height: 300,
    body: stack,
})
