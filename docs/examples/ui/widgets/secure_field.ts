// demonstrates: SecureField for password input
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import { App, VStack, SecureField, State } from "perry/ui"

const password = State("")

App({
    title: "SecureField",
    width: 400,
    height: 200,
    body: VStack(12, [
        SecureField("Enter password...", (value: string) => password.set(value)),
    ]),
})
