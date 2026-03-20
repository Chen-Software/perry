# Widget Components & Modifiers

Available components and modifiers for widgets.

## Text

```typescript
Text("Hello, World!")
Text(`${entry.name}: ${entry.value}`)
```

### Text Modifiers

```typescript
const t = Text("Styled");
t.font("title");       // .title, .headline, .body, .caption, etc.
t.color("blue");       // Named color or hex
t.bold();
```

## Layout

### VStack

```typescript
VStack([
  Text("Top"),
  Text("Bottom"),
])
```

### HStack

```typescript
HStack([
  Text("Left"),
  Spacer(),
  Text("Right"),
])
```

### ZStack

```typescript
ZStack([
  Image("background"),
  Text("Overlay"),
])
```

## Spacer

Flexible space that expands to fill available room:

```typescript
HStack([
  Text("Left"),
  Spacer(),
  Text("Right"),
])
```

## Image

Display SF Symbols or asset images:

```typescript
Image("star.fill")           // SF Symbol
Image("cloud.sun.rain.fill") // SF Symbol
```

## ForEach

Iterate over array entry fields to render a list of components:

```typescript
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

```typescript
VStack([
  Text("Above"),
  Divider(),
  Text("Below"),
])
```

## Label

A label with text and an SF Symbol icon:

```typescript
Label("Downloads", "arrow.down.circle")
Label(`${entry.count} items`, "folder.fill")
```

## Gauge

A circular or linear progress indicator:

```typescript
Gauge(entry.progress, 0, 100)       // value, min, max
Gauge(entry.battery, 0, 1.0)
```

## Modifiers

Widget components support SwiftUI-style modifiers:

### Font

```typescript
Text("Title").font("title")
Text("Body").font("body")
Text("Caption").font("caption")
```

### Color

```typescript
Text("Red text").color("red")
Text("Custom").color("#FF6600")
```

### Padding

```typescript
VStack([...]).padding(16)
```

### Frame

```typescript
widget.frame(width, height)
```

### Max Width

```typescript
widget.maxWidth("infinity")   // Expand to fill available width
```

### Minimum Scale Factor

Allow text to shrink to fit:

```typescript
Text("Long text").minimumScaleFactor(0.5)
```

### Container Background

Set background color for the widget container:

```typescript
VStack([...]).containerBackground("blue")
```

### Widget URL

Make the widget tappable with a deep link:

```typescript
VStack([...]).url("myapp://detail/123")
```

### Edge-Specific Padding

Apply padding to specific edges:

```typescript
VStack([...]).paddingEdge("top", 8)
VStack([...]).paddingEdge("horizontal", 16)
```

## Conditionals

Render different components based on entry data:

```typescript
render: (entry) =>
  VStack([
    entry.isOnline
      ? Text("Online").color("green")
      : Text("Offline").color("red"),
  ]),
```

## Complete Example

```typescript
import { Widget, Text, VStack, HStack, Image, Spacer } from "perry/widget";

Widget({
  kind: "StatsWidget",
  displayName: "Stats",
  description: "Shows daily stats",
  entryFields: {
    steps: "number",
    calories: "number",
    distance: "string",
  },
  render: (entry) =>
    VStack([
      HStack([
        Image("figure.walk"),
        Text("Daily Stats").font("headline"),
      ]),
      Spacer(),
      HStack([
        VStack([
          Text(`${entry.steps}`).font("title").bold(),
          Text("steps").font("caption").color("gray"),
        ]),
        Spacer(),
        VStack([
          Text(`${entry.calories}`).font("title").bold(),
          Text("cal").font("caption").color("gray"),
        ]),
        Spacer(),
        VStack([
          Text(entry.distance).font("title").bold(),
          Text("km").font("caption").color("gray"),
        ]),
      ]),
    ]).padding(16),
});
```

## Next Steps

- [Creating Widgets](creating-widgets.md) — Widget() API
- [Overview](overview.md) — Widget system overview
