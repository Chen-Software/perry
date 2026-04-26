// demonstrates: per-API dialog snippets shown in docs/src/ui/dialogs.md
// docs: docs/src/ui/dialogs.md
// platforms: macos, linux, windows

import {
    App,
    VStack, HStack,
    Text, Button,
    State,
    alert, alertWithButtons,
    openFileDialog, openFolderDialog, saveFileDialog,
    sheetCreate, sheetPresent, sheetDismiss,
} from "perry/ui"

// We don't actually invoke the dialogs in test mode (the harness exits after
// 500 ms; a modal would block input), but every snippet below compiles +
// links against the real perry/ui FFI surface.

const status = State("ready")

// ANCHOR: open-file
function pickFile(): void {
    openFileDialog((path: string) => {
        if (path.length > 0) {
            console.log(`Selected: ${path}`)
        } else {
            console.log("Open dialog cancelled")
        }
    })
}
// ANCHOR_END: open-file

// ANCHOR: open-folder
function pickFolder(): void {
    openFolderDialog((path: string) => {
        if (path.length > 0) {
            console.log(`Selected folder: ${path}`)
        }
    })
}
// ANCHOR_END: open-folder

// ANCHOR: save-file
function pickSaveTarget(): void {
    saveFileDialog((path: string) => {
        if (path.length > 0) {
            console.log(`Will save to: ${path}`)
        }
    }, "untitled", "txt")
}
// ANCHOR_END: save-file

// ANCHOR: alert
function showSimpleAlert(): void {
    alert("Operation Complete", "Your file has been saved successfully.")
}
// ANCHOR_END: alert

// ANCHOR: alert-with-buttons
function confirmDelete(): void {
    alertWithButtons(
        "Delete Item?",
        "This action cannot be undone.",
        ["Cancel", "Delete"],
        (index: number) => {
            if (index === 1) {
                console.log("user confirmed delete")
            }
        },
    )
}
// ANCHOR_END: alert-with-buttons

// ANCHOR: sheet
function showSheet(): void {
    let sheet = 0
    const body = VStack(16, [
        Text("Sheet Content"),
        Button("Close", () => sheetDismiss(sheet)),
    ])
    sheet = sheetCreate(body, 320, 200)
    sheetPresent(sheet)
}
// ANCHOR_END: sheet

App({
    title: "dialogs-snippets",
    width: 480,
    height: 320,
    body: VStack(12, [
        Text(`status: ${status.value}`),
        HStack(8, [
            Button("Open File", pickFile),
            Button("Open Folder", pickFolder),
            Button("Save As", pickSaveTarget),
        ]),
        HStack(8, [
            Button("Alert", showSimpleAlert),
            Button("Confirm", confirmDelete),
            Button("Sheet", showSheet),
        ]),
    ]),
})
