# Animation

Perry supports animating widget properties for smooth transitions. Every
snippet below is excerpted from
[`docs/examples/ui/animation/snippets.ts`](../../examples/ui/animation/snippets.ts) —
CI compiles and runs it on every PR.

`animateOpacity` and `animatePosition` are special: they're documented as
methods on the widget handle (the only methods perry/ui exposes), and the HIR
lowers them to `widgetAnimateOpacity` / `widgetAnimatePosition` calls under the
hood.

## Opacity Animation

```typescript
{{#include ../../examples/ui/animation/snippets.ts:opacity}}
```

## Position Animation

```typescript
{{#include ../../examples/ui/animation/snippets.ts:position}}
```

## Example: Fade-In Effect

When the first argument reads from a `State.value`, Perry auto-subscribes
the call to the state — toggling `visible` re-runs the animation.

```typescript
{{#include ../../examples/ui/animation/fade_in.ts}}
```

## Platform Notes

| Platform | Implementation |
|----------|---------------|
| macOS | NSAnimationContext / ViewPropertyAnimator |
| iOS | UIView.animate |
| Android | ViewPropertyAnimator |
| Windows | WM_TIMER-based animation |
| Linux | CSS transitions (GTK4) |
| Web | CSS transitions |

## Next Steps

- [Styling](styling.md) — Widget styling properties
- [Widgets](widgets.md) — All available widgets
- [Events](events.md) — User interaction
