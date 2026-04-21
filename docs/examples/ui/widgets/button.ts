// demonstrates: Button styling with buttonSet*/widgetSet* helpers
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: web, wasm

import {
    App,
    VStack,
    Button,
    buttonSetBordered,
    buttonSetContentTintColor,
    widgetSetEnabled,
    setCornerRadius,
} from "perry/ui"

const primary = Button("Click Me", () => console.log("Clicked!"))
buttonSetBordered(primary, 1)
buttonSetContentTintColor(primary, 1.0, 1.0, 1.0, 1.0)
setCornerRadius(primary, 8)

const disabled = Button("Can't click me", () => {})
widgetSetEnabled(disabled, 0)

App({
    title: "Button",
    width: 400,
    height: 200,
    body: VStack(12, [primary, disabled]),
})
