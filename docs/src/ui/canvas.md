# Canvas

The `Canvas` widget provides a 2D drawing surface for custom graphics.

> **Availability**: `Canvas` is wired in the **JS / web / wasm** codegen path
> only. The native LLVM codegen used for `--target macos`, `--target linux`,
> and `--target windows` does not yet expose `Canvas`, so the snippets below
> are not part of the doc-tests harness. To run them, use `perry app.ts -o
> app --target web`.

The drawing API is **method-based** on the canvas handle (matching the FFI
shape — `perry_ui_canvas_set_fill_color(handle, r, g, b, a)` etc.). Colors
are RGBA floats in `[0.0, 1.0]`.

## Creating a Canvas

```text
import { Canvas } from "perry/ui";

const canvas = Canvas(400, 300);
canvas.setFillColor(1.0, 0.4, 0.0, 1.0);
canvas.fillRect(10, 10, 100, 80);
```

`Canvas(width, height)` creates a canvas widget; subsequent draw operations
are method calls on the returned handle.

## Drawing Shapes

### Rectangles

```text
canvas.setFillColor(1.0, 0.0, 0.0, 1.0);    // red
canvas.fillRect(10, 10, 100, 80);

canvas.setStrokeColor(0.0, 0.0, 1.0, 1.0);  // blue
canvas.setLineWidth(2);
canvas.strokeRect(150, 10, 100, 80);
```

### Lines

```text
canvas.setStrokeColor(0.0, 0.0, 0.0, 1.0);
canvas.setLineWidth(1);
canvas.beginPath();
canvas.moveTo(10, 10);
canvas.lineTo(200, 150);
canvas.stroke();
```

### Circles and Arcs

```text
canvas.setFillColor(0.0, 1.0, 0.0, 1.0);
canvas.beginPath();
canvas.arc(200, 150, 50, 0, Math.PI * 2);  // x, y, radius, startAngle, endAngle
canvas.fill();
```

### Text

```text
canvas.setFillColor(0.0, 0.0, 0.0, 1.0);
canvas.setFont("16px sans-serif");
canvas.fillText("Hello Canvas!", 50, 50);
```

## Platform Notes

| Platform | Implementation | Status |
|----------|---------------|--------|
| Web | HTML5 Canvas | Fully wired |
| WASM | HTML5 Canvas via JS bridge | Fully wired |
| macOS | Core Graphics (CGContext) | Runtime FFI exists; not yet exposed in LLVM codegen |
| iOS | Core Graphics (CGContext) | Runtime FFI exists; not yet exposed in LLVM codegen |
| Linux | Cairo | Runtime FFI exists; not yet exposed in LLVM codegen |
| Windows | GDI | Planned |
| Android | Canvas/Bitmap | Runtime FFI exists; not yet exposed in LLVM codegen |

## Next Steps

- [Widgets](widgets.md) — All available widgets
- [Animation](animation.md) — Animating widget properties
- [Styling](styling.md) — Widget styling
