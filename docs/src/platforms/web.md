# Web

Perry can compile TypeScript UI apps to self-contained HTML files using `--target web`.

## Building

```bash
perry app.ts -o app --target web
open app.html   # Opens in your default browser
```

The output is a single `.html` file containing all JavaScript and CSS — no build step, no dependencies.

## How It Works

Instead of using Cranelift for native code generation, the `--target web` flag uses the `perry-codegen-js` crate to emit JavaScript from HIR. The output is a self-contained HTML file with:

- Inline JavaScript (your compiled TypeScript)
- A web runtime that maps `perry/ui` widgets to DOM elements
- CSS for layout (flexbox) and styling

The web target skips Cranelift, inlining, generator transforms, and closure conversion — JavaScript engines handle these natively.

## UI Mapping

Perry widgets map to HTML elements:

| Perry Widget | HTML Element |
|-------------|-------------|
| Text | `<span>` |
| Button | `<button>` |
| TextField | `<input type="text">` |
| SecureField | `<input type="password">` |
| Toggle | `<input type="checkbox">` |
| Slider | `<input type="range">` |
| Picker | `<select>` |
| ProgressView | `<progress>` |
| Image | `<img>` |
| VStack | `<div>` (flexbox column) |
| HStack | `<div>` (flexbox row) |
| ZStack | `<div>` (position: relative/absolute) |
| ScrollView | `<div>` (overflow: auto) |
| Canvas | `<canvas>` (2D context) |
| Table | `<table>` |

## Web-Specific Features

- **Clipboard**: `navigator.clipboard` API
- **Notifications**: Web Notification API
- **Dark mode**: `prefers-color-scheme` media query
- **Keychain**: localStorage (not truly secure — use for preferences only)
- **Dialogs**: `<input type="file">`, `alert()`, modal `<div>`
- **Keyboard shortcuts**: DOM keyboard event listeners
- **Multi-window**: Floating `<div>` panels

## Minification and Obfuscation

Web output is automatically minified. Perry's Rust-native JS minifier:

- Strips comments and collapses whitespace
- Mangles local variable, parameter, and non-exported function names (e.g., `myVariable` → `a`)
- Compresses the web runtime from ~3,337 lines to ~177

This is enabled by default for `--target web`. You can also use `--minify` explicitly with other targets.

## Limitations

- No file system access (browser sandbox)
- No database connections
- No background processes
- localStorage instead of secure keychain
- Single-page — no native app lifecycle

## Example

```typescript
import { App, Text, Button, VStack, State } from "perry/ui";

const count = State(0);

App("Web Counter", () =>
  VStack([
    Text(`Count: ${count.get()}`),
    Button("+1", () => count.set(count.get() + 1)),
  ])
);
```

```bash
perry counter.ts -o counter --target web
# Produces counter.html — open in any browser
```

## Next Steps

- [Platform Overview](overview.md) — All platforms
- [UI Overview](../ui/overview.md) — UI system
