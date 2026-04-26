# Cryptography

Perry natively implements password hashing, JWT tokens, and Ethereum cryptography.

## bcrypt

```typescript
{{#include ../../examples/stdlib/crypto/snippets.ts:bcrypt}}
```

## Argon2

```typescript
{{#include ../../examples/stdlib/crypto/snippets.ts:argon2}}
```

## JSON Web Tokens

```typescript
{{#include ../../examples/stdlib/crypto/snippets.ts:jwt}}
```

## Node.js Crypto

```typescript
{{#include ../../examples/stdlib/crypto/snippets.ts:crypto-node}}
```

## Ethers

The `ethers` runtime exposes utility functions (`formatEther`, `formatUnits`,
`parseEther`, `parseUnits`, `getAddress`) but the higher-level
`Wallet.createRandom()` constructor flow shown below is not yet wired into
the LLVM backend. Track the follow-up at issue #199.

```text
import { ethers } from "ethers";

// Create a wallet
const wallet = ethers.Wallet.createRandom();
console.log(wallet.address);

// Sign a message
const signature = await wallet.signMessage("Hello, Ethereum!");
```

## Next Steps

- [Utilities](utilities.md)
- [Overview](overview.md) — All stdlib modules
