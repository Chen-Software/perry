// demonstrates: Geisterhand-targetable Perry UI app — every common widget
// docs: docs/src/testing/geisterhand.md
// platforms: macos, linux, windows

// A complete Perry UI app exercising every widget type Geisterhand
// (the UI fuzzer) can interact with. The doc-tests harness compiles
// and runs this on every PR, so the snippet on the docs page can never
// drift from the real perry/ui API.

import {
    App, VStack, HStack,
    Text, Button, TextField, Slider, Toggle, Picker,
    State, stateOnChange,
    pickerAddItem,
    textSetString,
} from "perry/ui"

// State for reactive UI
const counterState = State(0)
const textState = State("")

// Labels
const title = Text("Geisterhand Demo")
const counterLabel = Text("Count: 0")

// Bind counter state to label via the free-function listener
stateOnChange(counterState, (val: number) => {
    textSetString(counterLabel, `Count: ${val}`)
})

// Button — widget_type = 0
const incrementBtn = Button("Increment", () => {
    counterState.set(counterState.value + 1)
})
const resetBtn = Button("Reset", () => {
    counterState.set(0)
})

// TextField(placeholder, onChange) — widget_type = 1
const nameField = TextField("Enter your name", (text: string) => {
    textState.set(text)
    console.log(`Name: ${text}`)
})

// Slider(min, max, onChange) — widget_type = 2
const volumeSlider = Slider(0, 100, (value: number) => {
    console.log(`Volume: ${value}`)
})

// Toggle(label, onChange) — widget_type = 3
const darkModeToggle = Toggle("Dark Mode", (on: boolean) => {
    console.log(`Dark mode: ${on}`)
})

// Picker(onChange); items added with pickerAddItem.
const sizePicker = Picker((index: number) => {
    console.log(`Size index: ${index}`)
})
pickerAddItem(sizePicker, "Small")
pickerAddItem(sizePicker, "Medium")
pickerAddItem(sizePicker, "Large")

// Layout
const buttonRow = HStack(8, [incrementBtn, resetBtn])
const stack = VStack(12, [
    title, counterLabel, buttonRow,
    nameField, volumeSlider, darkModeToggle, sizePicker,
])

App({
    title: "Geisterhand Demo",
    width: 400,
    height: 480,
    body: stack,
})
