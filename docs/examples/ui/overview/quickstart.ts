// demonstrates: the smallest complete Perry UI app
// docs: docs/src/ui/overview.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, Text, VStack } from "perry/ui"

App({
    title: "My App",
    width: 400,
    height: 300,
    body: VStack(16, [
        Text("Hello from Perry!"),
    ]),
})
