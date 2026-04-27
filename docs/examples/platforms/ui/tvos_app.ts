// demonstrates: minimal tvOS App() lifecycle snippet from the tvOS docs page
// docs: docs/src/platforms/tvos.md
// platforms: macos, linux, windows
// targets: tvos-simulator

// tvOS uses UIKit and the focus engine, so `App()` here uses the same shape
// as the iOS snippet — the underlying lifecycle is `UIApplicationMain` but
// the focus engine handles focus/highlight/select for the Siri Remote.

// ANCHOR: tvos-app
import { App, Text, VStack } from "perry/ui"

App({
    title: "My TV App",
    width: 1920,
    height: 1080,
    body: VStack(16, [
        Text("Hello, Apple TV!"),
    ]),
})
// ANCHOR_END: tvos-app
