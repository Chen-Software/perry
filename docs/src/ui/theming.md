# Theming

The `perry-styling` package provides a design system bridge for Perry UI — design token codegen and ergonomic styling helpers with compile-time platform detection.

## Installation

```bash
npm install perry-styling
```

## Design Token Codegen

Generate typed theme files from a JSON token definition:

```bash
perry-styling generate --tokens tokens.json --out src/theme.ts
```

### Token Format

```json
{
  "colors": {
    "primary": "#007AFF",
    "primary-dark": "#0A84FF",
    "background": "#FFFFFF",
    "background-dark": "#1C1C1E",
    "text": "#000000",
    "text-dark": "#FFFFFF"
  },
  "spacing": {
    "sm": 4,
    "md": 8,
    "lg": 16,
    "xl": 24
  },
  "radius": {
    "sm": 4,
    "md": 8,
    "lg": 16
  },
  "fontSize": {
    "body": 14,
    "heading": 20,
    "caption": 12
  },
  "borderWidth": {
    "thin": 1,
    "medium": 2
  }
}
```

Colors with a `-dark` suffix are used as the dark mode variant. If no dark variant is provided, the light value is used for both modes. Supported color formats: hex (`#RGB`, `#RRGGBB`, `#RRGGBBAA`), `rgb()`/`rgba()`, `hsl()`/`hsla()`, and CSS named colors.

## Generated Types

The codegen produces typed interfaces:

```text
interface PerryColor {
  r: number; g: number; b: number; a: number; // floats in [0, 1]
}

interface PerryTheme {
  light: { [key: string]: PerryColor };
  dark: { [key: string]: PerryColor };
  spacing: { [key: string]: number };
  radius: { [key: string]: number };
  fontSize: { [key: string]: number };
  borderWidth: { [key: string]: number };
}

interface ResolvedTheme {
  colors: { [key: string]: PerryColor };
  spacing: { [key: string]: number };
  radius: { [key: string]: number };
  fontSize: { [key: string]: number };
  borderWidth: { [key: string]: number };
}
```

## Theme Resolution

Resolve a theme at runtime based on the system's dark mode setting:

```text
import { getTheme } from "perry-styling";
import { theme } from "./theme"; // generated file

const resolved = getTheme(theme);
// resolved.colors.primary → the correct light/dark variant
```

`getTheme()` calls `isDarkMode()` from `perry/system` and returns the appropriate palette.

## Styling Helpers

Ergonomic functions for applying styles to widget handles. Perry's compiler
doesn't yet support passing `PerryColor` objects as parameters into user
functions, so the helpers take **flat primitives**: extract the channels at
the call site:

```text
import {
  applyBg, applyRadius, applyTextColor, applyFontSize, applyGradient,
} from "perry-styling";

const t = resolved;                    // your ResolvedTheme
const c = t.colors.text;               // a PerryColor
const bg = t.colors.background;
const start = t.colors.primary;
const end = t.colors["primary-dark"];

const label = Text("Hello");
applyTextColor(label, c.r, c.g, c.b, c.a);
applyFontSize(label, t.fontSize.heading);

const card = VStack(16, []);
applyBg(card, bg.r, bg.g, bg.b, bg.a);
applyRadius(card, t.radius.md);
applyGradient(card,
  start.r, start.g, start.b, start.a,
  end.r,   end.g,   end.b,   end.a,
  0,                                   // 0 = vertical, 1 = horizontal
);
```

### Available Helpers

| Function | Signature |
|----------|-----------|
| `applyBg(handle, r, g, b, a)` | Background color |
| `applyRadius(handle, radius)` | Corner radius |
| `applyTextColor(handle, r, g, b, a)` | Text color |
| `applyFontSize(handle, size)` | Font size (regular weight) |
| `applyFontBold(handle, size)` | Font size with bold weight |
| `applyFontFamily(handle, family)` | Font family |
| `applyWidth(handle, width)` | Fixed width |
| `applyTooltip(handle, text)` | Tooltip (no-op on iOS/Android) |
| `applyBorderColor(handle, r, g, b, a)` | Border color |
| `applyBorderWidth(handle, width)` | Border width |
| `applyEdgeInsets(handle, top, left, bottom, right)` | Edge insets (padding) |
| `applyOpacity(handle, alpha)` | Opacity |
| `applyGradient(handle, r1, g1, b1, a1, r2, g2, b2, a2, direction)` | Background gradient |
| `applyButtonBg(btn, r, g, b, a)` | Button background |
| `applyButtonTextColor(btn, r, g, b, a)` | Button text color |
| `applyButtonBordered(btn, bordered)` | Bordered button style (`true`/`false`) |

## Platform Constants

`perry-styling` exports compile-time platform constants based on the `__platform__` built-in:

```text
import { isMac, isIOS, isAndroid, isWindows, isLinux, isDesktop, isMobile } from "perry-styling";

if (isMobile) {
  applyFontSize(label, 16);
} else {
  applyFontSize(label, 14);
}
```

These are constant-folded by LLVM at compile time — dead branches are eliminated with zero runtime cost.

## Next Steps

- [Styling](styling.md) — Widget styling basics
- [State Management](state.md) — Reactive bindings
