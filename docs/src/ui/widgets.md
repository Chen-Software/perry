# Widgets

Perry provides native widgets that map to each platform's native controls.
Every example on this page is a real runnable program verified by CI
(`scripts/run_doc_tests.sh`) — the snippet you read is the same source that's
compiled and launched.

The widget API is **free functions**, not methods. A widget is a 64-bit
opaque handle; you pass it into helpers like `textSetFontSize(widget, 18)`
rather than calling `widget.setFontSize(18)`. That's the only shape perry/ui
supports — no fluent chain, no prototype methods.

## Text

Displays read-only text.

```typescript
{{#include ../../examples/ui/widgets/text.ts}}
```

Color is RGBA with each channel in `[0.0, 1.0]` — divide a hex byte by 255
(`0x33 / 255 ≈ 0.2`).

**Helpers:** `textSetString`, `textSetFontSize`, `textSetFontWeight`,
`textSetFontFamily`, `textSetColor`, `textSetWraps`, `textSetSelectable`.

Text widgets inside template literals with `state.value` update automatically
— perry detects the state read and rewires the widget to re-render on change.
See [State Management](state.md).

## Button

A clickable button.

```typescript
{{#include ../../examples/ui/widgets/button.ts}}
```

**Helpers:** `buttonSetTitle`, `buttonSetBordered`, `buttonSetImage`
(SF Symbol name on macOS/iOS), `buttonSetImagePosition`,
`buttonSetContentTintColor`, `buttonSetTextColor`, `widgetSetEnabled`.

## TextField

An editable single-line text input.

```typescript
{{#include ../../examples/ui/widgets/textfield.ts}}
```

`TextField(placeholder, onChange)` fires `onChange` as the user types. Pair
with `stateBindTextfield(state, field)` for two-way binding so programmatic
`state.set(…)` also updates the visible text.

**Helpers:** `textfieldSetString`, `textfieldSetFontSize`,
`textfieldSetTextColor`, `textfieldSetBackgroundColor`,
`textfieldSetBorderless`, `textfieldSetOnSubmit`, `textfieldSetOnFocus`,
`textfieldSetNextKeyView`.

## SecureField

A password input — identical signature to `TextField`, but text is masked.

```typescript
{{#include ../../examples/ui/widgets/secure_field.ts}}
```

## Toggle

A boolean on/off switch.

```typescript
{{#include ../../examples/ui/widgets/toggle.ts}}
```

## Slider

A numeric slider.

```typescript
{{#include ../../examples/ui/widgets/slider.ts}}
```

`Slider(min, max, onChange)` — `onChange` fires on every drag. Use
`stateBindSlider(state, slider)` for two-way binding.

## Picker

A dropdown selection control. Items are added with `pickerAddItem`.

```typescript
{{#include ../../examples/ui/widgets/picker.ts}}
```

## ImageFile / ImageSymbol

Two distinct constructors:

- `ImageFile(path)` — image from a file path
- `ImageSymbol(name)` — SF Symbol glyph name (macOS/iOS only)

```typescript
{{#include ../../examples/ui/widgets/image_symbol.ts}}
```

Use `widgetSetWidth(img, N)` / `widgetSetHeight(img, N)` to size the image.

## ProgressView

An indeterminate or determinate progress indicator.

```typescript
{{#include ../../examples/ui/widgets/progressview.ts}}
```

## TextArea

A multi-line text input. Same `(placeholder, onChange)` signature as
`TextField` but renders as a multi-line box.

```typescript
{{#include ../../examples/ui/widgets/textarea.ts}}
```

**Helpers:** `textareaSetString`.

## Sections

Group controls into labelled sections. Perry has no `Form()` widget — use a
`VStack` of `Section(title)`s and attach children via `widgetAddChild`.

```typescript
{{#include ../../examples/ui/widgets/sections.ts}}
```

## Platform-specific widgets

These exist only on specific platforms and aren't verified by the
cross-platform doc-tests:

- **`Table(rows, cols, renderer)`** — macOS only. A data table with rows,
  columns, and a cell renderer.
- **`QRCode(data, size)`** — macOS only. Renders a QR code.
- **`Canvas(width, height, draw)`** — all desktop platforms. A drawing
  surface; see [Canvas](canvas.md).
- **`CameraView()`** — iOS only (other platforms planned). See
  [Camera](camera.md).

These are linked from their own pages with platform-specific examples.

## Common widget helpers

Every widget handle accepts these:

| Helper | Description |
|---|---|
| `widgetSetWidth(w, n)` / `widgetSetHeight(w, n)` | Explicit size in points |
| `widgetSetBackgroundColor(w, r, g, b, a)` | RGBA in [0, 1] |
| `setCornerRadius(w, r)` | Rounded corners in points |
| `widgetSetOpacity(w, alpha)` | Opacity in [0, 1] |
| `widgetSetEnabled(w, flag)` | `0` disables, `1` enables |
| `widgetSetHidden(w, flag)` | `0` visible, `1` hidden |
| `widgetSetTooltip(w, text)` | Tooltip on hover (desktop only) |
| `widgetSetOnClick(w, cb)` | Click handler |
| `widgetSetOnHover(w, cb)` | Hover enter/leave (desktop only) |
| `widgetSetOnDoubleClick(w, cb)` | Double-click handler |
| `widgetSetEdgeInsets(w, top, left, bottom, right)` | Padding around contents |
| `widgetSetBorderColor(w, r, g, b, a)` / `widgetSetBorderWidth(w, n)` | Border |
| `widgetAddChild(parent, child)` | Attach a child to a container |
| `widgetSetContextMenu(w, menu)` | Right-click menu |

See [Styling](styling.md) and [Events](events.md) for deeper coverage.

## Next Steps

- [Layout](layout.md) — Arranging widgets with stacks and containers
- [Styling](styling.md) — Colors, fonts, borders
- [State Management](state.md) — Reactive bindings
