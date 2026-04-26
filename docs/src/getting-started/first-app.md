# First Native App

Perry compiles declarative TypeScript UI code to native platform widgets. No
Electron, no WebView — real AppKit on macOS, UIKit on iOS, GTK4 on Linux,
Win32 on Windows. Every example on this page is a real source file under
`docs/examples/` that CI compiles and runs on every PR.

## A Simple Counter

```typescript
{{#include ../../examples/ui/counter.ts}}
```

Compile and run:

```bash
perry counter.ts -o counter
./counter
```

A native window opens with a label and two buttons. Clicking "Increment"
updates the count in real-time.

## How It Works

- **`App({ title, width, height, body })`** — Creates a native application window. `body` is the root widget.
- **`State(initialValue)`** — Creates reactive state. `.value` reads, `.set(v)` writes and triggers UI updates.
- **`VStack(spacing, [...])`** — Vertical stack layout (like SwiftUI's VStack or CSS flexbox column). The first arg is the gap in points between children.
- **`Text(string)`** — A text label. Template literals referencing `${state.value}` bind reactively.
- **`Button(label, onClick)`** — A native button with a click handler.

## A Todo App

```typescript
{{#include ../../examples/ui/state/todo_app.ts}}
```

`ForEach(count, render)` iterates by index — keep an item array and a count
state in sync, then read items via `array.value[i]` inside the closure. See
[State Management](../ui/state.md) for the full pattern.

## Cross-Platform

The same code runs on all 6 platforms:

```bash
# macOS (default)
perry app.ts -o app
./app

# iOS Simulator
perry app.ts -o app --target ios-simulator

# Web (compiles to WebAssembly + DOM bridge in a self-contained HTML file)
perry app.ts -o app --target web   # alias: --target wasm
open app.html

# Other platforms
perry app.ts -o app --target windows
perry app.ts -o app --target linux
perry app.ts -o app --target android
```

Each target compiles to the platform's native widget toolkit. See
[Platforms](../platforms/overview.md) for details.

## Adding Styling

Styling is applied via free functions that take the widget handle as their
first argument. Colors are RGBA floats in `[0.0, 1.0]` — divide a hex byte by
255 to convert (`0x33 / 255 ≈ 0.2`).

```typescript
{{#include ../../examples/getting-started/styled_counter.ts}}
```

See [Styling](../ui/styling.md) for all available style properties.

## Next Steps

- [Project Configuration](project-config.md) — Set up `package.json` for Perry projects
- [UI Overview](../ui/overview.md) — Complete guide to Perry's UI system
- [Widgets Reference](../ui/widgets.md) — All available widgets
- [State Management](../ui/state.md) — Reactive state and bindings
