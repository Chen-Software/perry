# parallelFilter

**Signature**: `parallelFilter<T>(data: T[], predicate: (item: T) => boolean): T[]` — imported from `perry/thread`.

Filters an array in parallel across all available CPU cores. Returns a new array containing only the elements where the predicate returned a truthy value. Order is preserved.

## Basic Usage

```typescript
{{#include ../../examples/runtime/thread_snippets.ts:parallel-filter-basic}}
```

## How It Works

```
Input: [a, b, c, d, e, f, g, h]     (8 elements, 4 CPU cores)

  Core 1: [a, b] → test → [a]       (b filtered out)
  Core 2: [c, d] → test → [c, d]    (both kept)
  Core 3: [e, f] → test → []        (both filtered out)
  Core 4: [g, h] → test → [h]       (g filtered out)

Output: [a, c, d, h]                 (concatenated in original order)
```

Each core independently tests its chunk of elements. Results are merged in the original element order after all threads complete.

## Why Not Just Use `.filter()`?

Regular `.filter()` runs on a single thread. For large arrays with expensive predicates, `parallelFilter` distributes the work:

```typescript
{{#include ../../examples/runtime/thread_snippets.ts:parallel-filter-vs-filter}}
```

The tradeoff: `parallelFilter` has overhead from copying values between threads. Use it when the predicate is expensive enough to justify that cost.

## Capturing Variables

Like `parallelMap`, the predicate can capture outer variables. Captures are deep-copied to each thread:

```typescript
{{#include ../../examples/runtime/thread_snippets.ts:parallel-filter-capture}}
```

Mutable variables cannot be captured — the compiler rejects this at compile time.

## Examples

### Filtering Large Datasets

```typescript
{{#include ../../examples/runtime/thread_snippets.ts:parallel-filter-large}}
```

### Combined with parallelMap

```typescript
{{#include ../../examples/runtime/thread_snippets.ts:parallel-filter-combined}}
```

### Predicate with Heavy Computation

```typescript
{{#include ../../examples/runtime/thread_snippets.ts:parallel-filter-heavy}}
```

## Performance

Use `parallelFilter` when:
- The array has many elements (hundreds or more)
- The predicate function does meaningful work per element
- You need to keep the UI responsive during filtering

For trivial predicates on small arrays, regular `.filter()` is faster (no threading overhead).
