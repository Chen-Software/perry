# Preferences

Store and retrieve user preferences using the platform's native storage.
Every snippet below is excerpted from
[`docs/examples/system/snippets.ts`](../../examples/system/snippets.ts) — CI
links it on every PR.

## Usage

`preferencesSet(key, value)` accepts strings **or** numbers and round-trips
them natively (NSUserDefaults / GSettings / Registry preserve the original
type). `preferencesGet(key)` returns `string | number | undefined`:

```typescript
{{#include ../../examples/system/snippets.ts:preferences}}
```

## Platform Storage

| Platform | Backend |
|----------|---------|
| macOS | NSUserDefaults |
| iOS | NSUserDefaults |
| Android | SharedPreferences |
| Windows | Windows Registry |
| Linux | GSettings / file-based |
| Web | localStorage |

Preferences persist across app launches. They are not encrypted — use
[Keychain](keychain.md) for sensitive data.

## Next Steps

- [Keychain](keychain.md) — Secure storage
- [Overview](overview.md) — All system APIs
