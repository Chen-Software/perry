# Type System

Perry erases types at compile time, similar to how `tsc` removes type annotations when emitting JavaScript. However, Perry also performs type inference to generate efficient native code.

## Type Inference

Perry infers types from expressions without requiring annotations:

```typescript
{{#include ../../examples/language/type_system.ts:inference-basics}}
```

Inference works through:
- **Literal values**: `5` → `number`, `"hi"` → `string`
- **Binary operations**: `a + b` where both are numbers → `number`
- **Variable propagation**: if `x` is `number`, then `let y = x` is `number`
- **Method returns**: `"hello".trim()` → `string`, `[1,2].length` → `number`
- **Function returns**: user-defined function return types are propagated to callers

```typescript
{{#include ../../examples/language/type_system.ts:inference-function}}
```

## Type Annotations

Standard TypeScript annotations work:

```typescript
{{#include ../../examples/language/type_system.ts:annotations}}
```

## Utility Types

Common TypeScript utility types are erased at compile time (they don't affect code generation):

```typescript
{{#include ../../examples/language/type_system.ts:utility-types}}
```

These are all recognized and erased — they won't cause compilation errors.

## Generics

Generic type parameters are erased:

```typescript
{{#include ../../examples/language/type_system.ts:generics}}
```

At runtime, all values are NaN-boxed — the generic parameter doesn't affect code generation.

## Type Checking with `--type-check`

For stricter type checking, Perry can integrate with Microsoft's TypeScript checker:

```bash
perry file.ts --type-check
```

This resolves cross-file types, interfaces, and generics via an IPC protocol. It falls back gracefully if the type checker is not installed.

Without `--type-check`, Perry relies on its own inference engine, which handles common patterns but doesn't perform full TypeScript type checking.

## Union and Intersection Types

Union types are recognized syntactically but don't affect code generation:

```typescript
{{#include ../../examples/language/type_system.ts:union-narrowing}}
```

Use `typeof` checks for runtime type narrowing.

## Type Guards

```typescript
{{#include ../../examples/language/type_system.ts:type-guards}}
```

The `value is string` annotation is erased, but the `typeof` check works at runtime.

## Next Steps

- [Supported Features](supported-features.md) — Complete feature list
- [Limitations](limitations.md) — What's not supported
