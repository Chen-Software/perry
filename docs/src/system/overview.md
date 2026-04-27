# System APIs Overview

The `perry/system` module provides access to platform-native system features:
preferences, secure storage, notifications, dark-mode detection, audio
capture, and app introspection. Every snippet below is excerpted from
[`docs/examples/system/snippets.ts`](../../examples/system/snippets.ts) — CI
links the file on every PR.

```typescript
{{#include ../../examples/system/snippets.ts:imports}}
```

## Available APIs

| Function | Description | Platforms |
|----------|------------|-----------|
| `openURL(url)` | Open URL in default browser/app | All |
| `isDarkMode()` | Check system dark mode | All |
| `getDeviceIdiom()` | `"phone"`, `"pad"`, `"mac"`, `"tv"`, … | All |
| `getDeviceModel()` | Device model identifier (e.g. `"iPhone13,4"`) | All |
| `preferencesSet(key, value)` | Store a preference (string or number) | All |
| `preferencesGet(key)` | Read a preference (returns `string | number | undefined`) | All |
| `keychainSave(key, value)` | Secure storage write | All |
| `keychainGet(key)` | Secure storage read | All |
| `keychainDelete(key)` | Secure storage remove | All |
| `notificationSend(title, body)` | Local notification | All |
| `notificationCancel(id)` | Cancel a scheduled notification | Apple |
| `notificationOnTap(cb)` | Handle banner taps | Apple |
| `notificationRegisterRemote(cb)` / `notificationOnReceive(cb)` | Push (APNs) | iOS, macOS |
| `audioStart()` / `audioStop()` | Microphone capture | All |
| `audioGetLevel()` / `audioGetPeak()` | RMS / peak amplitude (`0..1`) | All |
| `audioGetWaveform(n)` | Recent waveform samples for visualization | All |

> **Clipboard** lives in `perry/ui` (not `perry/system`): import `clipboardRead`
> and `clipboardWrite` from there.

## Quick Example

```typescript
{{#include ../../examples/system/snippets.ts:dark-mode}}
```

```typescript
{{#include ../../examples/system/snippets.ts:preferences}}
```

```typescript
{{#include ../../examples/system/snippets.ts:open-url}}
```

## Next Steps

- [Preferences](preferences.md)
- [Keychain](keychain.md)
- [Notifications](notifications.md)
- [Audio Capture](audio.md)
- [Other](other.md)
