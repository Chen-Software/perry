// demonstrates: file-open / save dialogs wired to a tiny text editor
// docs: docs/src/ui/dialogs.md
// platforms: macos, linux, windows

import {
    App,
    VStack, HStack,
    Text, Button, TextField,
    State,
    openFileDialog, saveFileDialog, alert,
} from "perry/ui"
import { readFileSync, writeFileSync } from "fs"

const content = State("")
const filePath = State("")

App({
    title: "Text Editor",
    width: 800,
    height: 600,
    body: VStack(12, [
        HStack(8, [
            Button("Open", () => {
                openFileDialog((path: string) => {
                    if (path.length === 0) return
                    filePath.set(path)
                    content.set(readFileSync(path, "utf-8") as string)
                })
            }),
            Button("Save As", () => {
                saveFileDialog((path: string) => {
                    if (path.length === 0) return
                    writeFileSync(path, content.value)
                    filePath.set(path)
                    alert("Saved", `File saved to ${path}`)
                }, "untitled", "txt")
            }),
        ]),
        Text(filePath.value === "" ? "No file open" : `File: ${filePath.value}`),
        TextField("Start typing...", (value: string) => content.set(value)),
    ]),
})
