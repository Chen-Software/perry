# Styling

Perry widgets support native styling properties that map to each platform's styling system.

## Coming from CSS

Perry's layout model is closer to SwiftUI or Flutter than CSS. If you're coming from web development, here's how concepts translate:

| CSS | Perry |
|-----|-------|
| `display: flex; flex-direction: column` | `VStack(spacing, [...])` |
| `display: flex; flex-direction: row` | `HStack(spacing, [...])` |
| `justify-content` | `stackSetDistribution(stack, mode)` + `Spacer()` |
| `align-items` | `stackSetAlignment(stack, value)` |
| `position: absolute` | `widgetAddOverlay` + `widgetSetOverlayFrame` |
| `width: 100%` | `widgetMatchParentWidth(widget)` |
| `padding: 10px 20px` | `widgetSetEdgeInsets(w, 10, 20, 10, 20)` |
| `gap: 16px` | `VStack(16, [...])` — first argument is the gap |
| CSS variables / design tokens | `perry-styling` package ([Theming](theming.md)) |
| `opacity` | `widgetSetOpacity(w, value)` |
| `border-radius` | `setCornerRadius(w, value)` |

See [Layout](layout.md) for full details on alignment, distribution, overlays, and split views.

Perry's styling API is a flat set of free functions: `widgetSet*`, `textSet*`,
`buttonSet*`. They take the widget handle as the first argument. Colors are
RGBA floats in `[0.0, 1.0]` (divide each hex byte by 255 — `0xFF3B30` →
`(1.0, 0.231, 0.188, 1.0)`).

Every snippet below is excerpted from
[`docs/examples/ui/styling/snippets.ts`](../../examples/ui/styling/snippets.ts),
which CI compiles and runs on every PR — so the API drawn here is always the
API the compiler accepts.

```typescript
{{#include ../../examples/ui/styling/snippets.ts:imports}}
```

## Colors

```typescript
{{#include ../../examples/ui/styling/snippets.ts:colors}}
```

## Fonts

```typescript
{{#include ../../examples/ui/styling/snippets.ts:fonts}}
```

Use `"monospaced"` for the system monospaced font.

## Corner Radius

```typescript
{{#include ../../examples/ui/styling/snippets.ts:corner-radius}}
```

## Borders

```typescript
{{#include ../../examples/ui/styling/snippets.ts:borders}}
```

> **GTK4 note:** `widgetSetBorderColor` / `widgetSetBorderWidth` are macOS, iOS,
> and Windows only — GTK4 styles borders through CSS rather than per-widget
> properties.

## Padding and Insets

```typescript
{{#include ../../examples/ui/styling/snippets.ts:padding}}
```

## Sizing

```typescript
{{#include ../../examples/ui/styling/snippets.ts:sizing}}
```

## Opacity

```typescript
{{#include ../../examples/ui/styling/snippets.ts:opacity}}
```

## Background Gradient

```typescript
{{#include ../../examples/ui/styling/snippets.ts:gradient}}
```

## Control Size

```typescript
{{#include ../../examples/ui/styling/snippets.ts:control-size}}
```

> **macOS**: Maps to `NSControl.ControlSize`. Other platforms may interpret differently.

## Tooltips

```typescript
{{#include ../../examples/ui/styling/snippets.ts:tooltip}}
```

> **macOS/Windows/Linux**: Native tooltips. **iOS/Android**: No tooltip support. **Web**: HTML `title` attribute.

## Enabled/Disabled

```typescript
{{#include ../../examples/ui/styling/snippets.ts:enabled}}
```

## Complete Styling Example

```typescript
{{#include ../../examples/ui/styling/counter_card.ts}}
```

## Composing Styles

Reduce repetition by creating helper functions:

```typescript
{{#include ../../examples/ui/styling/snippets.ts:card-helper}}
```

For larger apps, use the `perry-styling` package to define design tokens in JSON and generate a typed theme file. See [Theming](theming.md) for the full workflow.

## Next Steps

- [Widgets](widgets.md) — All available widgets
- [Layout](layout.md) — Layout containers
- [Animation](animation.md) — Animate style changes
