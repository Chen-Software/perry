// demonstrates: ProgressView as an indeterminate spinner
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, ProgressView } from "perry/ui"

App({
    title: "ProgressView",
    width: 400,
    height: 200,
    body: VStack(12, [
        Text("Loading..."),
        ProgressView(),
    ]),
})
