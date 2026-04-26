# Keychain

Securely store sensitive data like tokens, passwords, and API keys using the
platform's secure storage. Every snippet below is excerpted from
[`docs/examples/system/snippets.ts`](../../examples/system/snippets.ts) — CI
links it on every PR.

## Usage

```typescript
{{#include ../../examples/system/snippets.ts:keychain}}
```

The free-function API is `keychainSave(key, value)`, `keychainGet(key)` (returns
the stored string, or an empty string if the key isn't present), and
`keychainDelete(key)`.

## Platform Storage

| Platform | Backend |
|----------|---------|
| macOS | Security.framework (Keychain) |
| iOS | Security.framework (Keychain) |
| Android | Android Keystore |
| Windows | Windows Credential Manager (CredWrite/CredRead/CredDelete) |
| Linux | libsecret |
| Web | localStorage (not truly secure) |

> **Web**: The web platform uses `localStorage`, which is not encrypted. For
> web apps handling sensitive data, consider server-side storage instead.

## Next Steps

- [Preferences](preferences.md) — Non-sensitive preferences
- [Notifications](notifications.md) — Local notifications
- [Overview](overview.md) — All system APIs
