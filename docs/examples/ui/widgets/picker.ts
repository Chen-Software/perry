// demonstrates: Picker with items added via pickerAddItem
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, Text, Picker, State, pickerAddItem } from "perry/ui"

const selected = State(0)
const picker = Picker((index: number) => selected.set(index))
pickerAddItem(picker, "Option A")
pickerAddItem(picker, "Option B")
pickerAddItem(picker, "Option C")

App({
    title: "Picker",
    width: 400,
    height: 200,
    body: VStack(12, [
        picker,
        Text(`Selected index: ${selected.value}`),
    ]),
})
