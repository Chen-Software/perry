// demonstrates: Section grouping with widgetAddChild (no Form widget in Perry)
// docs: docs/src/ui/widgets.md
// platforms: macos, linux, windows
// targets: ios-simulator, web, wasm

import {
    App,
    VStack,
    Section,
    TextField,
    Toggle,
    State,
    widgetAddChild,
} from "perry/ui"

const name = State("")
const notifications = State(true)

const personal = Section("Personal Info")
widgetAddChild(personal, TextField("Name", (value: string) => name.set(value)))

const settings = Section("Settings")
widgetAddChild(
    settings,
    Toggle("Notifications", (on: boolean) => notifications.set(on)),
)

App({
    title: "Sections",
    width: 500,
    height: 400,
    body: VStack(16, [personal, settings]),
})
