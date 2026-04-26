// demonstrates: Slider with a numeric range
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, Slider, State } from "perry/ui"

const value = State(50)

App({
    title: "Slider",
    width: 400,
    height: 200,
    body: VStack(12, [
        Slider(0, 100, (v: number) => value.set(v)),
        Text(`Value: ${value.value}`),
    ]),
})
