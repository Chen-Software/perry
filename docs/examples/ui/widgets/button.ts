// demonstrates: Button styling with buttonSet*/widgetSet* helpers
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import {
    App,
    VStack,
    Button,
    buttonSetBordered,
    widgetSetEnabled,
    setCornerRadius,
} from "perry/ui"

// Note: buttonSetContentTintColor is macOS/iOS-only (maps to NSButton /
// UIButton tint). GTK4/Win32 don't have an equivalent — set
// widgetSetBackgroundColor(btn, r, g, b, a) there instead.
const primary = Button("Click Me", () => console.log("Clicked!"))
buttonSetBordered(primary, 1)
setCornerRadius(primary, 8)

const disabled = Button("Can't click me", () => {})
widgetSetEnabled(disabled, 0)

App({
    title: "Button",
    width: 400,
    height: 200,
    body: VStack(12, [primary, disabled]),
})
