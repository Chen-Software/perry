// demonstrates: minimal iOS App() lifecycle snippet from the iOS docs page
// docs: docs/src/platforms/ios.md
// platforms: macos, linux, windows
// targets: ios-simulator

// On iOS, `App()` triggers `UIApplicationMain` and the render function fires
// from `PerryAppDelegate` once the app is ready. The same source compiles on
// macOS (AppKit), Linux (GTK4), and Windows (Win32) — only the message text
// changes, but the API surface is identical, which is the point of this
// snippet in the iOS docs.

// ANCHOR: ios-app
import { App, Text, VStack } from "perry/ui"

App({
    title: "My iOS App",
    width: 400,
    height: 800,
    body: VStack(16, [
        Text("Hello, iPhone!"),
    ]),
})
// ANCHOR_END: ios-app
