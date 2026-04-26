// demonstrates: Toggle widget bound to State
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, Toggle, State } from "perry/ui"

const enabled = State(false)

App({
    title: "Toggle",
    width: 400,
    height: 200,
    body: VStack(12, [
        Toggle("Enable notifications", (on: boolean) => enabled.set(on)),
        Text(`Enabled: ${enabled.value}`),
    ]),
})
