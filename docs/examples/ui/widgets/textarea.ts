// demonstrates: TextArea for multi-line input
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, TextArea, State } from "perry/ui"

const content = State("")

App({
    title: "TextArea",
    width: 500,
    height: 400,
    body: VStack(12, [
        TextArea("Enter multi-line text...", (value: string) => content.set(value)),
        Text(`Length: ${content.value.length}`),
    ]),
})
