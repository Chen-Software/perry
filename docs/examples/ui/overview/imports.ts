// demonstrates: the canonical perry/ui import surface (used by docs/src/ui/overview.md)
// docs: docs/src/ui/overview.md
// platforms: macos, linux, windows
// run: false

// `run: false` because this file is purely an import-surface witness — it
// imports every public name from perry/ui and references each one once so
// the linker doesn't dead-strip it. Compile-link is enough to verify that
// every name we tell users to import actually exists.

// ANCHOR: imports
import {
    // App lifecycle
    App, onActivate, onTerminate,

    // Widgets
    Text, Button, TextField, SecureField, TextArea,
    Toggle, Slider, ProgressView, Picker, ImageFile, ImageSymbol,

    // Layout
    VStack, HStack, ZStack, ScrollView, Spacer, Divider,
    NavStack, TabBar, LazyVStack, Section,
    VStackWithInsets, HStackWithInsets, SplitView, splitViewAddChild,

    // Layout control
    stackSetAlignment, stackSetDistribution, stackSetDetachesHidden,
    widgetMatchParentWidth, widgetMatchParentHeight, widgetSetHugging,
    widgetAddOverlay, widgetSetOverlayFrame,

    // State
    State, ForEach,

    // Dialogs
    openFileDialog, openFolderDialog, saveFileDialog,
    alert, alertWithButtons,
    sheetCreate, sheetPresent, sheetDismiss,

    // Menus
    menuCreate, menuAddItem, menuAddSeparator, menuAddSubmenu,
    menuBarCreate, menuBarAddMenu, menuBarAttach,
    widgetSetContextMenu,

    // Window
    Window,
} from "perry/ui"
// ANCHOR_END: imports

// Reference each import once so the verifier knows the symbol resolves.
// (Many of these need a widget handle; we just record the function value.)
const _refs: unknown[] = [
    App, onActivate, onTerminate,
    Text, Button, TextField, SecureField, TextArea,
    Toggle, Slider, ProgressView, Picker, ImageFile, ImageSymbol,
    VStack, HStack, ZStack, ScrollView, Spacer, Divider,
    NavStack, TabBar, LazyVStack, Section,
    VStackWithInsets, HStackWithInsets, SplitView, splitViewAddChild,
    stackSetAlignment, stackSetDistribution, stackSetDetachesHidden,
    widgetMatchParentWidth, widgetMatchParentHeight, widgetSetHugging,
    widgetAddOverlay, widgetSetOverlayFrame,
    State, ForEach,
    openFileDialog, openFolderDialog, saveFileDialog,
    alert, alertWithButtons,
    sheetCreate, sheetPresent, sheetDismiss,
    menuCreate, menuAddItem, menuAddSeparator, menuAddSubmenu,
    menuBarCreate, menuBarAddMenu, menuBarAttach,
    widgetSetContextMenu,
    Window,
]
console.log(`perry/ui surface refs: ${_refs.length}`)
