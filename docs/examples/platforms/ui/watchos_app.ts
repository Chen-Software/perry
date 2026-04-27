// demonstrates: minimal watchOS App() lifecycle snippet from the watchOS docs page
// docs: docs/src/platforms/watchos.md
// platforms: macos, linux, windows

// On watchOS this lowers through the data-driven SwiftUI bridge (see
// `PerryWatchApp.swift`). On macOS / Linux / Windows it lowers through the
// usual native UI library — same TypeScript API, different backends.

// ANCHOR: watchos-app
import { App, Text, VStack, Button } from "perry/ui"

App({
    title: "My Watch App",
    width: 200,
    height: 200,
    body: VStack(8, [
        Text("Hello, Apple Watch!"),
        Button("Tap me", () => {
            console.log("Button tapped!")
        }),
    ]),
})
// ANCHOR_END: watchos-app
