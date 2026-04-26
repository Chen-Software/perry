// demonstrates: per-API overview snippets shown in docs/src/ui/overview.md
// docs: docs/src/ui/overview.md
// platforms: macos, linux, windows

import {
    App, onActivate, onTerminate,
    VStack, HStack, Text, Button,
    textSetFontSize, textSetColor,
} from "perry/ui"

// ANCHOR: app-shell
function runAppShell(): void {
    App({
        title: "Window Title",
        width: 800,
        height: 600,
        body: VStack(16, [
            Text("Content here"),
        ]),
    })
}
// ANCHOR_END: app-shell

// ANCHOR: lifecycle
onActivate(() => {
    console.log("App became active")
})

onTerminate(() => {
    console.log("App is closing")
})
// ANCHOR_END: lifecycle

// ANCHOR: widget-tree
function runWidgetTree(): void {
    App({
        title: "Layout Demo",
        width: 400,
        height: 300,
        body: VStack(16, [
            Text("Header"),
            HStack(8, [
                Button("Left", () => console.log("left")),
                Button("Right", () => console.log("right")),
            ]),
            Text("Footer"),
        ]),
    })
}
// ANCHOR_END: widget-tree

// ANCHOR: handle-modify
const label = Text("Hello")
textSetFontSize(label, 18)              // Modifies the native widget
textSetColor(label, 1.0, 0.0, 0.0, 1.0) // RGBA floats in [0,1]
// ANCHOR_END: handle-modify

// We can only call App() once per program; the snippets above gate it behind
// helper functions so the main App() call below is the one the harness
// observes (and exits after 500 ms in test mode).
App({
    title: "overview-snippets",
    width: 480,
    height: 320,
    body: VStack(8, [label, Text("compile-only run")]),
})
