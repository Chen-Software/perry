// demonstrates: TextField + two-way binding via stateBindTextfield
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, TextField, State, stateBindTextfield } from "perry/ui"

const text = State("")
const field = TextField("Placeholder...", (value: string) => text.set(value))
stateBindTextfield(text, field) // programmatic text.set() also updates the field

App({
    title: "TextField",
    width: 400,
    height: 200,
    body: VStack(12, [
        field,
        Text(`You typed: ${text.value}`),
    ]),
})
