// demonstrates: click + hover + double-click + keyboard shortcut all wired to
// a single State-backed status label
// docs: docs/src/ui/events.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import {
    App,
    Text,
    Button,
    VStack,
    State,
    Spacer,
    addKeyboardShortcut,
    widgetSetOnHover,
    widgetSetOnDoubleClick,
} from "perry/ui"

const lastEvent = State("No events yet")

// Cmd+R (modifiers: 1 = Cmd/Ctrl).
addKeyboardShortcut("r", 1, () => {
    lastEvent.set("Keyboard: Cmd+R")
})

const hoverBtn = Button("Hover me", () => {})
widgetSetOnHover(hoverBtn, () => {
    lastEvent.set("Hover fired")
})

const dblLabel = Text("Double-click me")
widgetSetOnDoubleClick(dblLabel, () => {
    lastEvent.set("Double-clicked!")
})

App({
    title: "Events Demo",
    width: 400,
    height: 300,
    body: VStack(16, [
        Text(`Last event: ${lastEvent.value}`),
        Spacer(),
        Button("Click me", () => {
            lastEvent.set("Button clicked")
        }),
        hoverBtn,
        dblLabel,
    ]),
})
