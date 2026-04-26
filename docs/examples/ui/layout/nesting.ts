// demonstrates: nested VStack/HStack + Spacer + Divider
// docs: docs/src/ui/layout.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, HStack, Text, Button, Spacer, Divider } from "perry/ui"

App({
    title: "Layout Example",
    width: 800,
    height: 600,
    body: VStack(16, [
        // Header
        HStack(8, [
            Text("My App"),
            Spacer(),
            Button("Settings", () => {}),
        ]),
        Divider(),
        // Content
        VStack(12, [
            Text("Welcome!"),
            HStack(8, [
                Button("Action 1", () => {}),
                Button("Action 2", () => {}),
            ]),
        ]),
        Spacer(),
        // Footer
        Text("v1.0.0"),
    ]),
})
