# Menus

Perry supports native menu bars, context menus, and toolbars across all
platforms. Every snippet below is excerpted from
[`docs/examples/ui/menus/snippets.ts`](../../examples/ui/menus/snippets.ts) —
CI compiles and runs it on every PR.

The menu API is **handle-based** and free-function: build menus with
`menuCreate()`, fill them with `menuAddItem` / `menuAddItemWithShortcut`, and
attach them with `menuBarAddMenu(bar, title, menu)`. Submenus go through
`menuAddSubmenu(parent, title, submenu)`.

## Menu Bar

```typescript
{{#include ../../examples/ui/menus/snippets.ts:menubar}}
```

### Menu Bar Functions

| Function | Description |
|----------|-------------|
| `menuBarCreate()` | Create a new (empty) menu bar |
| `menuCreate()` | Create a new menu — used as a child of the bar or as a submenu |
| `menuBarAddMenu(bar, title, menu)` | Attach a top-level menu under `title` |
| `menuAddItem(menu, label, callback)` | Append an item without a shortcut |
| `menuAddItemWithShortcut(menu, label, shortcut, callback)` | Append an item with a keyboard shortcut |
| `menuAddSeparator(menu)` | Append a horizontal separator line |
| `menuAddSubmenu(parent, title, submenu)` | Nest a previously-created menu under a label |
| `menuBarAttach(bar)` | Install the bar as the application's main menu |

### Keyboard Shortcuts

The third argument to `menuAddItemWithShortcut` is the shortcut key:

| Shortcut | macOS | Other |
|----------|-------|-------|
| `"n"` | Cmd+N | Ctrl+N |
| `"S"` | Cmd+Shift+S | Ctrl+Shift+S |
| `"+"` | Cmd++ | Ctrl++ |

Uppercase letters imply Shift.

## Context Menus

Right-click menus are attached to widgets via `widgetSetContextMenu(widget, menu)`.
Build the menu the same way as a menu-bar entry, then bind it:

```typescript
{{#include ../../examples/ui/menus/snippets.ts:context-menu}}
```

## Toolbar

Add a toolbar to a window. `toolbarAddItem` takes an *identifier* (used by
AppKit to deduplicate items) and a *label*:

```typescript
{{#include ../../examples/ui/menus/snippets.ts:toolbar}}
```

## Platform Notes

| Platform | Menu Bar | Context Menu | Toolbar |
|----------|----------|-------------|---------|
| macOS | NSMenu | NSMenu | NSToolbar |
| iOS | — (no menu bar) | UIMenu | UIToolbar |
| Windows | HMENU/SetMenu | — | Horizontal layout |
| Linux | GMenu/set_menubar | — | HeaderBar |
| Web | DOM | DOM | DOM |

> **iOS**: Menu bars are not applicable. Use toolbar and navigation patterns instead.

## Next Steps

- [Events](events.md) — Keyboard shortcuts and interactions
- [Dialogs](dialogs.md) — File dialogs and alerts
- [Layout](layout.md) — Toolbar and navigation patterns
