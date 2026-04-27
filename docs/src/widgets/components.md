# Widget Components & Modifiers

Available components and modifiers for widgets.

> **Status:** this page mixes (a) tiny fragments showing component shape —
> rendered as plain `text` because they're not standalone declarations and
> can't compile — and (b) one full verified Widget at the bottom that
> compile-links via
> [`docs/examples/widgets/snippets.ts`](https://github.com/PerryTS/perry/blob/main/docs/examples/widgets/snippets.ts).
> The doc-tests harness can't drive `--target ios-widget`/`android-widget`/
> `watchos-widget`/`wearos-tile` directly (each needs `--app-bundle-id` and a
> platform SDK — [#194](https://github.com/PerryTS/perry/issues/194)).
> Note also that the modifier parser in `crates/perry-hir/src/lower.rs`
> (`parse_modifiers_from_args` / `parse_single_modifier`) only reads modifiers
> from **inline option-object arguments** — e.g. `Text("hi", { font: "title",
> color: "red" })` and `VStack([...], { padding: 16 })`. Method-style chains
> like `Text("hi").font("title")` shown below parse without error but **drop
> the modifier** at HIR-lowering time
> ([#195](https://github.com/PerryTS/perry/issues/195)). The chain form is
> documented here because it reads naturally; the verified Complete Example
> at the bottom of the page uses the inline-options form that actually
> round-trips through the widget codegen path. The end-to-end reference is
> [`examples/widget_demo.ts`](https://github.com/PerryTS/perry/blob/main/examples/widget_demo.ts).

## Text

```text
Text("Hello, World!")
Text(`${entry.name}: ${entry.value}`)
```

### Text Modifiers

```text
const t = Text("Styled");
t.font("title");       // .title, .headline, .body, .caption, etc.
t.color("blue");       // Named color or hex
t.bold();
```

## Layout

### VStack

```text
VStack([
  Text("Top"),
  Text("Bottom"),
])
```

### HStack

```text
HStack([
  Text("Left"),
  Spacer(),
  Text("Right"),
])
```

### ZStack

```text
ZStack([
  Image("background"),
  Text("Overlay"),
])
```

## Spacer

Flexible space that expands to fill available room:

```text
HStack([
  Text("Left"),
  Spacer(),
  Text("Right"),
])
```

## Image

Display SF Symbols or asset images:

```text
Image("star.fill")           // SF Symbol
Image("cloud.sun.rain.fill") // SF Symbol
```

## ForEach

Iterate over array entry fields to render a list of components:

```text
ForEach(entry.items, (item) =>
  HStack([
    Text(item.name),
    Spacer(),
    Text(`${item.value}`),
  ])
)
```

## Divider

A visual separator line:

```text
VStack([
  Text("Above"),
  Divider(),
  Text("Below"),
])
```

## Label

A label with text and an SF Symbol icon:

```text
Label("Downloads", "arrow.down.circle")
Label(`${entry.count} items`, "folder.fill")
```

## Gauge

A circular or linear progress indicator:

```text
Gauge(entry.progress, 0, 100)       // value, min, max
Gauge(entry.battery, 0, 1.0)
```

## Modifiers

Widget components support SwiftUI-style modifiers. The chain forms shown below
parse but drop their modifiers at HIR-lowering time
([#195](https://github.com/PerryTS/perry/issues/195)) — use the inline
option-object form (`Text("hi", { font: "title" })`,
`VStack([...], { padding: 16 })`) for the form that actually reaches the
codegen, as in the [Complete Example](#complete-example) at the bottom of
this page.

### Font

```text
Text("Title").font("title")
Text("Body").font("body")
Text("Caption").font("caption")
```

### Color

```text
Text("Red text").color("red")
Text("Custom").color("#FF6600")
```

### Padding

```text
VStack([...]).padding(16)
```

### Frame

```text
widget.frame(width, height)
```

### Max Width

```text
widget.maxWidth("infinity")   // Expand to fill available width
```

### Minimum Scale Factor

Allow text to shrink to fit:

```text
Text("Long text").minimumScaleFactor(0.5)
```

### Container Background

Set background color for the widget container:

```text
VStack([...]).containerBackground("blue")
```

### Widget URL

Make the widget tappable with a deep link:

```text
VStack([...]).url("myapp://detail/123")
```

### Edge-Specific Padding

Apply padding to specific edges:

```text
VStack([...]).paddingEdge("top", 8)
VStack([...]).paddingEdge("horizontal", 16)
```

## Conditionals

Render different components based on entry data:

```text
render: (entry) =>
  VStack([
    entry.isOnline
      ? Text("Online").color("green")
      : Text("Offline").color("red"),
  ]),
```

## Complete Example

The full Widget below is the verified extract — it compile-links on the host
LLVM target and uses the inline-options modifier form that round-trips through
the codegen.

```typescript
{{#include ../../examples/widgets/snippets.ts:stats-widget}}
```

## Next Steps

- [Creating Widgets](creating-widgets.md) — Widget() API
- [Overview](overview.md) — Widget system overview
