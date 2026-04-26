// demonstrates: perry/system APIs for the macOS docs page
// docs: docs/src/platforms/macos.md
// platforms: macos, linux, windows
// run: false

// `run: false` so the harness compiles + links but doesn't actually open
// a URL or mutate NSUserDefaults during the test. The drift-protection
// guarantee is "every symbol resolves and the call shape compiles."
//
// We import a minimal `App` from `perry/ui` so the linker pulls in
// libperry_ui_macos.a (and the equivalent UI lib on Linux/Windows) —
// that's where the `perry_system_*` FFI symbols actually live.

import { App, VStack, Text } from "perry/ui"

// ANCHOR: macos-system
import { openURL, isDarkMode, preferencesSet, preferencesGet } from "perry/system"

openURL("https://example.com")          // Opens in default browser
const dark = isDarkMode()               // Check appearance
preferencesSet("key", "value")          // NSUserDefaults
const val = preferencesGet("key")       // NSUserDefaults
// ANCHOR_END: macos-system

// Reference the values so the optimizer can't elide the calls above.
console.log(`darkMode=${dark} val=${val}`)

// Compile-only sanity body so the file is a runnable program when needed.
App({
    title: "macos-system-snippets",
    width: 320,
    height: 200,
    body: VStack(8, [Text("compile-only example for docs/src/platforms/macos.md")]),
})
