// demonstrates: per-API menu / context-menu / toolbar snippets shown in
// docs/src/ui/menus.md
// docs: docs/src/ui/menus.md
// platforms: macos, linux, windows

import {
    App,
    VStack,
    Text,
    State,
    menuCreate, menuAddItem, menuAddSeparator, menuAddSubmenu,
    menuAddItemWithShortcut,
    menuBarCreate, menuBarAddMenu, menuBarAttach,
    widgetSetContextMenu,
    toolbarCreate, toolbarAddItem, toolbarAttach,
    Window,
} from "perry/ui"

const status = State("ready")

// ANCHOR: menubar
// Menus are created independently, then attached. Build child menus first,
// then hand them to `menuBarAddMenu(bar, title, menu)`.
const menuBar = menuBarCreate()

// File menu
const fileMenu = menuCreate()
menuAddItemWithShortcut(fileMenu, "New",         "n", () => status.set("file/new"))
menuAddItemWithShortcut(fileMenu, "Open…",       "o", () => status.set("file/open"))
menuAddSeparator(fileMenu)
menuAddItemWithShortcut(fileMenu, "Save",        "s", () => status.set("file/save"))
menuAddItemWithShortcut(fileMenu, "Save As…",    "S", () => status.set("file/saveAs"))
menuBarAddMenu(menuBar, "File", fileMenu)

// Edit menu
const editMenu = menuCreate()
menuAddItemWithShortcut(editMenu, "Undo", "z", () => status.set("edit/undo"))
menuAddItemWithShortcut(editMenu, "Redo", "Z", () => status.set("edit/redo"))
menuAddSeparator(editMenu)
menuAddItemWithShortcut(editMenu, "Cut",   "x", () => status.set("edit/cut"))
menuAddItemWithShortcut(editMenu, "Copy",  "c", () => status.set("edit/copy"))
menuAddItemWithShortcut(editMenu, "Paste", "v", () => status.set("edit/paste"))
menuBarAddMenu(menuBar, "Edit", editMenu)

// Submenu: View → Zoom
const viewMenu = menuCreate()
const zoomSubmenu = menuCreate()
menuAddItemWithShortcut(zoomSubmenu, "Zoom In",     "+", () => status.set("zoom/in"))
menuAddItemWithShortcut(zoomSubmenu, "Zoom Out",    "-", () => status.set("zoom/out"))
menuAddItemWithShortcut(zoomSubmenu, "Actual Size", "0", () => status.set("zoom/reset"))
menuAddSubmenu(viewMenu, "Zoom", zoomSubmenu)
menuBarAddMenu(menuBar, "View", viewMenu)

menuBarAttach(menuBar)
// ANCHOR_END: menubar

// ANCHOR: context-menu
const label = Text("Right-click me")
const ctx = menuCreate()
menuAddItem(ctx, "Copy",   () => status.set("ctx/copy"))
menuAddItem(ctx, "Paste",  () => status.set("ctx/paste"))
menuAddSeparator(ctx)
menuAddItem(ctx, "Delete", () => status.set("ctx/delete"))
widgetSetContextMenu(label, ctx)
// ANCHOR_END: context-menu

// ANCHOR: toolbar
const toolbar = toolbarCreate()
toolbarAddItem(toolbar, "new",  "New",  () => status.set("tb/new"))
toolbarAddItem(toolbar, "save", "Save", () => status.set("tb/save"))
toolbarAddItem(toolbar, "run",  "Run",  () => status.set("tb/run"))

// `toolbarAttach(toolbar, window)` mounts onto a specific window.
const win = Window("Toolbar Demo", 800, 600)
toolbarAttach(toolbar, win as unknown as number)
// ANCHOR_END: toolbar

App({
    title: "menus-snippets",
    width: 800,
    height: 600,
    body: VStack(16, [
        Text(`status: ${status.value}`),
        label,
    ]),
})
